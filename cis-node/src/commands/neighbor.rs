//! # 邻居节点管理命令
//!
//! 简化的节点发现和添加流程

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};
use std::time::Duration;
use tokio::time::timeout;

/// 邻居管理子命令
#[derive(Subcommand, Debug)]
pub enum NeighborCommands {
    /// 发现局域网中的节点
    Discover {
        /// 发现超时时间（秒）
        #[arg(short, long, default_value = "10")]
        timeout_secs: u64,
        /// 持续监听模式
        #[arg(long)]
        watch: bool,
    },
    /// 列出已发现的节点
    List {
        /// 显示详细信息
        #[arg(short, long)]
        verbose: bool,
    },
    /// 添加发现的节点为邻居
    Add {
        /// 节点ID或hostname
        node: String,
        /// 自动确认（无需交互）
        #[arg(long)]
        yes: bool,
    },
    /// 显示本机节点信息
    Info,
}

/// 邻居命令参数
#[derive(Args, Debug)]
pub struct NeighborArgs {
    #[command(subcommand)]
    pub command: NeighborCommands,
}

/// 处理邻居命令
pub async fn handle(args: NeighborArgs) -> Result<()> {
    match args.command {
        NeighborCommands::Discover { timeout_secs, watch } => {
            discover_nodes(timeout_secs, watch).await
        }
        NeighborCommands::List { verbose } => list_discovered(verbose).await,
        NeighborCommands::Add { node, yes } => add_neighbor(node, yes).await,
        NeighborCommands::Info => show_node_info().await,
    }
}

/// 发现局域网节点
async fn discover_nodes(timeout_secs: u64, watch: bool) -> Result<()> {
    use cis_core::network::SimpleDiscovery;
    use cis_core::storage::paths::Paths;
    
    // 读取当前节点配置
    let config_path = Paths::config_dir().join("config.toml");
    let config_str = tokio::fs::read_to_string(&config_path).await
        .map_err(|e| anyhow!("Failed to read config: {}. Please run `cis init` first", e))?;
    
    // 简单解析获取 node_id 和 did
    let node_id = parse_config_value(&config_str, "node_id")
        .unwrap_or_else(|| "unknown".to_string());
    let did = parse_config_value(&config_str, "did")
        .unwrap_or_else(|| "unknown".to_string());
    
    println!("🔍 启动节点发现服务...");
    println!("   本机节点: {} ({})", node_id, gethostname::gethostname().to_string_lossy());
    println!();
    
    let discovery = SimpleDiscovery::new(&node_id, &did)?;
    discovery.start().await?;
    
    if watch {
        // 持续监听模式
        println!("👀 持续监听中...按 Ctrl+C 停止\n");
        
        let mut interval = tokio::time::interval(Duration::from_secs(3));
        let mut last_count = 0;
        
        loop {
            interval.tick().await;
            
            let nodes = discovery.get_discovered_nodes();
            if nodes.len() != last_count {
                last_count = nodes.len();
                print_discovered_nodes(&nodes);
            }
        }
    } else {
        // 单次发现模式
        println!("⏱️  发现中，等待 {} 秒...\n", timeout_secs);
        
        tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
        
        let nodes = discovery.get_discovered_nodes();
        
        if nodes.is_empty() {
            println!("❌ 未发现任何节点");
            println!();
            println!("可能的原因:");
            println!("  • 同一网络中没有其他 CIS 节点");
            println!("  • 防火墙阻止了 UDP 广播（端口 6767）");
            println!("  • 节点使用了不同的网络接口");
            println!();
            println!("建议:");
            println!("  • 使用 --watch 模式持续监听");
            println!("  • 检查防火墙设置");
            println!("  • 手动添加节点: cis neighbor add <ip:port>");
        } else {
            println!("✅ 发现 {} 个节点:\n", nodes.len());
            print_discovered_nodes(&nodes);
            println!();
            println!("💡 添加节点为邻居:");
            for node in &nodes {
                println!("   cis neighbor add {}", node.node_id);
            }
        }
    }
    
    Ok(())
}

