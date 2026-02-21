# TASK 1.2: 提取 cis-types

> **Phase**: 1 - cis-common 基础
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: f41816b
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

将 `cis-core` 中的公共类型提取到独立的 `cis-types` crate。

## 工作内容

### 1. 分析现有类型

审查 `cis-core/src/types.rs`，提取以下类型：

```rust
// === Task 相关类型 ===
pub type TaskId = String;
pub type NodeId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskLevel {
    Mechanical { retry: u8 },           // 机械级：自动执行，失败重试
    Recommended { default_action: Action, timeout_secs: u16 },  // 推荐级：倒计时执行，可干预
    Confirmed,                          // 确认级：模态确认，必须手动确认
    Arbitrated { stakeholders: Vec<String> },  // 仲裁级：暂停 DAG，等待仲裁
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    Execute,
    Skip,
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FailureType {
    Ignorable,  // 可忽略债务，继续下游
    Blocking,   // 阻塞债务，冻结 DAG
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AmbiguityPolicy {
    AutoBest,
    Suggest { default: Action, timeout_secs: u16 },
    Ask,
    Escalate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Default)]
pub enum TaskPriority {
    Low = 1,
    #[default]
    Medium = 2,
    High = 3,
    Urgent = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub description: Option<String>,
    pub group_name: String,
    pub completion_criteria: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub dependencies: Vec<TaskId>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub workspace_dir: Option<String>,
    pub sandboxed: bool,
    pub allow_network: bool,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub node_id: Option<NodeId>,
    pub metadata: HashMap<String, String>,
    pub level: TaskLevel,
    pub on_ambiguity: AmbiguityPolicy,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub rollback: Option<Vec<String>>,
    pub idempotent: bool,
    pub failure_type: Option<FailureType>,
    pub skill_id: Option<String>,
    pub skill_params: Option<serde_json::Value>,
    pub skill_result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub completed_at: DateTime<Utc>,
}

// === Memory 相关类型 ===
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemoryCategory {
    Execution,  // 执行记录
    Result,     // 结果数据
    Error,      // 错误信息
    Context,    // 上下文信息
    Skill,      // Skill 经验
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemoryDomain {
    Private,  // 私域加密记忆
    Public,   // 公域共享记忆
}

// === Debt 机制类型 ===
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtEntry {
    pub task_id: TaskId,
    pub dag_run_id: String,
    pub failure_type: FailureType,
    pub error_message: String,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
}

// === Skill 相关类型 ===
pub type SkillTask = Task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}
```

### 2. 创建 cis-types crate

```
cis-common/cis-types/
├── Cargo.toml
└── src/
    ├── lib.rs              # Re-export all types
    ├── task.rs             # TaskId, NodeId, TaskStatus, TaskLevel, Task, TaskResult
    ├── memory.rs           # MemoryCategory, MemoryDomain
    ├── debt.rs             # DebtEntry, FailureType
    └── skill.rs            # SkillTask, SkillExecutionResult
```

### 3. 配置依赖

**File**: `cis-common/cis-types/Cargo.toml`

```toml
[package]
name = "cis-types"
version = "1.2.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
```

### 4. Re-export 组织

**File**: `cis-common/cis-types/src/lib.rs`

```rust
//! # CIS Common Types
//!
//! Core data structures and domain types for CIS.

pub mod task;
pub mod memory;
pub mod debt;
pub mod skill;

// Task types
pub use task::{TaskId, NodeId, TaskStatus, TaskLevel, Task, TaskResult, TaskPriority, Action, AmbiguityPolicy};

// Memory types
pub use memory::{MemoryCategory, MemoryDomain};

// Debt types
pub use debt::{DebtEntry, FailureType};

// Skill types
pub use skill::{SkillTask, SkillExecutionResult};
```

### 5. 确保向后兼容

**File**: `cis-core/src/types.rs`

```rust
// Re-export from cis-types for backward compatibility
pub use cis_types::*;
```

**File**: `cis-core/Cargo.toml`

```toml
[dependencies]
cis-types = { path = "../cis-common/cis-types", version = "1.2.0" }
```

## 验收标准

- [ ] 所有公共类型成功提取到 cis-types crate
- [ ] Task 相关类型完整 (TaskId, TaskStatus, TaskLevel, Task, TaskResult, TaskPriority, Action, AmbiguityPolicy)
- [ ] Memory 相关类型完整 (MemoryCategory, MemoryDomain)
- [ ] Debt 相关类型完整 (DebtEntry, FailureType)
- [ ] Skill 相关类型完整 (SkillTask, SkillExecutionResult)
- [ ] cis-core 通过重导出保持向后兼容
- [ ] 单元测试覆盖所有类型边界情况
- [ ] 文档注释完整（包含四级决策机制说明）
- [ ] 依赖关系正确配置

## 依赖

- Task 1.1 (创建 cis-common)

## 阻塞

- Task 2.1 (提取 cis-storage)

---
