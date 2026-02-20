use std::error::Error;

#[derive(Debug)]
pub enum SchedulerError {
    InvalidTask(String),
    CyclicDependency,
    TaskNotFound(String),
    ExecutionFailed(String),
    Cancelled,
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchedulerError::InvalidTask(e) => write!(f, "Invalid task: {}", e),
            SchedulerError::CyclicDependency => write!(f, "Cyclic dependency detected"),
            SchedulerError::TaskNotFound(e) => write!(f, "Task not found: {}", e),
            SchedulerError::ExecutionFailed(e) => write!(f, "Execution failed: {}", e),
            SchedulerError::Cancelled => write!(f, "Execution cancelled"),
        }
    }
}

impl Error for SchedulerError {}
