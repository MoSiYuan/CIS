# TASK 1.3: 定义 cis-traits Crate

> **Phase**: 1 - cis-common 基础
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 1-2

---

## 任务概述

创建所有核心 trait 抽象，参考 ZeroClaw 的 trait 设计模式，实现接口与实现的完全分离。

**设计原则**:
- trait 只定义抽象接口
- 实现在各个独立 crate 中
- 所有 trait 都是 `Send + Sync` 的
- 支持 async_trait 异步方法

## 工作内容

### 1. 创建 trait 层次结构

```
cis-common/cis-traits/
├── Cargo.toml
└── src/
    ├── lib.rs              # Re-export all traits
    ├── memory.rs           # NEW - Memory, MemoryVectorIndex, MemorySync
    ├── scheduler.rs        # NEW - DagScheduler, TaskExecutor
    ├── agent.rs            # NEW - Agent, AgentPool
    ├── lifecycle.rs        # NEW - Lifecycle, Named
    ├── storage.rs          # Existing - StorageService
    ├── network.rs          # Existing - NetworkService
    ├── event_bus.rs        # Existing - EventBus
    ├── skill_executor.rs   # Existing - SkillExecutor
    ├── ai_provider.rs      # Existing - AiProvider
    └── embedding.rs        # Existing - EmbeddingService
```

### 2. 定义核心 Traits

#### 2.1 Memory Trait (NEW - 最高优先级)

**File**: `cis-traits/src/memory.rs`

```rust
use cis_types::{MemoryDomain, MemoryCategory, MemoryEntry};
use async_trait::async_trait;

#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;

    // 基础 CRUD
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> anyhow::Result<bool>;

    // 批量查询
    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> anyhow::Result<Vec<String>>;

    // 健康检查
    async fn health_check(&self) -> bool;
}

// 向量索引扩展 trait (模仿 ZeroClaw 的分层设计)
#[async_trait]
pub trait MemoryVectorIndex: Memory {
    async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> anyhow::Result<Vec<SearchResult>>;
    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>) -> anyhow::Result<Vec<HybridSearchResult>>;
}

// 同步扩展 trait (CIS 特色能力)
#[async_trait]
pub trait MemorySync: Memory {
    async fn get_pending_sync(&self, limit: usize) -> anyhow::Result<Vec<SyncMarker>>;
    async fn mark_synced(&self, key: &str, peers: Vec<String>) -> anyhow::Result<()>;
}

// 类型定义
pub struct SearchResult {
    pub key: String,
    pub value: Vec<u8>,
    pub score: f32,
}

pub struct HybridSearchResult {
    pub key: String,
    pub value: Vec<u8>,
    pub vector_score: f32,
    pub keyword_score: f32,
    pub final_score: f32,
}

pub struct SyncMarker {
    pub key: String,
    pub domain: MemoryDomain,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub sync_peers: Vec<String>,
}
```

#### 2.2 Scheduler Trait (NEW)

**File**: `cis-traits/src/scheduler.rs`

```rust
use cis_types::{Task, TaskResult, TaskLevel};
use async_trait::async_trait;

#[async_trait]
pub trait DagScheduler: Send + Sync {
    fn name(&self) -> &str;

    // DAG 构建
    async fn build_dag(&mut self, tasks: Vec<Task>) -> anyhow::Result<Dag>;
    async fn validate_dag(&self, dag: &Dag) -> anyhow::Result<()>;

    // DAG 执行 (支持四级决策: Mechanical → Recommended → Confirmed → Arbitrated)
    async fn execute_dag(&self, dag: Dag) -> anyhow::Result<DagExecutionResult>;
    async fn cancel_execution(&self, execution_id: &str) -> anyhow::Result<bool>;

    // 状态查询
    async fn get_execution_status(&self, execution_id: &str) -> anyhow::Result<ExecutionStatus>;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, task: &Task) -> anyhow::Result<TaskResult>;
    async fn cancel_task(&self, task_id: &str) -> anyhow::Result<bool>;
    fn can_handle(&self, task_type: &TaskType) -> bool;
}

// 类型定义
pub struct Dag {
    pub tasks: Vec<TaskNode>,
    pub edges: Vec<(TaskId, TaskId)>,  // from → to
}

pub struct TaskNode {
    pub id: TaskId,
    pub task: Task,
    pub dependencies: Vec<TaskId>,
}

#[derive(Debug, Clone)]
pub struct DagExecutionResult {
    pub execution_id: String,
    pub results: HashMap<TaskId, TaskResult>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
```

#### 2.3 Agent Trait (NEW)

**File**: `cis-traits/src/agent.rs`

