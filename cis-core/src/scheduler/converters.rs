//! DAG 定义转换器
//!
//! 提供不同 DAG 定义之间的双向转换：
//! - TaskDag ↔ UnifiedDag
//! - DagDefinition ↔ UnifiedDag
//! - DagTaskDefinition ↔ UnifiedTask
//!
//! 设计原则：
//! - 零拷贝转换（尽可能使用引用）
//! - 保持数据完整性
//! - 提供清晰的错误信息
//! - 支持批量转换

use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use crate::error::{CisError, Result};
use super::{
    DagNode, DagTask, TaskDag, RuntimeType,
    DagError as SchedulerDagError,
};
use crate::types::TaskLevel;

// ============================================================================
// UnifiedDag 定义（临时放在这里，后续移到独立模块）
// ============================================================================

/// 统一 DAG 定义
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnifiedDag {
    /// DAG 元数据
    pub metadata: DagMetadata,

    /// 任务列表
    #[serde(rename = "tasks")]
    pub tasks: Vec<UnifiedTask>,

    /// 执行策略
    #[serde(default)]
    pub execution_policy: ExecutionPolicy,
}

/// DAG 元数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DagMetadata {
    /// DAG 唯一标识符
    pub id: String,

    /// DAG 名称
    pub name: String,

    /// DAG 描述
    #[serde(default)]
    pub description: Option<String>,

    /// DAG 版本
    #[serde(default = "default_version")]
    pub version: String,

    /// 创建时间
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,

    /// 作者/创建者
    #[serde(default)]
    pub author: Option<String>,

    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// 统一任务定义
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnifiedTask {
    /// 任务 ID
    pub id: String,

    /// 任务名称
    #[serde(default)]
    pub name: Option<String>,

    /// 任务描述
    #[serde(default)]
    pub description: Option<String>,

    /// Skill 名称或 ID
    pub skill: String,

    /// Skill 方法
    #[serde(default = "default_skill_method")]
    pub method: String,

    /// 任务参数
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

    /// 重试次数
    #[serde(default)]
    pub retry: Option<u32>,

    /// 任务条件
    #[serde(default)]
    pub condition: Option<String>,

    /// 是否幂等
    #[serde(default)]
    pub idempotent: bool,

    /// 输出映射
    #[serde(default)]
    pub outputs: Option<Map<String, String>>,
}

fn default_skill_method() -> String {
    "execute".to_string()
}

/// Agent 任务配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTaskConfig {
    /// Agent Runtime 类型
    #[serde(default)]
    pub runtime: RuntimeType,

    /// 复用已有 Agent ID
    #[serde(default)]
    pub reuse_agent_id: Option<String>,

    /// 是否保持 Agent
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    /// 所有任务必须成功
    AllSuccess,
    /// 任一任务成功即可
    FirstSuccess,
    /// 允许技术债务
    AllowDebt,
    /// 继续执行直到阻塞失败
    ContinueUntilBlocking,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self::AllSuccess
    }
}

// ============================================================================
// TaskDag → UnifiedDag 转换器
// ============================================================================

impl From<TaskDag> for UnifiedDag {
    fn from(task_dag: TaskDag) -> Self {
        let tasks = task_dag.nodes.values()
            .map(|node| UnifiedTask::from_dag_node(node))
            .collect();

        Self {
            metadata: DagMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Migrated from TaskDag".to_string(),
                description: Some("自动从 TaskDag 转换的 DAG".to_string()),
                version: "1.0.0".to_string(),
                created_at: Some(Utc::now()),
                author: None,
                tags: vec!["migrated".to_string(), "task-dag".to_string()],
            },
            tasks,
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }
}

impl UnifiedTask {
    /// 从 DagNode 转换
    fn from_dag_node(node: &DagNode) -> Self {
        // 从 level 中提取 retry（如果是 Mechanical 级别）
        let retry = if let TaskLevel::Mechanical { retry: r } = &node.level {
            Some(*r as u32)
        } else {
            None
        };

        Self {
            id: node.task_id.clone(),
            name: Some(node.task_id.clone()),
            description: None,
            skill: node.skill_id.clone().unwrap_or_else(|| "unknown".to_string()),
            method: "execute".to_string(),
            params: Map::new(),
            dependencies: node.dependencies.clone(),
            level: node.level.clone(),
            agent_config: if node.agent_runtime.is_some()
                || node.reuse_agent.is_some()
                || node.agent_config.is_some() {
                Some(AgentTaskConfig {
                    runtime: node.agent_runtime.unwrap_or(RuntimeType::Default),
                    reuse_agent_id: node.reuse_agent.clone(),
                    keep_agent: node.keep_agent,
                    model: node.agent_config.as_ref().and_then(|c| c.model.clone()),
                    system_prompt: None,
                    work_dir: None,
                })
            } else {
                None
            },
            rollback: node.rollback.clone(),
            timeout_secs: None,
            retry,
            condition: None,
            idempotent: false,
            outputs: None,
        }
    }
}

