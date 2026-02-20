# TASK 1.1: 创建 cis-common Workspace

> **Phase**: 1 - cis-common 基础
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

创建独立的 `cis-common` workspace，包含 7 个独立 crates，实现 CIS 基础模块的 trait 抽象与实现分离。

**设计理念**: 参考 ZeroClaw 的 trait 抽象设计，将接口定义与实现分离

## 工作内容

### 1. 创建 Workspace 目录结构

```bash
cis-common/
├── Cargo.toml                    # Workspace root
├── cis-types/                    # 基础类型（零依赖）
├── cis-traits/                   # Trait 抽象（仅依赖 types）
├── cis-storage/                  # 存储层（依赖 types, traits）
├── cis-memory/                   # 记忆系统（依赖 storage, traits, types）
├── cis-scheduler/                # DAG 编排（依赖 types, traits）
├── cis-vector/                   # 向量搜索（依赖 types, traits, storage, memory）
└── cis-p2p/                      # P2P 网络（依赖 types, traits, storage）
```

### 2. 配置 Workspace Root

**File**: `cis-common/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "cis-types",
    "cis-traits",
    "cis-storage",
    "cis-memory",
    "cis-scheduler",
    "cis-vector",
    "cis-p2p",
]

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync", "time"] }
async-trait = "0.1"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Database (for cis-storage)
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono"], optional = true }

# Vector search (for cis-vector)
fastembed = { version = "3.0", optional = true }
sqlite-vec = { version = "0.5", optional = true }

# P2P (for cis-p2p)
libp2p = { version = "0.54", optional = true }
prost = { version = "0.12", optional = true }
tonic = { version = "0.11", optional = true }

# Internal dependencies
cis-types = { path = "cis-types", version = "1.2.0" }
cis-traits = { path = "cis-traits", version = "1.2.0" }
cis-storage = { path = "cis-storage", version = "1.2.0" }
```

### 3. 配置根 Cargo.toml

**File**: `/Users/jiangxiaolong/work/project/CIS/Cargo.toml`

**Add cis-common to workspace**:
```toml
[workspace]
resolver = "2"
members = [
    "cis-common",         # NEW - cis-common workspace
    "cis-core",
    "cis-node",
    # ... 其他成员
]
```

### 4. 更新 workspace 依赖

**File**: `cis-core/Cargo.toml`

**Add cis-common dependencies**:
```toml
[dependencies]
# cis-common workspace dependencies
cis-types = { path = "../cis-common/cis-types", version = "1.2.0" }
cis-traits = { path = "../cis-common/cis-traits", version = "1.2.0" }
cis-storage = { path = "../cis-common/cis-storage", version = "1.2.0" }
cis-memory = { path = "../cis-common/cis-memory", version = "1.2.0" }
cis-scheduler = { path = "../cis-common/cis-scheduler", version = "1.2.0" }

# Optional modules
cis-vector = { path = "../cis-common/cis-vector", version = "1.2.0", optional = true }
cis-p2p = { path = "../cis-common/cis-p2p", version = "1.2.0", optional = true }
```

## 验收标准

- [ ] cis-common workspace 创建完成
- [ ] 7 个 crates 目录结构创建
- [ ] Workspace 配置编译通过
- [ ] 根 workspace 配置更新完成
- [ ] 文档说明 workspace 结构

## 依赖

- 无 (这是第一个任务)

## 阻塞

- TASK_1_2 (提取 cis-types)
- TASK_1_3 (定义 cis-traits)
- TASK_1_4 (更新 workspace 配置)

---
