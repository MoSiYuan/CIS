//! # 使用 Mock 进行测试示例
//!
//! 本示例展示如何使用 Mock 服务进行单元测试。
//!
//! ## 运行方式
//!
//! ```bash
//! cargo run --example di_test_with_mocks --features test-utils
//! ```

use cis_core::test::mocks::{
    MockNetworkService, MockStorageService,
};
use cis_core::CisError;

/// 待测试的服务
pub struct DataSyncService {
    network: std::sync::Arc<MockNetworkService>,
    storage: std::sync::Arc<MockStorageService>,
}

impl DataSyncService {
    pub fn new(
        network: std::sync::Arc<MockNetworkService>,
        storage: std::sync::Arc<MockStorageService>,
    ) -> Self {
        Self { network, storage }
    }

    /// 同步数据到远程节点
    pub async fn sync_to_remote(&self, key: &str, node_id: &str) -> Result<(), String> {
        // 从存储获取数据
        let value: String = self.storage.get(key).await
            .map_err(|e| format!("Storage error: {}", e))?
            .ok_or_else(|| format!("Key not found: {}", key))?;

        // 发送到远程节点 (使用 MockNetworkService 的 send 方法)
        self.network.send(node_id, value).await
            .map_err(|e| format!("Network error: {}", e))?;

        Ok(())
    }

    /// 从远程节点获取数据
    pub async fn fetch_from_remote(&self, key: &str, node_id: &str) -> Result<(), String> {
        // 请求数据（这里简化处理，实际实现会更复杂）
        let request = format!(r#"{{"action": "get", "key": "{}"}}"#, key);
        self.network.send(node_id, request).await
            .map_err(|e| format!("Network error: {}", e))?;

        // 存储结果（模拟）
        self.storage.set(&format!("cached:{}", key), "fetched_data").await
            .map_err(|e| format!("Storage error: {}", e))?;

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing with Mocks Example ===\n");

    // 测试 1: 成功的同步
    println!("Test 1: Successful sync_to_remote");
    {
        let network = std::sync::Arc::new(MockNetworkService::new());
        let storage = std::sync::Arc::new(MockStorageService::new());
        let service = DataSyncService::new(network.clone(), storage.clone());

        // 预设数据
        storage.seed("user:123", "Alice").await;
        network.preset_send("node-1", Ok(())).await;

        // 执行测试
        service.sync_to_remote("user:123", "node-1").await
            .expect("sync should succeed");

        // 验证
        network.assert_called("send");
        println!("   ✓ Test 1 passed: Data synced successfully");
    }

    // 测试 2: 存储错误
    println!("\nTest 2: Storage error handling");
    {
        let network = std::sync::Arc::new(MockNetworkService::new());
        let storage = std::sync::Arc::new(MockStorageService::new());
        let service = DataSyncService::new(network.clone(), storage.clone());

        // 不预设数据，模拟 key 不存在
        let result = service.sync_to_remote("nonexistent", "node-1").await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
        println!("   ✓ Test 2 passed: Error handled correctly");
    }

    // 测试 3: 网络错误
    println!("\nTest 3: Network error handling");
    {
        let network = std::sync::Arc::new(MockNetworkService::new());
        let storage = std::sync::Arc::new(MockStorageService::new());
        let service = DataSyncService::new(network.clone(), storage.clone());

        // 预设数据但网络失败
        storage.seed("user:456", "Bob").await;
        network.will_fail_next(CisError::network("Connection refused"));

        let result = service.sync_to_remote("user:456", "node-1").await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Network error"));
        println!("   ✓ Test 3 passed: Network error handled correctly");
    }

    // 测试 4: fetch_from_remote
    println!("\nTest 4: fetch_from_remote");
    {
        let network = std::sync::Arc::new(MockNetworkService::new());
        let storage = std::sync::Arc::new(MockStorageService::new());
        let service = DataSyncService::new(network.clone(), storage.clone());

        network.preset_send("node-2", Ok(())).await;

        service.fetch_from_remote("user:789", "node-2").await
            .expect("fetch should succeed");

        // 验证网络发送
        network.assert_called("send");
        println!("   ✓ Test 4 passed: Fetch completed");
    }

    // 测试 5: 调用次数验证
    println!("\nTest 5: Call count verification");
    {
        let network = std::sync::Arc::new(MockNetworkService::new());
        let storage = std::sync::Arc::new(MockStorageService::new());
        let service = DataSyncService::new(network.clone(), storage.clone());

        // 预设多次调用
        storage.seed("key1", "value1").await;
        storage.seed("key2", "value2").await;
        network.preset_send("node-3", Ok(())).await;

        service.sync_to_remote("key1", "node-3").await?;
        service.sync_to_remote("key2", "node-3").await?;

        // 验证调用次数
        network.assert_call_count("send", 2);
        storage.assert_call_count("get", 2);
        println!("   ✓ Test 5 passed: Call counts verified");
    }

    println!("\n=== All tests passed! ===");
    
    Ok(())
}
