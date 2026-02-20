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

## Phase 7: 多 Agent 分工架构（Optional，P3）✨ **NEW**

> **整合自**: Kimi 的 
> **GLM 补充**: 审阅问题解答、实施细节
> **定位**: 发挥 CIS 特色（DAG 编排、P2P 跨设备、四级决策）

### 架构定位

CIS v1.2.0 采用**真多 Agent 架构**，与 ZeroClaw 的单 Agent + Delegate Tool 有本质区别：

| 维度 | ZeroClaw | CIS v1.2.0 |
|------|----------|------------|
| **Agent 模式** | 单 Agent + Delegate Tool | 多 Agent 实例常驻 |
| **任务拆分** | Tool 级别委派 | Agent 级别分工 + DAG 编排 |
| **跨设备** | ❌ 不支持 | ✅ P2P 跨设备调用 |
| **记忆隔离** | session_id | Agent 命名空间 + Task ID + Device ID |
| **决策机制** | 无 | 四级决策（Mechanical → Arbitrated）|

### 核心架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                    CIS 多 Agent 生态系统                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Receptionist Agent（前台接待）                           │  │
│  │  ├─ IM 接入（Matrix/Telegram/Discord）                    │  │
│  │  ├─ 任务分类 → 四级决策路由                                │  │
│  │  ├─ 轻量级模型（快速响应）                                │  │
│  │  └─ 记忆命名空间: "receptionist/"                        │  │
│  └────────────────┬─────────────────────────────────────────┘  │
│                   │ 委派任务                                     │
│      ┌────────────┼────────────┬──────────────┬─────────────┐   │
│      ▼            ▼            ▼              ▼             ▼   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   ┌──────────────┐      │
│  │Coder    │ │Doc      │ │Debugger │   │Remote Agent  │      │
│  │Agent    │ │Agent    │ │Agent    │   │(跨设备 P2P)   │      │
│  │         │ │         │ │         │   │              │      │
│  │Claude   │ │OpenCode │ │Kimi     │   │Remote Device │      │
│  │Sonnet   │ │GLM-4    │ │DeepSeek │   │Worker        │      │
│  └─────────┘ └─────────┘ └─────────┘   └──────────────┘      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  CIS 核心服务（特色能力）                                  │  │
│  │  ├─ cis-scheduler: DAG 编排 + 四级决策                     │  │
│  │  ├─ cis-memory:    分组记忆 + 来源追踪                     │  │
│  │  ├─ cis-p2p:       跨设备 Agent 发现/调用                  │  │
│  │  └─ cis-identity:  DID 身份 + 联邦协调                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 子任务 7.1: Receptionist Agent 实现（Week 1-2）

**定位**: 前台接待、IM 交互入口、任务分类与四级决策路由

**核心职责**:
1. **IM 接入**: Matrix, Telegram, Discord, Slack
2. **任务分类**: 使用轻量级 LLM 快速分类
3. **四级决策路由**:
   - Mechanical (自动执行) → 直接委派 Worker Agent
   - Recommended (建议执行) → 倒计时确认后委派
   - Confirmed (需确认) → 人工确认后委派
   - Arbitrated (需仲裁) → 多方投票后委派
4. **快速响应**: Claude Haiku / GPT-4o-mini，延迟 < 2s

**配置示例**:
```toml
[agents.receptionist]
name = "receptionist"
runtime = "claude"
model = "claude-haiku-3.5"
temperature = 0.7
system_prompt = """
You are the receptionist for CIS, a multi-agent system.
Your responsibilities:
1. Greet users and classify their requests
2. Answer simple questions directly
3. Delegate complex tasks to appropriate worker agents
4. Keep responses concise and friendly
"""

[agents.receptionist.memory]
namespace = "receptionist"
categories = ["conversation", "user_preferences"]
max_context_entries = 5
```

### 子任务 7.2: Worker Agents 实现（Week 3-5）

#### Coder Agent
```toml
[agents.coder]
name = "coder"
runtime = "claude"
model = "claude-sonnet-4-20250514"
temperature = 0.2  # 低温度，确定性输出
```

#### Doc Agent
```toml
[agents.doc]
name = "doc"
runtime = "opencode"
model = "glm-4.7-free"
temperature = 0.5
```

#### Debugger Agent
```toml
[agents.debugger]
name = "debugger"
runtime = "kimi"
model = "kimi-latest"
temperature = 0.3
```

### 子任务 7.3: DAG 编排多 Agent 协作（Week 6-7）

**场景**: CI/CD Pipeline

```
[1] 代码审查 (Coder Agent)
      │
      ▼
[2] 运行测试 (Debugger Agent)
      │
      ├─ [2a] 单元测试
      └─ [2b] 集成测试
      │
      ▼
[3] 生成文档 (Doc Agent)
      │
      ▼
[4] 部署 (Remote Agent - 需仲裁)
```

### 子任务 7.4: P2P 跨设备 Agent 调用（Week 8-9）

**核心功能**:
1. 设备发现（通过 mDNS/DHT）
2. 远程 Agent 调用
3. 设备偏好路由（Local / LowLatency / HighPerformance）
4. 记忆跨设备同步

### 子任务 7.5: 记忆分组与幻觉降低（Week 10-11）

**三级记忆隔离**:
- Level 1: Agent 级隔离（receptionist/, coder/, doc/）
- Level 2: Task 级隔离（task_001/, task_002/）
- Level 3: Device 级隔离（device_local/, device_remote_A/）

**降低幻觉的四层过滤**:
1. Layer 1: 相关性过滤（分数 >= 0.7）
2. Layer 2: 不可信记忆过滤（ai_summary_*, assistant_resp_*）
3. Layer 3: 来源验证（必须有 source）
4. Layer 4: 数量限制（最多 5 条）

### 子任务 7.6: 集成测试（Week 12-13）

- [ ] 端到端测试
- [ ] 性能基准测试
- [ ] 安全审计
- [ ] 文档完善

**完整参考**: 详见 [CIS_V1.2.0_MULTI_AGENT_ARCHITECTURE_kimi.md](./CIS_V1.2.0_MULTI_AGENT_ARCHITECTURE_kimi.md)

---


