//! # 事件总线模块
//!
//! 实现 CIS 系统的事件发布订阅机制。
//!
//! ## 设计原则
//!
//! - **无全局状态**: 每个事件总线实例独立，禁止全局单例
//! - **显式路由**: 每个事件有明确的发送者和接收者
//! - **事件持久化**: 重要事件持久化存储，禁止丢失
//! - **内存安全**: 防止订阅者泄漏导致的内存泄漏
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::event_bus::{MemoryEventBus, EventBus};
//! use cis_core::events::{RoomMessageEvent, MessageContent};
//! use std::sync::Arc;
//!
//! # async fn example() {
//! let bus = Arc::new(MemoryEventBus::new());
//!
//! // 订阅事件
//! let subscription = bus.subscribe_fn("room.message", |event| {
//!     println!("Received: {:?}", event);
//! }).await.unwrap();
//!
//! // 发布事件
//! let event = RoomMessageEvent::new(
//!     "room-1",
//!     "user-1",
//!     MessageContent::Text { body: "Hello".to_string() }
//! );
//! bus.publish(event).await.unwrap();
//!
//! // 取消订阅
//! bus.unsubscribe(&subscription).await.unwrap();
//! # }
//! ```

use async_trait::async_trait;
use crate::error::{CisError, Result};
use crate::events::EventWrapper;
use std::sync::Arc;

pub mod memory;
pub mod handler;

pub use memory::MemoryEventBus;
pub use handler::{EventHandler, TypedEventHandler};

/// 订阅句柄
///
/// 用于管理事件订阅的生命周期。当订阅不再需要时，
/// 必须调用 `unsubscribe` 方法或让句柄离开作用域。
#[derive(Debug, Clone)]
pub struct Subscription {
    /// 订阅唯一 ID
    pub id: String,
    /// 订阅的主题
    pub topic: String,
}

impl Subscription {
    /// 创建新的订阅句柄
    pub fn new(id: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            topic: topic.into(),
        }
    }
}

/// 事件处理器类型
pub type EventHandlerFn = Box<dyn Fn(EventWrapper) + Send + Sync>;

/// 事件总线 Trait
///
/// 定义事件发布和订阅的抽象接口。
///
/// ## 注意
///
/// 此 trait 是 dyn 兼容的（object safe），可以安全地用于 `Arc<dyn EventBus>`。
#[async_trait]
pub trait EventBus: Send + Sync {
    /// 发布事件
    ///
    /// # Arguments
    /// * `event` - 事件包装器
    ///
    /// # Returns
    /// * `Ok(())` - 发布成功
    /// * `Err(CisError)` - 发布失败
    async fn publish(&self, event: EventWrapper) -> Result<()>;

    /// 使用函数回调订阅事件
    ///
    /// # Arguments
    /// * `topic` - 事件主题
    /// * `handler` - 事件处理函数（Box 包装）
    ///
    /// # Returns
    /// * `Ok(Subscription)` - 订阅成功，返回订阅句柄
    /// * `Err(CisError)` - 订阅失败
    async fn subscribe_boxed(
        &self,
        topic: &str,
        handler: EventHandlerFn,
    ) -> Result<Subscription>;

    /// 取消订阅
    ///
    /// # Arguments
    /// * `subscription` - 订阅句柄
    ///
    /// # Returns
    /// * `Ok(())` - 取消成功
    /// * `Err(CisError)` - 取消失败（订阅不存在）
    async fn unsubscribe(&self, subscription: &Subscription) -> Result<()>;

    /// 获取指定主题的历史事件
    ///
    /// # Arguments
    /// * `topic` - 事件主题
    /// * `limit` - 最大返回数量
    ///
    /// # Returns
    /// * `Ok(Vec<EventWrapper>)` - 历史事件列表
    async fn get_history(&self, topic: &str, limit: usize) -> Result<Vec<EventWrapper>>;

    /// 获取订阅者数量
    ///
    /// # Arguments
    /// * `topic` - 事件主题（可选，为空时返回总订阅者数）
    ///
    /// # Returns
    /// * 订阅者数量
    async fn subscriber_count(&self, topic: Option<&str>) -> usize;
}

/// EventBus 扩展 trait（非 dyn 兼容）
///
/// 提供更方便的泛型方法，需要具体类型才能使用。
#[async_trait]
pub trait EventBusExt: EventBus {
    /// 使用函数回调订阅事件（泛型版本）
    ///
    /// # Arguments
    /// * `topic` - 事件主题
    /// * `handler` - 事件处理函数
    ///
    /// # Returns
    /// * `Ok(Subscription)` - 订阅成功，返回订阅句柄
    /// * `Err(CisError)` - 订阅失败
    async fn subscribe_fn<F>(&self, topic: &str, handler: F) -> Result<Subscription>
    where
        F: Fn(EventWrapper) + Send + Sync + 'static,
    {
        self.subscribe_boxed(topic, Box::new(handler)).await
    }
}

