# Task Manager 快速参考

> **版本**: v1.1.6
> **用途**: 快速查找常用 API 和用法

---

## 快速开始

### 初始化

```rust
use cis_core::task::*;
use std::sync::Arc;

let pool = Arc::new(create_database_pool(Some(path), 5).await);
let task_repo = Arc::new(TaskRepository::new(pool.clone()));
let session_repo = Arc::new(SessionRepository::new(pool));
let manager = TaskManager::new(task_repo, session_repo);
```

---

## 核心 API

### 任务 CRUD

| 方法 | 说明 | 示例 |
|------|------|------|
| `create_task` | 创建单个任务 | `manager.create_task(task).await?` |
| `create_tasks_batch` | 批量创建 | `manager.create_tasks_batch(tasks).await?` |
| `query_tasks` | 查询任务 | `manager.query_tasks(filter).await?` |
| `get_task_by_id` | 按 ID 获取 | `manager.get_task_by_id(123).await?` |
| `get_task_by_task_id` | 按 task_id 获取 | `manager.get_task_by_task_id("V-1").await?` |
| `update_task_status` | 更新状态 | `manager.update_task_status(id, status, None).await?` |
| `delete_task` | 删除任务 | `manager.delete_task(id).await?` |
| `search_tasks` | 全文搜索 | `manager.search_tasks("CLI", 10).await?` |

### DAG 和编排

| 方法 | 说明 | 示例 |
|------|------|------|
| `build_dag` | 构建 DAG | `manager.build_dag(task_ids).await?` |
| `orchestrate_tasks` | 完整编排 | `manager.orchestrate_tasks(task_ids).await?` |
| `assign_tasks_to_teams` | 智能分配 | `manager.assign_tasks_to_teams(task_ids).await?` |

### Session 管理

| 方法 | 说明 | 示例 |
|------|------|------|
| `create_session` | 创建 Session | `manager.create_session(agent_id, "claude", 100000, 60).await?` |
| `acquire_session` | 获取可用 Session | `manager.acquire_session(agent_id, 50000).await?` |
| `release_session` | 归还 Session | `manager.release_session(session_id).await?` |
| `cleanup_expired_sessions` | 清理过期 | `manager.cleanup_expired_sessions().await?` |

---

## 常用过滤器

### 按状态过滤

```rust
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    ..Default::default()
};
```

### 按优先级过滤

```rust
let filter = TaskFilter {
    min_priority: Some(TaskPriority::P0),
    max_priority: Some(TaskPriority::P1),
    ..Default::default()
};
```

### 按类型过滤

```rust
let filter = TaskFilter {
    task_types: Some(vec![
        TaskType::ModuleRefactoring,
        TaskType::CodeReview
    ]),
    ..Default::default()
};
```

### 按 Team 过滤

```rust
let filter = TaskFilter {
    assigned_team: Some("Team-V-CLI".to_string()),
    ..Default::default()
};
```

### 组合过滤

```rust
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    task_types: Some(vec![TaskType::ModuleRefactoring]),
    min_priority: Some(TaskPriority::P0),
    sort_by: TaskSortBy::Priority,
    sort_order: TaskSortOrder::Asc,
    limit: Some(100),
    ..Default::default()
};
```

---

## Team 匹配速查

### ModuleRefactoring

| 关键词 | Team |
|--------|-------|
| `CLI`, `cli` | Team-V-CLI |
| `scheduler`, `core` | Team-Q-Core |
| `memory` | Team-V-Memory |
| `skill` | Team-T-Skill |
| 其他 | Team-U-Other |

### EngineCodeInjection

| 引擎类型 | Team |
|---------|-------|
| `Unreal5.7`, `Unreal5.6` | Team-E-Unreal |
| `Unity` | Team-E-Unity |
| `Godot` | Team-E-Godot |
| 其他 | Team-E-Engine |

### 其他类型

| 类型 | Team |
|------|-------|
| `PerformanceOptimization` + `database/storage` | Team-Q-Core |
| `PerformanceOptimization` + `network/p2p` | Team-N-Network |
| `PerformanceOptimization` 其他 | Team-O-Optimization |
| `CodeReview` + `CLI` | Team-V-CLI |
| `CodeReview` + `scheduler` | Team-Q-Core |
| `CodeReview` 其他 | Team-R-Review |
| `TestWriting` | Team-T-Test |
| `Documentation` | Team-D-Docs |

---

## 数据结构速查

### TaskEntity

