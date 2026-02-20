# CIS v1.2.0 执行计划 - 三层架构重构（最终版）

> **版本**: v3.2 Final (整合 Kimi 优化建议)
> **更新日期**: 2026-02-20
> **定位**: CIS 1.2.0 - 独立可用 + 共用模块独立化 + 可选 zeroclaw 集成
> **基于**:
> - 全量CIS模块分析报告（2个探索agents + 1个设计agent）
> - Kimi 优化建议（Builder Pattern, Feature Flags, 类型映射）
>
> **核心改进**:
> - ✅ 吸纳 Kimi 的详细 trait 定义
> - ✅ 添加 Builder Pattern（P2 Optional）
> - ✅ 添加类型映射表（CIS ↔ zeroclaw）
> - ✅ 添加 Feature Flag 精细化设计（发布时优化）
> - ❌ 不采用 Capability Declaration（仅 zeroclaw adapter 层使用）

---

## 执行摘要

### 🎯 核心架构转变

**从**：CIS 贡献模块给 zeroclaw（v2.0 plan）
**到**：**CIS 主项目独立可用，共用模块独立化**（v3.0 plan）

**三层架构** (三明治架构)：
```
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: 可选集成层 (Optional Integration)                 │
│  ├── zeroclaw trait adapters (可选)                         │
│  └── 用户选择的第三方能力                                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: CIS Components (组合层)                           │
│  ├── cis-core (重组件，依赖 cis-common)                     │
│  ├── agent/, ai/, matrix/, skill/ 等 CIS 特有能力          │
│  └── re-export cis-common 模块                             │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: cis-common (独立基础模块)                         │
│  ├── 7个独立 crates (可独立编译，可双向引用)                │
│  ├── cis-types (基础类型，零依赖)                           │
│  ├── cis-traits (trait 抽象)                                │
│  ├── cis-storage (存储层)                                   │
│  ├── cis-memory (记忆系统)                                  │
│  ├── cis-scheduler (DAG 编排)                              │
│  ├── cis-vector (向量搜索)                                  │
│  └── cis-p2p (P2P 网络)                                     │
└─────────────────────────────────────────────────────────────┘
```

### 📊 模块分析总结

**已Trait化的模块** ✅（6个）：
- NetworkService - P2P网络通信
- StorageService - 数据持久化
- EventBus - 事件发布订阅
- SkillExecutor - Skill执行
- AiProvider - AI服务
- EmbeddingService - 向量化服务

**未Trait化的核心模块** ❌（4个）：
1. **Memory** - 记忆系统（最高优先级，唯一缺失的核心模块）
2. **Scheduler** - DAG任务调度（DagScheduler, TaskManager）
3. **Agent** - Agent Pool管理
4. **Lifecycle** - 统一生命周期管理（跨模块需求）

**可独立化的模块** ⭐（5个高独立度模块）：
- cis-memory (⭐⭐⭐⭐⭐) - 完全独立的记忆系统
- cis-scheduler (⭐⭐⭐⭐⭐) - DAG 编排与调度
- cis-storage (⭐⭐⭐⭐) - SQLite 存储抽象
- cis-vector (⭐⭐⭐⭐) - 向量索引与搜索
- cis-p2p (⭐⭐⭐⭐) - P2P 网络层

**关键发现**：
- ✅ CIS 当前有 29 个模块在 cis-core 中，耦合度高
- ✅ 5 个模块可以独立化为 cis-common crates
- ✅ 需要新增 4 个核心 traits (Memory, Scheduler, Agent, Lifecycle)
- ✅ NetworkService 已存在，只需扩展 P2P 功能
- ✅ Identity 模块已实现 DID，不需要重构
- ✅ 类型系统设计良好（TaskLevel, MemoryDomain 等枚举）

---

## 核心原则

### 🎯 **架构定位：CIS 主项目独立可用，共用模块独立化**

**三层架构设计原则**：

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: cis-common (独立基础模块 workspace)              │
│  ├─ 7个独立 crates                                          │
│  ├─ 每个 crate 可独立编译                                   │
│  ├─ 清晰的依赖层级：types ← traits ← storage/memory/...   │
│  └─ 双向引用：CIS 使用 ←→ zeroclaw 可 PR 引用              │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: CIS Components (cis-core 重组件层)               │
│  ├─ 依赖 cis-common 模块                                    │
│  ├─ re-export cis-common（向后兼容）                        │
│  ├─ CIS 特有能力（agent, ai, matrix, skill, ...）          │
│  └─ 不依赖 zeroclaw（核心功能独立可用）                     │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: 可选集成层 (Optional Integration)                 │
│  ├─ zeroclaw trait adapters (feature flag)                 │
│  ├─ 用户可选启用                                            │
│  └─ 不影响 CIS 核心功能                                     │
└─────────────────────────────────────────────────────────────┘
```

### 🚨 **关键约束（CRITICAL）**

**CIS 主项目独立可用**：
- ✅ CIS 必须能**不依赖 zeroclaw** 编译和运行
- ✅ cis-common crates 必须能**独立编译**（零 zeroclaw 依赖）
- ✅ cis-core 只依赖 cis-common，不强制依赖 zeroclaw
- ✅ 用户可以使用 CIS 而不接触 zeroclaw

**共用模块独立化**：
- ✅ 7 个 cis-common crates 是**独立项目**（可独立发布）
- ✅ 每个 crate 有自己的版本号、README、License
- ✅ 依赖关系清晰：cis-types (0 deps) → cis-traits (1 dep) → storage/memory/... (2-3 deps)
- ✅ 任何项目都可以依赖这些 crates（不仅是 CIS）

**双向引用模式**：
- ✅ CIS → cis-common：cis-core 依赖 cis-*/crates
- ✅ zeroclaw ← cis-common：cis-common 可通过 PR 贡献给 zeroclaw
- ✅ CIS ← zeroclaw：CIS 可选集成 zeroclaw（feature flag）

### 🎯 能力边界

**cis-common crates（独立基础模块）**：
- ✅ cis-types - 基础类型（TaskLevel, MemoryDomain, 等）
- ✅ cis-traits - Trait 抽象（Memory, Scheduler, Storage, 等）
- ✅ cis-storage - SQLite 存储抽象
- ✅ cis-memory - 记忆系统（私域/公域 + 向量索引）
- ✅ cis-scheduler - DAG 编排（四级决策 + 联邦协调）
- ✅ cis-vector - 向量搜索（sqlite-vec + hybrid search）
- ✅ cis-p2p - P2P 网络（DID + QUIC + CRDT）

**CIS Components（cis-core 重组件层）**：
- ✅ 依赖 cis-common crates
- ✅ Re-export cis-common 模块（向后兼容）
- ✅ CIS 特有能力（agent/, ai/, matrix/, skill/, identity/, workflow/, 等）
- ✅ 可选集成 zeroclaw（feature: "zeroclaw"）

**Optional zeroclaw Integration**：
- ✅ 22+ AI Providers（可选使用）
- ✅ 13+ Communication Channels（可选使用）
- ✅ 3000+ Skill Ecosystem（可选使用）
- ✅ Agent Loop（可选使用）

**不复刻**：
- ❌ 不复刻 zeroclaw Agent 核心（可选集成）
- ❌ 不复刻 Provider 系统（可选集成）
- ❌ 不复刻 Channel 系统（可选集成）

---

## Implementation Plan

### Phase 0: 研究与分析 ✅ **已完成**

**Deliverables**:
- ✅ `docs/plan/v1.2.0/task/zeroclaw_trait_patterns.md` - zeroclaw设计模式分析
- ✅ CIS模块结构分析报告（2个探索agents）
- ✅ CIS类型系统分析报告
- ✅ CIS trait系统现状分析报告
- ✅ 三层架构设计方案（1个Plan agent）

**关键发现**:
1. **CIS主项目必须独立可用** - 不依赖zeroclaw就能编译运行
2. **共用模块独立化** - 7个crates可独立编译和发布
3. **Memory是唯一缺失的核心trait** - 必须优先处理
4. **Scheduler和Agent需要trait化** - 提升可测试性
5. **Lifecycle统一管理** - 简化服务启动/关闭

---

### Phase 1: 创建 cis-common Workspace（Week 1-2）🔥 **P0**

#### Task 1.1: 创建 cis-common 目录结构

**目标**：建立独立的 cis-common workspace，包含 7 个独立 crates

**目录结构**：
```bash
cis-common/
├── Cargo.toml                    # Workspace root
├── cis-types/                    # 基础类型（零依赖）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── tasks.rs              # TaskLevel, Task, TaskResult
│       ├── memory.rs             # MemoryDomain, MemoryCategory
│       ├── agent.rs              # AgentRuntime, AgentStatus
│       └── mod.rs
├── cis-traits/                   # Trait 定义（仅依赖 types）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── memory.rs             # NEW - Memory trait
│       ├── scheduler.rs          # NEW - Scheduler trait
│       ├── agent.rs              # NEW - Agent trait
│       ├── lifecycle.rs          # NEW - Lifecycle trait
│       ├── storage.rs            # Existing - Storage trait
│       ├── network.rs            # Existing - NetworkService trait
│       └── mod.rs
├── cis-storage/                  # 存储层（依赖 types, traits）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── sqlite.rs             # SQLite backend
│       ├── migrations/           # Database migrations
│       └── mod.rs
├── cis-memory/                   # 记忆系统（依赖 storage, traits, types）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── service.rs            # MemoryService
│       ├── vector.rs             # Vector storage
│       ├── sync.rs               # P2P sync
│       └── mod.rs
├── cis-scheduler/                # DAG 编排（依赖 types, traits）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── dag.rs                # DAG building
│       ├── executor.rs           # Task execution
│       ├── coordinator.rs        # NEW - 联邦协调器
│       └── mod.rs
├── cis-vector/                   # 向量搜索（依赖 types, traits, storage, memory）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── embedding.rs          # Embedding service
│       ├── search.rs             # Hybrid search
│       └── mod.rs
└── cis-p2p/                      # P2P 网络（依赖 types, traits, storage）
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── network.rs            # P2P network
        ├── discovery.rs          # mDNS + DHT discovery
        ├── sync.rs               # CRDT sync
        └── mod.rs
```

**Workspace 配置**：

`cis-common/Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = [
    "cis-types",
    "cis-traits",
    "cis-storage",
    "cis-memory",
    "cis-scheduler",
    "cis-vector",
    "cis-p2p",
]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync", "time"] }
async-trait = "0.1"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Database (for cis-storage)
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono"] }

# Vector search (for cis-vector)
fastembed = { version = "3.0", optional = true }
sqlite-vec = { version = "0.5", optional = true }

# P2P (for cis-p2p)
libp2p = { version = "0.54", optional = true }
prost = { version = "0.12", optional = true }
tonic = { version = "0.11", optional = true }

