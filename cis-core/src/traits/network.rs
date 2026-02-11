//! # NetworkService Trait
//!
//! 网络服务的抽象接口，定义 P2P 通信的基本操作。
//!
//! ## 设计原则
//!
//! - **异步非阻塞**: 所有网络操作都是异步的
//! - **错误透明**: 每个方法返回 Result，网络错误可追踪
//! - **超时控制**: 内置超时机制防止无限等待
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::traits::{NetworkService, SendOptions};
//! use std::sync::Arc;
//!
//! # async fn example(network: Arc<dyn NetworkService>) -> anyhow::Result<()> {
//! // 发送消息到指定节点
//! network.send_to("peer-123", b"Hello").await?;
//!
//! // 广播消息
//! let sent_count = network.broadcast(b"Broadcast message").await?;
//! println!("Broadcasted to {} peers", sent_count);
//!
//! // 获取连接节点
//! let peers = network.connected_peers().await?;
//! for peer in peers {
//!     println!("Connected to: {}", peer.node_id);
//! }
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use crate::error::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 消息优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    /// 关键消息（如心跳、控制命令）
    Critical = 0,
    /// 高优先级（如用户交互）
    High = 1,
    /// 普通优先级（默认）
    Normal = 2,
    /// 低优先级（如后台同步）
    Low = 3,
    /// 后台任务（如日志传输）
    Background = 4,
}

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

/// 发送选项
#[derive(Debug, Clone)]
pub struct SendOptions {
    /// 消息优先级
    pub priority: MessagePriority,
    /// 超时时间
    pub timeout: Duration,
    /// 是否需要确认
    pub require_ack: bool,
    /// 重试次数
    pub retry_count: u32,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            priority: MessagePriority::Normal,
            timeout: Duration::from_secs(30),
            require_ack: false,
            retry_count: 3,
            metadata: HashMap::new(),
        }
    }
}

impl SendOptions {
    /// 创建默认发送选项
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置需要确认
    pub fn with_ack(mut self, require_ack: bool) -> Self {
        self.require_ack = require_ack;
        self
    }

