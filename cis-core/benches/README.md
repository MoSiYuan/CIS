# CIS v1.1.6 Core Module Benchmarks

Comprehensive performance benchmarks for CIS core modules using criterion.rs.

## Overview

This benchmark suite measures critical performance metrics for CIS v1.1.6 core modules:
- Database operations
- DAG operations
- WeeklyArchivedMemory
- Task Manager

## Running Benchmarks

### Run All Benchmarks

```bash
cd cis-core
cargo bench
```

### Run Specific Benchmark

```bash
# Database operations
cargo bench --bench database_operations

# DAG operations
cargo bench --bench dag_operations

# WeeklyArchivedMemory
cargo bench --bench weekly_archived_memory

# Task Manager
cargo bench --bench task_manager
```

### Generate Plots

```bash
# Generate comparison plots
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main
```

## Benchmark Suites

### 1. Database Operations Benchmark

**File**: `database_operations.rs` (~399 lines)

**Benchmarks**:
- `db_insert_single` - Single task insertion performance
- `db_insert_batch` - Batch insertion (10, 50, 100, 500, 1000 records)
- `db_query_simple` - Simple query by ID
- `db_query_complex` - Complex queries with filters and LIKE patterns
- `db_update` - Single and batch update operations
- `db_delete` - Single and batch delete operations
- `db_transaction` - Transaction overhead comparison

**Key Metrics**:
- Insert throughput (records/sec)
- Query latency (mean, median, stddev)
- Transaction overhead

**Configuration**:
- WAL mode enabled
- In-memory temp store
- Batch sizes: 10, 50, 100, 500, 1000

---

### 2. DAG Operations Benchmark

**File**: `dag_operations.rs` (~451 lines)

**Benchmarks**:
- `dag_construct_linear` - Linear chain DAG construction (10, 50, 100, 500, 1000 nodes)
- `dag_construct_tree` - Tree structure DAG (various fan-out/depth)
- `dag_construct_random` - Random dependency DAG construction
- `dag_topological_sort` - Topological sorting performance
- `dag_validation` - Cycle detection and validation
- `dag_diamond` - Diamond pattern DAG (multiple merging paths)
- `dag_batch_add` - Batch node addition

**Key Metrics**:
- Node construction throughput
- Topological sort time
- Cycle detection latency
- Memory usage patterns

**Test Patterns**:
- Linear chains (1→2→3→...→n)
- Trees (fan-out: 2-10, depth: 2-5)
- Diamond graphs (multiple parallel paths)
- Random dependencies (cycle-free)

---

### 3. WeeklyArchivedMemory Benchmark

**File**: `weekly_archived_memory.rs` (~456 lines)

**Benchmarks**:
- `memory_write_single` - Single write (small: 1KB, large: 10KB)
- `memory_write_batch` - Batch writes (10, 50, 100, 500 items)
- `memory_read` - Key-based read operations
- `memory_vector_search` - Vector/embedding-based search
- `memory_index_lookup` - Index lookup (user preferences, project configs)
- `memory_cross_week_query` - Cross-week query patterns
- `memory_classification` - Memory entry classification (indexing decision)
- `memory_mixed_ops` - Mixed read/write/search workload
- `memory_domain_ops` - Public vs Private domain operations
- `memory_index_strategy` - Index strategy evaluation

**Key Metrics**:
- Write throughput (bytes/sec, records/sec)
- Read latency
- Search precision and recall
- Index hit rate
- Cross-week query performance

**Memory Types**:
- User preferences (indexed)
- Project configs (indexed)
- Sensitive data (not indexed)
- Temporary data (not indexed)

---

### 4. Task Manager Benchmark

**File**: `task_manager.rs` (~589 lines)

**Benchmarks**:
- `task_manager_team_matching` - Team/capability matching algorithm
- `task_manager_assignment` - Task assignment with varying team pool sizes (5, 10, 20, 50)
- `task_manager_execution_plan` - Execution plan generation from DAG (10, 50, 100, 500 tasks)
- `task_manager_status` - Task status tracking and updates
- `task_manager_priority` - Priority-based scheduling (10, 50, 100 tasks)
- `task_manager_concurrent` - Concurrent task assignment simulation
- `task_manager_retry` - Task retry eligibility check
- `task_manager_dependencies` - DAG dependency resolution
- `task_manager_metadata` - Task metadata handling (5, 20, 50 entries)

