# TASK 4.3: ZeroClaw 集成测试

> **Phase**: 4 - ZeroClaw 兼容
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 8-9

---

## 任务概述

编写 ZeroClaw 集成测试，验证 CIS 作为 ZeroClaw backend 的功能正确性。

## 工作内容

### 1. 创建集成测试文件

**文件**: `cis-core/tests/zeroclaw_integration.rs`

```rust
#![cfg(feature = "zeroclaw")]

use cis_core::zeroclaw::{ZeroclawMemoryAdapter, ZeroclawSchedulerAdapter};
use cis_memory::CISMemoryService;
use cis_storage::RocksDbStorage;
use zeroclaw::memory::{Memory, MemoryCategory, MemoryEntry};
use zeroclaw::scheduler::{Scheduler, Task as ZcTask};

#[tokio::test]
async fn test_zeroclaw_memory_adapter_basic() {
    // 创建 CIS Memory 服务
    let storage = RocksDbStorage::new("sqlite::memory:").await.unwrap();
    let cis_memory = CISMemoryService::new(storage).await.unwrap();
    
    // 创建 ZeroClaw 适配器
    let adapter = ZeroclawMemoryAdapter::new(cis_memory);
    
    // 测试 store
    adapter.store(
        "test_key",
        "test_value",
        MemoryCategory::Core,
        None,
    ).await.unwrap();
    
    // 测试 recall
    let results = adapter.recall("test", 10, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "test_key");
}

#[tokio::test]
async fn test_zeroclaw_memory_adapter_session() {
    let storage = RocksDbStorage::new("sqlite::memory:").await.unwrap();
    let cis_memory = CISMemoryService::new(storage).await.unwrap();
    let adapter = ZeroclawMemoryAdapter::new(cis_memory);
    
    // 存储带 session_id 的记忆
    adapter.store(
        "session_key",
        "session_value",
        MemoryCategory::Conversation,
        Some("session_001"),
    ).await.unwrap();
    
    // 按 session 召回
    let results = adapter
        .recall_with_session("session", 10, "session_001")
        .await
        .unwrap();
    
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_zeroclaw_memory_adapter_categories() {
    let storage = RocksDbStorage::new("sqlite::memory:").await.unwrap();
    let cis_memory = CISMemoryService::new(storage).await.unwrap();
    let adapter = ZeroclawMemoryAdapter::new(cis_memory);
    
    // 存储不同 category 的记忆
    adapter.store(
        "core_key",
        "core_value",
        MemoryCategory::Core,
        None,
    ).await.unwrap();
    
    adapter.store(
        "daily_key",
        "daily_value",
        MemoryCategory::Daily,
        None,
    ).await.unwrap();
    
    // 按 category 过滤
    let core_results = adapter
        .recall_with_category("value", 10, MemoryCategory::Core)
        .await
        .unwrap();
    
    assert_eq!(core_results.len(), 1);
    assert_eq!(core_results[0].key, "core_key");
}

#[tokio::test]
async fn test_zeroclaw_scheduler_adapter() {
    let scheduler = cis_scheduler::CISJobScheduler::new().await.unwrap();
    let adapter = ZeroclawSchedulerAdapter::new(scheduler);
    
    // 创建 ZeroClaw 任务
    let zc_task = ZcTask::new("test_task")
        .with_description("Test task")
        .with_level(zeroclaw::scheduler::TaskLevel::Auto);
    
    // 调度任务
    let result = adapter.schedule(zc_task).await.unwrap();
    assert!(result.task_id.is_some());
}
```

### 2. 创建测试工具

**文件**: `cis-core/tests/common/mod.rs`

```rust
// 测试工具函数

pub async fn setup_test_memory() -> CISMemoryService {
    let storage = RocksDbStorage::new("sqlite::memory:").await.unwrap();
    CISMemoryService::new(storage).await.unwrap()
}

pub async fn setup_test_zeroclaw_adapter() -> ZeroclawMemoryAdapter {
    let memory = setup_test_memory().await;
    ZeroclawMemoryAdapter::new(memory)
}
```

### 3. 添加端到端测试

**文件**: `cis-core/tests/e2e/zeroclaw_workflow.rs`

```rust
//! ZeroClaw 端到端工作流测试

#![cfg(feature = "zeroclaw")]

use cis_core::Runtime;

#[tokio::test]
async fn test_e2e_zeroclaw_backend_workflow() {
    // 1. 创建 CIS Runtime
    let runtime = Runtime::builder()
        .with_storage(RocksDbStorage::new("sqlite::memory:").await.unwrap())
        .with_memory(CISMemoryService::new(...).await.unwrap())
        .with_scheduler(CISJobScheduler::new().await.unwrap())
        .build()
        .unwrap();
    
    // 2. 创建 ZeroClaw 适配器
    let memory_adapter = ZeroclawMemoryAdapter::new(runtime.memory().clone());
    let scheduler_adapter = ZeroclawSchedulerAdapter::new(runtime.scheduler().clone());
    
    // 3. 模拟 ZeroClaw Agent 使用 CIS backend
    // 存储记忆
    memory_adapter.store(
        "user_pref_theme",
        "dark",
        MemoryCategory::Core,
        Some("session_001"),
    ).await.unwrap();
    
    // 召回记忆
    let prefs = memory_adapter
        .recall("theme", 5, Some("session_001"))
        .await
        .unwrap();
    
    assert!(!prefs.is_empty());
    
    // 4. 调度任务
    let task = ZcTask::new("process_request")
        .with_input(&prefs[0].content);
    
    let result = scheduler_adapter.schedule(task).await.unwrap();
    assert_eq!(result.status, TaskStatus::Completed);
}
```

### 4. 配置 CI 集成测试

**文件**: `.github/workflows/zeroclaw-integration.yml`

```yaml
name: ZeroClaw Integration Tests

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Run ZeroClaw integration tests
        run: cargo test --package cis-core --features zeroclaw --test zeroclaw_integration
      
      - name: Run e2e tests
        run: cargo test --package cis-core --features zeroclaw --test e2e
```

### 5. 性能对比测试

**文件**: `cis-core/benches/zeroclaw_adapter_overhead.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_memory_adapter_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // 原生 CIS Memory
    let cis_memory = rt.block_on(async {
        CISMemoryService::new(...).await.unwrap()
    });
    
    // ZeroClaw Adapter
    let adapter = ZeroclawMemoryAdapter::new(cis_memory);
    
    c.bench_function("zeroclaw_adapter_store", |b| {
        b.to_async(&rt).iter(|| async {
            adapter.store(
                black_box("key"),
                black_box("value"),
                MemoryCategory::Core,
                None,
            ).await.unwrap();
        });
    });
}

criterion_group!(benches, benchmark_memory_adapter_overhead);
criterion_main!(benches);
```

## 验收标准

- [ ] Memory Adapter 测试通过
- [ ] Scheduler Adapter 测试通过
- [ ] 端到端工作流测试通过
- [ ] 类型映射测试完整
- [ ] CI 集成测试配置完成
- [ ] 性能开销 < 20%

## 依赖

- Task 4.1 (适配层实现)
- Task 4.2 (E2E 验证)

## 阻塞

- Task 5.1 (测试框架)

---
