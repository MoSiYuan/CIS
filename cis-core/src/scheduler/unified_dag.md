# Unified DAG 设计文档

> **版本**: v1.0
> **日期**: 2026-02-12
> **作者**: Team K (CIS v1.1.6)
> **状态**: 设计阶段

---

## 1. 背景和问题分析

### 1.1 当前状态

CIS scheduler 模块中存在两套并行的 DAG 定义：

#### 定义 A: `scheduler::dag_executor::DagDefinition`

**位置**: `cis-core/src/scheduler/dag_executor.rs`

```rust
pub struct DagDefinition {
    pub id: String,
    pub name: String,
    pub nodes: Vec<DagNode>,
}

pub struct DagNode {
    pub id: String,
    pub skill_name: String,
    pub method: String,
    pub params: Vec<u8>,
    pub dependencies: Vec<String>,
}
```

**特点**:
- 简单的 DAG 定义，用于 Skill 执行
- 包含 skill_name, method 等执行细节
- 缺少四级决策支持
- 缺少 Agent 配置支持

#### 定义 B: `scheduler::TaskDag`

**位置**: `cis-core/src/scheduler/mod.rs`

```rust
pub struct TaskDag {
    nodes: HashMap<String, DagNode>,
    root_nodes: Vec<String>,
}

pub struct DagNode {
    pub task_id: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub status: DagNodeStatus,
    pub level: TaskLevel,
    pub rollback: Option<Vec<String>>,

    // Agent Teams 字段
    pub agent_runtime: Option<RuntimeType>,
    pub reuse_agent: Option<String>,
    pub keep_agent: bool,
    pub agent_config: Option<AgentConfig>,
}
```

**特点**:
- 功能完整的 DAG 定义，支持调度和执行
- 支持四级决策（TaskLevel）
- 支持 Agent Teams 配置
- 包含运行时状态（status）
- 包含依赖管理（dependents）

#### 定义 C: `skill::manifest::DagDefinition`

**位置**: `cis-core/src/skill/manifest.rs` (不存在，但 skill_executor.rs 中引用)

实际上 `skill_executor.rs` 使用的是 `DagTaskDefinition`，位于 skill manifest。

### 1.2 问题分析

| 问题 | 影响 | 严重性 |
|------|------|--------|
| **代码重复** | 三套定义，功能重叠 | 🟠 高 |
| **类型转换开销** | 需要在不同定义间转换 | 🟡 中 |
| **维护困难** | 修改需要同步多处 | 🟠 高 |
| **功能不一致** | 某些定义缺少字段 | 🟡 中 |
| **序列化混乱** | TOML/JSON 解析不统一 | 🟠 高 |
| **测试复杂** | 需要测试多套定义 | 🟡 中 |

### 1.3 统一目标

1. **单一定义**: 创建 `UnifiedDag` 作为唯一的 DAG 定义
2. **向后兼容**: 现有 DAG 文件无需修改即可加载
3. **零拷贝转换**: 旧定义 → 新定义的高效转换
4. **功能完整**: 支持所有现有功能（四级决策、Agent Teams、依赖管理）
5. **类型安全**: 强类型，编译期检查
6. **可扩展**: 易于添加新字段

---

## 2. UnifiedDag 设计

### 2.1 核心结构

