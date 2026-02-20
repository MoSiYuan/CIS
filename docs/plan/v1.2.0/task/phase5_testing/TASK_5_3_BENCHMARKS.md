# TASK 5.3: 性能基准测试

> **Phase**: 5 - 测试与验证
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 10

---

## 任务概述

建立全面的性能基准测试体系，对比 v1.1.5 和 v1.2.0 的性能，确保 trait 抽象层开销在可接受范围内（< 5%），并建立持续性能监控机制。

## 工作内容

### 1. 创建基准测试套件

**文件**: `benches/cis_v1_2_0_benchmarks.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

// Memory 操作基准测试
fn benchmark_memory_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // 初始化 Memory 服务
    let memory = rt.block_on(async {
        let storage = cis_storage::RocksDbStorage::new("/tmp/bench_memory").await.unwrap();
        cis_memory::CISMemoryService::new(storage).await.unwrap()
    });
    
    let mut group = c.benchmark_group("memory_operations");
    group.measurement_time(Duration::from_secs(10));
    
    // 基准测试: remember
    group.bench_function("remember", |b| {
        b.to_async(&rt).iter(|| async {
            let entry = MemoryEntry::builder()
                .key(format!("key_{}", rand::random::<u64>()))
                .value(b"test value".to_vec())
                .build()
                .unwrap();
            
            memory.remember(black_box(entry)).await.unwrap();
        });
    });
    
    // 基准测试: recall
    group.bench_function("recall", |b| {
        b.to_async(&rt).iter(|| async {
            let results = memory
                .recall(black_box("test query"), 10)
                .await
                .unwrap();
            black_box(results);
        });
    });
    
    // 基准测试: hybrid_search
    group.bench_function("hybrid_search", |b| {
        b.to_async(&rt).iter(|| async {
            let results = memory
                .hybrid_search(black_box("test query"), 10, None, None)
                .await
                .unwrap();
            black_box(results);
        });
    });
    
    group.finish();
}

// Storage 操作基准测试
fn benchmark_storage_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let storage = rt.block_on(async {
        cis_storage::RocksDbStorage::new("/tmp/bench_storage").await.unwrap()
    });
    
    let mut group = c.benchmark_group("storage_operations");
    group.measurement_time(Duration::from_secs(10));
    
    group.bench_function("set", |b| {
        b.to_async(&rt).iter(|| async {
            storage.set(
                black_box(&format!("key_{}", rand::random::<u64>())),
                black_box(b"test value"),
            ).await.unwrap();
        });
    });
    
    group.bench_function("get", |b| {
        b.to_async(&rt).iter(|| async {
            let value = storage.get(black_box("test_key")).await.unwrap();
            black_box(value);
        });
    });
    
    group.finish();
}

// Scheduler 基准测试
fn benchmark_scheduler_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let scheduler = rt.block_on(async {
        cis_scheduler::CISDagScheduler::new().await.unwrap()
    });
    
    let mut group = c.benchmark_group("scheduler_operations");
    group.measurement_time(Duration::from_secs(10));
    
    // 基准测试: build_dag
    group.bench_function("build_dag_100", |b| {
        b.to_async(&rt).iter(|| async {
            let mut dag = Dag::new();
            for i in 0..100 {
                dag.add_node(Task::builder()
                    .id(format!("task_{}", i))
                    .agent("test")
                    .build()
                    .unwrap()
                ).unwrap();
            }
            black_box(dag);
        });
    });
    
    // 基准测试: topological_sort
    group.bench_function("topological_sort_1000", |b| {
        b.iter(|| {
            let dag = create_test_dag(1000);
            let sorted = dag.topological_sort().unwrap();
            black_box(sorted);
        });
    });
    
    group.finish();
}

// Trait dispatch 开销测试
fn benchmark_trait_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // 具体类型
    let concrete_storage = rt.block_on(async {
        cis_storage::RocksDbStorage::new("/tmp/bench_concrete").await.unwrap()
    });
    
    // Trait 对象
    let trait_storage: Box<dyn StorageService> = Box::new(
        rt.block_on(async {
            cis_storage::RocksDbStorage::new("/tmp/bench_trait").await.unwrap()
        })
    );
    
    let mut group = c.benchmark_group("trait_dispatch_overhead");
    
    group.bench_function("concrete_call", |b| {
        b.to_async(&rt).iter(|| async {
            concrete_storage.set(black_box("key"), black_box(b"value")).await.unwrap();
        });
    });
    
    group.bench_function("trait_object_call", |b| {
        b.to_async(&rt).iter(|| async {
            trait_storage.set(black_box("key"), black_box(b"value")).await.unwrap();
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_memory_operations,
    benchmark_storage_operations,
    benchmark_scheduler_operations,
    benchmark_trait_dispatch,
);
criterion_main!(benches);
```

