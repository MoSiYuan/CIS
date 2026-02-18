//! # CIS DAG Scheduler
//!
//! Provides DAG-based task dependency management and scheduling.
//!
//! ## Features
//!
//! - Task dependency tracking
//! - Cycle detection
//! - Topological sorting for execution order
//! - Failure propagation
//! - Parallel execution support at same level
//!
//! ## Architecture
//!
//! This module is 100% inherited from AgentFlow's proven implementation,
//! adapted only for CIS crate naming.

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::types::{Action, DebtEntry, FailureType, TaskLevel};
use crate::agent::{AgentConfig, AgentType};

/// DAG error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    /// Cycle detected
    CycleDetected(Vec<String>),
    /// Node not found
    NodeNotFound(String),
    /// Duplicate node
    DuplicateNode(String),
    /// Invalid dependency
    InvalidDependency(String),
    /// Invalid operation
    InvalidOperation(String),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagError::CycleDetected(cycle) => {
                write!(f, "Cycle detected in dependency graph: {:?}", cycle)
            }
            DagError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            DagError::DuplicateNode(id) => write!(f, "Duplicate node: {}", id),
            DagError::InvalidDependency(dep) => {
                write!(f, "Invalid dependency: {}", dep)
            }
            DagError::InvalidOperation(op) => write!(f, "Invalid operation: {}", op),
        }
    }
}

impl std::error::Error for DagError {}

// ===== 新模块结构 (v1.1.6) =====
pub mod core;
pub mod execution;
pub mod persistence;
pub mod events;
pub mod error;
pub mod node_selector;  // P1-10: 异构任务路由

// 重新导出新模块的类型
pub use core::{DagScheduler, SchedulerDagError, SchedulerDagNode, DagStats, SchedulerCore, TaskQueue, TaskQueueItem, TaskQueueError, TaskQueueStats};
pub use execution::{Executor, ExecutionResult, ExecutorStats, SyncExecutor, ParallelExecutor};
pub use events::{SchedulerEvent, SchedulerEventType, EventListener, EventRegistry, LoggingEventListener};
pub use persistence::{Persistence, SqlitePersistence, MemoryPersistence};
pub use node_selector::{NodeSelector, NodeInfo, NodeResources, NodeSelectorFilter};  // P1-10
// error 模块导出 Result 类型
pub use error::Result as SchedulerResult;

// ===== 旧模块（保持向后兼容） =====
pub mod event_driven;
pub mod local_executor;
pub mod multi_agent_executor;
pub mod multi_agent_executor_unified;
pub mod notify;
pub mod persistence_old;
pub mod skill_executor;
pub mod skill_executor_unified;
pub mod todo_monitor;

// 重新导出旧的 persistence 类型
pub use persistence_old::{DagPersistence, TaskExecution, TaskExecutionStatus};

// DAG 定义统一模块（v1.1.6 新增）
pub mod converters;

// DAG 统一集成测试
#[cfg(test)]
mod tests {
    use super::*;
    include!("tests/dag_tests.rs");
}

pub use event_driven::{EventDrivenConfig, EventDrivenScheduler, ExecutionSummary};
pub use local_executor::{LocalExecutor, WorkerInfo, WorkerSummary, ExecutorStats as LocalExecutorStats};
pub use multi_agent_executor::{
    MultiAgentDagExecutor, MultiAgentExecutorConfig, MultiAgentExecutionReport,
    TaskExecutionResult,
};
pub use notify::{
    CompletionNotifier, ErrorNotifier, ErrorSeverity, NotificationBundle, ReadyNotify,
    TaskCompletion, TaskError,
};
// 重新导出旧的 persistence 类型
pub use old_persistence::{DagPersistence, TaskExecution, TaskExecutionStatus};
pub use skill_executor::SkillDagExecutor;
pub use todo_monitor::{TodoListMonitor, TodoChangeEvent, TodoListLoader, FileSystemLoader};

/// Permission check result for four-tier decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionResult {
    /// Auto approve (Mechanical level)
    AutoApprove,
    /// Countdown execution (Recommended level)
    Countdown { seconds: u16, default_action: Action },
    /// Needs confirmation (Confirmed level)
    NeedsConfirmation,
    /// Needs arbitration (Arbitrated level)
    NeedsArbitration { stakeholders: Vec<String> },
}

/// DAG node status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DagNodeStatus {
    /// Waiting (dependencies not met)
    Pending,
    /// Ready (dependencies met, can execute)
    Ready,
    /// Running
    Running,
    /// Completed
    Completed,
    /// Failed
    Failed,
    /// Skipped (due to failed dependency)
    Skipped,
    /// In arbitration (human intervention required)
    Arbitrated,
    /// Technical debt (ignorable failure)
    Debt(FailureType),
}

/// DagTask - DAG 任务定义（用于 YAML 解析和内部使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTask {
    pub task_id: String,
    pub dependencies: Vec<String>,
    pub skill_id: Option<String>,
    pub level: TaskLevel,

    // === 新增字段 ===
    /// 指定使用的 Agent Runtime
    #[serde(default)]
    pub agent_runtime: Option<RuntimeType>,

    /// 复用已有 Agent ID（同 DAG 内）
    #[serde(default)]
    pub reuse_agent: Option<String>,

    /// 是否保持 Agent（执行后不销毁）
    #[serde(default = "default_keep_agent")]
    pub keep_agent: bool,

    /// Agent 配置（创建新 Agent 时用）
    #[serde(default)]
    pub agent_config: Option<AgentConfig>,

    /// P1-10: 节点选择器（异构任务路由）
    #[serde(default)]
    pub node_selector: Option<crate::scheduler::node_selector::NodeSelector>,
}

fn default_keep_agent() -> bool {
    false
}

impl std::fmt::Display for DagNodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagNodeStatus::Pending => write!(f, "pending"),
            DagNodeStatus::Ready => write!(f, "ready"),
            DagNodeStatus::Running => write!(f, "running"),
            DagNodeStatus::Completed => write!(f, "completed"),
            DagNodeStatus::Failed => write!(f, "failed"),
            DagNodeStatus::Skipped => write!(f, "skipped"),
            DagNodeStatus::Arbitrated => write!(f, "arbitrated"),
            DagNodeStatus::Debt(_) => write!(f, "debt"),
        }
    }
}

/// DAG task node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    /// Task ID
    pub task_id: String,
    /// List of dependency task_ids
    pub dependencies: Vec<String>,
    /// List of task_ids that depend on this task
    pub dependents: Vec<String>,
    /// Node status
    pub status: DagNodeStatus,
    /// Task level for four-tier decision
    pub level: TaskLevel,
    /// Rollback commands
    pub rollback: Option<Vec<String>>,

    // === Agent Teams 相关字段 ===
    /// 指定使用的 Agent Runtime
    #[serde(default)]
    pub agent_runtime: Option<RuntimeType>,

    /// 复用已有 Agent ID（同 DAG 内）
    #[serde(default)]
    pub reuse_agent: Option<String>,

    /// 是否保持 Agent（执行后不销毁）
    #[serde(default)]
    pub keep_agent: bool,

    /// Agent 配置（创建新 Agent 时用）
    #[serde(default)]
    pub agent_config: Option<AgentConfig>,

    /// P1-10: 节点选择器（异构任务路由）
    #[serde(default)]
    pub node_selector: Option<crate::scheduler::node_selector::NodeSelector>,
}

impl DagNode {
    /// Create new DAG node
    pub fn new(task_id: String, dependencies: Vec<String>) -> Self {
        Self {
            task_id,
            dependencies,
            dependents: Vec::new(),
            status: DagNodeStatus::Pending,
            level: TaskLevel::Mechanical { retry: 3 },
            rollback: None,
            agent_runtime: None,
            reuse_agent: None,
            keep_agent: false,
            agent_config: None,
            node_selector: None,  // P1-10
        }
    }

    /// Check if node can execute (all dependencies completed)
    pub fn is_ready(&self) -> bool {
        self.status == DagNodeStatus::Ready || self.status == DagNodeStatus::Running
    }

    /// Check if node is finished (completed, failed, or skipped)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            DagNodeStatus::Completed | DagNodeStatus::Failed | DagNodeStatus::Skipped
        )
    }
    
    /// Check if node is in terminal state (completed, failed, skipped, or debt)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            DagNodeStatus::Completed 
                | DagNodeStatus::Failed 
                | DagNodeStatus::Skipped
        )
    }
}

/// DAG graph structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDag {
    /// All nodes mapping (task_id -> DagNode)
    nodes: HashMap<String, DagNode>,
    /// Root nodes list (nodes with no dependencies)
    root_nodes: Vec<String>,
}

