//! 密钥迁移工具 v1 -> v2
//!
//! 将 CIS v1 格式的加密密钥迁移到 v2 格式
//!
//! 使用方法:
//! ```bash
//! cargo run --bin migrate_keys -- --from ~/.cis/keys/v1 --to ~/.cis/keys/v2
//! cargo run --bin migrate_keys -- --batch --backup ~/.cis/backup/
//! ```

use anyhow::{Context, Result};
use cis_core::memory::encryption::MemoryEncryption as EncryptionV1;
use cis_core::memory::encryption_v2::{EncryptionKeyV2, MemoryEncryptionV2 as EncryptionV2};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "migrate_keys")]
#[command(about = "Migrate CIS encryption keys from v1 to v2 format", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 迁移单个密钥文件
    Migrate {
        /// 源密钥路径 (v1 格式)
        #[arg(short, long)]
        from: PathBuf,

        /// 目标路径 (v2 格式)
        #[arg(short, long)]
        to: PathBuf,

        /// 节点唯一 ID (用于派生 v2 密钥)
        #[arg(short, long)]
        node_id: String,

        /// 节点密钥 (32 字节)
        #[arg(short, long)]
        node_key: PathBuf,
    },

    /// 批量迁移
    Batch {
        /// 源目录
        #[arg(short, long)]
        from_dir: PathBuf,

        /// 目标目录
        #[arg(short, long)]
        to_dir: PathBuf,

        /// 备份目录
        #[arg(short, long)]
        backup: Option<PathBuf>,

        /// 节点密钥文件
        #[arg(short, long)]
        node_key: PathBuf,

        /// 节点 ID 映射文件 (JSON)
        #[arg(short, long)]
        node_id_map: Option<PathBuf>,
    },

    /// 验证迁移结果
    Verify {
        /// v2 密钥路径
        #[arg(short, long)]
        key_path: PathBuf,

        /// 测试数据文件 (使用 v1 密钥加密)
        #[arg(short, long)]
        test_data: PathBuf,
    },
}

fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Migrate {
            from,
            to,
            node_id,
            node_key,
        } => {
            migrate_single_key(from, to, &node_id, node_key)?;
        }
        Commands::Batch {
            from_dir,
            to_dir,
            backup,
            node_key,
            node_id_map,
        } => {
            migrate_batch(from_dir, to_dir, backup, node_key, node_id_map)?;
        }
        Commands::Verify {
            key_path,
            test_data,
        } => {
            verify_migration(key_path, test_data)?;
        }
    }

    Ok(())
}

/// 迁移单个密钥
fn migrate_single_key(
    from_path: PathBuf,
    to_path: PathBuf,
    node_id: &str,
    node_key_path: PathBuf,
) -> Result<()> {
    info!("开始迁移密钥: {:?}", from_path);

    // 读取节点密钥
    let node_key = fs::read(&node_key_path)
        .context("无法读取节点密钥文件")?;

    if node_key.len() != 32 {
        anyhow::bail!("节点密钥必须是 32 字节");
    }

    // 创建 v1 加密器
    let v1_encryption = EncryptionV1::from_node_key(&node_key);

    // 创建 v2 密钥和加密器
    let v2_key = EncryptionKeyV2::from_node_key_v2(&node_key, node_id.as_bytes())
        .context("无法派生 v2 密钥")?;

    let v2_encryption = EncryptionV2::from_key(&v2_key);

    // 保存 v2 密钥
    v2_key.save(&to_path)
        .context("无法保存 v2 密钥")?;

    info!("v2 密钥已保存到: {:?}", to_path);

    // 如果源目录包含加密数据，可以选择迁移
    // 这里只是示例，实际需要根据数据存储结构实现

    info!("迁移完成: {:?} -> {:?}", from_path, to_path);
    Ok(())
}

