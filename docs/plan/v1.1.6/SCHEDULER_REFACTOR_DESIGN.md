# CIS Scheduler 模块拆分设计

> **设计日期**: 2026-02-12
> **目标**: 拆分3439行的巨型scheduler模块为职责清晰的子模块
> **原则**: 单一职责、模块化、可测试

---

## 1. 当前问题分析

### 1.1 文件规模问题

| 文件 | 行数 | 问题描述 |
|------|------|----------|
| `mod.rs` | 394 | 巨型文件，难以维护 |
| `event_driven.rs` | ~800 | 事件驱动调度器 |
| `local_executor.rs` | ~600 | 本地执行器 |
| `multi_agent_executor.rs` | ~700 | 多agent执行器 |
| `persistence.rs` | ~500 | 持久化 |
| `notify.rs` | ~400 | 通知系统 |
| `todo_monitor.rs` | ~300 | TODO监控（简化版） |

**总计**: ~4400行（含注释和空白行）

### 1.2 模块职责混乱

**event_driven.rs**:
- DAG执行
- 事件监听
- 状态管理
- 调度

**local_executor.rs**:
- 本地任务执行
- Worker线程池
- 进度跟踪

**multi_agent_executor.rs**:
- 多Agent管理
- 任务分配
- 结果收集

**职责重叠**:
- 三个executor都实现了"执行器"职责
- persistence和notify系统分散在多个模块中
- 缺乏统一的错误处理策略

### 1.3 架构目标

根据 [REFACTORING_EXECUTION_PLAN.md](./REFACTORING_EXECUTION_PLAN.md)，scheduler应该是：

**核心功能**:
1. DAG管理（节点、依赖、拓扑排序）
2. 任务调度（分配、执行、监控）
3. 持久化（保存、恢复状态）
4. 事件处理（触发、传播）

**非功能**（应该移到其他模块）:
- Agent管理（应该属于agent模块）
- 通知（可以使用现有event_bus）
- TODO监控（可以使用task模块的repository）

---

## 2. 新模块结构设计

### 2.1 整体架构

```
cis-core/src/scheduler/
├── mod.rs              # 模块导出
├── core/              # 核心调度器
│   ├── mod.rs        # 核心：DAG管理、调度逻辑
│   ├── dag.rs         # DAG：节点、依赖、拓扑排序
│   └── queue.rs      # 任务队列、优先级管理
├── execution/          # 执行器
│   ├── mod.rs        # 执行器trait定义
│   ├── sync.rs       # 同步执行（本地单线程）
│   └── parallel.rs   # 并行执行（线程池）
├── persistence/        # 持久化
│   ├── mod.rs        # Persistence trait
│   ├── sqlite.rs      # SQLite持久化实现
│   └── memory.rs     # 内存持久化（测试用）
├── events/            # 事件集成
│   ├── mod.rs        # 事件监听器注册
│   ├── handler.rs    # 事件处理器
│   └── emitter.rs   # 事件发射器
└── error.rs           # 错误类型定义
```

### 2.2 核心模块设计

#### 2.2.1 `core/mod.rs` - 核心调度器

```rust
//! # Scheduler Core
//!
//! 负责任务调度的核心逻辑：
//! - DAG 构建和验证
//! - 任务分配
//! - 执行协调

use crate::types::{Task, TaskId, TaskStatus, TaskPriority};

pub use dag::DagScheduler;
pub use execution::{Executor, ExecutionResult};
pub use persistence::Persistence;

/// CIS 调度器
pub struct Scheduler {
    dag: DagScheduler,
    executor: Box<dyn Executor>,
    persistence: Box<dyn Persistence>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            dag: DagScheduler::new(),
            executor: Box::new(SyncExecutor::new()),
            persistence: Box::new(SqlitePersistence::new()),
        }
    }

    /// 调度并执行任务
    pub async fn schedule(&mut self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        // 1. 构建DAG
        let dag = self.dag.build(tasks.iter().map(|t| t.id).collect())?;

        // 2. 拓扑排序
        let levels = dag.topological_levels()?;

        // 3. 按层级执行
        let mut results = Vec::new();
        for level_tasks in levels {
            let level_results = self.executor.execute_parallel(level_tasks).await?;
            results.extend(level_results);
        }

        // 4. 持久化状态
        for result in &results {
            self.persistence.save_execution(&result)?;
        }

        Ok(results)
    }
}
```