impl TaskDag {
    /// Create empty DAG graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
        }
    }

    /// Add node to DAG
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    /// * `dependencies` - List of dependency task IDs
    ///
    /// # Returns
    /// - `Ok(())` - Successfully added
    /// - `Err(DagError)` - Failed to add (e.g., node already exists)
    pub fn add_node(
        &mut self,
        task_id: String,
        dependencies: Vec<String>,
    ) -> Result<(), DagError> {
        // Check if node already exists
        if self.nodes.contains_key(&task_id) {
            return Err(DagError::DuplicateNode(task_id));
        }

        // Create new node
        let node = DagNode::new(task_id.clone(), dependencies);

        // Update dependency nodes (add current node to dependents list)
        for dep_id in &node.dependencies {
            if let Some(dep_node) = self.nodes.get_mut(dep_id) {
                dep_node.dependents.push(task_id.clone());
            }
        }

        // If node has no dependencies, add to root nodes list
        if node.dependencies.is_empty() {
            self.root_nodes.push(task_id.clone());
        }

        // Add node to graph
        self.nodes.insert(task_id.clone(), node);

        Ok(())
    }

    /// Add node with specified task level
    pub fn add_node_with_level(
        &mut self,
        task_id: String,
        dependencies: Vec<String>,
        level: TaskLevel,
    ) -> Result<(), DagError> {
        self.add_node(task_id.clone(), dependencies)?;
        if let Some(node) = self.nodes.get_mut(&task_id) {
            node.level = level;
        }
        Ok(())
    }

    /// Add node with rollback commands
    pub fn add_node_with_rollback(
        &mut self,
        task_id: String,
        dependencies: Vec<String>,
        level: TaskLevel,
        rollback: Option<Vec<String>>,
    ) -> Result<(), DagError> {
        self.add_node_with_level(task_id.clone(), dependencies, level)?;
        if let Some(node) = self.nodes.get_mut(&task_id) {
            node.rollback = rollback;
        }
        Ok(())
    }

    /// Batch add nodes
    ///
    /// # Arguments
    /// * `tasks` - Task list, each element is (task_id, dependencies)
    ///
    /// # Returns
    /// - `Ok(())` - Successfully added
    /// - `Err(DagError)` - Failed to add
    pub fn add_nodes(&mut self, tasks: Vec<(String, Vec<String>)>) -> Result<(), DagError> {
        // First add all nodes (temporarily ignore dependencies)
        for (task_id, _) in &tasks {
            if self.nodes.contains_key(task_id) {
                return Err(DagError::DuplicateNode(task_id.clone()));
            }
        }

        // Add nodes one by one (this time handling dependencies)
        for (task_id, dependencies) in tasks {
            self.add_node(task_id, dependencies)?;
        }

        Ok(())
    }

    /// Validate if DAG has circular dependencies
    ///
    /// # Returns
    /// - `Ok(())` - No circular dependencies
    /// - `Err(DagError::CycleDetected)` - Circular dependencies exist
    pub fn validate(&self) -> Result<(), DagError> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node_id in self.nodes.keys() {
            if !visited.contains(node_id) {
                if let Some(cycle) = self.dfs_detect_cycle(node_id, &mut visited, &mut rec_stack, &mut path) {
                    return Err(DagError::CycleDetected(cycle));
                }
            }
        }

        Ok(())
    }

    /// DFS detect circular dependencies (helper function)
    fn dfs_detect_cycle(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());
        path.push(node_id.to_string());

        if let Some(node) = self.nodes.get(node_id) {
            for dep_id in &node.dependencies {
                if !visited.contains(dep_id) {
                    if let Some(cycle) =
                        self.dfs_detect_cycle(dep_id, visited, rec_stack, path)
                    {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(dep_id) {
                    // Found cycle, build cycle path
                    let cycle_start = path.iter().position(|id| id == dep_id).unwrap();
                    let mut cycle = path[cycle_start..].to_vec();
                    cycle.push(dep_id.clone());
                    return Some(cycle);
                }
            }
        }

        path.pop();
        rec_stack.remove(node_id);
        None
    }

    /// Get all executable nodes (dependencies satisfied)
    ///
    /// # Returns
    /// List of executable task IDs
    pub fn get_ready_tasks(&self) -> Vec<String> {
        let mut ready_tasks = Vec::new();

        for node in self.nodes.values() {
            if node.status == DagNodeStatus::Ready {
                ready_tasks.push(node.task_id.clone());
            }
        }

        ready_tasks
    }

    /// Mark task completed, update dependent node status
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    ///
    /// # Returns
    /// - `Ok(new_ready_tasks)` - Successfully marked, returns list of newly ready tasks
    /// - `Err(DagError)` - Task doesn't exist
    pub fn mark_completed(&mut self, task_id: String) -> Result<Vec<String>, DagError> {
        let node = self
            .nodes
            .get_mut(&task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.clone()))?;

        node.status = DagNodeStatus::Completed;

        // Get dependent nodes list
        let dependents: Vec<String> = node.dependents.clone();

        // Check all other nodes that depend on this task
        let mut new_ready_tasks = Vec::new();

        for dependent_id in dependents {
            // Check if update needed
            let should_update = {
                if let Some(dependent_node) = self.nodes.get(&dependent_id) {
                    if dependent_node.status == DagNodeStatus::Pending {
                        // Check if all dependencies of this node are completed
                        self.check_dependencies_ready(dependent_node)
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            // Update node status
            if should_update {
                if let Some(dependent_node) = self.nodes.get_mut(&dependent_id) {
                    dependent_node.status = DagNodeStatus::Ready;
                    new_ready_tasks.push(dependent_id);
                }
            }
        }

        Ok(new_ready_tasks)
    }

    /// Mark task failed
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    ///
    /// # Returns
    /// - `Ok(skipped_tasks)` - Successfully marked, returns list of skipped tasks
    /// - `Err(DagError)` - Task doesn't exist
    pub fn mark_failed(&mut self, task_id: String) -> Result<Vec<String>, DagError> {
        let node = self
            .nodes
            .get_mut(&task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.clone()))?;

        node.status = DagNodeStatus::Failed;

        // Recursively mark all tasks that depend on this as Skipped
        let mut skipped_tasks = Vec::new();

        // Get dependent nodes list
        let dependents: Vec<String> = node.dependents.clone();
        for dependent_id in dependents {
            self.mark_dependents_skipped(&dependent_id, &mut skipped_tasks);
        }

        Ok(skipped_tasks)
    }

    /// Mark task as skipped
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    ///
    /// # Returns
    /// - `Ok(skipped_tasks)` - Successfully marked, returns list of skipped tasks (including downstream)
    /// - `Err(DagError)` - Task doesn't exist
    pub fn mark_skipped(&mut self, task_id: String) -> Result<Vec<String>, DagError> {
        let node = self
            .nodes
            .get_mut(&task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.clone()))?;

        // 只有当任务处于 Pending 或 Ready 状态时才标记为跳过
        if node.status == DagNodeStatus::Pending || node.status == DagNodeStatus::Ready {
            node.status = DagNodeStatus::Skipped;
        }

        // 递归标记所有依赖此任务的下游任务为跳过
        let mut skipped_tasks = vec![task_id];
        let dependents: Vec<String> = node.dependents.clone();
        
        for dependent_id in dependents {
            self.mark_dependents_skipped(&dependent_id, &mut skipped_tasks);
        }

        Ok(skipped_tasks)
    }

    /// Mark task as completed with ignorable debt (continue downstream)
    pub fn mark_task_ignorable(&mut self, task_id: &str) -> std::result::Result<Vec<String>, DagError> {
        // Mark as Debt status
        let node = self
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.to_string()))?;
        
        node.status = DagNodeStatus::Debt(FailureType::Ignorable);
        
        // Get dependent nodes list
        let dependents: Vec<String> = node.dependents.clone();
        let mut new_ready = Vec::new();
        
        // Check all other nodes that depend on this task
        for dependent_id in dependents {
            if let Some(dependent_node) = self.nodes.get(&dependent_id) {
                if dependent_node.status == DagNodeStatus::Pending {
                    // Check if all dependencies of this node are completed or ignorable
                    let all_ready = dependent_node.dependencies.iter().all(|dep_id| {
                        if let Some(dep_node) = self.nodes.get(dep_id) {
                            matches!(dep_node.status, 
                                DagNodeStatus::Completed | DagNodeStatus::Debt(FailureType::Ignorable))
                        } else {
                            false
                        }
                    });
                    
                    if all_ready {
                        if let Some(node) = self.nodes.get_mut(&dependent_id) {
                            node.status = DagNodeStatus::Ready;
                            new_ready.push(dependent_id);
                        }
                    }
                }
            }
        }
        
        Ok(new_ready)
    }

    /// Recursively mark dependent tasks as skipped
    fn mark_dependents_skipped(&mut self, task_id: &str, skipped: &mut Vec<String>) {
        // First mark current task as Skipped (if it's still Pending or Ready status)
        if let Some(node) = self.nodes.get_mut(task_id) {
            if node.status == DagNodeStatus::Pending || node.status == DagNodeStatus::Ready {
                node.status = DagNodeStatus::Skipped;
                skipped.push(task_id.to_string());
            }
        } else {
            return;
        }

        // Get list of nodes that depend on this task
        let dependents: Vec<String> = if let Some(node) = self.nodes.get(task_id) {
            node.dependents.clone()
        } else {
            return;
        };

        // Recursively process all dependent nodes
        for dependent_id in dependents {
            self.mark_dependents_skipped(&dependent_id, skipped);
        }
    }

    /// Mark task as running
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    ///
    /// # Returns
    /// - `Ok(())` - Successfully marked
    /// - `Err(DagError)` - Task doesn't exist or status is incorrect
    pub fn mark_running(&mut self, task_id: String) -> Result<(), DagError> {
        let node = self
            .nodes
            .get_mut(&task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.clone()))?;

        if node.status != DagNodeStatus::Ready {
            return Err(DagError::InvalidDependency(format!(
                "Task '{}' is not ready (current status: {})",
                task_id, node.status
            )));
        }

        node.status = DagNodeStatus::Running;
        Ok(())
    }

    /// Get topologically sorted execution levels (tasks in each level can execute in parallel)
    ///
    /// # Returns
    /// - `Ok(levels)` - Execution levels, each level is a list of task IDs
    /// - `Err(DagError)` - Circular dependencies exist
    pub fn get_execution_order(&self) -> Result<Vec<Vec<String>>, DagError> {
        // First validate for circular dependencies
        self.validate()?;

        let mut levels = Vec::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue = VecDeque::new();

        // Calculate in-degree for each node
        for node in self.nodes.values() {
            let degree = node
                .dependencies
                .iter()
                .filter(|dep| {
                    // Only count unfinished dependencies
                    if let Some(dep_node) = self.nodes.get(*dep) {
                        !dep_node.is_finished()
                    } else {
                        false
                    }
                })
                .count();

            in_degree.insert(node.task_id.clone(), degree);

            if degree == 0 && !node.is_finished() {
                queue.push_back(node.task_id.clone());
            }
        }

        // Kahn's algorithm for layered topological sort
        while !queue.is_empty() {
            let level_size = queue.len();
            let mut current_level = Vec::new();

            for _ in 0..level_size {
                let node_id = queue.pop_front().unwrap();
                current_level.push(node_id.clone());

                // Update in-degree of other nodes that depend on this node
                if let Some(node) = self.nodes.get(&node_id) {
                    for dependent_id in &node.dependents {
                        if let Some(degree) = in_degree.get_mut(dependent_id) {
                            if *degree > 0 {
                                *degree -= 1;
                                if *degree == 0 {
                                    queue.push_back(dependent_id.clone());
                                }
                            }
                        }
                    }
                }
            }

            levels.push(current_level);
        }

        Ok(levels)
    }

    /// Get node status
    ///
    /// # Arguments
    /// * `task_id` - Task ID
    ///
    /// # Returns
    /// - `Some(status)` - Node status
    /// - `None` - Node doesn't exist
    pub fn get_node_status(&self, task_id: &str) -> Option<DagNodeStatus> {
        self.nodes.get(task_id).map(|node| node.status)
    }

    /// Check if node's dependencies are all completed
    fn check_dependencies_ready(&self, node: &DagNode) -> bool {
        node.dependencies.iter().all(|dep_id| {
            if let Some(dep_node) = self.nodes.get(dep_id) {
                dep_node.status == DagNodeStatus::Completed
            } else {
                false
            }
        })
    }

    /// Get all nodes
    pub fn nodes(&self) -> &HashMap<String, DagNode> {
        &self.nodes
    }

    /// Get root nodes list
    pub fn root_nodes(&self) -> &[String] {
        &self.root_nodes
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Check if DAG is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Initialize all node statuses
    ///
    /// Mark all nodes with no dependencies or satisfied dependencies as Ready
    pub fn initialize(&mut self) {
        // Collect nodes to update
        let mut nodes_to_update: Vec<String> = Vec::new();

        for (task_id, node) in &self.nodes {
            if node.status == DagNodeStatus::Pending && node.dependencies.is_empty() {
                nodes_to_update.push(task_id.clone());
            }
        }

        // Update node statuses
        for task_id in nodes_to_update {
            if let Some(node) = self.nodes.get_mut(&task_id) {
                node.status = DagNodeStatus::Ready;
            }
        }
    }

    /// Reset all node statuses to Pending
    pub fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.status = DagNodeStatus::Pending;
        }
    }

    /// Check task permission based on TaskLevel
    pub fn check_task_permission(&self, task_id: &str) -> Result<PermissionResult, DagError> {
        let node = self.nodes.get(task_id).ok_or_else(|| DagError::NodeNotFound(task_id.to_string()))?;
        
        Ok(match &node.level {
            TaskLevel::Mechanical { .. } => PermissionResult::AutoApprove,
            TaskLevel::Recommended { default_action, timeout_secs } => PermissionResult::Countdown {
                seconds: *timeout_secs,
                default_action: *default_action,
            },
            TaskLevel::Confirmed => PermissionResult::NeedsConfirmation,
            TaskLevel::Arbitrated { stakeholders } => PermissionResult::NeedsArbitration {
                stakeholders: stakeholders.clone(),
            },
        })
    }

    /// Mark task as arbitrated
    pub fn mark_arbitrated(&mut self, task_id: String) -> Result<(), DagError> {
        let node = self.nodes.get_mut(&task_id).ok_or_else(|| DagError::NodeNotFound(task_id.clone()))?;
        node.status = DagNodeStatus::Arbitrated;
        Ok(())
    }

    /// Get node
    pub fn get_node(&self, task_id: &str) -> Option<&DagNode> {
        self.nodes.get(task_id)
    }

    /// Get mutable node
    pub fn get_node_mut(&mut self, task_id: &str) -> Option<&mut DagNode> {
        self.nodes.get_mut(task_id)
    }

    /// Get mutable nodes map
    pub fn nodes_mut(&mut self) -> &mut HashMap<String, DagNode> {
        &mut self.nodes
    }

    /// Reset a specific node to Pending status
    pub fn reset_node(&mut self, task_id: &str) -> Result<(), DagError> {
        let node = self.nodes.get_mut(task_id)
            .ok_or_else(|| DagError::NodeNotFound(task_id.to_string()))?;
        
        node.status = DagNodeStatus::Pending;
        Ok(())
    }

    /// Get task dependencies
    pub fn get_task_dependencies(&self, task_id: &str) -> Vec<String> {
        self.nodes.get(task_id)
            .map(|node| node.dependencies.clone())
            .unwrap_or_default()
    }

    /// Get accumulated debts
    pub fn get_debts(&self, dag_run_id: &str) -> Vec<DebtEntry> {
        let mut debts = Vec::new();
        for node in self.nodes.values() {
            if let DagNodeStatus::Debt(failure_type) = node.status {
                debts.push(DebtEntry {
                    task_id: node.task_id.clone(),
                    dag_run_id: dag_run_id.to_string(),
                    failure_type,
                    error_message: String::new(),
                    created_at: chrono::Utc::now(),
                    resolved: false,
                });
            }
        }
        debts
    }

    /// Resolve a debt
    pub fn resolve_debt(&mut self, task_id: &str, resume_downstream: bool) -> Result<Vec<String>, DagError> {
        let node = self.nodes.get_mut(task_id).ok_or_else(|| DagError::NodeNotFound(task_id.to_string()))?;
        
        match node.status {
            DagNodeStatus::Debt(_) => {
                if resume_downstream {
                    self.mark_completed(task_id.to_string())
                } else {
                    node.status = DagNodeStatus::Failed;
                    Ok(Vec::new())
                }
            }
            _ => Err(DagError::InvalidOperation(format!(
                "Task '{}' is not in Debt status (current: {})", task_id, node.status
            ))),
        }
    }
}