```rust
/// 统一 DAG 定义
///
/// # 特性
/// - 支持从 TaskDag 和 DagDefinition 转换
/// - 支持序列化/反序列化（TOML, JSON, YAML）
/// - 支持四级决策机制
/// - 支持 Agent Teams 配置
/// - 支持依赖管理和验证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedDag {
    /// DAG 元数据
    pub metadata: DagMetadata,

    /// 任务列表（使用 Vec 保持顺序）
    #[serde(rename = "tasks")]
    pub tasks: Vec<UnifiedTask>,

    /// 执行策略
    #[serde(default)]
    pub execution_policy: ExecutionPolicy,
}

/// DAG 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagMetadata {
    /// DAG 唯一标识符
    pub id: String,

    /// DAG 名称
    pub name: String,

    /// DAG 描述
    #[serde(default)]
    pub description: Option<String>,

    /// DAG 版本（用于版本管理）
    #[serde(default = "default_version")]
    pub version: String,

    /// 创建时间
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,

    /// 作者/创建者
    #[serde(default)]
    pub author: Option<String>,

    /// 标签（用于分类和搜索）
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// 统一任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTask {
    /// 任务 ID（唯一）
    pub id: String,

    /// 任务名称
    #[serde(default)]
    pub name: Option<String>,

    /// 任务描述
    #[serde(default)]
    pub description: Option<String>,

    /// Skill 名称或 ID
    pub skill: String,

    /// Skill 方法（可选，默认 "execute"）
    #[serde(default = "default_skill_method")]
    pub method: String,

    /// 任务参数（JSON 对象）
    #[serde(default)]
    pub params: Map<String, Value>,

    /// 依赖任务 ID 列表
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// 四级决策级别
    #[serde(flatten)]
    pub level: TaskLevel,

    /// Agent Runtime 配置
    #[serde(default)]
    pub agent_config: Option<AgentTaskConfig>,

    /// 回滚命令
    #[serde(default)]
    pub rollback: Option<Vec<String>>,

    /// 超时时间（秒）
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// 重试次数（仅 Mechanical 级别）
    #[serde(default)]
    pub retry: Option<u32>,

    /// 任务条件（表达式，可选）
    #[serde(default)]
    pub condition: Option<String>,

    /// 是否幂等（用于断点续传）
    #[serde(default)]
    pub idempotent: bool,

    /// 输出映射（用于下游任务引用）
    #[serde(default)]
    pub outputs: Option<Map<String, String>>,
}

fn default_skill_method() -> String {
    "execute".to_string()
}

/// Agent 任务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskConfig {
    /// Agent Runtime 类型
    #[serde(default)]
    pub runtime: RuntimeType,

    /// 复用已有 Agent ID
    #[serde(default)]
    pub reuse_agent_id: Option<String>,

    /// 是否保持 Agent（执行后不销毁）
    #[serde(default)]
    pub keep_agent: bool,

    /// 模型配置
    #[serde(default)]
    pub model: Option<String>,

    /// Agent 系统提示词
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// 工作目录
    #[serde(default)]
    pub work_dir: Option<String>,
}

/// 执行策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    /// 所有任务必须成功
    AllSuccess,

    /// 任一任务成功即可
    FirstSuccess,

    /// 允许技术债务（ignorable 失败）
    AllowDebt,

    /// 继续执行直到阻塞失败
    ContinueUntilBlocking,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self::AllSuccess
    }
}
```

### 2.2 设计决策

#### 2.2.1 为什么使用 `Vec<UnifiedTask>` 而不是 `HashMap`？

**选择 Vec 的原因**:
1. **保持顺序**: 任务在文件中的顺序通常有意义
2. **序列化友好**: TOML/JSON/YAML 都支持数组
3. **可验证**: 容易检查重复 ID
4. **索引快速**: Vec 访问是 O(1)

**运行时优化**:
```rust
impl UnifiedDag {
    /// 运行时构建索引（lazy 初始化）
    pub fn task_index(&self) -> HashMap<&str, &UnifiedTask> {
        self.tasks.iter()
            .map(|t| (t.id.as_str(), t))
            .collect()
    }

    /// 快速查找
    pub fn get_task(&self, id: &str) -> Option<&UnifiedTask> {
        self.tasks.iter()
            .find(|t| t.id == id)
    }
}
```

#### 2.2.2 如何处理运行时状态？

**设计选择**: 分离定义和状态

```rust
/// 运行时状态（不序列化）
#[derive(Debug, Clone)]
pub struct UnifiedDagRun {
    /// DAG 定义
    pub dag: UnifiedDag,

    /// 运行时状态
    pub task_states: HashMap<String, TaskState>,

    /// 运行 ID
    pub run_id: String,

    /// 开始时间
    pub started_at: DateTime<Utc>,

    /// 运行状态
    pub status: DagRunStatus,
}

/// 任务运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// 任务状态
    pub status: TaskStatus,

    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,

    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,

    /// 输出结果
    pub output: Option<String>,

    /// 错误信息
    pub error: Option<String>,

    /// 重试次数
    pub retry_count: u32,

    /// 执行日志
    pub logs: Vec<String>,
}
```

#### 2.2.3 如何兼容现有字段？

**字段映射表**:

