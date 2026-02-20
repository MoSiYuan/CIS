# CIS v1.2.0 代码-任务映射索引

> **目的**: 帮助Worker Agent快速定位任务相关的代码文件，减少上下文浪费
> **更新时间**: 2026-02-20

---

## 使用说明

当Worker Agent执行某个任务时，参考此索引直接读取相关代码文件，避免全局搜索。

**格式**: `任务ID → 涉及的代码文件路径`

---

## Phase 1: cis-common 基础

### TASK_1_1: 创建 cis-common Workspace

**涉及代码**: 
- 当前尚无（新创建）

**需参考文件**:
- `Cargo.toml` (根目录workspace配置模板)
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - 现有依赖配置参考

---

### TASK_1_2: 提取 cis-types

**涉及代码**:
- [cis-core/src/types.rs](../../cis-core/src/types.rs) - **核心源文件** (375行)
  - Task相关: TaskId, NodeId, TaskStatus, TaskLevel, Task, TaskResult, TaskPriority, Action, AmbiguityPolicy
  - Memory相关: MemoryCategory, MemoryDomain
  - Debt相关: DebtEntry, FailureType
  - Skill相关: SkillTask, SkillExecutionResult

**需参考文件**:
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - serde, chrono依赖配置
- [cis-core/src/lib.rs](../../cis-core/src/lib.rs) - 类型重导出位置

---

### TASK_1_3: 定义 cis-traits

**涉及代码**:
- [cis-core/src/traits/mod.rs](../../cis-core/src/traits/mod.rs) - **现有trait定义**
  - NetworkService, StorageService, EventBus, SkillExecutor
  - AiProvider, EmbeddingService
  
**需新增trait** (参考现有风格):
- Memory trait ← 参考cis-core/src/memory/mod.rs的MemoryServiceTrait
- Scheduler (DagScheduler, TaskExecutor) ← 参考cis-core/src/scheduler/mod.rs
- Agent (Agent, AgentPool) ← 新设计
- Lifecycle, Named ← 新设计

**需参考文件**:
- [cis-core/src/memory/mod.rs](../../cis-core/src/memory/mod.rs) - MemoryServiceTrait定义（第15-25行）
- [cis-core/src/scheduler/mod.rs](../../cis-core/src/scheduler/mod.rs) - 现有scheduler实现

---

### TASK_1_4: 更新 Workspace 配置

**涉及代码**:
- [Cargo.toml](../../Cargo.toml) - **根workspace配置** (需更新)
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - **依赖配置** (需更新)
- [cis-core/src/lib.rs](../../cis-core/src/lib.rs) - **重导出** (需更新)

---

## Phase 2: 模块提取

### TASK_2_1: 提取 cis-storage

**涉及代码**:
- [cis-core/src/storage/](../../cis-core/src/storage/) - **整个模块**
  - mod.rs - 模块导出
  - sqlite_storage.rs - SQLite实现
  - memory_db.rs - MemoryDb实现
  - crypto.rs - 存储加密

**需参考trait**:
- [cis-core/src/traits/storage.rs](../../cis-core/src/traits/storage.rs) - StorageService trait定义

---

### TASK_2_2: 提取 cis-memory

**涉及代码**:
- [cis-core/src/memory/](../../cis-core/src/memory/) - **整个模块**
  - mod.rs (168行) - 模块定义和导出
  - service.rs - MemoryService实现（私域/公域分离）
  - encryption.rs, encryption_v2.rs - 加密实现
  - ops/ - 操作（hybrid_search.rs已存在）
  - scope.rs - MemoryScope
  - guard.rs - 冲突检测

**关键类型**:
- MemoryService (第15-25行的trait定义)
- MemoryDomain, MemoryCategory (从cis-types引入)

**依赖**:
- TASK_2_1 (cis-storage) - Memory使用Storage

---

### TASK_2_3: 提取 cis-scheduler

**涉及代码**:
- [cis-core/src/scheduler/](../../cis-core/src/scheduler/) - **整个模块**
  - mod.rs - 模块导出
  - task_manager.rs - TaskManager实现
  - dag.rs - DAG定义和执行

**关键类型** (来自cis-types):
- Task, TaskStatus, TaskLevel
- TaskResult

**依赖**:
- TASK_1_2 (cis-types) - Task相关类型
- TASK_1_3 (cis-traits) - DagScheduler trait

---

### TASK_2_4: 提取 cis-vector

**涉及代码**:
- [cis-core/src/memory/ops/hybrid_search.rs](../../cis-core/src/memory/ops/hybrid_search.rs) - **混合搜索实现**
  - 70%向量相似度 + 30%BM25关键词