impl Default for TaskDag {
    fn default() -> Self {
        Self::new()
    }
}

/// DAG scope for worker isolation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DagScope {
    /// Global scope - shared worker for all DAGs
    /// Default: reuse existing worker unless force_new is set
    #[default]
    Global,
    
    /// Project scope - isolated worker per project
    /// Default: reuse existing worker unless force_new is set
    Project { 
        project_id: String,
        /// Whether to force creating a new worker (for cross-validation scenarios)
        #[serde(default)]
        force_new: bool,
    },
    
    /// User scope - isolated worker per user
    /// Default: reuse existing worker unless force_new is set
    User { 
        user_id: String,
        /// Whether to force creating a new worker
        #[serde(default)]
        force_new: bool,
    },
    
    /// Type scope - isolated worker per DAG type (backup/deploy/test)
    /// Default: reuse existing worker unless force_new is set
    Type { 
        dag_type: String,
        /// Whether to force creating a new worker
        #[serde(default)]
        force_new: bool,
    },
}

impl DagScope {
    /// Check if this scope requires a new worker (not reusing existing)
    pub fn force_new_worker(&self) -> bool {
        match self {
            Self::Global => false, // Global always reuses
            Self::Project { force_new, .. } => *force_new,
            Self::User { force_new, .. } => *force_new,
            Self::Type { force_new, .. } => *force_new,
        }
    }
    
    /// Get a unique worker key for this scope
    /// If force_new is true, includes a unique suffix
    pub fn worker_key(&self) -> String {
        let base = self.worker_id();
        if self.force_new_worker() {
            format!("{}-new-{}", base, uuid::Uuid::new_v4().to_string().split('-').next().unwrap())
        } else {
            base
        }
    }
    
    /// Generate worker identifier from scope
    pub fn worker_id(&self) -> String {
        match self {
            DagScope::Global => "worker-global".to_string(),
            DagScope::Project { project_id, .. } => format!("worker-project-{}", project_id),
            DagScope::User { user_id, .. } => format!("worker-user-{}", user_id),
            DagScope::Type { dag_type, .. } => format!("worker-type-{}", dag_type),
        }
    }
    
    /// Infer scope from DAG content
    pub fn infer_from_dag(dag_id: &str, tasks: &[DagTaskSpec]) -> Self {
        // 1. Try to extract from dag_id naming convention
        // Format: proj-{id}-* or user-{id}-* or {type}-*
        let parts: Vec<&str> = dag_id.split('-').collect();
        if parts.len() >= 2 {
            if parts[0] == "proj" || parts[0] == "project" {
                return DagScope::Project { 
                    project_id: parts[1].to_string(),
                    force_new: false,
                };
            }
            if parts[0] == "user" {
                return DagScope::User { 
                    user_id: parts[1].to_string(),
                    force_new: false,
                };
            }
            // Common DAG types
            let dag_types = ["backup", "deploy", "test", "build", "sync"];
            if dag_types.contains(&parts[0]) {
                return DagScope::Type { 
                    dag_type: parts[0].to_string(),
                    force_new: false,
                };
            }
        }
        
        // 2. Try to extract from task environment variables
        for task in tasks {
            if let Some(project_id) = task.env.get("PROJECT_ID") {
                return DagScope::Project { 
                    project_id: project_id.clone(),
                    force_new: false,
                };
            }
            if let Some(user_id) = task.env.get("USER_ID") {
                return DagScope::User { 
                    user_id: user_id.clone(),
                    force_new: false,
                };
            }
        }
        
        // 3. Default to Global
        DagScope::Global
    }
    
    /// Parse scope from dag_id using convention
    pub fn parse_from_id(dag_id: &str) -> Option<Self> {
        let parts: Vec<&str> = dag_id.split('-').collect();
        
        if parts.len() >= 2 {
            match parts[0] {
                "proj" | "project" => {
                    return Some(DagScope::Project { 
                        project_id: parts[1].to_string(),
                        force_new: false,
                    });
                }
                "user" => {
                    return Some(DagScope::User { 
                        user_id: parts[1].to_string(),
                        force_new: false,
                    });
                }
                "backup" | "deploy" | "test" | "build" | "sync" => {
                    return Some(DagScope::Type { 
                        dag_type: parts[0].to_string(),
                        force_new: false,
                    });
                }
                _ => {}
            }
        }
        
        None
    }
    
    /// Convert to database fields (scope_type, scope_id)
    pub fn to_db_fields(&self) -> (String, Option<String>) {
        match self {
            DagScope::Global => ("Global".to_string(), None),
            DagScope::Project { project_id, .. } => ("Project".to_string(), Some(project_id.clone())),
            DagScope::User { user_id, .. } => ("User".to_string(), Some(user_id.clone())),
            DagScope::Type { dag_type, .. } => ("Type".to_string(), Some(dag_type.clone())),
        }
    }
    
    /// Restore from database fields
    pub fn from_db_fields(scope_type: &str, scope_id: Option<&str>) -> Self {
        match scope_type {
            "Project" => DagScope::Project { 
                project_id: scope_id.unwrap_or("default").to_string(),
                force_new: false,
            },
            "User" => DagScope::User { 
                user_id: scope_id.unwrap_or("unknown").to_string(),
                force_new: false,
            },
            "Type" => DagScope::Type { 
                dag_type: scope_id.unwrap_or("default").to_string(),
                force_new: false,
            },
            _ => DagScope::Global,
        }
    }
}

/// DAG task specification (for external API - GLM/CLI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub command: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Agent Runtime 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    /// Claude Code Runtime
    Claude,
    /// Kimi Code Runtime
    Kimi,
    /// Aider Runtime
    Aider,
    /// OpenCode Runtime
    OpenCode,
    /// 使用 DAG 配置的默认 Runtime
    Default,
}

impl Default for RuntimeType {
    fn default() -> Self {
        RuntimeType::Default
    }
}

impl From<AgentType> for RuntimeType {
    fn from(agent_type: AgentType) -> Self {
        match agent_type {
            AgentType::Claude => RuntimeType::Claude,
            AgentType::Kimi => RuntimeType::Kimi,
            AgentType::Aider => RuntimeType::Aider,
            AgentType::OpenCode => RuntimeType::OpenCode,
            AgentType::Custom => RuntimeType::Default,
        }
    }
}

impl RuntimeType {
    /// 转换为 AgentType（如果可能）
    pub fn to_agent_type(self) -> Option<AgentType> {
        match self {
            RuntimeType::Claude => Some(AgentType::Claude),
            RuntimeType::Kimi => Some(AgentType::Kimi),
            RuntimeType::Aider => Some(AgentType::Aider),
            RuntimeType::OpenCode => Some(AgentType::OpenCode),
            RuntimeType::Default => None,
        }
    }
    
    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            RuntimeType::Claude => "Claude Code",
            RuntimeType::Kimi => "Kimi Code",
            RuntimeType::Aider => "Aider",
            RuntimeType::OpenCode => "OpenCode",
            RuntimeType::Default => "Default",
        }
    }
}

/// TODO item status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoItemStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Blocked,
    Skipped,
}

/// TODO item for DAG checkpoint and dynamic adjustment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DagTodoItem {
    /// Unique item ID
    pub id: String,
    /// Human readable description
    pub description: String,
    /// Current status
    #[serde(default)]
    pub status: TodoItemStatus,
    /// Related task ID (optional, for linking to DAG tasks)
    #[serde(default)]
    pub task_id: Option<String>,
    /// Priority (higher = more important)
    #[serde(default)]
    pub priority: i32,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Updated timestamp
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Completed timestamp
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Notes from agent
    #[serde(default)]
    pub notes: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

impl DagTodoItem {
    /// Create a new TODO item
    pub fn new(id: String, description: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            description,
            status: TodoItemStatus::Pending,
            task_id: None,
            priority: 0,
            created_at: now,
            updated_at: None,
            completed_at: None,
            notes: String::new(),
            tags: Vec::new(),
        }
    }

    /// Link to a task
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Mark as in progress
    pub fn mark_in_progress(&mut self) {
        self.status = TodoItemStatus::InProgress;
        self.updated_at = Some(chrono::Utc::now());
    }

    /// Mark as completed
    pub fn mark_completed(&mut self) {
        self.status = TodoItemStatus::Completed;
        self.updated_at = Some(chrono::Utc::now());
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark as blocked
    pub fn mark_blocked(&mut self, reason: impl Into<String>) {
        self.status = TodoItemStatus::Blocked;
        self.updated_at = Some(chrono::Utc::now());
        self.notes = reason.into();
    }

    /// Mark as skipped
    pub fn mark_skipped(&mut self, reason: impl Into<String>) {
        self.status = TodoItemStatus::Skipped;
        self.updated_at = Some(chrono::Utc::now());
        self.notes = reason.into();
    }

    /// Check if item is completed
    pub fn is_completed(&self) -> bool {
        self.status == TodoItemStatus::Completed
    }

    /// Check if item is pending
    pub fn is_pending(&self) -> bool {
        self.status == TodoItemStatus::Pending
    }
}

/// TODO list for DAG execution checkpoint and dynamic adjustment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DagTodoList {
    /// TODO items
    #[serde(default)]
    pub items: Vec<DagTodoItem>,
    /// Last checkpoint timestamp
    #[serde(default)]
    pub last_checkpoint: Option<chrono::DateTime<chrono::Utc>>,
    /// Agent notes
    #[serde(default)]
    pub agent_notes: String,
    /// Pending proposals (awaiting Worker review)
    #[serde(default)]
    pub pending_proposals: Vec<TodoListProposal>,
    /// Proposal history (accepted/rejected)
    #[serde(default)]
    pub proposal_history: Vec<ProposalResult>,
}