```rust
TaskEntity {
    id: i64,                              // 数据库 ID
    task_id: String,                        // 任务 ID（V-1）
    name: String,                           // 名称
    task_type: TaskType,                     // 类型
    priority: TaskPriority,                   // 优先级（P0-P3）
    prompt_template: String,                  // Prompt 模板
    context_variables: serde_json::Value,     // 上下文变量
    description: Option<String>,              // 描述
    estimated_effort_days: Option<f64>,      // 预估工时
    dependencies: Vec<String>,               // 依赖任务 ID
    engine_type: Option<String>,             // 引擎类型
    engine_context_id: Option<i64>,          // 引擎上下文 ID
    status: TaskStatus,                      // 状态
    assigned_team_id: Option<String>,        // 分配的 Team
    assigned_agent_id: Option<i64>,         // 分配的 Agent
    assigned_at: Option<i64>,               // 分配时间
    result: Option<TaskResult>,             // 执行结果
    error_message: Option<String>,           // 错误信息
    started_at: Option<i64>,               // 开始时间
    completed_at: Option<i64>,             // 完成时间
    duration_seconds: Option<f64>,           // 执行时长
    metadata: Option<serde_json::Value>,     // 元数据
    created_at_ts: i64,                    // 创建时间戳
    updated_at_ts: i64,                    // 更新时间戳
}
```

### TaskStatus

```rust
pub enum TaskStatus {
    Pending,   // 待执行
    Assigned,   // 已分配
    Running,    // 执行中
    Completed,  // 已完成
    Failed,     // 失败
}
```

### TaskType

```rust
pub enum TaskType {
    ModuleRefactoring,       // 模块重构
    EngineCodeInjection,     // 引擎代码注入
    PerformanceOptimization, // 性能优化
    CodeReview,            // 代码审查
    TestWriting,           // 测试编写
    Documentation,         // 文档编写
}
```

### TaskPriority

```rust
pub enum TaskPriority {
    P0,  // 最高优先级（0）
    P1,  // 高优先级（1）
    P2,  // 中优先级（2）
    P3,  // 低优先级（3）
}
```

---

## 编排结果结构

### TaskOrchestrationResult

```rust
TaskOrchestrationResult {
    plan: ExecutionPlan,        // 执行计划
    status: OrchestrationStatus, // Ready/Running/Completed/Failed
}
```

### ExecutionPlan

```rust
ExecutionPlan {
    dag: Dag,                           // DAG 结构
    levels: Vec<LevelAssignment>,          // 层级分配
    estimated_total_duration_secs: u64,   // 预估总时长
}
```

### LevelAssignment

```rust
LevelAssignment {
    level: u32,                    // 层级编号（0 开始）
    assignments: Vec<TaskAssignment>, // Team 分配
}
```

### TaskAssignment

```rust
TaskAssignment {
    team_id: String,               // Team ID
    task_ids: Vec<String>,          // 任务 ID 列表
    priority: TaskPriority,         // 最高优先级
    estimated_duration_secs: u64,    // 预估时长（秒）
}
```

---

## 常用模式

### 创建并编排任务

```rust
// 1. 创建任务
let tasks = vec![
    create_task("V-1", "CLI", vec![]),
    create_task("V-2", "scheduler", vec!["V-1".into()]),
];
manager.create_tasks_batch(tasks).await?;

// 2. 编排
let result = manager.orchestrate_tasks(vec!["V-1".into(), "V-2".into()]).await?;

// 3. 输出
for level in &result.plan.levels {
    println!("Level {}:", level.level);
    for assignment in &level.assignments {
        println!("  {}: {:?}", assignment.team_id, assignment.task_ids);
    }
}
```

### 查询并分配

```rust
// 1. 查询待分配任务
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    min_priority: Some(TaskPriority::P0),
    ..Default::default()
};

let tasks = manager.query_tasks(filter).await?;
let task_ids: Vec<String> = tasks.iter().map(|t| t.task_id.clone()).collect();

// 2. 分配
let assignments = manager.assign_tasks_to_teams(task_ids).await?;

// 3. 更新任务状态
for assignment in assignments {
    for task_id in &assignment.task_ids {
        manager.assign_task_to_team(
            task_id.parse()?,
            assignment.team_id.clone(),
            None
        ).await?;
    }
}
```

### Session 复用模式

```rust
// 1. 尝试复用 Session
if let Some(session) = manager.acquire_session(agent_id, 50000).await? {
    println!("Reusing session: {}", session.session_id);

    // 2. 使用 Session 执行任务...

    // 3. 归还 Session
    manager.release_session(session.id).await?;
} else {
    // 4. 创建新 Session
    let session_id = manager.create_session(agent_id, "claude", 100000, 60).await?;
    println!("Created new session: {}", session_id);
}
```

---

## 时间计算速查

### 任务级别

```
1 人日 = 8 小时 = 28,800 秒
```

### DAG 级别

