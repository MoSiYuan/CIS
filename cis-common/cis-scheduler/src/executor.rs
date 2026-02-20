use async_trait::async_trait;
use cis_types::{Task, TaskResult, TaskStatus};
use cis_traits::scheduler::TaskExecutor;
use chrono::Utc;
use std::error::Error;

pub struct SimpleExecutor;

impl SimpleExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SimpleExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskExecutor for SimpleExecutor {
    async fn execute_task(&self, task: &Task) -> Result<TaskResult, Box<dyn Error + Send + Sync>> {
        let start = std::time::Instant::now();
        
        Ok(TaskResult {
            task_id: task.id.clone(),
            success: true,
            output: Some(format!("Task {} executed", task.title)),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            completed_at: Utc::now(),
        })
    }

    async fn cancel_task(&self, task_id: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(true)
    }

    fn can_handle(&self, task_type: &str) -> bool {
        true
    }
}