impl TryFrom<UnifiedDag> for TaskDag {
    type Error = CisError;

    fn try_from(unified: UnifiedDag) -> Result<Self> {
        let mut dag = TaskDag::new();

        // 第一遍：添加节点
        for task in &unified.tasks {
            dag.add_node_with_level(
                task.id.clone(),
                task.dependencies.clone(),
                task.level.clone(),
            ).map_err(|e| CisError::scheduler(format!("Failed to add node: {}", e)))?;
        }

        // 第二遍：更新节点配置
        for task in unified.tasks {
            if let Some(node) = dag.get_node_mut(&task.id) {
                node.skill_id = Some(task.skill);

                if let Some(agent_config) = task.agent_config {
                    node.agent_runtime = Some(agent_config.runtime);
                    node.reuse_agent = agent_config.reuse_agent_id;
                    node.keep_agent = agent_config.keep_agent;

                    // 更新或创建 agent_config
                    if node.agent_config.is_none() {
                        node.agent_config = Some(super::AgentConfig::default());
                    }
                    if let Some(cfg) = &mut node.agent_config {
                        if let Some(model) = agent_config.model {
                            cfg.model = Some(model);
                        }
                    }
                }

                node.rollback = task.rollback;
            }
        }

        Ok(dag)
    }
}

// ============================================================================
// DagDefinition → UnifiedDag 转换器
// ============================================================================

// 首先定义 DagDefinition（来自 dag_executor.rs）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DagDefinition {
    pub id: String,
    pub name: String,
    pub nodes: Vec<DagDefinitionNode>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DagDefinitionNode {
    pub id: String,
    pub skill_name: String,
    pub method: String,
    pub params: Vec<u8>,
    pub dependencies: Vec<String>,
}

