# Task System API Reference

**Version**: CIS v1.1.6
**Module**: `cis_core::task`
**Last Updated**: 2026-02-13

---

## Table of Contents

- [Overview](#overview)
- [Core Types](#core-types)
- [Data Models](#data-models)
- [Database Operations](#database-operations)
- [Task Repository](#task-repository)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)
- [Usage Examples](#usage-examples)

---

## Overview

The Task System provides a comprehensive task management infrastructure for CIS, including:

- **Task Storage**: SQLite-backed persistent storage with full-text search
- **Task Models**: Rich data structures for task entities, priorities, types, and statuses
- **Repository Pattern**: Clean abstraction for CRUD operations and complex queries
- **Dependency Management**: Support for task dependencies and DAG construction
- **Agent Integration**: Session management for agent task execution

### Key Features

- **Async/Await**: Full async support using Tokio
- **Type Safety**: Strongly typed enums for status, priority, and task types
- **SQLite Integration**: Efficient database operations with connection pooling
- **Full-Text Search**: Built-in FTS5 support for task searching
- **Serialization**: JSON serialization/deserialization for all models

---

## Core Types

### TaskEntity

The primary data structure representing a task in the system.

```rust
pub struct TaskEntity {
    pub id: i64,                              // Auto-increment primary key
    pub task_id: String,                        // Unique task identifier
    pub name: String,                           // Human-readable name
    pub task_type: TaskType,                     // Task category
    pub priority: TaskPriority,                  // Execution priority
    pub prompt_template: String,                  // AI prompt template
    pub context_variables: serde_json::Value,     // Context variables
    pub description: Option<String>,             // Optional description
    pub estimated_effort_days: Option<f64>,       // Estimated effort
    pub dependencies: Vec<String>,                // Dependent task IDs
    pub engine_type: Option<String>,              // Engine type (e.g., "Unreal5.7")
    pub engine_context_id: Option<i64>,           // Engine context ID
    pub status: TaskStatus,                       // Current status
    pub assigned_team_id: Option<String>,          // Assigned team ID
    pub assigned_agent_id: Option<i64>,           // Assigned agent ID
    pub assigned_at: Option<i64>,                 // Assignment timestamp
    pub result: Option<TaskResult>,               // Execution result
    pub error_message: Option<String>,            // Error message if failed
    pub started_at: Option<i64>,                 // Start timestamp
    pub completed_at: Option<i64>,               // Completion timestamp
    pub duration_seconds: Option<f64>,            // Execution duration
    pub metadata: Option<serde_json::Value>,     // Additional metadata
    pub created_at_ts: i64,                     // Creation timestamp
    pub updated_at_ts: i64,                     // Last update timestamp
}
```

**Methods:**

- `created_at() -> chrono::DateTime<chrono::Utc>`: Returns creation time as DateTime
- `updated_at() -> chrono::DateTime<chrono::Utc>`: Returns update time as DateTime

---

### TaskType

Enumeration of supported task categories.

```rust
pub enum TaskType {
    ModuleRefactoring,        // Module restructuring and optimization
    EngineCodeInjection,      // Game engine code integration
    PerformanceOptimization,  // Performance improvements
    CodeReview,              // Code review tasks
    TestWriting,             // Test generation
    Documentation,           // Documentation tasks
}
```

**Serialization:**
- Snake case in JSON: `"module_refactoring"`, `"code_review"`, etc.
- Implements `ToSql` and `FromSql` for SQLite storage

**Usage:**

```rust
let task = TaskEntity {
    task_type: TaskType::CodeReview,
    // ...
};

// JSON serialization
let json = serde_json::to_string(&task)?;
// Result: {"type": "code_review", ...}
```

---

### TaskPriority

Four-level priority system for task execution ordering.

```rust
pub enum TaskPriority {
    P0,  // Highest priority (critical)
    P1,  // High priority
    P2,  // Medium priority
    P3,  // Low priority
}
```

**Properties:**

- Implements `Ord` and `PartialOrd`: P0 < P1 < P2 < P3
- Lower numeric value = higher priority
- Used for sorting and filtering

**Methods:**

- `value() -> i32`: Returns numeric value (0-3)
- `from_value(i32) -> Option<Self>`: Creates from numeric value

**Examples:**

```rust
let priority = TaskPriority::P0;
assert_eq!(priority.value(), 0);
assert!(priority < TaskPriority::P1);

// Filtering
let filter = TaskFilter {
    min_priority: Some(TaskPriority::P0),
    max_priority: Some(TaskPriority::P1),
    ..Default::default()
};
```

---

### TaskStatus

Lifecycle states for task execution.

```rust
pub enum TaskStatus {
    Pending,   // Task is queued, not yet assigned
    Assigned,  // Task assigned to an agent/team
    Running,   // Task is currently executing
    Completed, // Task completed successfully
    Failed,    // Task execution failed
}
```

**State Transitions:**

```
Pending → Assigned → Running → Completed
                               ↘ Failed
```

**Database Storage:**
- Stored as lowercase strings: `"pending"`, `"running"`, etc.
- Implements `ToSql` and `FromSql` for SQLite

---

### TaskResult

Execution result container.

```rust
pub struct TaskResult {
    pub success: bool,              // Execution success flag
    pub output: Option<String>,     // Standard output
    pub artifacts: Vec<String>,     // Generated artifacts
    pub exit_code: Option<i32>,     // Process exit code
}
```

**JSON Representation:**

```json
{
  "success": true,
  "output": "Task completed successfully",
  "artifacts": ["/path/to/artifact1", "/path/to/artifact2"],
  "exit_code": 0
}
```

---

## Data Models

### TaskFilter

Advanced filtering for task queries.

```rust
pub struct TaskFilter {
    pub status: Option<Vec<TaskStatus>>,      // Filter by status
    pub task_types: Option<Vec<TaskType>>,    // Filter by type
    pub min_priority: Option<TaskPriority>,    // Minimum priority
    pub max_priority: Option<TaskPriority>,    // Maximum priority
    pub assigned_team: Option<String>,         // Filter by team
    pub engine_type: Option<String>,           // Filter by engine
    pub sort_by: TaskSortBy,                  // Sort field
    pub sort_order: TaskSortOrder,             // Sort direction
    pub limit: Option<usize>,                  // Result limit
    pub offset: Option<usize>,                 // Result offset
}
```

**Sort Fields:**

```rust
pub enum TaskSortBy {
    Priority,          // Sort by priority
    CreatedAt,        // Sort by creation time
    UpdatedAt,        // Sort by update time
    Name,             // Sort by name
    EstimatedEffort,  // Sort by effort estimate
}
```

**Sort Order:**

```rust
pub enum TaskSortOrder {
    Asc,   // Ascending order
    Desc,  // Descending order
}
```

**Usage Examples:**

```rust
// Get all pending P0-P1 tasks, sorted by priority
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    min_priority: Some(TaskPriority::P0),
    max_priority: Some(TaskPriority::P1),
    sort_by: TaskSortBy::Priority,
    sort_order: TaskSortOrder::Asc,
    limit: Some(50),
    ..Default::default()
};

let tasks = repository.query(filter).await?;
```

---

### SessionStatus

Agent session lifecycle states.

```rust
pub enum SessionStatus {
    Active,   // Session is in use
    Idle,     // Session available for reuse
    Expired,   // Session has expired
    Released,  // Session explicitly released
}
```

---

### AgentEntity

Represents an AI agent in the system.

```rust
pub struct AgentEntity {
    pub id: i64,                              // Auto-increment ID
    pub agent_type: String,                     // Agent type (e.g., "claude")
    pub display_name: String,                   // Human-readable name
    pub enabled: bool,                          // Active flag
    pub config: serde_json::Value,               // Agent configuration
    pub capabilities: Vec<String>,               // Supported capabilities
    pub created_at: i64,                       // Creation timestamp
    pub updated_at: i64,                       // Update timestamp
}
```

**Example Config:**

```json
{
  "model": "claude-3-sonnet",
  "max_tokens": 200000,
  "temperature": 0.7,
  "timeout_secs": 300
}
```

**Example Capabilities:**

```json
[
  "code_review",
  "module_refactoring",
  "test_writing"
]
```

---

### AgentSessionEntity

Manages agent session lifecycle for context reuse.

```rust
pub struct AgentSessionEntity {
    pub id: i64,                    // Session database ID
    pub session_id: String,          // Unique session identifier
    pub agent_id: i64,              // Associated agent ID
    pub runtime_type: String,         // Runtime type (e.g., "claude")
    pub status: SessionStatus,        // Current status
    pub context_capacity: i64,        // Max context tokens
    pub context_used: i64,           // Used context tokens
    pub created_at: i64,             // Creation timestamp
    pub last_used_at: Option<i64>,   // Last usage timestamp
    pub expires_at: i64,             // Expiration timestamp
}
```

**Session Lifecycle:**

1. **Create**: Initialize with capacity and TTL
2. **Acquire**: Find reusable session with sufficient capacity
3. **Use**: Update context usage during execution
4. **Release**: Mark as available for reuse
5. **Expire**: TTL expiration or manual cleanup

---

### TaskExecutionLog

Detailed execution logging for tasks.

```rust
pub struct TaskExecutionLog {
    pub id: i64,                            // Log entry ID
    pub task_id: i64,                        // Associated task ID
    pub session_id: i64,                      // Associated session ID
    pub stage: ExecutionStage,                 // Execution stage
    pub log_level: LogLevel,                  // Log level
    pub message: String,                      // Log message
    pub details: Option<serde_json::Value>,   // Additional details
    pub duration_ms: Option<i64>,             // Stage duration
    pub tokens_used: Option<i64>,             // Tokens consumed
    pub timestamp: i64,                       // Log timestamp
}
```

**Execution Stages:**

```rust
pub enum ExecutionStage {
    Preparing,   // Task preparation
    Executing,   // Task execution
    Completed,   // Task completion
    Failed,      // Task failure
}
```

**Log Levels:**

```rust
pub enum LogLevel {
    Debug,  // Debug information
    Info,   // Informational messages
    Warn,   // Warning messages
    Error,  // Error messages
}
```

---

## Database Operations

### DatabasePool

SQLite connection pool with async support.

```rust
pub struct DatabasePool {
    // Internal implementation
}
```

**Creation:**

```rust
use cis_core::task::db::create_database_pool;

// Create with file backing
let pool = create_database_pool(
    Some("/path/to/database.db"),
    5  // max_connections
).await;

// Create in-memory database
let pool = create_database_pool(None, 5).await;
```

**Methods:**

- `acquire(&self) -> Result<Connection>`: Acquire a connection from the pool
- Transaction support via `transaction()` method

---

### Schema Initialization

```rust
use cis_core::task::db::initialize_schema;

// Initialize database schema
initialize_schema(&pool).await?;
```

**Creates Tables:**
- `tasks`: Task storage with FTS5
- `agents`: Agent registry
- `agent_sessions`: Session management
- `task_execution_logs`: Execution logging

---

### Database Statistics

```rust
use cis_core::task::db::DatabaseStats;

pub struct DatabaseStats {
    pub total_tasks: i64,
    pub active_sessions: i64,
    pub database_size_bytes: u64,
    pub page_count: i64,
}

// Get statistics
let stats = pool.get_stats().await?;
println!("Total tasks: {}", stats.total_tasks);
```

---

### Database Vacuum

```rust
use cis_core::task::db::vacuum_database;

// Reclaim database space
vacuum_database(&pool).await?;
```

**When to Use:**
- After large deletions
- Periodic maintenance
- Database size optimization

---

## Task Repository

### Creating Tasks

#### Single Task Creation

```rust
use cis_core::task::repository::TaskRepository;
use cis_core::task::models::*;

let repository = TaskRepository::new(pool);

let task = TaskEntity {
    id: 0,  // Will be auto-assigned
    task_id: "task-001".to_string(),
    name: "Review Authentication Module".to_string(),
    task_type: TaskType::CodeReview,
    priority: TaskPriority::P0,
    prompt_template: "Review the following code for security issues...".to_string(),
    context_variables: serde_json::json!({
        "module": "authentication",
        "focus_areas": ["security", "performance"]
    }),
    description: Some("Security review of auth module".to_string()),
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
    metadata: Some(serde_json::json!({
        "project": "cis-core",
        "module": "auth"
    })),
    created_at_ts: chrono::Utc::now().timestamp(),
    updated_at_ts: chrono::Utc::now().timestamp(),
};

let task_id = repository.create(&task).await?;
println!("Created task with ID: {}", task_id);
```

#### Batch Task Creation

```rust
let tasks = vec![
    task1,
    task2,
    task3,
];

let task_ids = repository.batch_create(&tasks).await?;
println!("Created {} tasks", task_ids.len());
```

**Performance:**
- Batch creation uses a single transaction
- Significantly faster than individual inserts
- Atomic operation (all or nothing)

---

### Querying Tasks

#### Get by ID

```rust
// Get by database ID
let task = repository.get_by_id(123).await?;

// Get by task_id string
let task = repository.get_by_task_id("task-001").await?;

if let Some(task) = task {
    println!("Found task: {}", task.name);
}
```

#### Advanced Filtering

```rust
use cis_core::task::models::TaskFilter;

let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending, TaskStatus::Assigned]),
    task_types: Some(vec![TaskType::CodeReview, TaskType::ModuleRefactoring]),
    min_priority: Some(TaskPriority::P0),
    max_priority: Some(TaskPriority::P1),
    assigned_team: Some("Team-R-Review".to_string()),
    sort_by: TaskSortBy::Priority,
    sort_order: TaskSortOrder::Asc,
    limit: Some(100),
    offset: Some(0),
    ..Default::default()
};

let tasks = repository.query(filter).await?;

for task in tasks {
    println!("Task: {} (Priority: {:?})", task.name, task.priority);
}
```

#### Counting Tasks

```rust
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),
    ..Default::default()
};

let count = repository.count(filter).await?;
println!("Pending tasks: {}", count);
```

---

### Updating Tasks

#### Update Status

```rust
// Update with error message
repository.update_status(
    task_id,
    TaskStatus::Failed,
    Some("Connection timeout".to_string())
).await?;

// Update without error
repository.update_status(
    task_id,
    TaskStatus::Running,
    None
).await?;
```

#### Update Assignment

```rust
repository.update_assignment(
    task_id,
    Some("Team-V-CLI".to_string()),  // team_id
    Some(456)                         // agent_id
).await?;
```

#### Update Result

```rust
use cis_core::task::models::TaskResult;

let result = TaskResult {
    success: true,
    output: Some("Code review completed".to_string()),
    artifacts: vec!["/tmp/review-report.md".to_string()],
    exit_code: Some(0),
};

repository.update_result(
    task_id,
    &result,
    123.45  // duration_seconds
).await?;
```

#### Mark as Running

```rust
repository.mark_running(task_id).await?;
```

**Side Effects:**
- Sets status to `Running`
- Records `started_at` timestamp
- Updates `updated_at` timestamp

---

### Deleting Tasks

#### Single Deletion

```rust
repository.delete(task_id).await?;
```

#### Batch Deletion

```rust
let task_ids = vec![1, 2, 3, 4, 5];
let deleted_count = repository.batch_delete(&task_ids).await?;
println!("Deleted {} tasks", deleted_count);
```

---

### Searching Tasks

#### Full-Text Search

```rust
// Search using FTS5
let results = repository.search(
    "authentication security review",
    10  // limit
).await?;

for task in results {
    println!("Found: {} - {}", task.task_id, task.name);
}
```

**Search Features:**
- Fast full-text search using FTS5
- Supports Boolean operators: `AND`, `OR`, `NOT`
- Phrase search with quotes
- Ranking by relevance

**Advanced Queries:**

```rust
// Boolean search
repository.search("authentication AND security", 10).await?;

// Phrase search
repository.search("\"authentication module\"", 10).await?;

// Negative search
repository.search("review NOT documentation", 10).await?;
```

---

## Error Handling

### Error Types

All repository operations return `rusqlite::Result<T>`.

```rust
pub type Result<T> = std::result::Result<T, rusqlite::Error>;
```

### Common Errors

```rust
use rusqlite::Error;

match repository.create(&task).await {
    Ok(id) => println!("Created: {}", id),
    Err(Error::SqliteFailure(err, msg)) => {
        eprintln!("SQLite error: {:?} - {:?}", err, msg);
    }
    Err(Error::QueryReturnedNoRows) => {
        eprintln!("Task not found");
    }
    Err(Error::ToSqlConversionFailure(err)) => {
        eprintln!("Serialization error: {:?}", err);
    }
    Err(err) => {
        eprintln!("Database error: {}", err);
    }
}
```

---

## Performance Considerations

### Connection Pooling

**Optimal Pool Size:**

```rust
// For CPU-bound operations: number of CPU cores
let pool_size = num_cpus::get();

// For I/O-bound operations: 2-4x CPU cores
let pool_size = num_cpus::get() * 4;

// Create pool
let pool = create_database_pool(Some(path), pool_size).await;
```

### Batch Operations

**Always use batch operations for multiple inserts:**

```rust
// ❌ Slow: Individual inserts
for task in tasks {
    repository.create(&task).await?;  // Separate transaction each
}

// ✅ Fast: Batch insert
repository.batch_create(&tasks).await?;  // Single transaction
```

### Query Optimization

**Use indexed fields in filters:**

```rust
// Indexed fields: status, priority, type, assigned_team_id
let filter = TaskFilter {
    status: Some(vec![TaskStatus::Pending]),  // Uses index
    min_priority: Some(TaskPriority::P0),       // Uses index
    ..Default::default()
};
```

**Pagination for large results:**

```rust
let page_size = 100;
let mut offset = 0;

loop {
    let filter = TaskFilter {
        limit: Some(page_size),
        offset: Some(offset),
        ..Default::default()
    };

    let tasks = repository.query(filter).await?;
    if tasks.is_empty() {
        break;
    }

    // Process tasks
    process_tasks(tasks)?;

    offset += page_size;
}
```

### Database Maintenance

**Regular vacuuming:**

```rust
// Run periodically (e.g., weekly)
vacuum_database(&pool).await?;
```

**Analyze query performance:**

```rust
// Enable SQLite query logging
let conn = pool.acquire().await?;
conn.execute("PRAGMA optimize", [])?;
```

---

## Usage Examples

### Complete Task Lifecycle

```rust
use cis_core::task::*;
use cis_core::task::models::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize
    let pool = db::create_database_pool(
        Some("/path/to/cis.db"),
        5
    ).await;
    let repo = TaskRepository::new(pool);

    // Create task
    let task = TaskEntity {
        id: 0,
        task_id: "review-001".to_string(),
        name: "Security Review".to_string(),
        task_type: TaskType::CodeReview,
        priority: TaskPriority::P0,
        prompt_template: "Review code...".to_string(),
        context_variables: json!({}),
        description: Some("Security review".to_string()),
        estimated_effort_days: Some(1.0),
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

    let task_id = repo.create(&task).await?;

    // Mark as running
    repo.mark_running(task_id).await?;

    // Simulate execution
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Complete with result
    let result = TaskResult {
        success: true,
        output: Some("Review completed".to_string()),
        artifacts: vec![],
        exit_code: Some(0),
    };
    repo.update_result(task_id, &result, 5.0).await?;

    // Verify
    let completed = repo.get_by_id(task_id).await?.unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);

    Ok(())
}
```

### Task Dependency Management

```rust
// Create dependent tasks
let task_a = create_task("task-a", "Base Task", vec![]);
let task_b = create_task("task-b", "Dependent Task", vec!["task-a".to_string()]);
let task_c = create_task("task-c", "Final Task", vec!["task-b".to_string()]);

let id_a = repo.create(&task_a).await?;
let id_b = repo.create(&task_b).await?;
let id_c = repo.create(&task_c).await?;

// Query with dependency information
let task_b_loaded = repo.get_by_task_id("task-b").await?.unwrap();
assert_eq!(task_b_loaded.dependencies, vec!["task-a".to_string()]);
```

### Full-Text Search Example

```rust
// Index multiple tasks
let tasks = vec![
    create_task_with_desc("auth-001", "Authentication Module", "Handles user login"),
    create_task_with_desc("auth-002", "Authorization Module", "Permission checks"),
    create_task_with_desc("db-001", "Database Module", "Data persistence"),
];

for task in &tasks {
    repo.create(task).await?;
}

// Search for authentication-related tasks
let results = repo.search("authentication login", 10).await?;
assert!(results.len() >= 1);

// Search with boolean operators
let results = repo.search("authentication NOT database", 10).await?;
```

---

## API Reference Summary

### Repository Methods

| Method | Parameters | Return | Description |
|--------|------------|---------|-------------|
| `create` | `&TaskEntity` | `Result<i64>` | Create single task |
| `batch_create` | `&[TaskEntity]` | `Result<Vec<i64>>` | Batch create tasks |
| `get_by_id` | `i64` | `Result<Option<TaskEntity>>` | Get by database ID |
| `get_by_task_id` | `&str` | `Result<Option<TaskEntity>>` | Get by task_id |
| `query` | `TaskFilter` | `Result<Vec<TaskEntity>>` | Filter and sort |
| `count` | `TaskFilter` | `Result<i64>` | Count matching tasks |
| `search` | `&str, usize` | `Result<Vec<TaskEntity>>` | Full-text search |
| `update_status` | `i64, TaskStatus, Option<String>` | `Result<()>` | Update status |
| `update_assignment` | `i64, Option<String>, Option<i64>` | `Result<()>` | Update assignment |
| `update_result` | `i64, &TaskResult, f64` | `Result<()>` | Update result |
| `mark_running` | `i64` | `Result<()>` | Mark as running |
| `delete` | `i64` | `Result<()>` | Delete task |
| `batch_delete` | `&[i64]` | `Result<usize>` | Batch delete |

---

**See Also:**
- [Session Management API](./session-management.md)
- [DAG Builder API](./dag-builder.md)
- [Task Manager API](./task-manager.md)
