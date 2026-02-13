# CIS v1.1.6 Agent Teams 执行策略

> **日期**: 2026-02-12
> **目的**: 使用 Agent Teams 并行执行，节省主 Agent 上下文空间，保障任务细节不丢失

---

## 1. 问题分析

### 1.1 主 Agent 上下文限制

**当前问题**:
- 主 Agent (Claude Sonnet 4.5) 上下文窗口有限（200K tokens）
- 大量任务定义和代码会快速消耗上下文
- 任务执行细节容易被截断或丢失
- 长时间执行导致上下文碎片化

**影响**:
- ❌ 无法同时处理多个大型任务
- ❌ 任务间上下文传递困难
- ❌ 执行结果可能不完整
- ❌ 代码审阅质量下降

### 1.2 Agent Teams 解决方案

**核心思路**:
```
主 Agent (协调器)
    ↓
分配具体任务 → Sub Agents (独立上下文)
    ↓
收集结果 → 主 Agent (汇总)
```

**优势**:
- ✅ 每个 Sub Agent 拥有独立上下文（200K tokens）
- ✅ 主 Agent 只负责任务分配和结果汇总
- ✅ Sub Agent 可以并行执行（7个 Teams）
- ✅ 任务细节在独立上下文中完整保留
- ✅ 主 Agent 上下文消耗降低 >80%

---

## 2. Agent Teams 架构

### 2.1 Teams 定义（更新）

基于 [V1.1.6_INTEGRATED_EXECUTION_PLAN.md](./V1.1.6_INTEGRATED_EXECUTION_PLAN.md)，添加记忆系统 Teams：

| Team ID | 名称 | 负责模块 | Runtime | 并发数 | 能力 |
|---------|------|-----------|---------|---------|------|
| **Team-V-CLI** | CLI 架构修复 | cis-node CLI | Claude | 3 | CodeReview, ModuleRefactoring |
| **Team-Q-Core** | 核心模块拆分 | scheduler, config | Claude | 5 | ModuleRefactoring, PerformanceOptimization |
| **Team-R-Config** | 配置模块 | config | Claude | 3 | ModuleRefactoring |
| **Team-V-Memory** | 记忆系统改版 | memory (weekly) | Claude | 4 | ModuleRefactoring, PerformanceOptimization |
| **Team-T-Skill** | Skill 系统 | skill | Claude | 3 | ModuleRefactoring, TestWriting |
| **Team-S-P2P** | P2P 网络 | p2p | Claude | 3 | ModuleRefactoring |
| **Team-U-Other** | 其他模块 | agent, decision, etc. | Claude | 3 | ModuleRefactoring, TestWriting |
| **Team-M-Archive** | 记忆归档 | memory (archive) | Claude | 2 | ModuleRefactoring |

**新增**:
- **Team-V-Memory**: 专门负责记忆系统改版（54周分db、精准索引）
- **Team-M-Archive**: 专门负责记忆归档和清理

### 2.2 Agent Pool 配置

```toml
# ~/.cis/config.toml

[agent_pool]
name = "cis-v1.1.6-refactor"
max_teams = 8
max_concurrent_tasks = 20

[agent_pool.teams]
# ... 现有 Teams 配置 ...

# 新增: 记忆系统 Teams
[[agent_pool.teams]]
id = "Team-V-Memory"
name = "Memory Refactor Team"
runtime = "claude"
max_concurrent = 4
capabilities = ["ModuleRefactoring", "PerformanceOptimization"]
work_dir = "~/cis/cis-core/src/memory"

[[agent_pool.teams]]
id = "Team-M-Archive"
name = "Memory Archive Team"
runtime = "claude"
max_concurrent = 2
capabilities = ["ModuleRefactoring"]
work_dir = "~/cis/cis-core/src/memory"
```

---

## 3. 任务分配策略

### 3.1 主 Agent 角色

**协调器职责**（不执行具体代码）:
1. **任务拆分**: 将大型任务拆分为可并行的子任务
2. **依赖解析**: 构建 DAG，确定执行顺序
3. **Team 分配**: 根据任务类型和能力匹配 Team
4. **进度监控**: 跟踪各 Team 执行进度
5. **结果汇总**: 收集各 Team 结果，生成报告
6. **冲突解决**: 处理 Team 间的依赖冲突

**主 Agent 上下文使用**:
- 任务定义: ~5K tokens
- DAG 结构: ~10K tokens
- Team 分配: ~5K tokens
- 进度信息: ~5K tokens
- **总计**: ~25K tokens（仅 12.5% 上下文）

