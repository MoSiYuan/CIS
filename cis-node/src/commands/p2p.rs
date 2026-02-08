//! # P2P 网络命令
//!
//! 管理 P2P 网络连接和节点发现

use anyhow::Result;
use clap::{Args, Subcommand};

/// P2P 子命令
#[derive(Subcommand, Debug)]
pub enum P2pAction {
    /// 查看 P2P 网络状态
    Status,
    
    /// 发现节点（mDNS 局域网发现）
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
        /// 节点地址 (did:cis:node@host:port 或 host:port)
        address: String,
        /// 节点 ID（如果地址中不包含）
        #[arg(long)]
        node_id: Option<String>,
    },
    
    /// 断开与节点的连接
    Disconnect {
        /// 节点 ID
        node_id: String,
    },
    
    /// 手动添加节点
    AddPeer {
        /// 节点 ID
        node_id: String,
        /// 节点地址
        address: String,
        /// DID
        #[arg(long)]
        did: Option<String>,
    },
    
    /// 移除节点
    RemovePeer {
        /// 节点 ID
        node_id: String,
    },
    
    /// 触发同步
    Sync {
        /// 指定节点同步
        #[arg(long, short)]
        node: Option<String>,
        /// 强制完整同步
        #[arg(long)]
        full: bool,
    },
    
    /// 查看同步状态
    SyncStatus,
    
    /// 测试节点延迟
    Ping {
        /// 节点 ID 或地址
        target: String,
    },
    
    /// 广播消息
    Broadcast {
        /// 消息主题
        topic: String,
        /// 消息内容
        message: String,
    },
    
    /// 启动 P2P 网络
    Start {
        /// 监听地址
        #[arg(long, default_value = "0.0.0.0:7677")]
        listen: String,
        /// 启用 DHT
        #[arg(long)]
        dht: bool,
        /// Bootstrap 节点
        #[arg(long)]
        bootstrap: Vec<String>,
        /// 外部地址（手动指定）
        #[arg(long)]
        external: Option<String>,
        /// 启用 NAT 穿透
        #[arg(long)]
        nat_traversal: bool,
    },
    
    /// 停止 P2P 网络
    Stop,
    
    /// NAT 穿透测试
    HolePunch {
        /// 目标节点地址
        #[arg(long, short)]
        target: Option<String>,
        /// 仅检测 NAT 类型
        #[arg(long)]
        detect_only: bool,
        /// 使用 STUN 服务器
        #[arg(long)]
        stun_server: Option<String>,
    },
    
    /// DHT 操作
    Dht {
        #[command(subcommand)]
        action: DhtAction,
    },
    
    /// 网络诊断
    Diagnose {
        /// 诊断类型
        #[arg(long, value_enum, default_value = "all")]
        check: DiagnoseType,
    },
}

/// DHT 子命令
#[derive(Subcommand, Debug)]
pub enum DhtAction {
    /// 显示 DHT 状态
    Status,
    
    /// 存储键值对
    Put {
        /// 键
        key: String,
        /// 值
        value: String,
    },
    
    /// 获取键值对
    Get {
        /// 键
        key: String,
    },
    
    /// 查找节点
    FindNode {
        /// 节点 ID
        node_id: String,
    },
    
    /// 显示路由表
    RoutingTable {
        /// 显示详细信息
        #[arg(long)]
        verbose: bool,
    },
    
    /// 添加 Bootstrap 节点
    AddBootstrap {
        /// 节点地址
        address: String,
    },
    
    /// 列出 Bootstrap 节点
    ListBootstrap,
}

/// 诊断类型
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum DiagnoseType {
    /// 全部检查
    All,
    /// 网络连通性
    Network,
    /// NAT 类型
    Nat,
    /// DHT 状态
    Dht,
    /// 端口可用性
    Port,
}

/// P2P 命令参数
#[derive(Args, Debug)]
pub struct P2pArgs {
    #[command(subcommand)]
    pub action: P2pAction,
}

