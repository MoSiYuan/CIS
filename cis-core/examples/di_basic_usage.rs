//! # 依赖注入基本用法示例
//!
//! 本示例展示如何使用 ServiceContainer 进行依赖注入。
//!
//! ## 运行方式
//!
//! ```bash
//! cargo run --example dependency_injection/basic_usage --features test-utils
//! ```

use cis_core::container::ServiceContainer;
use cis_core::traits::EmbeddingServiceTrait;
use cis_core::test::mocks::{
    MockNetworkService, MockStorageService, MockEventBus,
    MockSkillExecutor, MockEmbeddingService,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CIS Dependency Injection Example ===\n");

    // 1. 创建测试容器（使用 Mock 实现）
    println!("1. Creating test container with Mock implementations...");
    
    let mock_network = Arc::new(MockNetworkService::new());
    let mock_storage = Arc::new(MockStorageService::new());
    let mock_event_bus = Arc::new(MockEventBus::new());
    let mock_executor = Arc::new(MockSkillExecutor::new());
    let mock_embedding = Arc::new(MockEmbeddingService::new());

    let container = ServiceContainer::test()
        .with_network(mock_network.clone())
        .with_storage(mock_storage.clone())
        .with_event_bus(mock_event_bus.clone())
        .with_skill_executor(mock_executor.clone())
        .with_embedding(mock_embedding.clone())
        .build();

    println!("   ✓ Container created successfully\n");

    // 2. 从容器获取服务
    println!("2. Getting services from container...");
    
    let _network = container.network();
    let _storage = container.storage();
    let _event_bus = container.event_bus();
    let _executor = container.skill_executor();
    let _embedding = container.embedding();

    println!("   ✓ All services retrieved successfully\n");

    // 3. 使用 Mock 服务
    println!("3. Using Mock services...");

    // 模拟网络操作
    mock_network.preset_connect("ws://localhost:8080", Ok(())).await;
    mock_network.connect("ws://localhost:8080").await?;
    println!("   ✓ Network connect simulated");

    // 模拟存储操作
    mock_storage.seed("key1", "value1").await;
    let value = mock_storage.get("key1").await?;
    println!("   ✓ Storage get: {:?}", value);

    // 模拟嵌入
    let embedding: Vec<f32> = mock_embedding.embed("Hello world").await?;
    println!("   ✓ Embedding generated: dim={}", embedding.len());

    println!();

    // 4. 验证调用
    println!("4. Verifying mock calls...");
    mock_network.assert_called("connect");
    println!("   ✓ Network connect was called");

    mock_storage.assert_called("get");
    println!("   ✓ Storage get was called");

    println!("\n=== Example completed successfully! ===");
    
    Ok(())
}
