# CIS v1.1.6 任务执行 DAG 工作流

> **设计日期**: 2026-02-12
> **目标**: 基于 SQLite 任务存储，使用 Agent Teams 并发执行 v1.1.6 开发
> **核心原则**: 任务去耦、Agent 可替换、Session 复用、智能分配

---

## 1. DAG 工作流设计

### 1.1 总体流程图

```
┌──────────────────────────────────────────────────────────────────────┐
│                    用户任务                             │
│                         ↓                                  │
│              ┌─────────────┐                           │
│              │  Task       │                           │
│              │  Creator     │                           │
│              └──────┬──────┘                           │
│                     │                                  │
│              ┌────────▼─────────┐                       │
│              │                  │                        │
│         ┌────▼────┐   │                        │
│         │ DAG Builder   │   │                        │
│         └──┬────┘───┘   │                        │
│             │              │                             │
│        ┌─▼──▼──┐                            │
│        │  Dependencies│                            │
│        └──┬────────┘                            │
│             │  │                                    │
│        ┌──▼──▼────┐                            │
│        │  Ready Tasks  │                            │
│        │  └──┬───────┘                            │
│             │  │                                     │
│      ┌────▼────▼────▼──┐                         │
│      │  Task Assigner  │                         │
│      │  └──┬───────────┘                         │
│                   │  │                                  │
│              ┌───▼──────────▼──┐                     │
│              │                   │  │                        │
│         ┌────▼────────┐    │  │                        │
│         │  Agent Pool     │    │  │                        │
│         │ └──┬───────┘────│  │                        │
│              │  │              │   │                        │
│      ┌─────▼────▼────▼─────▼┐                     │
│      │ Team Q    Team V   Team R  │                        │
│      │ (scheduler) (CLI) (config) │                        │
│      │  +more teams...         │                        │
│      └───────────────────────────┘                        │
│                   │  │                                  │
│              ┌──▼──────────▼──▼──┐                        │
│              │  Task Executor  │  │                        │
│              │  └──┬───────┬────┘│                        │
│                   │  │              │  │   │                        │
│              ┌────▼────▼──────▼──▼──▼─┐                     │
│              │    Sessions    Events  Results  │                        │
│              └───────────────────────────────┘                        │
└──────────────────────────────────────────────────────────────────────┘
```

### 1.2 阶段定义

| 阶段 | 职责 | 输入 | 输出 |
|------|------|------|------|------|
| **Task Creator** | 定义任务 | 用户请求 | Task 对象 |
| **DAG Builder** | 分析依赖、构建 DAG | Task 对象 | Ready Task 对象集合 |
| **Dependency Resolver** | 解析依赖关系 | Ready Task 对象 | 解析后的 Task 对象 |
| **Task Assigner** | 分配任务到 Team | Ready Task 对象 | Assignment 对象 |
| **Agent Pool** | 管理 Teams 和 Sessions | Assignment 对象 | 执行结果 |
| **Task Executor** | 执行单个任务 | Task 对象 + Session | TaskResult |
| **Result Collector** | 收集结果 | TaskResult 对象 | 完成报告 |

---

## 2. 核心数据结构

### 2.1 任务对象（存储到 SQLite）

```rust
/// 任务实体（数据库存储）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntity {
    /// 主键
    pub id: i64,

    /// 任务 ID（业务标识，如 V-1, V-2）
    #[serde(rename = "task_id")]
    pub task_id: String,

    /// 任务名称
    pub name: String,

    /// 任务类型
    #[serde(rename = "type")]
    pub task_type: TaskType,

    /// 优先级
    pub priority: TaskPriority,

    /// Prompt 模板
    pub prompt_template: String,

    /// 上下文变量（JSON）
    #[serde(rename = "context_variables")]
    pub context_variables: String,

    /// 描述
    pub description: Option<String>,

    /// 预估工作量（人日）
    pub estimated_effort_days: f64,

    /// 任务状态
    pub status: TaskStatus,

    /// 引擎类型（如果需要）
    pub engine_type: Option<String>,

    /// 引擎上下文 ID（关联到 engine_contexts 表）
    pub engine_context_id: Option<i64>,

    /// 依赖任务 ID（JSON 数组）
    #[serde(rename = "dependencies")]
    pub dependencies: String,

    /// 分配的 Team ID
    pub assigned_team_id: Option<String>,

    /// 分配的 Session ID
    pub assigned_session_id: Option<i64>,

    /// 任务结果
    pub result_json: Option<String>,

    /// 错误信息
    pub error_message: Option<String>,

    /// 创建时间
    #[serde(rename = "created_at")]
    pub created_at: i64,

    /// 更新时间
    #[serde(rename = "updated_at")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskType {
    #[serde(rename = "module_refactoring")]
    ModuleRefactoring,

    #[serde(rename = "engine_code_injection")]
    EngineCodeInjection,

    #[serde(rename = "performance_optimization")]
    PerformanceOptimization,

    #[serde(rename = "code_review")]
    CodeReview,

    #[serde(rename = "test_writing")]
    TestWriting,

    #[serde(rename = "documentation")]
    Documentation,

    #[serde(rename = "composite")]
    Composite, // 包含多个子任务
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    #[serde(rename = "p0")]
    P0,

    #[serde(rename = "p1")]
    P1,

    #[serde(rename = "p2")]
    P2,

    #[serde(rename = "p3")]
    P3,
}
```

