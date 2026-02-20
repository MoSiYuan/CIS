use crate::task::{FailureType, TaskId};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtEntry {
    pub task_id: TaskId,
    pub dag_run_id: String,
    pub failure_type: FailureType,
    pub error_message: String,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
}
