# CIS 四级决策机制 (Four-Tier Decision Mechanism)

## 概述

CIS 四级决策机制是一个分层的任务执行权限管理系统，根据任务的风险等级和重要性，提供从完全自动化到人工仲裁的不同执行策略。

## 四级决策层级

### 1. Mechanical (机械级) ✅

**特点**：
- 完全自动执行
- 失败时自动重试
- 无需人工干预

**适用场景**：
- 低风险、高确定性的任务
- 数据备份、日志清理等例行操作
- 已经过充分测试的标准流程

**配置示例**：
```toml
level = { type = "mechanical", retry = 3 }
```

### 2. Recommended (推荐级) ⏱️

**特点**：
- 倒计时后自动执行默认动作
- 用户可在倒计时期间中断
- 超时行为可配置（执行/跳过/中止）

**适用场景**：
- 中等风险操作
- 需要用户知情但不需要显式确认的任务
- 批量处理、资源分配等操作

**配置示例**：
```toml
level = { type = "recommended", default_action = "execute", timeout_secs = 30 }
```

**CLI 命令**：
```bash
# 设置任务为推荐级别
cis task-level recommended <task-id> --timeout 30 --default-action execute

# 查看倒计时进度（执行时自动显示）
cis dag status --run-id <run-id>
```

### 3. Confirmed (确认级) ⏸️

**特点**：
- 暂停执行等待用户显式确认
- 超时后自动取消
- 支持 CLI 和 GUI 确认

**适用场景**：
- 高风险操作
- 数据修改、系统变更等关键操作
- 需要人工审核的决策点

**配置示例**：
```toml
level = { type = "confirmed" }
```

**CLI 命令**：
```bash
# 设置任务为确认级别
cis task-level confirmed <task-id>

# 查看待确认的决策
 cis decision status

# 确认执行
cis decision confirm <request-id>

# 拒绝执行
cis decision reject <request-id>
```

### 4. Arbitrated (仲裁级) 🗳️

**特点**：
- 暂停执行并启动多方投票
- 需要利益相关者达成共识
- 支持可配置的投票阈值

**适用场景**：
- 高风险、高影响的操作
- 需要多方共识的重大决策
- 跨团队协作的关键节点

**配置示例**：
```toml
level = { type = "arbitrated", stakeholders = ["alice", "bob", "charlie"] }
```

**CLI 命令**：
```bash
# 设置任务为仲裁级别
cis task-level arbitrated <task-id> --stakeholders alice,bob,charlie

# 查看活跃仲裁
 cis decision arbitrations

# 参与投票
cis decision vote <vote-id> --stakeholder <name> --approve
# 或
cis decision vote <vote-id> --stakeholder <name> --reject
```

## 配置

### 配置文件

配置文件路径：`~/.config/cis/decision.toml`

```toml
# Recommended 级别超时时间（秒）
timeout_recommended = 30

# Confirmed 级别超时时间（秒）
timeout_confirmed = 300

# Arbitrated 级别超时时间（秒）
timeout_arbitrated = 3600

# 是否显示倒计时进度
show_countdown = true

# 是否启用交互式确认
interactive = true

# 仲裁投票通过阈值（0.0-1.0）
arbitration_threshold = 0.5
```

### 环境变量

可以通过环境变量覆盖配置：

```bash
export CIS_DECISION_TIMEOUT_RECOMMENDED=60
export CIS_DECISION_TIMEOUT_CONFIRMED=600
export CIS_DECISION_TIMEOUT_ARBITRATED=7200
export CIS_DECISION_SHOW_COUNTDOWN=true
export CIS_DECISION_INTERACTIVE=true
export CIS_DECISION_ARBITRATION_THRESHOLD=0.66
```

### 初始化配置

```bash
cis decision init
```

## DAG 中使用四级决策

### TOML 格式示例

```toml
[dag]
policy = "all_success"

[[dag.tasks]]
id = "low-risk-task"
name = "Low Risk Task"
skill = "backup"
command = "backup-data.sh"
level = { type = "mechanical", retry = 3 }

[[dag.tasks]]
id = "medium-risk-task"
name = "Medium Risk Task"
skill = "deploy"
command = "deploy-staging.sh"
depends_on = ["low-risk-task"]
level = { type = "recommended", default_action = "execute", timeout_secs = 60 }

[[dag.tasks]]
id = "high-risk-task"
name = "High Risk Task"
skill = "deploy"
command = "deploy-production.sh"
depends_on = ["medium-risk-task"]
level = { type = "confirmed" }

[[dag.tasks]]
id = "critical-task"
name = "Critical Task"
skill = "migration"
command = "database-migration.sh"
depends_on = ["high-risk-task"]
level = { type = "arbitrated", stakeholders = ["dba", "sre", "architect"] }
```