#### 2.2.2 `core/dag.rs` - DAG管理

**职责**:
- DAG节点管理
- 依赖关系维护
- 拓扑排序（Kahn算法）
- 循环检测

**核心数据结构**:
```rust
pub struct DagNode {
    pub id: TaskId,
    pub dependencies: Vec<TaskId>,
    pub dependents: Vec<TaskId>,
    pub status: NodeStatus,
}

pub struct Dag {
    nodes: HashMap<TaskId, DagNode>,
    root_nodes: Vec<TaskId>,
}

pub struct DagBuilder {
    nodes: Vec<DagNode>,
    edges: Vec<(TaskId, TaskId)>,
}

impl DagBuilder {
    pub fn add_node(&mut self, node: DagNode) -> Result<()> {
        // 验证和添加节点
    }

    pub fn build(&mut self) -> Result<Dag> {
        // 构建DAG并验证
    }
}
```

#### 2.2.3 `core/queue.rs` - 任务队列

**职责**:
- 优先级队列（二叉堆）
- 任务去重
- 状态过滤

```rust
pub struct TaskQueue {
    inner: BinaryHeap<TaskQueueItem>,
}

pub struct TaskQueueItem {
    task: Task,
    priority: TaskPriority,
}

impl TaskQueue {
    pub fn push(&mut self, task: Task) {
        // 添加到队列
    }

    pub fn pop(&mut self) -> Option<Task> {
        // 弹出最高优先级任务
    }
}
```

### 2.3 执行器模块设计

#### 2.3.1 `execution/mod.rs` - Executor trait

```rust
//! Task Executor Trait
//!
//! 定义统一的任务执行接口，支持多种执行策略。

use crate::types::Task;

#[async_trait]
pub trait Executor: Send + Sync {
    /// 执行单个任务
    async fn execute(&self, task: Task) -> Result<ExecutionResult>;

    /// 批量执行任务（可并行）
    async fn execute_batch(&self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();
        for task in tasks {
            match self.execute(task).await {
                Ok(result) => results.push(result),
                Err(e) => return Err(e),
            }
        }
        Ok(results)
    }
}

/// 执行结果
pub struct ExecutionResult {
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub output: serde_json::Value,
    pub duration_secs: f64,
}
```

#### 2.3.2 `execution/sync.rs` - 同步执行器

```rust
//! Synchronous Executor
//!
//! 单线程顺序执行任务。

use super::Executor;
use crate::types::Task;

pub struct SyncExecutor {
    runtime: tokio::runtime::Runtime,
}

impl Executor for SyncExecutor {
    async fn execute(&self, task: Task) -> Result<ExecutionResult> {
        // 单线程执行
    }
}
```

#### 2.3.3 `execution/parallel.rs` - 并行执行器

```rust
//! Parallel Executor
//!
//! 使用线程池并行执行任务。

use super::Executor;
use crate::types::Task;

pub struct ParallelExecutor {
    pool: rayon::ThreadPool,
}

impl Executor for ParallelExecutor {
    async fn execute_batch(&self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        // 并行执行
    }
}
```

### 2.4 持久化模块设计

#### 2.4.1 `persistence/mod.rs` - Persistence trait

```rust
//! Persistence Trait
//!
//! 定义任务持久化接口，支持多种存储后端。

use crate::types::{Task, TaskId, TaskStatus};

#[async_trait]
pub trait Persistence: Send + Sync {
    /// 保存执行结果
    async fn save_execution(&self, result: &ExecutionResult) -> Result<()>;

    /// 加载任务状态
    async fn load_status(&self, task_id: TaskId) -> Result<TaskStatus>;

    /// 获取所有任务
    async fn get_all_tasks(&self) -> Result<Vec<Task>>;
}
```

#### 2.4.2 `persistence/sqlite.rs` - SQLite持久化

```rust
//! SQLite Persistence
//!
//! 使用任务数据库提供持久化。

use super::Persistence;
use rusqlite::Connection;

pub struct SqlitePersistence {
    db: Arc<Mutex<Connection>>,
}

impl Persistence for SqlitePersistence {
    async fn save_execution(&self, result: &ExecutionResult) -> Result<()> {
        // 保存到tasks表
    }
}
```

