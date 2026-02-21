# TASK 1.4: 更新根 Workspace Cargo.toml

> **Phase**: 1 - cis-common 基础
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: f41816b
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

更新项目根目录的 Cargo.toml，将 cis-common workspace 添加到 members 中。

## 工作内容

### 1. 更新根 Cargo.toml

**文件**: `/Users/jiangxiaolong/work/project/CIS/Cargo.toml`

添加 cis-common workspace 所有成员到根 workspace：

```toml
[workspace]
resolver = "2"
members = [
    # cis-common workspace (新增)
    "cis-common/cis-types",
    "cis-common/cis-traits",
    "cis-common/cis-storage",
    "cis-common/cis-memory",
    "cis-common/cis-scheduler",
    "cis-common/cis-vector",
    "cis-common/cis-p2p",
    # cis-core (现有)
    "cis-core",
    # ... 其他现有成员
]

[workspace.package]
version = "1.2.0"
edition = "2021"
rust-version = "1.70"
authors = ["CIS Team"]
license = "MIT"

[workspace.dependencies]
# === cis-common crates ===
cis-types = { path = "cis-common/cis-types", version = "1.2.0" }
cis-traits = { path = "cis-common/cis-traits", version = "1.2.0" }
cis-storage = { path = "cis-common/cis-storage", version = "1.2.0" }
cis-memory = { path = "cis-common/cis-memory", version = "1.2.0" }
cis-scheduler = { path = "cis-common/cis-scheduler", version = "1.2.0" }
cis-vector = { path = "cis-common/cis-vector", version = "1.2.0" }
cis-p2p = { path = "cis-common/cis-p2p", version = "1.2.0" }

# === cis-core ===
cis-core = { path = "cis-core", version = "1.2.0" }

# === Async runtime ===
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync", "time"] }
async-trait = "0.1"
futures = "0.3"

# === Error handling ===
anyhow = "1.0"
thiserror = "1.0"

# === Serialization ===
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# === Logging ===
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# === Time ===
chrono = { version = "0.4", features = ["serde"] }
```

### 2. 创建 cis-common 子 workspace 配置

**文件**: `cis-common/Cargo.toml`

为 cis-common 创建独立的 workspace 配置：

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
rust-version = "1.70"
authors = ["CIS Team"]
license = "MIT"
repository = "https://github.com/cis-projects/cis"

[workspace.dependencies]
# === Internal crates ===
cis-types = { path = "cis-types", version = "1.2.0" }
cis-traits = { path = "cis-traits", version = "1.2.0" }

# === Async runtime ===
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync"] }
async-trait = "0.1"

# === Error handling ===
anyhow = "1.0"
thiserror = "1.0"

# === Serialization ===
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# === Time ===
chrono = { version = "0.4", features = ["serde"] }

# === Database (cis-storage) ===
rusqlite = { version = "0.32", features = ["bundled"] }
sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio", "macros", "chrono"], optional = true }

# === Vector search (cis-vector) ===
fastembed = { version = "4.0", optional = true }
sqlite-vec = { version = "0.1", optional = true }
ndarray = { version = "0.15", optional = true }

# === P2P (cis-p2p) ===
quinn = { version = "0.11", optional = true }
libp2p = { version = "0.54", optional = true }
```

### 3. 更新 cis-core 依赖

**文件**: `cis-core/Cargo.toml`

添加 cis-common crates 依赖，并配置 feature flags：

```toml
[package]
name = "cis-core"
version = "1.2.0"
edition = "2021"

[dependencies]
# cis-common crates (可选依赖，渐进式迁移)
cis-types = { workspace = true }
cis-traits = { workspace = true, optional = true }
cis-storage = { workspace = true, optional = true }
cis-memory = { workspace = true, optional = true }
cis-scheduler = { workspace = true, optional = true }
cis-vector = { workspace = true, optional = true }
cis-p2p = { workspace = true, optional = true }

# 其他现有依赖保持不变...
tokio = { workspace = true }
async-trait = { workspace = true }
# ...
```

**配置 feature flags 以支持渐进式迁移**:

```toml
[features]
default = [
    "encryption",
    "vector",
    "p2p",
    "wasm",
    "parking_lot",
    "use-cis-common",  # 默认启用 cis-common crates
]

