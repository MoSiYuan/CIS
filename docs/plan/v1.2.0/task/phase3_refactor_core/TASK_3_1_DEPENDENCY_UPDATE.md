# TASK 3.1: 更新 cis-core 依赖

> **Phase**: 3 - cis-core 重构
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 6

---

## 任务概述

更新 `cis-core` 以依赖新提取的 crates，移除重复代码。

## 工作内容

### 1. 更新 Cargo.toml

```toml
[dependencies]
# 新提取的 crates
cis-common = { path = "../cis-common" }
cis-types = { path = "../cis-types" }
cis-traits = { path = "../cis-traits" }
cis-storage = { path = "../cis-storage" }
cis-memory = { path = "../cis-memory" }
cis-scheduler = { path = "../cis-scheduler" }
cis-vector = { path = "../cis-vector" }
cis-p2p = { path = "../cis-p2p" }

# 保留的核心依赖
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

### 2. 移除重复模块

删除以下已迁移的模块：
```
cis-core/src/common/     -> 使用 cis-common
cis-core/src/types/      -> 使用 cis-types
cis-core/src/traits/     -> 使用 cis-traits
cis-core/src/storage/    -> 使用 cis-storage
cis-core/src/memory/     -> 使用 cis-memory
cis-core/src/scheduler/  -> 使用 cis-scheduler
cis-core/src/vector/     -> 使用 cis-vector
cis-core/src/p2p/        -> 使用 cis-p2p
```

### 3. 创建兼容性重导出

```rust
// cis-core/src/lib.rs
// 为了保持向后兼容，重导出所有类型

pub use cis_common::*;
pub use cis_types::*;
pub use cis_traits::*;
pub use cis_storage::*;
pub use cis_memory::*;
pub use cis_scheduler::*;
pub use cis_vector::*;
pub use cis_p2p::*;

// 保留 cis-core 特有的实现
pub mod orchestration;
pub mod runtime;
pub mod context;
```

### 4. 更新内部引用

```rust
// 修改前
use crate::storage::Storage;
use crate::memory::{Memory, CISMemory};

// 修改后
use cis_storage::Storage;
use cis_memory::{Memory, CISMemory};
```

## 验收标准

- [ ] Cargo.toml 依赖更新完成
- [ ] 重复代码已移除
- [ ] 所有模块通过重导出可用
- [ ] 编译通过
- [ ] 现有测试仍然通过

## 依赖

- Task 2.1 - 2.5 (所有模块提取完成)

## 阻塞

- Task 3.2 (cis-core 重构)

---
