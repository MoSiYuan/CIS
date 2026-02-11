//! # Mock Event Bus
//!
//! 事件总线的 Mock 实现，用于测试事件驱动功能。

use super::MockCallTracker;
use crate::error::{CisError, Result};
use crate::event_bus::{EventBus, Subscription, EventHandlerFn};
use crate::events::EventWrapper;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock as AsyncRwLock;

/// 事件 ID 生成器
static EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Mock 事件
#[derive(Debug, Clone)]
pub struct MockEvent {
    pub id: u64,
    pub topic: String,
    pub payload: String,
    pub timestamp: u64,
}

impl MockEvent {
    /// 创建新事件
    pub fn new(topic: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            id: EVENT_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            topic: topic.into(),
            payload: payload.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// 从 EventWrapper 创建
    pub fn from_wrapper(wrapper: &EventWrapper) -> Self {
        let topic = wrapper.event_type().to_string();
        let payload = serde_json::to_string(wrapper).unwrap_or_default();
        Self::new(topic, payload)
    }
}

/// 事件处理器类型
pub type MockEventHandler = Arc<dyn Fn(&MockEvent) + Send + Sync>;

/// 事件总线 Mock
#[derive(Clone)]
pub struct MockEventBus {
    tracker: MockCallTracker,
    subscribers: Arc<AsyncRwLock<HashMap<String, Vec<(String, MockEventHandler)>>>>,
    published_events: Arc<AsyncRwLock<Vec<MockEvent>>>,
    should_fail_next: Arc<Mutex<Option<CisError>>>,
    auto_invoke: Arc<Mutex<bool>>,
}

impl std::fmt::Debug for MockEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockEventBus")
            .field("event_count", &self.published_events.try_read().map(|e| e.len()).unwrap_or(0))
            .finish()
    }
}

impl MockEventBus {
    /// 创建新的 Mock
    pub fn new() -> Self {
        Self {
            tracker: MockCallTracker::new(),
            subscribers: Arc::new(AsyncRwLock::new(HashMap::new())),
            published_events: Arc::new(AsyncRwLock::new(Vec::new())),
            should_fail_next: Arc::new(Mutex::new(None)),
            auto_invoke: Arc::new(Mutex::new(true)),
        }
    }

    /// 创建不自动调用处理器的 Mock
    pub fn without_auto_invoke() -> Self {
        let bus = Self::new();
        *bus.auto_invoke.lock().unwrap() = false;
        bus
    }

    /// 订阅事件（旧 API）
    pub async fn subscribe<F>(&self, topic: impl Into<String>, handler: F)
    where
        F: Fn(&MockEvent) + Send + Sync + 'static,
    {
        let topic = topic.into();
        self.tracker.record("subscribe", vec![topic.clone()]);

        let sub_id = format!("sub_{}", uuid::Uuid::new_v4());
        let mut subscribers = self.subscribers.write().await;
        subscribers
            .entry(topic)
            .or_insert_with(Vec::new)
            .push((sub_id, Arc::new(handler)));
    }

    /// 取消订阅（旧 API）
    pub async fn unsubscribe_topic(&self, topic: impl Into<String>) {
        let topic = topic.into();
        self.tracker.record("unsubscribe", vec![topic.clone()]);

        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(&topic);
    }

    /// 发布事件（旧 API）
    pub async fn publish_mock(&self, topic: impl Into<String>, payload: impl Into<String>) -> Result<()> {
        let topic = topic.into();
        let payload = payload.into();
        self.tracker.record("publish", vec![topic.clone(), payload.clone()]);

        // 检查是否有预设的错误
        if let Some(err) = self.should_fail_next.lock().unwrap().take() {
            return Err(err);
        }

        let event = MockEvent::new(&topic, payload);

        // 记录事件
        {
            let mut events = self.published_events.write().await;
            events.push(event.clone());
        }

        // 自动调用处理器
        if *self.auto_invoke.lock().unwrap() {
            let subscribers = self.subscribers.read().await;
            if let Some(handlers) = subscribers.get(&topic) {
                for (_, handler) in handlers {
                    handler(&event);
                }
            }
            
            // 调用通配符处理器
            if let Some(wildcard_handlers) = subscribers.get("*") {
                for (_, handler) in wildcard_handlers {
                    handler(&event);
                }
            }
        }

        Ok(())
    }