### 2.2 DAG 节点对象

```rust
/// DAG 节点（可执行单元）
#[derive(Debug, Clone)]
pub struct DagNode {
    /// 节点 ID
    pub id: String,

    /// 对应的任务 ID
    pub task_id: i64,

    /// 节点类型
    pub node_type: DagNodeType,

    /// 节点名称
    pub name: String,

    /// 描述
    pub description: Option<String>,

    /// 前置依赖（必须完成的任务 ID 列表）
    pub dependencies: Vec<i64>,

    /// 后置依赖（必须完成的任务 ID 列表）
    pub dependents: Vec<i64>,

    /// 执行参数
    pub params: serde_json::Value,

    /// 执行策略
    pub execution_strategy: ExecutionStrategy,

    /// 重试策略
    pub retry_strategy: RetryStrategy,

    /// 超时时间（秒）
    pub timeout_secs: u64,

    /// 预期执行时间（秒）
    pub estimated_duration_secs: u64,

    /// 实际执行时间（秒）
    pub actual_duration_secs: Option<u64>,

    /// 执行次数
    pub execution_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DagNodeType {
    /// 任务节点
    Task,

    /// 引擎代码扫描节点
    EngineScan,

    /// 并行节点
    Parallel,

    /// 串行节点
    Sequential,

    /// 条件节点
    Conditional,

    /// 循环节点
    Loop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// 机械级：自动执行，失败重试
    Mechanical {
        max_retries: u32,
    retry_delay_secs: u64,
    },

    /// 推荐级：执行但可撤销
    Recommended {
        timeout_secs: u64,
        cancel_after_secs: u64,
    },

    /// 确认级：需要人工确认
    Confirmed {
        timeout_secs: u64,
    prompt_user: bool,
    },

    /// 仲裁级：多方投票
    Arbitrated {
        timeout_hours: u64,
        min_stakeholders: u32,
    },
}
```

---

## 3. DAG 构建器实现

