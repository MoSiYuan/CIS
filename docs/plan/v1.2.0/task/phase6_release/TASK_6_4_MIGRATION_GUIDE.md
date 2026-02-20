# TASK 6.4: 迁移指南编写

> **Phase**: 6 - 发布准备
> **状态**: 🔄 进行中 (占位符已创建，MIGRATION.md 已创建)
> **负责人**: TBD
> **周期**: Week 13

---

## 任务概述

编写从 CIS v1.1.5 到 v1.2.0 的详细迁移指南，帮助用户和开发者平滑升级到新版本。

## 工作内容

### 1. 破坏性变更文档

**文件**: `docs/migration/v1.1.5-to-v1.2.0.md`

#### 1.1 类型路径变更

列出所有类型路径变更：

```markdown
## 类型路径变更

### Task 相关类型

**变更前 (v1.1.5)**:
```rust
use cis_core::types::{Task, TaskStatus, TaskLevel, TaskPriority};
use cis_core::types::{TaskResult, TaskId, NodeId};
```

**变更后 (v1.2.0)**:
```rust
use cis_types::{Task, TaskStatus, TaskLevel, TaskPriority};
use cis_types::{TaskResult, TaskId, NodeId};

// 或者通过 cis-core 重导出（向后兼容）
use cis_core::types::{Task, TaskStatus, TaskLevel, TaskPriority};
```

### Memory 相关类型

**变更前 (v1.1.5)**:
```rust
use cis_core::memory::{MemoryDomain, MemoryCategory};
use cis_core::memory::MemoryService;
```

**变更后 (v1.2.0)**:
```rust
use cis_types::{MemoryDomain, MemoryCategory};
use cis_memory::CisMemoryService;

// 或通过 cis-core 重导出
use cis_core::memory::{MemoryDomain, MemoryCategory, MemoryService};
```

### Trait 导入变更

**变更前 (v1.1.5)**:
```rust
use cis_core::traits::{NetworkService, StorageService, EventBus};
```

**变更后 (v1.2.0)**:
```rust
use cis_traits::{NetworkService, StorageService, EventBus};

// 或通过 cis-core 重导出
use cis_core::traits::{NetworkService, StorageService, EventBus};
```
```

#### 1.2 Trait 方法签名变更

```markdown
## Trait 方法签名变更

### Memory Trait

**变更前 (v1.1.5)**:
```rust
pub trait MemoryServiceTrait: Send + Sync {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    fn set(&self, key: &str, value: &[u8]) -> Result<()>;
}
```

**变更后 (v1.2.0)**:
```rust
#[async_trait]
pub trait Memory: Send + Sync {
    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>>;
    async fn set(
        &self, 
        key: &str, 
        value: &[u8], 
        domain: MemoryDomain, 
        category: MemoryCategory
    ) -> anyhow::Result<()>;
}
```

**影响**:
- 现在是 async trait，需要 `async_trait`
- `set` 方法新增 `domain` 和 `category` 参数
- 返回类型从 `Option` 改为 `Result`

**迁移示例**:
```rust
// Before
let value = memory.get("key");
if let Some(data) = value {
    // ...
}

