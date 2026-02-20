# TASK 5.3: 性能基准测试

> **Phase**: 5 - 测试与验证
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 10

---

## 任务概述

建立性能基准测试，对比重构前后的性能，确保 trait 抽象层开销在可接受范围内。

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

## 验收标准

- [ ] 基准测试覆盖 Memory/Storage/Scheduler
- [ ] Trait dispatch 开销 < 5%
- [ ] CI 性能监控配置完成
- [ ] 性能回归检测可用
- [ ] 内存使用测试通过

## 依赖

- Task 5.1 (测试框架)
- Task 5.2 (CI 配置)

## 阻塞

- Task 6.2 (发布)

---
