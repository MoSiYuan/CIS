# CIS v1.1.6 Performance Benchmarks Summary

## Overview

Created comprehensive performance benchmarks for CIS v1.1.6 core modules using criterion.rs framework.

**Total Benchmark Code**: 1,895 lines across 4 benchmark suites

**Location**: `/Users/jiangxiaolong/work/project/CIS/cis-core/benches/`

## Benchmark Suites

### 1. Database Operations Benchmark

**File**: `database_operations.rs` (399 lines)

**Purpose**: Measure SQLite database performance for task storage and retrieval.

**Key Benchmarks**:

| Benchmark | Description | Variants |
|-----------|-------------|-----------|
| `db_insert_single` | Single task insertion | - |
| `db_insert_batch` | Batch insertion | 10, 50, 100, 500, 1000 records |
| `db_query_simple` | Query by ID | - |
| `db_query_complex` | Complex queries with filters | LIKE, multi-condition |
| `db_update` | Update operations | Single, batch |
| `db_delete` | Delete operations | Single, batch |
| `db_transaction` | Transaction overhead | With/without transaction |

**Performance Targets**:
- Single insert: < 100μs
- Batch insert (100): < 10ms
- Simple query: < 50μs
- Complex query: < 1ms

**Database Configuration**:
```rust
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA temp_store = memory;
PRAGMA locking_mode = EXCLUSIVE;
```

---

### 2. DAG Operations Benchmark

**File**: `dag_operations.rs` (451 lines)

**Purpose**: Measure DAG construction, validation, and topological sorting performance.

**Key Benchmarks**:

| Benchmark | Description | Scale |
|-----------|-------------|--------|
| `dag_construct_linear` | Linear chain DAG | 10, 50, 100, 500, 1000 nodes |
| `dag_construct_tree` | Tree structure DAG | Fan-out: 2-10, Depth: 2-5 |
| `dag_construct_random` | Random dependencies | 10, 50, 100, 500 nodes |
| `dag_topological_sort` | Execution order generation | 10-1000 nodes |
| `dag_validation` | Cycle detection | 10-1000 nodes |
| `dag_diamond` | Diamond pattern (merge) | Width: 5-20, Depth: 2-3 |
| `dag_batch_add` | Batch node addition | 10, 50, 100, 500 nodes |

**Performance Targets**:
- Construction (100 nodes): < 1ms
- Construction (1000 nodes): < 20ms
- Topological sort (500 nodes): < 5ms
- Validation (1000 nodes): < 10ms

**DAG Patterns Tested**:
1. **Linear Chain**: Sequential dependencies (1→2→3→...→n)
2. **Tree**: Fan-out patterns (root → children → grandchildren)
3. **Diamond**: Parallel paths merging (A→[B,C]→D)
4. **Random**: Complex dependency graphs (cycle-free)

---

### 3. WeeklyArchivedMemory Benchmark

**File**: `weekly_archived_memory.rs` (456 lines)

**Purpose**: Benchmark the weekly archived memory system with vector indexing.

**Key Benchmarks**:

| Benchmark | Description | Variants |
|-----------|-------------|-----------|
| `memory_write_single` | Single write | Small (1KB), Large (10KB) |
| `memory_write_batch` | Batch writes | 10, 50, 100, 500 items |
| `memory_read` | Key-based read | - |
| `memory_vector_search` | Vector/embedding search | 100, 500, 1000 items |
| `memory_index_lookup` | Index lookup | User preferences, project configs |
| `memory_cross_week_query` | Cross-week queries | Current week |
| `memory_classification` | Entry classification | User pref, sensitive, config, temp |
| `memory_mixed_ops` | Mixed workload | Write + read + search |
| `memory_domain_ops` | Domain operations | Public vs Private |
| `memory_index_strategy` | Index strategy evaluation | - |

**Performance Targets**:
- Single write (1KB): < 200μs
- Batch write (100): < 50ms
- Read: < 100μs
- Vector search (1000 items): < 10ms

**Memory Types Tested**:
- **User Preferences**: Indexed (`user/preference/*`)
- **Project Configs**: Indexed (`project/*`, `config/*`)
- **Sensitive Data**: Not indexed (`secret/*`, `api_key/*`)
- **Temporary Data**: Not indexed (`temp/*`, `cache/*`)

**Index Strategy**:
- Target index ratio: ~10%
- Minimum importance: 0.7
- Max index entries: 10,000

---

### 4. Task Manager Benchmark

**File**: `task_manager.rs` (589 lines)

**Purpose**: Benchmark team matching, task assignment, and execution planning.

**Key Benchmarks**:

| Benchmark | Description | Scale |
|-----------|-------------|--------|
| `task_manager_team_matching` | Team/capability matching | Single/multi capability |
| `task_manager_assignment` | Task assignment | 5, 10, 20, 50 teams |
| `task_manager_execution_plan` | Plan generation | 10, 50, 100, 500 tasks |
| `task_manager_status` | Status tracking | Create, update |
| `task_manager_priority` | Priority scheduling | 10, 50, 100 tasks |
| `task_manager_concurrent` | Concurrent assignment | 50 tasks, 10 teams |
| `task_manager_retry` | Retry eligibility check | - |
| `task_manager_dependencies` | DAG dependency resolution | 10-500 tasks |
| `task_manager_metadata` | Metadata handling | 5, 20, 50 entries |

