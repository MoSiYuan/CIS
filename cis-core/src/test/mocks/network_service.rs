//! # Mock Network Service
//!
//! 网络服务的 Mock 实现，用于测试网络相关功能。

use super::MockCallTracker;
use crate::error::{CisError, Result};
use crate::traits::{
    NetworkService, NetworkStatus, PeerInfo, SendOptions, MessagePriority,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock as AsyncRwLock;

/// 网络连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Mock 网络连接
#[derive(Debug, Clone)]
pub struct MockConnection {
    pub url: String,
    pub state: MockConnectionState,
    pub messages_sent: Vec<String>,
    pub messages_received: Vec<String>,
}

impl MockConnection {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            state: MockConnectionState::Disconnected,
            messages_sent: Vec::new(),
            messages_received: Vec::new(),
        }
    }
}

/// 网络服务 Mock
#[derive(Debug, Clone)]
pub struct MockNetworkService {
    tracker: MockCallTracker,
    connections: Arc<AsyncRwLock<HashMap<String, MockConnection>>>,
    connect_behaviors: Arc<AsyncRwLock<HashMap<String, std::result::Result<(), CisError>>>>,
    send_behaviors: Arc<AsyncRwLock<HashMap<String, std::result::Result<(), CisError>>>>,
    receive_queue: Arc<AsyncRwLock<Vec<Vec<String>>>>,
    should_fail_next: Arc<Mutex<Option<CisError>>>,
    node_id: Arc<Mutex<String>>,
    did: Arc<Mutex<String>>,
}

impl MockNetworkService {
    /// 创建新的 Mock
    pub fn new() -> Self {
        Self {
            tracker: MockCallTracker::new(),
            connections: Arc::new(AsyncRwLock::new(HashMap::new())),
            connect_behaviors: Arc::new(AsyncRwLock::new(HashMap::new())),
            send_behaviors: Arc::new(AsyncRwLock::new(HashMap::new())),
            receive_queue: Arc::new(AsyncRwLock::new(Vec::new())),
            should_fail_next: Arc::new(Mutex::new(None)),
            node_id: Arc::new(Mutex::new("mock-node".to_string())),
            did: Arc::new(Mutex::new("did:cis:mock".to_string())),
        }
    }

    /// 创建带指定 node_id 的 Mock
    pub fn with_node_id(node_id: impl Into<String>) -> Self {
        let service = Self::new();
        *service.node_id.lock().unwrap() = node_id.into();
        service
    }

    /// 预设连接行为
    pub async fn preset_connect(&self, url: impl Into<String>, result: std::result::Result<(), CisError>) {
        let mut behaviors = self.connect_behaviors.write().await;
        behaviors.insert(url.into(), result);
    }

    /// 预设发送行为
    pub async fn preset_send(&self, url: impl Into<String>, result: std::result::Result<(), CisError>) {
        let mut behaviors = self.send_behaviors.write().await;
        behaviors.insert(url.into(), result);
    }

    /// 预设接收消息
    pub async fn preset_receive(&self, messages: Vec<String>) {
        let mut queue = self.receive_queue.write().await;
        queue.push(messages);
    }

    /// 模拟连接（内部方法）
    pub async fn mock_connect(&self, url: impl Into<String>) -> Result<()> {
        let url = url.into();
        self.tracker.record("connect", vec![url.clone()]);

        // 检查是否有预设的错误
        if let Some(err) = self.should_fail_next.lock().unwrap().take() {
            return Err(err);
        }

        // 检查行为预设
        let behaviors = self.connect_behaviors.read().await;
        if let Some(result) = behaviors.get(&url) {
            let is_ok = result.is_ok();
            let result = match result {
                Ok(_) => Ok(()),
                Err(e) => Err(CisError::p2p(e.to_string())),
            };
            if is_ok {
                let mut connections = self.connections.write().await;
                let mut conn = MockConnection::new(&url);
                conn.state = MockConnectionState::Connected;
                connections.insert(url, conn);
            }
            return result;
        }

        // 默认成功
        let mut connections = self.connections.write().await;
        let mut conn = MockConnection::new(&url);
        conn.state = MockConnectionState::Connected;
        connections.insert(url, conn);
        Ok(())
    }

