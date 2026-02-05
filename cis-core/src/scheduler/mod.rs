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

pub mod local_executor;
pub mod persistence;
pub mod skill_executor;

pub use local_executor::{LocalExecutor, WorkerInfo, WorkerSummary, ExecutorStats};
pub use persistence::DagPersistence;
pub use skill_executor::SkillDagExecutor;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DagScope {
    /// Global scope - shared worker for all DAGs
    Global,
    
    /// Project scope - isolated worker per project
    Project { 
        project_id: String,
        /// Whether to reuse existing worker
        #[serde(default = "default_reuse")]
        reuse_worker: bool,
    },
    
    /// User scope - isolated worker per user
    User { 
        user_id: String,
    },
    
    /// Type scope - isolated worker per DAG type (backup/deploy/test)
    Type { 
        dag_type: String,
    },
}

fn default_reuse() -> bool {
    true
}

impl DagScope {
    /// Generate worker identifier from scope
    pub fn worker_id(&self) -> String {
        match self {
            DagScope::Global => "worker-global".to_string(),
            DagScope::Project { project_id, .. } => format!("worker-project-{}", project_id),
            DagScope::User { user_id } => format!("worker-user-{}", user_id),
            DagScope::Type { dag_type } => format!("worker-type-{}", dag_type),
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
                    reuse_worker: true,
                };
            }
            if parts[0] == "user" {
                return DagScope::User { 
                    user_id: parts[1].to_string(),
                };
            }
            // Common DAG types
            let dag_types = ["backup", "deploy", "test", "build", "sync"];
            if dag_types.contains(&parts[0]) {
                return DagScope::Type { 
                    dag_type: parts[0].to_string(),
                };
            }
        }
        
        // 2. Try to extract from task environment variables
        for task in tasks {
            if let Some(project_id) = task.env.get("PROJECT_ID") {
                return DagScope::Project { 
                    project_id: project_id.clone(),
                    reuse_worker: true,
                };
            }
            if let Some(user_id) = task.env.get("USER_ID") {
                return DagScope::User { 
                    user_id: user_id.clone(),
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
                        reuse_worker: true,
                    });
                }
                "user" => {
                    return Some(DagScope::User { 
                        user_id: parts[1].to_string(),
                    });
                }
                "backup" | "deploy" | "test" | "build" | "sync" => {
                    return Some(DagScope::Type { 
                        dag_type: parts[0].to_string(),
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
            DagScope::User { user_id } => ("User".to_string(), Some(user_id.clone())),
            DagScope::Type { dag_type } => ("Type".to_string(), Some(dag_type.clone())),
        }
    }
    
    /// Restore from database fields
    pub fn from_db_fields(scope_type: &str, scope_id: Option<&str>) -> Self {
        match scope_type {
            "Project" => DagScope::Project { 
                project_id: scope_id.unwrap_or("default").to_string(),
                reuse_worker: true,
            },
            "User" => DagScope::User { 
                user_id: scope_id.unwrap_or("unknown").to_string(),
            },
            "Type" => DagScope::Type { 
                dag_type: scope_id.unwrap_or("default").to_string(),
            },
            _ => DagScope::Global,
        }
    }
}

impl Default for DagScope {
    fn default() -> Self {
        DagScope::Global
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
}

fn default_version() -> i64 {
    1
}

impl DagSpec {
    /// Create new DAG specification
    pub fn new(dag_id: String, tasks: Vec<DagTaskSpec>) -> Self {
        let scope = DagScope::infer_from_dag(&dag_id, &tasks);
        
        Self {
            dag_id,
            description: String::new(),
            tasks,
            target_node: None,
            scope,
            priority: crate::types::TaskPriority::Medium,
            schedule: None,
            version: 1,
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

/// DAG run status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        }
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
}

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

    pub fn get_run(&self, run_id: &str) -> Option<&DagRun> {
        self.runs.get(run_id)
    }

    pub fn get_run_mut(&mut self, run_id: &str) -> Option<&mut DagRun> {
        self.runs.get_mut(run_id)
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
}