    /// 设置重试次数
    pub fn with_retry(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// 对等节点信息
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// 节点 ID
    pub node_id: String,
    /// DID
    pub did: String,
    /// 地址
    pub address: String,
    /// 是否已连接
    pub connected: bool,
    /// 最后可见时间
    pub last_seen: std::time::SystemTime,
    /// 最后同步时间
    pub last_sync_at: Option<DateTime<Utc>>,
    /// 延迟（毫秒）
    pub latency_ms: Option<u64>,
    /// 协议版本
    pub protocol_version: String,
    /// 能力列表
    pub capabilities: Vec<String>,
}

/// 网络状态
#[derive(Debug, Clone)]
pub struct NetworkStatus {
    /// 是否运行中
    pub running: bool,
    /// 节点 ID
    pub node_id: String,
    /// 监听地址
    pub listen_addr: String,
    /// 运行时长（秒）
    pub uptime_secs: u64,
    /// 已连接节点数
    pub connected_peers: usize,
    /// 已发现节点数
    pub discovered_peers: usize,
    /// 发送字节数
    pub bytes_sent: u64,
    /// 接收字节数
    pub bytes_received: u64,
    /// 错误计数
    pub error_count: u64,
}

/// 网络服务抽象接口
///
/// 定义 P2P 网络通信的基本操作，包括点对点消息发送、广播和节点管理。
///
/// ## 实现要求
///
/// - 所有方法必须是线程安全的 (Send + Sync)
/// - 所有异步方法必须返回 Result 类型
/// - 实现应该处理连接断开和重连逻辑
///
/// ## 使用示例
///
/// ```rust,no_run
/// use cis_core::traits::{NetworkService, SendOptions, MessagePriority};
/// use std::sync::Arc;
///
/// # async fn example(network: Arc<dyn NetworkService>) -> anyhow::Result<()> {
/// // 发送高优先级消息
/// network.send_to_with_options(
///     "peer-123",
///     b"Urgent message",
///     SendOptions::new()
///         .with_priority(MessagePriority::High)
///         .with_ack(true)
/// ).await?;
///
/// // 广播到所有节点
/// let count = network.broadcast(b"Hello everyone").await?;
/// println!("Message sent to {} peers", count);
///
/// // 获取网络状态
/// let status = network.status().await?;
/// println!("Connected peers: {}", status.connected_peers);
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait NetworkService: Send + Sync {
    /// 发送消息到指定节点
    ///
    /// # Arguments
    /// * `node_id` - 目标节点 ID
    /// * `data` - 消息数据
    ///
    /// # Returns
    /// * `Ok(())` - 发送成功
    /// * `Err(CisError::P2P(_))` - 网络错误或节点不可达
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::NetworkService;
    ///
    /// # async fn example(network: &dyn NetworkService) -> anyhow::Result<()> {
    /// network.send_to("peer-123", b"Hello").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;

    /// 使用选项发送消息到指定节点
    ///
    /// # Arguments
    /// * `node_id` - 目标节点 ID
    /// * `data` - 消息数据
    /// * `options` - 发送选项
    ///
    /// # Returns
    /// * `Ok(())` - 发送成功
    /// * `Err(CisError::P2P(_))` - 网络错误或超时
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::{NetworkService, SendOptions, MessagePriority};
    /// use std::time::Duration;
    ///
    /// # async fn example(network: &dyn NetworkService) -> anyhow::Result<()> {
    /// network.send_to_with_options(
    ///     "peer-123",
    ///     b"Important data",
    ///     SendOptions::new()
    ///         .with_priority(MessagePriority::High)
    ///         .with_timeout(Duration::from_secs(10))
    ///         .with_ack(true)
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn send_to_with_options(
        &self,
        node_id: &str,
        data: &[u8],
        options: SendOptions,
    ) -> Result<()>;

    /// 广播消息到所有连接节点
    ///
    /// # Arguments
    /// * `data` - 消息数据
    ///
    /// # Returns
    /// * `Ok(usize)` - 成功发送的节点数
    /// * `Err(CisError::P2P(_))` - 广播过程中发生错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::NetworkService;
    ///
    /// # async fn example(network: &dyn NetworkService) -> anyhow::Result<()> {
    /// let count = network.broadcast(b"Broadcast message").await?;
    /// println!("Broadcasted to {} peers", count);
    /// # Ok(())
    /// # }
    /// ```
    async fn broadcast(&self, data: &[u8]) -> Result<usize>;

    /// 使用选项广播消息
    ///
    /// # Arguments
    /// * `data` - 消息数据
    /// * `options` - 发送选项
    ///
    /// # Returns
    /// * `Ok(usize)` - 成功发送的节点数
    async fn broadcast_with_options(&self, data: &[u8], options: SendOptions) -> Result<usize>;

    /// 连接到指定节点
    ///
    /// # Arguments
    /// * `addr` - 节点地址
    ///
    /// # Returns
    /// * `Ok(())` - 连接成功
    /// * `Err(CisError::P2P(_))` - 连接失败
    async fn connect(&self, addr: &str) -> Result<()>;

    /// 断开与节点的连接
    ///
    /// # Arguments
    /// * `node_id` - 目标节点 ID
    ///
    /// # Returns
    /// * `Ok(())` - 断开成功
    /// * `Err(CisError::P2P(_))` - 断开过程中发生错误
    async fn disconnect(&self, node_id: &str) -> Result<()>;

    /// 获取已连接节点列表
    ///
    /// # Returns
    /// * `Ok(Vec<PeerInfo>)` - 连接节点列表
    /// * `Err(CisError::P2P(_))` - 获取失败
    async fn connected_peers(&self) -> Result<Vec<PeerInfo>>;

    /// 获取已发现的节点列表
    ///
    /// # Returns
    /// * `Ok(Vec<PeerInfo>)` - 发现的节点列表
    /// * `Err(CisError::P2P(_))` - 获取失败
    async fn discovered_peers(&self) -> Result<Vec<PeerInfo>>;

    /// 获取特定节点信息
    ///
    /// # Arguments
    /// * `node_id` - 目标节点 ID
    ///
    /// # Returns
    /// * `Ok(Some(PeerInfo))` - 节点信息
    /// * `Ok(None)` - 节点未找到
    /// * `Err(CisError::P2P(_))` - 查询失败
    async fn get_peer(&self, node_id: &str) -> Result<Option<PeerInfo>>;

    /// 获取网络状态
    ///
    /// # Returns
    /// * `Ok(NetworkStatus)` - 网络状态
    /// * `Err(CisError::P2P(_))` - 获取失败
    async fn status(&self) -> Result<NetworkStatus>;

    /// 启动网络服务
    ///
    /// # Returns
    /// * `Ok(())` - 启动成功
    /// * `Err(CisError::P2P(_))` - 启动失败
    async fn start(&self) -> Result<()>;

    /// 停止网络服务
    ///
    /// # Returns
    /// * `Ok(())` - 停止成功
    /// * `Err(CisError::P2P(_))` - 停止过程中发生错误
    async fn stop(&self) -> Result<()>;

    /// 获取节点 ID
    ///
    /// # Returns
    /// * `Ok(String)` - 节点 ID
    /// * `Err(CisError::P2P(_))` - 获取失败（如服务未启动）
    fn node_id(&self) -> Result<String>;

    /// 获取 DID
    ///
    /// # Returns
    /// * `Ok(String)` - DID
    /// * `Err(CisError::Identity(_))` - 获取失败
    fn did(&self) -> Result<String>;

    /// 检查节点是否已连接
    ///
    /// # Arguments
    /// * `node_id` - 节点 ID
    ///
    /// # Returns
    /// * `Ok(true)` - 已连接
    /// * `Ok(false)` - 未连接
    /// * `Err(CisError::P2P(_))` - 检查失败
    async fn is_connected(&self, node_id: &str) -> Result<bool>;
}

/// NetworkService 的 Arc 包装类型
pub type NetworkServiceRef = Arc<dyn NetworkService>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_options_builder() {
        let opts = SendOptions::new()
            .with_priority(MessagePriority::High)
            .with_timeout(Duration::from_secs(60))
            .with_ack(true)
            .with_retry(5)
            .with_metadata("key", "value");

        assert_eq!(opts.priority, MessagePriority::High);
        assert_eq!(opts.timeout, Duration::from_secs(60));
        assert!(opts.require_ack);
        assert_eq!(opts.retry_count, 5);
        assert_eq!(opts.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_message_priority_ordering() {
        assert!(MessagePriority::Critical < MessagePriority::High);
        assert!(MessagePriority::High < MessagePriority::Normal);
        assert!(MessagePriority::Normal < MessagePriority::Low);
        assert!(MessagePriority::Low < MessagePriority::Background);
    }

    #[test]
    fn test_peer_info_creation() {
        let peer = PeerInfo {
            node_id: "node-1".to_string(),
            did: "did:cis:node-1".to_string(),
            address: "127.0.0.1:8080".to_string(),
            connected: true,
            last_seen: std::time::SystemTime::now(),
            last_sync_at: Some(Utc::now()),
            latency_ms: Some(50),
            protocol_version: "1.0".to_string(),
            capabilities: vec!["storage".to_string(), "compute".to_string()],
        };

        assert_eq!(peer.node_id, "node-1");
        assert!(peer.connected);
        assert_eq!(peer.latency_ms, Some(50));
    }
}