impl From<DagDefinition> for UnifiedDag {
    fn from(def: DagDefinition) -> Self {
        let tasks = def.nodes.into_iter()
            .map(|node| UnifiedTask::from_definition_node(node))
            .collect();

        Self {
            metadata: DagMetadata {
                id: def.id,
                name: def.name,
                description: None,
                version: "1.0.0".to_string(),
                created_at: Some(Utc::now()),
                author: None,
                tags: vec!["migrated".to_string(), "dag-definition".to_string()],
            },
            tasks,
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }
}

impl UnifiedTask {
    /// 从 DagDefinitionNode 转换
    fn from_definition_node(node: DagDefinitionNode) -> Self {
        // 尝试反序列化 params
        let params = if let Ok(map) = serde_json::from_slice::<Map<String, Value>>(&node.params) {
            map
        } else {
            // 如果反序列化失败，将参数作为 base64 编码的字符串存储
            let mut map = Map::new();
            map.insert(
                "raw".to_string(),
                Value::String(base64::encode(&node.params))
            );
            map
        };

        Self {
            id: node.id.clone(),
            name: Some(node.id.clone()),
            description: None,
            skill: node.skill_name,
            method: node.method,
            params,
            dependencies: node.dependencies,
            level: TaskLevel::Mechanical { retry: 3 },
            agent_config: None,
            rollback: None,
            timeout_secs: None,
            retry: Some(3),
            condition: None,
            idempotent: false,
            outputs: None,
        }
    }
}

impl From<UnifiedDag> for DagDefinition {
    fn from(unified: UnifiedDag) -> Self {
        let nodes = unified.tasks.into_iter()
            .map(|task| {
                let params = if task.params.is_empty() {
                    Vec::new()
                } else {
                    serde_json::to_vec(&Value::Object(task.params))
                        .unwrap_or_default()
                };

                DagDefinitionNode {
                    id: task.id,
                    skill_name: task.skill,
                    method: task.method,
                    params,
                    dependencies: task.dependencies,
                }
            })
            .collect();

        Self {
            id: unified.metadata.id,
            name: unified.metadata.name,
            nodes,
        }
    }
}

// ============================================================================
// 批量转换和辅助功能
// ============================================================================

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
            return Err(DagValidationError::CycleDetected(self.find_cycle()?));
        }

        // 4. 检查根节点
        let has_root = self.tasks.iter().any(|t| t.dependencies.is_empty());
        if !has_root {
            return Err(DagValidationError::NoRootTask);
        }

        Ok(())
    }

    /// 检测循环依赖
    fn has_cycle(&self) -> Result<bool> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();
        let task_map: HashMap<&str, &UnifiedTask> = self.tasks.iter()
            .map(|t| (t.id.as_str(), t))
            .collect();

        for task in &self.tasks {
            if !visited.contains(&task.id) {
                if self.dfs_check_cycle(&task.id, &task_map, &mut visited, &mut recursion_stack)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn dfs_check_cycle<'a>(
        &self,
        task_id: &str,
        task_map: &HashMap<&'a str, &'a UnifiedTask>,
        visited: &mut HashSet<String>,
        recursion_stack: &mut HashSet<String>,
    ) -> Result<bool> {
        visited.insert(task_id.to_string());
        recursion_stack.insert(task_id.to_string());

        if let Some(task) = task_map.get(task_id) {
            for dep_id in &task.dependencies {
                if !visited.contains(dep_id) {
                    if self.dfs_check_cycle(dep_id, task_map, visited, recursion_stack)? {
                        return Ok(true);
                    }
                } else if recursion_stack.contains(dep_id) {
                    return Ok(true);
                }
            }
        }

        recursion_stack.remove(task_id);
        Ok(false)
    }

    /// 查找循环路径
    fn find_cycle(&self) -> Result<Vec<String>> {
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        let task_map: HashMap<&str, &UnifiedTask> = self.tasks.iter()
            .map(|t| (t.id.as_str(), t))
            .collect();

        for task in &self.tasks {
            if !visited.contains(&task.id) {
                if let Some(cycle) = self.dfs_find_cycle(&task.id, &task_map, &mut visited, &mut vec![])? {
                    return Ok(cycle);
                }
            }
        }

        Ok(vec![])
    }

    fn dfs_find_cycle<'a>(
        &self,
        task_id: &str,
        task_map: &HashMap<&'a str, &'a UnifiedTask>,
        visited: &mut HashSet<String>,
        path: &[String],
    ) -> Result<Option<Vec<String>>> {
        visited.insert(task_id.to_string());

        let mut new_path = path.to_vec();
        new_path.push(task_id.to_string());

        if let Some(task) = task_map.get(task_id) {
            for (i, dep_id) in task.dependencies.iter().enumerate() {
                if let Some(pos) = new_path.iter().position(|id| id == dep_id) {
                    // 找到环，提取从环开始到结束的路径
                    let cycle = new_path[pos..].to_vec();
                    cycle.push(dep_id.clone());
                    return Ok(Some(cycle));
                }

                if !visited.contains(dep_id) {
                    if let Some(cycle) = self.dfs_find_cycle(dep_id, task_map, visited, &new_path)? {
                        return Ok(Some(cycle));
                    }
                }
            }
        }

        Ok(None)
    }

    /// 获取拓扑排序的任务列表
    pub fn topological_order(&self) -> Result<Vec<String>, DagValidationError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

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
        let mut queue: Vec<String> = in_degree.iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(task_id) = queue.pop() {
            result.push(task_id.clone());

            if let Some(neighbors) = adj.get(&task_id) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.tasks.len() {
            return Err(DagValidationError::CycleDetected(vec![]));
        }

        Ok(result)
    }

    /// 获取任务索引（用于快速查找）
    pub fn task_index(&self) -> HashMap<&str, &UnifiedTask> {
        self.tasks.iter()
            .map(|t| (t.id.as_str(), t))
            .collect()
    }

    /// 获取任务
    pub fn get_task(&self, id: &str) -> Option<&UnifiedTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// 获取根任务（无依赖的任务）
    pub fn root_tasks(&self) -> Vec<&UnifiedTask> {
        self.tasks.iter()
            .filter(|t| t.dependencies.is_empty())
            .collect()
    }
}