### 2. 性能对比报告模板

**文件**: `benches/README.md`

```markdown
# 性能基准测试

## 目标

- 确保 trait 抽象层开销 < 5%
- Memory 操作延迟增加 < 10%
- Scheduler 构建时间 < 50ms (1000 tasks)

## 运行基准测试

```bash
cargo bench
```

## 结果记录

### v1.2.0 基准

| 操作 | 耗时 | 相比 v1.1.x |
|------|------|-------------|
| memory.remember | XX ms | ±X% |
| memory.recall | XX ms | ±X% |
| storage.set | XX ms | ±X% |
| dag.build_100 | XX ms | ±X% |
| trait dispatch | XX ns | ±X% |
```

### 3. 持续性能监控

**文件**: `.github/workflows/performance.yml`

```yaml
name: Performance Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Run benchmarks
        run: cargo bench -- --save-baseline pr
      
      - name: Compare with main
        if: github.event_name == 'pull_request'
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench -- --baseline main --load-baseline pr
      
      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/
```

### 4. 内存使用测试

**文件**: `benches/memory_usage.rs`

```rust
//! 内存使用测试

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let allocated = ALLOCATED.fetch_add(size, Ordering::SeqCst) + size;
        PEAK.fetch_max(allocated, Ordering::SeqCst);
        System.alloc(layout)
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

#[test]
fn test_memory_usage() {
    let before = ALLOCATED.load(Ordering::SeqCst);
    
    // 创建 Runtime
    let runtime = create_test_runtime();
    
    let after = ALLOCATED.load(Ordering::SeqCst);
    let used = after - before;
    
    println!("Runtime memory usage: {} KB", used / 1024);
    assert!(used < 100 * 1024 * 1024, "Memory usage should be < 100 MB");
}
```

### 5. 建立性能基线 (Baseline Establishment)

**文件**: `benches/baseline/v1.1.5_baseline.json`

在开始 v1.2.0 开发前，先建立 v1.1.5 的性能基线：

```bash
# 1. 切换到 v1.1.5 分支
git checkout v1.1.5

# 2. 运行完整基准测试
cargo bench -- --save-baseline v1_1_5

# 3. 提取基准数据
cargo bench -- --bench memory_operations -- --save-baseline v1_1_5 --output-format bencher | tee baseline_v1.1.5.txt

# 4. 保存到版本控制
cp -r target/criterion/benches/baseline/v1_1_5 benches/baseline/v1.1.5/
git add benches/baseline/
git commit -m "Add v1.1.5 performance baseline"
```

**基线指标**:

```markdown
## v1.1.5 性能基线

### Memory Operations
| 操作 | 平均耗时 | 最小 | 最大 | 标准差 |
|------|---------|------|------|--------|
| remember | 2.5 ms | 2.1 ms | 3.2 ms | 0.3 ms |
| recall (semantic) | 45 ms | 38 ms | 62 ms | 8 ms |
| hybrid_search | 52 ms | 44 ms | 71 ms | 10 ms |

### Storage Operations
| 操作 | 平均耗时 | 最小 | 最大 | 标准差 |
|------|---------|------|------|--------|
| set | 1.2 ms | 0.9 ms | 1.8 ms | 0.2 ms |
| get | 0.8 ms | 0.6 ms | 1.2 ms | 0.15 ms |
| batch_set (100) | 85 ms | 78 ms | 98 ms | 8 ms |

### Scheduler Operations
| 操作 | 平均耗时 | 最小 | 最大 | 标准差 |
|------|---------|------|------|--------|
| build_dag (100 tasks) | 12 ms | 10 ms | 16 ms | 2 ms |
| build_dag (1000 tasks) | 145 ms | 128 ms | 178 ms | 18 ms |
| execute_dag (parallel) | 850 ms | 780 ms | 980 ms | 75 ms |

### Trait Dispatch Overhead
| 调用方式 | 平均耗时 | 开销 |
|----------|---------|------|
| concrete_call | 1.15 μs | 基准 |
| trait_object_call | 1.19 μs | +3.5% |
| async_trait_call | 1.22 μs | +6.1% |
```