```rust
use rusqlite::{Connection, Result};
use crate::task::entity::TaskEntity;
use crate::dag::node::DagNode;
use std::collections::{HashMap, HashSet};

/// DAG 构建器
pub struct DagBuilder {
    db: Arc<Mutex<Connection>>,
    task_cache: HashMap<i64, TaskEntity>,
}

impl DagBuilder {
    pub fn new(db: Arc<Mutex<Connection>>) -> Self {
        Self {
            db,
            task_cache: HashMap::new(),
        }
    }

    /// 从单个任务 ID 构建 DAG
    pub async fn build_from_task(&self, task_id: i64) -> Result<DagNode> {
        // 1. 加载任务
        let task = self.load_task(task_id).await?;

        // 2. 解析依赖关系
        let dependencies = Self::parse_dependencies(&task.dependencies)?;

        // 3. 创建 DAG 节点
        let node = DagNode {
            id: task.task_id.clone(),
            task_id,
            node_type: DagNodeType::Task,
            name: task.name.clone(),
            description: task.description,
            dependencies,
            dependents: vec![],
            params: serde_json::json!({
                "task_id": task.task_id,
                "type": task.task_type,
                "priority": task.priority,
                "prompt_template": task.prompt_template,
                "context_variables": task.context_variables,
            }),
            execution_strategy: Self::determine_strategy(&task),
            retry_strategy: RetryStrategy::default(),
            timeout_secs: task.estimated_effort_days * 8 * 3600, // 1 人日 = 8 小时
            estimated_duration_secs: task.estimated_effort_days * 8 * 3600,
        };

        Ok(node)
    }

    /// 从任务列表构建复合 DAG
    pub async fn build_from_tasks(&self, task_ids: &[i64]) -> Result<DagNode> {
        // 1. 创建复合任务节点
        let mut task_ids = task_ids.to_vec();
        task_ids.sort(); // 确保 ID 顺序

        // 2. 构建 DAG 结构（按依赖顺序）
        let mut nodes: Vec<DagNode> = task_ids
            .iter()
            .map(|id| self.build_from_task(*id))
            .collect();

        // 3. 创建虚拟根节点
        let root = DagNode {
            id: format!("composite-{}", uuid::Uuid::new_v4()),
            node_type: DagNodeType::Sequential,
            name: "复合任务".to_string(),
            description: Some("包含多个子任务".to_string()),
            dependencies: vec![],
            dependents: task_ids.to_vec(),
            params: serde_json::json!({
                "task_ids": task_ids,
            }),
            execution_strategy: ExecutionStrategy::Sequential,
            ..Default::default()
        };

        nodes.push(root);

        Ok(root)
    }

    /// 添加引擎扫描节点
    pub async fn add_engine_scan(
        &self,
        engine_type: &str,
        base_dir: &PathBuf,
        task_id: i64,
    ) -> Result<i64> {
        let scan_task = TaskEntity {
            id: 0, // 临时 ID，实际插入时生成
            task_id: format!("engine-scan-{}", uuid::Uuid::new_v4()),
            name: format!("扫描 {} 引擎代码", engine_type),
            task_type: TaskType::EngineCodeInjection,
            priority: TaskPriority::P0,
            prompt_template: format!(
                "扫描以下目录的 {} 引擎代码：\\n
                base_directory: {}\\n
                请识别可复用和可注入的代码范围。",
                engine_type,
                base_dir
            ),
            context_variables: serde_json::json!({
                "engine_type": engine_type,
                "base_dir": base_dir.to_string_lossy(),
            }),
            description: Some("引擎代码扫描，为后续任务准备代码上下文"),
            estimated_effort_days: 0.5, // 4 小时
            ..Default::default()
        };

        // 插入到数据库
        let conn = self.db.lock().await;
        conn.execute(
            "INSERT INTO tasks (
                task_id, name, type, priority, prompt_template,
                context_variables, description, estimated_effort_days,
                status, engine_type, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?10)",
            params![
                &scan_task.task_id,
                &scan_task.name,
                &scan_task.task_type,
                &scan_task.priority,
                &scan_task.prompt_template,
                &scan_task.context_variables,
                &scan_task.description,
                &scan_task.estimated_effort_days,
                TaskStatus::Pending as i32,
                &scan_task.engine_type,
                chrono::Utc::now().timestamp(),
                chrono::Utc::now().timestamp(),
            ],
        )?;

        Ok(scan_task.id)
    }

    /// 解析依赖关系
    fn parse_dependencies(deps_json: &str) -> Result<HashSet<i64>> {
        if ps_json.trim().is_empty() {
            return Ok(HashSet::new());
        }

        let ids: Vec<i64> = serde_json::from_str(ps_json)
            .map_err(|e| anyhow::anyhow!("Invalid dependencies JSON: {}", e))?;

        Ok(ids.into_iter().collect())
    }

    /// 加载任务（从缓存或数据库）
    async fn load_task(&self, task_id: i64) -> Result<TaskEntity> {
        // 1. 检查缓存
        if let Some(task) = self.task_cache.get(&task_id) {
            return Ok(task);
        }

        // 2. 从数据库加载
        let conn = self.db.lock().await;
        let mut stmt = conn.prepare(
            "SELECT * FROM tasks WHERE id = ?1"
        )?;

        let task = stmt.query_row(params![task_id], |row| {
            Ok(TaskEntity {
                id: row.get(0)?,
                task_id: row.get(1)?,
                name: row.get(2)?,
                task_type: row.get(3)?,
                priority: row.get(4)?,
                prompt_template: row.get(5)?,
                context_variables: row.get(6)?,
                description: row.get(7)?,
                estimated_effort_days: row.get(8)?,
                status: row.get::<i32>(9)?,
                engine_type: row.get::<Option<String>>(10)?,
                engine_context_id: row.get::<Option<i64>>(11)?,
                dependencies: row.get::<String>(12)?,
                assigned_team_id: row.get::<Option<String>>(13)?,
                assigned_session_id: row.get::<Option<i64>>(14)?,
                result_json: row.get::<Option<String>>(15)?,
                error_message: row.get::<Option<String>>(16)?,
                created_at: row.get::<i64>(17)?,
                updated_at: row.get::<i64>(18)?,
            })
        })?;

        // 更新缓存
        self.task_cache.insert(task_id, task.clone());

        Ok(task)
    }

    /// 确定执行策略
    fn determine_strategy(task: &TaskEntity) -> ExecutionStrategy {
        // 根据优先级确定策略
        match task.priority {
            TaskPriority::P0 => ExecutionStrategy::Mechanical {
                max_retries: 3,
                retry_delay_secs: 5,
            },
            TaskPriority::P1 => ExecutionStrategy::Mechanical {
                max_retries: 2,
                retry_delay_secs: 10,
            },
            TaskPriority::P2 => ExecutionStrategy::Recommended {
                timeout_secs: 600,
                cancel_after_secs: 30,
            },
            TaskPriority::P3 => ExecutionStrategy::Confirmed {
                timeout_secs: 3600,
                prompt_user: true,
            },
        }
    }
}
```