# Internal dependencies
cis-types = { path = "cis-types", version = "1.2.0" }
cis-traits = { path = "cis-traits", version = "1.2.0" }
cis-storage = { path = "cis-storage", version = "1.2.0" }
```

#### Task 1.2: 提取 cis-types crate

**源文件**: `cis-core/src/types.rs`

**目标**: 将所有基础类型提取到独立 crate

**Files to create**:
- `cis-common/cis-types/Cargo.toml`
- `cis-common/cis-types/src/lib.rs`
- `cis-common/cis-types/src/tasks.rs` (TaskLevel, Task, TaskResult)
- `cis-common/cis-types/src/memory.rs` (MemoryDomain, MemoryCategory, MemoryEntry)
- `cis-common/cis-types/src/agent.rs` (AgentRuntime, AgentStatus, AgentConfig)
- `cis-common/cis-types/src/network.rs` (PeerInfo, NetworkStatus)
- `cis-common/cis-types/src/error.rs` (Error, Result)

**依赖**: 无（零依赖）

#### Task 1.3: 定义 cis-traits crate

**目标**: 创建所有 trait 抽象

**Files to create**:
```
cis-common/cis-traits/src/
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

**Dependencies**:
```toml
[dependencies]
cis-types = { path = "../cis-types", version = "1.2.0" }
async-trait = "0.1"
```

**New traits to define**:

`cis-traits/src/memory.rs`:
```rust
use cis_types::{MemoryDomain, MemoryCategory, MemoryEntry};
use async_trait::async_trait;

#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> anyhow::Result<()>;
    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> anyhow::Result<bool>;
    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> anyhow::Result<Vec<String>>;
    async fn health_check(&self) -> bool;
}

#[async_trait]
pub trait MemoryVectorIndex: Memory {
    async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> anyhow::Result<Vec<SearchResult>>;
    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>) -> anyhow::Result<Vec<HybridSearchResult>>;
}
```

`cis-traits/src/scheduler.rs`:
```rust
use cis_types::{Task, TaskResult, TaskLevel};
use async_trait::async_trait;

#[async_trait]
pub trait DagScheduler: Send + Sync {
    fn name(&self) -> &str;
    async fn build_dag(&mut self, tasks: Vec<Task>) -> anyhow::Result<Dag>;
    async fn execute_dag(&self, dag: Dag) -> anyhow::Result<DagExecutionResult>;
    async fn validate_dag(&self, dag: &Dag) -> anyhow::Result<()>;
    async fn cancel_execution(&self, execution_id: &str) -> anyhow::Result<bool>;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, task: &Task) -> anyhow::Result<TaskResult>;
    async fn cancel_task(&self, task_id: &str) -> anyhow::Result<bool>;
}
```

`cis-traits/src/lifecycle.rs`:
```rust
use async_trait::async_trait;

#[async_trait]
pub trait Lifecycle: Send + Sync {
    async fn start(&mut self) -> anyhow::Result<()>;
    async fn stop(&mut self) -> anyhow::Result<()>;
    async fn shutdown(&mut self) -> anyhow::Result<()>;
    fn is_running(&self) -> bool;
    async fn health_check(&self) -> HealthStatus;
}

pub trait Named {
    fn name(&self) -> &str;
}
```

#### Task 1.4: 更新根 workspace Cargo.toml

**File**: `/Users/jiangxiaolong/work/project/CIS/Cargo.toml`

**Add cis-common to workspace**:
```toml
[workspace]
resolver = "2"
members = [
    "cis-common",         # NEW - cis-common workspace
    "cis-core",
    "cis-node",
    # ... 其他成员
]
```

---

### Phase 2: 提取 Common Modules（Week 3-8）🔧 **P0**

#### Task 2.1: 提取 cis-storage（Week 3）

**源目录**: `cis-core/src/storage/`

**目标**: 提取存储层到独立 crate

**Files to create**:
- `cis-common/cis-storage/Cargo.toml`
- `cis-common/cis-storage/src/lib.rs`
- `cis-common/cis-storage/src/sqlite.rs` (从 cis-core/src/storage/sqlite_storage.rs 提取)
- `cis-common/cis-storage/src/migrations/` (数据库迁移脚本)

**Dependencies**:
```toml
[dependencies]
cis-types = { path = "../cis-types", version = "1.2.0" }
cis-traits = { path = "../cis-traits", version = "1.2.0" }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono"] }
anyhow = "1.0"
```

**Implement StorageService trait**:
```rust
use cis_traits::StorageService;

pub struct SqliteStorage {
    pool: sqlx::SqlitePool,
}

#[async_trait]
impl StorageService for SqliteStorage {
    // 实现现有功能
}
```

#### Task 2.2: 提取 cis-memory（Week 4-5）

**源目录**: `cis-core/src/memory/`

**目标**: 提取记忆系统到独立 crate

**Files to create**:
- `cis-common/cis-memory/Cargo.toml`
- `cis-common/cis-memory/src/lib.rs`
- `cis-common/cis-memory/src/service.rs` (从 cis-core/src/memory/service.rs 提取)
- `cis-common/cis-memory/src/vector.rs` (向量存储)
- `cis-common/cis-memory/src/sync.rs` (P2P 同步)

**Dependencies**:
```toml
[dependencies]
cis-types = { path = "../cis-types", version = "1.2.0" }
cis-traits = { path = "../cis-traits", version = "1.2.0" }
cis-storage = { path = "../cis-storage", version = "1.2.0" }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite"] }
sqlite-vec = { version = "0.5", optional = true }
```

**Implement Memory traits**:
```rust
use cis_traits::{Memory, MemoryVectorIndex, MemorySync};

pub struct CisMemoryService {
    storage: Arc<dyn cis_traits::StorageService>,
    db: Arc<sqlx::SqlitePool>,
}

#[async_trait]
impl Memory for CisMemoryService {
    // 实现基础 CRUD
}

#[async_trait]
impl MemoryVectorIndex for CisMemoryService {
    // 实现向量搜索
}

#[async_trait]
impl MemorySync for CisMemoryService {
    // 实现 P2P 同步
}
```

#### Task 2.3: 提取 cis-scheduler（Week 5-6）

**源目录**: `cis-core/src/scheduler/`

**目标**: 提取 DAG 编排系统到独立 crate

**Files to create**:
- `cis-common/cis-scheduler/Cargo.toml`
- `cis-common/cis-scheduler/src/lib.rs`
- `cis-common/cis-scheduler/src/dag.rs` (从 cis-core/src/scheduler/dag_scheduler.rs 提取)
- `cis-common/cis-scheduler/src/executor.rs` (任务执行器)
- `cis-common/cis-scheduler/src/coordinator.rs` (NEW - 联邦协调器)

**Dependencies**:
```toml
[dependencies]
cis-types = { path = "../cis-types", version = "1.2.0" }
cis-traits = { path = "../cis-traits", version = "1.2.0" }
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync"] }
```

**Implement Scheduler traits**:
```rust
use cis_traits::{DagScheduler, TaskExecutor};

pub struct CisDagScheduler {
    executor: Arc<dyn TaskExecutor>,
}

#[async_trait]
impl DagScheduler for CisDagScheduler {
    // 实现四级决策机制
}

#[async_trait]
impl TaskExecutor for CisTaskExecutor {
    // 实现任务执行（支持 Mechanical → Arbitrated）
}
```

#### Task 2.4: 提取 cis-vector（Week 7）

**源目录**: `cis-core/src/vector/`

**目标**: 提取向量搜索到独立 crate

**Dependencies**: cis-types, cis-traits, cis-storage, cis-memory

#### Task 2.5: 提取 cis-p2p（Week 8）

**源目录**: `cis-core/src/p2p/`

**目标**: 提取 P2P 网络到独立 crate

**Dependencies**: cis-types, cis-traits, cis-storage

---

### Phase 3: 重构 cis-core（Week 9）🎯 **P1**

#### Task 3.1: 更新 cis-core/Cargo.toml

**File**: `cis-core/Cargo.toml`

**添加 cis-common 依赖**:
```toml
[dependencies]
# cis-common workspace dependencies
cis-types = { path = "../cis-common/cis-types", version = "1.2.0" }
cis-traits = { path = "../cis-common/cis-traits", version = "1.2.0" }
cis-storage = { path = "../cis-common/cis-storage", version = "1.2.0" }
cis-memory = { path = "../cis-common/cis-memory", version = "1.2.0" }
cis-scheduler = { path = "../cis-common/cis-scheduler", version = "1.2.0" }

# Optional modules
cis-vector = { path = "../cis-common/cis-vector", version = "1.2.0", optional = true }
cis-p2p = { path = "../cis-common/cis-p2p", version = "1.2.0", optional = true }

# Optional zeroclaw integration
zeroclaw = { git = "https://github.com/zeroclaw-org/zeroclaw", optional = true }

[features]
default = ["encryption", "vector", "p2p", "wasm", "parking_lot"]
vector = ["cis-vector"]
p2p = ["cis-p2p"]
zeroclaw = ["dep:zeroclaw"]  # Optional!
```

#### Task 3.2: 更新 cis-core/src/lib.rs

**File**: `cis-core/src/lib.rs`

**Re-export cis-common 模块**:
```rust
// Re-export cis-common types (backward compatibility)
pub use cis_types::{
    TaskLevel, Task, TaskResult,
    MemoryDomain, MemoryCategory, MemoryEntry,
    AgentRuntime, AgentStatus,
    // ... 其他类型
};

// Re-export cis-common traits
pub use cis_traits::{
    Memory, DagScheduler, TaskExecutor, Lifecycle, Named,
    StorageService, NetworkService, EventBus, AiProvider,
    // ... 其他 traits
};

// CIS-specific modules (remain in cis-core)
pub mod error;
pub mod config;
pub mod sandbox;
pub mod skill;
pub mod ai;
pub mod agent;
pub mod matrix;
pub mod identity;
pub mod workflow;
pub mod security;
pub mod crypto;
// ... 其他 CIS 特有模块
```

#### Task 3.3: 移除已提取的模块

**删除以下目录**（已迁移到 cis-common）:
- `cis-core/src/types.rs` → `cis-common/cis-types/`
- `cis-core/src/traits/` → `cis-common/cis-traits/`
- `cis-core/src/storage/` → `cis-common/cis-storage/`
- `cis-core/src/memory/` → `cis-common/cis-memory/`
- `cis-core/src/scheduler/` → `cis-common/cis-scheduler/`
- `cis-core/src/vector/` → `cis-common/cis-vector/`
- `cis-core/src/p2p/` → `cis-common/cis-p2p/`

#### Task 3.4: 更新依赖模块

**更新所有依赖已提取模块的代码**:
- cis-core/src/agent/
- cis-core/src/skill/
- cis-core/src/ai/
- cis-core/src/workflow/

**修改导入语句**:
```rust
// Before
use crate::memory::MemoryService;
use crate::types::TaskLevel;

// After
use cis_memory::CisMemoryService;
use cis_types::TaskLevel;
```