/// 列出已发现的节点
async fn list_discovered(verbose: bool) -> Result<()> {
    println!("📋 显示最近发现的节点...");
    println!("   (发现服务需要运行中，请使用 `cis neighbor discover --watch`)");
    println!();
    println!("💡 要发现新节点，请运行:");
    println!("   cis neighbor discover     # 单次发现");
    println!("   cis neighbor discover --watch  # 持续监听");
    Ok(())
}

/// 添加邻居节点
async fn add_neighbor(node_ref: String, yes: bool) -> Result<()> {
    use cis_core::service::NodeService;
    
    println!("➕ 添加邻居节点: {}", node_ref);
    
    // 解析节点引用（可以是 node_id, hostname, 或 ip:port）
    let (node_id, address) = if node_ref.contains(':') {
        // IP:port 格式
        (node_ref.clone(), node_ref.clone())
    } else {
        // 尝试作为 node_id 或 hostname
        (node_ref.clone(), format!("{}:7676", node_ref))
    };
    
    // 显示确认信息
    println!();
    println!("节点信息:");
    println!("  ID: {}", node_id);
    println!("  地址: {}", address);
    println!();
    
    if !yes {
        print!("确认添加此节点为邻居? [Y/n] ");
        use std::io::Write;
        std::io::stdout().flush()?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() == "n" {
            println!("已取消");
            return Ok(());
        }
    }
    
    // 添加到节点服务
    let node_service = NodeService::new()
        .map_err(|e| anyhow!("Failed to initialize node service: {}", e))?;
    
    // 创建绑定选项
    let bind_options = cis_core::service::node_service::BindOptions {
        endpoint: address.clone(),
        did: None, // 将在首次连接时验证
        trust_level: cis_core::service::node_service::TrustLevel::Limited,
        auto_sync: true,
    };
    
    match node_service.bind(bind_options).await {
        Ok(info) => {
            println!();
            println!("✅ 成功添加邻居节点");
            println!("  节点ID: {}", info.summary.id);
            println!("  地址: {}", info.summary.endpoint);
            println!();
            println!("💡 验证节点身份:");
            println!("   cis node inspect {}", info.summary.id);
        }
        Err(e) => {
            println!();
            println!("❌ 添加失败: {}", e);
            println!();
            println!("可能的原因:");
            println!("  • 节点ID已存在");
            println!("  • 网络不可达");
            println!("  • 配置错误");
        }
    }
    
    Ok(())
}

/// 显示本机节点信息
async fn show_node_info() -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    
    println!("📱 本机节点信息");
    println!();
    println!("基本信息:");
    println!("  设备名称: {}", hostname);
    println!();
    
    // 读取配置
    let config_path = Paths::config_dir().join("config.toml");
    if let Ok(config_str) = tokio::fs::read_to_string(&config_path).await {
        if let Some(node_id) = parse_config_value(&config_str, "node_id") {
            println!("  节点ID: {}", node_id);
        }
        if let Some(did) = parse_config_value(&config_str, "did") {
            println!("  DID: {}", did);
        }
    }
    
    println!();
    println!("网络配置:");
    println!("  发现端口: UDP 6767 (广播)");
    println!("  服务端口: TCP 7676 (WebSocket)");
    println!();
    println!("💡 其他节点可以通过以下方式发现你:");
    println!("   cis neighbor discover");
    println!();
    println!("💡 手动添加你的节点:");
    println!("   cis neighbor add {}:7676", hostname);
    
    Ok(())
}

/// 辅助函数：解析配置值
fn parse_config_value(config: &str, key: &str) -> Option<String> {
    config
        .lines()
        .find(|line| line.trim().starts_with(key))
        .and_then(|line| line.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
}

/// 辅助函数：打印发现的节点列表
fn print_discovered_nodes(nodes: &[cis_core::network::DiscoveredNode]) {
    if nodes.is_empty() {
        println!("  (暂无发现的节点)");
        return;
    }
    
    println!("  {:<20} {:<15} {:<20}", "节点ID", "设备名称", "地址");
    println!("  {}", "-".repeat(60));
    
    for node in nodes {
        let addr = node.addresses
            .first()
            .map(|a| a.to_string())
            .unwrap_or_default();
        println!("  {:<20} {:<15} {}:{}", 
            node.node_id,
            node.hostname,
            addr,
            node.port
        );
    }
}
