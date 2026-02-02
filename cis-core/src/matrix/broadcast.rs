//! Room 事件联邦广播

use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, error, warn};

use crate::error::Result;
use crate::matrix::anchor::{CloudAnchor, PeerEndpoint};
use crate::matrix::federation::types::CisMatrixEvent;
use crate::matrix::nucleus::MatrixEvent;
use crate::matrix::websocket::tunnel::{TunnelError, TunnelManager};
use crate::storage::federation_db::{FederationDb, PeerInfo, SyncTask};

/// 事件广播器
pub struct EventBroadcaster {
    tunnel_manager: Arc<TunnelManager>,
    federation_db: Arc<Mutex<FederationDb>>,
    anchor: Arc<CloudAnchor>,
}

impl EventBroadcaster {
    /// 创建新的事件广播器
    pub fn new(
        tunnel_manager: Arc<TunnelManager>,
        federation_db: Arc<Mutex<FederationDb>>,
        anchor: Arc<CloudAnchor>,
    ) -> Self {
        Self {
            tunnel_manager,
            federation_db,
            anchor,
        }
    }

    /// 广播事件到联邦中的所有 peers
    pub async fn broadcast_event(
        &self,
        room_id: &str,
        event: &MatrixEvent,
    ) -> Result<BroadcastResult> {
        // 1. 检查 room 是否 federate
        if !self.should_federate(room_id).await? {
            return Ok(BroadcastResult::Skipped);
        }

        // 2. 获取在线 peers
        let peers = self.get_online_peers().await?;

        // 3. 并行发送
        let mut success = Vec::new();
        let mut failed = Vec::new();

        for peer in peers {
            match self.send_to_peer(room_id, &peer, event).await {
                Ok(_) => success.push(peer.node_id),
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!(
                        "Failed to broadcast event {} to peer {}: {}",
                        event.event_id, peer.node_id, error_msg
                    );
                    failed.push((peer.node_id.clone(), error_msg));
                    // 记录到 pending_sync 队列
                    if let Err(e) = self.queue_retry(room_id, event, &peer).await {
                        error!("Failed to queue retry: {}", e);
                    }
                }
            }
        }

        debug!(
            "Broadcasted event {} to room {}: {} success, {} failed",
            event.event_id,
            room_id,
            success.len(),
            failed.len()
        );

        Ok(BroadcastResult::Completed { success, failed })
    }

    /// 发送事件到单个 peer
    async fn send_to_peer(
        &self,
        room_id: &str,
        peer: &PeerEndpoint,
        event: &MatrixEvent,
    ) -> std::result::Result<(), TunnelError> {
        // 通过 WebSocket 隧道发送
        let cis_event = CisMatrixEvent::new(
            event.event_id.as_str(),
            room_id,
            event.sender.as_str(),
            &event.event_type,
            event.content.clone(),
        );

        self.tunnel_manager.send_to(&peer.node_id, &cis_event).await
    }

    /// 检查是否应该联邦
    async fn should_federate(&self, _room_id: &str) -> Result<bool> {
        // 通过 anchor 获取本节点 DID，然后查询数据库
        // 这里简化处理：查询 network_peers 表确认联邦是否启用
        // 实际应该查询 matrix_rooms 表的 federate 字段

        // 暂时默认启用联邦，具体实现依赖于上层调用时检查
        // 或者通过 federation_db 的某种方式查询
        Ok(true)
    }

    /// 获取在线 peers
    async fn get_online_peers(&self) -> Result<Vec<PeerEndpoint>> {
        // 从 federation_db 获取状态为 online 的 peers
        let db = self.federation_db.lock().await;
        let peers = db.list_online_peers()?;

        // 转换为 PeerEndpoint
        let endpoints: Vec<PeerEndpoint> = peers
            .into_iter()
            .filter_map(|info| self.peer_info_to_endpoint(info))
            .collect();

        Ok(endpoints)
    }

    /// 将 PeerInfo 转换为 PeerEndpoint
    fn peer_info_to_endpoint(&self, info: PeerInfo) -> Option<PeerEndpoint> {
        let endpoint_ws = info.endpoint_ws?;

        Some(PeerEndpoint {
            node_id: info.node_id,
            did: info.did,
            endpoint: endpoint_ws,
            last_seen: info.last_seen,
            rtt_ms: info.rtt_ms,
        })
    }

    /// 失败时加入重试队列
    async fn queue_retry(
        &self,
        room_id: &str,
        event: &MatrixEvent,
        peer: &PeerEndpoint,
    ) -> Result<()> {
        let task = SyncTask {
            id: None,
            target_node: peer.node_id.clone(),
            room_id: room_id.to_string(),
            since_event_id: event.event_id.to_string(),
            priority: 0,
        };

        let db = self.federation_db.lock().await;
        db.add_sync_task(&task)
    }

    /// 获取 tunnel manager
    pub fn tunnel_manager(&self) -> &TunnelManager {
        &self.tunnel_manager
    }

    /// 获取 federation db
    pub fn federation_db(&self) -> &Arc<Mutex<FederationDb>> {
        &self.federation_db
    }

    /// 获取 cloud anchor
    pub fn anchor(&self) -> &CloudAnchor {
        &self.anchor
    }
}

impl std::fmt::Debug for EventBroadcaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBroadcaster")
            .field("anchor", &self.anchor)
            .finish()
    }
}

/// 广播结果
#[derive(Debug, Clone)]
pub enum BroadcastResult {
    /// room 不联邦，跳过
    Skipped,
    /// 广播完成
    Completed {
        /// 成功的节点列表
        success: Vec<String>,
        /// 失败的节点列表（节点ID, 错误信息）
        failed: Vec<(String, String)>,
    },
}

impl BroadcastResult {
    /// 是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, BroadcastResult::Completed { failed, .. } if failed.is_empty())
    }

    /// 获取成功数量
    pub fn success_count(&self) -> usize {
        match self {
            BroadcastResult::Skipped => 0,
            BroadcastResult::Completed { success, .. } => success.len(),
        }
    }

    /// 获取失败数量
    pub fn failed_count(&self) -> usize {
        match self {
            BroadcastResult::Skipped => 0,
            BroadcastResult::Completed { failed, .. } => failed.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::nucleus::{EventId, RoomId, UserId};

    #[test]
    fn test_broadcast_result() {
        let skipped = BroadcastResult::Skipped;
        assert!(!skipped.is_success());
        assert_eq!(skipped.success_count(), 0);
        assert_eq!(skipped.failed_count(), 0);

        let completed = BroadcastResult::Completed {
            success: vec!["node1".to_string(), "node2".to_string()],
            failed: vec![],
        };
        assert!(completed.is_success());
        assert_eq!(completed.success_count(), 2);
        assert_eq!(completed.failed_count(), 0);

        let with_failures = BroadcastResult::Completed {
            success: vec!["node1".to_string()],
            failed: vec![("node2".to_string(), "timeout".to_string())],
        };
        assert!(!with_failures.is_success());
        assert_eq!(with_failures.success_count(), 1);
        assert_eq!(with_failures.failed_count(), 1);
    }
}