#### Task 3.5: 测试编译

```bash
# 1. 测试 cis-common workspace
cd cis-common
cargo build --release
cargo test

# 2. 测试 cis-core
cd ../cis-core
cargo build --release
cargo test

# 3. 测试完整 workspace
cd ..
cargo build --release
cargo test
```

---

### Phase 4: zeroclaw 集成（Week 10-11）🌟 **P2 - Optional**

#### Task 4.1: 添加 zeroclaw trait adapters

**Files to create**:
```
cis-core/src/zeroclaw/
├── mod.rs              # Adapters module
├── memory_adapter.rs   # Implement zeroclaw::Memory using cis-memory
├── scheduler_adapter.rs # Implement zeroclaw::Scheduler using cis-scheduler
└── channel_adapter.rs  # Implement zeroclaw::Channel using cis-p2p
```

**memory_adapter.rs**:
```rust
#[cfg(feature = "zeroclaw")]
use async_trait::async_trait;
#[cfg(feature = "zeroclaw")]
use zeroclaw::memory::{Memory as ZeroclawMemory, MemoryEntry, MemoryCategory};

#[cfg(feature = "zeroclaw")]
pub struct ZeroclawMemoryAdapter {
    service: Arc<cis_memory::CisMemoryService>,
}

#[cfg(feature = "zeroclaw")]
#[async_trait]
impl ZeroclawMemory for ZeroclawMemoryAdapter {
    fn name(&self) -> &str { "cis-memory" }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        // 映射 zeroclaw MemoryCategory → CIS MemoryDomain
        let domain = match category {
            MemoryCategory::Core => cis_types::MemoryDomain::Private,
            _ => cis_types::MemoryDomain::Public,
        };

        self.service.set(key, content.as_bytes(), domain, cis_types::MemoryCategory::Context).await
            .map_err(|e| anyhow::anyhow!("CIS memory error: {}", e))
    }

    // ... 其他方法
}
```

#### Task 4.2: 添加 feature flag

**File**: `cis-core/Cargo.toml`

```toml
[features]
default = ["encryption", "vector", "p2p", "wasm", "parking_lot"]
# ... 其他 features

# Optional zeroclaw integration
zeroclaw = [
    "dep:zeroclaw",
    "cis-memory/zeroclaw",
    "cis-scheduler/zeroclaw",
    "cis-p2p/zeroclaw",
]
```

#### Task 4.3: 编写集成测试

**File**: `cis-core/tests/zeroclaw_integration.rs`

```rust
#[cfg(feature = "zeroclaw")]
#[tokio::test]
async fn test_zeroclaw_memory_adapter() {
    use cis_core::zeroclaw::ZeroclawMemoryAdapter;
    use zeroclaw::memory::Memory;

    let cis_memory = cis_memory::CisMemoryService::new("test", "/tmp/test").await.unwrap();
    let adapter = ZeroclawMemoryAdapter::new(cis_memory);

    // Test zeroclaw::Memory trait methods
    adapter.store("key1", "value1", MemoryCategory::Core, None).await.unwrap();
    let results = adapter.recall("value1", 10, None).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "key1");
}
```

#### Task 4.4: 文档

**Files to create**:
- `docs/zeroclaw-integration.md` - zeroclaw 集成指南
- `docs/migration-guide.md` - 从 v1.1.5 迁移到 v1.2.0

---

### Phase 5: 测试和文档（Week 11-12）📝 **P1**

#### Task 5.1: 单元测试

**测试覆盖率目标**: > 80%

```bash
# 测试所有 cis-common crates
cd cis-common
cargo tarpaulin --out Html

# 测试 cis-core
cd ../cis-core
cargo tarpaulin --out Html
```

#### Task 5.2: 集成测试

**File**: `cis-core/tests/integration_full_stack.rs`

```rust
#[tokio::test]
async fn test_full_stack_with_cis_common() {
    // 使用 cis-common crates
    let storage = cis_storage::SqliteStorage::new("sqlite::memory:").await.unwrap();
    let memory = cis_memory::CisMemoryService::new(storage).await.unwrap();
    let scheduler = cis_scheduler::CisDagScheduler::new().await.unwrap();

    // 测试完整流程
    memory.set("test", b"value", cis_types::MemoryDomain::Public, cis_types::MemoryCategory::Context).await.unwrap();

    let results = memory.hybrid_search("test", 10, None, None).await.unwrap();
    assert!(!results.is_empty());
}
```

#### Task 5.3: 性能基准测试

**File**: `cis-core/benches/cis_common_overhead.rs`

- [ ] 测量 cis-common crate 调用开销
- [ ] 对比重构前后性能
- [ ] 优化热点路径（如果开销 > 5%）

**Performance targets**:
- Trait dispatch overhead: < 5%
- Memory operation latency: < 10% increase
- Scheduler build time: < 50ms for 1000 tasks

---

### Phase 6: 发布和 PR（Week 13+）🚀 **P2 - Optional**

#### Task 6.1: 发布 cis-common crates

```bash
# 发布到 crates.io
cd cis-common/cis-types
cargo publish

cd ../cis-traits
cargo publish

# ... 依次发布所有 crates
```

#### Task 6.2: 提交 PR 到 zeroclaw

**PR 1: cis-memory as zeroclaw Memory backend**
- Source: `cis-common/cis-memory/`
- Target: `zeroclaw/crates/memory/`
- Content: 私域/公域 + 向量索引 + 混合搜索

**PR 2: cis-scheduler as zeroclaw Scheduler**
- Source: `cis-common/cis-scheduler/`
- Target: `zeroclaw/crates/scheduler/`
- Content: 四级决策 + 联邦协调

**PR 3: cis-p2p as zeroclaw Channel**
- Source: `cis-common/cis-p2p/`
- Target: `zeroclaw/crates/channels/`
- Content: DID + QUIC + NAT 穿透

#### Task 6.3: 发布 CIS v1.2.0

```bash
git tag cis-v1.2.0
git push origin cis-v1.2.0

# GitHub Release
# - Release notes
# - Migration guide
# - Breaking changes documentation
```

**Deliverables**:
- ✅ `docs/plan/v1.2.0/task/zeroclaw_trait_patterns.md` - zeroclaw设计模式分析
- ✅ CIS模块结构分析报告（3个探索agents）
- ✅ CIS类型系统分析报告
- ✅ CIS trait系统现状分析报告

**关键发现**:
1. **Memory是唯一缺失的核心trait** - 必须优先处理
2. **Scheduler和Agent需要trait化** - 提升可测试性
3. **Lifecycle统一管理** - 简化服务启动/关闭
4. **NetworkService已存在** - 只需扩展P2P功能

---

### Phase 1: 核心 Trait 抽象（Week 1-3）🔥 **P0**

> **设计原则**：
> 1. **CIS 为主**：trait 设计基于 CIS 自身需求
> 2. **zeroclaw 兼容**：考虑 zeroclaw trait 接口，便于实现和贡献
> 3. **独立模块**：每个功能可以作为独立 crate 贡献给 zeroclaw

#### Task 1.1: Memory Trait ⚡ **最高优先级**

**Rationale**:
- MemoryService是唯一没有trait抽象的核心模块
- 被skill, scheduler, agent等多个模块依赖
- 高耦合度（memory→storage直接依赖）
- **可贡献给 zeroclaw**：作为 Memory backend 实现

**设计考虑**：
```rust
// CIS trait 设计（考虑 zeroclaw 兼容）
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;  // zeroclaw 兼容
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> Result<bool>;
    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>>;
    async fn health_check(&self) -> bool;
    async fn stats(&self) -> Result<MemoryStats>;
}

// zeroclaw 兼容层（Task 4.3）
// cis-memory-backend crate 实现 zeroclaw::Memory trait
// 内部委托给 CIS Memory trait
```

**Files to create**:
```
cis-core/src/traits/
├── memory.rs           # NEW - 核心trait定义
└── mod.rs              # MODIFY - 添加memory模块导出
```

**Trait层次结构**:
```rust
// 核心trait - 基础CRUD
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> Result<bool>;
    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>>;
    async fn health_check(&self) -> bool;
    async fn stats(&self) -> Result<MemoryStats>;
}

// 向量索引扩展
#[async_trait]
pub trait MemoryVectorIndex: Memory {
    async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>>;
    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>) -> Result<Vec<HybridSearchResult>>;
    async fn rebuild_index(&self, batch_size: usize) -> Result<usize>;
}

// P2P同步扩展
#[async_trait]
pub trait MemorySync: Memory {
    async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>>;
    async fn mark_synced(&self, key: &str, peer_id: &str) -> Result<()>;
    async fn apply_remote_update(&self, entry: &MemoryEntry, source_peer_id: &str) -> Result<bool>;
}
```

**Backend implementations** (Phase 2):
- `CisMemoryBackend`: Wrapper around existing `MemoryService`
- `MockMemoryBackend`: HashMap-based for tests

---

#### Task 1.2: Scheduler Trait ⚡ **高优先级**

**Rationale**:
- DagScheduler和TaskManager都是具体struct
- 被agent和workflow系统依赖
- 需要支持不同的调度策略

**Files to create**:
```
cis-core/src/traits/
├── scheduler.rs        # NEW - DAG和任务调度trait
└── mod.rs              # MODIFY
```

**Define traits**:
```rust
#[async_trait]
pub trait DagScheduler: Send + Sync {
    async fn build_dag(&mut self, tasks: Vec<Task>) -> Result<Dag>;
    async fn execute_dag(&self, dag: Dag) -> Result<DagExecutionResult>;
    async fn validate_dag(&self, dag: &Dag) -> Result<()>;
    async fn cancel_execution(&self, execution_id: &str) -> Result<bool>;
    async fn get_execution_status(&self, execution_id: &str) -> Result<ExecutionStatus>;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, task: &Task) -> Result<TaskResult>;
    async fn cancel_task(&self, task_id: &str) -> Result<bool>;
    async fn get_task_status(&self, task_id: &str) -> Result<TaskStatus>;
    async fn list_tasks(&self, filter: Option<TaskFilter>) -> Result<Vec<Task>>;
}
```

---

#### Task 1.3: Agent Trait 📊 **中优先级**

**Rationale**:
- Agent Pool管理需要抽象
- 支持不同的runtime（Claude, OpenCode, Kimi）
- 持久化Agent需要trait化

**Files to create**:
```
cis-core/src/traits/
├── agent.rs            # NEW - Agent和Pool管理trait
└── mod.rs              # MODIFY
```

**Define traits**:
```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &str;
    fn runtime(&self) -> AgentRuntime;
    fn status(&self) -> AgentStatus;
    async fn start(&self) -> Result<()>;
    async fn execute(&self, task: TaskRequest) -> Result<TaskResponse>;
    async fn attach(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

#[async_trait]
pub trait AgentPool: Send + Sync {
    type Agent: Agent;
    async fn acquire(&self, config: AgentAcquireConfig) -> Result<Self::Agent>;
    async fn release(&self, agent: Self::Agent);
    async fn stats(&self) -> Result<PoolStats>;
    async fn scale_to(&self, min_size: usize, max_size: usize) -> Result<()>;
}
```