/// 批量迁移
fn migrate_batch(
    from_dir: PathBuf,
    to_dir: PathBuf,
    backup_dir: Option<PathBuf>,
    node_key_path: PathBuf,
    node_id_map: Option<PathBuf>,
) -> Result<()> {
    info!("开始批量迁移: {:?} -> {:?}", from_dir, to_dir);

    // 读取节点密钥
    let node_key = fs::read(&node_key_path)
        .context("无法读取节点密钥文件")?;

    if node_key.len() != 32 {
        anyhow::bail!("节点密钥必须是 32 字节");
    }

    // 创建目标目录
    fs::create_dir_all(&to_dir)
        .context("无法创建目标目录")?;

    // 创建备份目录
    if let Some(backup) = &backup_dir {
        fs::create_dir_all(backup)
            .context("无法创建备份目录")?;
    }

    // 扫描源目录中的密钥文件
    let entries = fs::read_dir(&from_dir)
        .context("无法读取源目录")?;

    let mut migrated = 0;
    let mut failed = 0;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // 跳过目录
        if path.is_dir() {
            continue;
        }

        // 只处理 .key 或 .json 文件
        let ext = path.extension().and_then(|s| s.to_str());
        if !matches!(ext, Some("key") | Some("json")) {
            continue;
        }

        info!("处理文件: {:?}", path);

        // 确定节点 ID
        let node_id = if let Some(map_path) = &node_id_map {
            // 从映射文件读取
            let node_id = extract_node_id_from_map(&path, map_path)?;
            node_id
        } else {
            // 从文件名提取
            extract_node_id_from_filename(&path)?
        };

        // 执行迁移
        let to_path = to_dir.join(format!("{}_v2.key", node_id));
        match migrate_single_key(path.clone(), to_path, &node_id, node_key_path.clone()) {
            Ok(_) => {
                migrated += 1;

                // 备份原文件
                if let Some(backup) = &backup_dir {
                    let backup_path = backup.join(path.file_name().unwrap());
                    fs::copy(&path, &backup_path)
                        .with_context(|| format!("无法备份文件: {:?}", path))?;
                    info!("已备份到: {:?}", backup_path);
                }
            }
            Err(e) => {
                failed += 1;
                warn!("迁移失败: {:?} - {}", path, e);
            }
        }
    }

    info!("批量迁移完成:");
    info!("  成功: {}", migrated);
    info!("  失败: {}", failed);

    if failed > 0 {
        anyhow::bail!("部分密钥迁移失败");
    }

    Ok(())
}

/// 从文件名提取节点 ID
fn extract_node_id_from_filename(path: &Path) -> Result<String> {
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("无效的文件名"))?;

    // 移除可能的后缀
    let node_id = filename.trim_end_matches("_v1")
        .trim_end_matches("_key")
        .trim_end_matches("_encrypted")
        .to_string();

    Ok(node_id)
}

/// 从映射文件提取节点 ID
fn extract_node_id_from_map(path: &Path, map_path: &Path) -> Result<String> {
    let filename = path.file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("无效的文件名"))?;

    // 读取映射文件 (JSON 格式: {"file1.key": "node1", ...})
    let content = fs::read_to_string(map_path)
        .context("无法读取节点 ID 映射文件")?;

    let map: serde_json::Map<String, String> = serde_json::from_str(&content)
        .context("无法解析节点 ID 映射文件")?;

    map.get(filename)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("未找到文件 {} 的节点 ID 映射", filename))
}