### 3.2 Sub Agent 角色

**执行职责**（专注具体任务）:
1. **接收任务**: 从主 Agent 获取任务定义
2. **读取代码**: 独立读取相关代码文件
3. **执行任务**: 具体的重构、实现、测试
4. **报告结果**: 向主 Agent 返回结果和变更

**Sub Agent 上下文使用**（每个独立）:
- 任务 prompt: ~30K tokens
- 相关代码: ~100K tokens
- 执行输出: ~50K tokens
- **总计**: ~180K tokens（独立上下文，不影响主 Agent）

### 3.3 上下文传递机制

**主 Agent → Sub Agent**:
```json
{
  "task_id": "M-1",
  "task_name": "记忆周归档实现",
  "prompt_template": "...",
  "context": {
    "files_to_read": [
      "cis-core/src/memory/weekly_archived.rs",
      "docs/plan/v1.1.6/MEMORY_ARCHITECTURE_DESIGN.md"
    ],
    "output_format": "rust_code",
    "test_requirements": ["unit", "integration"]
  },
  "dependencies": []
}
```

**Sub Agent → 主 Agent**:
```json
{
  "task_id": "M-1",
  "status": "completed",
  "result": {
    "files_created": ["cis-core/src/memory/weekly_archived.rs"],
    "files_modified": ["cis-core/src/memory/mod.rs"],
    "tests_added": 15,
    "performance_metrics": {
      "query_time_ms": 15,
      "index_ratio": 0.1
    }
  },
  "next_tasks": ["M-2"],
  "blocking_issues": []
}
```

---

## 4. 执行流程

### 4.1 准备阶段（主 Agent）

```bash
# Step 1: 加载所有任务定义
cis task load-tasks docs/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Step 2: 构建 DAG
cis dag build --tasks "M-1,M-2,M-3,V-1,V-2,..."

# Step 3: 依赖解析和拓扑排序
cis dag resolve --topological-sort

# Step 4: 创建 Agent Pool
cis agent pool create --config docs/plan/v1.1.6/AGENT_POOL_CONFIG.toml
```

### 4.2 并行执行阶段（Sub Agents）

```bash
# 主 Agent 分配任务到 Teams
cis agent pool assign-tasks --parallel

# Team-V-CLI 执行 V-1（独立 Sub Agent）
# - Sub Agent 上下文: 200K tokens
# - 读取: cis-node/src/cli/handlers/*
# - 执行: 重构 handlers
# - 输出: 变更文件列表

# Team-V-Memory 执行 M-1（独立 Sub Agent）
# - Sub Agent 上下文: 200K tokens
# - 读取: cis-core/src/memory/*
# - 执行: 实现周归档
# - 输出: 新代码文件

# Team-Q-Core 执行 V-2（独立 Sub Agent）
# - Sub Agent 上下文: 200K tokens
# - 读取: cis-core/src/scheduler/*
# - 执行: 拆分模块
# - 输出: 模块结构
```

### 4.3 监控和协调阶段（主 Agent）

```bash
# 主 Agent 监控进度（低上下文消耗）
cis agent pool watch --interval 60s

# 自动检测:
# - 任务完成
# - 依赖解除
# - 新任务可启动
# - 冲突和阻塞

# 自动动作:
# - 分配就绪任务
# - 收集完成结果
# - 更新 DAG 状态
# - 生成进度报告
```

### 4.4 汇总阶段（主 Agent）

```bash
# 收集所有 Team 结果
cis agent pool collect-results

# 生成执行报告
cis agent pool report --format markdown \
    --output docs/plan/v1.1.6/EXECUTION_REPORT.md

# 包含:
# - 任务完成情况
# - 代码变更统计
# - 性能指标
# - 遗留问题
# - 下一步建议
```

---

## 5. 通信协议

### 5.1 任务协议（Task Protocol）

