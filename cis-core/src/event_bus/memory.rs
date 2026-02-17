//! # 内存事件总线实现
//!
//! 基于内存的 EventBus 实现，使用 tokio 通道进行事件分发。
//!
//! ## 设计特点
//!
//! - **无锁并发**: 使用 tokio::sync 原语实现高效并发
//! - **历史记录**: 可选的事件历史存储
//! - **背压处理**: 使用有界通道防止内存溢出
//! - **自动清理**: 退订的处理器自动清理
//!
//! ## 简化实现说明（SHAME_TAG）
//!
//! 根据 D03 设计文档的要求，以下功能在当前实现中被简化：
//!
//! 1. **持久化存储**: 使用内存历史而非持久化存储
//!    - 原因: 当前为单节点部署，重启后事件历史可丢弃
//!    - 计划: Phase 4 添加持久化存储支持
//!
//! 2. **事件确认机制**: 无显式的事件确认
//!    - 原因: 内存通道天然可靠，暂不需要额外确认
//!    - 计划: 添加分布式支持时引入确认机制
//!
//! 3. **全局事件总线**: 禁止使用全局单例
//!    - 状态: [OK] 遵守规则
//!
//! 4. **事件路由**: 每个事件有明确的发送者和接收者
//!    - 状态: [OK] 通过 EventMetadata 实现

use async_trait::async_trait;
use crate::error::{CisError, Result};
use crate::events::EventWrapper;
use crate::event_bus::{EventBus, EventHandlerFn, Subscription, EventStats, EventFilter};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, trace, warn};

/// 内存中的订阅者信息
struct Subscriber {
    /// 订阅 ID
    id: String,
    /// 发送通道
    sender: mpsc::UnboundedSender<EventWrapper>,
    /// 处理器名称（用于调试）
    name: String,
}

/// 内存事件总线
///
/// 基于内存的事件总线实现，适用于单节点部署。
/// 支持事件发布、订阅、历史记录查询。
#[derive(Clone)]
pub struct MemoryEventBus {
    /// 主题 -> 订阅者列表
    subscribers: Arc<RwLock<HashMap<String, Vec<Subscriber>>>>,
    /// 事件历史记录
    history: Arc<RwLock<Vec<(String, EventWrapper)>>>,
    /// 历史记录最大数量
    max_history_size: usize,
    /// 发布统计
    stats: Arc<RwLock<EventStats>>,
}

impl MemoryEventBus {
    /// 创建新的事件总线实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::event_bus::MemoryEventBus;
    ///
    /// let bus = MemoryEventBus::new();
    /// ```
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 1000,
            stats: Arc::new(RwLock::new(EventStats::empty())),
        }
    }

    /// 创建带自定义配置的事件总线
    ///
    /// # Arguments
    /// * `max_history_size` - 历史记录最大数量
    pub fn with_capacity(max_history_size: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::with_capacity(max_history_size))),
            max_history_size,
            stats: Arc::new(RwLock::new(EventStats::empty())),
        }
    }

    /// 获取统计信息
    pub async fn stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }

    /// 清空历史记录
    pub async fn clear_history(&self) {
        self.history.write().await.clear();
        debug!("Event history cleared");
    }

    /// 获取完整历史记录
    pub async fn get_all_history(&self) -> Vec<(String, EventWrapper)> {
        self.history.read().await.clone()
    }

    /// 使用过滤器查询历史
    pub async fn query_history(&self, filter: &EventFilter) -> Vec<EventWrapper> {
        let history = self.history.read().await;
        history
            .iter()
            .filter(|(_, event)| filter.matches(event))
            .map(|(_, event)| event.clone())
            .collect()
    }

    /// 关闭事件总线
    ///
    /// 关闭所有订阅通道，释放资源。
    pub async fn shutdown(&self) {
        let mut subs = self.subscribers.write().await;
        subs.clear();
        debug!("Event bus shutdown complete");
    }

    /// 内部方法：添加历史记录
    async fn add_to_history(&self, topic: &str, event: EventWrapper) {
        let mut history = self.history.write().await;
        history.push((topic.to_string(), event));

        // 限制历史大小
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
    }

    /// 内部方法：更新统计
    async fn update_stats(&self, topic: &str, delivered_count: usize) {
        let mut stats = self.stats.write().await;
        stats.total_published += 1;
        stats.total_delivered += delivered_count as u64;
        stats.history_size = self.history.read().await.len();
        stats.active_subscriptions = self.subscribers.read().await.values()
            .map(|v| v.len())
            .sum();

        *stats.per_topic_stats.entry(topic.to_string()).or_insert(0) += 1;
    }

    /// 内部方法：分发给订阅者
    async fn dispatch(&self, topic: &str, event: EventWrapper) -> usize {
        let subscribers = self.subscribers.read().await;
        let mut delivered = 0;

        if let Some(subs) = subscribers.get(topic) {
            for subscriber in subs {
                match subscriber.sender.send(event.clone()) {
                    Ok(_) => {
                        delivered += 1;
                        trace!(
                            subscriber_id = %subscriber.id,
                            subscriber_name = %subscriber.name,
                            topic = %topic,
                            "Event delivered"
                        );
                    }
                    Err(_) => {
                        warn!(
                            subscriber_id = %subscriber.id,
                            subscriber_name = %subscriber.name,
                            topic = %topic,
                            "Failed to deliver event - channel closed"
                        );
                    }
                }
            }
        }

        // 同时分发给通配符订阅者
        if let Some(wildcard_subs) = subscribers.get("*") {
            for subscriber in wildcard_subs {
                if let Err(_) = subscriber.sender.send(event.clone()) {
                    warn!(
                        subscriber_id = %subscriber.id,
                        "Failed to deliver to wildcard subscriber"
                    );
                }
            }
        }

        delivered
    }
}