    /// 模拟断开连接（内部方法）
    pub async fn mock_disconnect(&self, url: impl Into<String>) -> Result<()> {
        let url = url.into();
        self.tracker.record("disconnect", vec![url.clone()]);

        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&url) {
            conn.state = MockConnectionState::Disconnected;
        }
        Ok(())
    }

    /// 发送消息（内部方法）
    pub async fn mock_send(&self, url: impl Into<String>, message: impl Into<String>) -> Result<()> {
        let url = url.into();
        let message = message.into();
        self.tracker.record("send", vec![url.clone(), message.clone()]);

        let behaviors = self.send_behaviors.read().await;
        if let Some(result) = behaviors.get(&url) {
            let is_ok = result.is_ok();
            let result = match result {
                Ok(_) => Ok(()),
                Err(e) => Err(CisError::p2p(e.to_string())),
            };
            if is_ok {
                let mut connections = self.connections.write().await;
                if let Some(conn) = connections.get_mut(&url) {
                    conn.messages_sent.push(message);
                }
            }
            return result;
        }

        // 默认成功
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.get_mut(&url) {
            conn.messages_sent.push(message);
        }
        Ok(())
    }

    /// 接收消息
    pub async fn receive(&self) -> Result<Vec<String>> {
        self.tracker.record("receive", vec![]);

        let mut queue = self.receive_queue.write().await;
        if !queue.is_empty() {
            return Ok(queue.remove(0));
        }

        Ok(Vec::new())
    }

    /// 检查连接状态
    pub async fn is_mock_connected(&self, url: impl Into<String>) -> bool {
        let url = url.into();
        let connections = self.connections.read().await;
        connections
            .get(&url)
            .map(|c| c.state == MockConnectionState::Connected)
            .unwrap_or(false)
    }

    /// 获取连接信息
    pub async fn get_connection(&self, url: impl Into<String>) -> Option<MockConnection> {
        let url = url.into();
        let connections = self.connections.read().await;
        connections.get(&url).cloned()
    }

    /// 获取所有连接
    pub async fn get_all_connections(&self) -> Vec<MockConnection> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }

    /// 设置下一次操作失败
    pub fn will_fail_next(&self, error: CisError) {
        *self.should_fail_next.lock().unwrap() = Some(error);
    }

    // === 验证方法 ===

    /// 断言：方法被调用
    pub fn assert_called(&self, method: &str) {
        self.tracker.assert_called(method);
    }

    /// 断言：方法被调用指定次数
    pub fn assert_call_count(&self, method: &str, expected: usize) {
        self.tracker.assert_call_count(method, expected);
    }

    /// 断言：方法从未被调用
    pub fn assert_not_called(&self, method: &str) {
        self.tracker.assert_not_called(method);
    }

    /// 断言：发送了指定消息
    pub async fn assert_sent_message(&self, url: impl AsRef<str>, message: impl AsRef<str>) {
        let url = url.as_ref();
        let connections = self.connections.read().await;
        let conn = connections
            .get(url)
            .expect(&format!("Connection to '{}' not found", url));
        
        let expected = message.as_ref();
        assert!(
            conn.messages_sent.iter().any(|m| m.contains(expected)),
            "Expected message containing '{}' to be sent to '{}', but sent messages were: {:?}",
            expected, url, conn.messages_sent
        );
    }

    /// 断言：建立了连接
    pub async fn assert_connected(&self, url: impl AsRef<str>) {
        let url = url.as_ref();
        assert!(
            self.is_mock_connected(url).await,
            "Expected connection to '{}' to be established",
            url
        );
    }

    /// 断言：连接已断开
    pub async fn assert_disconnected(&self, url: impl AsRef<str>) {
        let url = url.as_ref();
        assert!(
            !self.is_mock_connected(url).await,
            "Expected connection to '{}' to be disconnected",
            url
        );
    }

    /// 清空调用记录
    pub fn clear(&self) {
        self.tracker.clear();
    }

    /// 获取调用追踪器
    pub fn tracker(&self) -> &MockCallTracker {
        &self.tracker
    }
}