| 旧定义字段 | UnifiedDag 字段 | 转换逻辑 |
|-----------|----------------|---------|
| `DagNode.task_id` | `UnifiedTask.id` | 直接映射 |
| `DagNode.skill_name` | `UnifiedTask.skill` | 直接映射 |
| `DagNode.method` | `UnifiedTask.method` | 默认 "execute" |
| `DagNode.params` | `UnifiedTask.params` | Vec<u8> → Map<String, Value> |
| `DagNode.dependencies` | `UnifiedTask.dependencies` | 直接映射 |
| `DagNode.level` | `UnifiedTask.level` | 直接映射 |
| `DagNode.agent_runtime` | `UnifiedTask.agent_config.runtime` | 嵌套到 agent_config |
| `DagNode.reuse_agent` | `UnifiedTask.agent_config.reuse_agent_id` | 嵌套到 agent_config |
| `DagNode.keep_agent` | `UnifiedTask.agent_config.keep_agent` | 嵌套到 agent_config |
| `DagNode.agent_config` | `UnifiedTask.agent_config` | 扁平化字段 |
| `DagNode.rollback` | `UnifiedTask.rollback` | 直接映射 |

---

## 3. 转换器设计

### 3.1 TaskDag → UnifiedDag

```rust
impl From<TaskDag> for UnifiedDag {
    fn from(task_dag: TaskDag) -> Self {
        let tasks = task_dag.nodes.values()
            .map(|node| UnifiedTask {
                id: node.task_id.clone(),
                name: None,
                description: None,
                skill: node.skill_id.clone().unwrap_or_default(),
                method: "execute".to_string(),
                params: Map::new(),
                dependencies: node.dependencies.clone(),
                level: node.level.clone(),
                agent_config: node.agent_config.clone().map(|ac| AgentTaskConfig {
                    runtime: node.agent_runtime.unwrap_or(RuntimeType::Default),
                    reuse_agent_id: node.reuse_agent.clone(),
                    keep_agent: node.keep_agent,
                    model: ac.model,
                    system_prompt: None,
                    work_dir: None,
                }),
                rollback: node.rollback.clone(),
                timeout_secs: None,
                retry: None,
                condition: None,
                idempotent: false,
                outputs: None,
            })
            .collect();

        Self {
            metadata: DagMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Migrated from TaskDag".to_string(),
                description: None,
                version: "1.0.0".to_string(),
                created_at: Some(Utc::now()),
                author: None,
                tags: vec!["migrated".to_string()],
            },
            tasks,
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }
}
```

### 3.2 DagDefinition → UnifiedDag

```rust
impl From<DagDefinition> for UnifiedDag {
    fn from(def: DagDefinition) -> Self {
        let tasks = def.nodes.into_iter()
            .map(|node| UnifiedTask {
                id: node.id.clone(),
                name: Some(node.id.clone()),
                description: None,
                skill: node.skill_name,
                method: node.method,
                params: {
                    // 尝试反序列化 params
                    if let Ok(map) = serde_json::from_slice::<Map<String, Value>>(&node.params) {
                        map
                    } else {
                        let mut map = Map::new();
                        map.insert("raw".to_string(), Value::String(
                            base64::encode(&node.params)
                        ));
                        map
                    }
                },
                dependencies: node.dependencies,
                level: TaskLevel::Mechanical { retry: 3 },
                agent_config: None,
                rollback: None,
                timeout_secs: None,
                retry: None,
                condition: None,
                idempotent: false,
                outputs: None,
            })
            .collect();

        Self {
            metadata: DagMetadata {
                id: def.id,
                name: def.name,
                description: None,
                version: "1.0.0".to_string(),
                created_at: Some(Utc::now()),
                author: None,
                tags: vec!["migrated".to_string()],
            },
            tasks,
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }
}
```

### 3.3 反向转换

```rust
impl TryFrom<UnifiedDag> for TaskDag {
    type Error = ConversionError;

    fn try_from(unified: UnifiedDag) -> Result<Self, Self::Error> {
        let mut dag = TaskDag::new();

        for task in unified.tasks {
            dag.add_node_with_level(
                task.id.clone(),
                task.dependencies.clone(),
                task.level.clone(),
            )?;

            // 更新节点配置
            if let Some(node) = dag.get_node_mut(&task.id) {
                node.skill_id = Some(task.skill);
                if let Some(agent_config) = task.agent_config {
                    node.agent_runtime = Some(agent_config.runtime);
                    node.reuse_agent = agent_config.reuse_agent_id;
                    node.keep_agent = agent_config.keep_agent;
                }
                node.rollback = task.rollback;
            }
        }

        Ok(dag)
    }
}
```

