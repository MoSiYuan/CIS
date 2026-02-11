//! # 事件总线集成测试
//!
//! 测试事件总线的核心功能。

use cis_core::event_bus::{MemoryEventBus, EventBus, EventBusExt, Subscription, EventFilter};
use cis_core::events::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// ==================== 基础功能测试 ====================

#[tokio::test]
async fn test_basic_publish_subscribe() {
    let bus = MemoryEventBus::new();
    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();

    // 订阅
    bus.subscribe_fn("room.message", move |_event| {
        received_clone.fetch_add(1, Ordering::SeqCst);
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

    assert_eq!(received.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_multiple_subscribers_same_topic() {
    let bus = MemoryEventBus::new();
    let counter1 = Arc::new(AtomicUsize::new(0));
    let counter2 = Arc::new(AtomicUsize::new(0));
    let counter3 = Arc::new(AtomicUsize::new(0));

    let c1 = counter1.clone();
    let c2 = counter2.clone();
    let c3 = counter3.clone();

    // 三个订阅者订阅同一主题
    bus.subscribe_fn("skill.execute", move |_| { c1.fetch_add(1, Ordering::SeqCst); }).await.unwrap();
    bus.subscribe_fn("skill.execute", move |_| { c2.fetch_add(1, Ordering::SeqCst); }).await.unwrap();
    bus.subscribe_fn("skill.execute", move |_| { c3.fetch_add(1, Ordering::SeqCst); }).await.unwrap();

    // 发布事件
    let event = SkillExecuteEvent::new(
        "test-skill",
        "execute",
        serde_json::json!({"arg": "value"}),
        "user-1",
        ExecutionContext::from_room("room-1", "user-1"),
    );
    bus.publish(EventWrapper::SkillExecute(event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(counter1.load(Ordering::SeqCst), 1);
    assert_eq!(counter2.load(Ordering::SeqCst), 1);
    assert_eq!(counter3.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_different_topics_isolation() {
    let bus = MemoryEventBus::new();
    let room_counter = Arc::new(AtomicUsize::new(0));
    let skill_counter = Arc::new(AtomicUsize::new(0));
    let agent_counter = Arc::new(AtomicUsize::new(0));

    let rc = room_counter.clone();
    let sc = skill_counter.clone();
    let ac = agent_counter.clone();

    bus.subscribe_fn("room.message", move |_| { rc.fetch_add(1, Ordering::SeqCst); }).await.unwrap();
    bus.subscribe_fn("skill.execute", move |_| { sc.fetch_add(1, Ordering::SeqCst); }).await.unwrap();
    bus.subscribe_fn("agent.online", move |_| { ac.fetch_add(1, Ordering::SeqCst); }).await.unwrap();

    // 只发布 room.message
    let event = RoomMessageEvent::new(
        "room-1",
        "user-1",
        MessageContent::Text { body: "Hello".to_string() },
    );
    bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(room_counter.load(Ordering::SeqCst), 1);
    assert_eq!(skill_counter.load(Ordering::SeqCst), 0);
    assert_eq!(agent_counter.load(Ordering::SeqCst), 0);
}

// ==================== 订阅管理测试 ====================

#[tokio::test]
async fn test_unsubscribe() {
    let bus = MemoryEventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    // 订阅并获取句柄
    let sub = bus.subscribe_fn("room.message", move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    }).await.unwrap();

    // 发布第一个事件
    let event1 = RoomMessageEvent::new("room-1", "user-1", MessageContent::Text { body: "msg1".to_string() });
    bus.publish(EventWrapper::RoomMessage(event1)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // 取消订阅
    bus.unsubscribe(&sub).await.unwrap();

    // 发布第二个事件
    let event2 = RoomMessageEvent::new("room-1", "user-1", MessageContent::Text { body: "msg2".to_string() });
    bus.publish(EventWrapper::RoomMessage(event2)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    // 计数应该还是 1，因为已取消订阅
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_subscriber_count() {
    let bus = MemoryEventBus::new();

    assert_eq!(bus.subscriber_count(None).await, 0);
    assert_eq!(bus.subscriber_count(Some("room.message")).await, 0);

    let sub1 = bus.subscribe_fn("room.message", |_event| {}).await.unwrap();
    let sub2 = bus.subscribe_fn("room.message", |_event| {}).await.unwrap();
    let _sub3 = bus.subscribe_fn("skill.execute", |_event| {}).await.unwrap();

    assert_eq!(bus.subscriber_count(None).await, 3);
    assert_eq!(bus.subscriber_count(Some("room.message")).await, 2);
    assert_eq!(bus.subscriber_count(Some("skill.execute")).await, 1);
    assert_eq!(bus.subscriber_count(Some("nonexistent")).await, 0);

    // 取消一个订阅
    bus.unsubscribe(&sub1).await.unwrap();
    assert_eq!(bus.subscriber_count(Some("room.message")).await, 1);

    bus.unsubscribe(&sub2).await.unwrap();
    assert_eq!(bus.subscriber_count(Some("room.message")).await, 0);
}

// ==================== 历史记录测试 ====================

#[tokio::test]
async fn test_history_basic() {
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

    // 验证最新的在前
    if let EventWrapper::RoomMessage(e) = &history[0] {
        assert_eq!(e.text_body(), Some("msg-4"));
    } else {
        panic!("Expected RoomMessage event");
    }
}

#[tokio::test]
async fn test_history_clear() {
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
async fn test_history_with_capacity() {
    let bus = MemoryEventBus::with_capacity(3);

    // 发布超过容量的消息
    for i in 0..5 {
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: format!("msg-{}", i) },
        );
        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();
    }

    // 获取所有历史
    let all_history = bus.get_all_history().await;
    // 应该只保留最新的 3 条
    assert_eq!(all_history.len(), 3);
}

// ==================== 统计信息测试 ====================

#[tokio::test]
async fn test_stats() {
    let bus = MemoryEventBus::new();

    // 初始状态
    let stats = bus.stats().await;
    assert_eq!(stats.total_published, 0);
    assert_eq!(stats.total_delivered, 0);

    // 添加订阅者
    bus.subscribe_fn("room.message", |_event| {}).await.unwrap();

    // 发布事件
    for i in 0..3 {
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: format!("msg-{}", i) },
        );
        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    let stats = bus.stats().await;
    assert_eq!(stats.total_published, 3);
    assert_eq!(stats.total_delivered, 3);
    assert_eq!(stats.active_subscriptions, 1);
    assert!(stats.per_topic_stats.contains_key("room.message"));
    assert_eq!(stats.per_topic_stats.get("room.message"), Some(&3));
}

// ==================== 事件类型测试 ====================

#[tokio::test]
async fn test_all_event_types() {
    let bus = MemoryEventBus::new();
    let counts = Arc::new((
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ));

    bus.subscribe_fn("room.message", {
        let counts = counts.clone();
        move |_| { counts.0.fetch_add(1, Ordering::SeqCst); }
    }).await.unwrap();
    bus.subscribe_fn("skill.execute", {
        let counts = counts.clone();
        move |_| { counts.1.fetch_add(1, Ordering::SeqCst); }
    }).await.unwrap();
    bus.subscribe_fn("skill.completed", {
        let counts = counts.clone();
        move |_| { counts.2.fetch_add(1, Ordering::SeqCst); }
    }).await.unwrap();
    bus.subscribe_fn("agent.online", {
        let counts = counts.clone();
        move |_| { counts.3.fetch_add(1, Ordering::SeqCst); }
    }).await.unwrap();
    bus.subscribe_fn("federation.task", {
        let counts = counts.clone();
        move |_| { counts.4.fetch_add(1, Ordering::SeqCst); }
    }).await.unwrap();

    // 发布所有类型的事件
    let room_event = RoomMessageEvent::new("room-1", "user-1", MessageContent::Text { body: "test".to_string() });
    bus.publish(EventWrapper::RoomMessage(room_event)).await.unwrap();

    let skill_event = SkillExecuteEvent::new(
        "test-skill",
        "execute",
        serde_json::json!({}),
        "user-1",
        ExecutionContext::from_room("room-1", "user-1"),
    );
    bus.publish(EventWrapper::SkillExecute(skill_event)).await.unwrap();

    let completed_event = SkillCompletedEvent::new(
        "orig-1",
        "test-skill",
        ExecutionResult::success(serde_json::json!({"result": "ok"})),
        "executor-1",
    );
    bus.publish(EventWrapper::SkillCompleted(completed_event)).await.unwrap();

    let agent_event = AgentOnlineEvent::new(
        "node-1",
        "agent-1",
        vec![Capability {
            id: "skill.echo".to_string(),
            name: "Echo".to_string(),
            capability_type: "skill".to_string(),
            parameters: None,
        }],
    );
    bus.publish(EventWrapper::AgentOnline(agent_event)).await.unwrap();

    let task_event = FederationTaskEvent::new(
        "task-1",
        "node-1",
        "node-2",
        Task {
            task_type: "compute".to_string(),
            parameters: serde_json::json!({"data": "value"}),
            priority: 1,
            timeout_secs: 60,
        },
    );
    bus.publish(EventWrapper::FederationTask(task_event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(counts.0.load(Ordering::SeqCst), 1, "room.message count mismatch");
    assert_eq!(counts.1.load(Ordering::SeqCst), 1, "skill.execute count mismatch");
    assert_eq!(counts.2.load(Ordering::SeqCst), 1, "skill.completed count mismatch");
    assert_eq!(counts.3.load(Ordering::SeqCst), 1, "agent.online count mismatch");
    assert_eq!(counts.4.load(Ordering::SeqCst), 1, "federation.task count mismatch");
}

// ==================== 并发测试 ====================

#[tokio::test]
async fn test_concurrent_publish() {
    let bus = Arc::new(MemoryEventBus::new());
    let counter = Arc::new(AtomicUsize::new(0));

    // 订阅
    let counter_clone = counter.clone();
    bus.subscribe_fn("room.message", move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    }).await.unwrap();

    // 并发发布
    let mut handles = vec![];
    for i in 0..100 {
        let bus_clone = bus.clone();
        let handle = tokio::spawn(async move {
            let event = RoomMessageEvent::new(
                "room-1",
                "user-1",
                MessageContent::Text { body: format!("msg-{}", i) },
            );
            bus_clone.publish(EventWrapper::RoomMessage(event)).await.unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 100);
}

// ==================== EventBus Trait 对象测试 ====================

#[tokio::test]
async fn test_event_bus_trait_object() {
    let bus: Arc<dyn EventBus> = Arc::new(MemoryEventBus::new());
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    bus.subscribe_fn("room.message", move |_| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    }).await.unwrap();

    let event = RoomMessageEvent::new(
        "room-1",
        "user-1",
        MessageContent::Text { body: "test".to_string() },
    );
    bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

// ==================== 过滤器测试 ====================

#[tokio::test]
async fn test_event_filter() {
    let bus = MemoryEventBus::new();

    // 发布不同类型的事件
    let room_event = RoomMessageEvent::new("room-1", "user-1", MessageContent::Text { body: "test".to_string() });
    bus.publish(EventWrapper::RoomMessage(room_event)).await.unwrap();

    let skill_event = SkillExecuteEvent::new(
        "test-skill",
        "execute",
        serde_json::json!({}),
        "user-1",
        ExecutionContext::from_room("room-1", "user-1"),
    );
    bus.publish(EventWrapper::SkillExecute(skill_event)).await.unwrap();

    // 使用过滤器查询
    let filter = EventFilter::new()
        .with_event_type("room.message");
    
    let results = bus.query_history(&filter).await;
    assert_eq!(results.len(), 1);

    let filter2 = EventFilter::new()
        .with_event_type("skill.execute");
    
    let results2 = bus.query_history(&filter2).await;
    assert_eq!(results2.len(), 1);
}

// ==================== 边界情况测试 ====================

#[tokio::test]
async fn test_no_subscribers() {
    let bus = MemoryEventBus::new();

    // 发布到没有订阅者的主题
    let event = RoomMessageEvent::new(
        "room-1",
        "user-1",
        MessageContent::Text { body: "test".to_string() },
    );
    
    // 不应该出错
    bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

    // 历史应该仍然记录
    let history = bus.get_history("room.message", 10).await.unwrap();
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn test_unsubscribe_nonexistent() {
    let bus = MemoryEventBus::new();

    let fake_sub = Subscription::new("fake-id", "fake-topic");
    let result = bus.unsubscribe(&fake_sub).await;
    
    // 应该返回错误
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_history() {
    let bus = MemoryEventBus::new();

    let history = bus.get_history("nonexistent", 10).await.unwrap();
    assert!(history.is_empty());
}

// ==================== 内存泄漏测试 ====================

#[tokio::test]
async fn test_no_memory_leak_on_unsubscribe() {
    let bus = Arc::new(MemoryEventBus::new());

    // 循环创建和销毁订阅
    for _ in 0..1000 {
        let sub = bus.subscribe_fn("room.message", |_event| {}).await.unwrap();
        bus.unsubscribe(&sub).await.unwrap();
    }

    // 所有订阅都应该被清理
    assert_eq!(bus.subscriber_count(None).await, 0);
}

#[tokio::test]
async fn test_history_memory_limit() {
    let bus = MemoryEventBus::with_capacity(10);

    // 发布大量事件
    for i in 0..100 {
        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: format!("msg-{}", i) },
        );
        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();
    }

    // 历史应该被限制在容量范围内
    let all_history = bus.get_all_history().await;
    assert_eq!(all_history.len(), 10);
}