---

## 4. 任务分配器实现

```rust
use crate::task::entity::TaskEntity;
use crate::task::assigner::{TaskAssigner, AssignmentResult};
use crate::agent::pool::AgentPool;
use crate::dag::builder::DagBuilder;

/// 任务分配器
pub struct TaskAssigner {
    agent_pool: Arc<AgentPool>,
    dag_builder: DagBuilder,
}

impl TaskAssigner {
    pub fn new(agent_pool: Arc<AgentPool>) -> Self {
        Self {
            agent_pool,
            dag_builder: DagBuilder::new(),
        }
    }

    /// 分配单个任务
    pub async fn assign_task(&self, task_id: i64) -> Result<AssignmentResult> {
        // 1. 加载任务
        let task = self.dag_builder.load_task(task_id).await?;

        // 2. 分析任务需求
        let requirements = Self::analyze_requirements(&task)?;

        // 3. 查找合适的 Team
        let best_team = self.agent_pool.find_best_team(&requirements).await?;

        // 4. 创建任务分配记录
        let assignment = AssignmentResult {
            task_id: task_id.clone(),
            task_name: task.name.clone(),
            team_id: best_team.id.clone(),
            agent_type: best_team.agent_type.clone(),
            assigned_at: chrono::Utc::now().timestamp(),
        };

        // 5. 更新任务状态
        sql::execute(
            "UPDATE tasks SET status = 'assigned', assigned_team_id = ?3, assigned_at = ?4
             WHERE id = ?1",
            params![
                &best_team.id,
                chrono::Utc::now().timestamp(),
                task_id,
            ],
        )?;

        Ok(assignment)
    }

    /// 分配 DAG（智能并行分配）
    pub async fn assign_dag(&self, dag_id: &str) -> Result<Vec<AssignmentResult>> {
        // 1. 加载 DAG
        let dag = self.dag_builder.build_from_dag_id(dag_id).await?;

        // 2. 拓扑排序（获取可执行任务列表）
        let executable_tasks = Self::topological_sort(&dag)?;

        // 3. 分析所有任务需求
        let task_requirements: Vec<_> = executable_tasks
            .iter()
            .map(|task| Self::analyze_requirements(task))
            .collect();

        // 4. 智能分配到 Teams
        let mut assignments = Vec::new();
        for task in executable_tasks {
            match self.agent_pool.find_best_team(&task_requirements).await {
                Ok(team) => {
                    let assignment = AssignmentResult {
                        task_id: task.id.clone(),
                        task_name: task.name.clone(),
                        team_id: team.id.clone(),
                        agent_type: team.agent_type.clone(),
                        assigned_at: chrono::Utc::now().timestamp(),
                    };
                    assignments.push(assignment);

                    // 更新任务状态
                    sql::execute(
                        "UPDATE tasks SET status = 'assigned', assigned_team_id = ?3, assigned_at = ?4
                         WHERE id = ?1",
                        params![
                            &team.id,
                            chrono::Utc::now().timestamp(),
                            task.id,
                        ],
                    )?;
                }
                Err(e) => {
                    tracing::error!("无法为任务 {} 分配 Team: {}", task.id, e);
                    return Err(e);
                }
            }
        }

        Ok(assignments)
    }

    /// 分析任务需求
    fn analyze_requirements(task: &TaskEntity) -> TaskRequirements {
        let mut requirements = TaskRequirements::default();

        // 从任务类型推断需求
        match task.task_type {
            TaskType::ModuleRefactoring => {
                requirements.capabilities.push(Capability::ModuleRefactoring);
            }
            TaskType::EngineCodeInjection => {
                requirements.capabilities.push(Capability::EngineCodeInjection);
                if let Some(engine_type) = &task.engine_type {
                    requirements.engine_type = Some(engine_type.clone());
                }
            }
            TaskType::PerformanceOptimization => {
                requirements.capabilities.push(Capability::PerformanceOptimization);
            }
            TaskType::CodeReview => {
                requirements.capabilities.push(Capability::CodeReview);
            }
            _ => {}
        }

        // 从优先级推断超时
        requirements.timeout_secs = match task.priority {
            TaskPriority::P0 => 300,   // 5 分钟
            TaskPriority::P1 => 600,   // 10 分钟
            TaskPriority::P2 => 1800,  // 30 分钟
            TaskPriority::P3 => 3600,  // 60 分钟
        };

        requirements
    }

    /// 拓扑排序（返回可执行任务列表）
    fn topological_sort(dag: &DagNode) -> Result<Vec<&TaskEntity>> {
        // 简单实现：使用 Kahn 算法
        let mut in_degree: HashMap<i64, usize> = HashMap::new();
        let mut all_tasks: Vec<&TaskEntity> = Vec::new();

        // 1. 收集所有任务节点
        fn collect_tasks(node: &DagNode) {
            if let DagNodeType::Task = &node.node_type {
                if let Ok(task) = self.dag_builder.load_task(node.task_id) {
                    all_tasks.push(task);
                }
            } else if let DagNodeType::EngineScan = &node.node_type {
                // 引擎扫描任务总是先执行
                if let Ok(task) = self.dag_builder.load_task(node.task_id) {
                    all_tasks.push(task);
                }
            } else if let DagNodeType::Parallel = &node.node_type {
                // 并行节点：收集所有子任务
                for dep_id in &node.dependencies {
                    if let Ok(task) = self.dag_builder.load_task(dep_id) {
                        all_tasks.push(task);
                    }
                }
                for dep_id in &node.dependents {
                    if let Ok(task) = self.dag_builder.load_task(dep_id) {
                        all_tasks.push(task);
                    }
                }
            }
        }

        // 2. 计算入度
        for task in &all_tasks {
            if let Some(deps_str) = &task.dependencies {
                if let Ok(ids) = Self::parse_dependencies(deps_str) {
                    in_degree.insert(task.id, ids.len());
                }
            }
        }

        // 3. 拓扑排序（入度 0 → N）
        let mut sorted_tasks = Vec::new();
        let mut visited = HashSet::new();

        while let Some(&task) = all_tasks.iter()
            .find(|t| in_degree.get(&t.id) == 0 && visited.insert(t.id))
        {
            sorted_tasks.push(task);
            visited.insert(task.id);

            // 减少依赖任务的入度
            if let Some(deps_str) = &task.dependencies {
                if let Ok(ids) = Self::parse_dependencies(deps_str) {
                    for dep_id in ids {
                        if let Some(dep) = self.task_cache.get(&dep_id) {
                            if let Some(count) = in_degree.get_mut(&dep_id) {
                                if *count > 0 {
                                    *count -= 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(sorted_tasks)
    }
}

/// 任务需求
#[derive(Debug, Clone)]
pub struct TaskRequirements {
    pub capabilities: Vec<String>,
    pub engine_type: Option<String>,
    pub timeout_secs: u64,
    pub max_concurrent_tasks: Option<usize>,
}

impl Default for TaskRequirements {
    fn default() -> Self {
        Self {
            capabilities: Vec::new(),
            engine_type: None,
            timeout_secs: 600,
            max_concurrent_tasks: None,
        }
}
```

