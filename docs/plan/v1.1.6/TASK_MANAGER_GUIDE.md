# Task Manager 使用指南

> **版本**: v1.1.6
> **最后更新**: 2026-02-12

---

## 概述

TaskManager 是 CIS v1.1.6 的核心任务管理和编排系统，整合了：
- **TaskRepository**: 任务 CRUD 和查询
- **DagBuilder**: DAG 构建和依赖解析
- **SessionRepository**: Agent Session 管理

### 核心能力

1. **任务管理**: 创建、查询、更新、删除任务
2. **智能分配**: 基于任务类型和名称自动匹配 Team
3. **DAG 编排**: 构建依赖图、拓扑排序、层级分配
4. **执行计划**: 生成完整的执行计划和预估时间

---

## 数据结构

### TaskAssignment

任务到 Team 的分配结果：

```rust
pub struct TaskAssignment {
    pub team_id: String,              // Team ID
    pub task_ids: Vec<String>,         // 分配的任务 ID 列表
    pub priority: TaskPriority,         // 优先级（取最高）
    pub estimated_duration_secs: u64,  // 预估执行时长（秒）
}
```

### LevelAssignment

按 DAG 层级的任务分配：

```rust
pub struct LevelAssignment {
    pub level: u32,                    // 层级编号（0 = 根层级）
    pub assignments: Vec<TaskAssignment>,  // 该层级的所有分配
}
```

### ExecutionPlan

完整的执行计划：

```rust
pub struct ExecutionPlan {
    pub dag: Dag,                           // DAG 结构
    pub levels: Vec<LevelAssignment>,          // 按层级分配的任务
    pub estimated_total_duration_secs: u64,    // 预估总时长
}
```

### TaskOrchestrationResult

编排结果：

```rust
pub struct TaskOrchestrationResult {
    pub plan: ExecutionPlan,           // 执行计划
    pub status: OrchestrationStatus,    // 编排状态
}

pub enum OrchestrationStatus {
    Ready,              // 就绪
    Running,            // 运行中
    Completed,          // 已完成
    Failed(String),     // 失败（含错误信息）
}
```

---

## API 使用示例

### 1. 创建 TaskManager

```rust
use cis_core::task::{
    TaskManager, TaskRepository, SessionRepository,
    create_database_pool
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建数据库连接池
    let db_path = Some("~/.cis/data/tasks.db".into());
    let pool = Arc::new(create_database_pool(db_path, 5).await);

    // 2. 创建 Repository
    let task_repo = Arc::new(TaskRepository::new(pool.clone()));
    let session_repo = Arc::new(SessionRepository::new(pool));

    // 3. 创建 TaskManager
    let manager = TaskManager::new(task_repo, session_repo);

    Ok(())
}
```

### 2. 创建任务

```rust
use cis_core::task::models::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 创建单个任务
    let task = TaskEntity {
        id: 0,
        task_id: "V-1".to_string(),
        name: "CLI 架构修复".to_string(),
        task_type: TaskType::ModuleRefactoring,
        priority: TaskPriority::P0,
        prompt_template: "审查以下 CLI handler...".to_string(),
        context_variables: serde_json::json!({
            "handlers_dir": "cis-node/src/cli/handlers"
        }),
        description: Some("审查并重构所有 CLI handler".to_string()),
        estimated_effort_days: Some(5.0),
        dependencies: vec![],
        engine_type: None,
        engine_context_id: None,
        status: TaskStatus::Pending,
        assigned_team_id: None,
        assigned_agent_id: None,
        assigned_at: None,
        result: None,
        error_message: None,
        started_at: None,
        completed_at: None,
        duration_seconds: None,
        metadata: None,
        created_at_ts: chrono::Utc::now().timestamp(),
        updated_at_ts: chrono::Utc::now().timestamp(),
    };

    let task_id = manager.create_task(task).await?;
    println!("Created task with ID: {}", task_id);

    Ok(())
}
```

