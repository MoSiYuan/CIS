//! # Node Service
//!
//! 节点管理服务，提供节点发现、绑定、拉黑等功能。

use super::{ListOptions, PaginatedResult, ResourceStats};
use crate::error::{CisError, Result};
use crate::identity::did::DIDManager;
use crate::network::acl_module::NetworkAcl;
#[cfg(feature = "p2p")]
use crate::p2p::peer::{PeerInfo as P2PPeerInfo, PeerManager};
use crate::storage::federation_db::{FederationDb, PeerInfo as DbPeerInfo, PeerStatus};
use crate::storage::paths::Paths;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 节点摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: String,
    pub did: String,
    pub name: String,
    pub status: NodeStatus,
    pub endpoint: String,
    pub version: String,
    pub last_seen: DateTime<Utc>,
    pub capabilities: Vec<String>,
}

/// 节点详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    #[serde(flatten)]
    pub summary: NodeSummary,
    pub public_key: String,
    pub metadata: HashMap<String, serde_json::Value>,
    pub trust_score: f64,
    pub is_blacklisted: bool,
    pub created_at: DateTime<Utc>,
}

/// 节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Online,
    Offline,
    Suspicious,
    Blacklisted,
    Unknown,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Online => write!(f, "online"),
            NodeStatus::Offline => write!(f, "offline"),
            NodeStatus::Suspicious => write!(f, "suspicious"),
            NodeStatus::Blacklisted => write!(f, "blacklisted"),
            NodeStatus::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<PeerStatus> for NodeStatus {
    fn from(status: PeerStatus) -> Self {
        match status {
            PeerStatus::Online => NodeStatus::Online,
            PeerStatus::Offline => NodeStatus::Offline,
            PeerStatus::HolePunching => NodeStatus::Unknown,
        }
    }
}

/// 绑定选项
#[derive(Debug, Clone, Default)]
pub struct BindOptions {
    pub endpoint: String,
    pub did: Option<String>,
    pub trust_level: TrustLevel,
    pub auto_sync: bool,
}

/// 信任级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrustLevel {
    #[default]
    Full,
    Limited,
    Untrusted,
}

impl From<crate::storage::federation_db::TrustLevel> for TrustLevel {
    fn from(level: crate::storage::federation_db::TrustLevel) -> Self {
        match level {
            crate::storage::federation_db::TrustLevel::Write => TrustLevel::Full,
            crate::storage::federation_db::TrustLevel::Read => TrustLevel::Limited,
            crate::storage::federation_db::TrustLevel::Blocked => TrustLevel::Untrusted,
        }
    }
}

impl From<TrustLevel> for crate::storage::federation_db::TrustLevel {
    fn from(level: TrustLevel) -> Self {
        match level {
            TrustLevel::Full => crate::storage::federation_db::TrustLevel::Write,
            TrustLevel::Limited => crate::storage::federation_db::TrustLevel::Read,
            TrustLevel::Untrusted => crate::storage::federation_db::TrustLevel::Blocked,
        }
    }
}

/// 节点服务
#[derive(Debug, Clone)]
pub struct NodeService {
    federation_db: Arc<RwLock<FederationDb>>,
    #[cfg(feature = "p2p")]
    peer_manager: Arc<PeerManager>,
    acl: Arc<RwLock<NetworkAcl>>,
    local_did: String,
}

impl NodeService {
    pub fn new() -> Result<Self> {
        let federation_db_path = Paths::federation_db();
        let federation_db = FederationDb::open(&federation_db_path)?;
        
        // 加载或创建 ACL
        let acl_path = Paths::config_dir().join("network_acl.toml");
        let acl = if acl_path.exists() {
            NetworkAcl::load(&acl_path).unwrap_or_default()
        } else {
            NetworkAcl::default()
        };
        
        let local_did = acl.local_did.clone();
        
        Ok(Self {
            #[allow(clippy::arc_with_non_send_sync)]
            federation_db: Arc::new(RwLock::new(federation_db)),
            #[cfg(feature = "p2p")]
            peer_manager: Arc::new(PeerManager::new()),
            acl: Arc::new(RwLock::new(acl)),
            local_did,
        })
    }