impl DagTodoList {
    /// Create a new empty TODO list
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            last_checkpoint: None,
            agent_notes: String::new(),
            pending_proposals: Vec::new(),
            proposal_history: Vec::new(),
        }
    }

    /// Submit a proposal (external agents call this)
    /// 
    /// Returns the proposal ID. If source is WorkerAgent, auto-merges.
    pub fn submit_proposal(&mut self, proposal: TodoListProposal) -> String {
        let id = proposal.id.clone();
        
        // 检查是否过期
        if proposal.is_expired() {
            self.proposal_history.push(ProposalResult::Expired { 
                proposal_id: id.clone() 
            });
            return id;
        }
        
        // WorkerAgent 的提案直接合并
        if !proposal.requires_review() {
            let result = self.merge_proposal(&proposal);
            self.proposal_history.push(result);
            return id;
        }
        
        // 外部提案需要审核
        self.pending_proposals.push(proposal);
        id
    }

    /// Worker 审核并合并提案
    /// 
    /// Worker 根据策略自主决定是否接受提案
    pub fn review_and_merge<F>(
        &mut self, 
        proposal_id: &str,
        should_accept: F
    ) -> ProposalResult
    where
        F: FnOnce(&TodoListProposal, &DagTodoList) -> bool,
    {
        let pos = self.pending_proposals.iter().position(|p| p.id == proposal_id);
        
        if let Some(pos) = pos {
            let proposal = self.pending_proposals.remove(pos);
            
            // 再次检查是否过期
            if proposal.is_expired() {
                let result = ProposalResult::Expired { 
                    proposal_id: proposal_id.to_string() 
                };
                self.proposal_history.push(result.clone());
                return result;
            }
            
            // Worker 自主决策
            if should_accept(&proposal, self) {
                let result = self.merge_proposal(&proposal);
                self.proposal_history.push(result.clone());
                result
            } else {
                let result = ProposalResult::Rejected {
                    proposal_id: proposal_id.to_string(),
                    reason: "Worker rejected the proposal".to_string(),
                };
                self.proposal_history.push(result.clone());
                result
            }
        } else {
            ProposalResult::Rejected {
                proposal_id: proposal_id.to_string(),
                reason: "Proposal not found".to_string(),
            }
        }
    }

    /// 自动合并所有安全的外部提案
    /// 
    /// 只合并低风险的变更（如优先级调整）
    pub fn auto_merge_safe_proposals(&mut self) -> Vec<ProposalResult> {
        // 分离安全和待审核的提案
        let mut safe = Vec::new();
        let mut pending = Vec::new();
        
        for proposal in std::mem::take(&mut self.pending_proposals) {
            if self.is_safe_proposal(&proposal) {
                safe.push(proposal);
            } else {
                pending.push(proposal);
            }
        }
        self.pending_proposals = pending;
        
        // 合并安全提案
        let mut results = Vec::new();
        for proposal in safe {
            let result = self.merge_proposal(&proposal);
            self.proposal_history.push(result.clone());
            results.push(result);
        }
        
        results
    }

    /// 判断提案是否安全（低风险）
    fn is_safe_proposal(&self, proposal: &TodoListProposal) -> bool {
        // 只添加新任务（不删除、不修改状态）是安全的
        let only_adds = proposal.changes.removed.is_empty() 
            && proposal.changes.modified.is_empty()
            && !proposal.changes.added.is_empty();
        
        // 只调整优先级是安全的
        let only_priority_changes = proposal.changes.added.is_empty()
            && proposal.changes.removed.is_empty()
            && proposal.changes.modified.iter().all(|m| {
                m.old_status == m.new_status && m.old_description == m.new_description
            });
        
        only_adds || only_priority_changes
    }

    /// 执行提案合并
    fn merge_proposal(&mut self, proposal: &TodoListProposal) -> ProposalResult {
        // 应用新增
        for item in &proposal.changes.added {
            if self.get(&item.id).is_none() {
                self.items.push(item.clone());
            }
        }
        
        // 应用删除
        for item in &proposal.changes.removed {
            self.remove(&item.id);
        }
        
        // 应用修改
        for change in &proposal.changes.modified {
            if let Some(item) = self.get_mut(&change.id) {
                item.status = change.new_status;
                item.priority = change.new_priority;
                item.description = change.new_description.clone();
                item.updated_at = Some(chrono::Utc::now());
            }
        }
        
        // 更新 checkpoint
        self.checkpoint(format!(
            "Merged proposal {} from {}: {}",
            proposal.id, proposal.proposer, proposal.reason
        ));
        
        ProposalResult::Accepted {
            proposal_id: proposal.id.clone(),
            merged_at: chrono::Utc::now(),
        }
    }

    /// 获取待审核的提案
    pub fn pending_review(&self) -> &[TodoListProposal] {
        &self.pending_proposals
    }

    /// 清理过期提案
    pub fn cleanup_expired_proposals(&mut self) -> usize {
        // 分离过期和待审核的提案
        let mut expired = Vec::new();
        let mut pending = Vec::new();
        
        for proposal in std::mem::take(&mut self.pending_proposals) {
            if proposal.is_expired() {
                expired.push(proposal);
            } else {
                pending.push(proposal);
            }
        }
        self.pending_proposals = pending;
        
        // 记录过期
        let count = expired.len();
        for proposal in expired {
            self.proposal_history.push(ProposalResult::Expired {
                proposal_id: proposal.id,
            });
        }
        
        count
    }

    /// Add a new item
    pub fn add_item(&mut self, item: DagTodoItem) {
        self.items.push(item);
    }

    /// Add a simple item
    pub fn add(&mut self, id: impl Into<String>, description: impl Into<String>) {
        self.add_item(DagTodoItem::new(id.into(), description.into()));
    }

    /// Get item by ID
    pub fn get(&self, id: &str) -> Option<&DagTodoItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Get mutable item by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut DagTodoItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Remove item by ID
    pub fn remove(&mut self, id: &str) -> Option<DagTodoItem> {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Get all pending items sorted by priority (desc)
    pub fn pending(&self) -> Vec<&DagTodoItem> {
        let mut items: Vec<_> = self.items.iter()
            .filter(|i| i.status == TodoItemStatus::Pending)
            .collect();
        items.sort_by_key(|i| -i.priority);
        items
    }

    /// Get all in-progress items
    pub fn in_progress(&self) -> Vec<&DagTodoItem> {
        self.items.iter()
            .filter(|i| i.status == TodoItemStatus::InProgress)
            .collect()
    }

    /// Get completion percentage
    pub fn completion_rate(&self) -> f64 {
        if self.items.is_empty() {
            return 1.0;
        }
        let completed = self.items.iter().filter(|i| i.is_completed()).count();
        completed as f64 / self.items.len() as f64
    }

    /// Save checkpoint
    pub fn checkpoint(&mut self, agent_notes: impl Into<String>) {
        self.last_checkpoint = Some(chrono::Utc::now());
        self.agent_notes = agent_notes.into();
    }

    /// Update item priority
    pub fn update_priority(&mut self, id: &str, priority: i32) -> bool {
        if let Some(item) = self.get_mut(id) {
            item.priority = priority;
            item.updated_at = Some(chrono::Utc::now());
            true
        } else {
            false
        }
    }

    /// Reorder items by priority (higher first)
    pub fn sort_by_priority(&mut self) {
        self.items.sort_by_key(|i| -i.priority);
    }

    /// Compare with another TODO list and return differences
    pub fn diff(&self, other: &DagTodoList) -> TodoListDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();

        // Find added and modified items
        for item in &other.items {
            match self.get(&item.id) {
                None => added.push(item.clone()),
                Some(existing) if existing != item => {
                    modified.push(TodoItemChange {
                        id: item.id.clone(),
                        old_status: existing.status,
                        new_status: item.status,
                        old_priority: existing.priority,
                        new_priority: item.priority,
                        old_description: existing.description.clone(),
                        new_description: item.description.clone(),
                    });
                }
                _ => {}
            }
        }

        // Find removed items
        for item in &self.items {
            if other.get(&item.id).is_none() {
                removed.push(item.clone());
            }
        }

        TodoListDiff {
            added,
            removed,
            modified,
        }
    }

    /// Create a snapshot for comparison
    pub fn snapshot(&self) -> DagTodoList {
        self.clone()
    }
}

/// Change details for a TODO item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItemChange {
    pub id: String,
    pub old_status: TodoItemStatus,
    pub new_status: TodoItemStatus,
    pub old_priority: i32,
    pub new_priority: i32,
    pub old_description: String,
    pub new_description: String,
}

/// DAG 变更提案来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSource {
    /// 来自 Room Agent（外部，需审核）
    RoomAgent,
    /// 来自 Worker Agent（本地，可信）
    WorkerAgent,
    /// 来自用户 CLI（外部，需审核）
    UserCLI,
    /// 来自自动系统（外部，需审核）
    AutoSystem,
}

impl ProposalSource {
    /// 是否需要 Worker 审核
    pub fn requires_review(&self) -> bool {
        match self {
            Self::WorkerAgent => false,  // 本地变更直接应用
            _ => true,  // 外部变更需要审核
        }
    }
}

/// TODO List 变更提案（安全模式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoListProposal {
    /// 提案 ID
    pub id: String,
    /// 提案来源
    pub source: ProposalSource,
    /// 提案者身份（DID 或 node_id）
    pub proposer: String,
    /// 变更内容
    pub changes: TodoListDiff,
    /// 提案理由
    pub reason: String,
    /// 提案时间
    pub proposed_at: chrono::DateTime<chrono::Utc>,
    /// 过期时间（可选）
    #[serde(default)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl TodoListProposal {
    /// 创建新的提案
    pub fn new(
        source: ProposalSource,
        proposer: impl Into<String>,
        changes: TodoListDiff,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("proposal-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap()),
            source,
            proposer: proposer.into(),
            changes,
            reason: reason.into(),
            proposed_at: chrono::Utc::now(),
            expires_at: None,
        }
    }

    /// 设置过期时间
    pub fn with_expiry(mut self, seconds: i64) -> Self {
        self.expires_at = Some(chrono::Utc::now() + chrono::Duration::seconds(seconds));
        self
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| chrono::Utc::now() > exp)
    }

    /// 是否需要 Worker 审核
    pub fn requires_review(&self) -> bool {
        self.source.requires_review()
    }
}

/// 提案处理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProposalResult {
    /// 已接受并合并
    Accepted { proposal_id: String, merged_at: chrono::DateTime<chrono::Utc> },
    /// 已拒绝
    Rejected { proposal_id: String, reason: String },
    /// 已过期
    Expired { proposal_id: String },
    /// 待审核（需要 Worker 确认）
    PendingReview { proposal_id: String },
}

impl ProposalResult {
    /// 获取提案 ID
    pub fn proposal_id(&self) -> &str {
        match self {
            ProposalResult::Accepted { proposal_id, .. } => proposal_id,
            ProposalResult::Rejected { proposal_id, .. } => proposal_id,
            ProposalResult::Expired { proposal_id } => proposal_id,
            ProposalResult::PendingReview { proposal_id } => proposal_id,
        }
    }
    
    /// 检查是否被接受
    pub fn is_accepted(&self) -> bool {
        matches!(self, ProposalResult::Accepted { .. })
    }
}

/// Difference between two TODO lists
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoListDiff {
    pub added: Vec<DagTodoItem>,
    pub removed: Vec<DagTodoItem>,
    pub modified: Vec<TodoItemChange>,
}

impl TodoListDiff {
    /// Check if there are any differences
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }

    /// Get count of all changes
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }

    /// Check if any high priority changes exist
    pub fn has_priority_changes(&self) -> bool {
        self.modified.iter().any(|m| m.old_priority != m.new_priority)
    }

    /// Check if any status changes exist
    pub fn has_status_changes(&self) -> bool {
        self.modified.iter().any(|m| m.old_status != m.new_status)
    }
}

