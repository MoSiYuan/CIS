# TASK 1.4: 更新根 Workspace Cargo.toml

> **Phase**: 1 - cis-common 基础
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

更新项目根目录的 Cargo.toml，将 cis-common workspace 添加到 members 中。

## 工作内容

### 1. 修改根 Cargo.toml

**文件**: `/Users/jiangxiaolong/work/project/CIS/Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "cis-common",         # NEW - cis-common workspace
    "cis-common/cis-types",
    "cis-common/cis-traits",
    "cis-common/cis-storage",
    "cis-common/cis-memory",
    "cis-common/cis-scheduler",
    "cis-common/cis-vector",
    "cis-common/cis-p2p",
    "cis-core",
    "cis-node",
    # ... 其他成员
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

# Internal dependencies (new crates)
cis-types = { path = "cis-common/cis-types", version = "1.2.0" }
cis-traits = { path = "cis-common/cis-traits", version = "1.2.0" }
cis-storage = { path = "cis-common/cis-storage", version = "1.2.0" }
cis-memory = { path = "cis-common/cis-memory", version = "1.2.0" }
cis-scheduler = { path = "cis-common/cis-scheduler", version = "1.2.0" }
cis-vector = { path = "cis-common/cis-vector", version = "1.2.0" }
cis-p2p = { path = "cis-common/cis-p2p", version = "1.2.0" }
```

### 2. 创建 cis-common workspace Cargo.toml

**文件**: `cis-common/Cargo.toml`

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

[workspace.package]
version = "1.2.0"
edition = "2021"
authors = ["CIS Team <team@cis.dev>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/cis-projects/cis"

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

# Database (for cis-storage)
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono"] }

# Vector search (for cis-vector)
fastembed = { version = "3.0", optional = true }
sqlite-vec = { version = "0.5", optional = true }

# P2P (for cis-p2p)
libp2p = { version = "0.54", optional = true }

# Internal dependencies
cis-types = { path = "cis-types", version = "1.2.0" }
cis-traits = { path = "cis-traits", version = "1.2.0" }
cis-storage = { path = "cis-storage", version = "1.2.0" }
```

### 3. 验证 Workspace 结构

```bash
# 检查 workspace 成员
cargo metadata --format-version 1 | jq '.workspace_members'

# 确保所有 crates 可被识别
cargo check --workspace
```

## 验收标准

- [ ] 根 Cargo.toml 添加了 cis-common 成员
- [ ] cis-common/Cargo.toml 创建完成
- [ ] `cargo check --workspace` 通过
- [ ] 所有 crates 可被正确识别

## 依赖

- Task 1.1, 1.2, 1.3 (基础 crate 创建)

## 阻塞

- Task 2.x (模块提取任务)

---
