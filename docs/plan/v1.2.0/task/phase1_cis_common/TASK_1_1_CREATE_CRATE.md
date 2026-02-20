# TASK 1.1: 创建 cis-common crate

> **Phase**: 1 - cis-common 基础
> **状态**: 🔄 进行中
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

创建新的 `cis-common` crate 作为所有 CIS 组件共享的基础库。

## 工作内容

### 1. 创建 crate 结构

```
crates/cis-common/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── peer.rs      # PeerId, DeviceId
│   │   ├── task.rs      # TaskId, TaskType
│   │   └── priority.rs  # Priority enum
│   └── utils/
│       ├── mod.rs
│       └── crypto.rs    # Shared crypto primitives
```

### 2. 配置 Cargo.toml

```toml
[package]
name = "cis-common"
version = "0.1.0"
edition = "2021"

[features]
default = ["std"]
std = []
no_std = ["alloc"]

[dependencies]
serde = { version = "1.0", default-features = false, features = ["derive"] }
thiserror = { version = "1.0", optional = true }
```

### 3. 迁移现有类型

从 `cis-core/src/common/` 迁移：
- `PeerId` 基础定义
- `TaskId` 基础定义
- `TaskType` 枚举
- 优先级常量

### 4. 设计 Error 类型

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CommonError {
    InvalidPeerId(String),
    InvalidTaskId(String),
    SerializationError(String),
}

#[cfg(feature = "std")]
impl std::error::Error for CommonError {}
```

## 验收标准

- [ ] crate 可被编译
- [ ] 所有基础类型定义完成
- [ ] Error 类型支持 std/no_std
- [ ] 单元测试覆盖率 > 80%
- [ ] 文档完整

## 依赖

- 无

## 阻塞

- Task 1.2 (提取 cis-types)
- Task 1.3 (提取 cis-traits)

---