impl Default for MockNetworkService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NetworkService for MockNetworkService {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        self.mock_send(node_id, String::from_utf8_lossy(data)).await
    }

    async fn send_to_with_options(
        &self,
        node_id: &str,
        data: &[u8],
        options: SendOptions,
    ) -> Result<()> {
        self.tracker.record("send_to_with_options", vec![
            node_id.to_string(),
            format!("priority={:?}", options.priority),
            format!("ack={}", options.require_ack),
        ]);
        self.mock_send(node_id, String::from_utf8_lossy(data)).await
    }

    async fn broadcast(&self, data: &[u8]) -> Result<usize> {
        self.tracker.record("broadcast", vec![format!("data_len={}", data.len())]);
        let connections = self.get_all_connections().await;
        let count = connections.len().max(1); // 至少返回 1
        Ok(count)
    }

    async fn broadcast_with_options(&self, data: &[u8], _options: SendOptions) -> Result<usize> {
        self.tracker.record("broadcast_with_options", vec![format!("data_len={}", data.len())]);
        let connections = self.get_all_connections().await;
        let count = connections.len().max(1);
        Ok(count)
    }

    async fn connect(&self, addr: &str) -> Result<()> {
        self.mock_connect(addr).await
    }

    async fn disconnect(&self, node_id: &str) -> Result<()> {
        self.mock_disconnect(node_id).await
    }

    async fn connected_peers(&self) -> Result<Vec<PeerInfo>> {
        let conns = self.get_all_connections().await;
        Ok(conns
            .into_iter()
            .filter(|c| matches!(c.state, MockConnectionState::Connected))
            .map(|c| PeerInfo {
                node_id: c.url.clone(),
                did: format!("did:cis:{}", c.url.replace(':', "-")),
                address: c.url.clone(),
                connected: true,
                last_seen: std::time::SystemTime::now(),
                last_sync_at: None,
                latency_ms: Some(10),
                protocol_version: "1.0".to_string(),
                capabilities: vec!["storage".to_string()],
            })
            .collect())
    }

    async fn discovered_peers(&self) -> Result<Vec<PeerInfo>> {
        // Mock doesn't support discovery
        Ok(Vec::new())
    }

    async fn get_peer(&self, node_id: &str) -> Result<Option<PeerInfo>> {
        let conn = self.get_connection(node_id).await;
        Ok(conn.map(|c| PeerInfo {
            node_id: c.url.clone(),
            did: format!("did:cis:{}", c.url.replace(':', "-")),
            address: c.url.clone(),
            connected: matches!(c.state, MockConnectionState::Connected),
            last_seen: std::time::SystemTime::now(),
            last_sync_at: None,
            latency_ms: Some(10),
            protocol_version: "1.0".to_string(),
            capabilities: vec![],
        }))
    }

    async fn status(&self) -> Result<NetworkStatus> {
        // 先获取所有需要的数据，避免 MutexGuard 跨越 await
        let node_id = self.node_id.lock().unwrap().clone();
        let connections = self.get_all_connections().await;
        
        Ok(NetworkStatus {
            running: true,
            node_id,
            listen_addr: "0.0.0.0:0".to_string(),
            uptime_secs: 3600,
            connected_peers: connections.len(),
            discovered_peers: 0,
            bytes_sent: 1024,
            bytes_received: 2048,
            error_count: 0,
        })
    }

    async fn start(&self) -> Result<()> {
        self.tracker.record("start", vec![]);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.tracker.record("stop", vec![]);
        Ok(())
    }

    fn node_id(&self) -> Result<String> {
        Ok(self.node_id.lock().unwrap().clone())
    }

    fn did(&self) -> Result<String> {
        Ok(self.did.lock().unwrap().clone())
    }

    async fn is_connected(&self, node_id: &str) -> Result<bool> {
        Ok(self.is_mock_connected(node_id).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_connect() {
        let mock = MockNetworkService::new();
        
        // 预设成功连接
        mock.preset_connect("ws://localhost:8080", Ok(())).await;
        
        // 测试连接
        mock.connect("ws://localhost:8080").await.unwrap();
        
        // 验证
        mock.assert_connected("ws://localhost:8080").await;
        mock.assert_call_count("connect", 1);
    }

    #[tokio::test]
    async fn test_mock_send_receive() {
        let mock = MockNetworkService::new();
        
        // 设置连接
        mock.preset_connect("ws://localhost:8080", Ok(())).await;
        mock.connect("ws://localhost:8080").await.unwrap();
        
        // 发送消息
        mock.send_to("ws://localhost:8080", b"{\"type\": \"hello\"}")
            .await
            .unwrap();
        
        mock.assert_sent_message("ws://localhost:8080", "hello").await;
        
        // 接收消息
        mock.preset_receive(vec![
            r#"{"type": "response"}"#.to_string(),
        ]).await;
        
        let msgs = mock.receive().await.unwrap();
        assert_eq!(msgs.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_disconnect() {
        let mock = MockNetworkService::new();
        
        mock.preset_connect("ws://localhost:8080", Ok(())).await;
        mock.connect("ws://localhost:8080").await.unwrap();
        mock.assert_connected("ws://localhost:8080").await;
        
        mock.disconnect("ws://localhost:8080").await.unwrap();
        mock.assert_disconnected("ws://localhost:8080").await;
    }

    #[tokio::test]
    async fn test_mock_error_simulation() {
        let mock = MockNetworkService::new();
        
        mock.will_fail_next(CisError::p2p("Connection refused"));
        let result = mock.connect("ws://localhost:8080").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_network_service_trait() {
        use crate::traits::NetworkService;

        let mock = MockNetworkService::with_node_id("test-node");
        
        // 测试 node_id 和 did 返回 Result
        assert_eq!(mock.node_id().unwrap(), "test-node");
        assert_eq!(mock.did().unwrap(), "did:cis:mock");
        
        // 测试 connected_peers 返回 Result
        let peers = mock.connected_peers().await.unwrap();
        assert!(peers.is_empty());
        
        // 测试 status 返回 Result
        let status = mock.status().await.unwrap();
        assert_eq!(status.node_id, "test-node");
    }
}
