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
        }
    }
}

impl std::error::Error for DagError {}

/// DAG node status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        }
    }
}

/// DAG task node
#[derive(Debug, Clone)]
pub struct DagNode {
    /// Task ID
    pub task_id: String,
    /// List of dependency task_ids
    pub dependencies: Vec<String>,
    /// List of task_ids that depend on this task
    pub dependents: Vec<String>,
    /// Node status
    pub status: DagNodeStatus,
}

impl DagNode {
    /// Create new DAG node
    pub fn new(task_id: String, dependencies: Vec<String>) -> Self {
        Self {
            task_id,
            dependencies,
            dependents: Vec::new(),
            status: DagNodeStatus::Pending,
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
}

/// DAG graph structure
#[derive(Debug, Clone)]
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
                task_id, node.status as i32
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
}

impl Default for TaskDag {
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