### 6. 性能预算 (Performance Budget)

**文件**: `benches/performance_budget.toml`

定义 v1.2.0 的性能预算（相比 v1.1.5）：

```toml
# 性能预算配置
[performance_budget]
# 允许的性能回归阈值（百分比）
allowed_regression = 5.0

# 关键操作的预算限制
[operations.memory]
remember = { max_increase_pct = 5, baseline_ms = 2.5 }
recall = { max_increase_pct = 8, baseline_ms = 45 }  # 搜索允许略高
hybrid_search = { max_increase_pct = 8, baseline_ms = 52 }

[operations.storage]
set = { max_increase_pct = 5, baseline_ms = 1.2 }
get = { max_increase_pct = 5, baseline_ms = 0.8 }

[operations.scheduler]
build_dag_100 = { max_increase_pct = 5, baseline_ms = 12 }
build_dag_1000 = { max_increase_pct = 5, baseline_ms = 145 }
execute_dag_parallel = { max_increase_pct = 10, baseline_ms = 850 }

[operations.trait]
dispatch_overhead = { max_increase_pct = 10, baseline_ns = 1150 }  # trait 允许略高

# 内存使用预算
[memory]
runtime_peak_mb = 100
memory_per_task_mb = 0.5
```

### 7. 对比指标 (Comparison Metrics)

**文件**: `benches/comparison.rs`

建立 v1.1.5 vs v1.2.0 的对比分析：

```rust
use criterion::{Criterion, BenchmarkId};
use std::collections::HashMap;

struct ComparisonResult {
    operation: String,
    v1_1_5_time: f64,
    v1_2_0_time: f64,
    change_pct: f64,
    within_budget: bool,
}

fn compare_with_baseline(
    c: &mut Criterion,
    operation: &str,
    baseline_ns: f64,
) -> ComparisonResult {
    let mut group = c.benchmark_group(format!("comparison_{}", operation));

    // 运行 v1.2.0 基准测试
    group.bench_function("v1.2.0", |b| {
        b.iter(|| {
            // 执行操作
            black_box(test_operation())
        })
    });

    // 获取 v1.2.0 结果
    let v1_2_0_time = get_average_time(&group, "v1.2.0");

    // 计算变化百分比
    let change_pct = ((v1_2_0_time - baseline_ns) / baseline_ns) * 100.0;

    // 检查是否在预算内
    let within_budget = change_pct <= get_budget_for_operation(operation);

    ComparisonResult {
        operation: operation.to_string(),
        v1_1_5_time: baseline_ns,
        v1_2_0_time,
        change_pct,
        within_budget,
    }
}

fn generate_comparison_report(results: Vec<ComparisonResult>) {
    println!("\n=== v1.1.5 vs v1.2.0 Performance Comparison ===\n");

    println!("{:<20} | {:>12} | {:>12} | {:>10} | {:>8}",
             "Operation", "v1.1.5 (ms)", "v1.2.0 (ms)", "Change", "Budget");
    println!("{:-<20}-|-{:>12}-|-{:>12}-|-{:>10}-|-{:>8}",
             "----------", "------------", "------------", "----------", "--------");

    for result in results {
        let status = if result.within_budget { "✅ PASS" } else { "❌ FAIL" };
        println!("{:<20} | {:>12.2} | {:>12.2} | {:>9.1}% | {:>8}",
                 result.operation,
                 result.v1_1_5_time,
                 result.v1_2_0_time,
                 result.change_pct,
                 status);
    }

    // 汇总统计
    let passed = results.iter().filter(|r| r.within_budget).count();
    let total = results.len();
    println!("\nSummary: {}/{} tests within budget", passed, total);

    if passed < total {
        println!("\n⚠️  WARNING: Some operations exceed performance budget!");
        println!("   Review the failing operations and consider optimization.");
    }
}
```

### 8. CI 性能监控增强

**文件**: `.github/workflows/performance-monitoring.yml`