### 3. 批量创建任务

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    let tasks = vec![
        create_task("V-1", "CLI 架构修复", TaskType::ModuleRefactoring, TaskPriority::P0, vec![]),
        create_task("V-2", "scheduler 拆分", TaskType::ModuleRefactoring, TaskPriority::P1, vec!["V-1".to_string()]),
        create_task("V-3", "memory 优化", TaskType::PerformanceOptimization, TaskPriority::P1, vec![]),
    ];

    let task_ids = manager.create_tasks_batch(tasks).await?;
    println!("Created {} tasks", task_ids.len());

    Ok(())
}
```

### 4. 智能任务分配

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 分配任务到 Teams
    let assignments = manager
        .assign_tasks_to_teams(vec!["V-1".to_string(), "V-2".to_string()])
        .await?;

    for assignment in assignments {
        println!("Team: {}", assignment.team_id);
        println!("  Tasks: {:?}", assignment.task_ids);
        println!("  Priority: {:?}", assignment.priority);
        println!("  Duration: {} seconds", assignment.estimated_duration_secs);
    }

    // 输出示例：
    // Team: Team-V-CLI
    //   Tasks: ["V-1"]
    //   Priority: P0
    //   Duration: 144000 seconds

    Ok(())
}
```

### 5. DAG 编排

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 创建有依赖的任务
    let tasks = vec![
        create_task("task-A", "基础任务", TaskType::ModuleRefactoring, TaskPriority::P0, vec![]),
        create_task("task-B", "依赖任务", TaskType::ModuleRefactoring, TaskPriority::P1, vec!["task-A".to_string()]),
        create_task("task-C", "并行任务", TaskType::CodeReview, TaskPriority::P0, vec![]),
    ];

    for task in tasks {
        manager.create_task(task).await?;
    }

    // 编排任务
    let result = manager
        .orchestrate_tasks(vec!["task-A".to_string(), "task-B".to_string(), "task-C".to_string()])
        .await?;

    println!("Orchestration Status: {:?}", result.status);
    println!("Total Levels: {}", result.plan.levels.len());
    println!("Estimated Duration: {} seconds", result.plan.estimated_total_duration_secs);

    // 打印每个层级
    for level_assignment in &result.plan.levels {
        println!("\n=== Level {} ===", level_assignment.level);
        for assignment in &level_assignment.assignments {
            println!("  Team: {}", assignment.team_id);
            for task_id in &assignment.task_ids {
                println!("    - {}", task_id);
            }
        }
    }

    // 输出示例：
    // === Level 0 ===
    //   Team: Team-Q-Core
    //     - task-A
    //   Team: Team-R-Review
    //     - task-C
    //
    // === Level 1 ===
    //   Team: Team-Q-Core
    //     - task-B

    Ok(())
}
```

### 6. 查询任务

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 查询所有 P0 任务
    let filter = TaskFilter {
        min_priority: Some(TaskPriority::P0),
        max_priority: Some(TaskPriority::P0),
        ..Default::default()
    };

    let p0_tasks = manager.query_tasks(filter).await?;
    println!("Found {} P0 tasks", p0_tasks.len());

    // 查询特定类型的任务
    let filter = TaskFilter {
        task_types: Some(vec![TaskType::ModuleRefactoring]),
        ..Default::default()
    };

    let refactoring_tasks = manager.query_tasks(filter).await?;
    println!("Found {} refactoring tasks", refactoring_tasks.len());

    // 查询特定 Team 的任务
    let filter = TaskFilter {
        assigned_team: Some("Team-V-CLI".to_string()),
        ..Default::default()
    };

    let team_tasks = manager.query_tasks(filter).await?;
    println!("Team-V-CLI has {} tasks", team_tasks.len());

    Ok(())
}
```

### 7. 更新任务状态

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 标记任务为运行中
    let task_id = 1; // 假设任务 ID
    manager
        .update_task_status(task_id, TaskStatus::Running, None)
        .await?;

    // 任务完成
    let result = TaskResult {
        success: true,
        output: Some("Task completed successfully".to_string()),
        artifacts: vec!["/path/to/artifact1".to_string()],
        exit_code: Some(0),
    };

    manager
        .update_task_result(task_id, &result, 3600.0)
        .await?;

    // 标记为完成状态
    manager
        .update_task_status(task_id, TaskStatus::Completed, None)
        .await?;

    Ok(())
}
```

### 8. Session 管理

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = setup_manager().await?;

    // 创建 Session
    let agent_id = 1;
    let session_id = manager
        .create_session(agent_id, "claude", 100000, 60)
        .await?;
    println!("Created session: {}", session_id);

    // 获取可复用的 Session
    let session = manager
        .acquire_session(agent_id, 50000)
        .await?;

    if let Some(s) = session {
        println!("Acquired session: {}", s.session_id);
        println!("Capacity: {}, Used: {}", s.context_capacity, s.context_used);

        // 使用 Session...

        // 归还 Session
        manager.release_session(s.id).await?;
    }

    // 清理过期 Sessions
    let cleaned = manager.cleanup_expired_sessions().await?;
    println!("Cleaned up {} sessions", cleaned);

    Ok(())
}
```

