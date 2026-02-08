//! Peer 管理命令
//!
//! 管理联邦网络中的对等节点

use anyhow::{Context, Result};
use cis_core::storage::federation_db::{FederationDb, PeerInfo, PeerStatus, TrustLevel};
use cis_core::storage::paths::Paths;

/// 添加对等节点
pub fn add_peer(
    node_id: &str,
    did: &str,
    endpoint: &str,
) -> Result<()> {
    let db = open_federation_db()?;
    
    let peer = PeerInfo {
        node_id: node_id.to_string(),
        did: did.to_string(),
        endpoint_ws: Some(endpoint.to_string()),
        status: PeerStatus::Offline,
        last_seen: 0,
        rtt_ms: None,
        public_key: String::new(), // 从 DID 解析获取
    };
    
    db.upsert_peer(&peer)
        .context("Failed to add peer to database")?;
    
    println!("✅ Added peer: {} ({})", node_id, did);
    println!("   Endpoint: {}", endpoint);
    
    Ok(())
}

/// 移除对等节点
pub fn remove_peer(node_id: &str) -> Result<()> {
    let db = open_federation_db()?;
    
    // 检查节点是否存在
    if db.get_peer(node_id)?.is_none() {
        return Err(anyhow::anyhow!("Peer '{}' not found", node_id));
    }
    
    // 从数据库删除
    db.conn().execute(
        "DELETE FROM network_peers WHERE node_id = ?1",
        [node_id],
    )?;
    
    println!("✅ Removed peer: {}", node_id);
    Ok(())
}

/// 列出所有对等节点
pub fn list_peers(show_offline: bool) -> Result<()> {
    let db = open_federation_db()?;
    
    // 查询所有节点
    let mut stmt = db.conn().prepare(
        "SELECT node_id, did, endpoint_ws, status, last_seen, rtt_ms 
         FROM network_peers 
         ORDER BY status DESC, last_seen DESC"
    )?;
    
    let peers = stmt.query_map([], |row| {
        Ok(PeerInfo {
            node_id: row.get(0)?,
            did: row.get(1)?,
            endpoint_ws: row.get(2)?,
            status: PeerStatus::from_i32(row.get(3)?),
            last_seen: row.get(4)?,
            rtt_ms: row.get(5)?,
            public_key: String::new(),
        })
    })?;
    
    let mut count = 0;
    let mut online_count = 0;
    
    println!("\n{:<20} {:<25} {:<12} {:<30} {:<10}", 
             "NODE ID", "DID", "STATUS", "ENDPOINT", "RTT(ms)");
    println!("{}", "-".repeat(105));
    
    for peer in peers {
        let peer = peer?;
        
        if !show_offline && peer.status == PeerStatus::Offline {
            continue;
        }
        
        if peer.status == PeerStatus::Online {
            online_count += 1;
        }
        count += 1;
        
        let status_str = match peer.status {
            PeerStatus::Online => "🟢 online",
            PeerStatus::Offline => "⚪ offline",
            PeerStatus::HolePunching => "🟡 hole-punching",
        };
        
        let endpoint = peer.endpoint_ws.unwrap_or_else(|| "-".to_string());
        let rtt_str = peer.rtt_ms.map(|r| r.to_string()).unwrap_or_else(|| "-".to_string());
        
        // 截断显示
        let did_short = if peer.did.len() > 25 {
            format!("{}...", &peer.did[..22])
        } else {
            peer.did.clone()
        };
        
        println!("{:<20} {:<25} {:<12} {:<30} {:<10}",
            peer.node_id,
            did_short,
            status_str,
            endpoint,
            rtt_str
        );
    }
    
    println!("\nTotal: {} peers ({} online)", count, online_count);
    
    Ok(())
}

