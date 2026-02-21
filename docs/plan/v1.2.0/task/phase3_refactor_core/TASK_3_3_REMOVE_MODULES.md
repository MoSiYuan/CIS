# TASK 3.3: 移除已提取的模块

> **Phase**: 3 - cis-core 重构
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **负责人**: TBD
> **周期**: Week 6

---

## 任务概述

从 cis-core 中移除已迁移到 cis-common 的模块，避免代码重复。

## 工作内容

### 1. 删除已提取的源文件

```bash
# 删除已迁移的模块
cis-core/src/types.rs          → cis-common/cis-types/
cis-core/src/traits/           → cis-common/cis-traits/
cis-core/src/storage/          → cis-common/cis-storage/
cis-core/src/memory/           → cis-common/cis-memory/
cis-core/src/scheduler/        → cis-common/cis-scheduler/
cis-core/src/vector/           → cis-common/cis-vector/
cis-core/src/p2p/              → cis-common/cis-p2p/
```

### 2. 更新 cis-core/src/lib.rs

```rust
// Before: 本地模块定义
pub mod types;
pub mod traits;
pub mod storage;
pub mod memory;
pub mod scheduler;
pub mod vector;
pub mod p2p;

// After: 从 cis-common 重导出
pub use cis_types::*;
pub use cis_traits::*;
pub use cis_storage::*;
pub use cis_memory::*;
pub use cis_scheduler::*;
pub use cis_vector::*;
pub use cis_p2p::*;

// CIS 特有模块保留
pub mod error;
pub mod config;
pub mod sandbox;
pub mod skill;
pub mod ai;
pub mod agent;
pub mod matrix;
pub mod identity;
pub mod workflow;
pub mod security;
pub mod crypto;
```

### 3. 更新模块内部引用

**文件**: `cis-core/src/agent/mod.rs`

```rust
// Before
use crate::memory::MemoryService;
use crate::types::TaskLevel;

// After
use cis_memory::MemoryService;
use cis_types::TaskLevel;
```

**文件**: `cis-core/src/skill/mod.rs`

```rust
// Before
use crate::storage::StorageService;

// After
use cis_storage::StorageService;
```

**文件**: `cis-core/src/ai/mod.rs`

```rust
// Before
use crate::traits::AiProvider;

// After
use cis_traits::AiProvider;
```

**文件**: `cis-core/src/workflow/mod.rs`

```rust
// Before
use crate::scheduler::DagScheduler;

// After
use cis_scheduler::DagScheduler;
```

### 4. 检查并删除空的 mod 声明

```bash
# 检查是否还有未删除的引用
grep -r "pub mod types" cis-core/src/
grep -r "pub mod storage" cis-core/src/
grep -r "pub mod memory" cis-core/src/
# ... 其他模块
```

### 5. 备份和验证

```bash
# 确保删除前备份（已提交到 git）
git status

# 验证删除后编译
cargo check -p cis-core
```

## 验收标准

- [ ] 所有已提取的源文件已删除
- [ ] cis-core/src/lib.rs 更新完成
- [ ] 所有内部引用已更新
- [ ] 编译通过
- [ ] 无重复代码警告

## 依赖

- Task 2.x (所有模块提取完成)
- Task 3.1 (依赖更新)

## 阻塞

- Task 3.4 (更新依赖模块)

---