---

#### Task 1.4: Lifecycle Trait 🔄 **跨模块统一**

**Rationale**:
- 统一所有服务的生命周期管理
- 简化服务启动和关闭流程
- 提供统一的健康检查接口

**Files to create**:
```
cis-core/src/traits/
├── lifecycle.rs        # NEW - 统一生命周期trait
└── mod.rs              # MODIFY
```

**Define traits**:
```rust
#[async_trait]
pub trait Lifecycle: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    fn is_running(&self) -> bool;
    async fn health_check(&self) -> HealthStatus;
}

pub trait Named {
    fn name(&self) -> &str;
}

// 为所有现有trait添加Lifecycle继承
// 例如: pub trait NetworkService: Lifecycle + { ... }
```

---

#### Task 1.5: Security & Identity 扩展 🔧 **已有基础**

**Files**:
```
cis-core/src/traits/
├── security.rs         # NEW - 统一安全trait
└── mod.rs              # MODIFY
```

**现状**:
- ✅ NetworkService trait已存在
- ✅ Identity模块已实现DID管理
- ✅ 加密功能已在identity模块实现
- ❌ 缺少统一的Security trait抽象

**新增Security trait**:
```rust
#[async_trait]
pub trait Encryption: Send + Sync {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
    fn algorithm(&self) -> &str;
}

#[async_trait]
pub trait Signature: Send + Sync {
    async fn sign(&self, data: &[u8]) -> Result<crypto::Signature>;
    async fn verify(&self, signature: &crypto::Signature, data: &[u8]) -> Result<bool>;
    fn public_key(&self) -> &[u8];
}
```

---

### Phase 2: Backend 实现（Week 4-5）🔧

#### Task 2.1: Memory Backend 实现

**Files**:
```
cis-core/src/memory/
├── backends/
│   ├── mod.rs          # NEW - backend模块导出
│   ├── cis.rs          # NEW - CisMemoryBackend
│   └── mock.rs         # NEW - MockMemoryBackend
└── service.rs          # MODIFY - 使用trait
```

**CisMemoryBackend实现**:
```rust
pub struct CisMemoryBackend {
    service: Arc<MemoryService>,
    node_id: String,
}

#[async_trait]
impl Memory for CisMemoryBackend {
    fn name(&self) -> &str { "cis-memory" }

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        self.service.set(key, value, domain, category).await
    }

    // ... 其他方法委托给现有MemoryService
}

#[async_trait]
impl MemoryVectorIndex for CisMemoryBackend {
    async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>> {
        // 委托给现有VectorStorage
    }

    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>) -> Result<Vec<HybridSearchResult>> {
        // 使用已实现的hybrid_search操作
    }
}
```

**MockMemoryBackend实现**:
```rust
pub struct MockMemoryBackend {
    data: Arc<Mutex<HashMap<String, MemoryEntry>>>,
}

#[async_trait]
impl Memory for MockMemoryBackend {
    // 简单的HashMap实现，用于测试
}
```

---

#### Task 2.2: Scheduler Backend 实现

**Files**:
```
cis-core/src/scheduler/
├── backends/
│   ├── mod.rs          # NEW
│   ├── cis.rs          # NEW - CisDagScheduler
│   └── mock.rs         # NEW - MockDagScheduler
```

---

#### Task 2.3: Agent Backend 实现

**Files**:
```
cis-core/src/agent/
├── backends/
│   ├── mod.rs          # NEW
│   ├── claude.rs       # NEW - ClaudeAgent
│   ├── opencode.rs     # NEW - OpenCodeAgent
│   └── mock.rs         # NEW - MockAgent
```

---

### Phase 3: 重构现有模块（Week 6-7）🔧

**重要说明**：
- ✅ **不需要向后兼容** - 用户已有本地编译，无升级推送机制
- ✅ **直接重构** - 移除旧API，统一使用trait
- ✅ **一次性迁移** - 所有模块同步切换到trait接口

#### Task 3.1: 重构 MemoryService 使用 Trait

**File**: `cis-core/src/memory/service.rs`

**Before**:
```rust
pub struct MemoryService {
    memory_db: Arc<Mutex<MemoryDb>>,
    vector_storage: Arc<VectorStorage>,
}
```

**After**:
```rust
pub struct MemoryService {
    memory: Box<dyn Memory>,
    vector_index: Box<dyn MemoryVectorIndex>,
    sync: Box<dyn MemorySync>,
}

impl MemoryService {
    pub fn new(
        memory: Box<dyn Memory>,
        vector_index: Box<dyn MemoryVectorIndex>,
        sync: Box<dyn MemorySync>,
    ) -> Result<Self> {
        Ok(Self {
            memory,
            vector_index,
            sync,
        })
    }

    // 工厂方法 - 创建默认CIS实现
    pub fn create_default(node_id: &str, data_dir: &Path) -> Result<Self> {
        let memory = Box::new(CisMemoryBackend::new(node_id, data_dir)?);
        let vector_index = Box::new(CisMemoryBackend::new(node_id, data_dir)?);
        let sync = Box::new(CisMemoryBackend::new(node_id, data_dir)?);

        Self::new(memory, vector_index, sync)
    }
}
```

**Task list**:
- [ ] 直接重构 `MemoryService` 使用trait
- [ ] 移除旧的直接实现
- [ ] 更新所有调用点（编译错误会指引）
- [ ] 更新测试

---

#### Task 3.2: 重构 DagScheduler 使用 Trait

**File**: `cis-core/src/scheduler/dag_scheduler.rs`

- [ ] 重构为使用 `DagScheduler` trait
- [ ] 更新 `TaskManager` 依赖trait
- [ ] 更新测试

---

#### Task 3.3: 重构 Agent Pool 使用 Trait

**File**: `cis-core/src/agent/pool.rs`

- [ ] 重构为使用 `AgentPool` trait
- [ ] 支持runtime动态切换
- [ ] 更新测试

---

### Phase 4: 贡献模块给 zeroclaw（Week 8-10）🌟 **Open Source**

#### Task 4.1: 创建 cis-dag-scheduler crate（贡献给 zeroclaw）

**目标**：将 CIS 独有的 DAG 编排系统作为独立 crate，通过 PR 贡献给 zeroclaw

**Project structure**:
```bash
# 在 CIS monorepo 中
cis-core/src/scheduler/
├── lib.rs              # NEW - 独立的 DAG scheduler 库
├── dag.rs              # 现有代码重构
├── coordinator.rs      # NEW - 联邦协调器
└── zeroclaw_compat.rs  # NEW - 实现 zeroclaw::Scheduler trait

# 作为独立 crate 发布（可选）
cis-dag-scheduler/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    └── scheduler.rs
```

**实现 zeroclaw trait**:
```rust
// cis-dag-scheduler/src/zeroclaw_compat.rs
use async_trait::async_trait;
use zeroclaw::scheduler::{Scheduler, Task, TaskResult};

pub struct CisDagScheduler {
    coordinator: Arc<FederationCoordinator>,
    config: SchedulerConfig,
}

#[async_trait]
impl Scheduler for CisDagScheduler {
    fn name(&self) -> &str { "cis-federal-dag" }

    async fn schedule(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // 1. 本地 DAG 编排
        let dag = self.build_dag(tasks).await?;

        // 2. 联邦协调（跨节点任务分配）
        let execution = self.coordinator.coordinate(dag).await?;

        // 3. 四级决策执行
        self.execute_with_levels(execution).await
    }

    async fn cancel(&self, task_id: &str) -> Result<bool> {
        self.coordinator.cancel_task(task_id).await
    }
}
```

**Pull Request 内容**:
- ✅ 四级决策机制（Mechanical → Arbitrated）
- ✅ 联邦 DAG 协调器
- ✅ CRDT 冲突解决
- ✅ Merkle DAG 版本控制

---

#### Task 4.2: 创建 cis-p2p-transport crate（贡献给 zeroclaw）

**目标**：将 CIS 的 P2P 传输层作为 zeroclaw 的 Channel 实现

**Project structure**:
```bash
cis-p2p-transport/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── transport.rs        # QUIC + DID 传输
    ├── channel_adapter.rs  # 实现 zeroclaw::Channel trait
    └── discovery.rs        # DID 节点发现
```

**实现 zeroclaw trait**:
```rust
// cis-p2p-transport/src/channel_adapter.rs
use async_trait::async_trait;
use zeroclaw::channels::{Channel, ChannelMessage, SendMessage};

pub struct CisP2PChannel {
    p2p: Arc<P2PNetwork>,
    identity: Arc<DidIdentity>,
}

#[async_trait]
impl Channel for CisP2PChannel {
    fn name(&self) -> &str { "cis-p2p" }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        // 通过 DID + P2P 发送消息
        let target_did = Did::parse(&message.recipient)?;
        let payload = serde_json::to_vec(&message)?;

        self.p2p.send_to_did(target_did, &payload).await
            .map_err(|e| anyhow::anyhow!("P2P send failed: {}", e))
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        // 监听 P2P 消息并转换为 ChannelMessage
        let mut p2p_rx = self.p2p.subscribe().await?;
        tokio::spawn(async move {
            while let Some(msg) = p2p_rx.recv().await {
                let channel_msg = ChannelMessage {
                    id: msg.id,
                    sender: msg.sender.did().to_string(),
                    reply_target: msg.reply_target,
                    content: msg.content,
                    channel: "cis-p2p".to_string(),
                    timestamp: msg.timestamp,
                    thread_ts: None,
                };
                tx.send(channel_msg).await.ok();
            }
        });
        Ok(())
    }
}
```

**Pull Request 内容**:
- ✅ DID 身份验证
- ✅ QUIC 传输 + NAT 穿透
- ✅ P2P 联邦网络
- ✅ 实现 zeroclaw::Channel trait

---

#### Task 4.3: 创建 cis-memory-backend crate（贡献给 zeroclaw）

**目标**：将 CIS 的 Memory 作为 zeroclaw 的 Memory backend

**Project structure**:
```bash
cis-memory-backend/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── memory.rs           # CIS Memory 实现
    ├── vector.rs           # sqlite-vec 向量索引
    └── zeroclaw_compat.rs  # 实现 zeroclaw::Memory trait
```