// After
let entry = memory.get("key").await?;
if let Some(entry) = entry {
    let data = entry.value;
    // ...
}
```

### Scheduler Trait

**变更前 (v1.1.5)**:
```rust
impl Scheduler {
    pub fn execute(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>>;
}
```

**变更后 (v1.2.0)**:
```rust
#[async_trait]
pub trait DagScheduler: Send + Sync {
    async fn build_dag(&mut self, tasks: Vec<Task>) -> anyhow::Result<Dag>;
    async fn execute_dag(&self, dag: Dag) -> anyhow::Result<DagExecutionResult>;
}
```

**迁移示例**:
```rust
// Before
let scheduler = Scheduler::new();
let results = scheduler.execute(tasks)?;

// After
let mut scheduler = DagScheduler::new();
let dag = scheduler.build_dag(tasks).await?;
let results = scheduler.execute_dag(dag).await?;
```
```

#### 1.3 Feature Flag 变更

```markdown
## Feature Flag 变更

### 新增 Feature Flags

```toml
# 启用 cis-common crates
use-cis-common = [
    "cis-traits",
    "cis-storage",
    "cis-memory",
    "cis-scheduler",
]

# 单独启用各个 crate
use-cis-storage = ["cis-storage"]
use-cis-memory = ["cis-memory"]
use-cis-scheduler = ["cis-scheduler"]
```

### 废弃 Feature Flags

```toml
# v1.1.5
# No feature flags (all features enabled by default)

# v1.2.0
# 默认启用 use-cis-common，但可以选择禁用
```
```

### 2. 迁移代码示例

#### 2.1 项目配置迁移

**文件**: `docs/migration/examples/project-config/`

**v1.1.5 配置**:
```toml
# .cis/config.toml (v1.1.5)
[ai]
provider = "claude"
model = "claude-3-sonnet"

[memory]
backend = "sqlite"
```

**v1.2.0 配置**:
```toml
# .cis/config.toml (v1.2.0)
[ai]
provider = "claude"
model = "claude-3-sonnet"

[memory]
backend = "cis-memory"  # 使用新的 cis-memory crate
domain_separation = true  # 启用私域/公域分离

[scheduler]
backend = "cis-scheduler"  # 使用新的 cis-scheduler crate
four_level_decision = true  # 启用四级决策
```

#### 2.2 代码迁移示例

**文件**: `docs/migration/examples/code/`

**Memory 服务迁移**:

```rust
// === v1.1.5 ===
use cis_core::memory::MemoryService;

let memory = MemoryService::new(config);
memory.set("key", b"value")?;
let value = memory.get("key");

// === v1.2.0 ===
use cis_memory::CisMemoryService;
use cis_types::{MemoryDomain, MemoryCategory};

let memory = CisMemoryService::new(config).await?;
memory.set(
    "key", 
    b"value", 
    MemoryDomain::Public,  // 明确指定域
    MemoryCategory::Context
).await?;

let entry = memory.get("key").await?;
if let Some(entry) = entry {
    let value = entry.value;
}
```

**Scheduler 迁移**:

```rust
// === v1.1.5 ===
use cis_core::scheduler::Scheduler;

let scheduler = Scheduler::new();
let tasks = vec![task1, task2];
let results = scheduler.execute(tasks)?;

// === v1.2.0 ===
use cis_scheduler::CisDagScheduler;

let mut scheduler = CisDagScheduler::new().await?;
let tasks = vec![task1, task2];

// 先构建 DAG
let dag = scheduler.build_dag(tasks).await?;

// 验证 DAG
scheduler.validate_dag(&dag).await?;

// 执行 DAG
let result = scheduler.execute_dag(dag).await?;
```

**Agent 使用迁移**:

```rust
// === v1.1.5 ===
use cis_core::agent::Agent;

let mut agent = Agent::new("claude");
let response = agent.turn("帮我写代码")?;

// === v1.2.0 ===
use cis_core::agent::{Agent, AgentPool, AgentType};

// 使用 Agent Pool 管理
let pool = AgentPool::new().await?;
let mut agent = pool.acquire(AgentType::Coder).await?;

let response = agent.turn("帮我写代码").await?;

// 归还 Agent
pool.release(agent).await?;
```

### 3. Before/After 对比表

| 组件 | v1.1.5 | v1.2.0 | 变更类型 |
|------|--------|--------|----------|
| **类型定义** | `cis_core::types` | `cis_types` | 位置变更 |
| **Trait 定义** | `cis_core::traits` | `cis_traits` | 位置变更 |
| **Memory 实现** | `cis_core::memory` | `cis_memory` | 独立 crate |
| **Scheduler 实现** | `cis_core::scheduler` | `cis_scheduler` | 独立 crate |
| **Memory::set** | `fn set(&self, key, value)` | `async fn set(&self, key, value, domain, category)` | 签名变更 |
| **Scheduler::execute** | `fn execute(&self, tasks)` | `async fn execute_dag(&self, dag)` | 签名变更 |
| **Task Level** | `Mechanical, Recommended, Confirmed` | + `Arbitrated` | 新增级别 |
| **Memory Domain** | 无 | `Private, Public` | 新增概念 |

### 4. 测试清单

**文件**: `docs/migration/checklist.md`

```markdown
## v1.2.0 升级测试清单

### 编译测试

- [ ] 项目使用 `use-cis-common` feature 编译通过
- [ ] 不使用 `use-cis-common` feature 编译通过（向后兼容）
- [ ] 所有依赖 cis-core 的子项目编译通过
- [ ] `cargo check --workspace` 无错误
- [ ] `cargo build --release --workspace` 成功

### 单元测试

- [ ] `cargo test --workspace` 全部通过
- [ ] Memory 相关测试通过
- [ ] Scheduler 相关测试通过
- [ ] Agent 相关测试通过
- [ ] P2P 相关测试通过

### 集成测试

- [ ] DAG 编排集成测试通过
- [ ] Agent Pool 管理测试通过
- [ ] 记忆隔离测试通过
- [ ] P2P 通信测试通过
- [ ] 端到端工作流测试通过

### 性能回归测试

- [ ] Agent turn 响应时间 < 2s
- [ ] DAG 执行吞吐量 > 10 tasks/s
- [ ] 记忆搜索延迟 (p99) < 100ms
- [ ] P2P 消息延迟 < 500ms

### 功能验证

- [ ] 私域/公域记忆分离正常
- [ ] 四级决策机制工作正常
- [ ] Agent Pool 动态扩缩容
- [ ] P2P 跨设备 Agent 调用
- [ ] 记忆同步正常
- [ ] 配置加载正确
```

### 5. 故障排查指南

**文件**: `docs/migration/troubleshooting.md`

```markdown
## 常见问题排查

### 问题 1: 编译错误 "use of undeclared crate"

**错误信息**:
```
error[E0433]: failed to resolve: use of undeclared crate or module `cis_types`
```

**原因**: 未启用 `use-cis-common` feature

**解决方案**:
```toml
# Cargo.toml
[dependencies]
cis-core = { version = "1.2.0", features = ["use-cis-common"] }
```

---

### 问题 2: async trait 调用错误

**错误信息**:
```
error[E0277]: `()` is not a future
```

**原因**: v1.2.0 trait 方法全部改为 async

**解决方案**:
```rust
// Before
let value = memory.get("key");

// After
let entry = memory.get("key").await?;  // 添加 .await
```

---

### 问题 3: Memory domain 参数缺失

**错误信息**:
```
error[E0061]: this function takes 4 arguments but 3 arguments were supplied
```

**原因**: `Memory::set` 新增必需的 `domain` 和 `category` 参数

**解决方案**:
```rust
// Before
memory.set("key", b"value")?;

// After
use cis_types::{MemoryDomain, MemoryCategory};

memory.set(
    "key", 
    b"value", 
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;
```

---

### 问题 4: TaskLevel 缺少 Arbitrated

**错误信息**:
```
error[E0599]: no variant named `Arbitrated` found
```

**原因**: 未导入 TaskLevel 变体

**解决方案**:
```rust
use cis_types::TaskLevel;

let level = TaskLevel::Arbitrated {
    stakeholders: vec!["alice".into(), "bob".into()]
};
```

---

### 问题 5: 性能下降

**现象**: 升级后 Agent 响应变慢

**可能原因**:
1. 未启用 `use-cis-common` feature（使用旧代码路径）
2. 记忆向量索引未重建
3. P2P 网络配置不当

**排查步骤**:
```bash
# 1. 检查 feature flags
cargo tree -i cis-core | grep features

# 2. 重建记忆索引
cis memory rebuild-index

# 3. 检查 P2P 连接
cis p2p status

# 4. 运行性能测试
cargo bench --workspace
```
```

### 6. 版本兼容性矩阵

**文件**: `docs/migration/compatibility.md`

```markdown
## CIS 版本兼容性

| CIS Version | Rust Version | tokio | serde | Notes |
|-------------|--------------|-------|-------|-------|
| v1.1.5 | 1.70+ | 1.35 | 1.0 | 稳定版本 |
| v1.2.0 | 1.70+ | 1.35+ | 1.0+ | **当前版本** |

### 依赖版本要求

```toml
[dependencies]
cis-core = "1.2.0"

# 必需依赖
cis-types = "1.2.0"
tokio = { version = "1.35", features = ["rt-multi-thread", "macros", "sync"] }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"

# 可选依赖（通过 feature flags 启用）
cis-traits = { version = "1.2.0", optional = true }
cis-storage = { version = "1.2.0", optional = true }
cis-memory = { version = "1.2.0", optional = true }
cis-scheduler = { version = "1.2.0", optional = true }
```

### 升级路径

```
v1.1.x → v1.2.0
  ↓
直接升级（breaking changes 已通过重导出兼容）
```
```

### 7. 自动化迁移工具

**文件**: `scripts/migrate/v1.1.5-to-v1.2.0.sh`

```bash
#!/bin/bash
# CIS v1.1.5 → v1.2.0 自动化迁移脚本

set -e

echo "🚀 CIS v1.1.5 → v1.2.0 迁移工具"
echo "================================="

# 1. 检查当前版本
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | cut -d'"' -f2)
echo "📌 当前版本: $CURRENT_VERSION"

# 2. 更新 Cargo.toml
echo "📝 更新 Cargo.toml..."
sed -i.bak 's/cis-core = "1.1"/cis-core = { version = "1.2.0", features = ["use-cis-common"] }/' Cargo.toml

# 3. 更新导入路径
echo "🔄 更新导入路径..."
find src -name "*.rs" -exec sed -i.bak 's/use cis_core::types/use cis_types/g' {} \;
find src -name "*.rs" -exec sed -i.bak 's/use cis_core::traits/use cis_traits/g' {} \;

# 4. 更新 async 调用
echo "⏳ 更新 async trait 调用..."
# 添加 .await 到 memory.get
find src -name "*.rs" -exec sed -i.bak 's/memory\.get(\(.*\))/memory.get(\1).await?/g' {} \;

# 5. 清理备份文件
echo "🧹 清理备份文件..."
find src -name "*.bak" -delete
find . -name "Cargo.toml.bak" -delete

# 6. 验证编译
echo "🔍 验证编译..."
cargo check --workspace

echo "✅ 迁移完成！"
echo "⚠️  请检查以下内容："
echo "   1. Memory::set 调用是否添加了 domain/category 参数"
echo "   2. Scheduler::execute 是否改为 build_dag + execute_dag"
echo "   3. 运行 cargo test 确保测试通过"
```

**使用方法**:
```bash
# 1. 备份项目
git commit -am "Backup before migration to v1.2.0"

# 2. 运行迁移脚本
chmod +x scripts/migrate/v1.1.5-to-v1.2.0.sh
./scripts/migrate/v1.1.5-to-v1.2.0.sh

# 3. 手动检查修改
git diff

# 4. 运行测试
cargo test --workspace

# 5. 提交变更
git commit -am "Migrate to CIS v1.2.0"
```

## 验收标准

- [ ] 破坏性变更文档完整
- [ ] 所有类型路径变更列明
- [ ] Trait 方法签名变更详细说明
- [ ] Feature flag 变更清晰
- [ ] 迁移代码示例完整可运行
- [ ] Before/After 对比表清晰
- [ ] 测试清单可执行
- [ ] 故障排查指南覆盖常见问题
- [ ] 版本兼容性矩阵准确
- [ ] 自动化迁移脚本可用
- [ ] 所有示例代码编译通过
- [ ] 文档格式统一（markdown）
- [ ] 包含中文和英文版本

## 依赖

- TASK_6_1 (更新版本号)
- TASK_6_2 (更新文档)
- TASK_6_3 (发布 CIS)

## 阻塞

- 无（Phase 6 最后一项）

---

**关键交付物**:
- ✅ 破坏性变更详细文档
- ✅ 代码迁移示例（Before/After）
- ✅ 升级测试清单
- ✅ 故障排查指南
- ✅ 自动化迁移脚本
- ✅ 版本兼容性矩阵

