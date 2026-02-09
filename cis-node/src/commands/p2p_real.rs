//! P2P 命令真实实现
//!
//! 替换模拟实现，使用真实的 P2PNetwork

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use std::time::Duration;

use cis_core::p2p::network::{P2PNetwork, P2PConfig};

/// P2P 子命令
#[derive(Subcommand, Debug)]
pub enum P2pCommands {
    /// 查看 P2P 网络状态
    Status,
    
    /// 发现节点（真实实现）
    Discover {
        /// 发现超时时间（秒）
        #[arg(long, default_value = "10")]
        timeout: u64,
        /// 显示详细信息
        #[arg(long, short)]
        verbose: bool,
    },
    
    /// 查看发现的节点
    Peers {
        /// 显示详细信息
        #[arg(long)]
        verbose: bool,
        /// 只显示已连接的节点
        #[arg(long)]
        connected: bool,
    },
    
    /// 连接到指定节点
    Connect {
        /// 节点地址 (host:port)
        address: String,
    },
    
    /// 断开与节点的连接
    Disconnect {
        /// 节点 ID
        node_id: String,
    },
    
    /// 启动 P2P 网络
    Start {
        /// 监听地址
        #[arg(long, default_value = "0.0.0.0:7677")]
        listen: String,
    },
    
    /// 停止 P2P 网络
    Stop,
}

/// 处理 P2P 命令
pub async fn handle_p2p(cmd: P2pCommands) -> Result<()> {
    match cmd {
        P2pCommands::Status => show_status().await,
        P2pCommands::Discover { timeout, verbose } => discover_nodes(timeout, verbose).await,
        P2pCommands::Peers { verbose, connected } => list_peers(verbose, connected).await,
        P2pCommands::Connect { address } => connect_node(&address).await,
        P2pCommands::Disconnect { node_id } => disconnect_node(&node_id).await,
        P2pCommands::Start { listen } => start_p2p(&listen).await,
        P2pCommands::Stop => stop_p2p().await,
    }
}

/// 显示 P2P 状态
async fn show_status() -> Result<()> {
    match P2PNetwork::global().await {
        Some(network) => {
            let status = network.status().await;
            println!("📡 P2P Network Status");
            println!("=====================");
            println!("Node ID:    {}", status.node_id);
            println!("Listen:     {}", status.listen_addr);
            println!("Uptime:     {}s", status.uptime_secs);
            println!("Connected:  {} peers", status.connected_peers);
            println!("Discovered: {} peers", status.discovered_peers);
        }
        None => {
            println!("🔴 P2P network not running");
            println!("   Run 'cis p2p start' to start");
        }
    }
    Ok(())
}

/// 发现节点（真实实现）
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started. Run 'cis p2p start' first."))?;
    
    println!("🔍 Discovering nodes...");
    println!("   Timeout: {} seconds\n", timeout_secs);
    
    // 显示发现的节点
    let start = std::time::Instant::now();
    let mut last_count = 0;
    
    while start.elapsed().as_secs() < timeout_secs {
        let peers = network.discovered_peers().await;
        
        if peers.len() != last_count {
            println!("   Found {} node(s)...", peers.len());
            last_count = peers.len();
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    // 显示最终结果
    let peers = network.discovered_peers().await;
    
    if peers.is_empty() {
        println!("\n❌ No nodes discovered");
        println!("\nPossible reasons:");
        println!("  • No CIS nodes on the same network");
        println!("  • Firewall blocking mDNS (port 6767)");
        return Ok(());
    }
    
    println!("\n✅ Discovered {} node(s):\n", peers.len());
    
    for (i, peer) in peers.iter().enumerate() {
        println!("  [{}] {}", i + 1, peer.node_id);
        println!("      Address: {}", peer.address);
        println!("      DID: {}", peer.did);
        println!("      Connected: {}", if peer.connected { "yes" } else { "no" });
        
        if verbose {
            println!("      Last seen: {:?}", peer.last_seen);
        }
        println!();
    }
    
    Ok(())
}

/// 列出节点
async fn list_peers(verbose: bool, connected_only: bool) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    let peers = if connected_only {
        network.connected_peers().await
    } else {
        network.discovered_peers().await
    };
    
    if peers.is_empty() {
        println!("No peers found");
        return Ok(());
    }
    
    println!("📋 {} peers:\n", peers.len());
    
    for peer in peers {
        let icon = if peer.connected { "🟢" } else { "⚪" };
        println!("{} {} @ {}", icon, peer.node_id, peer.address);
        
        if verbose {
            println!("   DID: {}", peer.did);
        }
    }
    
    Ok(())
}

/// 连接节点
async fn connect_node(address: &str) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    println!("🔗 Connecting to {}...", address);
    
    network.connect(address).await?;
    
    println!("✅ Connected to {}", address);
    Ok(())
}

/// 断开连接
async fn disconnect_node(node_id: &str) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    println!("🔌 Disconnecting from {}...", node_id);
    
    network.disconnect(node_id).await?;
    
    println!("✅ Disconnected from {}", node_id);
    Ok(())
}

/// 启动 P2P 网络
async fn start_p2p(listen: &str) -> Result<()> {
    // 检查是否已运行
    if P2PNetwork::global().await.is_some() {
        println!("⚠️  P2P network already running");
        return Ok(());
    }
    
    println!("🚀 Starting P2P network...");
    println!("   Listen: {}", listen);
    
    let config = P2PConfig {
        listen_addr: listen.to_string(),
        ..Default::default()
    };
    
    let _network = P2PNetwork::start(config).await?;
    
    println!("✅ P2P network started");
    println!("   Use 'cis p2p discover' to find nodes");
    
    Ok(())
}

/// 停止 P2P 网络
async fn stop_p2p() -> Result<()> {
    match P2PNetwork::global().await {
        Some(_) => {
            println!("🛑 Stopping P2P network...");
            P2PNetwork::stop().await?;
            println!("✅ P2P network stopped");
        }
        None => {
            println!("⚠️  P2P network not running");
        }
    }
    Ok(())
}