**需参考**:
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - fastembed, sqlite-vec依赖
- [cis-core/src/memory/service.rs](../../cis-core/src/memory/service.rs) - 向量索引集成

---

### TASK_2_5: 提取 cis-p2p

**涉及代码**:
- [cis-core/src/network/](../../cis-core/src/network/) - **整个网络模块**
  - mod.rs (385行) - 网络服务定义
  - p2p.rs - P2P实现
  - acl.rs - 访问控制
  - did.rs - DID验证

**关键trait**:
- [cis-core/src/traits/network.rs](../../cis-core/src/traits/network.rs) - NetworkService trait

---

## Phase 3: cis-core 重构

### TASK_3_1: 更新 cis-core 依赖

**涉及代码**:
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - **主要修改目标**
  - 添加cis-common crates依赖
  - 配置feature flags

---

### TASK_3_2: cis-core 核心重构

**涉及代码**:
- [cis-core/src/lib.rs](../../cis-core/src/lib.rs) - **主要修改目标**
  - 重导出cis-common类型
  - 重导出trait（可选feature）

---

### TASK_3_3: 移除已提取的模块

**涉及代码**:
- [cis-core/src/storage/](../../cis-core/src/storage/) - **删除** (移至cis-storage)
- [cis-core/src/memory/](../../cis-core/src/memory/) - **删除** (移至cis-memory)
- [cis-core/src/scheduler/](../../cis-core/src/scheduler/) - **删除** (移至cis-scheduler)
- [cis-core/src/network/](../../cis-core/src/network/) - **部分保留** (P2P高级功能)

---

### TASK_3_4: 更新导入语句

**涉及代码**:
- 所有 `use cis_core::` 改为 `use cis_types::` / `use cis_traits::` / `use cis_memory::` 等
- 搜索范围: cis-core/src/

**批量替换命令**:
```bash
# 查找所有需要更新的导入
grep -r "use cis_core::" cis-core/src/
```

---

### TASK_3_5: 测试编译

**涉及代码**:
- 整个workspace
- 运行: `cargo check --workspace`

---

## Phase 4: ZeroClaw 兼容

### TASK_4_1: 适配层实现

**涉及代码**:
- 新建: `cis-core/src/zeroclaw/` 模块
  - memory_adapter.rs - 实现zeroclaw::Memory trait
  - scheduler_adapter.rs - 实现zeroclaw::Scheduler trait
  
**参考实现**:
- [cis-core/src/memory/service.rs](../../cis-core/src/memory/service.rs) - 现有Memory实现
- [cis-core/src/scheduler/dag.rs](../../cis-core/src/scheduler/dag.rs) - 现有DAG实现

---

### TASK_4_2: E2E 验证

**涉及代码**:
- 新建集成测试: `tests/zeroclaw_integration_test.rs`

---

## Phase 5: 测试与验证

### TASK_5_1: 测试框架

**涉及代码**:
- [cis-core/tests/](../../cis-core/tests/) - 现有测试
- 新建: `tests/integration/` 目录

---

### TASK_5_2: CI 配置

**涉及代码**:
- [`.github/workflows/`](../../.github/workflows/) - CI配置文件

---

### TASK_5_3: 性能基准测试

**涉及代码**:
- [cis-core/benches/](../../cis-core/benches/) - 现有基准测试
  - agent_teams_benchmarks.rs
  - database_operations.rs
  - dag_operations.rs
  - weekly_archived_memory.rs
  - task_manager.rs

**需新增**:
- benches/cis_v1_2_0_benchmarks.rs - 全面基准测试
- benches/performance_budget.toml - 性能预算配置
- benches/baseline/v1.1.5/ - v1.1.5基线数据

---

## Phase 6: 发布准备

### TASK_6_1: 文档更新

**涉及代码**:
- [docs/](../../docs/) - 文档目录

---

### TASK_6_2: 发布准备

**涉及代码**:
- [cis-core/Cargo.toml](../../cis-core/Cargo.toml) - 版本号更新

---

### TASK_6_3: 发布 CIS

**涉及代码**:
- 发布流程，无特定代码文件

---

### TASK_6_4: 编写迁移指南

**涉及代码**:
- 新建文档: `docs/migration/v1.1.5-to-v1.2.0.md`
- 新建脚本: `scripts/migrate/v1.1.5-to-v1.2.0.sh`

---

## Phase 7: 多 Agent 架构

### TASK_7_1: Agent trait 实现