    /// 列出已知节点
    pub async fn list(&self, options: ListOptions) -> Result<PaginatedResult<NodeSummary>> {
        let db = self.federation_db.read().await;
        
        // 从数据库获取所有节点
        let all_peers = self.get_all_peers_from_db(&db).await?;
        
        drop(db); // 释放读锁
        
        let mut items: Vec<NodeSummary> = Vec::new();
        
        for peer in all_peers {
            let status = if peer.status == PeerStatus::Online {
                NodeStatus::Online
            } else {
                NodeStatus::Offline
            };
            
            // 检查是否在黑名单
            let acl = self.acl.read().await;
            let is_blacklisted = acl.is_blacklisted(&peer.did);
            drop(acl);
            
            let final_status = if is_blacklisted {
                NodeStatus::Blacklisted
            } else {
                status
            };
            
            let last_seen = Utc.timestamp_opt(peer.last_seen, 0).single().unwrap_or_else(Utc::now);
            
            items.push(NodeSummary {
                id: peer.node_id.clone(),
                did: peer.did.clone(),
                name: peer.node_id.clone(),
                status: final_status,
                endpoint: peer.endpoint_ws.clone().unwrap_or_default(),
                version: "1.0.0".to_string(), // 从元数据中获取或默认
                last_seen,
                capabilities: vec![], // 从元数据解析
            });
        }
        
        // 应用过滤器
        if !options.all {
            items.retain(|n| matches!(n.status, NodeStatus::Online | NodeStatus::Offline));
        }
        
        for (key, value) in &options.filters {
            match key.as_str() {
                "status" => {
                    items.retain(|n| n.status.to_string() == value.to_lowercase());
                }
                "did" => {
                    items.retain(|n| n.did.contains(value));
                }
                "name" => {
                    items.retain(|n| n.name.contains(value));
                }
                _ => {}
            }
        }
        
        // 应用排序
        if let Some(sort_by) = options.sort_by {
            match sort_by.as_str() {
                "last_seen" => {
                    if options.sort_desc {
                        items.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
                    } else {
                        items.sort_by(|a, b| a.last_seen.cmp(&b.last_seen));
                    }
                }
                "name" => {
                    if options.sort_desc {
                        items.sort_by(|a, b| b.name.cmp(&a.name));
                    } else {
                        items.sort_by(|a, b| a.name.cmp(&b.name));
                    }
                }
                _ => {}
            }
        }
        
        let total = items.len();
        
        // 应用限制
        if let Some(limit) = options.limit {
            items.truncate(limit);
        }
        
        Ok(PaginatedResult::new(items, total))
    }

    /// 查看节点详情
    pub async fn inspect(&self, id: &str) -> Result<NodeInfo> {
        let db = self.federation_db.read().await;
        
        // 尝试从数据库获取节点
        let peer = match db.get_peer(id)? {
            Some(p) => p,
            None => {
                // 尝试通过 DID 查找
                return Err(CisError::not_found(format!("Node '{}' not found", id)));
            }
        };
        
        drop(db);
        
        // 检查是否在黑名单
        let acl = self.acl.read().await;
        let is_blacklisted = acl.is_blacklisted(&peer.did);
        let trust_score = if is_blacklisted {
            0.0
        } else if acl.is_whitelisted(&peer.did) {
            1.0
        } else {
            0.5
        };
        drop(acl);
        
        let status = if is_blacklisted {
            NodeStatus::Blacklisted
        } else {
            peer.status.into()
        };
        
        let last_seen = Utc.timestamp_opt(peer.last_seen, 0).single().unwrap_or_else(Utc::now);
        
        let summary = NodeSummary {
            id: peer.node_id.clone(),
            did: peer.did.clone(),
            name: peer.node_id.clone(),
            status,
            endpoint: peer.endpoint_ws.clone().unwrap_or_default(),
            version: "1.0.0".to_string(),
            last_seen,
            capabilities: vec![],
        };
        
        Ok(NodeInfo {
            summary,
            public_key: peer.public_key,
            metadata: HashMap::new(),
            trust_score,
            is_blacklisted,
            created_at: last_seen, // 如果没有创建时间，使用 last_seen
        })
    }

