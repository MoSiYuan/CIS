//! # 服务抽象接口 (Traits)
//!
//! 定义核心服务的抽象接口，用于依赖注入和测试隔离。
//!
//! ## 设计原则
//!
//! - **显式依赖**: 所有服务通过构造函数注入
//! - **可 Mock**: 每个 trait 都有对应的 Mock 实现
//! - **Send + Sync**: 支持并发访问
//! - **异步优先**: 使用 async_trait 支持异步方法
//! - **错误处理**: 每个异步方法返回 Result 类型
//!
//! ## 服务列表
//!
//! | Trait | 用途 | 核心方法 |
//! |-------|------|----------|
//! | [`NetworkService`] | P2P 网络通信 | `send_to`, `broadcast`, `connected_peers` |
//! | [`StorageService`] | 数据持久化 | `get`, `put`, `delete`, `query` |
//! | [`EventBus`] | 事件发布订阅 | `publish`, `subscribe` |
//! | [`SkillExecutor`] | Skill 执行 | `execute`, `list_skills` |
//! | [`AiProvider`] | AI 服务 | `complete`, `embedding` |
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::traits::{NetworkService, StorageService, EventBus};
//! use std::sync::Arc;
//!
//! // 使用 Arc<dyn Trait> 作为依赖注入类型
//! async fn example(
//!     network: Arc<dyn NetworkService>,
//!     storage: Arc<dyn StorageService>,
//! ) -> anyhow::Result<()> {
//!     // 网络操作
//!     network.send_to("peer-1", b"hello").await?;
//!     network.broadcast(b"broadcast message").await?;
//!     
//!     // 存储操作
//!     storage.put("key", b"value").await?;
//!     let value = storage.get("key").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod network;
pub mod storage;
pub mod event_bus;
pub mod skill_executor;
pub mod embedding;
pub mod ai_provider;

// 重新导出主要类型
pub use network::{
    NetworkService, 
    NetworkServiceRef, 
    PeerInfo, 
    NetworkStatus,
    MessagePriority,
    SendOptions,
};

pub use storage::{
    StorageService, 
    StorageServiceRef, 
    StorageQuery, 
    StorageRecord,
    StorageStats,
    QueryOptions,
};

pub use event_bus::{
    EventBus, 
    EventBusRef, 
    EventBusExt,
    Subscription, 
    EventHandler, 
    BoxedEventHandler,
};

pub use skill_executor::{
    SkillExecutor, 
    SkillExecutorRef, 
    ExecutionContext, 
    ExecutionInfo, 
    ExecutionStatus, 
    ExecutionResult, 
    Skill, 
    SkillExecutionConfig, 
    ResourceLimits,
    SkillMetadata,
};

pub use embedding::{
    EmbeddingServiceTrait, 
    EmbeddingServiceRef,
    EmbeddingModelInfo,
};

pub use ai_provider::{
    AiProvider,
    AiProviderRef,
    CompletionRequest,
    CompletionResponse,
    EmbeddingRequest,
    EmbeddingResponse,
    TokenUsage,
    ModelInfo,
};

/// 服务容器 trait - 用于一次性获取所有核心服务
///
/// 这个 trait 提供了一个统一的方式来访问所有核心服务，
/// 便于在服务初始化时进行依赖注入。
///
/// ## 示例
///
/// ```rust,ignore
/// use cis_core::traits::{ServiceContainer, NetworkService, StorageService};
///
/// async fn init_services(container: &dyn ServiceContainer) -> anyhow::Result<()> {
///     let network = container.network_service();
///     let storage = container.storage_service();
///     
///     // 使用服务...
///     network.broadcast(b"Service initialized").await?;///     
///     Ok(())
/// }
/// ```
pub trait ServiceContainer: Send + Sync {
    /// 获取网络服务
    fn network_service(&self) -> NetworkServiceRef;
    
    /// 获取存储服务
    fn storage_service(&self) -> StorageServiceRef;
    
    /// 获取事件总线
    fn event_bus(&self) -> EventBusRef;
    
    /// 获取 Skill 执行器
    fn skill_executor(&self) -> SkillExecutorRef;
    
    /// 获取 AI Provider
    fn ai_provider(&self) -> AiProviderRef;
    
    /// 获取嵌入服务
    fn embedding_service(&self) -> EmbeddingServiceRef;
}

/// 服务容器的 Arc 包装类型
pub type ServiceContainerRef = std::sync::Arc<dyn ServiceContainer>;

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证所有服务引用类型实现了 Send + Sync
    #[test]
    fn test_service_refs_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NetworkServiceRef>();
        assert_send_sync::<StorageServiceRef>();
        assert_send_sync::<EventBusRef>();
        assert_send_sync::<SkillExecutorRef>();
        assert_send_sync::<EmbeddingServiceRef>();
        assert_send_sync::<AiProviderRef>();
        assert_send_sync::<ServiceContainerRef>();
    }
}
