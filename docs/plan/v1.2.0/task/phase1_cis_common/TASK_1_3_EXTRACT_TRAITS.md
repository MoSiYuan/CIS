# TASK 1.3: 提取 cis-traits

> **Phase**: 1 - cis-common 基础
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: f41816b
> **负责人**: TBD
> **周期**: Week 1-2

---

## 任务概述

将核心 trait 提取到独立的 `cis-traits` crate，实现 trait 与实现的分离。

## 工作内容

### 1. 创建 trait 层次结构

```
crates/cis-traits/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── storage.rs    # Storage trait
│   ├── memory.rs     # Memory trait
│   ├── lifecycle.rs  # Lifecycle trait
│   ├── agent.rs      # Agent trait (optional)
│   └── builder.rs    # Builder traits
```

### 2. 定义核心 traits

```rust
// storage.rs
#[async_trait]
pub trait Storage: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
    async fn set(&self, key: &str, value: &[u8]) -> Result<(), StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn scan(&self, prefix: &str) -> Result<Vec<String>, StorageError>;
}

// memory.rs
#[async_trait]
pub trait Memory: Send + Sync + Lifecycle {
    async fn remember(&self, entry: MemoryEntry) -> Result<(), MemoryError>;
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError>;
    async fn forget(&self, id: &MemoryId) -> Result<(), MemoryError>;
    fn namespace(&self) -> &str;
}

// lifecycle.rs
#[async_trait]
pub trait Lifecycle: Send + Sync {
    async fn initialize(&mut self) -> Result<(), LifecycleError>;
    async fn shutdown(&mut self) -> Result<(), LifecycleError>;
    fn is_initialized(&self) -> bool;
}
```

### 3. 定义 Error 类型

```rust
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("key not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("other: {0}")]
    Other(String),
}
```

### 4. 处理 feature flags

```toml
[features]
default = []
agent = ["dep:async-trait"]
multi-agent = ["agent"]
```

## 验收标准

- [ ] 核心 traits 成功提取
- [ ] Error 类型完整定义
- [ ] 支持 no_std 环境
- [ ] 文档包含 trait 使用示例
- [ ] 向后兼容层正常工作

## 依赖

- Task 1.1 (创建 cis-common)
- Task 1.2 (提取 cis-types)

## 阻塞

- Task 2.x (所有模块提取任务)

---
