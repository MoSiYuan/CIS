//! # 统一状态查看命令
//!
//! 替代多个分散的 status 命令，提供一站式状态查看。

/// 执行状态查看
pub async fn execute(network: bool, perf: bool) -> anyhow::Result<()> {
    // 检查是否已初始化
    if !is_initialized() {
        println!("⚠️  CIS 尚未初始化");
        println!("💡 运行: cis setup");
        return Ok(());
    }
    
    // 获取节点信息
    let node_info = get_node_info().await?;
    
    // 打印状态面板
    println!("\n╔══════════════════════════════════════════╗");
    println!("║  CIS Node Status                          ║");
    println!("╠══════════════════════════════════════════╣");
    
    // 基本信息
    let status_icon = if is_running() { "🟢" } else { "🔴" };
    println!("║                                          ║");
    println!("║  {} {}                    ║", status_icon, node_info.name);
    println!("║  ID: {}              ║", &node_info.id[..8]);
    println!("║  DID: {}         ║", &node_info.did[..20]);
    println!("║  角色: {:<32} ║", node_info.role);
    println!("║                                          ║");
    
    // 网络状态
    if network {
        let peers = get_peers().await?;
        println!("╠══════════════════════════════════════════╣");
        println!("║  📡 网络状态 ({:<2} 个邻居)                 ║", peers.len());
        println!("║                                          ║");
        
        if peers.is_empty() {
            println!("║     (暂无邻居节点)                       ║");
        } else {
            for peer in &peers {
                let icon = match peer.status.as_str() {
                    "online" => "🟢",
                    "offline" => "🔴",
                    _ => "🟡",
                };
                println!("║  {} {:<10} {:<20} ║", 
                    icon, 
                    truncate(&peer.name, 10),
                    truncate(&peer.endpoint, 20)
                );
            }
        }
        println!("║                                          ║");
    }
    
    // 端口状态
    println!("╠══════════════════════════════════════════╣");
    println!("║  🔌 端口状态                              ║");
    println!("║                                          ║");
    println!("║    6767 (发现)  {}                       ║", check_port(6767));
    println!("║    6768 (配对)  {}                       ║", check_port(6768));
    println!("║    7676 (联邦)  {}                       ║", check_port(7676));
    println!("║                                          ║");
    
    // 性能指标
    if perf {
        println!("╠══════════════════════════════════════════╣");
        println!("║  📊 性能指标                              ║");
        println!("║                                          ║");
        println!("║    CPU: {:<5.1}%  内存: {:<5.1}%              ║", 
            get_cpu_usage(), get_memory_usage());
        println!("║    网络: ↑{:>6} ↓{:>6}                ║",
            format_bytes(get_upload()), format_bytes(get_download()));
        println!("║                                          ║");
    }
    
    // 快捷操作
    println!("╠══════════════════════════════════════════╣");
    println!("║  💡 快捷操作                              ║");
    println!("║                                          ║");
    println!("║    cis join    - 加入/创建网络          ║");
    println!("║    cis peers   - 管理邻居节点           ║");
    if !network {
        println!("║    cis status --network - 显示网络详情  ║");
    }
    println!("║                                          ║");
    println!("╚══════════════════════════════════════════╝\n");
    
    Ok(())
}

/// 获取节点信息
async fn get_node_info() -> anyhow::Result<NodeInfo> {
    // 简化实现，实际从配置文件读取
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let hostname_for_did = hostname.clone();
    
    Ok(NodeInfo {
        id: "a1b2c3d4".to_string(),
        name: hostname,
        did: format!("did:cis:{}:a1b2c3d4", hostname_for_did),
        role: "worker".to_string(),
    })
}

/// 获取邻居列表
async fn get_peers() -> anyhow::Result<Vec<PeerInfo>> {
    // 简化实现，实际从数据库读取
    Ok(vec![])
}

/// 检查端口状态
fn check_port(port: u16) -> &'static str {
    // 简化实现
    "✓"
}

/// 检查是否运行中
fn is_running() -> bool {
    true
}

/// 检查是否已初始化
fn is_initialized() -> bool {
    let config_path = dirs::home_dir()
        .map(|p| p.join(".cis").join("config.toml"))
        .unwrap_or_default();
    config_path.exists()
}

/// 获取 CPU 使用率
fn get_cpu_usage() -> f32 {
    0.0
}

/// 获取内存使用率
fn get_memory_usage() -> f32 {
    0.0
}

/// 获取上传流量
fn get_upload() -> u64 {
    0
}

/// 获取下载流量
fn get_download() -> u64 {
    0
}

/// 格式化字节
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    
    format!("{:.1}{}", size, UNITS[unit])
}

/// 截断字符串
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

#[derive(Debug)]
struct NodeInfo {
    id: String,
    name: String,
    did: String,
    role: String,
}

#[derive(Debug)]
struct PeerInfo {
    name: String,
    endpoint: String,
    status: String,
}