---

## 5. Agent Pool 任务执行

```rust
use crate::task::executor::TaskExecutor;
use crate::agent::pool::AgentPool;
use crate::task::entity::TaskEntity;

/// Agent Pool 任务执行器
pub struct AgentTaskExecutor {
    agent_pool: Arc<AgentPool>,
    task_repo: Arc<TaskRepository>,
    dag_builder: DagBuilder,
    task_assigner: TaskAssigner,
    task_executor: TaskExecutor,
}

impl AgentTaskExecutor {
    pub fn new(
        agent_pool: Arc<AgentPool>,
        task_repo: Arc<TaskRepository>,
    dag_builder: DagBuilder,
        task_executor: TaskExecutor,
    ) -> Self {
        Self {
            agent_pool,
            task_repo,
            dag_builder,
            task_assigner: TaskAssigner {
                agent_pool: agent_pool.clone(),
                dag_builder: dag_builder,
            },
            task_executor,
        }
    }

    /// 执行单个任务
    pub async fn execute_task(&self, task: &TaskEntity) -> Result<TaskResult> {
        tracing::info!("开始执行任务: {} ({})", task.id, task.name);

        // 1. 查找分配的 Team
        let Some(team_id) = &task.assigned_team_id else {
            return Err(anyhow!("任务未分配"));
        };

        // 2. 查找或创建 Session
        let agent_session = self.agent_pool.acquire_session(team_id).await?;

        // 3. 准备 TaskContext
        let task_context = TaskContext {
            id: task.task_id.clone(),
            name: task.name.clone(),
            task_type: task.task_type.clone(),
            priority: task.priority,
            prompt_template: task.prompt_template.clone(),
            context_variables: task.context_variables.clone(),
            engine_context: None, // 稍后从 engine_contexts 表加载
            timeout_secs: None,  // 稍后从 ExecutionStrategy 读取
            dependencies: task.dependencies.clone(),
            metadata: serde_json::json!({}), // 添加元数据
        };

        // 4. 调用 Agent 执行
        let result = self.task_executor
            .execute(&mut *agent_session, task_context)
            .await;

        // 5. 保存结果
        let status = if result.is_ok() {
            "completed".to_string()
        } else {
            "failed".to_string()
        };

        let result_json = serde_json::to_string(&result)?;

        sql::execute(
            "UPDATE tasks
             SET status = ?2,
                 result_json = ?3,
                 error_message = ?4,
                 completed_at = ?5
             WHERE id = ?1",
            params![
                status,
                result_json,
                match &result {
                    Ok(_) => None,
                    Err(e) => Some(e.to_string()),
                },
                chrono::Utc::now().timestamp(),
                task.id,
            ],
        )?;

        // 6. 归还 Session
        self.agent_pool.release_session(agent_session).await?;

        tracing::info!("任务执行完成: {} ({}): {:?}", task.id, task.name, status);

        Ok(result)
    }

    /// 执行 DAG（并行执行）
    pub async fn execute_dag(&self, dag_id: &str) -> Result<DagResult> {
        tracing::info!("开始执行 DAG: {}", dag_id);

        // 1. 加载 DAG
        let dag = self.dag_builder.build_from_dag_id(dag_id).await?;

        // 2. 拓扑排序
        let tasks = self.task_assigner.topological_sort(&dag)?;

        // 3. 并行执行（有依赖限制）
        let mut results = Vec::new();
        let mut failed = Vec::new();

        for task in tasks {
            match self.execute_task(task).await {
                Ok(result) => {
                    results.push((task.clone(), result));
                }
                Err(e) => {
                    tracing::error!("任务失败: {} - {}", task.id, e);
                    failed.push((task.clone(), e));

                    // 依赖任务失败，停止执行
                    if !task.dependencies.is_empty() {
                        break;
                    }
                }
            }
        }

        // 4. 生成结果报告
        let success_count = results.len();
        let failed_count = failed.len();

        let result = DagResult {
            dag_id: dag_id.to_string(),
            total_tasks: tasks.len(),
            completed_tasks: success_count,
            failed_tasks: failed_count,
            results,
            failures: failed,
            status: if failed_count == 0 {
                "success".to_string()
            } else {
                "partial_failure".to_string()
            },
            executed_at: chrono::Utc::now().timestamp(),
        };

        Ok(result)
    }
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    task_id: String,
    task_name: String,
    agent_type: String,
    session_id: String,
    result: serde_json::Value,
    duration_secs: u64,
    tokens_used: u64,
}

/// DAG 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagResult {
    dag_id: String,
    total_tasks: usize,
    completed_tasks: usize,
    failed_tasks: usize,
    results: Vec<TaskResult>,
    failures: Vec<(TaskEntity, Error)>,
    status: String,
    executed_at: i64,
}
```