**涉及代码**:
- 新建: `cis-core/src/agent/` 模块
  - mod.rs - Agent模块定义
  - trait.rs - Agent, AgentPool trait定义

**参考设计**:
- [cis-core/src/types.rs](../../cis-core/src/types.rs) - Task相关类型
- [cis-core/src/scheduler/mod.rs](../../cis-core/src/scheduler/mod.rs) - Task执行模式

---

### TASK_7_2: Receptionist Agent

**涉及代码**:
- 新建: `cis-core/src/agent/receptionist.rs`
- 依赖: cis-traits (Agent trait)

---

### TASK_7_3: Worker Agents

**涉及代码**:
- 新建: `cis-core/src/agent/workers.rs`
  - CoderAgent
  - DocAgent
  - DebuggerAgent

**参考**:
- [cis-core/src/wasm/mod.rs](../../cis-core/src/wasm/mod.rs) - 现有Skill执行模式

---

### TASK_7_4: DAG 编排

**涉及代码**:
- [cis-core/src/scheduler/dag.rs](../../cis-core/src/scheduler/dag.rs) - 现有DAG实现
- [cis-core/src/types.rs](../../cis-core/src/types.rs) - TaskLevel四级决策

**需扩展**:
- Agent与DAG集成
- 四级决策机制实现

---

### TASK_7_5: 记忆隔离

**涉及代码**:
- [cis-core/src/memory/service.rs](../../cis-core/src/memory/service.rs) - MemoryService
- [cis-core/src/memory/scope.rs](../../cis-core/src/memory/scope.rs) - MemoryScope
- [cis-core/src/memory/guard.rs](../../cis-core/src/memory/guard.rs) - 冲突检测

**需实现**:
- Agent级记忆隔离
- 幻觉降低机制

---

### TASK_7_6: P2P 跨设备 Agent 调用

**涉及代码**:
- [cis-core/src/network/](../../cis-core/src/network/) - P2P网络
- 新建: `cis-core/src/agent/remote.rs` - RemoteAgentProxy

**依赖**:
- TASK_7_1 (Agent trait)
- TASK_2_5 (cis-p2p)

---

### TASK_7_7: 多 Agent 集成测试

**涉及代码**:
- 新建: `tests/integration/agent_tests.rs`
- 新建: `tests/integration/dag_tests.rs`
- 新建: `tests/integration/memory_tests.rs`
- 新建: `tests/integration/p2p_tests.rs`
- 新建: `tests/e2e/multi_agent_workflow.rs`

---

## 快速查找表

### 按文件类型查找

| 文件类型 | 主要位置 | 任务关联 |
|---------|---------|---------|
| **类型定义** | cis-core/src/types.rs | TASK_1_2 |
| **Trait定义** | cis-core/src/traits/ | TASK_1_3 |
| **Storage** | cis-core/src/storage/ | TASK_2_1 |
| **Memory** | cis-core/src/memory/ | TASK_2_2, TASK_7_5 |
| **Scheduler** | cis-core/src/scheduler/ | TASK_2_3, TASK_7_4 |
| **Network/P2P** | cis-core/src/network/ | TASK_2_5, TASK_7_6 |
| **Workspace配置** | Cargo.toml, cis-core/Cargo.toml | TASK_1_1, TASK_1_4, TASK_3_1 |
| **主入口** | cis-core/src/lib.rs | TASK_3_2, TASK_3_4 |

### 按任务类型查找

**新增类型**: TASK_1_2 → cis-core/src/types.rs
**新增trait**: TASK_1_3 → cis-core/src/traits/
**模块提取**: TASK_2_1~2_5 → 对应的cis-core/src子目录
**代码重构**: TASK_3_1~3_5 → cis-core/Cargo.toml, cis-core/src/lib.rs
**Agent相关**: TASK_7_1~7_7 → cis-core/src/agent/ (新建)

---

## Worker Agent 使用建议

1. **读取任务文件** - 首先阅读对应的TASK_XX.md了解需求
2. **查找代码映射** - 使用本索引快速定位相关代码文件
3. **精准读取** - 只读取任务相关的代码文件，避免全局搜索
4. **增量修改** - 按任务顺序执行，依赖关系已在任务文件中说明

**示例工作流**:
```
执行 TASK_1_2:
1. 阅读 docs/plan/v1.2.0/task/phase1_cis_common/TASK_1_2_EXTRACT_TYPES.md
2. 读取 cis-core/src/types.rs (375行)
3. 参考 cis-core/Cargo.toml (依赖配置)
4. 创建 cis-common/cis-types/ 目录和文件
```

---

**维护说明**: 当代码结构变化时，及时更新此索引。
