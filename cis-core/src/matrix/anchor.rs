//! Cloud Anchor 云端锚点
//!
//! 简化版：支持手动配置和可选的云端发现

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::CisError;

/// Cloud Anchor 云端锚点服务
pub struct CloudAnchor {
    /// 云端锚点 URL（可选，None 表示纯手动模式）
    endpoint: Option<String>,
    /// 本节点 DID
    did: String,
    /// 本节点 ID
    node_id: String,
    /// 手动配置的 peers（简化版主要用这个）
    manual_peers: Vec<PeerEndpoint>,
    /// HTTP 客户端
    client: Client,
}

impl CloudAnchor {
    /// 创建手动配置模式（无云端）
    pub fn manual(did: String, node_id: String) -> Self {
        Self {
            endpoint: None,
            did,
            node_id,
            manual_peers: Vec::new(),
            client: Client::new(),
        }
    }

    /// 创建云端模式
    pub fn with_cloud(endpoint: String, did: String, node_id: String) -> Self {
        Self {
            endpoint: Some(endpoint),
            did,
            node_id,
            manual_peers: Vec::new(),
            client: Client::new(),
        }
    }

    /// 添加手动 peer
    pub fn add_peer(&mut self, peer: PeerEndpoint) {
        self.manual_peers.push(peer);
    }

    /// 获取所有已知 peers（手动 + 云端）
    pub async fn discover_peers(&self) -> Result<Vec<PeerEndpoint>> {
        let mut peers = self.manual_peers.clone();

        // 如果有云端，查询云端
        if let Some(_endpoint) = &self.endpoint {
            let cloud_peers = self.query_cloud().await?;
            peers.extend(cloud_peers);
        }

        // 去重
        peers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        peers.dedup_by(|a, b| a.node_id == b.node_id);

        Ok(peers)
    }

    /// 查询云端锚点
    async fn query_cloud(&self) -> Result<Vec<PeerEndpoint>> {
        let url = format!("{}/v1/peers", self.endpoint.as_ref().unwrap());
        let response = self
            .client
            .get(&url)
            .header("X-CIS-DID", &self.did)
            .send()
            .await
            .map_err(|e| CisError::p2p(format!("Failed to query cloud: {}", e)))?;

        let peers = response
            .json::<Vec<PeerEndpoint>>()
            .await
            .map_err(|e| CisError::p2p(format!("Failed to parse cloud response: {}", e)))?;
        Ok(peers)
    }

    /// 心跳上报（云端模式）
    pub async fn heartbeat(&self, public_endpoint: &str) -> Result<Vec<PeerEndpoint>> {
        if let Some(endpoint) = &self.endpoint {
            let url = format!("{}/v1/heartbeat", endpoint);
            let request = HeartbeatRequest {
                did: self.did.clone(),
                node_id: self.node_id.clone(),
                endpoint: public_endpoint.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            };

            let response = self
                .client
                .post(&url)
                .json(&request)
                .send()
                .await
                .map_err(|e| CisError::p2p(format!("Failed to send heartbeat: {}", e)))?;

            let result = response
                .json::<HeartbeatResponse>()
                .await
                .map_err(|e| CisError::p2p(format!("Failed to parse heartbeat response: {}", e)))?;
            Ok(result.peers)
        } else {
            // 手动模式，返回手动配置的 peers
            Ok(self.manual_peers.clone())
        }
    }

    /// 查询特定节点
    pub async fn lookup_peer(&self, node_id: &str) -> Result<Option<PeerEndpoint>> {
        // 先查手动配置
        if let Some(peer) = self.manual_peers.iter().find(|p| p.node_id == node_id) {
            return Ok(Some(peer.clone()));
        }

        // 再查云端
        if let Some(endpoint) = &self.endpoint {
            let url = format!("{}/v1/peers/{}", endpoint, node_id);
            let response = self
                .client
                .get(&url)
                .send()
                .await
                .map_err(|e| CisError::p2p(format!("Failed to lookup peer: {}", e)))?;
            if response.status().is_success() {
                let peer = response
                    .json::<PeerEndpoint>()
                    .await
                    .map_err(|e| CisError::p2p(format!("Failed to parse peer: {}", e)))?;
                return Ok(Some(peer));
            }
        }

        Ok(None)
    }

    /// 获取本节点 DID
    pub fn did(&self) -> &str {
        &self.did
    }

    /// 获取本节点 ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

impl std::fmt::Debug for CloudAnchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudAnchor")
            .field("endpoint", &self.endpoint)
            .field("did", &self.did)
            .field("node_id", &self.node_id)
            .field("manual_peers_count", &self.manual_peers.len())
            .finish()
    }
}

/// Peer 端点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEndpoint {
    pub node_id: String,
    pub did: String,
    pub endpoint: String, // ws://host:port
    pub last_seen: i64,
    pub rtt_ms: Option<i32>,
}

impl PeerEndpoint {
    /// 创建新的 peer 端点
    pub fn new(node_id: impl Into<String>, did: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            did: did.into(),
            endpoint: endpoint.into(),
            last_seen: chrono::Utc::now().timestamp(),
            rtt_ms: None,
        }
    }

    /// 设置 RTT
    pub fn with_rtt(mut self, rtt_ms: i32) -> Self {
        self.rtt_ms = Some(rtt_ms);
        self
    }
}

#[derive(Debug, Serialize)]
struct HeartbeatRequest {
    did: String,
    node_id: String,
    endpoint: String,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
struct HeartbeatResponse {
    peers: Vec<PeerEndpoint>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloud_anchor_manual() {
        let anchor = CloudAnchor::manual(
            "did:cis:test".to_string(),
            "test-node".to_string(),
        );

        assert_eq!(anchor.did(), "did:cis:test");
        assert_eq!(anchor.node_id(), "test-node");
    }

    #[test]
    fn test_cloud_anchor_with_cloud() {
        let anchor = CloudAnchor::with_cloud(
            "https://cloud.cis.example".to_string(),
            "did:cis:test".to_string(),
            "test-node".to_string(),
        );

        assert_eq!(anchor.did(), "did:cis:test");
        assert_eq!(anchor.node_id(), "test-node");
    }

    #[test]
    fn test_peer_endpoint() {
        let peer = PeerEndpoint::new(
            "node1",
            "did:cis:node1",
            "ws://192.168.1.1:6768",
        )
        .with_rtt(50);

        assert_eq!(peer.node_id, "node1");
        assert_eq!(peer.did, "did:cis:node1");
        assert_eq!(peer.endpoint, "ws://192.168.1.1:6768");
        assert_eq!(peer.rtt_ms, Some(50));
    }

    #[tokio::test]
    async fn test_manual_peers() {
        let mut anchor = CloudAnchor::manual(
            "did:cis:test".to_string(),
            "test-node".to_string(),
        );

        let peer1 = PeerEndpoint::new("node1", "did:cis:node1", "ws://192.168.1.1:6768");
        let peer2 = PeerEndpoint::new("node2", "did:cis:node2", "ws://192.168.1.2:6768");

        anchor.add_peer(peer1);
        anchor.add_peer(peer2);

        let peers = anchor.discover_peers().await.unwrap();
        assert_eq!(peers.len(), 2);

        // 测试查找
        let found = anchor.lookup_peer("node1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_id, "node1");

        let not_found = anchor.lookup_peer("node3").await.unwrap();
        assert!(not_found.is_none());
    }
}