/// 查看节点详情
pub fn peer_info(node_id: &str) -> Result<()> {
    let db = open_federation_db()?;
    
    let peer = db.get_peer(node_id)?
        .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", node_id))?;
    
    println!("\n📡 Peer Information");
    println!("{}", "=".repeat(50));
    println!("Node ID:    {}", peer.node_id);
    println!("DID:        {}", peer.did);
    println!("Endpoint:   {}", peer.endpoint_ws.as_deref().unwrap_or("N/A"));
    println!("Status:     {:?}", peer.status);
    println!("Last Seen:  {}", format_timestamp(peer.last_seen));
    println!("RTT:        {}ms", peer.rtt_ms.map(|r| r.to_string()).unwrap_or_else(|| "N/A".to_string()));
    
    Ok(())
}

/// 设置节点信任级别
pub fn set_trust(node_id: &str, trust_level: &str) -> Result<()> {
    let level = match trust_level.to_lowercase().as_str() {
        "block" | "blocked" | "0" => TrustLevel::Blocked,
        "read" | "1" => TrustLevel::Read,
        "write" | "2" => TrustLevel::Write,
        _ => return Err(anyhow::anyhow!("Invalid trust level. Use: block, read, write")),
    };
    
    let db = open_federation_db()?;
    
    // 获取本节点 DID（简化处理，实际应从配置读取）
    let local_did = format!("did:cis:{}", whoami::username());
    
    // 获取目标节点 DID
    let peer = db.get_peer(node_id)?
        .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", node_id))?;
    
    db.set_trust(&local_did, &peer.did, level)?;
    
    println!("✅ Set trust level for {} to {:?}", node_id, level);
    
    Ok(())
}

/// 测试节点连接
pub async fn ping_peer(node_id: &str) -> Result<()> {
    let db = open_federation_db()?;
    
    let peer = db.get_peer(node_id)?
        .ok_or_else(|| anyhow::anyhow!("Peer '{}' not found", node_id))?;
    
    let endpoint = peer.endpoint_ws
        .ok_or_else(|| anyhow::anyhow!("Peer '{}' has no WebSocket endpoint", node_id))?;
    
    println!("📡 Pinging {} at {}...", node_id, endpoint);
    
    // 尝试 WebSocket 连接
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        try_connect_ws(&endpoint)
    ).await {
        Ok(Ok(rtt)) => {
            println!("✅ Peer is online (RTT: {}ms)", rtt);
            
            // 更新数据库
            db.update_peer_status(node_id, PeerStatus::Online)?;
            db.update_peer_rtt(node_id, rtt as i32)?;
        }
        Ok(Err(e)) => {
            println!("❌ Connection failed: {}", e);
        }
        Err(_) => {
            println!("❌ Connection timeout");
        }
    }
    
    Ok(())
}

/// 同步队列状态
pub fn sync_status() -> Result<()> {
    let db = open_federation_db()?;
    
    let tasks = db.get_pending_tasks(1000)?;
    
    println!("\n📋 Sync Queue Status");
    println!("{}", "=".repeat(50));
    println!("Pending tasks: {}", tasks.len());
    
    if !tasks.is_empty() {
        println!("\n{:<20} {:<25} {:<20}", "TARGET", "ROOM", "SINCE");
        println!("{}", "-".repeat(70));
        
        for task in tasks.iter().take(10) {
            println!("{:<20} {:<25} {:<20}",
                task.target_node,
                task.room_id,
                task.since_event_id
            );
        }
        
        if tasks.len() > 10 {
            println!("\n... and {} more tasks", tasks.len() - 10);
        }
    }
    
    Ok(())
}

// Helper functions
fn open_federation_db() -> Result<FederationDb> {
    let db_path = Paths::data_dir().join("federation.db");
    FederationDb::open(&db_path)
        .context("Failed to open federation database")
}

fn format_timestamp(ts: i64) -> String {
    if ts == 0 {
        return "Never".to_string();
    }
    let dt = chrono::DateTime::from_timestamp(ts, 0)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH);
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

async fn try_connect_ws(endpoint: &str) -> Result<u64> {
    use tokio_tungstenite::connect_async;
    
    let start = std::time::Instant::now();
    let (_, _) = connect_async(endpoint).await?;
    let rtt = start.elapsed().as_millis() as u64;
    
    Ok(rtt)
}