# 启用所有 cis-common crates
use-cis-common = [
    "cis-traits",
    "cis-storage",
    "cis-memory",
    "cis-scheduler",
]

# 单独启用各个 cis-common crate
use-cis-types = []       # cis-types 始终可用（无 optional）
use-cis-traits = ["cis-traits"]
use-cis-storage = ["cis-storage"]
use-cis-memory = ["cis-memory"]
use-cis-scheduler = ["cis-scheduler"]

# 向量功能使用 cis-vector
vector = [
    "fastembed",
    "sqlite-vec",
    "cis-vector",  # 新增：使用 cis-vector crate
]

# P2P 功能使用 cis-p2p
p2p = [
    "prost",
    "tonic",
    "encryption",
    "quinn",
    "rcgen",
    "mdns-sd",
    "rustls",
    "rustls-native-certs",
    "stun",
    "igd",
    "cis-p2p",  # 新增：使用 cis-p2p crate
]
```

### 4. 更新 lib.rs 重导出

**文件**: `cis-core/src/lib.rs`

添加重导出以保持向后兼容：

```rust
// Re-export cis-common types for backward compatibility
pub use cis_types::{
    TaskId, NodeId, TaskStatus, TaskLevel, Task, TaskResult, TaskPriority,
    Action, AmbiguityPolicy, MemoryCategory, MemoryDomain,
    DebtEntry, FailureType, SkillTask, SkillExecutionResult,
};

// Re-export cis-traits when feature is enabled
#[cfg(feature = "cis-traits")]
pub use cis_traits::{
    Memory, MemoryVectorIndex, MemorySync,
    DagScheduler, TaskExecutor,
    Agent, AgentPool,
    Lifecycle, Named,
};

// 其他现有重导出保持不变...
```

### 5. 验证 Workspace 结构

**命令**: 验证 workspace 配置正确

```bash
# 1. 检查 workspace 成员
cargo workspace --list

# 2. 查看完整的 workspace 依赖图
cargo metadata --format-version 1 | jq '.workspace_members'

# 3. 尝试编译所有 crates（包括 cis-common）
cargo check --workspace

# 4. 单独测试 cis-common workspace
cd cis-common && cargo check --workspace

# 5. 测试 cis-core 对 cis-common 的依赖
cd cis-core && cargo check --features use-cis-common

# 6. 验证版本一致性
cargo tree --workspace | grep "cis-" | sort | uniq
```

### 6. 更新 .gitignore

**文件**: `.gitignore`

添加 workspace 构建产物忽略规则：

```gitignore
# Workspace build artifacts
/target/
/cis-common/target/

# Cargo build cache
.cargo-cache/

# 但保留示例配置
!.cargo/config.toml.example
```

### 7. 创建 CI/CD 配置

**文件**: `.github/workflows/ci.yml` (如果存在)

确保 CI 测试所有 workspace 成员：

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1

      # 测试整个 workspace
      - name: Check workspace
        run: cargo check --workspace --all-features

      # 测试 cis-common
      - name: Check cis-common
        run: cd cis-common && cargo check --workspace

      # 运行所有测试
      - name: Run tests
        run: cargo test --workspace

      # 检查格式
      - name: Check formatting
        run: cargo fmt --all -- --check

      # 运行 clippy
      - name: Run clippy
        run: cargo clippy --workspace --all-features -- -D warnings
```

## 验收标准

- [ ] 根 Cargo.toml 包含所有 cis-common crates
- [ ] cis-common/Cargo.toml workspace 配置正确
- [ ] cis-core/Cargo.toml 正确依赖 cis-common crates
- [ ] Workspace dependencies 配置合理
- [ ] Feature flags 支持渐进式迁移（use-cis-common）
- [ ] cis-core/src/lib.rs 重导出保持向后兼容
- [ ] `cargo check --workspace` 编译通过
- [ ] `cargo test --workspace` 测试通过
- [ ] `cargo tree --workspace` 依赖关系正确
- [ ] 版本号统一为 1.2.0
- [ ] CI/CD 配置更新（如果存在）
- [ ] .gitignore 更新完整

## 依赖

- Task 1.1, 1.2, 1.3 (基础 crate 创建)

## 阻塞

- Task 2.x (模块提取任务)

---