---

## 4. DAG 文件格式

### 4.1 TOML 格式（推荐）

```toml
[metadata]
id = "code-review-and-deploy"
name = "Code Review and Deploy"
version = "1.0.0"
description = "自动化代码审查和部署流程"
author = "CIS Team"
tags = ["ci-cd", "code-review", "deployment"]

[policy]
type = "all_success"  # all_success | first_success | allow_debt | continue_until_blocking

[[tasks]]
id = "get-changes"
name = "获取代码变更"
skill = "git-diff"
method = "execute"

[tasks.level]
type = "mechanical"
retry = 3

[tasks.agent_config]
runtime = "claude"
model = "claude-3-sonnet"
keep_agent = false

[[tasks]]
id = "ai-review"
name = "AI 代码审查"
skill = "ai-code-review"
dependencies = ["get-changes"]

[tasks.level]
type = "confirmed"

[tasks.agent_config]
runtime = "claude"
model = "claude-3-opus"
system_prompt = "你是代码审查专家，请严格审查代码质量..."

[tasks.timeout_secs]
timeout = 600

[[tasks]]
id = "run-tests"
name = "运行测试"
skill = "cargo-test"
dependencies = ["ai-review"]

[tasks.level]
type = "mechanical"
retry = 2

[tasks.idempotent]
idempotent = true

[[tasks]]
id = "deploy"
name = "部署到生产"
skill = "deploy"
dependencies = ["run-tests"]

[tasks.level]
type = "recommended"
timeout_secs = 300
default_action = "skip"  # execute | skip | abort

[tasks.rollback]
commands = ["rollback-deployment", "notify-team"]
```

### 4.2 JSON 格式

```json
{
  "metadata": {
    "id": "code-review-and-deploy",
    "name": "Code Review and Deploy",
    "version": "1.0.0",
    "tags": ["ci-cd"]
  },
  "execution_policy": "all_success",
  "tasks": [
    {
      "id": "task-1",
      "skill": "git-diff",
      "dependencies": [],
      "level": {
        "Mechanical": {
          "retry": 3
        }
      }
    },
    {
      "id": "task-2",
      "skill": "ai-review",
      "dependencies": ["task-1"],
      "level": {
        "Confirmed": null
      }
    }
  ]
}
```

### 4.3 YAML 格式（可选）

```yaml
metadata:
  id: code-review-and-deploy
  name: Code Review and Deploy
  version: 1.0.0

execution_policy: all_success

tasks:
  - id: get-changes
    skill: git-diff
    dependencies: []
    level:
      type: mechanical
      retry: 3

  - id: ai-review
    skill: ai-code-review
    dependencies:
      - get-changes
    level:
      type: confirmed
```

---

## 5. 验证和约束

### 5.1 结构验证

```rust
impl UnifiedDag {
    /// 验证 DAG 结构
    pub fn validate(&self) -> Result<(), DagValidationError> {
        // 1. 检查任务 ID 唯一性
        let mut ids = HashSet::new();
        for task in &self.tasks {
            if !ids.insert(&task.id) {
                return Err(DagValidationError::DuplicateTaskId(task.id.clone()));
            }
        }

        // 2. 检查依赖存在性
        for task in &self.tasks {
            for dep_id in &task.dependencies {
                if !ids.contains(dep_id) {
                    return Err(DagValidationError::DependencyNotFound {
                        task: task.id.clone(),
                        dependency: dep_id.clone(),
                    });
                }
            }
        }

        // 3. 检查循环依赖
        if self.has_cycle()? {
            return Err(DagValidationError::CycleDetected);
        }

        // 4. 检查根节点（至少一个无依赖的任务）
        let has_root = self.tasks.iter()
            .any(|t| t.dependencies.is_empty());
        if !has_root {
            return Err(DagValidationError::NoRootTask);
        }

        Ok(())
    }

    /// 检测循环依赖（DFS）
    fn has_cycle(&self) -> Result<bool, DagValidationError> {
        enum VisitState {
            Unvisited,
            Visiting,
            Visited,
        }

        let mut state = HashMap::new();
        let mut tasks = self.task_index();

        fn dfs(
            task_id: &str,
            tasks: &HashMap<&str, &UnifiedTask>,
            state: &mut HashMap<String, VisitState>,
        ) -> Result<bool, DagValidationError> {
            let visit_state = state.entry(task_id.to_string())
                .or_insert(VisitState::Unvisited);

            match visit_state {
                VisitState::Visited => Ok(false),
                VisitState::Visiting => Ok(true), // 发现环
                VisitState::Unvisited => {
                    *state.get_mut(task_id).unwrap() = VisitState::Visiting;

                    let task = tasks.get(task_id)
                        .ok_or_else(|| DagValidationError::TaskNotFound(task_id.to_string()))?;

                    for dep_id in &task.dependencies {
                        if dfs(dep_id, tasks, state)? {
                            return Ok(true);
                        }
                    }

                    *state.get_mut(task_id).unwrap() = VisitState::Visited;
                    Ok(false)
                }
            }
        }

        for task_id in tasks.keys() {
            if dfs(task_id, &tasks, &mut state)? {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
```