**实现 zeroclaw trait**:
```rust
// cis-memory-backend/src/zeroclaw_compat.rs
use async_trait::async_trait;
use zeroclaw::memory::{Memory, MemoryEntry, MemoryCategory};

pub struct CisMemoryBackend {
    service: Arc<cis_core::memory::MemoryService>,
    node_id: String,
}

#[async_trait]
impl Memory for CisMemoryBackend {
    fn name(&self) -> &str { "cis-memory" }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        // 映射 zeroclaw MemoryCategory → CIS MemoryDomain
        let domain = match category {
            MemoryCategory::Core => MemoryDomain::Private,
            MemoryCategory::Daily => MemoryDomain::Public,
            MemoryCategory::Conversation => MemoryDomain::Public,
            MemoryCategory::Custom(_) => MemoryDomain::Public,
        };

        self.service.set(key, content.as_bytes(), domain, MemoryCategory::Context).await
            .map_err(|e| anyhow::anyhow!("CIS memory error: {}", e))
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // 使用 CIS 混合搜索（向量 + FTS5）
        let results = self.service.hybrid_search(query, limit, None, None).await
            .map_err(|e| anyhow::anyhow!("CIS search error: {}", e))?;

        Ok(results.into_iter().map(|r| MemoryEntry {
            id: r.key.clone(),
            key: r.key,
            content: String::from_utf8_lossy(&r.value).to_string(),
            category: MemoryCategory::Core, // 简化映射
            timestamp: Utc::now().to_rfc3339(),
            session_id: session_id.map(|s| s.to_string()),
            score: Some(r.final_score as f64),
        }).collect())
    }

    // ... 其他方法
}
```

**Pull Request 内容**:
- ✅ 私域/公域分离
- ✅ sqlite-vec 向量索引
- ✅ 混合搜索（向量 + FTS5）
- ✅ 54周归档
- ✅ 实现 zeroclaw::Memory trait

---

#### Task 4.4: 在 CIS 中集成 zeroclaw 能力

**File**: `cis-core/Cargo.toml`

```toml
[dependencies]
# 可选依赖：集成 zeroclaw 能力
zeroclaw = { git = "https://github.com/zeroclaw-labs/zeroclaw", version = "0.1", optional = true }

[features]
default = []
zeroclaw-integration = ["zeroclaw"]  # 用户可选启用
```

**使用示例**:
```rust
// cis-core/src/ai/mod.rs
#[cfg(feature = "zeroclaw-integration")]
use zeroclaw::providers::{Provider, OpenAiProvider};

pub fn get_provider(config: &AiConfig) -> Arc<dyn AiProvider> {
    #[cfg(feature = "zeroclaw-integration")]
    {
        if config.use_zeroclaw {
            return Arc::new(ZeroclawProviderAdapter::new(config));
        }
    }

    // 使用 CIS 原有实现
    Arc::new(CisAiProvider::new(config))
}
```

---

### Phase 5: 测试和文档（Week 10-11）📝

#### Task 5.1: 单元测试

**Files**:
```
cis-core/src/traits/tests/
├── memory_tests.rs     # NEW
├── scheduler_tests.rs  # NEW
├── agent_tests.rs      # NEW
└── lifecycle_tests.rs  # NEW
```

**测试覆盖率目标**: > 80%

---

#### Task 5.2: 集成测试

**File**: `cis-core/tests/integration_traits.rs` - NEW

```rust
#[tokio::test]
async fn test_full_stack_with_traits() {
    let memory = Box::new(CisMemoryBackend::new("test-node")?);
    let scheduler = Box::new(CisDagScheduler::new()?);

    let agent = Agent::builder()
        .memory(memory)
        .scheduler(scheduler)
        .build()?;

    agent.run().await?;
}
```

---

#### Task 5.3: 文档

**Files to create**:
- `docs/traits-guide.md` - Trait使用指南
- `docs/traits-architecture.md` - Trait架构设计
- `docs/migration-guide.md` - 迁移指南

---

### Phase 6: 性能优化（Week 12）⚡

#### Task 6.1: 性能基准测试

**File**: `cis-core/benches/trait_overhead.rs` - NEW

- [ ] 测量trait object dispatch开销
- [ ] 对比重构前后性能
- [ ] 优化热点路径（如果开销 > 5%）

---

## Critical Files Summary

### Phase 1: Create cis-common Workspace (NEW)

**Workspace configuration**:
- [`cis-common/Cargo.toml`](cis-common/Cargo.toml) - Workspace root with 7 members
- [`Cargo.toml`](Cargo.toml) - Root workspace (add cis-common member)

**cis-types crate**:
- `cis-common/cis-types/Cargo.toml` - Zero dependencies
- `cis-common/cis-types/src/lib.rs` - Re-export all types
- `cis-common/cis-types/src/tasks.rs` - TaskLevel, Task, TaskResult
- `cis-common/cis-types/src/memory.rs` - MemoryDomain, MemoryCategory, MemoryEntry

**cis-traits crate**:
- `cis-common/cis-traits/Cargo.toml` - Depends only on cis-types
- `cis-common/cis-traits/src/lib.rs` - Re-export all traits
- `cis-common/cis-traits/src/memory.rs` - **NEW** - Memory trait
- `cis-common/cis-traits/src/scheduler.rs` - **NEW** - Scheduler trait
- `cis-common/cis-traits/src/agent.rs` - **NEW** - Agent trait
- `cis-common/cis-traits/src/lifecycle.rs` - **NEW** - Lifecycle trait

### Phase 2: Extract Common Modules

**cis-storage** (Week 3):
- `cis-common/cis-storage/Cargo.toml` - Depends on cis-types, cis-traits
- `cis-common/cis-storage/src/lib.rs` - Re-export storage services
- `cis-common/cis-storage/src/sqlite.rs` - From cis-core/src/storage/sqlite_storage.rs

**cis-memory** (Week 4-5):
- `cis-common/cis-memory/Cargo.toml` - Depends on cis-storage, cis-traits
- `cis-common/cis-memory/src/lib.rs` - Re-export memory services
- `cis-common/cis-memory/src/service.rs` - From cis-core/src/memory/service.rs
- `cis-common/cis-memory/src/vector.rs` - Vector storage implementation

**cis-scheduler** (Week 5-6):
- `cis-common/cis-scheduler/Cargo.toml` - Depends on cis-types, cis-traits
- `cis-common/cis-scheduler/src/lib.rs` - Re-export scheduler services
- `cis-common/cis-scheduler/src/dag.rs` - From cis-core/src/scheduler/dag_scheduler.rs
- `cis-common/cis-scheduler/src/coordinator.rs` - **NEW** - Federation coordinator

### Phase 3: Refactor cis-core

**cis-core configuration**:
- [`cis-core/Cargo.toml`](cis-core/Cargo.toml) - Add cis-common dependencies
- [`cis-core/src/lib.rs`](cis-core/src/lib.rs) - Re-export cis-common modules

**Remove from cis-core** (migrated to cis-common):
- `cis-core/src/types.rs` → Delete (replaced by cis-types)
- `cis-core/src/traits/` → Delete (replaced by cis-traits)
- `cis-core/src/storage/` → Delete (replaced by cis-storage)
- `cis-core/src/memory/` → Delete (replaced by cis-memory)
- `cis-core/src/scheduler/` → Delete (replaced by cis-scheduler)

**Update dependencies**:
- `cis-core/src/agent/` - Update imports to use cis-memory, cis-scheduler
- `cis-core/src/skill/` - Update imports to use cis-memory
- `cis-core/src/ai/` - Update imports to use cis-memory, cis-scheduler
- `cis-core/src/workflow/` - Update imports to use cis-scheduler

### Phase 4: zeroclaw Integration (OPTIONAL)

**Adapters** (feature-gated):
- `cis-core/src/zeroclaw/mod.rs` - Adapters module
- `cis-core/src/zeroclaw/memory_adapter.rs` - Implement zeroclaw::Memory
- `cis-core/src/zeroclaw/scheduler_adapter.rs` - Implement zeroclaw::Scheduler
- `cis-core/src/zeroclaw/channel_adapter.rs` - Implement zeroclaw::Channel

**Tests**:
- `cis-core/tests/zeroclaw_integration.rs` - Integration tests

---

## Verification Checklist

### Phase 1 Verification: cis-common Workspace
- [ ] cis-common workspace 创建成功
- [ ] cis-types crate 独立编译通过（零依赖）
- [ ] cis-traits crate 编译通过（仅依赖 cis-types）
- [ ] **NEW traits 定义完成**：
  - [ ] Memory trait (基础 CRUD + 向量索引 + P2P 同步)
  - [ ] DagScheduler trait (DAG 编排)
  - [ ] TaskExecutor trait (四级决策执行)
  - [ ] Agent trait (Agent 生命周期)
  - [ ] Lifecycle trait (统一生命周期管理)
- [ ] 根 workspace Cargo.toml 包含 cis-common
- [ ] 所有 crate 有独立版本号（1.2.0）

### Phase 2 Verification: Extract Common Modules
- [ ] cis-storage 提取完成，独立编译通过
- [ ] cis-memory 提取完成，实现 Memory traits
- [ ] cis-scheduler 提取完成，实现 Scheduler traits
- [ ] cis-vector 提取完成（可选）
- [ ] cis-p2p 提取完成（可选）
- [ ] 每个 crate 依赖层级 < 5
- [ ] 无循环依赖
- [ ] 单元测试覆盖率 > 70%

### Phase 3 Verification: Refactor cis-core
- [ ] cis-core 依赖 cis-common crates
- [ ] cis-core/src/lib.rs re-export cis-common 模块
- [ ] 已提取的模块从 cis-core 删除
- [ ] 所有依赖模块更新导入语句
- [ ] `cargo build --release` 编译成功（无 warnings）
- [ ] `cargo test` 全部通过
- [ ] 性能回归 < 5%

### Phase 4 Verification: zeroclaw Integration (OPTIONAL)
- [ ] zeroclaw 适配器创建成功（feature-gated）
- [ ] Memory adapter 实现 zeroclaw::Memory
- [ ] Scheduler adapter 实现 zeroclaw::Scheduler
- [ ] Channel adapter 实现 zeroclaw::Channel
- [ ] `cargo build --features zeroclaw` 编译成功
- [ ] 集成测试通过
- [ ] CIS 在**不启用 zeroclaw feature 时**正常工作

### Phase 5 Verification: Testing & Documentation
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试通过
- [ ] 性能基准测试完成
- [ ] Trait 开销 < 5%
- [ ] 文档完整：
  - [ ] cis-common README（每个 crate）
  - [ ] API 文档（rustdoc）
  - [ ] Migration guide（v1.1.5 → v1.2.0）
  - [ ] Architecture diagram
  - [ ] zeroclaw integration guide（可选）

### Phase 6 Verification: Release & PR (OPTIONAL)
- [ ] cis-common crates 发布到 crates.io
- [ ] CIS v1.2.0 发布
- [ ] PR 到 zeroclaw 提交（可选）：
  - [ ] cis-memory PR
  - [ ] cis-scheduler PR
  - [ ] cis-p2p PR

---

## Success Criteria

