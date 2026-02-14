# DAG Builder API Reference

**Version**: CIS v1.1.6
**Module**: `cis_core::task::dag`, `cis_core::scheduler::core::dag`
**Last Updated**: 2026-02-13

---

## Table of Contents

- [Overview](#overview)
- [Core Concepts](#core-concepts)
- [DAG Builder](#dag-builder)
- [DAG Structure](#dag-structure)
- [Topological Sorting](#topological-sorting)
- [Cycle Detection](#cycle-detection)
- [Execution Levels](#execution-levels)
- [Scheduler Integration](#scheduler-integration)
- [Usage Examples](#usage-examples)
- [Error Handling](#error-handling)
- [Performance Considerations](#performance-considerations)

---

## Overview

The DAG (Directed Acyclic Graph) Builder provides task dependency management and execution orchestration for CIS:

- **Dependency Resolution**: Automatically resolve task dependencies
- **Topological Sorting**: Generate valid execution orders
- **Cycle Detection**: Prevent circular dependencies
- **Parallel Execution**: Identify parallelizable task groups
- **Depth Calculation**: Compute task dependency depths for layering

### Key Features

- **Automatic Cycle Detection**: DFS-based cycle detection with path extraction
- **Kahn's Algorithm**: Efficient topological sorting O(V + E)
- **Execution Levels**: Group tasks by dependency depth for parallel execution
- **Validation**: Ensure DAG integrity before execution

### Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                      DagBuilder                             │
├──────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
│  │   Task 1   │───▶│   Task 2   │───▶│   Task 3   │  │
│  └─────────────┘    └─────────────┘    └─────────────┘  │
│         │                                      │          │
│         ▼                                      ▼          │
│  ┌─────────────┐                        ┌─────────────┐    │
│  │   Task 4   │                        │   Task 5   │    │
│  └─────────────┘                        └─────────────┘    │
│                                                            │
│  • Build dependency graph                                     │
│  • Detect cycles                                            │
│  • Calculate depths                                          │
│  • Topological sort                                         │
└──────────────────────────────────────────────────────────────┘
```

---

## Core Concepts

### DAG (Directed Acyclic Graph)

A directed graph with no directed cycles, representing task dependencies.

**Properties:**
- **Nodes**: Tasks with unique IDs
- **Edges**: Dependencies between tasks
- **Acyclic**: No circular dependencies
- **Directed**: Dependencies have direction (A → B means A must complete before B)

**Example:**

```
A ──▶ B ──▶ D
 │             ▲
 └────▶ C ────┘

Valid execution orders:
- A, B, C, D
- A, C, B, D
- C, A, B, D
```

### Topological Sort

Linear ordering of nodes where for every directed edge (u → v), u comes before v.

**Properties:**
- Not unique (multiple valid orderings possible)
- Only exists for DAGs (not for cyclic graphs)
- Used for task execution ordering

### Execution Levels

Grouping of tasks that can be executed in parallel.

**Example:**

```
Level 0: A, C (no dependencies, can run in parallel)
Level 1: B (depends on A or C)
Level 2: D (depends on B)
```

---

## DAG Builder

### Creating the Builder

```rust
use cis_core::task::dag::DagBuilder;
use cis_core::task::repository::TaskRepository;
use std::sync::Arc;

let task_repo = Arc::new(TaskRepository::new(pool));
let mut dag_builder = DagBuilder::new(task_repo);
```

### Building a DAG

```rust
// Build DAG from task IDs
let task_ids = vec![
    "task-001".to_string(),
    "task-002".to_string(),
    "task-003".to_string(),
];

let dag = dag_builder.build(&task_ids).await?;
```

**Process:**
1. Load all tasks from repository
2. Resolve dependency IDs
3. Build dependency graph
4. Calculate node depths
5. Detect cycles
6. Identify root nodes

**Returns:**
- `Result<Dag, DagError>`

---

### DagNode

Represents a single node in the DAG.

```rust
pub struct DagNode {
    pub id: i64,                    // Database ID of the task
    pub task_id: String,             // Task identifier
    pub name: String,               // Task name
    pub dependencies: Vec<i64>,     // List of dependency node IDs
    pub dependents: Vec<i64>,       // List of dependent node IDs
    pub depth: usize,               // Depth in the DAG (0 = root)
}
```

**Depth Examples:**

```
A (depth 0)
├── B (depth 1)
│   └── D (depth 2)
└── C (depth 1)
    └── E (depth 2)
```

---

## DAG Structure

### The Dag Type

```rust
pub struct Dag {
    pub nodes: HashMap<i64, DagNode>,  // All nodes in the DAG
    pub roots: Vec<i64>,               // Root node IDs (no dependencies)
}
```

**Accessing Nodes:**

```rust
let dag = dag_builder.build(&task_ids).await?;

// Get all nodes
for (id, node) in &dag.nodes {
    println!("Node {}: {}", id, node.name);
}

// Get root nodes
for root_id in &dag.roots {
    let root_node = &dag.nodes[root_id];
    println!("Root: {}", root_node.name);
}

// Get specific node
if let Some(node) = dag.nodes.get(&node_id) {
    println!("Node: {}", node.name);
    println!("Dependencies: {:?}", node.dependencies);
}
```

---

## Topological Sorting

### Kahn's Algorithm

Generate a linear execution order using Kahn's algorithm.

```rust
let sorted = dag.topological_sort()?;

for (i, node_id) in sorted.iter().enumerate() {
    let node = &dag.nodes[node_id];
    println!("Step {}: {}", i + 1, node.name);
}
```

**Algorithm:**

1. Calculate in-degree for all nodes
2. Add nodes with in-degree 0 to queue
3. While queue not empty:
   - Remove node from queue
   - Add to result
   - Decrease in-degree of dependents
   - Add nodes with in-degree 0 to queue
4. Check if all nodes processed (no cycles)

**Complexity:**
- Time: O(V + E) where V = vertices, E = edges
- Space: O(V)

**Returns:**
- `Result<Vec<i64>, DagError>` - Ordered node IDs

**Error Cases:**
- `DagError::CycleDetected` if cycle exists

---

## Execution Levels

### Get Execution Levels

Group tasks by depth for parallel execution.

```rust
let levels = dag.get_execution_levels();

for (level_num, level_nodes) in levels.iter().enumerate() {
    println!("Level {} ({} tasks):", level_num, level_nodes.len());

    for node_id in level_nodes {
        let node = &dag.nodes[node_id];
        println!("  - {}", node.name);
    }
}
```

**Output Example:**

```
Level 0 (2 tasks):
  - Base Task A
  - Base Task B

Level 1 (1 task):
  - Dependent Task C

Level 2 (1 task):
  - Final Task D
```

**Parallel Execution Strategy:**

```rust
for level in levels {
    // Execute all tasks in this level in parallel
    let futures: Vec<_> = level.iter()
        .map(|node_id| {
            let node = &dag.nodes[node_id];
            execute_task(node)
        })
        .collect();

    // Wait for all tasks in level to complete
    join_all(futures).await?;
}
```

---

## Cycle Detection

### Automatic Detection

Cycles are detected during DAG construction.

```rust
use cis_core::task::dag::DagError;

match dag_builder.build(&task_ids).await {
    Ok(dag) => println!("DAG built successfully"),
    Err(DagError::CycleDetected(cycle_path)) => {
        eprintln!("Cycle detected: {:?}", cycle_path);
        eprintln!("Tasks form a circular dependency!");
    }
    Err(err) => {
        eprintln!("Error: {}", err);
    }
}
```

### Cycle Path Extraction

When a cycle is detected, the full path is returned.

```rust
// Example cycle: A → B → C → A
Err(DagError::CycleDetected(vec![
    "task-a".to_string(),
    "task-b".to_string(),
    "task-c".to_string(),
    "task-a".to_string(),
]))
```

### Manual Cycle Detection

```rust
// Detect cycles in existing DAG
if dag.detect_cycles().is_err() {
    eprintln!("Cycle detected!");
}
```

---

## Scheduler Integration

### Converting to Scheduler DAG

Convert task DAG to scheduler DAG for execution.

```rust
use cis_core::scheduler::core::dag::DagScheduler;

let task_repo = Arc::new(TaskRepository::new(pool));
let mut scheduler = DagScheduler::new(task_repo);

// Build task DAG first
let task_dag = dag_builder.build(&task_ids).await?;

// Convert to scheduler DAG
scheduler.from_task_dag(task_dag).await?;
```

### SchedulerDagNode

Scheduler-specific node with status tracking.

```rust
pub struct SchedulerDagNode {
    pub id: String,                       // Node ID
    pub dependencies: Vec<String>,         // Dependencies
    pub dependents: Vec<String>,          // Dependents
    pub status: DagNodeStatus,            // Execution status
    pub depth: usize,                     // Node depth
}
```

**Status Values:**

```rust
pub enum DagNodeStatus {
    Pending,    // Not yet ready
    Ready,      // Dependencies completed
    Running,    // Currently executing
    Completed,  // Successfully completed
    Failed,     // Execution failed
    Skipped,    // Skipped (e.g., filtered)
    Arbitrated, // Under arbitration
    Debt(u32),  // Technical debt
}
```

### Getting Ready Nodes

Find nodes ready for execution.

```rust
let ready_nodes = scheduler.get_ready_nodes();

for node_id in ready_nodes {
    println!("Ready to execute: {}", node_id);
}
```

**Criteria:**
- Status is `Pending` or `Ready`
- All dependencies completed

### Updating Node Status

Track execution progress.

```rust
// Mark node as running
scheduler.update_node_status("task-001", DagNodeStatus::Running)?;

// Mark as completed
scheduler.update_node_status("task-001", DagNodeStatus::Completed)?;

// Mark as failed
scheduler.update_node_status("task-001", DagNodeStatus::Failed)?;
```

### DAG Statistics

Get execution statistics.

```rust
use cis_core::scheduler::core::dag::DagStats;

let stats = scheduler.get_stats();

println!("Total nodes: {}", stats.total);
println!("Pending: {}", stats.pending);
println!("Running: {}", stats.running);
println!("Completed: {}", stats.completed);
println!("Failed: {}", stats.failed);
```

**Stats Structure:**

```rust
pub struct DagStats {
    pub total: usize,      // Total nodes
    pub pending: usize,     // Pending nodes
    pub ready: usize,      // Ready nodes
    pub running: usize,     // Running nodes
    pub completed: usize,   // Completed nodes
    pub failed: usize,     // Failed nodes
    pub skipped: usize,    // Skipped nodes
    pub arbitrated: usize, // Arbitrated nodes
    pub debt: usize,       // Technical debt
}
```

---

## Usage Examples

### Example 1: Simple Linear Chain

```rust
use cis_core::task::*;
use cis_core::task::dag::*;

#[tokio::main]
async fn main() -> Result<()> {
    let pool = db::create_database_pool(Some("/path/to/db.db"), 5).await;
    let repo = Arc::new(TaskRepository::new(pool.clone()));
    let mut builder = DagBuilder::new(repo.clone());

    // Create tasks: A → B → C
    let task_a = create_task("task-a", "Task A", vec![]);
    let task_b = create_task("task-b", "Task B", vec!["task-a".to_string()]);
    let task_c = create_task("task-c", "Task C", vec!["task-b".to_string()]);

    repo.create(&task_a).await?;
    repo.create(&task_b).await?;
    repo.create(&task_c).await?;

    // Build DAG
    let dag = builder.build(&[
        "task-a".to_string(),
        "task-b".to_string(),
        "task-c".to_string(),
    ]).await?;

    // Topological sort
    let sorted = dag.topological_sort()?;
    assert_eq!(sorted.len(), 3);

    // Get execution levels
    let levels = dag.get_execution_levels();
    assert_eq!(levels.len(), 3); // Each task in its own level

    Ok(())
}
```

---

### Example 2: Parallel Execution

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pool = db::create_database_pool(Some("/path/to/db.db"), 5).await;
    let repo = Arc::new(TaskRepository::new(pool.clone()));
    let mut builder = DagBuilder::new(repo.clone());

    // Create parallel tasks
    let task_a = create_task("task-a", "Base A", vec![]);
    let task_b = create_task("task-b", "Base B", vec![]);
    let task_c = create_task("task-c", "Dependent", vec![
        "task-a".to_string(),
        "task-b".to_string(),
    ]);

    repo.create(&task_a).await?;
    repo.create(&task_b).await?;
    repo.create(&task_c).await?;

    // Build DAG
    let dag = builder.build(&[
        "task-a".to_string(),
        "task-b".to_string(),
        "task-c".to_string(),
    ]).await?;

    // Execute by levels
    let levels = dag.get_execution_levels();

    for (level_idx, level_nodes) in levels.iter().enumerate() {
        println!("Level {}:", level_idx);

        // Execute nodes in parallel
        let futures: Vec<_> = level_nodes.iter()
            .map(|node_id| {
                let node = &dag.nodes[node_id];
                tokio::spawn(execute_task(node.clone()))
            })
            .collect();

        // Wait for completion
        for future in futures {
            future.await??;
        }
    }

    Ok(())
}

async fn execute_task(node: DagNode) -> Result<()> {
    println!("Executing: {}", node.name);
    // Task execution logic
    Ok(())
}
```

---

### Example 3: Cycle Detection

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let pool = db::create_database_pool(Some("/path/to/db.db"), 5).await;
    let repo = Arc::new(TaskRepository::new(pool.clone()));
    let mut builder = DagBuilder::new(repo.clone());

    // Create cyclic dependency: A → B → C → A
    let task_a = create_task("task-a", "Task A", vec!["task-c".to_string()]);
    let task_b = create_task("task-b", "Task B", vec!["task-a".to_string()]);
    let task_c = create_task("task-c", "Task C", vec!["task-b".to_string()]);

    repo.create(&task_a).await?;
    repo.create(&task_b).await?;
    repo.create(&task_c).await?;

    // Attempt to build DAG
    match builder.build(&[
        "task-a".to_string(),
        "task-b".to_string(),
        "task-c".to_string(),
    ]).await {
        Ok(_) => {
            eprintln!("Unexpected: DAG should have cycle!");
        }
        Err(DagError::CycleDetected(path)) => {
            println!("Cycle detected: {:?}", path);
            println!("Path: {} → {} → {} → {}",
                path[0], path[1], path[2], path[3]
            );
        }
        Err(err) => {
            eprintln!("Error: {}", err);
        }
    }

    Ok(())
}
```

---

### Example 4: Dependency Chain

```rust
use cis_core::task::dag::Dag;

fn analyze_dependency_chain(dag: &Dag, task_id: i64) {
    let chain = dag.get_dependency_chain(task_id);

    println!("Dependency chain:");
    for (i, node_id) in chain.iter().enumerate() {
        let node = &dag.nodes[node_id];
        println!("  {}. {}", i + 1, node.name);
    }
}

// Example usage:
let task_id = 5; // Some task ID
let chain = dag.get_dependency_chain(task_id);

// Output:
// Dependency chain:
//   1. Root Task
//   2. Intermediate Task
//   3. Final Task
```

---

## Error Handling

### DagError Types

```rust
pub enum DagError {
    TaskNotFound(String),           // Task doesn't exist
    DependencyNotFound(String),      // Dependency doesn't exist
    CycleDetected(Vec<String>),     // Circular dependency
    DatabaseError(rusqlite::Error),  // Database error
}
```

### Error Handling Examples

```rust
use cis_core::task::dag::DagError;

match dag_builder.build(&task_ids).await {
    Ok(dag) => {
        println!("DAG built with {} nodes", dag.nodes.len());
    }
    Err(DagError::TaskNotFound(id)) => {
        eprintln!("Task not found: {}", id);
    }
    Err(DagError::DependencyNotFound(dep)) => {
        eprintln!("Dependency not found: {}", dep);
        eprintln!("Make sure all dependent tasks are included in the build");
    }
    Err(DagError::CycleDetected(path)) => {
        eprintln!("Circular dependency detected!");
        eprintln!("Cycle path: {}", path.join(" → "));
        eprintln!("Break the cycle by removing one of the dependencies");
    }
    Err(DagError::DatabaseError(err)) => {
        eprintln!("Database error: {}", err);
    }
}
```

---

## Performance Considerations

### Build Performance

**Time Complexity:**
- Building: O(V + E)
- Cycle detection: O(V + E)
- Topological sort: O(V + E)

Where V = number of tasks, E = number of dependencies

**Optimizations:**

1. **Batch Task Loading**
   ```rust
   // Load all tasks in one query
   let tasks = repo.query(TaskFilter {
       task_ids: Some(task_ids.clone()),
       ..Default::default()
   }).await?;
   ```

2. **Cache DAGs**
   ```rust
   // Cache built DAGs for reuse
   let mut dag_cache: HashMap<String, Dag> = HashMap::new();

   let cache_key = task_ids.join(",");
   if let Some(cached) = dag_cache.get(&cache_key) {
       return Ok(cached.clone());
   }

   let dag = dag_builder.build(&task_ids).await?;
   dag_cache.insert(cache_key, dag.clone());
   ```

### Memory Considerations

**Memory Usage:**
- Per node: ~200 bytes
- Total: ~200 * V bytes

For large DAGs (1000+ nodes):
- Consider incremental building
- Use streaming topological sort
- Implement DAG pruning

---

## Best Practices

### DO ✓

1. **Validate dependencies before building**
   ```rust
   // Check that all dependencies exist
   for task_id in &task_ids {
       let task = repo.get_by_task_id(task_id).await?;
       for dep_id in &task.dependencies {
           if repo.get_by_task_id(dep_id).await?.is_none() {
               return Err(format!("Dependency not found: {}", dep_id).into());
           }
       }
   }
   ```

2. **Handle cycles gracefully**
   ```rust
   match dag_builder.build(&task_ids).await {
       Ok(dag) => execute_dag(dag).await,
       Err(DagError::CycleDetected(path)) => {
           eprintln!("Cycle detected, breaking automatically");
           // Remove last dependency to break cycle
           fix_and_retry(task_ids).await
       }
       Err(err) => Err(err.into()),
   }
   ```

3. **Use execution levels for parallelism**
   ```rust
   let levels = dag.get_execution_levels();

   for level in levels {
       let handles: Vec<_> = level.iter()
           .map(|id| tokio::spawn(execute_node(*id)))
           .collect();

       for handle in handles {
           handle.await??;
       }
   }
   ```

### DON'T ✗

1. **Don't ignore cycle detection**
   ```rust
   // ✗ Ignoring cycles
   let dag = dag_builder.build(&task_ids).await.unwrap();

   // ✓ Handle cycles
   let dag = dag_builder.build(&task_ids).await?;
   ```

2. **Don't modify dependencies after building**
   ```rust
   let mut dag = dag_builder.build(&task_ids).await?;

   // ✗ Modifying DAG after build
   dag.nodes.get_mut(&id)?.dependencies.push(new_dep);

   // ✓ Rebuild DAG after changes
   dag_builder.build(&updated_task_ids).await?;
   ```

3. **Don't assume single topological order**
   ```rust
   // ✗ Assuming deterministic order
   let order1 = dag.topological_sort()?;
   let order2 = dag.topological_sort()?;
   assert_eq!(order1, order2);  // May fail!

   // ✓ Both orders are valid
   let order1 = dag.topological_sort()?;
   let order2 = dag.topological_sort()?;
   validate_order(&order1)?;
   validate_order(&order2)?;
   ```

---

## API Reference Summary

### DagBuilder Methods

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `new` | `Arc<TaskRepository>` | `Self` | Create builder |
| `build` | `&[String]` | `Result<Dag>` | Build DAG from task IDs |

### Dag Methods

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `topological_sort` | - | `Result<Vec<i64>>` | Kahn's algorithm |
| `get_execution_levels` | - | `Vec<Vec<i64>>` | Parallel execution levels |
| `get_dependency_chain` | `i64` | `Vec<i64>` | Get dependency path |

### DagScheduler Methods

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `new` | `Arc<TaskRepository>` | `Self` | Create scheduler |
| `from_task_dag` | `Dag` | `Result<()>` | Load from task DAG |
| `get_ready_nodes` | - | `Vec<String>` | Ready for execution |
| `update_node_status` | `&str, DagNodeStatus` | `Result<()>` | Update status |
| `get_stats` | - | `DagStats` | Execution statistics |
| `is_completed` | - | `bool` | All nodes completed |
| `get_failed_nodes` | - | `Vec<String>` | Failed nodes |
| `reset` | - | `()` | Reset all status |

---

**See Also:**
- [Task System API](./task-system.md)
- [Session Management API](./session-management.md)
- [Task Manager API](./task-manager.md)
