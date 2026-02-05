# CIS-DAG 作为 Skill 执行架构

**文档版本**: 1.0  
**日期**: 2026-02-02  
**状态**: 设计草案

---

## 1. 核心问题：DAG 与 Skill 的关系

### 1.1 当前状态

| 系统 | 用途 | 当前实现 | 问题 |
|------|------|----------|------|
| **Skill** | 可热插拔的功能模块 | 独立 WASM/Native 模块 | 执行方式不明确 |
| **Skill Chain** | 技能流水线编排 | 线性执行 (Vec<ChainStep>) | 无依赖管理 |
| **DAG Scheduler** | 任务依赖调度 | 刚实现的四级决策+债务 | 未连接 Skill |
| **Task** | DAG 中的节点 | 通用任务结构 | 未关联 Skill |

### 1.2 核心洞察

**DAG 应该是 Skill 的执行引擎，而不是独立系统。**

```
当前分离的架构:                    目标统一架构:
┌──────────────┐                   ┌──────────────────────────┐
│   Skill      │  ← 如何执行？     │     Skill (定义)         │
│   (定义)     │                   │     - 输入输出契约       │
└──────────────┘                   │     - 能力声明           │
       │                           └───────────┬──────────────┘
       ↓                                       │
┌──────────────┐                   ┌───────────▼──────────────┐
│  Skill Chain │  ← 线性执行       │  DAG Task (执行实例)     │
│  (线性)      │                   │  - TaskLevel (四级决策)  │
└──────────────┘                   │  - FailureType (债务)    │
       │                           │  - Sandbox (隔离)        │
       ↓                           └───────────┬──────────────┘
┌──────────────┐                               │
│  DAG Task    │  ← 刚实现，                    ↓
│  (独立)      │     未连接 Skill      ┌──────────────┐
└──────────────┘                       │ DAG Scheduler│
                                       │ (执行引擎)   │
                                       └──────────────┘
```

---

## 2. 架构设计：DAG 即 Skill 执行

### 2.1 统一模型

```rust
/// Skill 定义（声明式）
pub struct Skill {
    /// 唯一标识
    pub id: String,
    /// 语义定义
    pub semantics: SkillSemantics,
    /// 输入输出契约
    pub contract: SkillContract,
    /// 能力要求
    pub capabilities: Vec<Capability>,
}

/// Skill 执行方式可以是：
pub enum SkillExecution {
    /// 单一可执行文件
    Binary(PathBuf),
    /// WASM 模块
    Wasm(Vec<u8>),
    /// DAG 编排（复合 Skill）
    Dag(DagDefinition),
}

/// DAG 定义（用于复合 Skill）
pub struct DagDefinition {
    /// DAG 中的任务（每个任务是一个 Skill 调用）
    pub tasks: Vec<SkillTask>,
    /// 执行策略
    pub policy: DagPolicy,
}

/// Skill 任务 = Task + Skill 引用
pub struct SkillTask {
    /// 继承自 Task 的所有字段
    #[serde(flatten)]
    pub task: Task,
    /// 要调用的 Skill ID
    pub skill_id: String,
    /// Skill 参数（运行时填充）
    pub skill_params: serde_json::Value,
}
```