### Technical Metrics
| Metric | Before | After | Verification |
|--------|--------|-------|-------------|
| **独立 crates** | 0 | 7 | `ls cis-common/` |
| **cis-core modules** | 29 | 22 | `ls cis-core/src/` |
| **Test coverage** | 65% | 80% | `cargo tarpaulin` |
| **Trait count** | 6 | 10 | `ls cis-common/cis-traits/src/*.rs` |
| **CIS independent** | ❌ | ✅ | `cargo build --no-default-features` |
| **cis-common independent** | N/A | ✅ | `cd cis-common && cargo build` |
| **Trait overhead** | N/A | < 5% | Benchmark report |
| **Compilation time** | 60s | 65s | `cargo build --release` |

### Functional Capabilities
| Feature | Status | Location |
|---------|--------|----------|
| **cis-types crate** | ✅ New | cis-common/cis-types/ |
| **cis-traits crate** | ✅ New | cis-common/cis-traits/ |
| **cis-storage crate** | ✅ Extracted | cis-common/cis-storage/ |
| **cis-memory crate** | ✅ Extracted | cis-common/cis-memory/ |
| **cis-scheduler crate** | ✅ Extracted | cis-common/cis-scheduler/ |
| **cis-vector crate** | ✅ Extracted | cis-common/cis-vector/ |
| **cis-p2p crate** | ✅ Extracted | cis-common/cis-p2p/ |
| **Memory trait** | ✅ New | cis-traits/src/memory.rs |
| **Scheduler trait** | ✅ New | cis-traits/src/scheduler.rs |
| **Agent trait** | ✅ New | cis-traits/src/agent.rs |
| **Lifecycle trait** | ✅ New | cis-traits/src/lifecycle.rs |
| **zeroclaw adapters** | ✅ New (optional) | cis-core/src/zeroclaw/ |

### Architecture Quality
- ✅ **三层架构清晰**：cis-common → cis-core → optional integration
- ✅ **独立编译**：每个 cis-common crate 可独立编译
- ✅ **CIS 独立可用**：不依赖 zeroclaw
- ✅ **双向引用**：CIS 使用 cis-common，zeroclaw 可 PR 引用
- ✅ **依赖层级清晰**：types (0 deps) ← traits (1 dep) ← storage/memory/... (2-3 deps)
- ✅ **可测试性提升**：Mock 实现独立于 CIS

### Code Quality
- ✅ 统一的 trait 抽象层
- ✅ 低耦合度（trait 依赖）
- ✅ 高内聚（模块独立）
- ✅ 可选集成 zeroclaw（feature flag）
- ✅ 向后兼容（re-export cis-common）

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| **cis-common 提取复杂度高** | 分阶段提取：Week 1-2 创建 workspace，Week 3-8 逐个提取模块，每个模块独立验证 |
| **破坏现有 API** | ✅ **不需要向后兼容** - 用户已有本地编译，直接重构；re-export cis-common 保持部分兼容 |
| **编译错误大量** | ✅ **编译错误即指引** - `cargo build` 会告诉我们所有需要更新的调用点；批量修复 |
| **循环依赖** | 严格的依赖层级：types (0 deps) ← traits (1 dep) ← storage/memory/... (2-3 deps)；使用 `cargo machete` 检测 |
| **zeroclaw 依赖风险** | ✅ **完全可选** - feature flag 控制；CIS 独立可用；不影响核心功能 |
| **编译时间增加** | 每个独立 crate 编译更快；整体编译时间可能略增，但增量编译更快 |
| **维护负担增加** | 7 个独立 crates 但职责清晰；版本号独立管理；可单独发布和升级 |
| **PR 到 zeroclaw 被拒绝** | 不影响 CIS 使用；cis-common crates 独立存在；可作为替代方案提供给社区 |

---

## Timeline Summary

| Phase | Duration | Focus | Deliverables |
|-------|----------|-------|--------------|
| **Phase 0** | ✅ Completed | 模块分析 | 2 探索 agents + 1 设计 agent |
| **Phase 1** | Week 1-2 | 创建 cis-common | 7 个独立 crates，定义 traits |
| **Phase 2** | Week 3-8 | 提取 common modules | storage, memory, scheduler, vector, p2p |
| **Phase 3** | Week 9 | 重构 cis-core | 移除已提取模块，更新依赖 |
| **Phase 4** | Week 10-11 | zeroclaw 集成（可选） | Adapters, feature flags |
| **Phase 5** | Week 11-12 | 测试和文档 | 单元测试 >80%，集成测试，文档 |
| **Phase 6** | Week 13+ | 发布和 PR（可选） | 发布 crates.io，PR 到 zeroclaw |

**Total**: 12-13 weeks（3个月）

**Milestones**:
- **Week 2**: cis-common workspace 创建完成，所有 traits 定义完成
- **Week 8**: 所有 5 个 common modules 提取完成
- **Week 9**: cis-core 重构完成，CIS 独立可用
- **Week 11**: 所有测试通过，文档完整
- **Week 13**: CIS v1.2.0 发布（可选：PR 到 zeroclaw）

---

## Next Actions

### Immediate (This Week)

1. **创建 cis-common 目录结构**
   ```bash
   mkdir -p cis-common/{cis-types,cis-traits,cis-storage,cis-memory,cis-scheduler,cis-vector,cis-p2p}
   ```

2. **创建 cis-common/Cargo.toml**
   - 定义 workspace with 7 members
   - 统一依赖版本管理

3. **提取 cis-types crate**
   - 从 `cis-core/src/types.rs` 提取所有基础类型
   - 确保**零依赖**
   - 验证独立编译

4. **定义 cis-traits crate**
   - 创建 `memory.rs`（Memory, MemoryVectorIndex, MemorySync）
   - 创建 `scheduler.rs`（DagScheduler, TaskExecutor）
   - 创建 `lifecycle.rs`（Lifecycle, Named）
   - 创建 `agent.rs`（Agent, AgentPool）

5. **验证**
   ```bash
   cd cis-common
   cargo build --release
   cargo test
   ```

### This Month (Month 1)

- ✅ Week 1-2: 创建 cis-common workspace
- ✅ Week 3: 提取 cis-storage
- ✅ Week 4-5: 提取 cis-memory

### This Quarter (Quarter 1)

- ✅ Week 1-2: cis-common workspace
- ✅ Week 3-8: 提取 5 个 common modules
- ✅ Week 9: 重构 cis-core
- ✅ Week 10-12: 测试、文档、发布

---

## Architecture Diagrams

### Before (v1.1.5)

```
┌─────────────────────────────────────────────────────────────┐
│  CIS Monorepo                                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  cis-core/                                                 │
│  ├── src/                                                  │
│  │   ├── types.rs           (所有类型)                    │
│  │   ├── traits/            (6 traits)                    │
│  │   ├── storage/           (存储层)                       │
│  │   ├── memory/            (记忆系统)                     │
│  │   ├── scheduler/         (DAG 编排)                    │
│  │   ├── vector/            (向量搜索)                     │
│  │   ├── p2p/               (P2P 网络)                     │
│  │   ├── agent/             (Agent 管理)                  │
│  │   ├── ai/                (AI 服务)                     │
│  │   ├── skill/             (Skill 执行)                  │
│  │   └── ... (22 more modules)                           │
│  └── Cargo.toml              (170+ dependencies)           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
    ❌ 高耦合度 (29 modules in cis-core)
    ❌ 无法独立编译
    ❌ 无法被其他项目引用
```

### After (v1.2.0)

```
┌─────────────────────────────────────────────────────────────┐
│  CIS Monorepo                                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  cis-common/ ✨ NEW (独立 workspace)                        │
│  ├── cis-types/           (基础类型，零依赖)               │
│  ├── cis-traits/          (trait 抽象)                     │
│  ├── cis-storage/         (存储层)                         │
│  ├── cis-memory/          (记忆系统)                       │
│  ├── cis-scheduler/       (DAG 编排)                       │
│  ├── cis-vector/          (向量搜索)                       │
│  └── cis-p2p/             (P2P 网络)                       │
│                                                             │
│  cis-core/ ✨ REFACTORED (重组件层)                         │
│  ├── src/                                                  │
│  │   ├── lib.rs             (re-export cis-common)        │
│  │   ├── agent/             (Agent 管理 - CIS 特有)       │
│  │   ├── ai/                (AI 服务 - CIS 特有)          │
│  │   ├── skill/             (Skill 执行 - CIS 特有)       │
│  │   ├── matrix/            (Matrix 联邦 - CIS 特有)      │
│  │   ├── identity/          (DID 身份 - CIS 特有)         │
│  │   ├── workflow/          (Workflow - CIS 特有)         │
│  │   └── ... (CIS-specific modules only)                 │
│  └── Cargo.toml              (依赖 cis-common)             │
│                                                             │
│  Optional Integration Layer                                 │
│  ├── cis-core/src/zeroclaw/ (可选 - feature: "zeroclaw")  │
│  └── adapters for zeroclaw traits                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
    ✅ 三层架构清晰
    ✅ cis-common 可独立编译
    ✅ cis-core 依赖 cis-common
    ✅ 可选集成 zeroclaw
    ✅ 双向引用（CIS ←→ zeroclaw）
```

### Dependency Graph

```
                cis-types (0 dependencies)
                     ↓
                cis-traits (1 dependency)
                     ↓
        ┌────────────┼────────────┐
        ↓            ↓            ↓
   cis-storage   cis-memory   cis-scheduler
        ↓            ↓            ↓
     cis-vector    cis-p2p
        ↓            ↓
        └──────┬─────┘
               ↓
          cis-core
               ↓
        (optional zeroclaw)
```

---

## 优化设计（采纳 Kimi 建议）

### Builder Pattern 设计 🏗️ **P2 - Optional**

**优先级**: P2（Optional，锦上添花）
**目标**: 提升复杂结构体的 API 可用性

**适用场景**:
- ✅ 复杂对象构造（> 5 个字段）
- ✅ 有可选字段的对象
- ✅ 需要验证逻辑的对象
- ❌ 简单数据结构（< 5 个字段）

**TaskBuilder 实现**:

```rust
// cis-common/cis-types/src/builder.rs
use crate::{Task, TaskLevel, TaskPriority};
use serde::Serialize;

pub struct TaskBuilder {
    id: String,
    title: String,
    description: Option<String>,
    group_name: String,
    level: TaskLevel,
    priority: TaskPriority,
    dependencies: Vec<String>,
    skill_id: Option<String>,
    skill_params: Option<serde_json::Value>,
}

impl TaskBuilder {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            group_name: "default".to_string(),
            level: TaskLevel::Mechanical { retry: 3 },
            priority: TaskPriority::default(),
            dependencies: Vec::new(),
            skill_id: None,
            skill_params: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_level(mut self, level: TaskLevel) -> Self {
        self.level = level;
        self
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_skill(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }

    pub fn build(self) -> Task {
        Task {
            id: self.id,
            title: self.title,
            description: self.description,
            group_name: self.group_name,
            level: self.level,
            priority: self.priority,
            dependencies: self.dependencies,
            skill_id: self.skill_id,
            skill_params: self.skill_params,
            ..Default::default()
        }
    }
}
```