/// Observer for TODO list changes
pub struct TodoListObserver {
    /// Last known state
    last_snapshot: DagTodoList,
    /// Callback for changes
    #[allow(clippy::type_complexity)]
    change_handler: Option<Box<dyn Fn(&TodoListDiff) + Send + Sync>>,
}

impl TodoListObserver {
    /// Create new observer with initial state
    pub fn new(initial: DagTodoList) -> Self {
        Self {
            last_snapshot: initial,
            change_handler: None,
        }
    }

    /// Set change handler callback
    pub fn on_change<F>(&mut self, handler: F)
    where
        F: Fn(&TodoListDiff) + Send + Sync + 'static,
    {
        self.change_handler = Some(Box::new(handler));
    }

    /// Check for changes and trigger callback if needed
    pub fn check(&mut self, current: &DagTodoList) -> TodoListDiff {
        let diff = self.last_snapshot.diff(current);
        
        if diff.has_changes() {
            // Update snapshot
            self.last_snapshot = current.clone();
            
            // Trigger callback
            if let Some(handler) = &self.change_handler {
                handler(&diff);
            }
        }
        
        diff
    }

    /// Force update snapshot without triggering callback
    pub fn update_snapshot(&mut self, current: &DagTodoList) {
        self.last_snapshot = current.clone();
    }

    /// Get last snapshot
    pub fn snapshot(&self) -> &DagTodoList {
        &self.last_snapshot
    }
}

/// Dynamic task scheduler that responds to TODO list changes
pub struct DynamicTaskScheduler {
    /// Current execution plan
    current_plan: Vec<String>, // task IDs in execution order
    /// Whether to allow dynamic reordering
    allow_reorder: bool,
}

impl DynamicTaskScheduler {
    pub fn new(allow_reorder: bool) -> Self {
        Self {
            current_plan: Vec::new(),
            allow_reorder,
        }
    }

    /// Apply TODO list diff and return tasks that need re-scheduling
    pub fn apply_diff(&mut self, diff: &TodoListDiff) -> ScheduleChanges {
        let mut to_start = Vec::new();
        let mut to_cancel = Vec::new();
        let mut to_reorder = Vec::new();

        // Handle added items
        for item in &diff.added {
            if item.status == TodoItemStatus::Pending {
                to_start.push(item.id.clone());
                self.current_plan.push(item.id.clone());
            }
        }

        // Handle removed items
        for item in &diff.removed {
            to_cancel.push(item.id.clone());
            self.current_plan.retain(|id| id != &item.id);
        }

        // Handle modifications
        for change in &diff.modified {
            if change.old_status != change.new_status {
                match change.new_status {
                    TodoItemStatus::Pending => {
                        // Task reset to pending, need to re-execute
                        to_start.push(change.id.clone());
                    }
                    TodoItemStatus::Skipped => {
                        // Task skipped, cancel if running
                        to_cancel.push(change.id.clone());
                    }
                    _ => {}
                }
            }

            // Check priority changes for reordering
            if self.allow_reorder && change.old_priority != change.new_priority {
                to_reorder.push((change.id.clone(), change.new_priority));
            }
        }

        // Reorder by priority if needed
        if self.allow_reorder && !to_reorder.is_empty() {
            self.reorder_by_priority(&to_reorder);
        }

        ScheduleChanges {
            to_start,
            to_cancel,
            reordered: !to_reorder.is_empty(),
        }
    }

    /// Reorder current plan by priority
    fn reorder_by_priority(&mut self, priority_updates: &[(String, i32)]) {
        // Create a map for quick lookup
        let priority_map: std::collections::HashMap<_, _> = 
            priority_updates.iter().cloned().collect();

        // Sort current plan by priority (higher first)
        self.current_plan.sort_by_key(|id| {
            -priority_map.get(id).copied().unwrap_or(0)
        });
    }

    /// Get current execution plan
    pub fn current_plan(&self) -> &[String] {
        &self.current_plan
    }

    /// Initialize plan from TODO list
    pub fn init_from_todo(&mut self, todo_list: &DagTodoList) {
        self.current_plan = todo_list.pending()
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
    }
}

/// Changes to apply to the schedule
#[derive(Debug, Clone, Default)]
pub struct ScheduleChanges {
    pub to_start: Vec<String>,
    pub to_cancel: Vec<String>,
    pub reordered: bool,
}

impl ScheduleChanges {
    pub fn has_changes(&self) -> bool {
        !self.to_start.is_empty() || !self.to_cancel.is_empty() || self.reordered
    }
}

/// DAG specification for external API (from GLM/CLI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagSpec {
    /// Unique DAG identifier
    pub dag_id: String,
    
    /// Human readable description
    #[serde(default)]
    pub description: String,
    
    /// Task specifications
    pub tasks: Vec<DagTaskSpec>,
    
    /// Target node for execution (optional, for explicit routing)
    #[serde(default)]
    pub target_node: Option<String>,
    
    /// Scope for worker isolation
    #[serde(default)]
    pub scope: DagScope,
    
    /// Execution priority
    #[serde(default)]
    pub priority: crate::types::TaskPriority,
    
    /// Cron schedule (optional, for recurring DAGs)
    #[serde(default)]
    pub schedule: Option<String>,
    
    /// Version for optimistic locking
    #[serde(default = "default_version")]
    pub version: i64,
    
    /// TODO list for checkpoint and dynamic adjustment
    #[serde(default)]
    pub todo_list: DagTodoList,
}

fn default_version() -> i64 {
    1
}

impl DagSpec {
    /// Create new DAG specification
    pub fn new(dag_id: String, tasks: Vec<DagTaskSpec>) -> Self {
        let scope = DagScope::infer_from_dag(&dag_id, &tasks);
        
        // Initialize todo_list from tasks
        let mut todo_list = DagTodoList::new();
        for task in &tasks {
            todo_list.add_item(
                DagTodoItem::new(task.id.clone(), format!("Execute task: {}", task.id))
                    .with_task(&task.id)
            );
        }
        
        Self {
            dag_id,
            description: String::new(),
            tasks,
            target_node: None,
            scope,
            priority: crate::types::TaskPriority::Medium,
            schedule: None,
            version: 1,
            todo_list,
        }
    }
    
    /// Get content hash for deduplication
    pub fn content_hash(&self) -> String {
        use sha2::{Sha256, Digest};
        let content = serde_json::to_string(&self.tasks).unwrap_or_default();
        let hash = Sha256::digest(content.as_bytes());
        format!("{:x}", hash)
    }
    
    /// Convert to TaskDag for execution
    pub fn to_task_dag(&self) -> Result<TaskDag, DagError> {
        let mut dag = TaskDag::new();
        
        for task in &self.tasks {
            dag.add_node(task.id.clone(), task.depends_on.clone())?;
        }
        
        dag.initialize();
        Ok(dag)
    }
    
    /// Get worker identifier
    pub fn worker_id(&self) -> String {
        self.scope.worker_id()
    }
}

/// Scope inference and conflict detection
pub struct ScopeInferrer;

impl ScopeInferrer {
    /// Infer scope with full logic (Task 2.1)
    /// 
    /// Priority:
    /// 1. Explicit scope in spec
    /// 2. Environment variable inference (PROJECT_ID, USER_ID, SCOPE_TYPE)
    /// 3. dag_id pattern matching (proj-{id}-*, user-{id}-*, etc.)
    /// 4. Default to Global
    pub fn infer(
        explicit: Option<DagScope>,
        dag_id: &str,
        tasks: &[DagTaskSpec],
    ) -> DagScope {
        // 1. Explicit scope takes highest priority
        if let Some(scope) = explicit {
            tracing::debug!("Using explicit scope: {:?}", scope);
            return scope;
        }

        // 2. Try environment variable inference
        if let Some(scope) = Self::infer_from_env(tasks) {
            tracing::debug!("Inferred scope from env: {:?}", scope);
            return scope;
        }

        // 3. Try dag_id pattern matching
        if let Some(scope) = DagScope::parse_from_id(dag_id) {
            tracing::debug!("Inferred scope from dag_id: {:?}", scope);
            return scope;
        }

        // 4. Default to Global
        tracing::debug!("Using default Global scope");
        DagScope::Global
    }

    /// Infer scope from environment variables in tasks
    fn infer_from_env(tasks: &[DagTaskSpec]) -> Option<DagScope> {
        for task in tasks {
            let env = &task.env;
            // Check PROJECT_ID first
            if let Some(project_id) = env.get("PROJECT_ID") {
                return Some(DagScope::Project {
                    project_id: project_id.clone(),
                    force_new: false,
                });
            }
            // Then USER_ID
            if let Some(user_id) = env.get("USER_ID") {
                return Some(DagScope::User {
                    user_id: user_id.clone(),
                    force_new: false,
                });
            }
            // Then SCOPE_TYPE
            if let Some(scope_type) = env.get("SCOPE_TYPE") {
                return Some(DagScope::Type {
                    dag_type: scope_type.clone(),
                    force_new: false,
                });
            }
        }
        None
    }

    /// Detect scope conflicts (Task 2.2)
    /// 
    /// Returns list of conflict descriptions
    /// 
    /// Input: Vec<(dag_id, scope, target_node)>
    /// Conflict: Same worker_id but different target_node
    pub fn detect_conflicts(
        entries: &[(String, DagScope, Option<String>)],
    ) -> Vec<ScopeConflict> {
        use std::collections::HashMap;

        let mut worker_nodes: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();

        // Group by worker_id
        for (dag_id, scope, target_node) in entries {
            let worker_id = scope.worker_id();
            worker_nodes
                .entry(worker_id)
                .or_default()
                .push((dag_id.clone(), target_node.clone()));
        }

        let mut conflicts = Vec::new();

        // Check each worker group for conflicts
        for (worker_id, entries) in &worker_nodes {
            if entries.len() < 2 {
                continue;
            }

            // Collect unique non-empty target nodes
            let nodes: std::collections::HashSet<_> = entries
                .iter()
                .filter_map(|(_, node)| node.clone().filter(|n| !n.is_empty()))
                .collect();

            if nodes.len() > 1 {
                // Found conflict: same worker, different target nodes
                let dag_ids: Vec<_> = entries.iter().map(|(id, _)| id.clone()).collect();
                conflicts.push(ScopeConflict {
                    worker_id: worker_id.clone(),
                    dag_ids,
                    conflicting_nodes: nodes.into_iter().collect(),
                });
            }
        }

        conflicts
    }

    /// Validate entries and return Result
    pub fn validate(entries: &[(String, DagScope, Option<String>)]) -> Result<(), Vec<ScopeConflict>> {
        let conflicts = Self::detect_conflicts(entries);
        if conflicts.is_empty() {
            Ok(())
        } else {
            Err(conflicts)
        }
    }
}

/// Scope conflict information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConflict {
    /// Worker ID with conflict
    pub worker_id: String,
    /// DAG IDs involved
    pub dag_ids: Vec<String>,
    /// Conflicting target nodes
    pub conflicting_nodes: Vec<String>,
}

impl std::fmt::Display for ScopeConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Scope conflict in worker '{}': DAGs {:?} have different target nodes {:?}",
            self.worker_id, self.dag_ids, self.conflicting_nodes
        )
    }
}

/// DAG execution priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DagPriority {
    /// Low priority
    Low,
    /// Normal priority (default)
    #[default]
    Normal,
    /// High priority
    High,
    /// Critical priority (skip queue)
    Critical,
}

impl DagPriority {
    /// Get numeric priority value (higher = more important)
    pub fn value(&self) -> i32 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

/// DAG run status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DagRunStatus {
    Running,
    Paused,
    Completed,
    Failed,
}

