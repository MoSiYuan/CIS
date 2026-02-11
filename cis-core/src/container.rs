//! # 依赖注入容器
//!
//! 管理服务依赖的生命周期和注入。
//!
//! ## 使用模式
//!
//! ### 生产环境
//!
//! ```rust,no_run
//! let container = ServiceContainer::production(config).await?;
//! let node_service = NodeService::new(container.network(), container.storage());
//! ```
//!
//! ### 测试环境
//!
//! ```rust
//! let container = ServiceContainer::test()
//!     .with_network(Arc::new(MockNetworkService::new()))
//!     .with_storage(Arc::new(MockStorageService::new()))
//!     .build();
//! ```

use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
use crate::traits::{
    NetworkService, NetworkServiceRef,
    StorageService, StorageServiceRef,
    EventBus, EventBusRef,
    SkillExecutor, SkillExecutorRef,
    EmbeddingServiceTrait, EmbeddingServiceRef,
};

/// 服务容器
///
/// 管理所有服务的生命周期和依赖关系
#[derive(Clone)]
pub struct ServiceContainer {
    config: Arc<Config>,
    network: NetworkServiceRef,
    storage: StorageServiceRef,
    event_bus: EventBusRef,
    skill_executor: SkillExecutorRef,
    embedding: EmbeddingServiceRef,
    ai_provider: Option<Arc<dyn crate::ai::AiProvider>>,
}

impl ServiceContainer {
    /// 创建生产环境容器
    ///
    /// 初始化所有真实服务实现，按照以下依赖顺序：
    /// 
    /// ```text
    /// Config
    /// ├── Storage (SqliteStorage)
    /// ├── EventBus (MemoryEventBus)
    /// ├── Network (P2PNetwork with SecureP2PTransport)
    /// ├── SkillExecutor (SkillExecutorImpl)
    /// ├── Embedding (EmbeddingService)
    /// └── AiProvider (ClaudeProvider or KimiProvider)
    /// ```
    pub async fn production(config: Config) -> Result<Self> {
        use crate::storage::SqliteStorage;
        use crate::event_bus::MemoryEventBus;
        use crate::service::SkillExecutorImpl;
        use crate::skill::SkillManager;
        use crate::storage::db::DbManager;
        
        #[cfg(feature = "p2p")]
        use crate::p2p::P2PNetwork;
        
        tracing::info!("Initializing production service container...");
        
        let config = Arc::new(config);
        
        // 1. 创建存储服务 (SqliteStorage)
        tracing::info!("Initializing SqliteStorage...");
        let storage: StorageServiceRef = Arc::new(SqliteStorage::new()?);
        
        // 2. 创建事件总线 (MemoryEventBus)
        tracing::info!("Initializing MemoryEventBus...");
        let event_bus: EventBusRef = Arc::new(MemoryEventBus::new());
        
        // 3. 创建 P2P 网络服务
        tracing::info!("Initializing P2PNetwork...");
        let network: NetworkServiceRef = Self::create_network_service(&config).await?;
        
        // 4. 创建数据库管理器
        let db_manager = Arc::new(DbManager::new()?);
        
        // 5. 创建 Skill 管理器
        let skill_manager = Arc::new(SkillManager::new(db_manager)?);
        
        // 6. 创建 Skill 执行器
        tracing::info!("Initializing SkillExecutor...");
        let skill_executor: SkillExecutorRef = Arc::new(
            SkillExecutorImpl::new(skill_manager)?
        );
        
        // 7. 创建 Embedding 服务
        // SHAME_TAG NEW-3 REMOVED: Mock degradation eliminated in v1.1.5
        tracing::info!("Initializing EmbeddingService...");
        let embedding: EmbeddingServiceRef = match crate::ai::EmbeddingService::new().await {
            Ok(service) => Arc::new(service),
            Err(e) => {
                return Err(crate::error::CisError::ai(format!(
                    "Failed to initialize embedding service: {}. Vector feature is required for production.", e
                )));
            }
        };
        
        // 8. 创建 AI Provider（可选，根据配置）
        let ai_provider = Self::create_ai_provider(&config).await;
        
        tracing::info!("Production service container initialized successfully");
        
        Ok(Self {
            config,
            network,
            storage,
            event_bus,
            skill_executor,
            embedding,
            ai_provider,
        })
    }
    