**使用示例**:
```rust
let task = TaskBuilder::new("task-1", "Deploy service")
    .with_level(TaskLevel::Mechanical { retry: 3 })
    .with_priority(TaskPriority::High)
    .with_dependencies(vec!["setup".to_string()])
    .build()?;
```

**需要 Builder 的结构体**:
- `Task` ✅（复杂，> 5 字段）
- `MemoryEntry` ✅（有可选字段）
- `AgentConfig` ✅（复杂配置）
- ❌ `TaskLevel`（简单枚举）
- ❌ `MemoryDomain`（简单枚举）

---

### 类型映射表（CIS ↔ zeroclaw）📋

**目标**: 为 zeroclaw 集成提供完整的类型映射参考

#### Memory 类型映射

| CIS Type | ZeroClaw Type | 映射说明 | 代码示例 |
|----------|---------------|----------|----------|
| `MemoryDomain::Private` | `MemoryCategory::Core` | 私域 → Core | `MemoryCategory::Core` |
| `MemoryDomain::Public` | `MemoryCategory::Context` | 公域 → Context | `MemoryCategory::Context` |
| `MemoryCategory::Context` | `MemoryCategory::Context` | 直接映射 | - |
| `MemoryCategory::Skill` | `MemoryCategory::Tool` | Skill → Tool | `MemoryCategory::Tool` |
| `MemoryCategory::Result` | `MemoryCategory::Result` | 直接映射 | - |
| `MemoryCategory::Error` | `MemoryCategory::Error` | 直接映射 | - |
| `MemoryCategory::Execution` | `MemoryCategory::Action` | Execution → Action | `MemoryCategory::Action` |

**实现代码**:
```rust
// cis-core/src/zeroclaw/memory_adapter.rs
impl From<cis_types::MemoryDomain> for zeroclaw::memory::MemoryCategory {
    fn from(domain: cis_types::MemoryDomain) -> Self {
        match domain {
            cis_types::MemoryDomain::Private => Self::Core,
            cis_types::MemoryDomain::Public => Self::Context,
        }
    }
}

impl From<zeroclaw::memory::MemoryCategory> for cis_types::MemoryDomain {
    fn from(category: zeroclaw::memory::MemoryCategory) -> Self {
        match category {
            zeroclaw::memory::MemoryCategory::Core => Self::Private,
            _ => Self::Public,
        }
    }
}
```

#### Task 类型映射

| CIS Type | ZeroClaw Type | 映射说明 |
|----------|---------------|----------|
| `TaskLevel::Mechanical` | `ExecutionMode::Auto` | 自动执行 |
| `TaskLevel::Recommended` | `ExecutionMode::Suggest` | 建议模式 |
| `TaskLevel::Confirmed` | `ExecutionMode::Confirm` | 确认模式 |
| `TaskLevel::Arbitrated` | `ExecutionMode::Arbitrate` | 仲裁模式 |
| `TaskStatus::Pending` | `TaskState::Pending` | 直接映射 |
| `TaskStatus::Running` | `TaskState::Running` | 直接映射 |
| `TaskStatus::Completed` | `TaskState::Completed` | 直接映射 |
| `TaskStatus::Failed` | `TaskState::Failed` | 直接映射 |

#### Agent 类型映射

| CIS Type | ZeroClaw Type | 映射说明 |
|----------|---------------|----------|
| `AgentType::Cli` | `AgentKind::Cli` | 直接映射 |
| `AgentType::Web` | `AgentKind::Web` | 直接映射 |
| `AgentType::Embedded` | `AgentKind::Embedded` | 直接映射 |
| `AgentType::Remote` | `AgentKind::Remote` | 直接映射 |

---

### Feature Flag 精细化设计（可选）🔧 **P3 - Release-time Optimization**

**优先级**: P3（发布到 crates.io 时优化）
**当前**: 使用基础 feature flags
**目标**: 精细化控制，减少编译时间

**当前设计**（简单，够用）:
```toml
[features]
default = ["encryption", "vector", "p2p", "wasm", "parking_lot"]
vector = ["fastembed", "sqlite-vec"]
p2p = ["prost", "tonic", "encryption", "quinn"]
zeroclaw = ["dep:zeroclaw"]
```

**精细化设计**（发布时优化）:

```toml
# cis-common/cis-types/Cargo.toml
[features]
default = ["std", "serde", "chrono"]
std = []
serde = ["dep:serde", "dep:serde_json"]
chrono = ["dep:chrono"]

# cis-common/cis-traits/Cargo.toml
[features]
default = ["std", "async"]
std = ["cis-types/std"]
async = ["dep:async-trait", "dep:tokio", "cis-types/serde"]
memory = []
scheduler = []
agent = []

# cis-common/cis-memory/Cargo.toml
[features]
default = ["std", "async", "storage"]
std = ["cis-types/std", "cis-traits/std"]
async = ["cis-traits/async"]
storage = ["dep:cis-storage"]
vector = ["async", "dep:fastembed", "dep:sqlite-vec"]
sync = ["async", "dep:cis-p2p"]
encryption = ["dep:ring"]
zeroclaw = ["dep:zeroclaw", "storage", "vector"]
```

**何时启用**:
- ✅ 发布到 crates.io 之前
- ✅ 用户反馈编译时间过长时
- ❌ 初期开发阶段（使用简单版本即可）

---

### Default Implementation 规范 📝

**目标**: 为 trait 方法提供合理的默认实现，减少 boilerplate

**✅ 应该提供默认实现的方法**:
```rust
// 1. 健康检查 - 默认返回 true
async fn health_check(&self) -> bool { true }

// 2. 统计信息 - 默认实现（遍历 keys）
async fn count(&self) -> anyhow::Result<usize> {
    let keys = self.list_keys(None, None, None).await?;
    Ok(keys.len())
}

// 3. 列表操作 - 默认返回空列表
async fn list_running(&self) -> anyhow::Result<Vec<ExecutionSummary>> {
    Ok(Vec::new())
}

// 4. 暂停/恢复 - 默认不支持
async fn pause_execution(&self, _id: &str) -> anyhow::Result<bool> {
    Ok(false)
}

// 5. 权限检查 - 默认实现（四级决策）
async fn check_permission(&self, task: &Task) -> anyhow::Result<PermissionResult> {
    Ok(match &task.level {
        TaskLevel::Mechanical { .. } => PermissionResult::AutoApprove,
        TaskLevel::Recommended { default_action, timeout_secs } => {
            PermissionResult::Countdown {
                seconds: *timeout_secs,
                default_action: *default_action,
            }
        }
        TaskLevel::Confirmed => PermissionResult::NeedsConfirmation,
        TaskLevel::Arbitrated { stakeholders } => {
            PermissionResult::NeedsArbitration {
                stakeholders: stakeholders.clone(),
            }
        }
    })
}
```

**❌ 不应该提供默认实现的方法**:
```rust
// 核心功能 - 必须由实现者提供
async fn store(&self, key: &str, value: &[u8], ...) -> anyhow::Result<()>;
async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>>;
async fn execute_task(&self, task: &Task) -> anyhow::Result<TaskResult>;
```

**❌ 不合理的默认实现**（Kimi plan 中的问题）:
```rust
// ❌ O(n) 遍历所有 keys - 性能差
async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let keys = self.list_keys(None, None, None).await?;
    let mut results = Vec::new();
    for key in keys {
        if key.contains(query) {  // O(n) 字符串匹配
            if let Some(entry) = self.get(&key).await? {
                results.push(SearchResult { ... });
            }
        }
    }
    Ok(results)
}
```

**建议**: 如果 backend 不支持 search，应该**不实现** `MemoryVectorIndex` trait，而不是提供 O(n) 的默认实现。

---

## Key Benefits

### For CIS Project

1. **模块独立性** - 7 个独立 crates，每个可独立编译、测试、发布
2. **CIS 独立可用** - 不依赖 zeroclaw，完全独立运行
3. **更清晰的架构** - 三层架构（基础 → 组合 → 集成）
4. **更好的可测试性** - 每个 crate 有独立的测试套件
5. **更快的编译** - 独立 crate 增量编译更快
6. **版本管理灵活** - 每个 crate 可独立升级

### For zeroclaw Project

1. **即用型模块** - cis-memory, cis-scheduler, cis-p2p 可直接 PR
2. **生产就绪** - 这些模块已在 CIS 中验证和使用
3. **清晰抽象** - 基于 trait，易于集成
4. **可选依赖** - zeroclaw 可选择是否采纳

### For Users

1. **更灵活的集成** - 用户可以选择只使用部分 cis-common crates
2. **更小的依赖** - 例如，只需要记忆系统，只需依赖 cis-memory
3. **更好的性能** - 独立 crates 可以针对性优化
4. **社区共享** - CIS 和 zeroclaw 社区都可以受益

---

## 📝 To Kimi: 下一步补全指南

### 任务说明

请基于本 plan（v3.2 Final），补充以下**实施细节**，使 plan 可直接执行。

### 补全要求

#### 1. 代码示例完整化 ⭐⭐⭐⭐⭐

**当前状态**: Plan 中有基础代码示例，但不够完整

**需要补充**:

**Task 1.3 - cis-traits crate 定义**（优先级最高）

请补充完整的 trait 定义，包括：

```rust
// cis-traits/src/memory.rs
// 请补充完整的 trait 定义，包括：
// 1. Memory trait（基础 CRUD）
// 2. MemoryVectorIndex trait（向量搜索）
// 3. MemorySync trait（P2P 同步）
// 4. 相关类型定义（SearchResult, HybridSearchResult, SyncMarker, SyncResult, SyncStatus）
```

**参考要求**:
- ✅ 使用 `#[async_trait]`
- ✅ 返回类型：`anyhow::Result<T>`（而非 `Result<T, E>`）
- ✅ 包含合理的 Default Implementation（参考上文 "Default Implementation 规范"）
- ❌ 不要添加 Capability Declaration（不需要运行时能力检测）
- ✅ 方法签名要与现有 CIS 代码兼容

**Task 1.3 - cis-traits crate 定义**（Scheduler）

请补充完整的 Scheduler trait 定义：

```rust
// cis-traits/src/scheduler.rs
// 请补充完整的 trait 定义，包括：
// 1. DagScheduler trait（DAG 编排）
// 2. TaskExecutor trait（任务执行）
// 3. 相关类型定义（Dag, DagNode, DagEdge, DagExecutionResult, ExecutionStatus, ValidationResult, PermissionResult）
// 4. Default Implementation（参考上文）
```

**参考要求**:
- ✅ `check_permission` 方法提供四级决策默认实现
- ✅ `list_running` 提供默认实现（返回空 Vec）
- ✅ `pause_execution` / `resume_execution` 提供默认实现（返回 false）