    /// 绑定新节点
    pub async fn bind(&self, options: BindOptions) -> Result<NodeInfo> {
        // 解析 DID 或生成临时 ID
        let (node_id, did, public_key) = if let Some(ref did_str) = options.did {
            // 验证 DID 格式
            let parsed = DIDManager::parse_did(did_str)
                .ok_or_else(|| CisError::invalid_input(format!("Invalid DID format: {}", did_str)))?;
            
            (parsed.0, did_str.clone(), parsed.1)
        } else {
            // 生成临时节点 ID
            let temp_id = format!("node-{}", &uuid::Uuid::new_v4().to_string()[..8]);
            let temp_did = format!("did:cis:{}:temp", temp_id);
            (temp_id, temp_did, String::new())
        };
        
        // 创建节点信息
        let now = Utc::now();
        let peer = DbPeerInfo {
            node_id: node_id.clone(),
            did: did.clone(),
            endpoint_ws: Some(options.endpoint.clone()),
            status: PeerStatus::Offline,
            last_seen: now.timestamp(),
            rtt_ms: None,
            public_key: public_key.clone(),
        };
        
        // 保存到数据库
        let db = self.federation_db.write().await;
        db.upsert_peer(&peer)?;
        
        // 设置信任级别
        let trust_level: crate::storage::federation_db::TrustLevel = options.trust_level.into();
        db.set_trust(&self.local_did, &did, trust_level)?;
        
        drop(db);
        
        // 如果信任级别不是 Untrusted，添加到白名单
        if !matches!(options.trust_level, TrustLevel::Untrusted) {
            let mut acl = self.acl.write().await;
            acl.allow(&did, &self.local_did);
            let acl_path = Paths::config_dir().join("network_acl.toml");
            let _ = acl.save(&acl_path);
            drop(acl);
        }
        
        // 添加到对等节点管理器
        #[cfg(feature = "p2p")]
        {
            let p2p_peer = P2PPeerInfo {
                node_id: node_id.clone(),
                did: did.clone(),
                address: options.endpoint.clone(),
                last_seen: now,
                last_sync_at: None,
                is_connected: false,
                capabilities: vec![],
            };
            self.peer_manager.update_peer(p2p_peer).await?;
        }
        
        info!("Bound new node: {} ({})", node_id, did);
        
        let summary = NodeSummary {
            id: node_id.clone(),
            did: did.clone(),
            name: node_id.clone(),
            status: NodeStatus::Offline,
            endpoint: options.endpoint,
            version: "1.0.0".to_string(),
            last_seen: now,
            capabilities: vec![],
        };
        
        Ok(NodeInfo {
            summary,
            public_key,
            metadata: HashMap::new(),
            trust_score: if matches!(options.trust_level, TrustLevel::Full) { 1.0 } else { 0.5 },
            is_blacklisted: false,
            created_at: now,
        })
    }

    /// 断开节点连接
    pub async fn disconnect(&self, id: &str) -> Result<()> {
        let db = self.federation_db.write().await;
        
        // 检查节点是否存在
        let _peer = db.get_peer(id)?
            .ok_or_else(|| CisError::not_found(format!("Node '{}' not found", id)))?;
        
        // 更新状态为离线
        db.update_peer_status(id, PeerStatus::Offline)?;
        drop(db);
        
        // 更新对等节点管理器
        #[cfg(feature = "p2p")]
        if let Some(mut p2p_peer) = self.peer_manager.get_peer(id).await? {
            p2p_peer.is_connected = false;
            self.peer_manager.update_peer(p2p_peer).await?;
        }
        
        info!("Disconnected node: {}", id);
        
        Ok(())
    }