```rust
use async_trait::async_trait;
use cis_types::{AgentRuntime, AgentStatus};

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn runtime_type(&self) -> RuntimeType;
    fn status(&self) -> AgentStatus;

    // 执行回合
    async fn turn(&mut self, user_message: &str) -> anyhow::Result<String>;

    // 能力检测
    async fn can_handle(&self, task_type: &TaskType) -> bool;

    // 配置
    async fn configure(&mut self, config: AgentConfig) -> anyhow::Result<()>;
}

// Agent Pool 管理 (多 Agent 协作)
#[async_trait]
pub trait AgentPool: Send + Sync {
    async fn acquire(&self, agent_type: AgentType) -> anyhow::Result<Box<dyn Agent>>;
    async fn release(&self, agent: Box<dyn Agent>) -> anyhow::Result<()>;

    fn available_types(&self) -> Vec<AgentType>;
    async fn stats(&self) -> AgentPoolStats;
}

// 类型定义
#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Receptionist,
    Coder,
    Doc,
    Debugger,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeType {
    Claude,
    OpenCode,
    Kimi,
    Ollama,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct AgentPoolStats {
    pub total_agents: usize,
    pub available: usize,
    pub busy: usize,
    pub by_type: HashMap<AgentType, usize>,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<usize>,
}
```

#### 2.4 Lifecycle Trait (NEW)

**File**: `cis-traits/src/lifecycle.rs`

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Lifecycle: Send + Sync {
    // 启动服务
    async fn start(&mut self) -> anyhow::Result<()>;

    // 停止服务
    async fn stop(&mut self) -> anyhow::Result<()>;

    // 完全关闭（释放资源）
    async fn shutdown(&mut self) -> anyhow::Result<()>;

    // 状态查询
    fn is_running(&self) -> bool;

    // 健康检查
    async fn health_check(&self) -> HealthStatus;
}

// 命名支持 (所有服务都应该有名字)
pub trait Named {
    fn name(&self) -> &str;
}

// 类型定义
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}
```

### 3. 配置依赖

**File**: `cis-traits/Cargo.toml`

```toml
[package]
name = "cis-traits"
version = "1.2.0"
edition = "2021"

[dependencies]
cis-types = { path = "../cis-types", version = "1.2.0" }
async-trait = "0.1"

# Optional dependencies for implementations
tokio = { version = "1.35", optional = true }
serde = { version = "1.0", optional = true }
chrono = { version = "0.4", optional = true }
```

### 4. Feature Flags

```toml
[features]
default = []
# 基础 traits (始终可用)
core = []

# 可选 trait (用于实现层)
memory = ["core"]
scheduler = ["core"]
agent = ["core"]
lifecycle = ["core"]

# 完整功能
full = ["memory", "scheduler", "agent", "lifecycle"]
```

### 5. Re-export 组织

**File**: `cis-traits/src/lib.rs`

```rust
//! # CIS Traits - Core Abstractions
//!
//! 参考 ZeroClaw 的 trait 设计模式，定义 CIS 核心抽象接口。

pub mod memory;
pub mod scheduler;
pub mod agent;
pub mod lifecycle;
pub mod storage;
pub mod network;
pub mod event_bus;
pub mod skill_executor;
pub mod ai_provider;
pub mod embedding;

// Re-export all core traits
pub use memory::{Memory, MemoryVectorIndex, MemorySync};
pub use scheduler::{DagScheduler, TaskExecutor};
pub use agent::{Agent, AgentPool};
pub use lifecycle::{Lifecycle, Named};

// Re-export existing traits
pub use storage::StorageService;
pub use network::NetworkService;
pub use event_bus::EventBus;
pub use skill_executor::SkillExecutor;
pub use ai_provider::AiProvider;
pub use embedding::EmbeddingService;

// Type re-exports
pub use memory::{
    SearchResult,
    HybridSearchResult,
    SyncMarker
};
pub use scheduler::{
    Dag,
    TaskNode,
    DagExecutionResult,
    ExecutionStatus
};
pub use agent::{
    AgentType,
    RuntimeType,
    AgentPoolStats,
    AgentConfig
};
pub use lifecycle::HealthStatus;
```

## 验收标准

- [ ] 4 个新 trait 定义完整
- [ ] 所有 trait 使用 async_trait
- [ ] trait 方法签名参考 ZeroClaw 设计
- [ ] 类型定义清晰完整
- [ ] 文档注释完整
- [ ] 单元测试框架建立
- [ ] 依赖关系正确配置

## 依赖

- TASK_1_1 (创建 cis-common workspace)
- TASK_1_2 (提取 cis-types)

## 阻塞

- TASK_2_1 (提取 cis-storage - 需要 storage.rs)
- TASK_2_2 (提取 cis-memory - 需要 memory.rs)
- TASK_2_3 (提取 cis-scheduler - 需要 scheduler.rs)

---

**关键设计决策**:
- ✅ Memory trait 分层设计 (基础 → 向量 → 同步)
- ✅ Scheduler 支持四级决策机制
- ✅ Agent trait 支持多 Agent Pool
- ✅ Lifecycle 统一所有服务的生命周期