    /// 创建网络服务
    ///
    /// 根据 feature 配置创建 P2PNetwork 或 Mock
    #[cfg(feature = "p2p")]
    async fn create_network_service(config: &Arc<Config>) -> Result<NetworkServiceRef> {
        use crate::p2p::P2PNetwork;
        
        let node_id = format!("node-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or(""));
        let did = format!("did:cis:{}", uuid::Uuid::new_v4());
        let listen_addr = format!("{}:{}", config.network.bind_address, config.network.udp_port);
        
        let p2p_config = crate::p2p::P2PConfig {
            node_id: node_id.clone(),
            did: did.clone(),
            listen_addr: listen_addr.clone(),
            port: config.network.udp_port,
            enable_mdns: true,
            metadata: std::collections::HashMap::new(),
            enable_dht: false,
            bootstrap_nodes: vec![],
            enable_nat_traversal: false,
            external_address: None,
            transport_config: crate::p2p::transport_secure::SecureTransportConfig::default(),
            node_keys: None,
        };
        
        let network: NetworkServiceRef = Arc::new(
            P2PNetwork::new(node_id, did, &listen_addr, p2p_config).await?
        );
        
        // 启动网络服务
        network.start().await?;
        tracing::info!("P2PNetwork started on {}", listen_addr);
        
        Ok(network)
    }
    
    /// 创建网络服务（无 p2p feature 时的回退）
    // SHAME_TAG NEW-3 REMOVED: Mock degradation eliminated in v1.1.5
    #[cfg(not(feature = "p2p"))]
    async fn create_network_service(_config: &Arc<Config>) -> Result<NetworkServiceRef> {
        Err(crate::error::CisError::configuration(
            "P2P feature is required for production. Enable 'p2p' feature or provide a custom NetworkService.".to_string()
        ))
    }
    
    /// 创建 AI Provider
    ///
    /// 根据配置创建 Claude 或 Kimi Provider
    async fn create_ai_provider(_config: &Config) -> Option<Arc<dyn crate::ai::AiProvider>> {
        use crate::ai::AiProvider;
        
        // 尝试创建 Claude CLI Provider
        let claude = crate::ai::ClaudeCliProvider::default();
        if claude.available().await {
            tracing::info!("Claude CLI provider is available");
            return Some(Arc::new(claude));
        }
        
        // 尝试创建 Kimi Code Provider
        let kimi = crate::ai::KimiCodeProvider::default();
        if kimi.available().await {
            tracing::info!("Kimi Code provider is available");
            return Some(Arc::new(kimi));
        }
        
        tracing::warn!("No AI provider is available (neither Claude CLI nor Kimi Code found)");
        None
    }

    /// 创建测试环境容器构建器
    ///
    /// 使用 Mock 实现作为默认
    pub fn test() -> TestContainerBuilder {
        TestContainerBuilder::new()
    }

    /// 创建空容器（用于自定义初始化）
    ///
    /// 所有服务必须手动设置
    pub fn empty() -> EmptyContainerBuilder {
        EmptyContainerBuilder::new()
    }
    
    /// 执行健康检查
    ///
    /// 检查所有核心服务的健康状态
    pub async fn health_check(&self) -> HealthCheckResult {
        let mut result = HealthCheckResult::new();
        
        // 检查网络服务
        match self.network.status().await {
            Ok(status) => {
                result.add_check("network", status.running, Some(format!(
                    "Connected peers: {}", status.connected_peers
                )));
            }
            Err(e) => {
                result.add_check("network", false, Some(format!("Error: {}", e)));
            }
        }
        
        // 检查存储服务
        match self.storage.stats().await {
            Ok(stats) => {
                result.add_check("storage", true, Some(format!(
                    "Keys: {}, Size: {} bytes", stats.total_keys, stats.total_size
                )));
            }
            Err(e) => {
                result.add_check("storage", false, Some(format!("Error: {}", e)));
            }
        }
        
        // 检查事件总线
        let subscriber_count = self.event_bus.subscriber_count(None).await;
        result.add_check("event_bus", true, Some(format!(
            "Active subscribers: {}", subscriber_count
        )));
        
        // 检查 Embedding 服务
        match self.embedding.health_check().await {
            Ok(healthy) => {
                result.add_check("embedding", healthy, None);
            }
            Err(e) => {
                result.add_check("embedding", false, Some(format!("Error: {}", e)));
            }
        }
        
        result
    }
    
    /// 优雅关闭
    ///
    /// 按正确顺序关闭所有服务
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down ServiceContainer...");
        
        // 1. 停止网络服务
        tracing::info!("Stopping network service...");
        if let Err(e) = self.network.stop().await {
            tracing::warn!("Error stopping network: {}", e);
        }
        
        tracing::info!("ServiceContainer shutdown completed");
        Ok(())
    }

