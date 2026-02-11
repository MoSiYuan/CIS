//! 连接管理器测试
//!
//! 测试 P2P 连接的生命周期管理和消息路由。

use super::{ConnectionManager, ConnectionState};
use crate::p2p::peer::Message;
use tokio::sync::mpsc;
use std::time::Duration;

/// 测试连接管理器创建
#[tokio::test]
async fn test_connection_manager_creation() {
    let manager = ConnectionManager::new();
    let connections = manager.list_connections().await;
    assert!(connections.is_empty());
}

/// 测试添加连接
#[tokio::test]
async fn test_add_connection() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    let handle = manager.add_connection("node-1".to_string(), tx).await;
    
    assert_eq!(handle.node_id, "node-1");
    
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].0, "node-1");
    assert_eq!(connections[0].1, ConnectionState::Connected);
}

/// 测试移除连接
#[tokio::test]
async fn test_remove_connection() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    let result = manager.remove_connection("node-1").await;
    
    assert!(result.is_ok());
    
    let connections = manager.list_connections().await;
    assert!(connections.is_empty());
}

/// 测试移除不存在的连接
#[tokio::test]
async fn test_remove_nonexistent_connection() {
    let manager = ConnectionManager::new();
    
    // 移除不存在的连接应该返回 Ok（幂等操作）
    let result = manager.remove_connection("nonexistent").await;
    assert!(result.is_ok());
}

/// 测试获取连接
#[tokio::test]
async fn test_get_connection() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    
    let handle = manager.get_connection("node-1").await;
    assert!(handle.is_some());
    assert_eq!(handle.unwrap().node_id, "node-1");
    
    let handle = manager.get_connection("nonexistent").await;
    assert!(handle.is_none());
}

/// 测试发送消息到指定节点
#[tokio::test]
async fn test_send_to_node() {
    let manager = ConnectionManager::new();
    let (tx, mut rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    
    let message = Message::Ping;
    let result = manager.send_to("node-1", message.clone()).await;
    
    assert!(result.is_ok());
    
    let received = rx.recv().await.unwrap();
    assert!(matches!(received, Message::Ping));
}

/// 测试发送到不存在的节点
#[tokio::test]
async fn test_send_to_nonexistent_node() {
    let manager = ConnectionManager::new();
    
    let message = Message::Ping;
    let result = manager.send_to("nonexistent", message).await;
    
    assert!(result.is_err());
}

/// 测试发送到断开的连接
#[tokio::test]
async fn test_send_to_disconnected_node() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    manager.remove_connection("node-1").await.unwrap();
    
    let message = Message::Ping;
    let result = manager.send_to("node-1", message).await;
    
    assert!(result.is_err());
}

/// 测试广播消息
#[tokio::test]
async fn test_broadcast() {
    let manager = ConnectionManager::new();
    let (tx1, mut rx1) = mpsc::channel(100);
    let (tx2, mut rx2) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx1).await;
    manager.add_connection("node-2".to_string(), tx2).await;
    
    let message = Message::Ping;
    let results = manager.broadcast(message.clone()).await;
    
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(_, r)| r.is_ok()));
    
    let received1 = rx1.recv().await.unwrap();
    let received2 = rx2.recv().await.unwrap();
    assert!(matches!(received1, Message::Ping));
    assert!(matches!(received2, Message::Ping));
}

/// 测试广播到空连接列表
#[tokio::test]
async fn test_broadcast_empty() {
    let manager = ConnectionManager::new();
    
    let message = Message::Ping;
    let results = manager.broadcast(message).await;
    
    assert!(results.is_empty());
}

/// 测试多个连接管理
#[tokio::test]
async fn test_multiple_connections() {
    let manager = ConnectionManager::new();
    let node_count = 10;
    
    // 添加多个连接
    for i in 0..node_count {
        let (tx, _rx) = mpsc::channel(100);
        manager.add_connection(format!("node-{}", i), tx).await;
    }
    
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), node_count);
}

/// 测试重复添加相同节点
#[tokio::test]
async fn test_duplicate_connection() {
    let manager = ConnectionManager::new();
    let (tx1, _rx1) = mpsc::channel(100);
    let (tx2, _rx2) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx1).await;
    manager.add_connection("node-1".to_string(), tx2).await;
    
    let connections = manager.list_connections().await;
    // 后添加的连接应该覆盖之前的
    assert_eq!(connections.len(), 1);
}

/// 测试连接状态转换
#[tokio::test]
async fn test_connection_state_transitions() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    let handle = manager.add_connection("node-1".to_string(), tx).await;
    
    // 初始状态应该是 Connected
    let state = handle.state.read().await.clone();
    assert_eq!(state, ConnectionState::Connected);
    
    // 移除连接时状态应该变为 Disconnecting 然后 Disconnected
    manager.remove_connection("node-1").await.unwrap();
}

/// 测试消息类型
#[tokio::test]
async fn test_different_message_types() {
    let manager = ConnectionManager::new();
    let (tx, mut rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    
    // 测试 Ping
    manager.send_to("node-1", Message::Ping).await.unwrap();
    assert!(matches!(rx.recv().await.unwrap(), Message::Ping));
    
    // 测试 Pong
    manager.send_to("node-1", Message::Pong).await.unwrap();
    assert!(matches!(rx.recv().await.unwrap(), Message::Pong));
}

/// 测试连接活跃时间更新
#[tokio::test]
async fn test_connection_activity_update() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    let handle = manager.add_connection("node-1".to_string(), tx).await;
    let initial_active = *handle.last_active.read().await;
    
    // 等待一小段时间
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // 发送消息
    manager.send_to("node-1", Message::Ping).await.unwrap();
    
    let updated_active = *handle.last_active.read().await;
    assert!(updated_active > initial_active);
}

/// 测试并发连接操作
#[tokio::test]
async fn test_concurrent_operations() {
    use tokio::task::JoinSet;
    
    let manager = std::sync::Arc::new(ConnectionManager::new());
    let mut set = JoinSet::new();
    
    // 并发添加连接
    for i in 0..50 {
        let manager = manager.clone();
        set.spawn(async move {
            let (tx, _rx) = mpsc::channel(100);
            manager.add_connection(format!("node-{}", i), tx).await;
        });
    }
    
    while set.join_next().await.is_some() {}
    
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), 50);
}

/// 测试关闭通道后发送失败
#[tokio::test]
async fn test_send_after_receiver_dropped() {
    let manager = ConnectionManager::new();
    let (tx, rx) = mpsc::channel(100);
    
    manager.add_connection("node-1".to_string(), tx).await;
    
    // 丢弃接收端
    drop(rx);
    
    // 发送应该失败
    let result = manager.send_to("node-1", Message::Ping).await;
    assert!(result.is_err());
}
