//! # Mock 使用示例
//!
//! 本模块展示如何在测试中使用各种 Mock。

use crate::ai::AiProvider;
use crate::traits::NetworkService;
use crate::traits::StorageService;
use crate::event_bus::EventBus;
use crate::test::mocks::{MockAiProvider, MockEventBus, MockNetworkService, MockStorageService};
use serde_json::json;

/// 示例 1: 基础 MockNetworkService 使用
#[tokio::test]
async fn example_network_service_basic() {
    let mock = MockNetworkService::new();
    mock.preset_connect("ws://localhost:8080", Ok(())).await;
    mock.connect("ws://localhost:8080").await.unwrap();
    mock.assert_connected("ws://localhost:8080").await;
    mock.assert_call_count("connect", 1);
}

/// 示例 2: MockStorageService 基础操作
#[tokio::test]
async fn example_storage_service_basic() {
    let mock = MockStorageService::new();
    mock.preset_get("user:123", Some(r#"{"name": "Alice"}"#.to_string())).await;
    let value = mock.get("user:123").await.unwrap();
    assert_eq!(value, Some(r#"{"name": "Alice"}"#.to_string()));
    mock.assert_key_accessed("user:123");
}

/// 示例 3: MockEventBus 发布订阅
#[tokio::test]
async fn example_event_bus_pub_sub() {
    let mock = MockEventBus::new();
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = counter.clone();

    mock.subscribe("user.created", move |_event| {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }).await;

    mock.publish("user.created", json!({"id": 123})).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    mock.assert_event_published("user.created").await;
}

/// 示例 4: MockAiProvider 基础聊天
#[tokio::test]
async fn example_ai_provider_chat() {
    let mock = MockAiProvider::new();
    mock.preset_chat("Hello", "Hi there!").await;
    let response = mock.chat("Hello").await.unwrap();
    assert_eq!(response, "Hi there!");
    mock.assert_call_count("chat", 1);
}

/// 示例 5: 组合多个 Mock
#[tokio::test]
async fn example_combined_mocks() {
    let network = MockNetworkService::new();
    let storage = MockStorageService::new();
    let event_bus = MockEventBus::new();
    let ai = MockAiProvider::new();

    network.preset_connect("ws://server", Ok(())).await;
    storage.seed("config", r#"{"version": "1.0"}"#).await;
    ai.preset_chat("status", "System is operational").await;

    network.connect("ws://server").await.unwrap();
    let _config = storage.get("config").await.unwrap();
    let _analysis = ai.chat("status").await.unwrap();
    event_bus.publish("system.status", json!({"connected": true})).await.unwrap();

    network.assert_connected("ws://server").await;
    storage.assert_key_accessed("config");
    ai.assert_call_count("chat", 1);
    event_bus.assert_event_published("system.status").await;
}