```rust
// cis-core/src/task/protocol.rs

use serde::{Deserialize, Serialize};

/// 任务分配消息（主 Agent → Sub Agent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub task_name: String,
    pub task_type: TaskType,
    pub priority: TaskPriority,

    /// 任务 Prompt（详细执行指令）
    pub prompt_template: String,

    /// 上下文变量（文件路径、配置等）
    pub context_variables: TaskContext,

    /// 依赖任务
    pub dependencies: Vec<String>,

    /// 验收标准
    pub acceptance_criteria: Vec<String>,

    /// 超时时间（秒）
    pub timeout_secs: u64,
}

/// 任务上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    /// 需要读取的文件
    pub files_to_read: Vec<String>,

    /// 参考文档
    pub reference_docs: Vec<String>,

    /// 输出格式
    pub output_format: OutputFormat,

    /// 测试要求
    pub test_requirements: TestRequirements,
}

/// 任务结果消息（Sub Agent → 主 Agent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskCompletionStatus,

    /// 执行结果详情
    pub execution: TaskExecution,

    /// 产生的变更
    pub changes: CodeChanges,

    /// 性能指标
    pub metrics: PerformanceMetrics,

    /// 下一步任务建议
    pub next_tasks: Vec<String>,

    /// 阻塞问题
    pub blocking_issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChanges {
    pub files_created: Vec<String>,
    pub files_modified: Vec<String>,
    pub files_deleted: Vec<String>,
    pub lines_added: usize,
    pub lines_removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub execution_time_secs: f64,
    pub memory_used_mb: f64,
    pub test_coverage: Option<f64>,
    pub custom_metrics: serde_json::Value,
}
```

### 5.2 消息总线协议

```rust
// cis-core/src/agent/message_bus.rs

/// 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// 任务分配（主 → Sub）
    TaskAssigned(TaskAssignment),

    /// 任务完成（Sub → 主）
    TaskCompleted(TaskResult),

    /// 进度更新（Sub → 主）
    ProgressUpdate(ProgressUpdate),

    /// 阻塞报告（Sub → 主）
    BlockReport(Issue),

    /// 依赖就绪（主 → Sub）
    DependencyReady(String),

    /// 取消任务（主 → Sub）
    TaskCancelled(String),
}

/// 进度更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub task_id: String,
    pub progress_percent: f64,
    pub current_step: String,
    pub estimated_remaining_secs: u64,
}
```

---

## 6. 实现示例

### 6.1 主 Agent 伪代码

```rust
// Main Agent: 任务协调器

use cis_core::task::{TaskRepository, DagBuilder};
use cis_core::agent::pool::AgentPool;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 加载任务定义（低上下文消耗）
    let tasks = load_tasks_from_toml("docs/plan/v1.1.6/TASKS_DEFINITIONS.toml")?;

    // 2. 构建 DAG（自动化依赖解析）
    let task_repo = TaskRepository::new(db_pool);
    let mut dag_builder = DagBuilder::new(task_repo);
    let dag = dag_builder.build(&task_ids).await?;

    // 3. 拓扑排序，确定执行顺序
    let execution_levels = dag.get_execution_levels();

    // 4. 创建 Agent Pool
    let pool = AgentPool::new(config).await?;

    // 5. 按层级分配任务到 Teams（节省上下文）
    for (level, task_ids) in execution_levels.iter().enumerate() {
        println!("Level {}: {} tasks", level, task_ids.len());

        // 并行分配同一层的所有任务
        let assignments = pool.assign_tasks_to_teams(task_ids).await?;

        // 等待该层完成（不消耗上下文）
        pool.wait_for_level_completion(level).await?;

        // 收集结果（仅摘要，不加载详细代码）
        let summaries = pool.collect_summaries(task_ids).await?;

        // 更新 DAG 状态
        dag.update_task_statuses(&summaries).await?;
    }

    // 6. 生成最终报告（低上下文消耗）
    let report = pool.generate_final_report().await?;
    report.save_to("docs/plan/v1.1.6/EXECUTION_REPORT.md")?;

    Ok(())
}
```

### 6.2 Sub Agent 伪代码

```rust
// Sub Agent: 任务执行器

use cis_core::agent::message_bus::AgentMessage;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 接收任务分配
    let assignment: TaskAssignment = receive_from_main_agent().await?;

    println!("收到任务: {}", assignment.task_name);

    // 2. 读取相关文件（独立上下文）
    let code_context = read_files(&assignment.context.files_to_read).await?;

    // 3. 执行任务（使用完整 200K 上下文）
    let result = execute_task(
        &assignment.prompt_template,
        &code_context,
        &assignment.context,
    ).await?;

    // 4. 运行测试（如果有）
    if assignment.context.test_requirements.run_tests {
        run_tests(&assignment.context.test_requirements).await?;
    }

    // 5. 收集变更（git diff）
    let changes = collect_code_changes().await?;

    // 6. 发送结果给主 Agent（不阻塞主 Agent）
    send_to_main_agent(AgentMessage::TaskCompleted(TaskResult {
        task_id: assignment.task_id,
        status: TaskCompletionStatus::Completed,
        execution: result,
        changes,
        metrics: collect_metrics(),
        next_tasks: vec![],
        blocking_issues: vec![],
    })).await?;

    Ok(())
}
```

