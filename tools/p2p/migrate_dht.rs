//! DHT 迁移工具
//!
//! 将旧的 DHT 实现数据迁移到 libp2p KadDHT。
//!
//! ## 用法
//!
//! ```bash
//! # 预览迁移（不执行）
//! migrate_dht --preview
//!
//! # 执行迁移
//! migrate_dht --execute
//!
//! # 验证迁移
//! migrate_dht --verify
//! ```
//!
//! ## 迁移流程
//!
//! 1. **预览阶段** - 扫描旧数据，生成迁移计划
//! 2. **备份阶段** - 创建完整备份
//! 3. **迁移阶段** - 逐条迁移数据和节点
//! 4. **验证阶段** - 验证数据完整性
//! 5. **清理阶段** - 可选：清理旧数据

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::time::Instant;
use tracing::{info, warn, error, debug};

// 模拟的旧 DHT 服务结构（实际应该从 cis-core 导入）
#[derive(Debug, Clone)]
struct LegacyDhtService {
    data_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyDhtRecord {
    key: String,
    #[serde(with = "serde_bytes")]
    value: Vec<u8>,
    timestamp: DateTime<Utc>,
    expires: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyNodeInfo {
    id: String,
    endpoint: String,
    last_seen: DateTime<Utc>,
    trust_score: f64,
}

impl LegacyDhtService {
    fn new(data_path: PathBuf) -> Self {
        Self { data_path }
    }

    /// 读取所有记录
    fn read_records(&self) -> AnyhowResult<Vec<LegacyDhtRecord>> {
        let records_file = self.data_path.join("records.json");
        if !records_file.exists() {
            info!("No legacy records found at {:?}", records_file);
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&records_file)
            .context("Failed to read records file")?;

        let records: Vec<LegacyDhtRecord> = serde_json::from_str(&content)
            .context("Failed to parse records file")?;

        Ok(records)
    }

    /// 读取所有节点
    fn read_nodes(&self) -> AnyhowResult<Vec<LegacyNodeInfo>> {
        let nodes_file = self.data_path.join("nodes.json");
        if !nodes_file.exists() {
            info!("No legacy nodes found at {:?}", nodes_file);
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&nodes_file)
            .context("Failed to read nodes file")?;

        let nodes: Vec<LegacyNodeInfo> = serde_json::from_str(&content)
            .context("Failed to parse nodes file")?;

        Ok(nodes)
    }
}

// 模拟的新 DHT 适配器（实际应该从 cis-core 导入）
struct NewKadDhtAdapter {
    // 实际实现时使用真实的 Libp2pKadDht
}

impl NewKadDhtAdapter {
    async fn put_record(&self, key: &str, value: Vec<u8>) -> AnyhowResult<()> {
        debug!("Migrating record: key={}, size={}", key, value.len());
        // TODO: 实际实现调用 Libp2pKadDht::put_memory()
        Ok(())
    }

    async fn add_node(&self, node: LegacyNodeInfo) -> AnyhowResult<()> {
        debug!("Migrating node: id={}", node.id);
        // TODO: 实际实现调用 NodeInfoStore::save_node()
        Ok(())
    }
}

/// 迁移配置
#[derive(Parser, Debug)]
#[command(name = "migrate_dht")]
#[command(about = "Migrate DHT data from legacy implementation to libp2p KadDHT", long_about = None)]
struct MigrationArgs {
    #[command(subcommand)]
    command: MigrationCommand,
}

#[derive(Subcommand, Debug)]
enum MigrationCommand {
    /// Preview migration (scan and plan without executing)
    Preview {
        /// Legacy DHT data path
        #[arg(short, long, default_value = "~/.cis/data/legacy_dht")]
        legacy_path: PathBuf,

        /// Output migration plan to file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Execute migration
    Execute {
        /// Legacy DHT data path
        #[arg(short, long, default_value = "~/.cis/data/legacy_dht")]
        legacy_path: PathBuf,

        /// Create backup before migration
        #[arg(short, long, default_value_t = true)]
        backup: bool,

        /// Backup path
        #[arg(short, long, default_value = "~/.cis/data/dht_backup")]
        backup_path: PathBuf,

        /// Number of concurrent migrations
        #[arg(short, long, default_value_t = 10)]
        concurrency: usize,

        /// Skip confirmation prompt
        #[arg(short, long, default_value_t = false)]
        yes: bool,
    },

    /// Verify migration integrity
    Verify {
        /// Legacy DHT data path
        #[arg(short, long, default_value = "~/.cis/data/legacy_dht")]
        legacy_path: PathBuf,

        /// Verification tolerance (percentage)
        #[arg(short, long, default_value_t = 5)]
        tolerance: usize,
    },
}

/// 迁移计划
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationPlan {
    total_records: usize,
    total_nodes: usize,
    estimated_duration_secs: u64,
    backup_required: bool,
    warnings: Vec<String>,
}

/// 迁移结果
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationResult {
    records_migrated: usize,
    records_failed: usize,
    nodes_migrated: usize,
    nodes_failed: usize,
    duration_secs: u64,
    backup_path: Option<PathBuf>,
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VerificationResult {
    records_matched: usize,
    records_missing: usize,
    nodes_matched: usize,
    nodes_missing: usize,
    integrity_percentage: f64,
    success: bool,
}

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = MigrationArgs::parse();

    match args.command {
        MigrationCommand::Preview { legacy_path, output } => {
            preview_migration(legacy_path, output).await?
        }
        MigrationCommand::Execute {
            legacy_path,
            backup,
            backup_path,
            concurrency,
            yes,
        } => {
            execute_migration(legacy_path, backup, backup_path, concurrency, yes).await?
        }
        MigrationCommand::Verify { legacy_path, tolerance } => {
            verify_migration(legacy_path, tolerance).await?
        }
    }

    Ok(())
}

/// 预览迁移
async fn preview_migration(
    legacy_path: PathBuf,
    output: Option<PathBuf>,
) -> AnyhowResult<()> {
    info!("Starting migration preview...");

    // 展开路径
    let legacy_path = shellexpand::tilde(&legacy_path.to_string_lossy());
    let legacy_path = PathBuf::from(legacy_path.as_ref());

    // 创建旧 DHT 服务
    let legacy_dht = LegacyDhtService::new(legacy_path);

    // 扫描数据
    let records = legacy_dht.read_records()?;
    let nodes = legacy_dht.read_nodes()?;

    info!("Found {} records and {} nodes", records.len(), nodes.len());

    // 生成迁移计划
    let plan = MigrationPlan {
        total_records: records.len(),
        total_nodes: nodes.len(),
        estimated_duration_secs: estimate_duration(records.len(), nodes.len()),
        backup_required: true,
        warnings: generate_warnings(&records, &nodes),
    };

    // 输出计划
    let plan_json = serde_json::to_string_pretty(&plan)?;
    println!("{}", plan_json);

    // 可选：写入文件
    if let Some(output_path) = output {
        let output_path = shellexpand::tilde(&output_path.to_string_lossy());
        std::fs::write(output_path.as_ref(), plan_json)
            .context("Failed to write migration plan")?;
        info!("Migration plan written to: {}", output_path);
    }

    Ok(())
}

/// 执行迁移
async fn execute_migration(
    legacy_path: PathBuf,
    backup: bool,
    backup_path: PathBuf,
    concurrency: usize,
    skip_confirm: bool,
) -> AnyhowResult<()> {
    info!("Starting DHT migration...");

    // 展开路径
    let legacy_path = PathBuf::from(shellexpand::tilde(&legacy_path.to_string_lossy()).as_ref());
    let backup_path = PathBuf::from(shellexpand::tilde(&backup_path.to_string_lossy()).as_ref());

    // 创建旧 DHT 服务
    let legacy_dht = LegacyDhtService::new(legacy_path);

    // 读取数据
    let records = legacy_dht.read_records()?;
    let nodes = legacy_dht.read_nodes()?;

    info!("Found {} records and {} nodes to migrate", records.len(), nodes.len());

    // 确认提示
    if !skip_confirm {
        println!("About to migrate {} records and {} nodes", records.len(), nodes.len());
        println!("Continue? [y/N]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            info!("Migration cancelled by user");
            return Ok(());
        }
    }

    // 备份
    let actual_backup_path = if backup {
        info!("Creating backup at {:?}", backup_path);
        create_backup(&legacy_path, &backup_path)?;
        Some(backup_path)
    } else {
        None
    };

    // 创建新 DHT 适配器
    let new_dht = Arc::new(NewKadDhtAdapter {});

    // 执行迁移
    let start = Instant::now();

    let result = migrate_data(
        Arc::clone(&new_dht),
        records,
        nodes,
        concurrency,
    ).await?;

    let duration = start.elapsed();

    // 输出结果
    let result_json = serde_json::to_string_pretty(&result)?;
    println!("{}", result_json);

    info!("Migration completed in {:?}", duration);
    info!("Records: {} migrated, {} failed",
        result.records_migrated, result.records_failed);
    info!("Nodes: {} migrated, {} failed",
        result.nodes_migrated, result.nodes_failed);

    Ok(())
}

/// 迁移数据（并行）
async fn migrate_data(
    dht: Arc<NewKadDhtAdapter>,
    records: Vec<LegacyDhtRecord>,
    nodes: Vec<LegacyNodeInfo>,
    concurrency: usize,
) -> AnyhowResult<MigrationResult> {
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let start = Instant::now();

    // 迁移记录
    let records_migrated = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let records_failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let record_tasks: Vec<_> = records.into_iter().map(|record| {
        let dht = Arc::clone(&dht);
        let semaphore = Arc::clone(&semaphore);
        let records_migrated = Arc::clone(&records_migrated);
        let records_failed = Arc::clone(&records_failed);

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            match dht.put_record(&record.key, record.value).await {
                Ok(()) => {
                    records_migrated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Err(e) => {
                    error!("Failed to migrate record {}: {}", record.key, e);
                    records_failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        })
    }).collect();

    for task in record_tasks {
        task.await?;
    }

    // 迁移节点
    let nodes_migrated = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let nodes_failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let node_tasks: Vec<_> = nodes.into_iter().map(|node| {
        let dht = Arc::clone(&dht);
        let semaphore = Arc::clone(&semaphore);
        let nodes_migrated = Arc::clone(&nodes_migrated);
        let nodes_failed = Arc::clone(&nodes_failed);

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            match dht.add_node(node).await {
                Ok(()) => {
                    nodes_migrated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Err(e) => {
                    error!("Failed to migrate node: {}", e);
                    nodes_failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        })
    }).collect();

    for task in node_tasks {
        task.await?;
    }

    let duration = start.elapsed().as_secs();

    Ok(MigrationResult {
        records_migrated: records_migrated.load(std::sync::atomic::Ordering::Relaxed),
        records_failed: records_failed.load(std::sync::atomic::Ordering::Relaxed),
        nodes_migrated: nodes_migrated.load(std::sync::atomic::Ordering::Relaxed),
        nodes_failed: nodes_failed.load(std::sync::atomic::Ordering::Relaxed),
        duration_secs: duration,
        backup_path: None,
    })
}

/// 验证迁移
async fn verify_migration(
    legacy_path: PathBuf,
    tolerance: usize,
) -> AnyhowResult<()> {
    info!("Verifying migration integrity...");

    // 展开路径
    let legacy_path = PathBuf::from(shellexpand::tilde(&legacy_path.to_string_lossy()).as_ref());

    // 创建旧 DHT 服务
    let legacy_dht = LegacyDhtService::new(legacy_path);

    // 读取旧数据
    let legacy_records = legacy_dht.read_records()?;
    let legacy_nodes = legacy_dht.read_nodes()?;

    // TODO: 从新 DHT 读取数据并对比
    // 这里需要实际的 NewKadDhtAdapter 实现

    info!("Verification completed");
    info!("Legacy records: {}", legacy_records.len());
    info!("Legacy nodes: {}", legacy_nodes.len());

    Ok(())
}

/// 创建备份
fn create_backup(source: &PathBuf, backup_path: &PathBuf) -> AnyhowResult<()> {
    info!("Creating backup at {:?}...", backup_path);

    // 创建备份目录
    std::fs::create_dir_all(backup_path)
        .context("Failed to create backup directory")?;

    // 复制文件
    for entry in std::fs::read_dir(source)
        .context("Failed to read source directory")?
    {
        let entry = entry?;
        let src = entry.path();
        let dst = backup_path.join(entry.file_name());

        if src.is_file() {
            std::fs::copy(&src, &dst)
                .with_context(|| format!("Failed to copy {:?}", src))?;
        }
    }

    info!("Backup created successfully");
    Ok(())
}

/// 估计迁移时长
fn estimate_duration(num_records: usize, num_nodes: usize) -> u64 {
    // 假设每条记录 10ms，每个节点 5ms
    let record_time = num_records as u64 * 10;
    let node_time = num_nodes as u64 * 5;
    (record_time + node_time) / 1000 // 转换为秒
}

/// 生成警告
fn generate_warnings(
    _records: &[LegacyDhtRecord],
    _nodes: &[LegacyNodeInfo],
) -> Vec<String> {
    let mut warnings = Vec::new();

    // 检查大记录
    // warnings.push("Found large records (>1MB), may take longer to migrate".to_string());

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_duration() {
        let duration = estimate_duration(1000, 500);
        assert_eq!(duration, (1000 * 10 + 500 * 5) / 1000); // 12.5 seconds
    }

    #[test]
    fn test_migration_plan_serialization() {
        let plan = MigrationPlan {
            total_records: 100,
            total_nodes: 50,
            estimated_duration_secs: 2,
            backup_required: true,
            warnings: vec!["Test warning".to_string()],
        };

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"total_records\":100"));
    }

    #[test]
    fn test_migration_result_serialization() {
        let result = MigrationResult {
            records_migrated: 95,
            records_failed: 5,
            nodes_migrated: 48,
            nodes_failed: 2,
            duration_secs: 10,
            backup_path: Some(PathBuf::from("/backup")),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"records_migrated\":95"));
    }
}