**Performance Targets**:
- Team matching (10 teams): < 50μs
- Execution plan (100 tasks): < 5ms
- Priority sort (100 tasks): < 1ms
- Dependency resolution (500 tasks): < 10ms

**Simulated Teams**:
1. **Rust Team**: Capabilities = [rust, compilation], Capacity = 5
2. **Python Team**: Capabilities = [python, ml], Capacity = 3
3. **General Team**: Capabilities = [general], Capacity = 10
4. **Deployment Team**: Capabilities = [deploy, docker], Capacity = 4
5. **Test Team**: Capabilities = [test, qa], Capacity = 8

---

## Benchmark Configuration

All benchmarks use consistent criterion.rs settings:

```rust
Criterion::default()
    .warm_up_time(Duration::from_secs(2))  // Warm-up runs
    .measurement_time(Duration::from_secs(5)) // Measurement duration
    .sample_size(50-100)                    // Number of samples
```

### Statistical Analysis

Each benchmark generates:
- **Mean**: Average execution time
- **Median**: 50th percentile (more stable than mean)
- **StdDev**: Standard deviation (measure of consistency)
- **Min/Max**: Range of observed values
- **Confidence Intervals**: 95% confidence level

### Output Format

Results saved to: `cis-core/target/criterion/`

For each benchmark:
- Raw data: `.csv` files
- Statistics: `json` files
- Plots: `svg` charts
- Report: `index.html`

---

## Running the Benchmarks

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

### Run Specific Test Within Benchmark

```bash
cargo bench --bench database_operations -- db_insert_single
```

### Generate Comparison Plots

```bash
# Save baseline
cargo bench -- --save-baseline main

# Compare
cargo bench -- --baseline main
```

### View Results

```bash
open target/criterion/report/index.html
```

---

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench -- --save-baseline ci

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: target/criterion/report/index.html
```

---

## Performance Regression Detection

Criterion automatically detects performance regressions:

```bash
# Compare against previous baseline
cargo bench -- --baseline v1.1.5

# Criterion will warn if:
# - New implementation is > 5% slower
# - Confidence intervals don't overlap
```

---

## Benchmark Files Structure

```
cis-core/benches/
├── README.md                      # This file
├── database_operations.rs           # Database benchmarks (399 lines)
├── dag_operations.rs              # DAG benchmarks (451 lines)
├── weekly_archived_memory.rs      # Memory benchmarks (456 lines)
├── task_manager.rs               # Task Manager benchmarks (589 lines)
├── agent_teams_benchmarks.rs     # Existing agent teams benchmarks
├── dag_execution.rs              # Existing DAG execution benchmarks
├── skill_dispatch.rs             # Existing skill dispatch benchmarks
└── wasm_runtime.rs              # Existing WASM runtime benchmarks
```

---

## Dependencies

All benchmarks require:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }
tempfile = "3"
tokio = { version = "1.35", features = ["rt-multi-thread", "macros"] }
```

---

## Key Design Decisions

### 1. Criterion.rs Framework

**Why**: Provides statistical analysis, warm-up runs, and regression detection.

**Benefits**:
- Automatic warm-up (cold start elimination)
- Statistical confidence intervals
- HTML reports with plots
- Baseline comparison

### 2. Temporary Databases

**Why**: Isolate benchmarks from development data.

**Implementation**: Use `tempfile::TempDir` for clean test environments.

### 3. Multiple Input Sizes

**Why**: Performance characteristics vary with scale.

**Approach**: Test small (10), medium (100), large (1000) scales.

### 4. Async Benchmarks

**Why**: Core modules use Tokio runtime.

**Implementation**: Use `criterion/async_tokio` feature for fair async measurement.

---

## Future Improvements

### Potential Additions

1. **Memory Profiling**: Integrate with `heaptrack` or `valgrind`
2. **CPU Profiling**: Use `flamegraph` for hotspot analysis
3. **Cache Analysis**: Measure cache hit/miss ratios
4. **Stress Tests**: Sustained load benchmarks
5. **Network I/O**: When P2P features are complete

### Continuous Benchmarking

- Run benchmarks nightly on CI
- Track performance over time
- Alert on regressions > 10%

---

## Troubleshooting

### Issue: Benchmarks Too Noisy

**Solution**: Increase measurement time and sample size

```rust
.measurement_time(Duration::from_secs(10))
.sample_size(200)
```

### Issue: Out of Memory

**Solution**: Reduce test data sizes or batch operations

### Issue: Compilation Errors

**Check**:
- CIS core module compiles: `cargo check`
- All dependencies present in `Cargo.toml`
- Feature flags enabled: `cargo bench --features "encryption,vector"`

---

## Maintenance

When updating core modules:

1. **Update Imports**: Add new types/structs to benchmark imports
2. **Add Benchmarks**: For new public APIs
3. **Update Targets**: Revise performance expectations
4. **Run CI**: Ensure no regressions
5. **Update Docs**: Document any new benchmarks

---

## License

MIT License - See CIS project root for details.

---

**Version**: v1.1.6
**Last Updated**: 2026-02-13
**Total Lines**: 1,895 (excluding existing benchmarks)
**Total Benchmarks**: 35+ individual test cases