```yaml
name: Performance Monitoring

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  schedule:
    # 每天凌晨 2 点运行性能监控
    - cron: '0 2 * * *'

jobs:
  benchmark:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable]

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0  # 获取完整历史用于对比

      - name: Install Rust toolchain
        uses: dtolnay/rust-action@stable
        with:
          toolchain: ${{ matrix.rust }}

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-target-${{ hashFiles('**/Cargo.lock') }}

      - name: Run benchmarks (save baseline)
        run: |
          cargo bench --workspace -- \
            --save-baseline main \
            --output-format bencher | tee benchmark_results.txt

      - name: Compare with PR baseline (if PR)
        if: github.event_name == 'pull_request'
        run: |
          git fetch origin ${{ github.base_ref }}
          git checkout origin/${{ github.base_ref }}
          cargo bench --workspace -- \
            --baseline main \
            --load-baseline pr | tee comparison_results.txt

      - name: Check performance budget
        run: |
          cargo run --bin check_performance_budget -- \
            --baseline benchmark_results.txt \
            --budget benches/performance_budget.toml

      - name: Generate performance report
        run: |
          cargo run --bin generate_perf_report -- \
            --results benchmark_results.txt \
            --output perf_report.md

      - name: Upload benchmark results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results-${{ github.sha }}
          path: |
            target/criterion/
            benchmark_results.txt
            comparison_results.txt
            perf_report.md
          retention-days: 30

      - name: Comment PR with results
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v6
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('perf_report.md', 'utf8');

            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: report
            });

      - name: Store historical data
        if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        run: |
          mkdir -p performance-history
          cp benchmark_results.txt performance-history/${{ github.sha }}.txt
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add performance-history/
          git commit -m "Add performance data for ${{ github.sha }}" || true
          git push
```

### 9. 性能回归检测

**文件**: `scripts/check_performance_regression.sh`

```bash
#!/bin/bash
# 性能回归检测脚本

set -e

BASELINE_FILE="benches/baseline/v1.1.5/benchmark_results.txt"
CURRENT_FILE="target/criterion/benchmark_results.txt"
BUDGET_FILE="benches/performance_budget.toml"

echo "🔍 Performance Regression Detection"
echo "===================================="

# 检查基线文件是否存在
if [ ! -f "$BASELINE_FILE" ]; then
    echo "❌ Error: Baseline file not found: $BASELINE_FILE"
    echo "   Run: cargo bench -- --save-baseline v1_1_5"
    exit 1
fi

# 运行基准测试
echo "📊 Running benchmarks..."
cargo bench --workspace -- --output-format bencher | tee "$CURRENT_FILE"

# 对比结果
echo ""
echo "📈 Comparing with baseline..."
cargo run --bin compare_benchmarks -- \
    --baseline "$BASELINE_FILE" \
    --current "$CURRENT_FILE" \
    --budget "$BUDGET_FILE"

# 检查是否通过预算
if [ $? -eq 0 ]; then
    echo ""
    echo "✅ All benchmarks within performance budget!"
    exit 0
else
    echo ""
    echo "❌ Performance regression detected!"
    echo "   Some operations exceed the allowed budget."
    echo "   Please review and optimize before merging."
    exit 1
fi
```

## 验收标准

### 基准测试覆盖
- [ ] Memory 操作基准测试完整（remember, recall, hybrid_search）
- [ ] Storage 操作基准测试完整（set, get, batch）
- [ ] Scheduler 操作基准测试完整（build_dag, execute_dag, topological_sort）
- [ ] Trait dispatch 开销测试完成
- [ ] 内存使用测试通过

### 性能基线
- [ ] v1.1.5 性能基线建立并保存
- [ ] 基线数据提交到版本控制
- [ ] 基线指标文档化（平均耗时、最小、最大、标准差）

### 性能预算
- [ ] 性能预算配置文件创建（performance_budget.toml）
- [ ] 所有关键操作有明确的预算限制
- [ ] 允许的性能回归阈值定义（默认 < 5%）
- [ ] 特殊操作的例外说明（如搜索允许略高）

### CI 集成
- [ ] CI 性能监控 workflow 配置完成
- [ ] PR 自动对比性能并评论
- [ ] 性能回归检测脚本可用
- [ ] 历史性能数据存储机制
- [ ] 定时性能监控任务配置

### 对比分析
- [ ] v1.1.5 vs v1.2.0 对比报告生成
- [ ] 对比指标可视化（表格、图表）
- [ ] 性能回归告警机制
- [ ] 预算超限自动检测

### 性能目标
- [ ] Trait dispatch 开销 < 5%
- [ ] Memory 操作延迟增加 < 8%
- [ ] Storage 操作延迟增加 < 5%
- [ ] DAG 构建时间增加 < 5%（1000 tasks）
- [ ] 并行执行吞吐量无回归
- [ ] 内存使用峰值 < 100 MB

## 依赖

- Task 5.1 (测试框架)
- Task 5.2 (CI 配置)

## 阻塞

- Task 6.2 (发布)

---