### 2.2 三层架构

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 3: Skill 定义层 (声明式)                               │
├─────────────────────────────────────────────────────────────┤
│  Skill {                                                    │
│    id: "code-review",                                       │
│    semantics: {...},                                        │
│    contract: {...},                                         │
│    execution: Dag(DagDefinition {                           │
│      tasks: [                                               │
│        { skill_id: "git-diff", ... },                       │
│        { skill_id: "ai-analyze", deps: [0], ... },          │
│        { skill_id: "report-gen", deps: [1], ... },          │
│      ]                                                      │
│    })                                                       │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: DAG 执行层 (运行时)                                 │
├─────────────────────────────────────────────────────────────┤
│  DagScheduler {                                             │
│    runs: HashMap<run_id, DagRun>                            │
│  }                                                          │
│                                                             │
│  DagRun {                                                   │
│    dag: TaskDag<SkillTask>,  // Skill 任务的 DAG            │
│    debts: Vec<DebtEntry>,                                   │
│    level_states: HashMap<task_id, LevelState>,              │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Skill 执行层 (隔离)                                 │
├─────────────────────────────────────────────────────────────┤
│  SkillExecutor {                                            │
│    sandbox: Sandbox,                                        │
│    runtime: WasmRuntime | NativeProcess,                    │
│  }                                                          │
│                                                             │
│  执行单个 Skill，提供：                                      │
│  - 进程隔离 (chroot/namespace)                              │
│  - 资源限制 (cpu/mem/disk)                                  │
│  - 输入输出传递                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 关键设计决策

### 3.1 Task 与 Skill 的映射

| DAG Task 字段 | Skill 映射 | 说明 |
|--------------|-----------|------|
| `Task.exec` | `Skill.execution` | 如果 Skill 是 DAG，递归展开 |
| `Task.level` | 四级决策 | 控制是否需要人工介入 |
| `Task.inputs` | `Skill.contract.inputs` | 执行前验证输入存在 |
| `Task.outputs` | `Skill.contract.outputs` | 执行后验证输出产生 |
| `Task.rollback` | `Skill.rollback` | 失败时执行回滚 |
| `Task.sandbox` | `Sandbox` | 执行环境隔离 |

### 3.2 复合 Skill (DAG as Skill)

**场景**: 一个复杂的代码审查流程

```rust
// 定义复合 Skill
let code_review_skill = Skill {
    id: "comprehensive-code-review".to_string(),
    semantics: SkillSemantics::new("代码审查", "完整的代码审查流程"),
    contract: SkillContract {
        inputs: vec!["repo_path".to_string(), "commit_range".to_string()],
        outputs: vec!["review_report".to_string()],
    },
    capabilities: vec![Capability::Git, Capability::AiInference],
    execution: SkillExecution::Dag(DagDefinition {
        tasks: vec![
            SkillTask {
                task: Task::new("1", "获取代码变更", "review")
                    .with_level(TaskLevel::Mechanical { retry: 3 })
                    .with_inputs(vec!["repo_path", "commit_range"]),
                skill_id: "git-diff".to_string(),
                skill_params: json!({ "format": "unified" }),
            },
            SkillTask {
                task: Task::new("2", "AI 分析代码", "review")
                    .with_level(TaskLevel::Confirmed)  // 需要确认
                    .with_deps(vec!["1"])
                    .with_inputs(vec!["diff_output"]),
                skill_id: "ai-code-analyze".to_string(),
                skill_params: json!({ "model": "claude" }),
            },
            SkillTask {
                task: Task::new("3", "生成报告", "review")
                    .with_level(TaskLevel::Mechanical)
                    .with_deps(vec!["2"])
                    .with_rollback(vec!["rm report.md"]),
                skill_id: "report-generator".to_string(),
                skill_params: json!({ "template": "code-review" }),
            },
        ],
        policy: DagPolicy::AllSuccess,
    }),
};
```

**执行流程**:

```
用户调用: cis skill run comprehensive-code-review \
              --repo-path ./my-project \
              --commit-range HEAD~5..HEAD

    ↓

SkillRouter 解析参数
    ↓

发现 Skill 是 DAG 类型
    ↓

DagScheduler.create_run(code_review_skill.dag)
    ↓

Task 1 (git-diff): Mechanical → 自动执行
    - Sandbox 中执行 git diff
    - 输出 diff_output
    ↓

Task 2 (ai-code-analyze): Confirmed → 模态确认
    - GUI: 弹出确认对话框
    - CLI: 等待用户输入 "确认执行 AI 分析？"
    - 用户确认后继续
    ↓

Task 3 (report-generator): Mechanical → 自动执行
    - 生成报告
    - 标记完成
    ↓

返回 review_report 路径给调用者
```

### 3.3 四级决策在 Skill 执行中的体现

| 级别 | Skill 执行行为 | UI 表现 |
|------|---------------|---------|
| **Mechanical** | 直接调用 SkillExecutor | CLI: 进度条<br>GUI: 后台运行 |
| **Recommended** | 倒计时后自动执行 | CLI: "将在 30s 后执行 X，按 Enter 立即执行"<br>GUI: 顶部通知栏 |
| **Confirmed** | 暂停等待用户确认 | CLI: 模态提示<br>GUI: 模态弹窗 |
| **Arbitrated** | 冻结整个 DAG | CLI: 交互式选择<br>GUI: 决策工作区 |

### 3.4 债务机制在 Skill 中的应用

```rust
// 代码审查任务失败
Task 2 (ai-code-analyze) 失败
    ↓
判断失败类型:

情况 A: Ignorable Debt (API 限流)
    - 标记为 Debt(Ignorable)
    - 继续执行 Task 3（使用默认分析）
    - 报告: "本次执行累积 1 项技术债务 (AI 分析未完成)"

情况 B: Blocking Debt (API Key 失效)
    - 标记为 Debt(Blocking)
    - 冻结 DAG，等待用户
    - GUI: 打开决策工作区
    - 用户选择: [修复 API Key] [切换到本地模型] [回滚]
```

---

## 4. 实现路径

### 4.1 立即修改（保持兼容）

#### 4.1.1 修改 Task 结构添加 skill_id

```rust
// cis-core/src/types.rs
pub struct Task {
    // ... 现有字段
    
    /// 关联的 Skill ID（可选）
    pub skill_id: Option<String>,
    
    /// Skill 参数
    pub skill_params: Option<serde_json::Value>,
}

impl Task {
    /// 创建调用 Skill 的任务
    pub fn for_skill(skill_id: impl Into<String>) -> Self {
        Self {
            // ... 默认值
            skill_id: Some(skill_id.into()),
            skill_params: None,
            level: TaskLevel::Mechanical { retry: 3 },
        }
    }
}
```

#### 4.1.2 添加 SkillTaskDag 类型

```rust
// cis-core/src/scheduler/skill_dag.rs

/// 专门用于执行 Skill 的 DAG
pub struct SkillTaskDag {
    /// 基础 DAG
    dag: TaskDag,
    /// Skill 上下文
    skill_context: SkillContext,
}

pub struct SkillContext {
    /// 父 Skill ID（如果是嵌套 DAG）
    pub parent_skill: Option<String>,
    /// 全局输入参数
    pub global_inputs: serde_json::Value,
    /// 中间结果缓存
    pub intermediate_results: HashMap<String, serde_json::Value>,
}
```

#### 4.1.3 创建 SkillDagExecutor

```rust
// cis-core/src/scheduler/skill_executor.rs

/// 执行 Skill DAG
pub struct SkillDagExecutor {
    scheduler: DagScheduler,
    skill_manager: Arc<SkillManager>,
    sandbox: Sandbox,
}

impl SkillDagExecutor {
    /// 执行 Skill（支持复合 Skill）
    pub async fn execute_skill(
        &self,
        skill_id: &str,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let skill = self.skill_manager.get(skill_id)?;
        
        match skill.execution {
            SkillExecution::Binary(path) => {
                // 直接执行二进制
                self.execute_binary(path, inputs).await
            }
            SkillExecution::Wasm(bytes) => {
                // WASM 执行
                self.execute_wasm(bytes, inputs).await
            }
            SkillExecution::Dag(dag_def) => {
                // DAG 执行（递归）
                self.execute_dag_skill(dag_def, inputs).await
            }
        }
    }
    
    /// 执行 DAG 类型的 Skill
    async fn execute_dag_skill(
        &self,
        dag_def: DagDefinition,
        inputs: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // 1. 创建 DAG 运行
        let run_id = self.scheduler.create_run(dag_def.into_task_dag());
        
        // 2. 注入全局输入到第一个任务
        self.inject_inputs(&run_id, inputs);
        
        // 3. 执行 DAG
        while let Some(ready_task) = self.scheduler.get_ready_task(&run_id) {
            // 检查决策级别
            match self.check_permission(&ready_task) {
                PermissionResult::AutoApprove => {}
                PermissionResult::NeedsConfirmation => {
                    self.wait_for_confirmation(&ready_task).await?;
                }
                // ... 其他级别
            }
            
            // 执行 Skill
            let task = ready_task.task;
            if let Some(skill_id) = &task.skill_id {
                let result = self.execute_skill(
                    skill_id,
                    task.skill_params.clone().unwrap_or_default()
                ).await;
                
                // 处理结果
                match result {
                    Ok(output) => {
                        self.scheduler.mark_completed(&run_id, &task.id)?;
                        self.store_intermediate_result(&run_id, &task.id, output);
                    }
                    Err(e) => {
                        // 判断失败类型
                        let failure_type = self.classify_error(&e);
                        self.scheduler.mark_failed_with_type(
                            &run_id, 
                            &task.id, 
                            failure_type,
                            e.to_string()
                        )?;
                    }
                }
            }
        }
        
        // 4. 收集最终输出
        self.collect_outputs(&run_id)
    }
}
```

### 4.2 中期重构

#### 4.2.1 统一 Skill Chain 和 DAG

```rust
// 将 Skill Chain 改为 DAG 的特例
pub type SkillChain = SkillTaskDag;

impl SkillTaskDag {
    /// 从旧版 Skill Chain 迁移
    pub fn from_chain(chain: SkillChain) -> Self {
        // 将线性 chain 转换为 DAG
        // chain[0] → chain[1] → chain[2] ...
    }
}
```

#### 4.2.2 添加 Skill Manifest 支持 DAG

```toml
# skill.toml
[skill]
name = "comprehensive-code-review"
type = "dag"  # 新增类型

[[dag.tasks]]
id = "1"
skill = "git-diff"
level = "mechanical"
retry = 3

[[dag.tasks]]
id = "2"
skill = "ai-code-analyze"
level = "confirmed"
deps = ["1"]

[[dag.tasks]]
id = "3"
skill = "report-generator"
level = "mechanical"
deps = ["2"]
rollback = ["rm report.md"]
```

---

## 5. CLI/GUI 设计

### 5.1 CLI 设计

```bash
# 执行 Skill（自动判断是否为 DAG）
cis skill run comprehensive-code-review \
    --repo-path ./my-project \
    --commit-range HEAD~5..HEAD \
    --level confirmed  # 覆盖默认级别

# DAG 特定命令
cis dag run my-dag.toml              # 从文件加载 DAG
cis dag status --watch               # 实时查看进度
cis dag pause --run-id <id>          # 暂停执行
cis dag resume --run-id <id>         # 恢复执行
cis dag amend --task <id> --env K=V  # 热修改任务

# 债务管理
cis debt list                        # 查看累积债务
cis debt resolve <task-id>           # 解决债务
cis debt summary                     # 债务统计
```

### 5.2 TUI 设计（类似 cargo）

```
$ cis dag status --watch

DAG Run: comprehensive-code-review (a1b2c3d4)
Status: Running (1 failed, 2 completed, 1 running, 1 pending)
Debt: 1 Ignorable

Progress: [████████░░░░░░░░░░░░] 40%
ETA: 2m 30s

Tasks:
  ✓ git-diff            completed    2s
  ✓ ai-analyze          completed   15s
  ⚠ security-scan       failed       5s  (Ignorable debt)
  → report-gen          running     10s
  ⏳ notify-slack        pending      -

Debt Summary:
  1 Ignorable: security-scan (API timeout, continuing with basic check)

Press 'q' to quit, 'd' for details, 'r' to resolve debt
```

---

## 6. 总结

### 6.1 关键结论

1. **DAG 是 Skill 的执行引擎**：每个 Skill 可以是原子操作（Binary/WASM）或复合操作（DAG）

2. **四级决策统一 Skill 执行**：无论 Skill 是简单还是复杂，都遵循相同的决策机制

3. **债务机制提供容错**：Skill 失败不一定是致命错误，可以累积为技术债务继续执行

4. **向后兼容**：现有 Skill Chain 可以作为 DAG 的特例迁移

### 6.2 下一步行动

| 优先级 | 任务 | 文件 | 工作量 |
|-------|------|------|--------|
| 🔴 P0 | Task 添加 skill_id 字段 | `types.rs` | 2h |
| 🔴 P0 | 创建 SkillDagExecutor | `scheduler/skill_executor.rs` | 1d |
| 🟡 P1 | Skill 支持 DAG 类型 | `skill/manifest.rs` | 4h |
| 🟡 P1 | TUI 进度显示 | `cis-node/src/` | 1d |
| 🟢 P2 | GUI 决策界面完善 | `cis-gui/` | 2d |
| 🟢 P2 | Skill Chain 迁移 | `skill/chain.rs` | 4h |

---

**核心思想**: "Every Skill is a DAG, every DAG is a Skill execution."
