use async_trait::async_trait;
use cis_types::{Task, TaskResult, TaskId, TaskStatus};
use cis_traits::scheduler::{DagScheduler, Dag, TaskNode, DagExecutionResult, ExecutionStatus};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::sync::{Arc, RwLock};
use chrono::Utc;
use super::error::SchedulerError;
use super::dag::TaskGraph;

pub struct DagSchedulerImpl {
    name: String,
    executions: RwLock<HashMap<String, DagExecutionResult>>,
}

impl DagSchedulerImpl {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            executions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl DagScheduler for DagSchedulerImpl {
    fn name(&self) -> &str {
        &self.name
    }

    async fn build_dag(&mut self, tasks: Vec<Task>) -> Result<Dag, Box<dyn Error + Send + Sync>> {
        let graph = TaskGraph::from_tasks(tasks)?;
        Ok(Dag {
            tasks: graph.into_nodes(),
            edges: graph.into_edges(),
        })
    }

    async fn validate_dag(&self, dag: &Dag) -> Result<(), Box<dyn Error + Send + Sync>> {
        let graph = TaskGraph::from_dag(dag)?;
        graph.validate()?;
        Ok(())
    }

    async fn execute_dag(&self, dag: Dag) -> Result<DagExecutionResult, Box<dyn Error + Send + Sync>> {
        let execution_id = format!("exec-{}", Utc::now().timestamp_millis());
        
        let mut results = HashMap::new();
        let graph = TaskGraph::from_dag(&dag)?;
        let sorted = graph.topological_sort()?;

        let completed = RwLock::new(Vec::new());
        
        for task_id in sorted {
            let task = dag.tasks.iter().find(|t| t.id == task_id)
                .ok_or_else(|| Box::new(SchedulerError::TaskNotFound(task_id.clone())) as Box<dyn Error + Send + Sync>);
            
            if let Ok(t) = task {
                let result = TaskResult {
                    task_id: t.id.clone(),
                    success: true,
                    output: None,
                    error: None,
                    duration_ms: 0,
                    completed_at: Utc::now(),
                };
                results.insert(task_id, result);
            }
        }

        let status = ExecutionStatus::Completed;
        
        let exec_result = DagExecutionResult {
            execution_id: execution_id.clone(),
            results: results.clone(),
            status: status.clone(),
        };
        
        let mut executions = self.executions.write().unwrap();
        executions.insert(execution_id, exec_result.clone());
        
        Ok(exec_result)
    }

    async fn cancel_execution(&self, execution_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let mut executions = self.executions.write().unwrap();
        if let Some(result) = executions.get_mut(execution_id) {
            result.status = ExecutionStatus::Cancelled;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn get_execution_status(&self, execution_id: &str) -> Result<ExecutionStatus, Box<dyn Error + Send + Sync>> {
        let executions = self.executions.read().unwrap();
        Ok(executions.get(execution_id)
            .map(|r| r.status.clone())
            .unwrap_or(ExecutionStatus::Pending))
    }
}
