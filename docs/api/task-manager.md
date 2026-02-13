# Task Manager API Reference

**Version**: CIS v1.1.6
**Module**: `cis_core::task::manager`
**Last Updated**: 2026-02-13

---

## Table of Contents

- [Overview](#overview)
- [Core Concepts](#core-concepts)
- [TaskManager](#taskmanager)
- [Task Orchestration](#task-orchestration)
- [Team Assignment](#team-assignment)
- [Execution Planning](#execution-planning)
- [Session Management](#session-management)
- [Advanced Features](#advanced-features)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)

---

## Overview

The TaskManager is the high-level orchestration layer for CIS task management, providing:

- **Unified API**: Single interface for tasks, DAGs, and sessions
- **Team Assignment**: Intelligent task-to-team routing
- **Execution Planning**: DAG-based execution with parallelization
- **Status Tracking**: Comprehensive task lifecycle management
- **Session Integration**: Agent session management for context reuse

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TaskManager                             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Task Repo   │  │ DAG Builder  │  │Session Repo  │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│         │                 │                  │               │
│         └─────────────────┴──────────────────┘               │
│                           │                                │
│                           ▼                                │
│  ┌──────────────────────────────────────────────────────┐     │
│  │          Task Orchestration Engine                  │     │
│  │  • Build DAGs                                     │     │
│  │  • Assign to Teams                                │     │
│  │  • Create Execution Plans                          │     │
│  │  • Track Progress                                 │     │
│  └──────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### Task Orchestration

The complete process of managing tasks from creation to completion.

**Stages:**

1. **Task Creation**: Register tasks in the system
2. **DAG Building**: Resolve dependencies and build execution graph
3. **Team Assignment**: Match tasks to appropriate teams
4. **Execution Planning**: Generate parallel execution levels
5. **Status Tracking**: Monitor progress and handle failures

### Team Assignment

Intelligent routing of tasks to specialized teams based on:

- **Task Type**: Code review, refactoring, testing, etc.
- **Module Name**: CLI, scheduler, memory, etc.
- **Engine Type**: Unreal, Unity, Godot, etc.

**Supported Teams:**

| Team ID | Specialization |
|---------|---------------|
| `Team-V-CLI` | CLI module refactoring |
| `Team-V-Memory` | Memory module optimization |
| `Team-Q-Core` | Core scheduler refactoring |
| `Team-T-Skill` | Skill development |
| `Team-T-Test` | Test writing |
| `Team-D-Docs` | Documentation |
| `Team-R-Review` | Code review |
| `Team-E-Unreal` | Unreal engine integration |
| `Team-E-Unity` | Unity engine integration |
| `Team-N-Network` | Network/P2P tasks |
| `Team-O-Optimization` | Performance optimization |

### Execution Plan

A structured plan for executing a set of tasks.

**Components:**

- **DAG Structure**: Task dependency graph
- **Execution Levels**: Parallelizable task groups
- **Team Assignments**: Tasks assigned to teams per level
- **Duration Estimates**: Time estimates for planning

---

## TaskManager

### Creating the Manager

```rust
use cis_core::task::manager::TaskManager;
use cis_core::task::repository::TaskRepository;
use cis_core::task::session::SessionRepository;
use std::sync::Arc;

let pool = Arc::new(create_database_pool(Some("/path/to/db.db"), 5).await);

let task_repo = Arc::new(TaskRepository::new(pool.clone()));
let session_repo = Arc::new(SessionRepository::new(pool));

let manager = TaskManager::new(task_repo, session_repo);
```

---

### Task CRUD Operations

#### Create Single Task

```rust
use cis_core::task::models::*;

let task = TaskEntity {
    id: 0,
    task_id: "review-001".to_string(),
    name: "Authentication Module Review".to_string(),
    task_type: TaskType::CodeReview,
    priority: TaskPriority::P0,
    prompt_template: "Review the authentication module...".to_string(),
    context_variables: json!({"focus": "security"}),
    description: Some("Security-focused code review".to_string()),
    estimated_effort_days: Some(2.0),
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
```

#### Create Tasks Batch

```rust
let tasks = vec![
    create_task("task-1", "First task", TaskType::CodeReview),
    create_task("task-2", "Second task", TaskType::TestWriting),
    create_task("task-3", "Third task", TaskType::Documentation),
];

let task_ids = manager.create_tasks_batch(tasks).await?;
println!("Created {} tasks", task_ids.len());
```

#### Query Tasks

```rust
use cis_core::task::models::TaskFilter;

// Query with filter
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    min_priority: Some(TaskPriority::P0),
    max_priority: Some(TaskPriority::P1),
    sort_by: TaskSortBy::Priority,
    sort_order: TaskSortOrder::Asc,
    limit: Some(50),
    ..Default::default()
};

let tasks = manager.query_tasks(filter).await?;

for task in tasks {
    println!("Task: {} (Priority: {:?})", task.name, task.priority);
}
```

#### Get Task by ID

```rust
// By database ID
let task = manager.get_task_by_id(123).await?;

// By task_id string
let task = manager.get_task_by_task_id("review-001").await?;

if let Some(task) = task {
    println!("Found: {}", task.name);
}
```

#### Update Task Status

```rust
// Update to running
manager.update_task_status(
    123,                        // task_id
    TaskStatus::Running,
    None                         // error_message
).await?;

// Update to failed with error
manager.update_task_status(
    123,
    TaskStatus::Failed,
    Some("Network timeout".to_string())
).await?;
```

#### Update Task Result

```rust
use cis_core::task::models::TaskResult;

let result = TaskResult {
    success: true,
    output: Some("Review completed successfully".to_string()),
    artifacts: vec!["/tmp/review.md".to_string()],
    exit_code: Some(0),
};

manager.update_task_result(
    123,           // task_id
    &result,
    123.45         // duration_seconds
).await?;
```

#### Delete Tasks

```rust
// Single deletion
manager.delete_task(123).await?;

// Batch deletion
let task_ids = vec![1, 2, 3, 4, 5];
let deleted_count = manager.delete_tasks_batch(&task_ids).await?;
println!("Deleted {} tasks", deleted_count);
```

---

## Task Orchestration

### Building DAGs

```rust
let task_ids = vec![
    "task-001".to_string(),
    "task-002".to_string(),
    "task-003".to_string(),
];

let dag = manager.build_dag(task_ids).await?;

println!("DAG has {} nodes", dag.nodes.len());
println!("Root nodes: {:?}", dag.roots);
```

### Full Orchestration

Complete orchestration from tasks to execution plan.

```rust
use cis_core::task::manager::*;

let task_ids = vec![
    "base-task".to_string(),
    "dependent-task".to_string(),
    "final-task".to_string(),
];

let result = manager.orchestrate_tasks(task_ids).await?;

println!("Orchestration status: {:?}", result.status);

match result.status {
    OrchestrationStatus::Ready => {
        println!("Ready to execute");
        println!("Total duration: {} seconds",
            result.plan.estimated_total_duration_secs
        );
    }
    OrchestrationStatus::Failed(msg) => {
        eprintln!("Orchestration failed: {}", msg);
    }
    _ => {}
}
```

**Process:**

1. Build DAG from task IDs
2. Generate topological levels
3. Assign tasks to teams for each level
4. Calculate execution time estimates
5. Return execution plan

---

## Team Assignment

### Assigning Tasks to Teams

```rust
let task_ids = vec![
    "cli-refactor".to_string(),
    "memory-opt".to_string(),
    "test-write".to_string(),
];

let assignments = manager.assign_tasks_to_teams(task_ids).await?;

for assignment in assignments {
    println!("Team: {}", assignment.team_id);
    println!("Tasks: {:?}", assignment.task_ids);
    println!("Priority: {:?}", assignment.priority);
    println!("Estimated duration: {} seconds",
        assignment.estimated_duration_secs
    );
}
```

**Assignment Logic:**

The `match_team_for_task` function determines team assignment:

```rust
// CLI refactoring
"CLI handler refactoring" → Team-V-CLI

// Memory optimization
"Memory module optimization" → Team-V-Memory

// Engine code injection (Unreal)
"Unreal code injection" → Team-E-Unreal

// Code review
"Core module review" → Team-Q-Core

// Test writing
"Write unit tests" → Team-T-Test

// Documentation
"Update API docs" → Team-D-Docs
```

### Team Assignment Rules

#### Module Refactoring

```rust
TaskType::ModuleRefactoring
├── Contains "CLI" → Team-V-CLI
├── Contains "scheduler" or "core" → Team-Q-Core
├── Contains "memory" → Team-V-Memory
├── Contains "skill" → Team-T-Skill
└── Default → Team-U-Other
```

#### Engine Code Injection

```rust
TaskType::EngineCodeInjection
├── engine_type = "Unreal5.7" or "Unreal5.6" → Team-E-Unreal
├── engine_type = "Unity" → Team-E-Unity
├── engine_type = "Godot" → Team-E-Godot
└── Default → Team-E-Engine
```

#### Performance Optimization

```rust
TaskType::PerformanceOptimization
├── Contains "database" or "storage" → Team-Q-Core
├── Contains "network" or "p2p" → Team-N-Network
└── Default → Team-O-Optimization
```

#### Code Review

```rust
TaskType::CodeReview
├── Contains "CLI" → Team-V-CLI
├── Contains "scheduler" → Team-Q-Core
└── Default → Team-R-Review
```

#### Other Types

```rust
TaskType::TestWriting → Team-T-Test
TaskType::Documentation → Team-D-Docs
```

---

## Execution Planning

### ExecutionPlan Structure

```rust
pub struct ExecutionPlan {
    pub dag: Dag,                           // Task dependency graph
    pub levels: Vec<LevelAssignment>,         // Execution levels
    pub estimated_total_duration_secs: u64,   // Total duration
}

pub struct LevelAssignment {
    pub level: u32,                          // Level number
    pub assignments: Vec<TaskAssignment>,       // Team assignments
}

pub struct TaskAssignment {
    pub team_id: String,                      // Team identifier
    pub task_ids: Vec<String>,               // Assigned tasks
    pub priority: TaskPriority,               // Team priority
    pub estimated_duration_secs: u64,         // Duration estimate
}
```

### Execution Plan Example

```rust
let result = manager.orchestrate_tasks(vec![
    "base-1".to_string(),
    "base-2".to_string(),
    "dependent".to_string(),
]).await?;

let plan = result.plan;

println!("Execution Plan:");
println!("Total duration: {} seconds", plan.estimated_total_duration_secs);

for level in &plan.levels {
    println!("\nLevel {}:", level.level);

    for assignment in &level.assignments {
        println!("  Team: {}", assignment.team_id);
        println!("  Tasks: {:?}", assignment.task_ids);
        println!("  Priority: {:?}", assignment.priority);
        println!("  Duration: {} seconds", assignment.estimated_duration_secs);
    }
}
```

**Output Example:**

```
Execution Plan:
Total duration: 86400 seconds

Level 0:
  Team: Team-V-CLI
  Tasks: ["base-1"]
  Priority: P0
  Duration: 28800 seconds
  Team: Team-Q-Core
  Tasks: ["base-2"]
  Priority: P0
  Duration: 28800 seconds

Level 1:
  Team: Team-T-Test
  Tasks: ["dependent"]
  Priority: P0
  Duration: 28800 seconds
```

---

## Session Management

### Creating Sessions

```rust
let session_id = manager.create_session(
    123,            // agent_id
    "claude",       // runtime_type
    200000,         // context_capacity (tokens)
    60              // ttl_minutes
).await?;

println!("Created session: {}", session_id);
```

### Acquiring Sessions

```rust
let session = manager.acquire_session(
    123,     // agent_id
    50000     // min_capacity
).await?;

if let Some(session) = session {
    println!("Reusing session: {}", session.session_id);
    println!("Available: {} tokens",
        session.context_capacity - session.context_used
    );
}
```

### Releasing Sessions

```rust
manager.release_session(session_id).await?;
println!("Session released for reuse");
```

### Cleaning Up Sessions

```rust
let expired_count = manager.cleanup_expired_sessions().await?;
println!("Cleaned up {} expired sessions", expired_count);
```

---

## Advanced Features

### Task Statistics

```rust
use cis_core::task::manager::TaskStatistics;

let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    ..Default::default()
};

let count = manager.count_tasks(filter).await?;
println!("Pending tasks: {}", count);
```

### Full-Text Search

```rust
let results = manager.search_tasks(
    "authentication security review",
    10  // limit
).await?;

for task in results {
    println!("Found: {} - {}", task.task_id, task.name);
}
```

### Marking Tasks as Running

```rust
manager.mark_task_running(task_id).await?;
```

**Side Effects:**
- Sets status to `Running`
- Records `started_at` timestamp

### Assigning Tasks to Teams

```rust
manager.assign_task_to_team(
    123,                              // task_id
    "Team-V-CLI".to_string(),         // team_id
    Some(456)                         // agent_id (optional)
).await?;
```

---

## Usage Examples

### Example 1: Complete Task Lifecycle

```rust
use cis_core::task::*;
use cis_core::task::manager::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let pool = Arc::new(create_database_pool(Some("/path/to/db.db"), 5).await);
    let task_repo = Arc::new(TaskRepository::new(pool.clone()));
    let session_repo = Arc::new(SessionRepository::new(pool));
    let manager = TaskManager::new(task_repo, session_repo);

    // Create task
    let task = TaskEntity {
        id: 0,
        task_id: "review-001".to_string(),
        name: "Auth Module Review".to_string(),
        task_type: TaskType::CodeReview,
        priority: TaskPriority::P0,
        prompt_template: "Review the authentication module...".to_string(),
        context_variables: json!({"focus": "security"}),
        description: Some("Security review".to_string()),
        estimated_effort_days: Some(2.0),
        dependencies: vec![],
        status: TaskStatus::Pending,
        // ... other fields ...
    };

    let task_id = manager.create_task(task).await?;

    // Mark as running
    manager.mark_task_running(task_id).await?;

    // Execute task (simulated)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Complete with result
    let result = TaskResult {
        success: true,
        output: Some("Review completed".to_string()),
        artifacts: vec![],
        exit_code: Some(0),
    };
    manager.update_task_result(task_id, &result, 5.0).await?;

    // Verify completion
    let completed = manager.get_task_by_id(task_id).await?.unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);

    Ok(())
}
```

---

### Example 2: Multi-Level Orchestration

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pool = Arc::new(create_database_pool(Some("/path/to/db.db"), 5).await);
    let task_repo = Arc::new(TaskRepository::new(pool.clone()));
    let session_repo = Arc::new(SessionRepository::new(pool));
    let manager = TaskManager::new(task_repo, session_repo);

    // Create tasks with dependencies
    let tasks = vec![
        create_task("base", "Base Task", vec![]),
        create_task("dep1", "Dependent 1", vec!["base".to_string()]),
        create_task("dep2", "Dependent 2", vec!["base".to_string()]),
        create_task("final", "Final Task", vec![
            "dep1".to_string(),
            "dep2".to_string()
        ]),
    ];

    for task in &tasks {
        manager.create_task(task.clone()).await?;
    }

    // Orchestrate
    let result = manager.orchestrate_tasks(vec![
        "base".to_string(),
        "dep1".to_string(),
        "dep2".to_string(),
        "final".to_string(),
    ]).await?;

    println!("Orchestration: {:?}", result.status);
    println!("Levels: {}", result.plan.levels.len());

    // Execute by level
    for level in &result.plan.levels {
        println!("\nExecuting Level {}:", level.level);

        for assignment in &level.assignments {
            println!("  Team: {}", assignment.team_id);

            for task_id in &assignment.task_ids {
                println!("    Executing: {}", task_id);
                // Execute task...
            }
        }
    }

    Ok(())
}

fn create_task(id: &str, name: &str, deps: Vec<String>) -> TaskEntity {
    TaskEntity {
        id: 0,
        task_id: id.to_string(),
        name: name.to_string(),
        task_type: TaskType::ModuleRefactoring,
        priority: TaskPriority::P0,
        prompt_template: "Execute task".to_string(),
        context_variables: json!({}),
        description: Some(name.to_string()),
        estimated_effort_days: Some(1.0),
        dependencies: deps,
        status: TaskStatus::Pending,
        // ... other fields ...
        created_at_ts: chrono::Utc::now().timestamp(),
        updated_at_ts: chrono::Utc::now().timestamp(),
    }
}
```

---

### Example 3: Team-Based Assignment

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pool = Arc::new(create_database_pool(Some("/path/to/db.db"), 5).await);
    let task_repo = Arc::new(TaskRepository::new(pool.clone()));
    let session_repo = Arc::new(SessionRepository::new(pool));
    let manager = TaskManager::new(task_repo, session_repo);

    // Create diverse tasks
    let tasks = vec![
        create_cli_task("cli-refactor", "CLI refactoring"),
        create_memory_task("memory-opt", "Memory optimization"),
        create_engine_task("unreal-inject", "Unreal injection", "Unreal5.7"),
        create_test_task("test-write", "Write tests"),
        create_docs_task("api-docs", "API documentation"),
    ];

    for task in &tasks {
        manager.create_task(task.clone()).await?;
    }

    // Assign to teams
    let task_ids: Vec<String> = tasks.iter()
        .map(|t| t.task_id.clone())
        .collect();

    let assignments = manager.assign_tasks_to_teams(task_ids).await?;

    println!("Team Assignments:");
    for assignment in assignments {
        println!("  Team: {}", assignment.team_id);
        println!("  Priority: {:?}", assignment.priority);
        println!("  Tasks: {}", assignment.task_ids.join(", "));
        println!("  Duration: {} seconds", assignment.estimated_duration_secs);
        println!();
    }

    Ok(())
}
```

---

### Example 4: Session Reuse Pattern

```rust
async fn execute_with_session(
    manager: &TaskManager,
    agent_id: i64,
    task: &Task
) -> Result<()> {
    // Estimate required tokens
    let required = estimate_tokens(task);

    // Acquire or create session
    let session = loop {
        match manager.acquire_session(agent_id, required).await? {
            Some(session) => break session,
            None => {
                // Create new session
                let new_id = manager.create_session(
                    agent_id,
                    "claude",
                    200000,
                    60
                ).await?;
                break manager.get_session_by_id(new_id).await?.unwrap();
            }
        }
    };

    // Execute task
    let tokens_used = execute_task_with_session(&session, task).await?;

    // Update usage
    manager.update_session_usage(session.id, tokens_used).await?;

    // Release for reuse
    manager.release_session(session.id).await?;

    Ok(())
}
```

---

## Best Practices

### DO ✓

1. **Use orchestration for complex workflows**
   ```rust
   // ✓ Use orchestration for dependent tasks
   let result = manager.orchestrate_tasks(task_ids).await?;
   ```

2. **Batch operations when possible**
   ```rust
   // ✓ Batch create
   manager.create_tasks_batch(tasks).await?;

   // ✓ Batch delete
   manager.delete_tasks_batch(&task_ids).await?;
   ```

3. **Monitor task status**
   ```rust
   // ✓ Check status before execution
   let task = manager.get_task_by_id(task_id).await?;
   if let Some(task) = task {
       match task.status {
           TaskStatus::Pending => { /* execute */ }
           TaskStatus::Running => { /* wait */ }
           TaskStatus::Completed => { /* skip */ }
           _ => {}
       }
   }
   ```

4. **Handle errors gracefully**
   ```rust
   // ✓ Handle orchestration failures
   match manager.orchestrate_tasks(task_ids).await {
       Ok(result) => execute_plan(result.plan).await,
       Err(err) => {
           eprintln!("Orchestration failed: {}", err);
           // Handle error
       }
   }
   ```

### DON'T ✗

1. **Don't ignore dependencies**
   ```rust
   // ✗ Creating tasks without setting dependencies
   let task_b = create_task("task-b", "Task B", vec![]);

   // ✓ Set dependencies correctly
   let task_b = create_task("task-b", "Task B", vec!["task-a".to_string()]);
   ```

2. **Don't leak sessions**
   ```rust
   // ✗ Forgetting to release session
   let session = acquire_session(...).await?;
   execute(&session).await?;
   // Session not released!

   // ✓ Always release sessions
   let session = acquire_session(...).await?;
   execute(&session).await?;
   release_session(session.id).await?;
   ```

3. **Don't ignore task status**
   ```rust
   // ✗ Executing without checking status
   execute_task(task_id).await?;

   // ✓ Check status first
   let task = get_task(task_id).await?;
   if task.status == TaskStatus::Pending {
       execute_task(task_id).await?;
   }
   ```

---

## API Reference Summary

### TaskManager Methods

#### Task CRUD

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `create_task` | `TaskEntity` | `Result<i64>` | Create single task |
| `create_tasks_batch` | `Vec<TaskEntity>` | `Result<Vec<i64>>` | Batch create tasks |
| `get_task_by_id` | `i64` | `Result<Option<TaskEntity>>` | Get by database ID |
| `get_task_by_task_id` | `&str` | `Result<Option<TaskEntity>>` | Get by task_id |
| `query_tasks` | `TaskFilter` | `Result<Vec<TaskEntity>>` | Query with filter |
| `count_tasks` | `TaskFilter` | `Result<i64>` | Count tasks |
| `search_tasks` | `&str, usize` | `Result<Vec<TaskEntity>>` | Full-text search |
| `update_task_status` | `i64, TaskStatus, Option<String>` | `Result<()>` | Update status |
| `assign_task_to_team` | `i64, String, Option<i64>` | `Result<()>` | Assign to team |
| `update_task_result` | `i64, &TaskResult, f64` | `Result<()>` | Update result |
| `mark_task_running` | `i64` | `Result<()>` | Mark as running |
| `delete_task` | `i64` | `Result<()>` | Delete task |
| `delete_tasks_batch` | `&[i64]` | `Result<usize>` | Batch delete |

#### Orchestration

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `build_dag` | `Vec<String>` | `Result<Dag>` | Build DAG from task IDs |
| `orchestrate_tasks` | `Vec<String>` | `Result<TaskOrchestrationResult>` | Full orchestration |
| `assign_tasks_to_teams` | `Vec<String>` | `Result<Vec<TaskAssignment>>` | Team assignment |

#### Session Management

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `create_session` | `i64, &str, i64, i64` | `Result<i64>` | Create session |
| `acquire_session` | `i64, i64` | `Result<Option<AgentSessionEntity>>` | Acquire session |
| `release_session` | `i64` | `Result<()>` | Release session |
| `cleanup_expired_sessions` | - | `Result<usize>` | Cleanup expired |

### Key Types

#### TaskOrchestrationResult

```rust
pub struct TaskOrchestrationResult {
    pub plan: ExecutionPlan,           // Execution plan
    pub status: OrchestrationStatus,    // Orchestration status
}
```

#### OrchestrationStatus

```rust
pub enum OrchestrationStatus {
    Ready,                   // Ready for execution
    Running,                 // Currently running
    Completed,               // All tasks completed
    Failed(String),          // Failed with error message
}
```

#### ExecutionPlan

```rust
pub struct ExecutionPlan {
    pub dag: Dag,                          // Task dependency graph
    pub levels: Vec<LevelAssignment>,         // Execution levels
    pub estimated_total_duration_secs: u64,   // Total duration
}
```

#### TaskAssignment

```rust
pub struct TaskAssignment {
    pub team_id: String,                    // Team ID
    pub task_ids: Vec<String>,              // Assigned tasks
    pub priority: TaskPriority,              // Priority
    pub estimated_duration_secs: u64,        // Duration estimate
}
```

---

**See Also:**
- [Task System API](./task-system.md)
- [Session Management API](./session-management.md)
- [DAG Builder API](./dag-builder.md)