impl<T: EventBus + ?Sized> EventBusExt for T {}

/// EventBus 的 Arc 包装类型
pub type EventBusRef = Arc<dyn EventBus>;

/// 事件总线错误
#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("Topic not found: {0}")]
    TopicNotFound(String),
    
    #[error("Subscription not found: {0}")]
    SubscriptionNotFound(String),
    
    #[error("Event serialization failed: {0}")]
    SerializationError(String),
    
    #[error("Event deserialization failed: {0}")]
    DeserializationError(String),
    
    #[error("Handler error: {0}")]
    HandlerError(String),
    
    #[error("Channel closed")]
    ChannelClosed,
}

impl From<EventBusError> for CisError {
    fn from(err: EventBusError) -> Self {
        CisError::internal(err.to_string())
    }
}

/// 事件过滤条件
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// 事件类型过滤
    pub event_types: Vec<String>,
    /// 发送者过滤
    pub senders: Vec<String>,
    /// 源节点过滤
    pub source_nodes: Vec<String>,
    /// 时间范围 - 开始
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 时间范围 - 结束
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl EventFilter {
    /// 创建新的过滤器
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加事件类型过滤
    pub fn with_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_types.push(event_type.into());
        self
    }

    /// 添加发送者过滤
    pub fn with_sender(mut self, sender: impl Into<String>) -> Self {
        self.senders.push(sender.into());
        self
    }

    /// 添加源节点过滤
    pub fn with_source_node(mut self, node: impl Into<String>) -> Self {
        self.source_nodes.push(node.into());
        self
    }

    /// 设置时间范围
    pub fn with_time_range(
        mut self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    /// 检查事件是否匹配过滤器
    pub fn matches(&self, event: &EventWrapper) -> bool {
        // 检查事件类型
        if !self.event_types.is_empty() {
            let event_type = event.event_type();
            if !self.event_types.iter().any(|t| t == event_type) {
                return false;
            }
        }

        // 检查时间范围
        let timestamp = event.timestamp();
        if let Some(start) = self.start_time {
            if timestamp < start {
                return false;
            }
        }
        if let Some(end) = self.end_time {
            if timestamp > end {
                return false;
            }
        }

        true
    }
}

/// 事件统计信息
#[derive(Debug, Clone)]
pub struct EventStats {
    /// 总发布事件数
    pub total_published: u64,
    /// 总分发事件数
    pub total_delivered: u64,
    /// 活跃订阅数
    pub active_subscriptions: usize,
    /// 历史事件数
    pub history_size: usize,
    /// 按主题统计的发布数
    pub per_topic_stats: std::collections::HashMap<String, u64>,
}

impl EventStats {
    /// 创建空的统计信息
    pub fn empty() -> Self {
        Self {
            total_published: 0,
            total_delivered: 0,
            active_subscriptions: 0,
            history_size: 0,
            per_topic_stats: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBusExt;
    use crate::events::{RoomMessageEvent, MessageContent};

    #[tokio::test]
    async fn test_subscription_creation() {
        let sub = Subscription::new("sub-1", "room.message");
        assert_eq!(sub.id, "sub-1");
        assert_eq!(sub.topic, "room.message");
    }

    #[tokio::test]
    async fn test_event_filter_event_type() {
        let filter = EventFilter::new()
            .with_event_type("room.message");

        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );
        let wrapper = EventWrapper::RoomMessage(event);

        assert!(filter.matches(&wrapper));

        // 创建不同类型的过滤器
        let skill_filter = EventFilter::new()
            .with_event_type("skill.execute");
        
        assert!(!skill_filter.matches(&wrapper));
    }

    #[tokio::test]
    async fn test_event_filter_time_range() {
        let now = chrono::Utc::now();
        let filter = EventFilter::new()
            .with_time_range(now - chrono::Duration::hours(1), now + chrono::Duration::hours(1));

        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );
        let wrapper = EventWrapper::RoomMessage(event);

        assert!(filter.matches(&wrapper));

        // 未来时间的过滤器不应该匹配
        let future_filter = EventFilter::new()
            .with_time_range(now + chrono::Duration::hours(1), now + chrono::Duration::hours(2));
        
        assert!(!future_filter.matches(&wrapper));
    }

    #[test]
    fn test_event_stats_empty() {
        let stats = EventStats::empty();
        assert_eq!(stats.total_published, 0);
        assert_eq!(stats.active_subscriptions, 0);
    }
}
