//! P2P 连接处理循环

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use crate::error::{CisError, Result};
use crate::p2p::{PeerInfo, Message};

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
}

/// 连接句柄
#[derive(Clone)]
pub struct ConnectionHandle {
    pub node_id: String,
    pub state: Arc<RwLock<ConnectionState>>,
    pub tx: mpsc::Sender<Message>,
    pub last_active: Arc<RwLock<std::time::Instant>>,
}

/// 连接管理器
pub struct ConnectionManager {
    connections: Arc<RwLock<std::collections::HashMap<String, ConnectionHandle>>>,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(90),
        }
    }
    
    /// 添加连接
    pub async fn add_connection(&self, node_id: String, tx: mpsc::Sender<Message>) -> ConnectionHandle {
        let handle = ConnectionHandle {
            node_id: node_id.clone(),
            state: Arc::new(RwLock::new(ConnectionState::Connected)),
            tx,
            last_active: Arc::new(RwLock::new(std::time::Instant::now())),
        };
        
        self.connections.write().await.insert(node_id, handle.clone());
        handle
    }
    
    /// 移除连接
    pub async fn remove_connection(&self, node_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        
        if let Some(handle) = connections.remove(node_id) {
            let mut state = handle.state.write().await;
            *state = ConnectionState::Disconnecting;
            
            // 关闭发送通道
            drop(handle.tx);
            
            *state = ConnectionState::Disconnected;
            tracing::info!("Connection to {} removed", node_id);
        }
        
        Ok(())
    }
    
    /// 获取连接
    pub async fn get_connection(&self, node_id: &str) -> Option<ConnectionHandle> {
        self.connections.read().await.get(node_id).cloned()
    }
    
    /// 发送消息到指定节点
    pub async fn send_to(&self, node_id: &str, message: Message) -> Result<()> {
        let connections = self.connections.read().await;
        
        let handle = connections.get(node_id)
            .ok_or_else(|| CisError::p2p(format!("Connection to {} not found", node_id)))?;
        
        // 检查连接状态
        let state = handle.state.read().await;
        if *state != ConnectionState::Connected {
            return Err(CisError::p2p(format!(
                "Connection to {} is not active (state: {:?})",
                node_id, *state
            )));
        }
        
        // 发送消息
        handle.tx.send(message).await
            .map_err(|_| CisError::p2p(format!("Failed to send to {}", node_id)))?;
        
        // 更新活跃时间
        *handle.last_active.write().await = std::time::Instant::now();
        
        Ok(())
    }
    
    /// 广播消息到所有连接
    pub async fn broadcast(&self, message: Message) -> Vec<(String, Result<()>)> {
        let connections = self.connections.read().await;
        let mut results = Vec::new();
        
        for (node_id, handle) in connections.iter() {
            let result = handle.tx.send(message.clone()).await
                .map_err(|_| CisError::p2p(format!("Failed to broadcast to {}", node_id)));
            
            results.push((node_id.clone(), result));
        }
        
        results
    }
    
    /// 启动连接处理循环
    pub async fn start(&self, mut rx: mpsc::Receiver<(String, Message)>) {
        let connections = self.connections.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let heartbeat_timeout = self.heartbeat_timeout;
        
        // 消息处理任务
        let message_task = tokio::spawn(async move {
            while let Some((node_id, message)) = rx.recv().await {
                let conns = connections.read().await;
                if let Some(handle) = conns.get(&node_id) {
                    if let Err(e) = handle.tx.send(message).await {
                        tracing::error!("Failed to send to {}: {}", node_id, e);
                    }
                }
            }
        });
        
        // 心跳检测任务
        let connections = self.connections.clone();
        let heartbeat_task = tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);
            
            loop {
                interval.tick().await;
                
                let conns = connections.read().await;
                let now = std::time::Instant::now();
                
                for (node_id, handle) in conns.iter() {
                    let last_active = *handle.last_active.read().await;
                    
                    if now.duration_since(last_active) > heartbeat_timeout {
                        tracing::warn!("Connection to {} timed out", node_id);
                        // 标记为断开
                        let mut state = handle.state.write().await;
                        *state = ConnectionState::Disconnected;
                    }
                }
            }
        });
        
        // 等待任务完成
        tokio::select! {
            _ = message_task => {},
            _ = heartbeat_task => {},
        }
    }
    
    /// 获取所有连接
    pub async fn list_connections(&self) -> Vec<(String, ConnectionState)> {
        let connections = self.connections.read().await;
        let mut result = Vec::new();
        
        for (node_id, handle) in connections.iter() {
            let state = handle.state.read().await.clone();
            result.push((node_id.clone(), state));
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_management() {
        let manager = ConnectionManager::new();
        let (tx, mut rx) = mpsc::channel(100);
        
        // 添加连接
        let handle = manager.add_connection("node-1".to_string(), tx).await;
        
        // 发送消息
        let msg = Message::Ping;
        manager.send_to("node-1", msg.clone()).await.unwrap();
        
        // 接收消息
        let received = rx.recv().await.unwrap();
        assert!(matches!(received, Message::Ping));
        
        // 获取连接列表
        let connections = manager.list_connections().await;
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].0, "node-1");
    }
}