### 简单文本格式

```
# 格式: task_id: dependencies [level:LevelType|params]

backup: [level:mechanical]
deploy-staging: backup [level:recommended]
deploy-production: deploy-staging [level:confirmed]
db-migration: deploy-production [level:arbitrated]
```

## 决策流程图

```
┌─────────────────────────────────────────────────────────────────┐
│                        DAG Task Execution                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Check Task     │
                    │    Level        │
                    └────────┬────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
            ▼                ▼                ▼
    ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
    │  Mechanical  │ │  Recommended │ │   Confirmed  │
    │              │ │              │ │              │
    │  Auto-exec   │ │  Countdown   │ │ Wait Confirm │
    │  Retry: N    │ │  Timeout: N  │ │ Timeout: N   │
    └──────┬───────┘ └──────┬───────┘ └──────┬───────┘
           │                │                │
           │           ┌────┴────┐           │
           │           │         │           │
           │           ▼         ▼           ▼
           │      ┌────────┐  ┌────────┐  ┌────────┐
           │      │  Skip  │  │ Abort  │  │ Confirm│
           │      └────┬───┘  └────┬───┘  └────┬───┘
           │           │           │           │
           └───────────┴───────────┴───────────┘
                               │
                               ▼
                    ┌──────────────────┐
                    │    Arbitrated    │
                    │                  │
                    │  Multi-stakeholder│
                    │     Voting       │
                    └────────┬─────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
            ▼                ▼                ▼
      ┌──────────┐    ┌──────────┐    ┌──────────┐
      │ Approved │    │ Rejected │    │ Timeout  │
      └────┬─────┘    └────┬─────┘    └────┬─────┘
           │               │               │
           ▼               ▼               ▼
      ┌──────────┐    ┌──────────┐    ┌──────────┐
      │ Continue │    │  Abort   │    │  Abort   │
      └──────────┘    └──────────┘    └──────────┘
```

## 最佳实践

### 1. 级别选择

- **Mechanical**：低风险、可自动回滚、无副作用的操作
- **Recommended**：中等风险、需要用户知情但可以自动化的操作
- **Confirmed**：高风险、不可逆、需要人工审核的操作
- **Arbitrated**：关键决策、需要多方共识的操作

### 2. 超时设置

- **Recommended**：根据用户反应时间设置，通常 10-60 秒
- **Confirmed**：给用户足够时间评估，通常 5-15 分钟
- **Arbitrated**：考虑跨时区协作，通常 1-24 小时

### 3. 仲裁阈值

- **简单多数 (0.5)**：适用于一般决策
- **2/3 多数 (0.66)**：适用于重要决策
- **全票通过 (1.0)**：适用于关键决策

### 4. 利益相关者

- 选择有决策权的团队成员
- 确保关键角色被覆盖（如 DBA、SRE、架构师）
- 避免利益相关者过多导致决策困难

## 故障排除

### 问题：决策超时

**解决方案**：
```bash
# 查看超时配置
cat ~/.config/cis/decision.toml

# 增加超时时间
cis decision init --force
# 然后编辑配置文件
```

### 问题：无法确认任务

**解决方案**：
```bash
# 检查待确认任务
cis decision status

# 使用完整 ID 确认
cis decision confirm <full-request-id>
```

### 问题：仲裁投票无法通过

**解决方案**：
```bash
# 检查投票状态
cis decision arbitrations

# 降低阈值（在配置文件中）
# 或提醒未投票的利益相关者
```

## API 参考

### Rust API

```rust
use cis_core::decision::{DecisionEngine, DecisionResult};

// 创建决策引擎
let engine = DecisionEngine::new();

// 处理决策
let result = engine.process_decision(&task, &run_id).await;

match result {
    DecisionResult::Allow => println!("Task allowed"),
    DecisionResult::Skip => println!("Task skipped"),
    DecisionResult::Abort => println!("Task aborted"),
    DecisionResult::Pending(id) => println!("Waiting for decision: {}", id),
}
```

### CLI API

```bash
# 决策管理
cis decision confirm <request-id>
cis decision reject <request-id>
cis decision status [--all] [--run-id <id>]
cis decision vote <vote-id> --stakeholder <name> --approve|--reject
cis decision arbitrations [--all]
cis decision init [--force]

# 任务级别管理
cis task-level mechanical <task-id> [--retry <n>]
cis task-level recommended <task-id> [--timeout <secs>] [--default-action <action>]
cis task-level confirmed <task-id>
cis task-level arbitrated <task-id> [--stakeholders <list>]
```

## 相关文档

- [DAG 执行](./dag-execution.md)
- [任务管理](./task-management.md)
- [CLI 使用指南](./cli-guide.md)