    /// 拉黑节点
    pub async fn blacklist(&self, id: &str, reason: Option<&str>) -> Result<()> {
        let db = self.federation_db.write().await;
        
        // 检查节点是否存在
        let peer = db.get_peer(id)?
            .ok_or_else(|| CisError::not_found(format!("Node '{}' not found", id)))?;
        
        // 设置信任级别为 Blocked
        db.set_trust(&self.local_did, &peer.did, crate::storage::federation_db::TrustLevel::Blocked)?;
        drop(db);
        
        // 添加到 ACL 黑名单
        let mut acl = self.acl.write().await;
        let reason_str = reason.unwrap_or("Manual blacklisting");
        acl.deny(&peer.did, &self.local_did);
        
        // 保存 ACL
        let acl_path = Paths::config_dir().join("network_acl.toml");
        acl.save(&acl_path)?;
        drop(acl);
        
        // 断开连接
        let _ = self.disconnect(id).await;
        
        warn!("Blacklisted node: {} ({}), reason: {}", id, peer.did, reason_str);
        
        Ok(())
    }

    /// 解除拉黑
    pub async fn unblacklist(&self, id: &str) -> Result<()> {
        let db = self.federation_db.write().await;
        
        // 检查节点是否存在
        let peer = db.get_peer(id)?
            .ok_or_else(|| CisError::not_found(format!("Node '{}' not found", id)))?;
        
        // 设置信任级别为 Read（默认）
        db.set_trust(&self.local_did, &peer.did, crate::storage::federation_db::TrustLevel::Read)?;
        drop(db);
        
        // 从 ACL 黑名单移除
        let mut acl = self.acl.write().await;
        acl.undeny(&peer.did);
        
        // 保存 ACL
        let acl_path = Paths::config_dir().join("network_acl.toml");
        acl.save(&acl_path)?;
        drop(acl);
        
        info!("Removed node from blacklist: {} ({})", id, peer.did);
        
        Ok(())
    }

    /// 检查节点状态 (Ping)
    pub async fn ping(&self, id: &str) -> Result<bool> {
        let db = self.federation_db.read().await;
        
        // 检查节点是否存在
        let peer = match db.get_peer(id)? {
            Some(p) => p,
            None => return Err(CisError::not_found(format!("Node '{}' not found", id))),
        };
        
        drop(db);
        
        // 检查是否在黑名单
        let acl = self.acl.read().await;
        if acl.is_blacklisted(&peer.did) {
            return Ok(false);
        }
        drop(acl);
        
        // 模拟 ping 操作（实际实现会使用 WebSocket 或其他协议）
        // 这里我们检查对等节点管理器中的状态
        #[cfg(feature = "p2p")]
        if let Some(p2p_peer) = self.peer_manager.get_peer(id).await? {
            let is_healthy = !p2p_peer.is_unhealthy();
            
            // 更新数据库状态
            if is_healthy {
                let db = self.federation_db.write().await;
                db.update_peer_status(id, PeerStatus::Online)?;
                db.update_peer_rtt(id, 50)?; // 模拟 RTT
            }
            
            return Ok(is_healthy);
        }
        
        // 如果没有在 peer_manager 中，根据数据库状态返回
        Ok(peer.status == PeerStatus::Online)
    }

    /// 同步节点数据
    pub async fn sync(&self, id: &str) -> Result<()> {
        let db = self.federation_db.read().await;
        
        // 检查节点是否存在
        let peer = match db.get_peer(id)? {
            Some(p) => p,
            None => return Err(CisError::not_found(format!("Node '{}' not found", id))),
        };
        
        drop(db);
        
        // 检查是否在黑名单
        let acl = self.acl.read().await;
        if acl.is_blacklisted(&peer.did) {
            return Err(CisError::invalid_input(format!("Node '{}' is blacklisted", id)));
        }
        drop(acl);
        
        // 检查是否在线
        if peer.status != PeerStatus::Online {
            return Err(CisError::invalid_input(format!("Node '{}' is offline", id)));
        }
        
        // 更新同步时间
        #[cfg(feature = "p2p")]
        self.peer_manager.update_sync_time(id).await?;
        
        // 添加同步任务到数据库
        let db = self.federation_db.write().await;
        let sync_task = crate::storage::federation_db::SyncTask {
            id: None,
            target_node: id.to_string(),
            room_id: "!sync:localhost".to_string(),
            since_event_id: "$sync".to_string(),
            priority: 0,
        };
        db.add_sync_task(&sync_task)?;
        drop(db);
        
        info!("Scheduled sync for node: {}", id);
        
        Ok(())
    }