/// 验证迁移结果
fn verify_migration(v2_key_path: PathBuf, test_data_path: PathBuf) -> Result<()> {
    info!("验证迁移: {:?}", v2_key_path);

    // 加载 v2 密钥
    let v2_key = EncryptionKeyV2::load(&v2_key_path)
        .context("无法加载 v2 密钥")?;

    let v2_encryption = EncryptionV2::from_key(&v2_key);

    // 读取测试数据（使用 v1 密钥加密）
    let test_data = fs::read(&test_data_path)
        .context("无法读取测试数据")?;

    // 使用 v1 解密
    let node_key = fs::read("/path/to/node_key")  // 需要从某处获取
        .context("无法读取节点密钥")?;

    let v1_encryption = EncryptionV1::from_node_key(&node_key);
    let decrypted_v1 = v1_encryption.decrypt(&test_data)
        .context("v1 解密失败")?;

    // 使用 v2 解密
    let decrypted_v2 = v2_encryption.decrypt(&test_data)
        .context("v2 解密失败")?;

    // 比较结果
    if decrypted_v1 == decrypted_v2 {
        info!("验证成功: v1 和 v2 解密结果一致");
        Ok(())
    } else {
        anyhow::bail!("验证失败: v1 和 v2 解密结果不一致");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_node_id_from_filename() {
        let cases = vec![
            ("node1.key", "node1"),
            ("node2_v1.key", "node2"),
            ("node3_key.key", "node3"),
            ("node4_encrypted.key", "node4"),
        ];

        for (filename, expected) in cases {
            let path = PathBuf::from(filename);
            let result = extract_node_id_from_filename(&path).unwrap();
            assert_eq!(result, expected, "Failed for: {}", filename);
        }
    }

    #[test]
    fn test_migrate_single_key_integration() {
        // 创建临时目录
        let temp_dir = TempDir::new().unwrap();
        let from_path = temp_dir.path().join("test_v1.key");
        let to_path = temp_dir.path().join("test_v2.key");
        let node_key_path = temp_dir.path().join("node_key");

        // 创建节点密钥
        let node_key = [0u8; 32];
        fs::write(&node_key_path, &node_key).unwrap();

        // 创建假的 v1 密钥文件（仅用于测试路径处理）
        fs::write(&from_path, b"fake v1 key").unwrap();

        // 注意：这个测试会失败，因为 v1 密钥文件格式不正确
        // 实际需要使用真实的 v1 密钥格式
        let result = migrate_single_key(
            from_path,
            to_path,
            "test-node",
            node_key_path,
        );

        // 期望失败（因为 v1 密钥格式无效）
        assert!(result.is_err());
    }

    #[test]
    fn test_node_id_map() {
        let temp_dir = TempDir::new().unwrap();
        let map_path = temp_dir.path().join("map.json");

        // 创建映射文件
        let map_content = serde_json::json!({
            "file1.key": "node1",
            "file2.key": "node2"
        });
        fs::write(&map_path, map_content.to_string()).unwrap();

        // 测试提取
        let path1 = PathBuf::from("file1.key");
        let node_id1 = extract_node_id_from_map(&path1, &map_path).unwrap();
        assert_eq!(node_id1, "node1");

        let path2 = PathBuf::from("file2.key");
        let node_id2 = extract_node_id_from_map(&path2, &map_path).unwrap();
        assert_eq!(node_id2, "node2");

        // 测试不存在的文件
        let path3 = PathBuf::from("file3.key");
        let result = extract_node_id_from_map(&path3, &map_path);
        assert!(result.is_err());
    }
}

/// 生成节点 ID 映射文件模板
///
/// 使用方法:
/// ```bash
/// cargo run --bin migrate_keys -- generate-map --from-dir ~/.cis/keys/v1 --output map.json
/// ```
#[derive(Parser)]
struct GenerateMapCommand {
    /// 源目录
    #[arg(short, long)]
    from_dir: PathBuf,

    /// 输出文件
    #[arg(short, long)]
    output: PathBuf,
}

fn generate_node_id_map(cmd: GenerateMapCommand) -> Result<()> {
    info!("生成节点 ID 映射: {:?}", cmd.from_dir);

    let entries = fs::read_dir(&cmd.from_dir)
        .context("无法读取源目录")?;

    let mut map = serde_json::Map::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // 跳过非密钥文件
        if !filename.ends_with(".key") && !filename.ends_with(".json") {
            continue;
        }

        // 从文件名提取节点 ID（需要用户确认）
        let node_id = filename
            .trim_end_matches(".key")
            .trim_end_matches(".json")
            .trim_end_matches("_v1")
            .trim_end_matches("_encrypted")
            .to_string();

        map.insert(filename.to_string(), serde_json::Value::String(node_id));
    }

    // 保存映射文件
    let content = serde_json::to_string_pretty(&map)?;
    fs::write(&cmd.output, content)?;

    info!("节点 ID 映射已保存到: {:?}", cmd.output);
    info!("请手动编辑此文件，为每个文件指定正确的节点 ID");

    Ok(())
}