---

## 7. 上下文优化

### 7.1 主 Agent 上下文节省

**优化策略**:

| 优化项 | 节省 | 方法 |
|--------|--------|------|
| 任务定义传递 | ~15K tokens | 使用任务 ID 引用，不内联 prompt |
| 代码文件传递 | ~50K tokens | Sub Agent 自行读取，主 Agent 不传输代码 |
| 执行详情传递 | ~80K tokens | 只传递摘要和元数据 |
| DAG 结构存储 | ~10K tokens | 存储在数据库，不保留在上下文 |
| 进度信息传递 | ~5K tokens | 增量更新，不重复历史 |

**总计节省**: ~160K tokens (80% 上下文)

### 7.2 Sub Agent 上下文独立

**独立上下文优势**:

| Agent | 上下文 | 职责 |
|--------|---------|--------|
| Sub Agent 1 | 200K tokens | 专注执行 M-1（记忆周归档） |
| Sub Agent 2 | 200K tokens | 专注执行 V-1（CLI 架构） |
| Sub Agent 3 | 200K tokens | 专注执行 V-2（scheduler 拆分） |
| ... | ... | ... |

**无干扰**:
- ✅ Sub Agent 之间不共享上下文
- ✅ 每个 Agent 有完整的代码理解空间
- ✅ 执行细节不会因为上下文限制丢失
- ✅ 测试和代码审查有充足空间

---

## 8. 执行命令

### 8.1 启动并行执行（更新脚本）

```bash
# 使用更新的启动脚本
./cis-v1.1.6-start-parallel.sh \
    --max-teams 8 \
    --agent-context-limit 180000 \
    --main-context-limit 25000 \
    --parallel-levels
```

**脚本更新点**:
- [ ] 添加 Team-V-Memory 定义
- [ ] 添加 Team-M-Archive 定义
- [ ] 更新任务分配（包含 M-1, M-2, M-3）
- [ ] 配置上下文传递协议
- [ ] 实现消息总线

### 8.2 监控命令

```bash
# 查看所有 Team 状态
cis agent pool status --detail

# 查看特定 Team
cis agent pool team-status Team-V-Memory

# 查看任务分配
cis agent pool assignments --active

# 查看上下文使用
cis agent pool context-usage --per-team
```

---

## 9. 成功指标

### 9.1 上下文效率

| 指标 | 目标 | 测量方法 |
|--------|--------|----------|
| 主 Agent 上下文使用 | < 50K | 统计主 Agent token 消耗 |
| Sub Agent 上下文使用 | < 180K | 每个 Sub Agent 独立统计 |
| 上下文传递大小 | < 5K/消息 | 消息序列化后大小 |
| 任务完成率 | > 95% | 完成任务数 / 总任务数 |

### 9.2 并行效率

| 指标 | 目标 | 测量方法 |
|--------|--------|----------|
| Team 并行度 | 5-8 Teams | 同时运行的 Team 数量 |
| 任务吞吐量 | > 10 任务/天 | 完成任务数 / 时间 |
| 平均任务时长 | < 4 小时 | 任务执行时间统计 |
| 等待时间 | < 30 分钟 | 任务等待依赖时间 |

### 9.3 质量指标

| 指标 | 目标 | 测量方法 |
|--------|--------|----------|
| 代码审查通过率 | > 90% | Peer review 结果 |
| 测试覆盖率 | > 80% | tarpaulin 统计 |
| 编译成功率 | > 95% | cargo build 成功率 |
| 任务细节完整性 | 100% | 结果摘要完整性检查 |

---

## 10. 相关文档

- [V1.1.6_INTEGRATED_EXECUTION_PLAN.md](./V1.1.6_INTEGRATED_EXECUTION_PLAN.md) - 完整执行计划
- [TASK_DAG_WORKFLOW_DESIGN.md](./TASK_DAG_WORKFLOW_DESIGN.md) - DAG 工作流设计
- [AGENT_POOL_MULTI_RUNTIME_DESIGN.md](./AGENT_POOL_MULTI_RUNTIME_DESIGN.md) - Agent Pool 设计
- [MEMORY_DAG_INTEGRATION.md](./MEMORY_DAG_INTEGRATION.md) - 记忆系统 DAG 集成

---

**文档版本**: 1.0
**创建日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 设计完成，准备执行