**Key Metrics**:
- Team matching latency
- Assignment throughput
- Execution plan generation time
- Dependency resolution performance
- Concurrent operation scalability

**Team Simulation**:
- Rust team (rust, compilation)
- Python team (python, ml)
- General team (general)
- Deployment team (deploy, docker)
- Test team (test, qa)

---

## Benchmark Configuration

All benchmarks use consistent criterion.rs configuration:

```rust
Criterion::default()
    .warm_up_time(Duration::from_secs(2))
    .measurement_time(Duration::from_secs(5))
    .sample_size(50-100)
```

### Statistical Analysis

Each benchmark provides:
- **Mean**: Average execution time
- **Median**: 50th percentile
- **StdDev**: Standard deviation
- **Min/Max**: Range of measurements
- **Confidence Intervals**: 95% confidence

---

## Expected Results

### Database Operations

| Operation | Target Performance |
|-----------|------------------|
| Single Insert | < 100μs |
| Batch Insert (100) | < 10ms |
| Simple Query | < 50μs |
| Complex Query | < 1ms |
| Update | < 100μs |

### DAG Operations

| Operation | Nodes | Target |
|-----------|--------|--------|
| Construction | 100 | < 1ms |
| Construction | 1000 | < 20ms |
| Topological Sort | 500 | < 5ms |
| Validation | 1000 | < 10ms |

### WeeklyArchivedMemory

| Operation | Size | Target |
|-----------|-------|--------|
| Single Write | 1KB | < 200μs |
| Batch Write | 100 | < 50ms |
| Read | - | < 100μs |
| Vector Search | 1000 items | < 10ms |

### Task Manager

| Operation | Teams/Tasks | Target |
|-----------|-------------|--------|
| Team Matching | 10 teams | < 50μs |
| Execution Plan | 100 tasks | < 5ms |
| Priority Sort | 100 tasks | < 1ms |
| Dependency Resolution | 500 tasks | < 10ms |

---

## Integration with CI/CD

Add to CI pipeline:

```yaml
benchmarks:
  script:
    - cargo bench --bench database_operations -- --save-baseline ci
    - cargo bench --bench dag_operations -- --save-baseline ci
    - cargo bench --bench weekly_archived_memory -- --save-baseline ci
    - cargo bench --bench task_manager -- --save-baseline ci
  artifacts:
    paths:
      - target/criterion/
```

---

## Profiling Integration

Combine with flamegraph for deeper analysis:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bench database_operations

# View
open flamegraph.svg
```

---

## Benchmark Output Location

Results are saved to:
```
cis-core/target/criterion/
├── database_operations/
│   ├── insert/
│   ├── query/
│   └── report/index.html
├── dag_operations/
├── weekly_archived_memory/
└── task_manager/
```

Open `target/criterion/report/index.html` in a browser to view all results.

---

## Performance Regression Detection

Compare against baseline:

```bash
# Save baseline
cargo bench -- --save-baseline v1.1.5

# Run new benchmarks
cargo bench

# Compare
cargo bench -- --baseline v1.1.5
```

Criterion will warn if performance regresses by > 5%.

---

## Troubleshooting

### Benchmarks Too Noisy

Increase measurement time:
```rust
.measurement_time(Duration::from_secs(10))
.sample_size(200)
```

### Out of Memory

Reduce batch sizes or test data in benchmark functions.

### Slow Benchmarks

Disable debug assertions:
```bash
cargo bench --release
```

---

## Contributing

When adding new benchmarks:

1. Use descriptive names: `bench_operation_description`
2. Include multiple input sizes
3. Add warm-up and measurement time
4. Document expected results in this README
5. Update Cargo.toml with new benchmark target

---

## License

MIT License - See CIS project root for details.