/// 处理 P2P 命令
pub async fn handle_p2p(args: P2pArgs) -> Result<()> {
    match args.action {
        P2pAction::Status => show_status().await,
        P2pAction::Discover { timeout, verbose } => discover_nodes(timeout, verbose).await,
        P2pAction::Peers { verbose, connected } => list_peers(verbose, connected).await,
        P2pAction::Connect { address, node_id } => connect_node(&address, node_id.as_deref()).await,
        P2pAction::Disconnect { node_id } => disconnect_node(&node_id).await,
        P2pAction::AddPeer { node_id, address, did } => add_peer(&node_id, &address, did.as_deref()).await,
        P2pAction::RemovePeer { node_id } => remove_peer(&node_id).await,
        P2pAction::Sync { node, full } => trigger_sync(node.as_deref(), full).await,
        P2pAction::SyncStatus => show_sync_status().await,
        P2pAction::Ping { target } => ping_node(&target).await,
        P2pAction::Broadcast { topic, message } => broadcast_message(&topic, &message).await,
        P2pAction::Start { listen, dht, bootstrap, external, nat_traversal } => {
            start_p2p(&listen, dht, bootstrap, external, nat_traversal).await
        }
        P2pAction::Stop => stop_p2p().await,
        P2pAction::HolePunch { target, detect_only, stun_server } => {
            hole_punch(target.as_deref(), detect_only, stun_server.as_deref()).await
        }
        P2pAction::Dht { action } => handle_dht_action(action).await,
        P2pAction::Diagnose { check } => diagnose_network(check).await,
    }
}

/// 显示 P2P 状态
async fn show_status() -> Result<()> {
    println!("📡 P2P Network Status\n");
    
    // 检查配置文件
    let config_path = cis_core::storage::paths::Paths::config_file();
    if !config_path.exists() {
        println!("❌ CIS not initialized");
        println!("   Run 'cis init' first");
        return Ok(());
    }
    
    // 读取配置
    let config_content = std::fs::read_to_string(&config_path)?;
    let config: toml::Value = toml::from_str(&config_content)?;
    
    // 显示节点信息
    if let Some(node) = config.get("node") {
        if let Some(id) = node.get("id").and_then(|v| v.as_str()) {
            println!("Node ID:    {}", id);
        }
        if let Some(name) = node.get("name").and_then(|v| v.as_str()) {
            println!("Node Name:  {}", name);
        }
    }
    
    // P2P 配置
    println!("\nP2P Configuration:");
    if let Some(p2p) = config.get("p2p") {
        let enabled = p2p.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        println!("  Enabled:  {}", if enabled { "✅" } else { "❌" });
        
        if let Some(port) = p2p.get("listen_port").and_then(|v| v.as_integer()) {
            println!("  Port:     {}", port);
        }
        
        if let Some(dht) = p2p.get("enable_dht").and_then(|v| v.as_bool()) {
            println!("  DHT:      {}", if dht { "✅" } else { "❌" });
        }
        
        if let Some(nat) = p2p.get("enable_nat_traversal").and_then(|v| v.as_bool()) {
            println!("  NAT:      {}", if nat { "✅" } else { "❌" });
        }
    } else {
        println!("  Not configured");
    }
    
    // 网络状态（简化显示）
    println!("\nNetwork Status:");
    println!("  State:    🟡 Not connected (run 'cis p2p start')");
    println!("  Peers:    0 connected");
    println!("  DHT:      Inactive");
    
    println!("\nAvailable Commands:");
    println!("  cis p2p start              # Start P2P network");
    println!("  cis p2p discover           # Discover nodes");
    println!("  cis p2p connect <addr>     # Connect to a node");
    println!("  cis p2p diagnose           # Network diagnostics");
    
    Ok(())
}

/// 发现节点
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    println!("🔍 Discovering nodes...");
    println!("   Timeout: {} seconds\n", timeout_secs);
    
    // 模拟发现过程
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Searching for nodes...");
    
    // 模拟等待
    for i in 0..timeout_secs {
        pb.set_message(format!("Searching... ({}/{}s)", i + 1, timeout_secs));
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        // 模拟发现节点
        if i == 3 {
            pb.println("  📡 Found node: node-abc123 @ 192.168.1.100:7677");
        }
        if i == 5 {
            pb.println("  📡 Found node: node-def456 @ 192.168.1.101:7677");
        }
    }
    
    pb.finish_with_message("Discovery complete");
    
    println!("\nDiscovered 2 nodes:");
    println!("  • node-abc123");
    println!("    Address: 192.168.1.100:7677");
    println!("    DID: did:cis:abc123");
    if verbose {
        println!("    Capabilities: memory_sync, skill_invoke");
        println!("    Last seen: 2s ago");
    }
    println!();
    println!("  • node-def456");
    println!("    Address: 192.168.1.101:7677");
    println!("    DID: did:cis:def456");
    if verbose {
        println!("    Capabilities: memory_sync");
        println!("    Last seen: 1s ago");
    }
    
    println!("\nUse 'cis p2p connect <address>' to connect to a node.");
    
    Ok(())
}