### 5.2 拓扑排序

```rust
impl UnifiedDag {
    /// 获取拓扑排序的任务列表
    pub fn topological_order(&self) -> Result<Vec<String>, DagValidationError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        let mut tasks = self.task_index();

        // 初始化
        for task in &self.tasks {
            in_degree.insert(task.id.clone(), task.dependencies.len());
            adj.insert(task.id.clone(), Vec::new());
        }

        // 构建邻接表
        for task in &self.tasks {
            for dep_id in &task.dependencies {
                adj.entry(dep_id.clone())
                    .or_insert_with(Vec::new)
                    .push(task.id.clone());
            }
        }

        // Kahn 算法
        let mut queue: VecDeque<String> = in_degree.iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(task_id) = queue.pop_front() {
            result.push(task_id.clone());

            if let Some(neighbors) = adj.get(&task_id) {
                for neighbor in neighbors {
                    let degree = in_degree.get_mut(neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        if result.len() != self.tasks.len() {
            return Err(DagValidationError::CycleDetected);
        }

        Ok(result)
    }
}
```

---

## 6. 向后兼容性策略

### 6.1 版本管理

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagMetadata {
    pub version: String,
    // ...
}

impl DagMetadata {
    /// 检查版本兼容性
    pub fn is_compatible(&self) -> bool {
        let version = semver::Version::parse(&self.version)
            .unwrap_or(semver::Version::new(1, 0, 0));

        // 支持 >= 1.0.0 且 < 2.0.0
        version >= semver::Version::new(1, 0, 0)
            && version < semver::Version::new(2, 0, 0)
    }
}
```

### 6.2 自动迁移

```rust
impl UnifiedDag {
    /// 从旧格式文件加载（自动检测格式）
    pub async fn from_file(path: &Path) -> Result<Self, LoadError> {
        let content = tokio::fs::read_to_string(path).await?;

        // 检测文件格式
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        // 尝试解析为 UnifiedDag
        match ext {
            "toml" => {
                if let Ok(dag) = toml::from_str::<UnifiedDag>(&content) {
                    return Ok(dag);
                }
                // 回退到旧格式
                Self::migrate_from_legacy_toml(&content)
            }
            "json" => {
                if let Ok(dag) = serde_json::from_str::<UnifiedDag>(&content) {
                    return Ok(dag);
                }
                Self::migrate_from_legacy_json(&content)
            }
            _ => Err(LoadError::UnsupportedFormat(ext.to_string())),
        }
    }

    fn migrate_from_legacy_toml(content: &str) -> Result<Self, LoadError> {
        // 尝试解析为 TaskDag 格式
        if let Ok(task_dag) = toml::from_str::<TaskDag>(content) {
            return Ok(UnifiedDag::from(task_dag));
        }

        // 尝试解析为 DagDefinition 格式
        if let Ok(dag_def) = toml::from_str::<DagDefinition>(content) {
            return Ok(UnifiedDag::from(dag_def));
        }

        Err(LoadError::CannotMigrate)
    }
}
```

---

## 7. 性能优化

### 7.1 零拷贝转换

```rust
/// 零拷贝视图（当需要查看但不转换时）
pub struct TaskDagView<'a> {
    dag: &'a TaskDag,
}

impl<'a> TaskDagView<'a> {
    pub fn as_unified(&self) -> UnifiedDag {
        // 使用引用而非克隆
        // ...
    }
}
```

### 7.2 懒加载

```rust
pub struct LazyUnifiedDag {
    raw: String,
    cached: Option<UnifiedDag>,
}

