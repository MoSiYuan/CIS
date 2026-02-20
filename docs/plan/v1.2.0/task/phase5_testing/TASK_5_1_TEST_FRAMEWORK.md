# TASK 5.1: 测试框架搭建

> **Phase**: 5 - 测试与验证
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 9

---

## 任务概述

搭建完整的测试框架，包括单元测试、集成测试和性能测试。

## 工作内容

### 1. 配置测试依赖

```toml
# crates/*/Cargo.toml [dev-dependencies]
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.12"
tempfile = "3.0"
claim = "0.5"  # 断言宏
pretty_assertions = "1.4"
criterion = { version = "0.5", features = ["async_tokio"] }
```

### 2. 单元测试策略

每个 crate 的测试结构：
```
crates/cis-memory/
├── src/
│   └── ...
└── tests/
    ├── unit/              # 单元测试
    │   ├── memory_test.rs
    │   ├── entry_test.rs
    │   └── index_test.rs
    ├── integration/       # 集成测试
    │   ├── storage_integration.rs
    │   └── embedding_integration.rs
    └── fixtures/          # 测试数据
        └── sample_memories.json
```

### 3. Mock 实现

```rust
// crates/cis-traits/src/mock.rs
#[cfg(any(test, feature = "mock"))]
pub mod mock {
    use mockall::mock;
    
    mock! {
        pub Storage {}
        
        #[async_trait]
        impl Storage for Storage {
            async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
            async fn set(&self, key: &str, value: &[u8]) -> Result<(), StorageError>;
            async fn delete(&self, key: &str) -> Result<(), StorageError>;
        }
    }
    
    mock! {
        pub Memory {}
        
        #[async_trait]
        impl Memory for Memory {
            async fn remember(&self, entry: MemoryEntry) -> Result<(), MemoryError>;
            async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError>;
            fn namespace(&self) -> &str;
        }
    }
}
```

### 4. 集成测试场景

```rust
// tests/integration/full_workflow.rs
#[tokio::test]
async fn test_end_to_end_task_execution() {
    // 搭建测试环境
    let temp_dir = TempDir::new().unwrap();
    let storage = RocksDbStorage::new(temp_dir.path());
    let memory = CISMemory::new(storage.clone(), "test", mock_embedding());
    let scheduler = PriorityScheduler::new(4);
    
    let runtime = Runtime::builder()
        .with_storage(storage)
        .with_memory(memory)
        .with_scheduler(scheduler)
        .build()
        .unwrap();
    
    // 执行任务
    let task = Task::builder()
        .description("test task")
        .build()
        .unwrap();
    
    let result = runtime.execute(task).await;
    assert!(result.is_ok());
}
```

### 5. 性能测试

```rust
// benches/memory_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_remember(c: &mut Criterion) {
    let runtime = setup_runtime();
    
    c.bench_function("memory_remember", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let entry = create_test_entry();
                runtime.memory().remember(black_box(entry)).await.unwrap();
            })
    });
}

criterion_group!(benches, benchmark_remember);
criterion_main!(benches);
```

## 验收标准

- [ ] 所有 crates 有单元测试覆盖
- [ ] 集成测试覆盖主要场景
- [ ] Mock 实现可用
- [ ] 性能测试基线建立
- [ ] CI 测试流水线配置
- [ ] 代码覆盖率 > 80%

## 依赖

- Task 3.2 (cis-core 重构)

## 阻塞

- Task 5.2 (CI 配置)

---