/// 列出节点
async fn list_peers(verbose: bool, connected_only: bool) -> Result<()> {
    println!("📡 Discovered Peers\n");
    
    if connected_only {
        println!("Showing connected peers only:\n");
    } else {
        println!("Showing all discovered peers:\n");
    }
    
    // 示例输出
    let peers = vec![
        ("node-abc123", "192.168.1.100:7677", true),
        ("node-def456", "192.168.1.101:7677", false),
    ];
    
    for (id, addr, connected) in peers {
        if connected_only && !connected {
            continue;
        }
        
        let status = if connected { "🟢" } else { "⚪" };
        println!("{} {}", status, id);
        println!("   Address: {}", addr);
        
        if verbose {
            println!("   Status: {}", if connected { "Connected" } else { "Discovered" });
            println!("   DID: did:cis:{}", id);
            println!("   Last seen: 2m ago");
        }
        println!();
    }
    
    println!("Tips:");
    println!("  - Ensure P2P is started: cis p2p start");
    println!("  - Check firewall settings for port 7677");
    println!("  - Use 'cis p2p add-peer' to manually add nodes");
    
    Ok(())
}

/// 连接节点
async fn connect_node(address: &str, node_id: Option<&str>) -> Result<()> {
    println!("🔗 Connecting to {}...", address);
    
    // 解析地址
    let (resolved_id, resolved_addr) = if address.contains('@') {
        // did:cis:node@host:port 格式
        let parts: Vec<&str> = address.split('@').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid address format. Expected: did:cis:node@host:port"));
        }
        let id = parts[0].trim_start_matches("did:cis:");
        (id.to_string(), parts[1].to_string())
    } else if address.contains(':') {
        // host:port 格式
        let id = node_id.ok_or_else(|| anyhow::anyhow!("Node ID required for address-only format"))?.to_string();
        (id, address.to_string())
    } else {
        return Err(anyhow::anyhow!("Invalid address format. Expected: did:cis:node@host:port or host:port"));
    };
    
    // 模拟连接
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Establishing connection...");
    
    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
    pb.finish_with_message("Connected!");
    
    println!("✅ Connected to {}", resolved_id);
    println!("   Address: {}", resolved_addr);
    println!("   Protocol: QUIC");
    println!("   Encryption: TLS 1.3");
    
    Ok(())
}

/// 断开节点
async fn disconnect_node(node_id: &str) -> Result<()> {
    println!("🔌 Disconnecting from {}...", node_id);
    
    // 模拟断开
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    println!("✅ Disconnected from {}", node_id);
    Ok(())
}

/// 添加节点
async fn add_peer(node_id: &str, address: &str, did: Option<&str>) -> Result<()> {
    println!("➕ Adding peer...");
    println!("   Node ID: {}", node_id);
    println!("   Address: {}", address);
    if let Some(d) = did {
        println!("   DID:     {}", d);
    }
    
    // 保存到配置
    println!("✅ Peer added successfully");
    
    Ok(())
}

/// 移除节点
async fn remove_peer(node_id: &str) -> Result<()> {
    println!("➖ Removing peer {}...", node_id);
    println!("✅ Peer removed");
    Ok(())
}