impl LazyUnifiedDag {
    pub fn get(&mut self) -> &UnifiedDag {
        self.cached.get_or_insert_with(|| {
            serde_json::from_str(&self.raw).unwrap()
        })
    }
}
```

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_unique_ids() {
        let dag = UnifiedDag {
            metadata: DagMetadata {
                id: "test".to_string(),
                name: "Test".to_string(),
                version: "1.0.0".to_string(),
                created_at: None,
                author: None,
                tags: vec![],
                description: None,
            },
            tasks: vec![
                UnifiedTask {
                    id: "task-1".to_string(),
                    skill: "test".to_string(),
                    ..Default::default()
                },
                UnifiedTask {
                    id: "task-1".to_string(), // 重复 ID
                    skill: "test".to_string(),
                    ..Default::default()
                },
            ],
            execution_policy: ExecutionPolicy::AllSuccess,
        };

        assert!(matches!(
            dag.validate(),
            Err(DagValidationError::DuplicateTaskId(_))
        ));
    }

    #[test]
    fn test_topological_order() {
        // 测试拓扑排序
    }

    #[test]
    fn test_cycle_detection() {
        // 测试环检测
    }

    #[test]
    fn test_from_task_dag() {
        // 测试从 TaskDag 转换
    }

    #[test]
    fn test_from_dag_definition() {
        // 测试从 DagDefinition 转换
    }
}
```

### 8.2 集成测试

```rust
#[tokio::test]
async fn test_end_to_end_execution() {
    // 1. 加载 DAG 文件
    let dag = UnifiedDag::from_file(Path::new("test-dag.toml")).awaitunwrap();

    // 2. 验证
    dag.validate().unwrap();

    // 3. 执行
    let executor = UnifiedDagExecutor::new();
    let result = executor.execute(&dag).await;

    // 4. 验证结果
    assert!(result.is_ok());
}
```

### 8.3 性能测试

```rust
#[bench]
fn bench_conversion(b: &mut Bencher) {
    let task_dag = create_large_task_dag(1000);
    b.iter(|| {
        let unified = UnifiedDag::from(task_dag.clone());
        assert!(validate(&unified).is_ok());
    });
}
```

---

## 9. 迁移计划

### Phase 1: 实现核心结构（1 天）

- [ ] 创建 `unified_dag.rs` 文件
- [ ] 实现 `UnifiedDag`, `UnifiedTask`, `DagMetadata` 结构
- [ ] 实现基本验证逻辑

### Phase 2: 实现转换器（2 天）

- [ ] 实现 `TaskDag → UnifiedDag` 转换器
- [ ] 实现 `DagDefinition → UnifiedDag` 转换器
- [ ] 实现反向转换器
- [ ] 添加单元测试

### Phase 3: 更新执行器（2 天）

- [ ] 更新 `multi_agent_executor.rs`
- [ ] 更新 `skill_executor.rs`
- [ ] 更新 `dag_executor.rs`
- [ ] 更新 `mod.rs` 导出

### Phase 4: 测试和文档（1 天）

- [ ] 完整集成测试
- [ ] 性能基准测试
- [ ] 更新用户文档
- [ ] 更新 API 文档

### Phase 5: 发布和监控（持续）

- [ ] 在 main 分支发布
- [ ] 监控性能和错误
- [ ] 收集用户反馈
- [ ] 迭代改进

---

## 10. 风险和缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 转换器 bug | 高 | 中 | 完善测试，添加回退机制 |
| 性能下降 | 中 | 低 | 零拷贝设计，性能基准测试 |
| 向后兼容性破坏 | 高 | 中 | 保留旧代码路径，自动迁移 |
| 学习曲线 | 中 | 高 | 详细文档，示例代码 |

---

## 11. 总结

**核心改进**:
1. ✅ 统一三套 DAG 定义为一个
2. ✅ 支持所有现有功能
3. ✅ 向后兼容旧格式
4. ✅ 零拷贝转换
5. ✅ 完善的验证和约束
6. ✅ 清晰的迁移路径

**预期收益**:
- 代码减少 ~500 行
- 维护成本降低 40%
- 类型安全提升
- 用户体验改进

**下一步**:
- 实现 `converters.rs`
- 更新执行器
- 完善测试

---

**文档结束**
