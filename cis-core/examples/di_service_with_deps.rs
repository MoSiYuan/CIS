//! # 带依赖的服务示例
//!
//! 本示例展示如何编写依赖注入风格的服务。
//!
//! ## 运行方式
//!
//! ```bash
//! cargo run --example dependency_injection/service_with_deps --features test-utils
//! ```

use cis_core::container::ServiceContainer;
use cis_core::test::mocks::{
    MockNetworkService, MockStorageService, MockEventBus,
    MockSkillExecutor, MockEmbeddingService,
};
use cis_core::traits::{NetworkServiceRef, StorageServiceRef};
use std::sync::Arc;

/// 节点同步服务 - 展示依赖注入模式
///
/// 所有依赖通过构造函数显式注入，易于测试和替换
pub struct NodeSyncService {
    network: NetworkServiceRef,
    storage: StorageServiceRef,
}

impl NodeSyncService {
    /// 创建新的节点同步服务
    ///
    /// # Arguments
    /// * `network` - 网络服务
    /// * `storage` - 存储服务
    pub fn new(network: NetworkServiceRef, storage: StorageServiceRef) -> Self {
        Self { network, storage }
    }

    /// 从容器创建服务
    ///
    /// 这是便利构造函数，实际依赖仍来自容器
    pub fn from_container(container: &ServiceContainer) -> Self {
        Self::new(
            container.network(),
            container.storage(),
        )
    }

    /// 广播状态到所有节点
    pub async fn broadcast_status(&self) -> Result<(), Box<dyn std::error::Error>> {
        let status = b"{\"status\": \"online\"}";
        self.network.broadcast(status).await?;
        Ok(())
    }

    /// 同步节点数据
    pub async fn sync_node(&self, node_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 获取节点信息
        let peer = self.network.get_peer(node_id).await;
        
        if let Some(peer) = peer {
            // 存储节点信息
            let data = format!("{{\"node_id\": \"{}\", \"address\": \"{}\"}}", 
                peer.node_id, peer.address);
            self.storage.put(&format!("node:{}", node_id), data.as_bytes()).await?;
        }
        
        Ok(())
    }

    /// 获取网络状态摘要
    pub async fn get_network_summary(&self) -> Result<NetworkSummary, Box<dyn std::error::Error>> {
        let status = self.network.status().await;
        
        Ok(NetworkSummary {
            node_id: status.node_id,
            connected_peers: status.connected_peers,
            discovered_peers: status.discovered_peers,
            uptime_secs: status.uptime_secs,
        })
    }
}

/// 网络状态摘要
#[derive(Debug)]
pub struct NetworkSummary {
    pub node_id: String,
    pub connected_peers: usize,
    pub discovered_peers: usize,
    pub uptime_secs: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Service with Dependencies Example ===\n");

    // 1. 创建容器
    println!("1. Creating service container...");
    
    let mock_network = Arc::new(MockNetworkService::new());
    let mock_storage = Arc::new(MockStorageService::new());
    let mock_event_bus = Arc::new(MockEventBus::new());
    let mock_executor = Arc::new(MockSkillExecutor::new());
    let mock_embedding = Arc::new(MockEmbeddingService::new());

    let container = ServiceContainer::test()
        .with_network(mock_network.clone())
        .with_storage(mock_storage.clone())
        .with_event_bus(mock_event_bus)
        .with_skill_executor(mock_executor)
        .with_embedding(mock_embedding)
        .build();

    println!("   ✓ Container created\n");

    // 2. 创建带依赖的服务
    println!("2. Creating NodeSyncService with injected dependencies...");
    
    let sync_service = NodeSyncService::from_container(&container);
    println!("   ✓ NodeSyncService created\n");

    // 3. 预设置 Mock 行为
    println!("3. Setting up mock behaviors...");
    
    // 预设网络行为
    mock_network.preset_connect("ws://localhost:8080", Ok(())).await;
    mock_network.connect("ws://localhost:8080").await?;
    println!("   ✓ Network preset configured");

    // 预设存储行为
    mock_storage.seed("node:test-node", r#"{"node_id": "test-node", "address": "ws://localhost:8080"}"#).await;
    println!("   ✓ Storage preset configured\n");

    // 4. 使用服务
    println!("4. Using NodeSyncService...");
    
    // 广播状态
    sync_service.broadcast_status().await?;
    println!("   ✓ Status broadcasted");
    mock_network.assert_called("broadcast");
    println!("   ✓ Network broadcast verified");

    // 获取网络摘要
    let summary = sync_service.get_network_summary().await?;
    println!("   ✓ Network summary: {:?}", summary);

    println!();

    // 5. 直接注入依赖（另一种方式）
    println!("5. Creating service with direct dependency injection...");
    
    let direct_service = NodeSyncService::new(
        mock_network.clone(),
        mock_storage.clone(),
    );
    
    let summary2 = direct_service.get_network_summary().await?;
    println!("   ✓ Direct injection works: node_id={}", summary2.node_id);

    println!("\n=== Example completed successfully! ===");
    
    Ok(())
}