**Task 1.3 - cis-traits crate 定义**（Agent & Lifecycle）

请补充完整的 Agent & Lifecycle trait 定义：

```rust
// cis-traits/src/agent.rs
// 请补充完整的 Agent 和 AgentPool trait 定义

// cis-traits/src/lifecycle.rs
// 请补充完整的 Lifecycle, Named, Versioned trait 定义
```

#### 2. 文件清单完整化 ⭐⭐⭐⭐

**当前状态**: Plan 中有部分文件清单，但不够详细

**需要补充**:

**Phase 1: 创建 cis-common Workspace**

请补充详细的**文件创建清单**：

```markdown
#### Task 1.2: 提取 cis-types crate

**Files to create**:
- `cis-common/cis-types/Cargo.toml`
  ```toml
  # 请补充完整的 Cargo.toml 内容
  ```

- `cis-common/cis-types/src/lib.rs`
  ```rust
  // 请补充完整的 lib.rs 内容（re-export）
  ```

- `cis-common/cis-types/src/tasks.rs`
  ```rust
  // 请补充完整的 tasks.rs 内容
  // 包括：TaskLevel, Task, TaskResult, TaskStatus, Action, FailureType, AmbiguityPolicy
  ```

- `cis-common/cis-types/src/memory.rs`
  ```rust
  // 请补充完整的 memory.rs 内容
  // 包括：MemoryDomain, MemoryCategory, MemoryEntry, MemoryStats
  ```

- `cis-common/cis-types/src/agent.rs`
  ```rust
  // 请补充完整的 agent.rs 内容
  // 包括：AgentRuntime, AgentStatus, AgentConfig
  ```

- `cis-common/cis-types/src/error.rs`
  ```rust
  // 请补充完整的 error.rs 内容
  // 包括：Error, Result
  ```

**依赖**: 无（零依赖）
```

**Phase 2: 提取 Common Modules**

请为以下每个 task 补充详细的文件清单：

- Task 2.1: 提取 cis-storage（Week 3）
- Task 2.2: 提取 cis-memory（Week 4-5）
- Task 2.3: 提取 cis-scheduler（Week 5-6）
- Task 2.4: 提取 cis-vector（Week 7）
- Task 2.5: 提取 cis-p2p（Week 8）

每个 task 应包括：
```
**Files to create**:
- `path/to/file`
  ```rust
  // 完整的文件内容（关键部分）
  ```
- `path/to/another_file`
  ```toml
  # 完整的文件内容（关键部分）
  ```

**Dependencies**:
```toml
[dependencies]
# 详细的依赖列表
```

**Implementation**:
```rust
// 关键实现代码示例
```
```

#### 3. 实现代码示例完整化 ⭐⭐⭐⭐⭐

**当前状态**: Plan 中有简略的实现示例，但不够详细

**需要补充**:

**Task 2.2: 提取 cis-memory**

请补充完整的 `CisMemoryService` 实现：

```rust
// cis-common/cis-memory/src/service.rs
// 请补充完整的实现，包括：
// 1. CisMemoryService 结构体定义
// 2. Memory trait 实现
// 3. MemoryVectorIndex trait 实现
// 4. MemorySync trait 实现
// 5. 构造函数、工厂方法
// 6. 错误处理（使用 anyhow::Context 添加上下文）
```

**参考要求**:
- ✅ 完整实现所有 trait 方法
- ✅ 使用 `anyhow::Context` 添加错误上下文：
  ```rust
  self.service.set(key, value, domain, category).await
      .with_context(|| format!("Failed to set memory entry: key={}", key))?;
  ```
- ✅ 包含构造函数和工厂方法
- ✅ 包含单元测试示例

**Task 2.3: 提取 cis-scheduler**

请补充完整的 `CisDagScheduler` 实现：

```rust
// cis-common/cis-scheduler/src/dag.rs
// 请补充完整的实现，包括：
// 1. CisDagScheduler 结构体定义
// 2. DagScheduler trait 实现
// 3. DAG 构建逻辑（拓扑排序、循环检测）
// 4. 四级决策执行（Mechanical → Arbitrated）
```

**Task 3.1: 更新 cis-core/Cargo.toml**

请补充完整的 `cis-core/Cargo.toml` 配置：

```toml
# cis-core/Cargo.toml
# 请补充完整的 Cargo.toml 内容，包括：
# 1. [dependencies] - 所有 cis-common 依赖
# 2. [features] - 所有 feature flags
# 3. zeroclaw 可选依赖
```

**Task 3.2: 更新 cis-core/src/lib.rs**

请补充完整的 `cis-core/src/lib.rs`：

```rust
// cis-core/src/lib.rs
// 请补充完整的 lib.rs 内容，包括：
// 1. Re-export cis-common types
// 2. Re-export cis-common traits
// 3. Re-export cis-common builders（如果实现）
// 4. CIS-specific modules（保持不变）
```

#### 4. 测试代码完整化 ⭐⭐⭐

**当前状态**: Plan 中有测试示例，但不够完整

**需要补充**:

**Task 5.1: 单元测试**

请补充完整的单元测试示例：

```rust
// cis-common/cis-memory/src/tests/memory_tests.rs
// 请补充完整的单元测试，包括：
// 1. set/get/delete 测试
// 2. list_keys 测试
// 3. hybrid_search 测试
// 4. sync 测试
// 5. 错误处理测试
```

**Task 5.2: 集成测试**

请补充完整的集成测试：

```rust
// cis-core/tests/integration_full_stack.rs
// 请补充完整的集成测试，包括：
// 1. 创建 cis-common 实例
// 2. 测试完整 workflow（storage → memory → scheduler）
// 3. 测试 zeroclaw adapter（可选）
```

#### 5. 文档完整化 ⭐⭐⭐

**需要补充**:

**Task 5.4: 文档**

请补充以下文档的**完整大纲**和**关键内容**：

1. `cis-common/README.md` - cis-common workspace 说明
2. `cis-common/cis-types/README.md` - 基础类型说明
3. `cis-common/cis-traits/README.md` - Trait 使用指南
4. `cis-common/cis-memory/README.md` - Memory trait 实现指南
5. `cis-common/cis-scheduler/README.md` - Scheduler trait 使用指南
6. `docs/migration-guide.md` - 从 v1.1.5 迁移到 v1.2.0
7. `docs/architecture-v1.2.0.md` - 三层架构文档
8. `docs/zeroclaw-integration.md` - zeroclaw 集成指南（可选）

每个文档应包括：
```markdown
# 文档标题

## 概述
[简要说明]

## 核心概念
[关键概念说明]

## 使用示例
```rust
// 代码示例
```

## API 参考
[关键 API 说明]

## 注意事项
[重要提醒]
```

### 补全规范

#### 代码示例规范

✅ **应该**:
- 使用完整的 Rust 语法（包括 use 语句、完整类型）
- 包含错误处理（`anyhow::Result<T>`）
- 添加必要的注释
- 使用实际的 CIS 类型名称
- 代码可以直接编译（尽可能）

❌ **不应该**:
- 使用 `// ... 省略` 代替关键代码
- 使用伪代码
- 包含 TODO 或占位符

#### 文件清单规范

✅ **应该**:
- 列出所有需要创建/修改的文件
- 提供完整的文件路径
- 包含文件内容的**关键部分**
- 对于配置文件（如 Cargo.toml），提供完整内容

❌ **不应该**:
- 只列出文件名，没有内容
- 只说"创建文件"，没有说明创建什么

#### 测试代码规范

✅ **应该**:
- 测试覆盖核心功能
- 包含正常流程和错误流程
- 使用 `#[tokio::test]`（async tests）
- 包含断言（`assert!`, `assert_eq!`）

❌ **不应该**:
- 只写空测试函数
- 测试覆盖不完整

### 补全输出格式

请按照以下格式输出补全内容：

```markdown
## 补全：[Task 名称]

### 文件清单

**Files to create**:
- `path/to/file1`
  ```rust
  // 完整的文件内容
  ```
- `path/to/file2`
  ```toml
  # 完整的文件内容
  ```

### 实现代码

**结构体定义**:
```rust
// 完整的结构体定义
```

**Trait 实现**:
```rust
// 完整的 trait 实现
```

### 测试代码

```rust
// 完整的测试代码
```

### 注意事项

- [ ] 重要提醒 1
- [ ] 重要提醒 2
```

### 优先级

请按以下优先级补全：

**P0（最高优先级）**:
1. Task 1.3 - cis-traits crate 定义（memory, scheduler, agent, lifecycle）
2. Task 2.2 - cis-memory 实现代码
3. Task 2.3 - cis-scheduler 实现代码
4. Task 3.1-3.2 - cis-core 重构（Cargo.toml, lib.rs）

**P1（高优先级）**:
5. Task 1.2 - cis-types crate 文件清单
6. Task 2.1 - cis-storage 文件清单和实现
7. Task 5.1-5.2 - 测试代码

**P2（中优先级）**:
8. Task 2.4-2.5 - cis-vector, cis-p2p 文件清单
9. Task 4.1-4.3 - zeroclaw 集成代码
10. Task 5.4 - 文档大纲

### 审查标准

补全内容将按以下标准审查：

| 维度 | 标准 | 权重 |
|------|------|------|
| **完整性** | 是否覆盖所有要求的文件和代码 | 40% |
| **可执行性** | 代码是否可以直接使用（或接近直接使用） | 30% |
| **正确性** | 代码是否符合 Rust 语法和最佳实践 | 20% |
| **规范性** | 是否符合本文档的补全规范 | 10% |

### 常见问题

**Q1: Capability Declaration 是否需要实现？**

A: **不需要**。Capability Declaration（运行时能力检测）仅用于 zeroclaw adapter 层，cis-common crates 不需要。每个 crate 的能力是**编译时确定**的。

**Q2: Builder Pattern 是否必须实现？**

A: **不是必须的**。Builder Pattern 是 P2 Optional，可以后续添加。当前应聚焦核心功能（trait 定义、模块提取）。

**Q3: Feature Flag 是否需要精细化？**

A: **初期不需要**。当前使用基础 feature flags 即可。精细化设计（P3）在发布到 crates.io 之前优化即可。

**Q4: 代码示例需要多详细？**

A: 应该**尽可能完整**。关键代码（trait 定义、结构体实现）应该可以直接编译。配置文件（Cargo.toml）应该完整。

**Q5: 如何处理现有代码的迁移？**

A: Plan 中已说明（Phase 3），**不需要向后兼容**。用户已有本地编译，直接重构即可。re-export cis-common 保持部分兼容。

### 下一步

1. **阅读本 plan**（v3.2 Final）的所有内容
2. **按优先级**补全内容（P0 → P1 → P2）
3. **输出格式**遵循"补全输出格式"
4. **代码示例**尽可能完整和可直接使用
5. **如有疑问**，参考本 plan 的"常见问题"或咨询

---

**祝补全顺利！** 🚀
