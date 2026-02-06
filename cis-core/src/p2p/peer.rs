//! 对等节点管理

use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// 对等节点信息
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub did: String,
    pub address: String,
    pub last_seen: DateTime<Utc>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub is_connected: bool,
    pub capabilities: Vec<String>,
}

impl PeerInfo {
    /// 检查节点是否不健康
    pub fn is_unhealthy(&self) -> bool {
        !self.is_connected || Utc::now().signed_duration_since(self.last_seen).num_seconds() > 120
    }
}

/// 对等节点管理器
#[derive(Debug, Clone)]
pub struct PeerManager {
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
}

impl PeerManager {
    /// 创建新的对等节点管理器
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 添加/更新对等节点
    pub async fn update_peer(&self, info: PeerInfo) -> Result<()> {
        self.peers.write().await.insert(info.node_id.clone(), info);
        Ok(())
    }
    
    /// 获取对等节点
    pub async fn get_peer(&self, node_id: &str) -> Result<Option<PeerInfo>> {
        Ok(self.peers.read().await.get(node_id).cloned())
    }
    
    /// 获取所有对等节点
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }
    
    /// 获取已连接的对等节点
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await
            .values()
            .filter(|p| p.is_connected)
            .cloned()
            .collect()
    }
    
    /// 标记健康状态
    pub async fn mark_healthy(&self, node_id: &str) -> Result<()> {
        if let Some(peer) = self.peers.write().await.get_mut(node_id) {
            peer.is_connected = true;
            peer.last_seen = Utc::now();
        }
        Ok(())
    }
    
    /// 标记不健康
    pub async fn mark_unhealthy(&self, node_id: &str) -> Result<()> {
        if let Some(peer) = self.peers.write().await.get_mut(node_id) {
            peer.is_connected = false;
        }
        Ok(())
    }
    
    /// 更新同步时间
    pub async fn update_sync_time(&self, node_id: &str) -> Result<()> {
        if let Some(peer) = self.peers.write().await.get_mut(node_id) {
            peer.last_sync_at = Some(Utc::now());
        }
        Ok(())
    }
}

impl Default for PeerManager {
    fn default() -> Self {
        Self::new()
    }
}