---

## 6. 使用示例：完整工作流

### 6.1 创建 CLI 架构修复任务（V-1）

```bash
# 步骤 1: 使用 CLI 工具创建任务
cis task create \
    --task-id "V-1" \
    --name "CLI 架构修复" \
    --type module_refactoring \
    --priority p0 \
    --prompt-template "审查以下 CLI handler 并重构为 Server API 调用..." \
    --context-vars '{"handlers_dir": "cis-node/src/cli/handlers"}' \
    --description "审查并重构所有 CLI handler，确保只调用 Server API" \
    --estimated-effort 5

# 步骤 2: 创建引擎扫描任务（自动）
cis task create-engine-scan \
    --engine-type "unreal5.7" \
    --task-id "V-scan-001" \
    --priority p0 \
    --base-dir "/path/to/unreal/project" \
    --dependencies "V-1"

# 步骤 3: 创建 DAG（V-1 依赖 V-scan-001）
cis dag create \
    --name "CLI 架构修复 DAG" \
    --tasks "V-1,V-scan-001" \
    --description "CLI 架构修复，包含引擎代码扫描"

# 步骤 4: 分配并执行 DAG
cis dag execute "CLI 架构修复 DAG"
```

### 6.2 创建 scheduler 拆分任务（V-2）

```bash
# 主任务
cis task create \
    --task-id "V-2" \
    --name "scheduler 拆分" \
    --type module_refactoring \
    --priority p1 \
    --prompt-template "拆分 scheduler/mod.rs (3439 行) 为多个子模块..." \
    --context-vars '{"module_dir": "cis-core/src/scheduler"}' \
    --estimated-effort 15

# 子任务 1: 设计
cis task create \
    --task-id "V-2-design" \
    --name "scheduler 拆分设计" \
    --type module_refactoring \
    --priority p1 \
    --prompt-template "设计 scheduler 模块拆分方案..." \
    --dependencies "V-2" \
    --estimated-effort 3

# 子任务 2: 实现
cis task create \
    --task-id "V-2-impl" \
    --name "scheduler 拆分实现" \
    --type module_refactoring \
    --priority p1 \
    --dependencies "V-2-design" \
    --prompt-template "实现 scheduler 模块拆分..." \
    --estimated-effort 12

# 创建 DAG
cis dag create \
    --name "scheduler 拆分 DAG" \
    --tasks "V-2-design,V-2-impl" \
    --description "设计并实现 scheduler 拆分"

# 执行 DAG
cis dag execute "scheduler 拆分 DAG"
```