---

## Team 匹配规则

TaskManager 根据任务类型和名称智能匹配 Team：

### ModuleRefactoring（模块重构）

| 任务名称特征 | 匹配 Team |
|------------|-----------|
| 包含 "CLI" 或 "cli" | Team-V-CLI |
| 包含 "scheduler" 或 "core" | Team-Q-Core |
| 包含 "memory" | Team-V-Memory |
| 包含 "skill" | Team-T-Skill |
| 其他 | Team-U-Other |

### EngineCodeInjection（引擎代码注入）

| 引擎类型 | 匹配 Team |
|---------|-----------|
| Unreal5.7, Unreal5.6 | Team-E-Unreal |
| Unity | Team-E-Unity |
| Godot | Team-E-Godot |
| 其他 | Team-E-Engine |

### PerformanceOptimization（性能优化）

| 任务名称特征 | 匹配 Team |
|------------|-----------|
| 包含 "database" 或 "storage" | Team-Q-Core |
| 包含 "network" 或 "p2p" | Team-N-Network |
| 其他 | Team-O-Optimization |

### CodeReview（代码审查）

| 任务名称特征 | 匹配 Team |
|------------|-----------|
| 包含 "CLI" 或 "cli" | Team-V-CLI |
| 包含 "scheduler" | Team-Q-Core |
| 其他 | Team-R-Review |

### TestWriting（测试编写）

- 匹配 Team: **Team-T-Test**

### Documentation（文档编写）

- 匹配 Team: **Team-D-Docs**

---

## 时间估算

### 任务级别估算

```rust
fn estimate_team_duration(&self, tasks: &[TaskEntity]) -> u64 {
    let total_days: f64 = tasks
        .iter()
        .map(|t| t.estimated_effort_days.unwrap_or(1.0))
        .sum();

    // 1 人日 = 8 小时 = 28800 秒
    (total_days * 28800.0) as u64
}
```

### DAG 级别估算

```rust
fn estimate_total_duration(&self, assignments: &[LevelAssignment]) -> u64 {
    // 总时间 = 各层级中最长时间之和（串行）
    // 每个层级内部是并行的，取最长的 Team
    assignments
        .iter()
        .map(|level| {
            level
                .assignments
                .iter()
                .map(|a| a.estimated_duration_secs)
                .max()
                .unwrap_or(0)
        })
        .sum()
}
```

### 示例计算

假设有 2 个层级的 DAG：

**Level 0**（并行）:
- Team-V-CLI: 2 个任务，5.0 人日 → 144,000 秒
- Team-V-Memory: 1 个任务，3.0 人日 → 86,400 秒

**Level 1**（串行）:
- Team-Q-Core: 1 个任务，15.0 人日 → 432,000 秒

**总时长**: max(144000, 86400) + 432000 = 576,000 秒 = 160 小时

---

## 完整工作流示例

### 场景：CLI 架构修复

