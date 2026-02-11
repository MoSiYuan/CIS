//! # DHT 网络传输接口
//!
//! 抽象网络传输层，支持集成到现有的 P2PNetwork。

use super::message::{KademliaMessage, MessagePayload};
use super::kbucket::NodeInfo;
use crate::error::{CisError, Result};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

/// DHT 网络传输接口
#[async_trait]
pub trait DhtTransport: Send + Sync {
    /// 发送消息到指定节点
    async fn send_to(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
    ) -> Result<()>;

    /// 发送消息并等待响应
    async fn send_request(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
        timeout_duration: Duration,
    ) -> Result<KademliaMessage>;

    /// 获取本地节点信息
    fn local_node(&self) -> NodeInfo;
}

/// 基于 SecureP2PTransport 的传输实现
pub struct P2PNetworkTransport {
    /// SecureP2PTransport 引用
    transport: Arc<crate::p2p::transport_secure::SecureP2PTransport>,
    /// 本地节点信息
    local_node: NodeInfo,
    /// 响应等待器
    pending_responses: Arc<Mutex<std::collections::HashMap<u64, tokio::sync::oneshot::Sender<KademliaMessage>>>>,
}

impl P2PNetworkTransport {
    /// 从 SecureP2PTransport 创建传输层
    pub fn new(transport: Arc<crate::p2p::transport_secure::SecureP2PTransport>, local_node: NodeInfo) -> Self {
        Self {
            transport,
            local_node,
            pending_responses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// 序列化消息
    fn serialize_message(&self, message: &KademliaMessage) -> Result<Vec<u8>> {
        serde_json::to_vec(message)
            .map_err(|e| CisError::p2p(format!("Failed to serialize message: {}", e)))
    }

    /// 反序列化消息
    fn deserialize_message(&self, data: &[u8]) -> Result<KademliaMessage> {
        serde_json::from_slice(data)
            .map_err(|e| CisError::p2p(format!("Failed to deserialize message: {}", e)))
    }

    /// 处理接收到的响应
    pub async fn handle_response(&self, message: KademliaMessage) {
        if let Some(nonce) = message.nonce() {
            let mut pending = self.pending_responses.lock().await;
            if let Some(sender) = pending.remove(&nonce) {
                let _ = sender.send(message);
            }
        }
    }
    
    /// 检查是否已连接到指定节点
    async fn is_connected(&self, node_id: &str) -> bool {
        let connections = self.transport.list_connections().await;
        connections.iter().any(|c| c.node_id == node_id)
    }
}

#[async_trait]
impl DhtTransport for P2PNetworkTransport {
    async fn send_to(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
    ) -> Result<()> {
        let data = self.serialize_message(message)?;
        
        // 确保节点已连接
        if !self.is_connected(&node_info.id.to_string()).await {
            // 尝试连接
            if let Ok(addr) = node_info.address.parse::<SocketAddr>() {
                if let Err(e) = self.transport.connect(&node_info.id.to_string(), addr).await {
                    tracing::warn!("Failed to connect to {}: {}", node_info.id, e);
                }
            }
        }
        
        // 发送消息
        self.transport.send(&node_info.id.to_string(), &data).await
            .map_err(|e| CisError::p2p(format!("Failed to send message: {}", e)))
    }

    async fn send_request(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
        timeout_duration: Duration,
    ) -> Result<KademliaMessage> {
        let nonce = message.nonce().unwrap_or_else(|| rand::random());
        
        // 创建响应通道
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(nonce, tx);
        }
        
        // 发送请求
        if let Err(e) = self.send_to(node_info, message).await {
            let mut pending = self.pending_responses.lock().await;
            pending.remove(&nonce);
            return Err(e);
        }
        
        // 等待响应
        match timeout(timeout_duration, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(CisError::p2p("Response channel closed".to_string())),
            Err(_) => {
                let mut pending = self.pending_responses.lock().await;
                pending.remove(&nonce);
                Err(CisError::p2p("Request timeout".to_string()))
            }
        }
    }

    fn local_node(&self) -> NodeInfo {
        self.local_node.clone()
    }
}

/// 模拟传输层（用于测试）
pub struct MockTransport {
    local_node: NodeInfo,
    received_messages: Arc<Mutex<Vec<(NodeInfo, KademliaMessage)>>>,
    response_callback: Arc<Mutex<Option<Box<dyn Fn(&KademliaMessage) -> Option<KademliaMessage> + Send + Sync>>>>,
}

impl MockTransport {
    /// 创建新的模拟传输层
    pub fn new(local_node: NodeInfo) -> Self {
        Self {
            local_node,
            received_messages: Arc::new(Mutex::new(Vec::new())),
            response_callback: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置响应回调
    pub async fn set_response_callback<F>(&self, callback: F)
    where
        F: Fn(&KademliaMessage) -> Option<KademliaMessage> + Send + Sync + 'static,
    {
        let mut cb = self.response_callback.lock().await;
        *cb = Some(Box::new(callback));
    }

    /// 获取接收到的消息
    pub async fn get_received_messages(&self) -> Vec<(NodeInfo, KademliaMessage)> {
        self.received_messages.lock().await.clone()
    }
}

#[async_trait]
impl DhtTransport for MockTransport {
    async fn send_to(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
    ) -> Result<()> {
        tracing::debug!("[Mock] Sending message to {} at {}", node_info.id, node_info.address);
        
        let mut messages = self.received_messages.lock().await;
        messages.push((node_info.clone(), message.clone()));
        
        Ok(())
    }

    async fn send_request(
        &self,
        node_info: &NodeInfo,
        message: &KademliaMessage,
        _timeout_duration: Duration,
    ) -> Result<KademliaMessage> {
        tracing::debug!("[Mock] Sending request to {} at {}", node_info.id, node_info.address);
        
        let mut messages = self.received_messages.lock().await;
        messages.push((node_info.clone(), message.clone()));
        
        // 如果设置了回调，调用它
        let callback = self.response_callback.lock().await;
        if let Some(ref cb) = *callback {
            if let Some(response) = cb(message) {
                return Ok(response);
            }
        }
        
        // 默认返回 Pong
        let nonce = message.nonce().unwrap_or(0);
        Ok(KademliaMessage {
            sender_id: self.local_node.id.clone(),
            nonce,
            payload: MessagePayload::Pong { nodes: vec![] },
        })
    }

    fn local_node(&self) -> NodeInfo {
        self.local_node.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::node_id::NodeId;

    #[tokio::test]
    async fn test_mock_transport_send() {
        let local = NodeInfo::new(NodeId::random(), "127.0.0.1:8000");
        let remote = NodeInfo::new(NodeId::random(), "127.0.0.1:8001");
        let transport = MockTransport::new(local);
        
        let message = KademliaMessage::ping(NodeId::random());
        transport.send_to(&remote, &message).await.unwrap();
        
        let messages = transport.get_received_messages().await;
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_transport_request() {
        let local = NodeInfo::new(NodeId::random(), "127.0.0.1:8000");
        let remote = NodeInfo::new(NodeId::random(), "127.0.0.1:8001");
        let transport = MockTransport::new(local);
        
        // 设置响应回调
        transport.set_response_callback(|_msg| {
            Some(KademliaMessage {
                sender_id: NodeId::random(),
                nonce: 0,
                payload: MessagePayload::Pong { nodes: vec![] },
            })
        }).await;
        
        let message = KademliaMessage::ping(NodeId::random());
        let response = transport.send_request(&remote, &message, Duration::from_secs(1)).await.unwrap();
        
        assert_eq!(response.nonce(), Some(0));
    }
}