    /// 获取已发布的事件
    pub async fn get_published_events(&self) -> Vec<MockEvent> {
        self.published_events.read().await.clone()
    }

    /// 获取特定主题的事件
    pub async fn get_events_by_topic(&self, topic: impl AsRef<str>) -> Vec<MockEvent> {
        let topic = topic.as_ref();
        self.published_events
            .read()
            .await
            .iter()
            .filter(|e| e.topic == topic)
            .cloned()
            .collect()
    }

    /// 获取最后发布的事件
    pub async fn get_last_event(&self) -> Option<MockEvent> {
        self.published_events.read().await.last().cloned()
    }

    /// 获取事件数量
    pub async fn event_count(&self) -> usize {
        self.published_events.read().await.len()
    }

    /// 获取特定主题的事件数量
    pub async fn event_count_by_topic(&self, topic: impl AsRef<str>) -> usize {
        self.get_events_by_topic(topic).await.len()
    }

    /// 检查是否有订阅者
    pub async fn has_subscribers(&self, topic: impl AsRef<str>) -> bool {
        let topic = topic.as_ref();
        let subscribers = self.subscribers.read().await;
        subscribers
            .get(topic)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    /// 设置下一次操作失败
    pub fn will_fail_next(&self, error: CisError) {
        *self.should_fail_next.lock().unwrap() = Some(error);
    }

    /// 清空调用记录和事件
    pub async fn clear(&self) {
        self.tracker.clear();
        self.published_events.write().await.clear();
    }

    /// 手动触发处理器
    pub async fn invoke_handlers(&self, event: &MockEvent) {
        let subscribers = self.subscribers.read().await;
        if let Some(handlers) = subscribers.get(&event.topic) {
            for (_, handler) in handlers {
                handler(event);
            }
        }
    }

    // === 验证方法 ===

    /// 断言：事件被发布
    pub async fn assert_event_published(&self, topic: impl AsRef<str>) {
        let topic = topic.as_ref();
        let count = self.event_count_by_topic(topic).await;
        assert!(
            count > 0,
            "Expected event '{}' to be published at least once, but was never published",
            topic
        );
    }

    /// 断言：事件被发布指定次数
    pub async fn assert_event_count(&self, topic: impl AsRef<str>, expected: usize) {
        let topic = topic.as_ref();
        let actual = self.event_count_by_topic(topic).await;
        assert_eq!(
            actual, expected,
            "Expected event '{}' to be published {} times, but was published {} times",
            topic, expected, actual
        );
    }

    /// 断言：没有事件被发布
    pub async fn assert_no_events_published(&self) {
        let count = self.event_count().await;
        assert_eq!(
            count, 0,
            "Expected no events to be published, but {} events were published",
            count
        );
    }

    /// 断言：事件包含特定数据（简化版本，检查 payload 字符串）
    pub async fn assert_event_contains(&self, topic: impl AsRef<str>, key: &str, value: &serde_json::Value) {
        let topic = topic.as_ref();
        let events = self.get_events_by_topic(topic).await;
        
        let found = events.iter().any(|e| {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&e.payload) {
                json_val
                    .get(key)
                    .map(|v| v == value)
                    .unwrap_or(false)
            } else {
                false
            }
        });

        assert!(
            found,
            "Expected event '{}' to contain {}={:?}",
            topic, key, value
        );
    }

    /// 断言：有订阅者
    pub async fn assert_has_subscribers(&self, topic: impl AsRef<str>) {
        let topic = topic.as_ref();
        assert!(
            self.has_subscribers(topic).await,
            "Expected topic '{}' to have subscribers",
            topic
        );
    }

    /// 断言：方法被调用
    pub fn assert_called(&self, method: &str) {
        self.tracker.assert_called(method);
    }

    /// 断言：方法被调用指定次数
    pub fn assert_call_count(&self, method: &str, expected: usize) {
        self.tracker.assert_call_count(method, expected);
    }

    /// 获取调用追踪器
    pub fn tracker(&self) -> &MockCallTracker {
        &self.tracker
    }
}