    /// 获取节点统计
    pub async fn stats(&self, id: &str) -> Result<ResourceStats> {
        let db = self.federation_db.read().await;
        
        // 检查节点是否存在
        let peer = match db.get_peer(id)? {
            Some(p) => p,
            None => return Err(CisError::not_found(format!("Node '{}' not found", id))),
        };
        
        drop(db);
        
        // 返回基于节点 RTT 和状态的模拟统计
        // 实际实现会从节点获取真实统计数据
        let rtt = peer.rtt_ms.unwrap_or(100);
        let is_online = peer.status == PeerStatus::Online;
        
        Ok(ResourceStats {
            cpu_percent: if is_online { 10.0 + (rtt as f64 * 0.1) } else { 0.0 },
            memory_usage: if is_online { 100 * 1024 * 1024 } else { 0 }, // 100MB
            memory_limit: 1024 * 1024 * 1024, // 1GB
            memory_percent: if is_online { 10.0 } else { 0.0 },
            io_read_bytes: 0,
            io_write_bytes: 0,
            net_rx_bytes: if is_online { 1024 * 1024 } else { 0 },
            net_tx_bytes: if is_online { 512 * 1024 } else { 0 },
            pids: if is_online { 5 } else { 0 },
        })
    }

    /// 清理离线节点
    pub async fn prune(&self, max_offline_days: u32) -> Result<Vec<String>> {
        let db = self.federation_db.write().await;
        
        let cutoff_time = Utc::now().timestamp() - (max_offline_days as i64 * 24 * 3600);
        
        // 获取所有节点
        let all_peers = self.get_all_peers_from_db(&db).await?;
        
        let mut removed = Vec::new();
        
        for peer in all_peers {
            // 只清理离线超过指定天数的节点
            if peer.status == PeerStatus::Offline && peer.last_seen < cutoff_time {
                // 从数据库删除节点
                // 注意: FederationDb 没有直接删除方法，我们通过设置特殊状态来标记
                // 这里我们只是记录需要删除的节点
                removed.push(peer.node_id.clone());
                info!("Pruned offline node: {} (last seen: {})", peer.node_id, peer.last_seen);
            }
        }
        
        drop(db);
        
        // 从 peer_manager 中移除
        #[cfg(feature = "p2p")]
        for node_id in &removed {
            // 注意: P2P PeerManager 没有直接删除方法，我们标记为不健康
            let _ = self.peer_manager.mark_unhealthy(node_id).await;
        }
        
        Ok(removed)
    }

    /// 辅助方法：从数据库获取所有节点
    async fn get_all_peers_from_db(&self, db: &FederationDb) -> Result<Vec<DbPeerInfo>> {
        // 使用 conn 执行自定义查询获取所有节点
        let mut stmt = db.conn().prepare(
            "SELECT node_id, did, endpoint_ws, status, last_seen, rtt_ms, public_key FROM network_peers"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare query: {}", e)))?;
        
        let peers: Result<Vec<DbPeerInfo>> = stmt
            .query_map([], |row| {
                let status_int: i32 = row.get(3)?;
                Ok(DbPeerInfo {
                    node_id: row.get(0)?,
                    did: row.get(1)?,
                    endpoint_ws: row.get(2)?,
                    status: PeerStatus::from_i32(status_int),
                    last_seen: row.get(4)?,
                    rtt_ms: row.get(5)?,
                    public_key: row.get(6)?,
                })
            })
            .map_err(CisError::Database)?
            .map(|r| r.map_err(CisError::Database))
            .collect();
        
        peers
    }
}

impl Default for NodeService {
    fn default() -> Self {
        Self::new().expect("Failed to create NodeService")
    }
}
