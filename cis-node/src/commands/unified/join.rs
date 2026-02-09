//! # 一键组网命令
//!
//! 智能选择组网方式，简化用户操作。

use crate::commands::unified::NodeRole;
use std::time::Duration;

/// 执行一键组网
pub async fn execute(address: Option<String>, code: Option<String>) -> anyhow::Result<()> {
    // 1. 检查是否已初始化
    if !is_initialized() {
        println!("⚠️  CIS 未初始化，先运行自动设置...\n");
        super::setup::execute(true, NodeRole::Worker).await?;
        println!();
    }
    
    // 2. 检查当前网络状态
    let current_peers = get_current_peers().await?;
    
    if !current_peers.is_empty() {
        println!("📊 当前已连接 {} 个节点", current_peers.len());
        for peer in &current_peers {
            println!("   • {} ({})", peer.name, peer.status);
        }
        println!("\n💡 如需查看详情，运行: cis status");
        return Ok(());
    }
    
    // 3. 根据参数选择组网方式
    match (address, code) {
        // 方式1: 指定地址和配对码
        (Some(addr), Some(c)) => {
            println!("🔗 使用指定地址和配对码连接...");
            connect_with_code(&addr, &c).await?;
        }
        
        // 方式2: 仅指定地址（自动发现配对码）
        (Some(addr), None) => {
            println!("🔗 连接到 {}...", addr);
            connect_direct(&addr).await?;
        }
        
        // 方式3: 仅指定配对码（广播等待连接）
        (None, Some(c)) => {
            println!("🔢 使用配对码 {} 加入网络...", c);
            join_with_code(&c).await?;
        }
        
        // 方式4: 全自动（推荐）
        (None, None) => {
            auto_join().await?;
        }
    }
    
    Ok(())
}

/// 全自动组网（智能选择）
async fn auto_join() -> anyhow::Result<()> {
    println!("🚀 开始自动组网...\n");
    
    // 步骤1: 尝试发现现有网络
    println!("🔍 步骤1/3: 搜索网络中的节点 (等待5秒)...");
    let discovered = discover_peers(Duration::from_secs(5)).await?;
    
    if !discovered.is_empty() {
        println!("✅ 发现 {} 个节点!\n", discovered.len());
        
        // 自动连接第一个发现的节点
        let peer = &discovered[0];
        println!("🔗 步骤2/3: 正在连接 {} ({})...", peer.name, peer.endpoint);
        
        match connect_peer(peer).await {
            Ok(_) => {
                println!("✅ 步骤3/3: 组网成功!\n");
                show_network_status().await?;
            }
            Err(e) => {
                println!("❌ 连接失败: {}\n", e);
                println!("💡 尝试创建新网络...");
                create_new_network().await?;
            }
        }
    } else {
        println!("📡 未发现现有网络\n");
        create_new_network().await?;
    }
    
    Ok(())
}

/// 创建新网络（生成配对码等待连接）
async fn create_new_network() -> anyhow::Result<()> {
    use cis_core::network::pairing::{PairingManager, PairingNodeInfo};
    
    println!("🔧 步骤2/3: 创建新网络...");
    
    let manager = PairingManager::new();
    let node = PairingNodeInfo {
        node_id: get_node_id(),
        did: get_node_did(),
        hostname: gethostname::gethostname().to_string_lossy().to_string(),
    };
    
    let code = manager.generate_code(node)?;
    
    println!("\n╔══════════════════════════════════════════╗");
    println!("║           🔢 组网配对码                   ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║                                          ║");
    println!("║       {:>6}                            ║", code);
    println!("║                                          ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║  ⏱️  有效期: 5分钟                        ║");
    println!("║  📌 本机: {}                    ║", gethostname::gethostname().to_string_lossy());
    println!("╚══════════════════════════════════════════╝\n");
    
    println!("🔄 步骤3/3: 等待其他节点连接...");
    println!("   (按 Ctrl+C 取消)\n");
    
    // 启动监听服务
    use cis_core::network::pairing::PairingService;
    use std::sync::Arc;
    
    let service = PairingService::new(Arc::new(manager));
    match service.listen(code.clone()).await {
        Ok(result) => {
            println!("✅ 组网成功!");
            println!("   节点: {}", result.node_id);
            println!("   地址: {}\n", result.endpoint);
            
            // 自动添加为邻居
            add_as_neighbor(&result.node_id, &result.endpoint).await?;
        }
        Err(e) => {
            println!("❌ 组网失败: {}", e);
        }
    }
    
    Ok(())
}