    // === Getters ===

    /// 获取配置
    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    /// 获取网络服务
    pub fn network(&self) -> NetworkServiceRef {
        Arc::clone(&self.network)
    }

    /// 获取存储服务
    pub fn storage(&self) -> StorageServiceRef {
        Arc::clone(&self.storage)
    }

    /// 获取事件总线
    pub fn event_bus(&self) -> EventBusRef {
        Arc::clone(&self.event_bus)
    }

    /// 获取 Skill 执行器
    pub fn skill_executor(&self) -> SkillExecutorRef {
        Arc::clone(&self.skill_executor)
    }

    /// 获取嵌入服务
    pub fn embedding(&self) -> EmbeddingServiceRef {
        Arc::clone(&self.embedding)
    }
    
    /// 获取 AI Provider（如果可用）
    pub fn ai_provider(&self) -> Option<Arc<dyn crate::ai::AiProvider>> {
        self.ai_provider.as_ref().map(Arc::clone)
    }
}

/// 健康检查结果
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// 各服务健康状态
    pub services: Vec<ServiceHealth>,
    /// 整体健康状态
    pub healthy: bool,
}

/// 服务健康状态
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub name: String,
    pub healthy: bool,
    pub message: Option<String>,
}

impl HealthCheckResult {
    /// 创建新的健康检查结果
    fn new() -> Self {
        Self {
            services: Vec::new(),
            healthy: true,
        }
    }
    
    /// 添加健康检查项
    fn add_check(&mut self, name: &str, healthy: bool, message: Option<String>) {
        if !healthy {
            self.healthy = false;
        }
        self.services.push(ServiceHealth {
            name: name.to_string(),
            healthy,
            message,
        });
    }
}

/// 测试容器构建器
///
/// 用于构建测试环境的服务容器
pub struct TestContainerBuilder {
    config: Option<Arc<Config>>,
    network: Option<NetworkServiceRef>,
    storage: Option<StorageServiceRef>,
    event_bus: Option<EventBusRef>,
    skill_executor: Option<SkillExecutorRef>,
    embedding: Option<EmbeddingServiceRef>,
}