```
总时间 = Σ(max(每个层级中最长的 Team 时间))
```

### 示例

```
Level 0:
  Team-V-CLI: 2 人日 = 57,600 秒
  Team-V-Memory: 1 人日 = 28,800 秒
  → Level 0 = 57,600 秒

Level 1:
  Team-Q-Core: 3 人日 = 86,400 秒
  → Level 1 = 86,400 秒

总计 = 57,600 + 86,400 = 144,000 秒 = 40 小时
```

---

## 错误处理

### 标准模式

```rust
use cis_core::error::Result;

async fn my_function() -> Result<()> {
    manager.create_task(task).await?;
    Ok(())
}
```

### 详细错误处理

```rust
use cis_core::error::{CisError, Result};

async fn my_function() -> Result<()> {
    match manager.orchestrate_tasks(task_ids).await {
        Ok(result) => {
            // 处理成功
            Ok(())
        }
        Err(e) => {
            // 错误已经包含上下文
            eprintln!("编排失败: {}", e);
            Err(e)
        }
    }
}
```

---

## 导入路径

### TaskManager

```rust
use cis_core::task::TaskManager;
use cis_core::task::{
    TaskAssignment,
    LevelAssignment,
    ExecutionPlan,
    TaskOrchestrationResult,
    OrchestrationStatus,
};
```

### Models

```rust
use cis_core::task::models::{
    TaskEntity,
    TaskType,
    TaskPriority,
    TaskStatus,
    TaskFilter,
    TaskSortBy,
    TaskSortOrder,
    TaskResult,
};
```

### Repository

```rust
use cis_core::task::{TaskRepository, SessionRepository};
use cis_core::task::db::create_database_pool;
```

---

## 调试技巧

### 启用日志

```rust
env_logger::init();

// 或设置环境变量
export RUST_LOG=debug
```

### 查看 DAG 结构

```rust
let result = manager.orchestrate_tasks(task_ids).await?;

// 打印 DAG 节点
for (id, node) in &result.plan.dag.nodes {
    println!("Node {}: {}", id, node.name);
    println!("  Dependencies: {:?}", node.dependencies);
    println!("  Dependents: {:?}", node.dependents);
}
```

### 验证分配

```rust
let assignments = manager.assign_tasks_to_teams(task_ids).await?;

// 统计每个 Team 的任务数
use std::collections::HashMap;
let mut team_counts: HashMap<String, usize> = HashMap::new();

for assignment in assignments {
    *team_counts.entry(assignment.team_id).or_insert(0) += assignment.task_ids.len();
}

println!("Team 任务分配:");
for (team, count) in team_counts {
    println!("  {}: {} tasks", team, count);
}
```

---

## 性能优化速查

### 批量操作

```rust
// ✅ 批量创建
let ids = manager.create_tasks_batch(tasks).await?;

// ❌ 循环创建
for task in tasks {
    manager.create_task(task).await?;
}
```

### Session 复用

```rust
// ✅ 复用 Session
if let Some(session) = manager.acquire_session(agent_id, 50000).await? {
    manager.release_session(session.id).await?;
}

// ❌ 每次创建
let session_id = manager.create_session(agent_id, "claude", 100000, 60).await?;
```

### 过滤查询

```rust
// ✅ 使用过滤器
let filter = TaskFilter {
    limit: Some(100),
    ..Default::default()
};
let tasks = manager.query_tasks(filter).await?;

// ❌ 全部加载再过滤
let all_tasks = manager.query_tasks(TaskFilter::default()).await?;
let filtered: Vec<_> = all_tasks.into_iter().filter(|t| t.priority == TaskPriority::P0).collect();
```

---

## 常见命令

### Cargo 测试

```bash
# 运行所有测试
cargo test --package cis-core task::manager

# 运行单个测试
cargo test --package cis-core test_create_and_get_task

# 显示输出
cargo test --package cis-core task::manager -- --nocapture

# 运行文档示例
cargo test --package cis-core --doc
```

### 构建

```bash
# 构建
cargo build --package cis-core

# 发布构建
cargo build --package cis-core --release
```

---

## 相关资源

### 文档

- [Task Manager 使用指南](./TASK_MANAGER_GUIDE.md)
- [实现报告](./TASK_MANAGER_IMPLEMENTATION_REPORT.md)
- [Task 存储设计](./TASK_STORAGE_SQLITE_DESIGN.md)
- [DAG 工作流设计](./TASK_DAG_WORKFLOW_DESIGN.md)

### API 文档

```bash
# 生成文档
cargo doc --package cis-core --no-deps --open

# 查看特定模块
cargo doc --package cis-core --open cis_core::task::manager
```

---

**版本**: v1.1.6
**最后更新**: 2026-02-12