---

## 7. CLI 工具完整实现

### 7.1 任务创建命令

```rust
// cis-node/src/cli/commands/task.rs

use clap::{Parser, Subcommand};
use crate::task::cli::{TaskCreateArgs, TaskListArgs};

#[derive(Subcommand)]
pub struct TaskCommand;

#[clap(
    name = "task",
    about = "任务管理（基于 SQLite 存储）",
    long_about = "创建、查询、分配、执行任务"
)]
pub enum TaskSubCommand {
    #[clap(subcommand)]
    Create(TaskCreateArgs),

    #[clap(subcommand)]
    List(TaskListArgs),

    #[clap(subcommand)]
    Query(TaskQueryArgs),

    #[clap(subcommand)]
    Assign(TaskAssignArgs),

    #[clap(subcommand)]
    Execute(TaskExecuteArgs),

    #[clap(subcommand)]
    Dag(TaskDagArgs),

    #[clap(subcommand)]
    Archive(TaskArchiveArgs),
}
```

### 7.2 任务创建命令实现

```rust
#[derive(Parser)]
pub struct TaskCreateArgs {
    #[clap(short, long)]
    task_id: String,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    #[arg(value_enum)]
    #[arg(value_enum)]
    task_type: TaskType,

    #[clap(short, long)]
    #[clap(short, long)]
    priority: TaskPriority,

    #[clap(short, long)]
    prompt_template: String,

    #[clap(short, long)]
    #[clap(short, long)]
    context_vars: String,

    #[clap(short, long)]
    description: Option<String>,

    #[clap(short, long)]
    #[clap(short, long)]
    estimated_effort: f64,

    #[clap(short = "dependencies")]
    dependencies: Vec<String>,

    #[clap(short, long)]
    #[clap(short, long)]
    #[clap(short = "engine_type")]
    engine_type: Option<String>,

    #[clap(short = "base_dir")]
    base_dir: String,
}

impl TaskCreateArgs {
    /// 执行：创建任务到数据库
    pub async fn execute(&self, pool: &AgentPool) -> Result<i64> {
        // 1. 构建任务实体
        let task = TaskEntity {
            id: 0, // 临时 ID
            task_id: self.task_id.clone(),
            name: self.name.clone(),
            task_type: self.task_type,
            priority: self.priority,
            prompt_template: self.prompt_template.clone(),
            context_variables: self.context_vars.clone(),
            description: self.description,
            estimated_effort_days: self.estimated_effort,
            dependencies: self.dependencies.join(","), // JSON 数组转字符串
            engine_type: self.engine_type.clone(),
            status: TaskStatus::Pending as i32,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        // 2. 插入到数据库（通过 TaskRepository）
        let task_id = pool.task_repo.create(&task).await?;

        tracing::info!("任务已创建: {} ({})", task_id, self.name);

        Ok(task_id)
    }
}
```

---

## 8. 完整执行示例

### 8.1 示例：CLI 架构修复工作流

```bash
# ============================================
# CIS v1.1.6 并行开发工作流示例
# ============================================

# 1. 创建主任务
cis task create \
    --task-id "main-V-1" \
    --name "v1.1.6 重构" \
    --type composite \
    --dependencies "[]" \
    --estimated-effort 40

# 2. 创建子任务
cis task create --task-id "V-1" --name "CLI 架构修复" --type module_refactoring --priority p0 \
    --prompt-template "审查以下 CLI handler..." \
    --context-vars '{"handlers_dir": "cis-node/src/cli/handlers"}' \
    --estimated-effort 5

cis task create --task-id "V-scan" --name "引擎代码扫描" --type engine_code_injection --priority p0 \
    --engine-type "unreal5.7" --base-dir "/path/to/unreal" \
    --dependencies "V-1"

# 3. 创建 DAG
cis dag create \
    --name "v1.1.6 重构 DAG" \
    --tasks "V-1,V-scan" \
    --description "v1.1.6 重构，包含 CLI 架构修复和引擎代码扫描"

# 4. 执行 DAG
cis dag execute "v1.1.6 重构 DAG"

# ============================================
# 执行流程：
# 1. DagBuilder 解析依赖，生成执行顺序
# 2. TaskAssigner 分析任务需求，查找合适 Team
# 3. Agent Pool 获取或创建 Session（复用）
# 4. Agent 执行任务（可能包含引擎代码注入）
# 5. 结果自动保存到 SQLite
# 6. 实时监控和报告
# ============================================
```