```rust
use cis_core::task::*;
use cis_core::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化
    let pool = Arc::new(create_database_pool(
        Some("~/.cis/data/tasks.db".into()),
        5
    ).await);
    let task_repo = Arc::new(TaskRepository::new(pool.clone()));
    let session_repo = Arc::new(SessionRepository::new(pool));
    let manager = TaskManager::new(task_repo, session_repo);

    // 2. 创建任务
    let tasks = vec![
        TaskEntity {
            id: 0,
            task_id: "V-1".to_string(),
            name: "CLI 架构修复".to_string(),
            task_type: TaskType::ModuleRefactoring,
            priority: TaskPriority::P0,
            prompt_template: "审查以下 CLI handler...".to_string(),
            context_variables: serde_json::json!({
                "handlers_dir": "cis-node/src/cli/handlers"
            }),
            description: Some("审查并重构所有 CLI handler".to_string()),
            estimated_effort_days: Some(5.0),
            dependencies: vec![],
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: None,
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        },
    ];

    manager.create_tasks_batch(tasks).await?;

    // 3. 编排任务
    let result = manager
        .orchestrate_tasks(vec!["V-1".to_string()])
        .await?;

    // 4. 输出执行计划
    println!("=== 执行计划 ===");
    println!("状态: {:?}", result.status);
    println!("预估时长: {} 秒", result.plan.estimated_total_duration_secs);

    for level in &result.plan.levels {
        println!("\nLevel {}:", level.level);
        for assignment in &level.assignments {
            println!("  Team: {}", assignment.team_id);
            for task_id in &assignment.task_ids {
                println!("    - {}", task_id);
            }
        }
    }

    // 5. 分配到 Team
    let assignments = manager
        .assign_tasks_to_teams(vec!["V-1".to_string()])
        .await?;

    for assignment in assignments {
        println!("\n分配: {} → {}", assignment.task_ids.join(", "), assignment.team_id);
    }

    Ok(())
}
```

---

## 最佳实践

### 1. 任务命名规范

使用清晰的、描述性的任务名称：
- ✅ "CLI handler 架构修复"
- ✅ "scheduler 模块拆分设计"
- ❌ "fix-1"
- ❌ "task-abc"

### 2. 依赖关系管理

- 保持依赖关系简洁清晰
- 避免循环依赖（DAG 构建会检测）
- 合理使用优先级而非过度依赖

### 3. 工作量估算

- 基于 1 人日 = 8 小时
- 考虑代码审查、测试、文档时间
- 适当预留缓冲（建议 +20%）

### 4. Team 分配

- 信任智能匹配规则
- 必要时可手动调整
- 保持 Team 负载均衡

### 5. 错误处理

```rust
match manager.orchestrate_tasks(task_ids).await {
    Ok(result) => {
        // 处理成功结果
    }
    Err(e) => {
        eprintln!("编排失败: {}", e);
        // 根据错误类型处理
    }
}
```

---

## 性能优化

### 批量操作

```rust
// ✅ 推荐：批量创建
let ids = manager.create_tasks_batch(tasks).await?;

// ❌ 避免：循环创建
for task in tasks {
    manager.create_task(task).await?;  // 每次都开启事务
}
```

### 查询优化

```rust
// ✅ 使用过滤器
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    min_priority: Some(TaskPriority::P0),
    limit: Some(100),
    ..Default::default()
};
let tasks = manager.query_tasks(filter).await?;

// ❌ 避免查询全部再过滤
let all_tasks = manager.query_tasks(TaskFilter::default()).await?;
let p0_tasks: Vec<_> = all_tasks
    .into_iter()
    .filter(|t| t.priority == TaskPriority::P0)
    .collect();
```

### Session 复用

```rust
// ✅ 推荐：复用 Session
if let Some(session) = manager.acquire_session(agent_id, 50000).await? {
    // 使用 Session
    manager.release_session(session.id).await?;
}

// ❌ 避免：每次都创建新 Session
let session_id = manager.create_session(agent_id, "claude", 100000, 60).await?;
```

---

## 故障排查

### 问题 1: DAG 构建失败

```
Error: Failed to build DAG
```

**原因**: 循环依赖或依赖任务不存在

**解决**:
1. 检查任务的 `dependencies` 字段
2. 确保依赖的任务已创建
3. 使用 DAG 验证工具检查循环

### 问题 2: Team 分配不正确

```
Task assigned to wrong team
```

**原因**: 任务名称不符合匹配规则

**解决**:
1. 检查任务命名规范
2. 确保关键词正确（CLI, scheduler, memory 等）
3. 必要时手动指定 Team

### 问题 3: Session 获取失败

```
Failed to acquire session
```

**原因**: 没有可用 Session 或容量不足

**解决**:
1. 降低 `min_capacity` 要求
2. 创建新 Session
3. 清理过期 Session

---

## 相关文档

- [Task Storage Design](./TASK_STORAGE_SQLITE_DESIGN.md)
- [DAG Workflow Design](./TASK_DAG_WORKFLOW_DESIGN.md)
- [Agent Pool Design](./AGENT_POOL_MULTI_RUNTIME_DESIGN.md)

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