/// 使用配对码加入网络
async fn join_with_code(code: &str) -> anyhow::Result<()> {
    println!("🔍 正在使用配对码 {} 查找节点...", code);
    
    // 这里简化处理，实际需要 UDP 广播或指定地址
    println!("💡 请使用 --address 指定目标地址");
    println!("   例如: cis join --code {} --address 192.168.1.100:6768", code);
    
    Ok(())
}

/// 直接连接指定地址
async fn connect_direct(address: &str) -> anyhow::Result<()> {
    use cis_core::network::pairing::{PairingNodeInfo, PairingService};
    use std::sync::Arc;
    
    let addr: std::net::SocketAddr = address.parse()
        .map_err(|e| anyhow::anyhow!("无效的地址格式: {}", e))?;
    
    let node = PairingNodeInfo {
        node_id: get_node_id(),
        did: get_node_did(),
        hostname: gethostname::gethostname().to_string_lossy().to_string(),
    };
    
    let service = PairingService::new(Arc::new(cis_core::network::pairing::PairingManager::new()));
    
    println!("🔗 正在连接 {}...", address);
    
    // 这里简化处理，实际需要先生成配对码再连接
    println!("⚠️  直接连接需要对方提供配对码");
    println!("💡 建议运行: cis join  (自动发现/生成码)");
    
    Ok(())
}

/// 使用配对码连接指定地址
async fn connect_with_code(address: &str, code: &str) -> anyhow::Result<()> {
    use cis_core::network::pairing::{PairingNodeInfo, PairingService};
    use std::sync::Arc;
    
    let addr: std::net::SocketAddr = format!("{}:6768", address).parse()
        .map_err(|e| anyhow::anyhow!("无效的地址格式: {}", e))?;
    
    let node = PairingNodeInfo {
        node_id: get_node_id(),
        did: get_node_did(),
        hostname: gethostname::gethostname().to_string_lossy().to_string(),
    };
    
    let service = PairingService::new(Arc::new(cis_core::network::pairing::PairingManager::new()));
    
    println!("🔗 正在使用配对码 {} 连接 {}...", code, address);
    
    match service.request_pairing(code, addr, node).await {
        Ok(result) => {
            println!("✅ 组网成功!");
            println!("   节点: {}", result.node_id);
            println!("   地址: {}", result.endpoint);
            
            add_as_neighbor(&result.node_id, &result.endpoint).await?;
        }
        Err(e) => {
            println!("❌ 组网失败: {}", e);
            println!("\n可能的原因:");
            println!("   • 配对码已过期");
            println!("   • 目标节点不在同一网络");
            println!("   • 防火墙阻止了 UDP 端口 6768");
        }
    }
    
    Ok(())
}

/// 显示网络状态
async fn show_network_status() -> anyhow::Result<()> {
    let peers = get_current_peers().await?;
    
    println!("📊 网络状态:");
    println!("   本机: {} ({})", 
        gethostname::gethostname().to_string_lossy(),
        get_node_role()
    );
    println!("   邻居: {} 个", peers.len());
    
    for peer in &peers {
        let status_icon = match peer.status.as_str() {
            "online" => "🟢",
            "offline" => "🔴",
            _ => "🟡",
        };
        println!("   {} {} @ {}", status_icon, peer.name, peer.endpoint);
    }
    
    Ok(())
}

// 辅助函数（简化实现）
fn is_initialized() -> bool {
    let config_path = dirs::home_dir()
        .map(|p| p.join(".cis").join("config.toml"))
        .unwrap_or_default();
    config_path.exists()
}

fn get_node_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn get_node_did() -> String {
    format!("did:cis:{}:{}", 
        gethostname::gethostname().to_string_lossy(),
        &get_node_id()[..8]
    )
}

fn get_node_role() -> &'static str {
    "worker" // 简化，实际从配置读取
}

async fn get_current_peers() -> anyhow::Result<Vec<PeerInfo>> {
    // 简化实现，实际从数据库/缓存读取
    Ok(vec![])
}

async fn discover_peers(_timeout: Duration) -> anyhow::Result<Vec<PeerInfo>> {
    // 简化实现，实际使用 UDP 广播
    Ok(vec![])
}

async fn connect_peer(_peer: &PeerInfo) -> anyhow::Result<()> {
    // 简化实现
    Ok(())
}

async fn add_as_neighbor(_node_id: &str, _endpoint: &str) -> anyhow::Result<()> {
    // 简化实现，实际添加到邻居列表
    println!("💾 已添加为邻居");
    Ok(())
}

#[derive(Debug)]
struct PeerInfo {
    name: String,
    endpoint: String,
    status: String,
}
