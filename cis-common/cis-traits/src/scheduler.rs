use async_trait::async_trait;
use cis_types::{Task, TaskResult, TaskLevel, TaskId};
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct Dag {
    pub tasks: Vec<TaskNode>,
    pub edges: Vec<(TaskId, TaskId)>,
}

#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: TaskId,
    pub task: Task,
    pub dependencies: Vec<TaskId>,
}

#[derive(Debug, Clone)]
pub struct DagExecutionResult {
    pub execution_id: String,
    pub results: HashMap<TaskId, TaskResult>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[async_trait]
pub trait DagScheduler: Send + Sync {
    fn name(&self) -> &str;

    async fn build_dag(&mut self, tasks: Vec<Task>) -> Result<Dag, Box<dyn Error + Send + Sync>>;
    async fn validate_dag(&self, dag: &Dag) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn execute_dag(&self, dag: Dag) -> Result<DagExecutionResult, Box<dyn Error + Send + Sync>>;
    async fn cancel_execution(&self, execution_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;

    async fn get_execution_status(&self, execution_id: &str) -> Result<ExecutionStatus, Box<dyn Error + Send + Sync>>;
}

#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, task: &Task) -> Result<TaskResult, Box<dyn Error + Send + Sync>>;
    async fn cancel_task(&self, task_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;
    fn can_handle(&self, task_type: &str) -> bool;
}