impl std::fmt::Display for DagRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagRunStatus::Running => write!(f, "running"),
            DagRunStatus::Paused => write!(f, "paused"),
            DagRunStatus::Completed => write!(f, "completed"),
            DagRunStatus::Failed => write!(f, "failed"),
        }
    }
}

/// DAG execution instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagRun {
    pub run_id: String,
    pub dag: TaskDag,
    pub status: DagRunStatus,
    pub debts: Vec<DebtEntry>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Source DAG file path (for reloading task definitions)
    #[serde(default)]
    pub source_file: Option<String>,
    /// Task commands/skill mappings for execution (task_id -> command/skill)
    #[serde(default)]
    pub task_commands: HashMap<String, String>,
    /// DAG scope for worker assignment
    #[serde(default)]
    pub scope: DagScope,
    /// Target node for execution (None = any node)
    #[serde(default)]
    pub target_node: Option<String>,
    /// Priority for scheduling
    #[serde(default)]
    pub priority: DagPriority,
    /// TODO list for checkpoint and dynamic task adjustment
    #[serde(default)]
    pub todo_list: DagTodoList,
    /// Optimistic locking version (Task 5.2)
    #[serde(default = "default_version")]
    pub version: i64,
}

impl DagRun {
    pub fn new(dag: TaskDag) -> Self {
        let now = chrono::Utc::now();
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            dag,
            status: DagRunStatus::Running,
            debts: Vec::new(),
            created_at: now,
            updated_at: now,
            source_file: None,
            task_commands: HashMap::new(),
            scope: DagScope::default(),
            target_node: None,
            priority: DagPriority::default(),
            todo_list: DagTodoList::new(),
            version: 1,
        }
    }

    pub fn with_run_id(dag: TaskDag, run_id: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            run_id,
            dag,
            status: DagRunStatus::Running,
            debts: Vec::new(),
            created_at: now,
            updated_at: now,
            source_file: None,
            task_commands: HashMap::new(),
            scope: DagScope::default(),
            target_node: None,
            priority: DagPriority::default(),
            todo_list: DagTodoList::new(),
            version: 1,
        }
    }

    /// Set scope for the run
    pub fn with_scope(mut self, scope: DagScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set target node for the run
    pub fn with_target_node(mut self, node: impl Into<String>) -> Self {
        self.target_node = Some(node.into());
        self
    }

    /// Set priority for the run
    pub fn with_priority(mut self, priority: DagPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set TODO list for the run
    pub fn with_todo_list(mut self, todo_list: DagTodoList) -> Self {
        self.todo_list = todo_list;
        self
    }

    /// Initialize TODO list from DAG tasks
    pub fn init_todo_from_tasks(&mut self) {
        for node in self.dag.nodes().values() {
            if self.todo_list.get(&node.task_id).is_none() {
                self.todo_list.add_item(
                    DagTodoItem::new(node.task_id.clone(), format!("Execute: {}", node.task_id))
                        .with_task(&node.task_id)
                );
            }
        }
    }

    /// Checkpoint the current state
    pub fn checkpoint(&mut self, agent_notes: impl Into<String>) {
        self.todo_list.checkpoint(agent_notes);
        self.updated_at = chrono::Utc::now();
    }

    /// Sync TODO list with external source and return changes
    /// 
    /// This method compares the current TODO list with an external source
    /// (e.g., from storage or user update) and returns the differences.
    /// The agent should call this periodically to detect changes.
    pub fn sync_todo_list(&mut self, external: &DagTodoList) -> TodoListDiff {
        let diff = self.todo_list.diff(external);
        
        if diff.has_changes() {
            tracing::info!(
                "TODO list changed for run {}: {} added, {} removed, {} modified",
                self.run_id,
                diff.added.len(),
                diff.removed.len(),
                diff.modified.len()
            );
            
            // Apply external changes to local
            self.todo_list = external.clone();
            self.updated_at = chrono::Utc::now();
        }
        
        diff
    }

    /// Apply schedule changes based on TODO list diff
    pub fn apply_schedule_changes(&mut self, changes: &ScheduleChanges) {
        // Cancel tasks that were removed or skipped
        for task_id in &changes.to_cancel {
            if let Some(node) = self.dag.nodes_mut().get_mut(task_id) {
                if node.status == DagNodeStatus::Running {
                    tracing::info!("Cancelling task {} due to TODO list change", task_id);
                    // Note: Actual cancellation needs to be handled by executor
                }
            }
        }
        
        // Reset tasks that need re-execution
        for task_id in &changes.to_start {
            if let Err(e) = self.dag.reset_node(task_id) {
                tracing::warn!("Failed to reset task {}: {}", task_id, e);
            }
        }
        
        if changes.reordered {
            tracing::info!("Execution plan reordered for run {}", self.run_id);
        }
    }

    /// Get worker ID based on scope (respects force_new)
    pub fn worker_id(&self) -> String {
        self.scope.worker_id()
    }
    
    /// Get worker key (includes unique suffix if force_new)
    pub fn worker_key(&self) -> String {
        self.scope.worker_key()
    }

    pub fn to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn add_debt(&mut self, mut debt: DebtEntry) {
        debt.dag_run_id = self.run_id.clone();
        self.debts.push(debt);
    }

    pub fn unresolved_debts(&self) -> Vec<&DebtEntry> {
        self.debts.iter().filter(|d| !d.resolved).collect()
    }

    pub fn resolved_debts(&self) -> Vec<&DebtEntry> {
        self.debts.iter().filter(|d| d.resolved).collect()
    }

    pub fn resolve_debt(&mut self, task_id: &str) -> std::result::Result<(), DagError> {
        let debt = self.debts.iter_mut().find(|d| d.task_id == task_id && !d.resolved)
            .ok_or_else(|| DagError::InvalidOperation(format!("No unresolved debt found for task {}", task_id)))?;
        debt.resolved = true;
        Ok(())
    }

    pub fn update_status(&mut self) {
        let all_finished = self.dag.nodes().values().all(|n| n.is_terminal());
        let has_failed = self.dag.nodes().values().any(|n| matches!(n.status, DagNodeStatus::Failed | DagNodeStatus::Skipped));
        let has_blocking_debt = self.dag.nodes().values().any(|n| matches!(n.status, DagNodeStatus::Debt(FailureType::Blocking)));
        let has_unresolved_arbitration = self.dag.nodes().values().any(|n| n.status == DagNodeStatus::Arbitrated);

        self.status = if has_unresolved_arbitration {
            DagRunStatus::Paused
        } else if has_blocking_debt || has_failed {
            DagRunStatus::Failed
        } else if all_finished {
            DagRunStatus::Completed
        } else {
            DagRunStatus::Running
        };
    }

    /// Optimistic lock: Update with version check (Task 5.2)
    /// 
    /// Returns true if update succeeded, false if version mismatch
    /// 
    /// SQL equivalent:
    /// UPDATE dag_runs SET status=?, version=version+1 WHERE id=? AND version=?
    pub fn update_with_version(&mut self, expected_version: i64) -> bool {
        if self.version != expected_version {
            tracing::warn!(
                "Version mismatch for run {}: expected {}, got {}",
                self.run_id, expected_version, self.version
            );
            false
        } else {
            self.version += 1;
            self.updated_at = chrono::Utc::now();
            true
        }
    }

    /// Increment version (for local updates)
    pub fn bump_version(&mut self) {
        self.version += 1;
        self.updated_at = chrono::Utc::now();
    }

    /// Get current version
    pub fn version(&self) -> i64 {
        self.version
    }
}

/* // Deprecated: Use core::DagScheduler instead
/// DAG scheduler managing multiple DAG runs
pub struct DagScheduler {
    runs: HashMap<String, DagRun>,
    active_run: Option<String>,
    persistence: Option<DagPersistence>,
}

impl DagScheduler {
    pub fn new() -> Self {
        Self {
            runs: HashMap::new(),
            active_run: None,
            persistence: None,
        }
    }

    pub fn with_persistence(db_path: &str) -> Result<Self> {
        let persistence = DagPersistence::new(db_path)?;
        let runs_list = persistence.list_runs()?;
        let mut runs = HashMap::new();
        let active_run = runs_list.first().map(|(id, _, _)| id.clone());
        
        for (run_id, _, _) in runs_list {
            if let Some(run) = persistence.load_run(&run_id)? {
                runs.insert(run_id, run);
            }
        }
        
        Ok(Self { runs, active_run, persistence: Some(persistence) })
    }

    pub fn persistence(&self) -> Option<&DagPersistence> {
        self.persistence.as_ref()
    }

    fn persist_run(&self, run: &DagRun) -> Result<()> {
        if let Some(ref persistence) = self.persistence {
            persistence.save_run_simple(run)?;
        }
        Ok(())
    }

    pub fn create_run(&mut self, dag: TaskDag) -> String {
        let run = DagRun::new(dag);
        let run_id = run.run_id.clone();
        let _ = self.persist_run(&run);
        self.runs.insert(run_id.clone(), run);
        if self.active_run.is_none() {
            self.active_run = Some(run_id.clone());
        }
        run_id
    }

    pub fn create_run_with_id(&mut self, dag: TaskDag, run_id: String) -> String {
        let run = DagRun::with_run_id(dag, run_id.clone());
        let _ = self.persist_run(&run);
        self.runs.insert(run_id.clone(), run);
        if self.active_run.is_none() {
            self.active_run = Some(run_id.clone());
        }
        run_id
    }

    /// Create a new DAG run with source file and task commands
    pub fn create_run_with_source(
        &mut self,
        dag: TaskDag,
        run_id: Option<String>,
        source_file: Option<String>,
        task_commands: HashMap<String, String>,
    ) -> String {
        let mut run = if let Some(id) = run_id {
            DagRun::with_run_id(dag, id)
        } else {
            DagRun::new(dag)
        };
        run.source_file = source_file;
        run.task_commands = task_commands;
        
        let run_id = run.run_id.clone();
        let _ = self.persist_run(&run);
        self.runs.insert(run_id.clone(), run);
        if self.active_run.is_none() {
            self.active_run = Some(run_id.clone());
        }
        run_id
    }

    pub fn get_run(&self, run_id: &str) -> Option<&DagRun> {
        self.runs.get(run_id)
    }

    pub fn get_run_mut(&mut self, run_id: &str) -> Option<&mut DagRun> {
        self.runs.get_mut(run_id)
    }

    /// 标记任务为跳过状态
    pub fn mark_skipped(
        &mut self,
        run_id: &str,
        task_id: &str,
    ) -> std::result::Result<Vec<String>, DagError> {
        let run = self.runs.get_mut(run_id)
            .ok_or_else(|| DagError::NodeNotFound(run_id.to_string()))?;
        
        // 将任务标记为跳过，并获取被跳过的下游任务
        let skipped_tasks = run.dag.mark_skipped(task_id.to_string())?;
        
        run.update_status();
        run.updated_at = chrono::Utc::now();
        
        // 持久化
        let run_clone = run.clone();
        let _ = self.persist_run(&run_clone);
        
        Ok(skipped_tasks)
    }

    pub fn mark_failed_with_type(
        &mut self,
        run_id: &str,
        task_id: &str,
        failure_type: FailureType,
        error_message: String,
    ) -> std::result::Result<Vec<String>, DagError> {
        let run = self.runs.get_mut(run_id).ok_or_else(|| DagError::NodeNotFound(run_id.to_string()))?;
        
        // 添加债务记录
        run.add_debt(DebtEntry {
            task_id: task_id.to_string(),
            dag_run_id: run_id.to_string(),
            failure_type,
            error_message,
            created_at: chrono::Utc::now(),
            resolved: false,
        });
        
        // 根据失败类型处理
        let skipped = match failure_type {
            FailureType::Ignorable => {
                // 标记为 Ignorable 债务，但继续执行
                run.dag.mark_task_ignorable(task_id)?
            }
            FailureType::Blocking => {
                // 标记为 Blocking 债务，跳过下游
                run.status = DagRunStatus::Paused;
                run.dag.mark_failed(task_id.to_string())?
            }
        };
        
        run.update_status();
        run.updated_at = chrono::Utc::now();
        
        // 持久化
        let run_clone = run.clone();
        let _ = self.persist_run(&run_clone);
        
        Ok(skipped)
    }

    pub fn get_active_run(&self) -> Option<&DagRun> {
        self.active_run.as_ref().and_then(|id| self.runs.get(id))
    }

    pub fn get_active_run_mut(&mut self) -> Option<&mut DagRun> {
        self.active_run.as_ref().and_then(|id| self.runs.get_mut(id))
    }

    pub fn set_active_run(&mut self, run_id: String) -> std::result::Result<(), DagError> {
        if !self.runs.contains_key(&run_id) {
            return Err(DagError::NodeNotFound(run_id));
        }
        self.active_run = Some(run_id);
        Ok(())
    }

    pub fn remove_run(&mut self, run_id: &str) -> Option<DagRun> {
        if let Some(ref persistence) = self.persistence {
            let _ = persistence.delete_run(run_id);
        }
        let run = self.runs.remove(run_id);
        if self.active_run.as_deref() == Some(run_id) {
            self.active_run = self.runs.keys().next().cloned();
        }
        run
    }

    pub fn run_ids(&self) -> impl Iterator<Item = &String> {
        self.runs.keys()
    }

    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    pub fn resolve_run_debt(&mut self, run_id: &str, task_id: &str, resume_downstream: bool) -> std::result::Result<Vec<String>, DagError> {
        let run = self.runs.get_mut(run_id).ok_or_else(|| DagError::NodeNotFound(run_id.to_string()))?;
        let new_ready = run.dag.resolve_debt(task_id, resume_downstream)?;
        if resume_downstream {
            run.resolve_debt(task_id)?;
        }
        run.update_status();
        run.updated_at = chrono::Utc::now();
        let run_clone = run.clone();
        let _ = self.persist_run(&run_clone);
        Ok(new_ready)
    }

    pub fn update_run(&mut self, run: DagRun) -> Result<()> {
        let run_id = run.run_id.clone();
        let mut run = run;
        run.updated_at = chrono::Utc::now();
        self.persist_run(&run)?;
        self.runs.insert(run_id, run);
        Ok(())
    }

    pub fn get_run_debts(&self, run_id: &str) -> std::result::Result<Vec<DebtEntry>, DagError> {
        let run = self.runs.get(run_id).ok_or_else(|| DagError::NodeNotFound(run_id.to_string()))?;
        Ok(run.debts.clone())
    }

    pub fn get_all_debts(&self) -> Vec<DebtEntry> {
        let mut all_debts = Vec::new();
        for run in self.runs.values() {
            all_debts.extend(run.debts.clone());
        }
        all_debts
    }

    pub fn find_run_by_task(&self, task_id: &str) -> Option<String> {
        for run in self.runs.values() {
            if run.debts.iter().any(|d| d.task_id == task_id && !d.resolved) {
                return Some(run.run_id.clone());
            }
        }
        None
    }

    pub fn resume_run(&mut self, run_id: &str) -> std::result::Result<(), DagError> {
        let run = self.runs.get_mut(run_id).ok_or_else(|| DagError::NodeNotFound(run_id.to_string()))?;
        if run.status == DagRunStatus::Paused {
            run.status = DagRunStatus::Running;
            run.updated_at = chrono::Utc::now();
            let run_clone = run.clone();
            let _ = self.persist_run(&run_clone);
        }
        Ok(())
    }
}

impl Default for DagScheduler {
    fn default() -> Self {
        Self::new()
    }
}
*/ // End deprecated DagScheduler block


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node() {
        let mut dag = TaskDag::new();

        // Add node with no dependencies
        dag.add_node("task1".to_string(), vec![]).unwrap();
        assert_eq!(dag.node_count(), 1);
        assert_eq!(dag.root_nodes().len(), 1);

        // Add node with dependencies
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        assert_eq!(dag.node_count(), 2);
        assert_eq!(dag.root_nodes().len(), 1);
    }

    #[test]
    fn test_add_duplicate_node() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        let result = dag.add_node("task1".to_string(), vec![]);

        assert!(matches!(result, Err(DagError::DuplicateNode(_))));
    }

    #[test]
    fn test_add_node_with_forward_dependency() {
        let mut dag = TaskDag::new();

        // Allow adding node with dependency that doesn't exist yet (forward dependency)
        // Actual dependency relationships will be validated after all nodes added
        let result = dag.add_node("task1".to_string(), vec!["task2".to_string()]);
        assert!(result.is_ok());

        // Add the depended node
        let result = dag.add_node("task2".to_string(), vec![]);
        assert!(result.is_ok());

        // Validate DAG has no cycles
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_validate_no_cycle() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task4".to_string(), vec!["task2".to_string(), "task3".to_string()])
            .unwrap();

        assert!(dag.validate().is_ok());
    }

    #[test]
    fn test_validate_with_cycle() {
        let mut dag = TaskDag::new();

        // Create circular dependency: task1 -> task2 -> task3 -> task1
        dag.add_node("task1".to_string(), vec!["task3".to_string()])
            .unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task2".to_string()])
            .unwrap();

        let result = dag.validate();
        assert!(matches!(result, Err(DagError::CycleDetected(_))));
    }

    #[test]
    fn test_get_ready_tasks() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task1".to_string()])
            .unwrap();

        dag.initialize();

        let ready = dag.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&"task1".to_string()));
    }

    #[test]
    fn test_mark_completed() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task1".to_string()])
            .unwrap();

        dag.initialize();

        // Mark task1 as completed
        let new_ready = dag.mark_completed("task1".to_string()).unwrap();

        // task2 and task3 should become Ready
        assert_eq!(new_ready.len(), 2);
        assert!(new_ready.contains(&"task2".to_string()));
        assert!(new_ready.contains(&"task3".to_string()));
    }

    #[test]
    fn test_mark_failed() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task2".to_string()])
            .unwrap();

        dag.initialize();

        // Mark task2 as failed
        let skipped = dag.mark_failed("task2".to_string()).unwrap();

        // task3 should be skipped
        assert_eq!(skipped.len(), 1);
        assert!(skipped.contains(&"task3".to_string()));

        // Check status
        assert_eq!(
            dag.get_node_status("task2"),
            Some(DagNodeStatus::Failed)
        );
        assert_eq!(
            dag.get_node_status("task3"),
            Some(DagNodeStatus::Skipped)
        );
    }

    #[test]
    fn test_get_execution_order() {
        let mut dag = TaskDag::new();

        // Build a simple DAG:
        //     task1
        //     /    \
        // task2  task3
        //     \    /
        //     task4
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task3".to_string(), vec!["task1".to_string()])
            .unwrap();
        dag.add_node("task4".to_string(), vec!["task2".to_string(), "task3".to_string()])
            .unwrap();

        dag.initialize();

        let levels = dag.get_execution_order().unwrap();

        // Should have 3 levels
        assert_eq!(levels.len(), 3);

        // First level: task1
        assert_eq!(levels[0].len(), 1);
        assert!(levels[0].contains(&"task1".to_string()));

        // Second level: task2, task3
        assert_eq!(levels[1].len(), 2);
        assert!(levels[1].contains(&"task2".to_string()));
        assert!(levels[1].contains(&"task3".to_string()));

        // Third level: task4
        assert_eq!(levels[2].len(), 1);
        assert!(levels[2].contains(&"task4".to_string()));
    }

    #[test]
    fn test_mark_running() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        // Mark task as running
        dag.mark_running("task1".to_string()).unwrap();

        assert_eq!(
            dag.get_node_status("task1"),
            Some(DagNodeStatus::Running)
        );
    }

    #[test]
    fn test_reset() {
        let mut dag = TaskDag::new();

        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        dag.mark_running("task1".to_string()).unwrap();
        dag.mark_completed("task1".to_string()).unwrap();

        // Reset
        dag.reset();

        assert_eq!(
            dag.get_node_status("task1"),
            Some(DagNodeStatus::Pending)
        );
    }

    #[test]
    fn test_complex_dag() {
        let mut dag = TaskDag::new();

        // Build a more complex DAG
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.add_node("C".to_string(), vec!["A".to_string()]).unwrap();
        dag.add_node("D".to_string(), vec!["B".to_string()]).unwrap();
        dag.add_node("E".to_string(), vec!["B".to_string(), "C".to_string()])
            .unwrap();
        dag.add_node("F".to_string(), vec!["D".to_string(), "E".to_string()])
            .unwrap();

        dag.initialize();

        // Validate no cycles
        assert!(dag.validate().is_ok());

        // Get execution order
        let levels = dag.get_execution_order().unwrap();
        assert_eq!(levels.len(), 4); // A -> [B,C] -> [D,E] -> F

        // Test execution flow
        let ready = dag.get_ready_tasks();
        assert_eq!(ready, vec!["A".to_string()]);

        dag.mark_running("A".to_string()).unwrap();
        let new_ready = dag.mark_completed("A".to_string()).unwrap();
        assert_eq!(new_ready.len(), 2); // B and C become Ready

        dag.mark_running("B".to_string()).unwrap();
        let new_ready = dag.mark_completed("B".to_string()).unwrap();
        assert_eq!(new_ready.len(), 1); // D becomes Ready (D only depends on B)

        dag.mark_running("C".to_string()).unwrap();
        let new_ready = dag.mark_completed("C".to_string()).unwrap();
        assert_eq!(new_ready.len(), 1); // E becomes Ready (E depends on B and C, B completed)
    }

    // ===== Phase 2: Scope Inference Tests =====

    #[test]
    fn test_scope_infer_explicit() {
        // Test explicit scope takes priority
        let explicit = Some(DagScope::Project { 
            project_id: "test-proj".to_string(), 
            force_new: false 
        });
        let tasks = vec![];
        
        let scope = ScopeInferrer::infer(explicit, "any-dag", &tasks);
        assert_eq!(scope.worker_id(), "worker-project-test-proj");
    }

    #[test]
    fn test_scope_infer_from_env_project_id() {
        // Test PROJECT_ID env inference
        let tasks = vec![
            DagTaskSpec {
                id: "task1".to_string(),
                task_type: "shell".to_string(),
                command: "echo test".to_string(),
                depends_on: vec![],
                env: [("PROJECT_ID".to_string(), "env-project".to_string())].into_iter().collect(),
            }
        ];
        
        let scope = ScopeInferrer::infer(None, "any-dag", &tasks);
        assert_eq!(scope.worker_id(), "worker-project-env-project");
    }

    #[test]
    fn test_scope_infer_from_env_user_id() {
        // Test USER_ID env inference
        let tasks = vec![
            DagTaskSpec {
                id: "task1".to_string(),
                task_type: "shell".to_string(),
                command: "echo test".to_string(),
                depends_on: vec![],
                env: [("USER_ID".to_string(), "john".to_string())].into_iter().collect(),
            }
        ];
        
        let scope = ScopeInferrer::infer(None, "any-dag", &tasks);
        assert_eq!(scope.worker_id(), "worker-user-john");
    }

    #[test]
    fn test_scope_infer_from_dag_id_project() {
        // Test dag_id pattern: proj-{id}-*
        let tasks = vec![];
        
        let scope = ScopeInferrer::infer(None, "proj-alpha-backup-daily", &tasks);
        assert_eq!(scope.worker_id(), "worker-project-alpha");
    }

    #[test]
    fn test_scope_infer_from_dag_id_user() {
        // Test dag_id pattern: user-{id}-*
        let tasks = vec![];
        
        let scope = ScopeInferrer::infer(None, "user-john-data-sync", &tasks);
        assert_eq!(scope.worker_id(), "worker-user-john");
    }

    #[test]
    fn test_scope_infer_from_dag_id_type() {
        // Test dag_id pattern: {type}-*
        let tasks = vec![];
        
        let scope = ScopeInferrer::infer(None, "deploy-production", &tasks);
        assert_eq!(scope.worker_id(), "worker-type-deploy");
    }

    #[test]
    fn test_scope_infer_default_global() {
        // Test default to Global when no pattern matches
        let tasks = vec![];
        
        let scope = ScopeInferrer::infer(None, "my-custom-dag", &tasks);
        assert_eq!(scope.worker_id(), "worker-global");
    }

    #[test]
    fn test_scope_infer_priority_order() {
        // Test priority: explicit > env > dag_id > default
        let tasks = vec![
            DagTaskSpec {
                id: "task1".to_string(),
                task_type: "shell".to_string(),
                command: "echo test".to_string(),
                depends_on: vec![],
                env: [("PROJECT_ID".to_string(), "env-proj".to_string())].into_iter().collect(),
            }
        ];
        
        // dag_id would infer "project-alpha", but env takes priority
        let scope = ScopeInferrer::infer(None, "proj-alpha-backup", &tasks);
        assert_eq!(scope.worker_id(), "worker-project-env-proj");
    }

    #[test]
    fn test_scope_conflict_detection() {
        // Test conflict detection: same worker, different target nodes
        let entries = vec![
            ("dag1".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, Some("node1".to_string())),
            ("dag2".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, Some("node2".to_string())),
        ];
        
        let conflicts = ScopeInferrer::detect_conflicts(&entries);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].worker_id, "worker-project-shared");
        assert!(conflicts[0].conflicting_nodes.contains(&"node1".to_string()));
        assert!(conflicts[0].conflicting_nodes.contains(&"node2".to_string()));
    }

    #[test]
    fn test_scope_no_conflict_same_node() {
        // Test no conflict when same worker, same target node
        let entries = vec![
            ("dag1".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, Some("node1".to_string())),
            ("dag2".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, Some("node1".to_string())),
        ];
        
        let conflicts = ScopeInferrer::detect_conflicts(&entries);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_scope_no_conflict_no_target() {
        // Test no conflict when target_node is None (any node)
        let entries = vec![
            ("dag1".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, None),
            ("dag2".to_string(), DagScope::Project { 
                project_id: "shared".to_string(), 
                force_new: false 
            }, None),
        ];
        
        let conflicts = ScopeInferrer::detect_conflicts(&entries);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_scope_validate_ok() {
        let entries = vec![
            ("dag1".to_string(), DagScope::Global, Some("node1".to_string())),
            ("dag2".to_string(), DagScope::Global, Some("node1".to_string())),
        ];
        
        assert!(ScopeInferrer::validate(&entries).is_ok());
    }

    #[test]
    fn test_scope_validate_error() {
        let entries = vec![
            ("dag1".to_string(), DagScope::Global, Some("node1".to_string())),
            ("dag2".to_string(), DagScope::Global, Some("node2".to_string())),
        ];
        
        let result = ScopeInferrer::validate(&entries);
        assert!(result.is_err());
        
        let conflicts = result.unwrap_err();
        assert_eq!(conflicts.len(), 1);
    }

    // ===== TODO List Tests =====

    #[test]
    fn test_todo_item_lifecycle() {
        let mut item = DagTodoItem::new("task-1".to_string(), "Execute task 1".to_string());
        
        assert!(item.is_pending());
        assert!(!item.is_completed());
        
        item.mark_in_progress();
        assert_eq!(item.status, TodoItemStatus::InProgress);
        
        item.mark_completed();
        assert!(item.is_completed());
        assert_eq!(item.status, TodoItemStatus::Completed);
        assert!(item.completed_at.is_some());
    }

    #[test]
    fn test_todo_list_basic() {
        let mut list = DagTodoList::new();
        
        list.add("item-1", "First task");
        list.add("item-2", "Second task");
        
        assert_eq!(list.items.len(), 2);
        
        let item = list.get("item-1").unwrap();
        assert_eq!(item.description, "First task");
        
        let pending = list.pending();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_todo_list_priority() {
        let mut list = DagTodoList::new();
        
        list.add_item(DagTodoItem::new("low".to_string(), "Low priority".to_string()).with_priority(1));
        list.add_item(DagTodoItem::new("high".to_string(), "High priority".to_string()).with_priority(10));
        list.add_item(DagTodoItem::new("medium".to_string(), "Medium priority".to_string()).with_priority(5));
        
        list.sort_by_priority();
        
        assert_eq!(list.items[0].id, "high");
        assert_eq!(list.items[1].id, "medium");
        assert_eq!(list.items[2].id, "low");
    }

    #[test]
    fn test_todo_list_checkpoint() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        
        list.checkpoint("Agent progress: 50% complete");
        
        assert!(list.last_checkpoint.is_some());
        assert_eq!(list.agent_notes, "Agent progress: 50% complete");
    }

    #[test]
    fn test_todo_list_completion_rate() {
        let mut list = DagTodoList::new();
        
        assert_eq!(list.completion_rate(), 1.0); // Empty = 100%
        
        list.add("task-1", "Task 1");
        list.add("task-2", "Task 2");
        
        assert_eq!(list.completion_rate(), 0.0); // None completed
        
        if let Some(item) = list.get_mut("task-1") {
            item.mark_completed();
        }
        
        assert_eq!(list.completion_rate(), 0.5); // 1 of 2 completed
    }

    #[test]
    fn test_dag_scope_force_new() {
        let scope = DagScope::Project {
            project_id: "test".to_string(),
            force_new: true,
        };
        
        assert!(scope.force_new_worker());
        assert!(scope.worker_key().starts_with("worker-project-test-new-"));
    }

    #[test]
    fn test_dag_scope_reuse_default() {
        let scope = DagScope::Project {
            project_id: "test".to_string(),
            force_new: false,
        };
        
        assert!(!scope.force_new_worker());
        assert_eq!(scope.worker_key(), "worker-project-test");
    }

    #[test]
    fn test_proposal_source_requires_review() {
        assert!(ProposalSource::RoomAgent.requires_review());
        assert!(ProposalSource::UserCLI.requires_review());
        assert!(ProposalSource::AutoSystem.requires_review());
        assert!(!ProposalSource::WorkerAgent.requires_review());
    }

    #[test]
    fn test_todo_list_proposal_creation() {
        let diff = TodoListDiff::default();
        let proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            diff,
            "Test proposal",
        );
        
        assert!(proposal.requires_review());
        assert_eq!(proposal.source, ProposalSource::RoomAgent);
        assert_eq!(proposal.reason, "Test proposal");
        assert!(!proposal.id.is_empty());
    }

    #[test]
    fn test_todo_list_submit_worker_proposal_auto_merge() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        
        // Create a proposal from Worker Agent (should auto-merge)
        let mut diff = TodoListDiff::default();
        diff.added.push(DagTodoItem::new("task-2".to_string(), "Task 2".to_string()));
        
        let proposal = TodoListProposal::new(
            ProposalSource::WorkerAgent,
            "worker-1",
            diff,
            "Adding task 2",
        );
        
        let proposal_id = proposal.id.clone();
        list.submit_proposal(proposal);
        
        // Worker proposal should be auto-merged
        assert!(list.get("task-2").is_some());
        assert!(list.proposal_history.iter().any(|r| r.proposal_id() == proposal_id));
    }

    #[test]
    fn test_todo_list_submit_external_proposal_needs_review() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        
        // Create a proposal from Room Agent (needs review)
        let mut diff = TodoListDiff::default();
        diff.added.push(DagTodoItem::new("task-2".to_string(), "Task 2".to_string()));
        
        let proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            diff,
            "Adding task 2",
        );
        
        let proposal_id = proposal.id.clone();
        list.submit_proposal(proposal);
        
        // External proposal should not be merged yet
        assert!(list.get("task-2").is_none());
        assert!(list.pending_review().iter().any(|p| p.id == proposal_id));
    }

    #[test]
    fn test_todo_list_review_and_accept() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        
        // Submit external proposal
        let mut diff = TodoListDiff::default();
        diff.added.push(DagTodoItem::new("task-2".to_string(), "Task 2".to_string()));
        
        let proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            diff,
            "Adding task 2",
        );
        
        let proposal_id = proposal.id.clone();
        list.submit_proposal(proposal);
        
        // Review and accept
        let result = list.review_and_merge(&proposal_id, |_, _| true);
        
        assert!(result.is_accepted());
        assert!(list.get("task-2").is_some());
        assert!(list.pending_review().is_empty());
    }

    #[test]
    fn test_todo_list_review_and_reject() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        
        // Submit external proposal
        let mut diff = TodoListDiff::default();
        diff.added.push(DagTodoItem::new("task-2".to_string(), "Task 2".to_string()));
        
        let proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            diff,
            "Adding task 2",
        );
        
        let proposal_id = proposal.id.clone();
        list.submit_proposal(proposal);
        
        // Review and reject
        let result = list.review_and_merge(&proposal_id, |_, _| false);
        
        assert!(!result.is_accepted());
        assert!(list.get("task-2").is_none());
        assert!(list.pending_review().is_empty());
    }

    #[test]
    fn test_todo_list_auto_merge_safe_priority_change() {
        let mut list = DagTodoList::new();
        list.add("task-1", "Task 1");
        if let Some(item) = list.get_mut("task-1") {
            item.priority = 1;
        }
        
        // Create a priority-only change (safe)
        let change = TodoItemChange {
            id: "task-1".to_string(),
            old_status: TodoItemStatus::Pending,
            new_status: TodoItemStatus::Pending,
            old_priority: 1,
            new_priority: 5,
            old_description: "Task 1".to_string(),
            new_description: "Task 1".to_string(),
        };
        
        let diff = TodoListDiff {
            added: vec![],
            removed: vec![],
            modified: vec![change],
        };
        
        let proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            diff,
            "Increasing priority",
        );
        
        list.submit_proposal(proposal);
        
        // Auto-merge safe proposals
        let results = list.auto_merge_safe_proposals();
        
        assert_eq!(results.len(), 1);
        assert!(results[0].is_accepted());
        assert_eq!(list.get("task-1").unwrap().priority, 5);
    }

    #[test]
    fn test_todo_list_cleanup_expired_proposals() {
        let mut list = DagTodoList::new();
        
        // Create an expired proposal
        let mut proposal = TodoListProposal::new(
            ProposalSource::RoomAgent,
            "room-agent-1",
            TodoListDiff::default(),
            "Expired proposal",
        );
        proposal.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
        
        let proposal_id = proposal.id.clone();
        list.pending_proposals.push(proposal);
        
        // Clean up expired
        let count = list.cleanup_expired_proposals();
        
        assert_eq!(count, 1);
        assert!(list.pending_review().is_empty());
        assert!(list.proposal_history.iter().any(|r| 
            matches!(r, ProposalResult::Expired { proposal_id: id } if id == &proposal_id)
        ));
    }
}

/// From conversion implementations

impl From<DagTask> for DagNode {
    fn from(task: DagTask) -> Self {
        Self {
            task_id: task.task_id,
            dependencies: task.dependencies,
            dependents: Vec::new(),
            status: DagNodeStatus::Pending,
            level: task.level,
            rollback: None,
            agent_runtime: task.agent_runtime,
            reuse_agent: task.reuse_agent,
            keep_agent: task.keep_agent,
            agent_config: task.agent_config,
        }
    }
}