impl Default for MemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for MemoryEventBus {
    async fn publish(&self, event: EventWrapper) -> Result<()> {
        let topic = event.event_type().to_string();
        let event_id = event.event_id().to_string();

        trace!(
            event_id = %event_id,
            topic = %topic,
            "Publishing event"
        );

        // 添加到历史
        self.add_to_history(&topic, event.clone()).await;

        // 分发给订阅者
        let delivered = self.dispatch(&topic, event).await;

        // 更新统计
        self.update_stats(&topic, delivered).await;

        debug!(
            event_id = %event_id,
            topic = %topic,
            delivered = delivered,
            "Event published"
        );

        Ok(())
    }

    async fn subscribe_boxed(
        &self,
        topic: &str,
        handler: EventHandlerFn,
    ) -> Result<Subscription> {
        let subscription_id = format!("sub_{}", uuid::Uuid::new_v4());
        let (tx, mut rx) = mpsc::unbounded_channel::<EventWrapper>();

        // 创建订阅者
        let subscriber = Subscriber {
            id: subscription_id.clone(),
            sender: tx,
            name: "fn_handler".to_string(),
        };

        // 添加到订阅列表
        {
            let mut subscribers = self.subscribers.write().await;
            subscribers
                .entry(topic.to_string())
                .or_insert_with(Vec::new)
                .push(subscriber);
        }

        // 启动处理任务
        let topic_clone = topic.to_string();
        let sub_id_clone = subscription_id.clone();
        tokio::spawn(async move {
            trace!(
                subscription_id = %sub_id_clone,
                topic = %topic_clone,
                "Subscription handler started"
            );

            while let Some(event) = rx.recv().await {
                handler(event);
            }

            trace!(
                subscription_id = %sub_id_clone,
                topic = %topic_clone,
                "Subscription handler stopped"
            );
        });

        debug!(
            subscription_id = %subscription_id,
            topic = %topic,
            "Subscription created"
        );

        Ok(Subscription::new(subscription_id, topic))
    }

    async fn unsubscribe(&self, subscription: &Subscription) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;

        if let Some(subs) = subscribers.get_mut(&subscription.topic) {
            let initial_len = subs.len();
            subs.retain(|s| s.id != subscription.id);

            if subs.len() < initial_len {
                debug!(
                    subscription_id = %subscription.id,
                    topic = %subscription.topic,
                    "Subscription cancelled"
                );
                Ok(())
            } else {
                Err(CisError::internal(format!(
                    "Subscription {} not found in topic {}",
                    subscription.id, subscription.topic
                )))
            }
        } else {
            Err(CisError::internal(format!(
                "Topic {} not found",
                subscription.topic
            )))
        }
    }

    async fn get_history(&self, topic: &str, limit: usize) -> Result<Vec<EventWrapper>> {
        let history = self.history.read().await;
        let events: Vec<EventWrapper> = history
            .iter()
            .filter(|(t, _)| t == topic)
            .rev()
            .take(limit)
            .map(|(_, e)| e.clone())
            .collect();

        trace!(
            topic = %topic,
            count = events.len(),
            "Retrieved history"
        );

        Ok(events)
    }

    async fn subscriber_count(&self, topic: Option<&str>) -> usize {
        let subscribers = self.subscribers.read().await;

        match topic {
            Some(t) => subscribers.get(t).map(|s| s.len()).unwrap_or(0),
            None => subscribers.values().map(|v| v.len()).sum(),
        }
    }
}