### 2.5 事件模块设计

#### 2.5.1 `events/mod.rs` - 事件监听器注册

```rust
//! Event Listeners Registry
//!
//! 管理事件监听器的注册和触发。

use std::collections::HashMap;
use crate::types::Event;

pub struct EventRegistry {
    listeners: HashMap<EventType, Vec<EventListener>>,
}

impl EventRegistry {
    pub fn register(&mut self, event_type: EventType, listener: EventListener) {
        // 注册监听器
    }

    pub async fn emit(&self, event: Event) {
        // 触发事件
    }
}
```

---

## 3. 迁移步骤

### Phase 1: 创建新模块结构（Week 1）
- [ ] 创建 `scheduler/core/` 目录
- [ ] 创建 `scheduler/core/dag.rs`
- [ ] 创建 `scheduler/core/queue.rs`
- [ ] 创建 `scheduler/execution/mod.rs`
- [ ] 创建 `scheduler/execution/sync.rs`
- [ ] 创建 `scheduler/execution/parallel.rs`
- [ ] 创建 `scheduler/persistence/mod.rs`
- [ ] 创建 `scheduler/persistence/sqlite.rs`
- [ ] 创建 `scheduler/events/mod.rs`
- [ ] 更新 `scheduler/mod.rs` 导出新模块

### Phase 2: 迁移核心功能（Week 2）
- [ ] 迁移DAG管理逻辑到 `core/dag.rs`
- [ ] 迁移拓扑排序到 `core/dag.rs`
- [ ] 迁移队列管理到 `core/queue.rs`
- [ ] 迁移任务状态管理到 `core/` 或独立的 `state.rs`

### Phase 3: 迁移执行器（Week 3）
- [ ] 迁移 `local_executor.rs` 到 `execution/sync.rs`
- [ ] 迁移 `multi_agent_executor.rs` 到 `execution/parallel.rs`
- [ ] 移除旧的三种executor实现

### Phase 4: 迁移持久化（Week 4）
- [ ] 迁移 `persistence.rs` 的trait定义
- [ ] 实现SQLite持久化
- [ ] 移除旧的持久化代码

### Phase 5: 清理和测试（Week 5）
- [ ] 删除旧的executor模块
- [ ] 删除旧的事件系统代码
- [ ] 编写单元测试
- [ ] 集成测试

---

## 4. 接口设计

### 4.1 核心接口

```rust
// Scheduler
pub async fn schedule_tasks(&mut self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>>;

// Executor
pub async fn execute_task(&self, task: Task) -> Result<ExecutionResult>;

// Persistence
pub async fn save_task_state(&self, task_id: TaskId, status: TaskStatus) -> Result<()>;
```

### 4.2 事件系统

```rust
pub enum SchedulerEvent {
    TaskCompleted(TaskId, ExecutionResult),
    TaskFailed(TaskId, Error),
    DagBuilt(Dag),
}

pub trait EventListener: Send + Sync {
    async fn on_event(&self, event: SchedulerEvent) -> Result<()>;
}
```

---

## 5. 验收标准

### 5.1 代码质量
- [ ] 每个子模块 < 500行
- [ ] 单一职责明确
- [ ] 无循环依赖
- [ ] 测试覆盖率 > 80%

### 5.2 功能完整性
- [ ] 所有现有功能保留
- [ ] DAG构建和验证正常
- [ ] 任务调度正常
- [ ] 持久化正常

### 5.3 性能
- [ ] 拓扑排序性能: O(V+E) = O(V+E^2)
- [ ] 任务分配延迟: <100ms
- [ ] 持久化写入: <50ms

---

## 6. 相关文档

- [REFACTORING_EXECUTION_PLAN.md](./REFACTORING_EXECUTION_PLAN.md)
- [TASK_DAG_WORKFLOW_DESIGN.md](./TASK_DAG_WORKFLOW_DESIGN.md)
- [AGENT_TEAMS_EXECUTION_STRATEGY.md](./AGENT_TEAMS_EXECUTION_STRATEGY.md)

---

**文档版本**: 1.0
**创建日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 设计完成，待实施