impl Default for MockEventBus {
    fn default() -> Self {
        Self::new()
    }
}

// EventBus trait implementation for new interface
#[async_trait]
impl EventBus for MockEventBus {
    async fn publish(&self, event: EventWrapper) -> Result<()> {
        let mock_event = MockEvent::from_wrapper(&event);
        let payload = mock_event.payload.clone();
        
        self.tracker.record("publish", vec![mock_event.topic.clone(), payload]);

        // 检查是否有预设的错误
        if let Some(err) = self.should_fail_next.lock().unwrap().take() {
            return Err(err);
        }

        // 记录事件
        {
            let mut events = self.published_events.write().await;
            events.push(mock_event.clone());
        }

        // 自动调用处理器
        if *self.auto_invoke.lock().unwrap() {
            let subscribers = self.subscribers.read().await;
            let topic = mock_event.topic.clone();
            if let Some(handlers) = subscribers.get(&topic) {
                for (_, handler) in handlers {
                    handler(&mock_event);
                }
            }
            
            // 调用通配符处理器
            if let Some(wildcard_handlers) = subscribers.get("*") {
                for (_, handler) in wildcard_handlers {
                    handler(&mock_event);
                }
            }
        }

        Ok(())
    }

    async fn subscribe_boxed(
        &self,
        topic: &str,
        _handler: EventHandlerFn,
    ) -> Result<Subscription> {
        self.tracker.record("subscribe", vec![topic.to_string()]);

        let sub_id = format!("sub_{}", uuid::Uuid::new_v4());
        // 注意：MockEventBus 不支持新的 handler 类型，简化处理
        
        Ok(Subscription::new(sub_id, topic))
    }

    async fn unsubscribe(&self, subscription: &Subscription) -> Result<()> {
        self.tracker.record("unsubscribe", vec![subscription.topic.clone()]);
        
        let mut subscribers = self.subscribers.write().await;
        if let Some(subs) = subscribers.get_mut(&subscription.topic) {
            subs.retain(|(id, _)| id != &subscription.id);
        }
        
        Ok(())
    }

    async fn get_history(&self, topic: &str, limit: usize) -> Result<Vec<EventWrapper>> {
        // 返回空列表，因为 MockEventBus 存储的是 MockEvent
        // 在实际测试中，使用 get_published_events 方法
        let _ = (topic, limit);
        Ok(Vec::new())
    }

    async fn subscriber_count(&self, topic: Option<&str>) -> usize {
        let subscribers = self.subscribers.read().await;
        
        match topic {
            Some(t) => subscribers.get(t).map(|s| s.len()).unwrap_or(0),
            None => subscribers.values().map(|v| v.len()).sum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_mock_publish_subscribe() {
        let mock = MockEventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        mock.subscribe("test.event", move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        mock.publish_mock("test.event", r#"{"data": "value"}"#)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        mock.assert_event_published("test.event").await;
    }

    #[tokio::test]
    async fn test_mock_event_count() {
        let mock = MockEventBus::new();

        mock.publish_mock("event.a", "{}").await.unwrap();
        mock.publish_mock("event.a", "{}").await.unwrap();
        mock.publish_mock("event.b", "{}").await.unwrap();

        mock.assert_event_count("event.a", 2).await;
        mock.assert_event_count("event.b", 1).await;
    }

    #[tokio::test]
    async fn test_mock_event_data() {
        let mock = MockEventBus::new();

        mock.publish_mock(
            "user.created",
            r#"{"id": 123, "name": "Alice"}"#,
        )
        .await
        .unwrap();

        mock.assert_event_contains("user.created", "id", &serde_json::json!(123))
            .await;
    }

    #[tokio::test]
    async fn test_mock_error_simulation() {
        let mock = MockEventBus::new();

        mock.will_fail_next(CisError::internal("Event bus error"));
        let result = mock.publish_mock("test", "{}").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_unsubscribe() {
        let mock = MockEventBus::new();

        mock.subscribe("topic", |_e| {}).await;
        mock.assert_has_subscribers("topic").await;

        mock.unsubscribe_topic("topic").await;
        assert!(!mock.has_subscribers("topic").await);
    }
}