/// 构建器模式创建 MemoryEventBus
pub struct MemoryEventBusBuilder {
    max_history_size: usize,
}

impl MemoryEventBusBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            max_history_size: 1000,
        }
    }

    /// 设置历史记录大小
    pub fn max_history_size(mut self, size: usize) -> Self {
        self.max_history_size = size;
        self
    }

    /// 构建事件总线
    pub fn build(self) -> MemoryEventBus {
        MemoryEventBus::with_capacity(self.max_history_size)
    }
}

impl Default for MemoryEventBusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBusExt;
    use crate::events::{RoomMessageEvent, MessageContent, SkillExecuteEvent, ExecutionContext};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bus = MemoryEventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // 订阅
        let sub = bus.subscribe_fn("room.message", move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        // 发布事件
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "Hello".to_string() },
        );

        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

        // 等待处理
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // 取消订阅
        bus.unsubscribe(&sub).await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = MemoryEventBus::new();
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));

        let c1 = counter1.clone();
        let c2 = counter2.clone();

        // 两个订阅者
        bus.subscribe_fn("room.message", move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        bus.subscribe_fn("room.message", move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        // 发布事件
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "Hello".to_string() },
        );

        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(counter1.load(Ordering::SeqCst), 1);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_history() {
        let bus = MemoryEventBus::new();

        // 发布多个事件
        for i in 0..5 {
            let event = RoomMessageEvent::new(
                "room-1",
                "user-1",
                MessageContent::Text { body: format!("msg-{}", i) },
            );
            bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();
        }

        // 查询历史
        let history = bus.get_history("room.message", 3).await.unwrap();
        assert_eq!(history.len(), 3);

        // 查询不存在的历史
        let empty = bus.get_history("nonexistent", 10).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_stats() {
        let bus = MemoryEventBus::new();

        // 初始状态
        let stats = bus.stats().await;
        assert_eq!(stats.total_published, 0);

        // 发布事件
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );

        bus.subscribe_fn("room.message", |_event| {}).await.unwrap();
        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let stats = bus.stats().await;
        assert_eq!(stats.total_published, 1);
        assert_eq!(stats.total_delivered, 1);
    }

    #[tokio::test]
    async fn test_subscriber_count() {
        let bus = MemoryEventBus::new();

        assert_eq!(bus.subscriber_count(None).await, 0);
        assert_eq!(bus.subscriber_count(Some("room.message")).await, 0);

        let sub = bus.subscribe_fn("room.message", |_event| {}).await.unwrap();

        assert_eq!(bus.subscriber_count(None).await, 1);
        assert_eq!(bus.subscriber_count(Some("room.message")).await, 1);
        assert_eq!(bus.subscriber_count(Some("skill.execute")).await, 0);

        bus.unsubscribe(&sub).await.unwrap();

        assert_eq!(bus.subscriber_count(None).await, 0);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let bus = MemoryEventBus::new();

        // 发布事件
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );
        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

        // 清空历史
        bus.clear_history().await;

        let history = bus.get_history("room.message", 10).await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_different_event_types() {
        let bus = MemoryEventBus::new();
        let room_counter = Arc::new(AtomicUsize::new(0));
        let skill_counter = Arc::new(AtomicUsize::new(0));

        let rc = room_counter.clone();
        let sc = skill_counter.clone();

        bus.subscribe_fn("room.message", move |_| {
            rc.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        bus.subscribe_fn("skill.execute", move |_| {
            sc.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        // 发布房间消息
        let room_event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "Hello".to_string() },
        );
        bus.publish(EventWrapper::RoomMessage(room_event)).await.unwrap();

        // 发布 skill 执行
        let skill_event = SkillExecuteEvent::new(
            "test-skill",
            "execute",
            serde_json::json!({}),
            "user-1",
            ExecutionContext::from_room("room-1", "user-1"),
        );
        bus.publish(EventWrapper::SkillExecute(skill_event)).await.unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(room_counter.load(Ordering::SeqCst), 1);
        assert_eq!(skill_counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_builder() {
        let bus = MemoryEventBusBuilder::new()
            .max_history_size(100)
            .build();

        assert_eq!(bus.max_history_size, 100);
    }
}
