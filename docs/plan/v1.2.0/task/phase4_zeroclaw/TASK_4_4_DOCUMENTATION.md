# TASK 4.4: ZeroClaw 集成文档

> **Phase**: 4 - ZeroClaw 兼容
> **状态**: 🔄 进行中 (Phase 5/6 实现)
> **负责人**: TBD
> **周期**: Week 9

---

## 任务概述

编写 ZeroClaw 集成文档，包括集成指南、迁移指南和 API 文档。

## 工作内容

### 1. 创建集成指南

**文件**: `docs/zeroclaw-integration.md`

```markdown
# ZeroClaw 集成指南

CIS v1.2.0 提供了可选的 ZeroClaw 集成功能，允许 CIS 作为 ZeroClaw 的 backend 运行。

## 功能特性

- **Memory Backend**: 使用 CIS Memory 替代 ZeroClaw 原生记忆系统
- **Scheduler Backend**: 使用 CIS Scheduler 替代 ZeroClaw 原生调度器
- **四级决策**: 在 ZeroClaw 中启用 CIS 的四级决策机制
- **DAG 编排**: 使用 CIS DAG 编排复杂任务

## 快速开始

### 1. 启用 ZeroClaw 功能

```toml
[dependencies]
cis-core = { version = "1.2.0", features = ["zeroclaw"] }
```

### 2. 创建适配器

```rust
use cis_core::Runtime;
use cis_core::zeroclaw::{ZeroclawMemoryAdapter, ZeroclawSchedulerAdapter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建 CIS Runtime
    let runtime = Runtime::builder()
        .with_storage(RocksDbStorage::new("./data"))
        .with_memory(CISMemoryService::new(...))
        .build()?;
    
    // 创建适配器
    let memory_adapter = ZeroclawMemoryAdapter::new(runtime.memory().clone());
    let scheduler_adapter = ZeroclawSchedulerAdapter::new(runtime.scheduler().clone());
    
    // 使用适配器...
    
    Ok(())
}
```

### 3. 配置 ZeroClaw 使用 CIS Backend

```rust
use zeroclaw::AgentBuilder;

let agent = AgentBuilder::new()
    .with_memory_backend(memory_adapter)
    .with_scheduler_backend(scheduler_adapter)
    .build()?;
```

## 类型映射

| CIS Type | ZeroClaw Type | 说明 |
|----------|---------------|------|
| `MemoryDomain::Private` | `MemoryCategory::Core` | 私域记忆 |
| `MemoryDomain::Public` | `MemoryCategory::Context` | 公域记忆 |
| `TaskLevel::Mechanical` | `ExecutionMode::Auto` | 自动执行 |
| `TaskLevel::Arbitrated` | `ExecutionMode::Arbitrate` | 仲裁模式 |

## 性能考虑

- Adapter 开销: < 5%
- Memory 操作: 与原生 CIS 相当
- Scheduler 操作: 与原生 CIS 相当
```

### 2. 创建迁移指南

**文件**: `docs/migration-guide.md`

```markdown
# 迁移指南: v1.1.x 到 v1.2.0

## 破坏性变更

### 1. 模块路径变更

```rust
// v1.1.x
use cis_core::types::TaskLevel;
use cis_core::storage::StorageService;

// v1.2.0
use cis_types::TaskLevel;
use cis_storage::StorageService;
```

### 2. Runtime 初始化

```rust
// v1.1.x
let core = CISCore::new(config).await?;

// v1.2.0
let runtime = Runtime::builder()
    .with_storage(...)
    .with_memory(...)
    .build()?;
```

## 向后兼容

v1.2.0 提供了重导出层，旧代码仍可编译：

```rust
use cis_core::types::TaskLevel;  // 通过重导出，会有 deprecation warning
```

## 迁移步骤

1. 更新 Cargo.toml 依赖
2. 替换导入语句
3. 更新 Runtime 初始化代码
4. 测试验证
```

### 3. 更新 API 文档

**文件**: `cis-core/src/zeroclaw/mod.rs`

```rust
//! ZeroClaw 集成适配器
//!
//! 本模块提供了 CIS 与 ZeroClaw 的集成适配器。
//!
//! # 示例
//!
//! ```rust,no_run
//! use cis_core::Runtime;
//! use cis_core::zeroclaw::ZeroclawMemoryAdapter;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let runtime = Runtime::builder()
//!         .with_storage(...)
//!         .build()?;
//!     
//!     let adapter = ZeroclawMemoryAdapter::new(runtime.memory().clone());
//!     
//!     // 使用 ZeroClaw Memory trait
//!     use zeroclaw::memory::Memory;
//!     adapter.store("key", "value", MemoryCategory::Core, None).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod memory_adapter;
pub mod scheduler_adapter;

pub use memory_adapter::ZeroclawMemoryAdapter;
pub use scheduler_adapter::ZeroclawSchedulerAdapter;
```

### 4. 创建架构图

**文件**: `docs/architecture/zeroclaw-integration.md`

```markdown
# ZeroClaw 集成架构

```
┌─────────────────────────────────────────────────────────────┐
│                     ZeroClaw Application                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ ZeroClaw    │  │ ZeroClaw    │  │ ZeroClaw            │ │
│  │ Agent       │  │ Memory      │  │ Scheduler           │ │
│  │ (Core)      │  │ (Trait)     │  │ (Trait)             │ │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
│         │                │                     │            │
│         └────────────────┴─────────────────────┘            │
│                          │                                  │
│                    Adapter Layer                            │
│              (ZeroclawMemoryAdapter)                        │
│              (ZeroclawSchedulerAdapter)                     │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                      CIS Runtime                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │ cis-memory  │  │ cis-storage │  │ cis-scheduler       │ │
│  │ (Backend)   │  │ (SQLite)    │  │ (DAG + 四级决策)     │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```
```

## 验收标准

- [ ] `docs/zeroclaw-integration.md` 完成
- [ ] `docs/migration-guide.md` 完成
- [ ] API 文档完整（rustdoc）
- [ ] 架构图清晰
- [ ] 示例代码可运行

## 依赖

- Task 4.1 (适配层实现)
- Task 4.3 (集成测试)

## 阻塞

- Task 6.1 (文档更新)

---
