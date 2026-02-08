//! Gossip 协议实现

use crate::error::Result;
use crate::p2p::peer::{PeerManager, PeerInfo};
use crate::p2p::transport::{QuicTransport, Connection};
use std::sync::Arc as StdArc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use serde::{Serialize, Deserialize};

/// Gossip 协议
pub struct GossipProtocol {
    peer_manager: Arc<PeerManager>,
    transport: Arc<QuicTransport>,
    #[allow(clippy::type_complexity)]
    topics: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<Vec<u8>>>>>>,
    message_cache: Arc<RwLock<HashMap<String, bool>>>, // 防重复
    active_connections: Arc<RwLock<HashMap<String, StdArc<Connection>>>>,
}

impl GossipProtocol {
    /// 创建新的 Gossip 协议实例
    pub fn new(peer_manager: Arc<PeerManager>, transport: Arc<QuicTransport>) -> Self {
        Self {
            peer_manager,
            transport,
            topics: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 启动 Gossip 协议
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Gossip protocol started");
        // 启动 gossip 传播循环
        Ok(())
    }
    
    /// 广播消息到主题
    pub async fn broadcast(&self, topic: &str, data: Vec<u8>) -> Result<()> {
        // 计算消息 ID
        let msg_id = format!("{}:{}", topic, sha256(&data));
        
        // 检查是否已处理
        if self.message_cache.read().await.contains_key(&msg_id) {
            return Ok(());
        }
        
        // 缓存消息
        self.message_cache.write().await.insert(msg_id.clone(), true);
        
        // 本地回调
        if let Some(subs) = self.topics.read().await.get(topic) {
            for tx in subs {
                let _ = tx.send(data.clone()).await;
            }
        }
        
        // 转发到对等节点
        let peers = self.peer_manager.get_connected_peers().await;
        for peer in peers {
            // 随机选择部分节点转发（gossip 传播）
            if rand::random::<f32>() < 0.7 { // 70% 转发概率
                if let Err(e) = self.forward_to_peer(&peer, topic, &data).await {
                    tracing::warn!("Failed to forward to {}: {}", peer.node_id, e);
                }
            }
        }
        
        Ok(())
    }
    
    /// 订阅主题
    pub async fn subscribe<F>(&self, topic: &str, callback: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let (tx, mut rx) = mpsc::channel(100);
        
        self.topics.write().await
            .entry(topic.to_string())
            .or_insert_with(Vec::new)
            .push(tx);
        
        // 启动回调处理
        tokio::spawn(async move {
            while let Some(data) = rx.recv().await {
                callback(data);
            }
        });
        
        Ok(())
    }
    
    /// 转发消息到对等节点
    async fn forward_to_peer(&self, peer: &PeerInfo, topic: &str, data: &[u8]) -> Result<()> {
        tracing::debug!("Forwarding message to peer {}", peer.node_id);

        // 获取或建立连接
        let conn = self.get_or_create_connection(&peer.node_id, &peer.address).await?;

        // 构建 Gossip 消息
        let message = GossipMessage {
            topic: topic.to_string(),
            data: data.to_vec(),
            ttl: 3, // 生存时间
            timestamp: chrono::Utc::now(),
        };

        // 序列化并发送
        let message_bytes = serde_json::to_vec(&message)
            .map_err(|e| crate::error::CisError::storage(format!("Serialization error: {}", e)))?;

        conn.send(message_bytes).await?;

        tracing::debug!("Message forwarded to {}", peer.node_id);
        Ok(())
    }

    /// 获取或创建连接
    async fn get_or_create_connection(&self, node_id: &str, address: &str) -> Result<StdArc<Connection>> {
        // 检查现有连接
        {
            let connections = self.active_connections.read().await;
            if let Some(conn) = connections.get(node_id) {
                if conn.is_alive() {
                    return Ok(Arc::clone(conn));
                }
            }
        }

        // 创建新连接
        let conn = Arc::new(self.transport.connect(address).await?);

        // 保存连接
        self.active_connections.write().await.insert(node_id.to_string(), Arc::clone(&conn));

        Ok(conn)
    }

    /// 处理收到的 Gossip 消息
    pub async fn handle_message(&self, data: Vec<u8>) -> Result<()> {
        let message: GossipMessage = serde_json::from_slice(&data)
            .map_err(|e| crate::error::CisError::storage(format!("Serialization error: {}", e)))?;

        // 检查 TTL
        if message.ttl == 0 {
            tracing::debug!("Message TTL expired, dropping");
            return Ok(());
        }

        // 计算消息 ID 防止重复
        let msg_id = format!("{}:{}", message.topic, sha256(&message.data));
        if self.message_cache.read().await.contains_key(&msg_id) {
            return Ok(());
        }

        // 缓存消息
        self.message_cache.write().await.insert(msg_id, true);

        // 本地回调
        if let Some(subs) = self.topics.read().await.get(&message.topic) {
            for tx in subs {
                let _ = tx.send(message.data.clone()).await;
            }
        }

        // 继续转发（TTL - 1）
        let peers = self.peer_manager.get_connected_peers().await;
        for peer in peers {
            if rand::random::<f32>() < 0.7 { // 70% 转发概率
                let forwarded = GossipMessage {
                    topic: message.topic.clone(),
                    data: message.data.clone(),
                    ttl: message.ttl - 1,
                    timestamp: message.timestamp,
                };

                let data = serde_json::to_vec(&forwarded)?;
                if let Err(e) = self.forward_to_peer(&peer, &message.topic, &data).await {
                    tracing::warn!("Failed to forward to {}: {}", peer.node_id, e);
                }
            }
        }

        Ok(())
    }
}

/// Gossip 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GossipMessage {
    topic: String,
    data: Vec<u8>,
    ttl: u8,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// 计算 SHA256 哈希
fn sha256(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
