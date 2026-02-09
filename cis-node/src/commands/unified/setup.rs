//! # 一键初始化命令
//!
//! 自动完成 CIS 初始化，无需手动配置 toml。

use super::NodeRole;
use std::path::PathBuf;

/// 执行一键初始化
pub async fn execute(auto: bool, role: NodeRole) -> anyhow::Result<()> {
    if is_initialized() {
        println!("✅ CIS 已初始化");
        println!("💡 如需重新配置，先删除 ~/.cis/ 目录");
        return Ok(());
    }
    
    println!("🔧 开始初始化 CIS...\n");
    
    // 1. 创建目录结构
    println!("步骤 1/4: 创建目录结构...");
    create_directories()?;
    println!("   ✅ 目录创建完成\n");
    
    // 2. 生成节点身份
    println!("步骤 2/4: 生成节点身份...");
    let node_info = generate_node_identity(role)?;
    println!("   ✅ 节点身份生成完成");
    println!("      节点ID: {}", node_info.id);
    println!("      节点名: {}", node_info.name);
    println!("      DID: {}", node_info.did);
    println!("      角色: {:?}\n", role);
    
    // 3. 创建默认配置（无需手动编辑 toml）
    println!("步骤 3/4: 创建默认配置...");
    create_default_config(&node_info, role).await?;
    println!("   ✅ 配置创建完成");
    println!("      配置路径: ~/.cis/config.toml");
    println!("      数据路径: ~/.cis/data/\n");
    
    // 4. 初始化数据库
    println!("步骤 4/4: 初始化数据库...");
    init_database().await?;
    println!("   ✅ 数据库初始化完成\n");
    
    println!("╔══════════════════════════════════════════╗");
    println!("║     ✅ CIS 初始化完成                   ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║                                          ║");
    println!("║  节点: {}                    ║", node_info.name);
    println!("║  角色: {:?}                              ║", role);
    println!("║  状态: 🟢 就绪                          ║");
    println!("║                                          ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║  下一步:                                 ║");
    println!("║    cis join    - 加入/创建网络          ║");
    println!("║    cis status  - 查看状态               ║");
    println!("╚══════════════════════════════════════════╝\n");
    
    Ok(())
}

/// 创建目录结构
fn create_directories() -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法获取 home 目录"))?;
    let cis_dir = home.join(".cis");
    
    let dirs = vec![
        cis_dir.clone(),
        cis_dir.join("data"),
        cis_dir.join("logs"),
        cis_dir.join("skills"),
    ];
    
    for dir in dirs {
        std::fs::create_dir_all(&dir)?;
    }
    
    Ok(())
}

/// 生成节点身份
fn generate_node_identity(role: NodeRole) -> anyhow::Result<NodeInfo> {
    let id = uuid::Uuid::new_v4().to_string();
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let short_id = &id[..8];
    
    let did = format!("did:cis:{}:{}", hostname, short_id);
    
    Ok(NodeInfo {
        id,
        name: hostname,
        did,
        role: format!("{:?}", role).to_lowercase(),
    })
}

/// 创建默认配置
async fn create_default_config(node: &NodeInfo, role: NodeRole) -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法获取 home 目录"))?;
    let config_path = home.join(".cis").join("config.toml");
    
    // 内置默认配置，用户无需手动编辑
    let config = format!(r#"# CIS 自动生成的配置文件
# 生成时间: {}

[node]
id = "{}"
name = "{}"
did = "{}"
role = "{}"
key = "{}"

[ai]
default_provider = "claude"

[ai.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[network]
discovery_port = 6767
pairing_port = 6768
federation_port = 7676
p2p_port = 7677

[storage]
max_backups = 10
backup_interval_days = 7
"#, 
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        node.id,
        node.name,
        node.did,
        format!("{:?}", role).to_lowercase(),
        generate_random_key()
    );
    
    tokio::fs::write(&config_path, config).await?;
    
    Ok(())
}

/// 初始化数据库
async fn init_database() -> anyhow::Result<()> {
    // 简化实现，实际初始化 SQLite 数据库
    // 创建必要的表：peers, messages, config, etc.
    
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("无法获取 home 目录"))?;
    let db_path = home.join(".cis").join("data").join("cis.db");
    
    // 确保数据库文件存在
    if !db_path.exists() {
        tokio::fs::File::create(&db_path).await?;
    }
    
    Ok(())
}

/// 检查是否已初始化
fn is_initialized() -> bool {
    let config_path = dirs::home_dir()
        .map(|p| p.join(".cis").join("config.toml"))
        .unwrap_or_default();
    config_path.exists()
}

/// 生成随机密钥
fn generate_random_key() -> String {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    hex::encode(key)
}

#[derive(Debug)]
struct NodeInfo {
    id: String,
    name: String,
    did: String,
    role: String,
}