// ============================================================================
// 错误类型
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum DagValidationError {
    #[error("Duplicate task ID: {0}")]
    DuplicateTaskId(String),

    #[error("Dependency '{dependency}' not found for task '{task}'")]
    DependencyNotFound { task: String, dependency: String },

    #[error("Cycle detected in DAG: {:?}", cycle)]
    CycleDetected { cycle: Vec<String> },

    #[error("No root task found (all tasks have dependencies)")]
    NoRootTask,

    #[error("Task not found: {0}")]
    TaskNotFound(String),
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_unified_dag() -> UnifiedDag {
        UnifiedDag {
            metadata: DagMetadata {
                id: "test-dag".to_string(),
                name: "Test DAG".to_string(),
                description: Some("Test DAG description".to_string()),
                version: "1.0.0".to_string(),
                created_at: Some(Utc::now()),
                author: Some("Test Author".to_string()),
                tags: vec!["test".to_string()],
            },
            tasks: vec![
                UnifiedTask {
                    id: "task-1".to_string(),
                    name: Some("Task 1".to_string()),
                    description: None,
                    skill: "test-skill".to_string(),
                    method: "execute".to_string(),
                    params: Map::new(),
                    dependencies: vec![],
                    level: TaskLevel::Mechanical { retry: 3 },
                    agent_config: None,
                    rollback: None,
                    timeout_secs: None,
                    retry: Some(3),
                    condition: None,
                    idempotent: false,
                    outputs: None,
                },
                UnifiedTask {
                    id: "task-2".to_string(),
                    name: Some("Task 2".to_string()),
                    description: None,
                    skill: "test-skill".to_string(),
                    method: "execute".to_string(),
                    params: Map::new(),
                    dependencies: vec!["task-1".to_string()],
                    level: TaskLevel::Mechanical { retry: 2 },
                    agent_config: None,
                    rollback: None,
                    timeout_secs: None,
                    retry: Some(2),
                    condition: None,
                    idempotent: false,
                    outputs: None,
                },
            ],
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }

    #[test]
    fn test_validate_success() {
        let dag = create_test_unified_dag();
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_validate_duplicate_id() {
        let mut dag = create_test_unified_dag();
        let task2 = dag.tasks[1].clone();
        dag.tasks.push(task2); // 添加重复任务

        let result = dag.validate();
        assert!(matches!(result, Err(DagValidationError::DuplicateTaskId(_))));
    }

    #[test]
    fn test_validate_dependency_not_found() {
        let mut dag = create_test_unified_dag();
        dag.tasks[1].dependencies = vec!["non-existent".to_string()];

        let result = dag.validate();
        assert!(matches!(
            result,
            Err(DagValidationError::DependencyNotFound { .. })
        ));
    }

    #[test]
    fn test_validate_cycle() {
        let dag = UnifiedDag {
            metadata: DagMetadata {
                id: "cycle-dag".to_string(),
                name: "Cycle DAG".to_string(),
                version: "1.0.0".to_string(),
                created_at: None,
                author: None,
                tags: vec![],
                description: None,
            },
            tasks: vec![
                UnifiedTask {
                    id: "a".to_string(),
                    skill: "test".to_string(),
                    method: "execute".to_string(),
                    dependencies: vec!["b".to_string()],
                    ..Default::default()
                },
                UnifiedTask {
                    id: "b".to_string(),
                    skill: "test".to_string(),
                    method: "execute".to_string(),
                    dependencies: vec!["a".to_string()],
                    ..Default::default()
                },
            ],
            execution_policy: ExecutionPolicy::AllSuccess,
        };

        let result = dag.validate();
        assert!(matches!(result, Err(DagValidationError::CycleDetected { .. })));
    }

    #[test]
    fn test_topological_order() {
        let dag = create_test_unified_dag();
        let order = dag.topological_order().unwrap();

        assert_eq!(order.len(), 2);
        assert_eq!(order[0], "task-1");
        assert_eq!(order[1], "task-2");
    }

    #[test]
    fn test_task_index() {
        let dag = create_test_unified_dag();
        let index = dag.task_index();

        assert_eq!(index.len(), 2);
        assert!(index.contains_key("task-1"));
        assert!(index.contains_key("task-2"));
    }

    #[test]
    fn test_get_task() {
        let dag = create_test_unified_dag();
        let task = dag.get_task("task-1");

        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "task-1");
    }

    #[test]
    fn test_root_tasks() {
        let dag = create_test_unified_dag();
        let roots = dag.root_tasks();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].id, "task-1");
    }
}

// ============================================================================
// Default 实现
// ============================================================================

impl Default for UnifiedTask {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: None,
            description: None,
            skill: String::new(),
            method: "execute".to_string(),
            params: Map::new(),
            dependencies: Vec::new(),
            level: TaskLevel::Mechanical { retry: 3 },
            agent_config: None,
            rollback: None,
            timeout_secs: None,
            retry: Some(3),
            condition: None,
            idempotent: false,
            outputs: None,
        }
    }
}