---

## 9. 性能优化和监控

### 9.1 并发控制

```rust
/// 并发配置
pub struct ConcurrencyConfig {
    /// 每个类型最大并发任务数
    pub max_concurrent_per_type: HashMap<TaskType, usize>,

    /// 全局最大并发数
    pub global_max_concurrent: usize,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        let mut max_per_type = HashMap::new();
        max_per_type.insert(TaskType::ModuleRefactoring, 3);
        max_per_type.insert(TaskType::EngineCodeInjection, 2);
        max_per_type.insert(TaskType::PerformanceOptimization, 1);
        max_per_type.insert(TaskType::CodeReview, 5);

        Self {
            max_concurrent_per_type,
            global_max_concurrent: 10, // 全局限制
        }
    }
}

/// 并发控制器
pub struct ConcurrencyController {
    config: ConcurrencyConfig,
    running_tasks: Arc<Mutex<HashSet<i64>>>,
}

impl ConcurrencyController {
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            config,
            running_tasks: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 检查是否可以启动任务
    pub async fn can_start(&self, task: &TaskEntity) -> bool {
        let max = self.config.max_concurrent_per_type
            .get(&task.task_type)
            .copied()
            .unwrap_or(&self.config.global_max_concurrent);

        let running = self.running_tasks.lock().await;
        let running_count = running.len();

        running_count < max
    }

    /// 标记任务开始
    pub async fn mark_started(&self, task_id: i64) {
        let mut running = self.running_tasks.lock().await;
        running.insert(task_id);
    }

    /// 标记任务完成
    pub async fn mark_completed(&self, task_id: i64) {
        let mut running = self.running_tasks.lock().await;
        running.remove(&task_id);
    }
}
```

### 9.2 性能监控

```rust
use prometheus::{Counter, Histogram};

/// 任务执行指标
pub struct TaskMetrics {
    tasks_created_total: Counter,
    tasks_completed_total: Counter,
    tasks_failed_total: Counter,
    execution_duration_seconds: Histogram,
    tokens_used_total: Counter,
}

lazy_static! {
    static ref METRICS: TaskMetrics = TaskMetrics {
        tasks_created_total: register_counter!(
            "cis_tasks_created_total",
            "Total tasks created"
        ),
        tasks_completed_total: register_counter!(
            "cis_tasks_completed_total",
            "Total tasks completed"
        ),
        tasks_failed_total: register_counter!(
            "cis_tasks_failed_total",
            "Total tasks failed"
        ),
        execution_duration_seconds: register_histogram!(
            "cis_task_execution_duration_seconds",
            "Task execution duration in seconds"
        ),
        tokens_used_total: register_counter!(
            "cis_tokens_used_total",
            "Total tokens consumed by agents"
        ),
    };
}

impl TaskMetrics {
    /// 记录任务完成
    pub fn record_completion(&self, duration_secs: f64) {
        METRICS.tasks_completed_total.inc();
        METRICS.execution_duration_seconds.observe(duration_secs);
    }

    /// 记录任务失败
    pub fn record_failure(&self) {
        METRICS.tasks_failed_total.inc();
    }
}

/// Prometheus 端点
pub fn metrics_endpoint() -> String {
    "/metrics".to_string()
}
```

---

## 10. 总结

### 10.1 核心特性

1. **SQLite 任务存储**
   - 高性能查询
   - 关系型数据
   - 事务支持
   - 批量操作优化

2. **Agent Pool 架构**
   - 可替换 Agent 接口
   - Session 复用机制
   - 多 Runtime 支持
   - 引擎代码注入

3. **DAG 工作流**
   - 自动依赖解析
   - 拓扑排序
   - 并发执行
   - 智能分配

4. **性能优化**
   - 并发控制
   - 实时监控
   - 性能指标

### 10.2 预期性能

| 指标 | 当前 | 目标 | 改进 |
|------|------|------|------|
| **任务查询速度** | ~50ms | ~5ms | **10x** |
| **并发安全** | WAL 锁 | WAL + 锁 | **10x** |
| **Agent 复用率** | ~30% | >80% | **2.7x** |
| **任务吞吐量** | ~5 任务/分钟 | ~50 任务/分钟 | **10x** |

### 10.3 下一步

1. **实现任务存储** - SQLite Schema + Repository
2. **实现 DAG Builder** - 依赖解析和拓扑排序
3. **实现 Task Assigner** - 智能分配算法
4. **集成 Agent Pool** - 与任务系统完全集成
5. **性能测试** - 并发压力测试
6. **CLI 工具** - 完整命令行界面

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 设计完成