impl TestContainerBuilder {
    /// 创建新的构建器
    fn new() -> Self {
        Self {
            config: None,
            network: None,
            storage: None,
            event_bus: None,
            skill_executor: None,
            embedding: None,
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: Arc<Config>) -> Self {
        self.config = Some(config);
        self
    }

    /// 设置网络服务
    pub fn with_network(mut self, network: NetworkServiceRef) -> Self {
        self.network = Some(network);
        self
    }

    /// 设置存储服务
    pub fn with_storage(mut self, storage: StorageServiceRef) -> Self {
        self.storage = Some(storage);
        self
    }

    /// 设置事件总线
    pub fn with_event_bus(mut self, event_bus: EventBusRef) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// 设置 Skill 执行器
    pub fn with_skill_executor(mut self, executor: SkillExecutorRef) -> Self {
        self.skill_executor = Some(executor);
        self
    }

    /// 设置嵌入服务
    pub fn with_embedding(mut self, embedding: EmbeddingServiceRef) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// 构建容器
    ///
    /// 未设置的服务将使用 Mock 实现（仅在 test 模式下）
    #[cfg(any(test, feature = "test-utils"))]
    pub fn build(self) -> ServiceContainer {
        use crate::test::mocks::{
            MockNetworkService, MockStorageService, MockEventBus,
            MockSkillExecutor, MockEmbeddingService,
        };

        ServiceContainer {
            config: self.config.unwrap_or_else(|| Arc::new(Config::default())),
            network: self.network.unwrap_or_else(|| Arc::new(MockNetworkService::new())),
            storage: self.storage.unwrap_or_else(|| Arc::new(MockStorageService::new())),
            event_bus: self.event_bus.unwrap_or_else(|| Arc::new(MockEventBus::new())),
            skill_executor: self.skill_executor.unwrap_or_else(|| Arc::new(MockSkillExecutor::new())),
            embedding: self.embedding.unwrap_or_else(|| Arc::new(MockEmbeddingService::new())),
            ai_provider: None,
        }
    }

    /// 构建容器（非 test 模式）
    ///
    /// 非 test 模式下必须使用 EmptyContainerBuilder 并手动设置所有服务
    #[cfg(not(any(test, feature = "test-utils")))]
    pub fn build(self) -> ServiceContainer {
        panic!("TestContainerBuilder::build() is only available in test mode. Use EmptyContainerBuilder for production.")
    }
}

/// 空容器构建器
///
/// 用于完全自定义的服务初始化
pub struct EmptyContainerBuilder {
    config: Option<Arc<Config>>,
    network: Option<NetworkServiceRef>,
    storage: Option<StorageServiceRef>,
    event_bus: Option<EventBusRef>,
    skill_executor: Option<SkillExecutorRef>,
    embedding: Option<EmbeddingServiceRef>,
    ai_provider: Option<Arc<dyn crate::ai::AiProvider>>,
}

impl EmptyContainerBuilder {
    /// 创建新的构建器
    fn new() -> Self {
        Self {
            config: None,
            network: None,
            storage: None,
            event_bus: None,
            skill_executor: None,
            embedding: None,
            ai_provider: None,
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: Arc<Config>) -> Self {
        self.config = Some(config);
        self
    }

    /// 设置网络服务
    pub fn with_network(mut self, network: NetworkServiceRef) -> Self {
        self.network = Some(network);
        self
    }

    /// 设置存储服务
    pub fn with_storage(mut self, storage: StorageServiceRef) -> Self {
        self.storage = Some(storage);
        self
    }

    /// 设置事件总线
    pub fn with_event_bus(mut self, event_bus: EventBusRef) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// 设置 Skill 执行器
    pub fn with_skill_executor(mut self, executor: SkillExecutorRef) -> Self {
        self.skill_executor = Some(executor);
        self
    }

    /// 设置嵌入服务
    pub fn with_embedding(mut self, embedding: EmbeddingServiceRef) -> Self {
        self.embedding = Some(embedding);
        self
    }
    
    /// 设置 AI Provider
    pub fn with_ai_provider(mut self, ai_provider: Arc<dyn crate::ai::AiProvider>) -> Self {
        self.ai_provider = Some(ai_provider);
        self
    }

    /// 构建容器
    ///
    /// # Panics
    /// 如果任何必需的服务未设置，将 panic
    pub fn build(self) -> ServiceContainer {
        ServiceContainer {
            config: self.config.expect("Config is required"),
            network: self.network.expect("NetworkService is required"),
            storage: self.storage.expect("StorageService is required"),
            event_bus: self.event_bus.expect("EventBus is required"),
            skill_executor: self.skill_executor.expect("SkillExecutor is required"),
            embedding: self.embedding.expect("EmbeddingService is required"),
            ai_provider: self.ai_provider,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::mocks::{MockNetworkService, MockStorageService, MockEventBus, MockSkillExecutor, MockEmbeddingService};

    #[tokio::test]
    async fn test_test_container_builder() {
        let container = ServiceContainer::test()
            .with_network(Arc::new(MockNetworkService::new()))
            .with_storage(Arc::new(MockStorageService::new()))
            .with_event_bus(Arc::new(MockEventBus::new()))
            .with_skill_executor(Arc::new(MockSkillExecutor::new()))
            .with_embedding(Arc::new(MockEmbeddingService::new()))
            .build();

        // 验证服务可用
        let _network = container.network();
        let _storage = container.storage();
        let _event_bus = container.event_bus();
        let _executor = container.skill_executor();
        let _embedding = container.embedding();
    }

    #[test]
    fn test_empty_container_builder() {
        let container = ServiceContainer::empty()
            .with_config(Arc::new(Config::default()))
            .with_network(Arc::new(MockNetworkService::new()))
            .with_storage(Arc::new(MockStorageService::new()))
            .with_event_bus(Arc::new(MockEventBus::new()))
            .with_skill_executor(Arc::new(MockSkillExecutor::new()))
            .with_embedding(Arc::new(MockEmbeddingService::new()))
            .build();

        assert!(Arc::strong_count(&container.config()) > 0);
    }
    
    #[tokio::test]
    async fn test_health_check() {
        let container = ServiceContainer::test()
            .with_network(Arc::new(MockNetworkService::new()))
            .with_storage(Arc::new(MockStorageService::new()))
            .with_event_bus(Arc::new(MockEventBus::new()))
            .with_skill_executor(Arc::new(MockSkillExecutor::new()))
            .with_embedding(Arc::new(MockEmbeddingService::new()))
            .build();
        
        let health = container.health_check().await;
        
        // 应该有 4 个服务检查
        assert_eq!(health.services.len(), 4);
        
        // 检查每个服务都有状态
        for service in &health.services {
            assert!(!service.name.is_empty());
        }
    }
}