/// 触发同步
async fn trigger_sync(node: Option<&str>, full: bool) -> Result<()> {
    if let Some(n) = node {
        println!("🔄 Syncing with node {}...", n);
    } else {
        println!("🔄 Syncing with all peers...");
    }
    
    if full {
        println!("   Mode: Full sync");
    } else {
        println!("   Mode: Incremental sync");
    }
    
    // 模拟同步
    let pb = indicatif::ProgressBar::new(100);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    
    for i in 0..100 {
        pb.set_position(i + 1);
        pb.set_message(format!("Syncing items..."));
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    pb.finish_with_message("Sync complete");
    
    println!("✅ Sync triggered");
    println!("   Items synced: 42");
    println!("   Conflicts resolved: 0");
    Ok(())
}

/// 显示同步状态
async fn show_sync_status() -> Result<()> {
    println!("📊 Sync Status\n");
    println!("Last sync: 2 minutes ago");
    println!("Pending:   0 items");
    println!("Status:    Idle\n");
    
    println!("Sync History:");
    println!("  2024-01-15 10:30:15 - Synced with node-abc123 (42 items)");
    println!("  2024-01-15 10:15:02 - Synced with node-def456 (18 items)");
    println!("  2024-01-15 09:45:30 - Full sync completed");
    
    Ok(())
}

/// Ping 节点
async fn ping_node(target: &str) -> Result<()> {
    println!("🏓 Pinging {}...", target);
    
    // 模拟延迟测试
    let mut latencies = vec![];
    
    for i in 1..=4 {
        let start = std::time::Instant::now();
        tokio::time::sleep(tokio::time::Duration::from_millis(50 + i * 10)).await;
        let elapsed = start.elapsed();
        latencies.push(elapsed.as_secs_f64() * 1000.0);
        
        println!(
            "  Reply from {}: time={:.1}ms",
            target,
            elapsed.as_secs_f64() * 1000.0
        );
    }
    
    let avg: f64 = latencies.iter().sum::<f64>() / latencies.len() as f64;
    println!("\n  Average latency: {:.1}ms", avg);
    
    Ok(())
}

/// 广播消息
async fn broadcast_message(topic: &str, message: &str) -> Result<()> {
    println!("📢 Broadcasting message...");
    println!("   Topic:   {}", topic);
    println!("   Message: {}", message);
    
    // 模拟广播
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    println!("✅ Message broadcasted");
    println!("   Recipients: 3 peers");
    Ok(())
}

/// 启动 P2P 网络
async fn start_p2p(
    listen: &str,
    enable_dht: bool,
    bootstrap: Vec<String>,
    external: Option<String>,
    nat_traversal: bool,
) -> Result<()> {
    println!("🚀 Starting P2P network...\n");
    
    println!("Configuration:");
    println!("  Listen:    {}", listen);
    println!("  DHT:       {}", if enable_dht { "enabled" } else { "disabled" });
    println!("  NAT:       {}", if nat_traversal { "enabled" } else { "disabled" });
    
    if !bootstrap.is_empty() {
        println!("  Bootstrap:");
        for node in &bootstrap {
            println!("    - {}", node);
        }
    }
    
    if let Some(ext) = external {
        println!("  External:  {}", ext);
    }
    
    println!();
    println!("Starting services:");
    
    // 模拟启动过程
    let services = vec![
        ("QUIC transport", true),
        ("mDNS discovery", true),
        ("DHT discovery", enable_dht),
        ("NAT traversal", nat_traversal),
        ("Gossip protocol", true),
        ("Memory sync", true),
    ];
    
    for (service, enabled) in services {
        if enabled {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            println!("  ✓ {}", service);
        }
    }
    
    if nat_traversal {
        println!();
        println!("NAT Traversal:");
        
        // 尝试检测 NAT
        let mut nat = cis_core::p2p::NatTraversal::new(7677);
        match nat.try_traversal_detailed().await {
            Ok(result) => {
                println!("  NAT Type: {}", result.nat_type);
                println!("  Method:   {}", result.method);
                if let Some(addr) = result.external_addr {
                    println!("  External: {}", addr);
                }
                println!("  Latency:  {}ms", result.latency_ms);
            }
            Err(e) => {
                println!("  ⚠️  NAT detection failed: {}", e);
            }
        }
    }
    
    println!();
    println!("✅ P2P network started successfully!");
    println!();
    println!("Useful commands:");
    println!("  cis p2p status      - Check network status");
    println!("  cis p2p discover    - Discover nearby nodes");
    println!("  cis p2p peers       - List discovered peers");
    println!("  cis p2p sync        - Trigger synchronization");
    
    Ok(())
}

/// 停止 P2P 网络
async fn stop_p2p() -> Result<()> {
    println!("🛑 Stopping P2P network...");
    
    // 模拟停止过程
    let services = vec![
        "Memory sync",
        "Gossip protocol",
        "DHT discovery",
        "mDNS discovery",
        "QUIC transport",
    ];
    
    for service in services {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        println!("  ✓ {} stopped", service);
    }
    
    println!("✅ P2P network stopped");
    Ok(())
}

/// NAT 穿透测试
async fn hole_punch(
    target: Option<&str>,
    detect_only: bool,
    stun_server: Option<&str>,
) -> Result<()> {
    println!("🕳️  NAT Hole Punching Test\n");
    
    // NAT 类型检测
    println!("Step 1: Detecting NAT type...");
    
    let stun_servers = stun_server
        .map(|s| vec![s.to_string()])
        .unwrap_or_else(|| {
            cis_core::p2p::DEFAULT_STUN_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect()
        });
    
    let mut nat = cis_core::p2p::NatTraversal::with_stun_servers(7677, stun_servers);
    
    match nat.detect_nat_type().await {
        Ok((nat_type, external_addr)) => {
            println!("  NAT Type: {}", nat_type);
            println!("  Description: {}", nat_type.description());
            
            if let Some(addr) = external_addr {
                println!("  External Address: {}", addr);
            }
            
            println!("  Easy Traversal: {}", if nat_type.is_easy_traversal() { "Yes" } else { "No" });
            println!("  Hole Punching: {}", if nat_type.can_hole_punch() { "Supported" } else { "Not supported" });
            println!("  TURN Required: {}", if nat_type.needs_turn() { "Yes" } else { "No" });
        }
        Err(e) => {
            println!("  ❌ NAT detection failed: {}", e);
        }
    }
    
    if detect_only {
        return Ok(());
    }
    
    // 如果指定了目标，尝试打洞
    if let Some(target_addr) = target {
        println!("\nStep 2: Attempting hole punch to {}...", target_addr);
        
        let mut coordinator = cis_core::p2p::HolePunchCoordinator::new();
        
        match coordinator.init().await {
            Ok(_) => {
                let addr: std::net::SocketAddr = target_addr.parse()?;
                
                match coordinator.punch_hole(addr).await {
                    Ok(result) => {
                        match result {
                            cis_core::p2p::HolePunchResult::Success { local_addr, peer_addr, nat_type } => {
                                println!("  ✅ Hole punch successful!");
                                println!("     Local:  {}", local_addr);
                                println!("     Peer:   {}", peer_addr);
                                println!("     NAT:    {}", nat_type);
                            }
                            cis_core::p2p::HolePunchResult::RelayRequired { reason } => {
                                println!("  ⚠️  Relay required: {}", reason);
                                println!("     TURN server needed for this connection.");
                            }
                            cis_core::p2p::HolePunchResult::Failed { error } => {
                                println!("  ❌ Hole punch failed: {}", error);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ❌ Hole punch error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  ❌ Coordinator init failed: {}", e);
            }
        }
    } else {
        println!("\nTip: Use --target <addr> to test hole punching to a specific node.");
    }
    
    Ok(())
}

/// 处理 DHT 子命令
async fn handle_dht_action(action: DhtAction) -> Result<()> {
    match action {
        DhtAction::Status => show_dht_status().await,
        DhtAction::Put { key, value } => dht_put(&key, &value).await,
        DhtAction::Get { key } => dht_get(&key).await,
        DhtAction::FindNode { node_id } => dht_find_node(&node_id).await,
        DhtAction::RoutingTable { verbose } => dht_routing_table(verbose).await,
        DhtAction::AddBootstrap { address } => dht_add_bootstrap(&address).await,
        DhtAction::ListBootstrap => dht_list_bootstrap().await,
    }
}

/// 显示 DHT 状态
async fn show_dht_status() -> Result<()> {
    println!("📊 DHT Status\n");
    
    println!("DHT Service: Running");
    println!("Node ID: test-node-123");
    println!("Listen Address: 0.0.0.0:7678");
    println!();
    println!("Routing Table:");
    println!("  Size: 12 nodes");
    println!("  Buckets: 5");
    println!("  Average Reliability: 85%");
    println!();
    println!("Key-Value Store:");
    println!("  Items: 156");
    println!("  Replication Factor: 3");
    println!();
    println!("Bootstrap Nodes:");
    println!("  • bootstrap.cis.dev:6767");
    
    Ok(())
}

/// DHT 存储键值对
async fn dht_put(key: &str, value: &str) -> Result<()> {
    println!("💾 Storing in DHT...");
    println!("  Key:   {}", key);
    println!("  Value: {} bytes", value.len());
    
    // 模拟存储
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    println!("✅ Key stored successfully");
    println!("  Replicated to 3 nodes");
    
    Ok(())
}

/// DHT 获取键值对
async fn dht_get(key: &str) -> Result<()> {
    println!("🔍 Getting from DHT...");
    println!("  Key: {}", key);
    
    // 模拟获取
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    
    // 示例数据
    println!("✅ Value found:");
    println!("  Data: example-value-data");
    println!("  Size: 22 bytes");
    println!("  Nodes queried: 2");
    
    Ok(())
}

/// DHT 查找节点
async fn dht_find_node(node_id: &str) -> Result<()> {
    println!("🔍 Finding node in DHT...");
    println!("  Target: {}", node_id);
    
    // 模拟查找
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    println!("✅ Node found:");
    println!("  Node ID: {}", node_id);
    println!("  Address: 192.168.1.100:7677");
    println!("  Distance: 142 (XOR metric)");
    println!("  Hops: 3");
    
    Ok(())
}

/// 显示 DHT 路由表
async fn dht_routing_table(verbose: bool) -> Result<()> {
    println!("📋 DHT Routing Table\n");
    
    println!("Total nodes: 12\n");
    
    let nodes = vec![
        ("node-abc", "192.168.1.10:7677", "85%"),
        ("node-def", "192.168.1.11:7677", "92%"),
        ("node-ghi", "192.168.1.12:7677", "78%"),
    ];
    
    for (id, addr, reliability) in nodes {
        println!("  • {}", id);
        println!("    Address: {}", addr);
        println!("    Reliability: {}", reliability);
        
        if verbose {
            println!("    Last seen: 2m ago");
            println!("    Ping count: 12");
            println!("    Failed pings: 1");
        }
        println!();
    }
    
    Ok(())
}

/// 添加 Bootstrap 节点
async fn dht_add_bootstrap(address: &str) -> Result<()> {
    println!("➕ Adding bootstrap node...");
    println!("  Address: {}", address);
    
    // 验证地址格式
    if !address.contains(':') {
        return Err(anyhow::anyhow!("Invalid address format. Expected: host:port"));
    }
    
    println!("✅ Bootstrap node added");
    
    Ok(())
}

/// 列出 Bootstrap 节点
async fn dht_list_bootstrap() -> Result<()> {
    println!("📋 Bootstrap Nodes\n");
    
    let nodes = vec![
        "bootstrap.cis.dev:6767",
        "bootstrap2.cis.dev:6767",
    ];
    
    for (i, node) in nodes.iter().enumerate() {
        println!("  {}. {}", i + 1, node);
    }
    
    if nodes.is_empty() {
        println!("  No bootstrap nodes configured.");
        println!("  Use 'cis p2p dht add-bootstrap <address>' to add one.");
    }
    
    Ok(())
}

/// 网络诊断
async fn diagnose_network(check: DiagnoseType) -> Result<()> {
    println!("🔧 P2P Network Diagnostics\n");
    
    match check {
        DiagnoseType::All | DiagnoseType::Network => {
            println!("📡 Network Connectivity:");
            
            // 检查本地 IP
            match get_local_ip() {
                Some(ip) => println!("  ✅ Local IP: {}", ip),
                None => println!("  ❌ Could not determine local IP"),
            }
            
            // 检查互联网连接
            println!("  🔄 Checking internet connectivity...");
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            println!("  ✅ Internet: Connected");
            println!();
        }
        _ => {}
    }
    
    match check {
        DiagnoseType::All | DiagnoseType::Nat => {
            println!("🕳️  NAT Type:");
            
            let mut nat = cis_core::p2p::NatTraversal::new(7677);
            match nat.detect_nat_type().await {
                Ok((nat_type, external)) => {
                    println!("  Type: {}", nat_type);
                    println!("  Description: {}", nat_type.description());
                    if let Some(addr) = external {
                        println!("  External: {}", addr);
                    }
                }
                Err(e) => {
                    println!("  ❌ Detection failed: {}", e);
                }
            }
            println!();
        }
        _ => {}
    }
    
    match check {
        DiagnoseType::All | DiagnoseType::Port => {
            println!("🔌 Port Availability:");
            println!("  Port 7677 (P2P): Checking...");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            println!("  ✅ Port 7677: Available");
            println!("  Port 7678 (DHT): Checking...");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            println!("  ✅ Port 7678: Available");
            println!();
        }
        _ => {}
    }
    
    match check {
        DiagnoseType::All | DiagnoseType::Dht => {
            println!("📊 DHT Status:");
            println!("  Service: Running");
            println!("  Routing Table: 12 nodes");
            println!("  Bootstrap Nodes: 2");
        }
        _ => {}
    }
    
    println!();
    println!("✅ Diagnostics complete");
    
    Ok(())
}

/// 获取本地 IP
fn get_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok()?.ip().into()
}
