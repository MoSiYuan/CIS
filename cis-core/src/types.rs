//! # CIS Core Types
//!
//! Core data structures and domain types for CIS.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a task
pub type TaskId = String;

/// Unique identifier for a node (DID in the future)
pub type NodeId = String;

/// Task status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task is blocked (dependencies not met)
    Blocked,
    /// Task was cancelled
    Cancelled,
}

/// Task execution level - four-tier decision mechanism
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskLevel {
    /// Auto-execute, retry on failure
    Mechanical { retry: u8 },
    /// Countdown execution, can intervene
    Recommended { default_action: Action, timeout_secs: u16 },
    /// Modal confirmation, must manually confirm
    Confirmed,
    /// Pause DAG, wait for arbitration
    Arbitrated { stakeholders: Vec<String> },
}

/// Action for Recommended level default
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    Execute,
    Skip,
    Abort,
}

/// Failure type for debt mechanism
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FailureType {
    /// Ignorable debt, continue downstream
    Ignorable,
    /// Blocking debt, freeze DAG
    Blocking,
}

/// Ambiguity handling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AmbiguityPolicy {
    AutoBest,
    Suggest { default: Action, timeout_secs: u16 },
    Ask,
    Escalate,
}

impl TaskStatus {
    /// Returns true if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns true if the task can transition to running
    pub fn can_run(&self) -> bool {
        matches!(self, Self::Pending | Self::Blocked)
    }
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Default)]
pub enum TaskPriority {
    /// Urgent priority
    Urgent = 4,
    /// High priority
    High = 3,
    /// Medium priority
    #[default]
    Medium = 2,
    /// Low priority
    Low = 1,
}

/// Core task structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID
    pub id: TaskId,

    /// Optional parent task ID
    pub parent_id: Option<TaskId>,

    /// Task title
    pub title: String,

    /// Detailed description
    pub description: Option<String>,

    /// Task group/category
    pub group_name: String,

    /// Completion criteria
    pub completion_criteria: Option<String>,

    /// Current status
    pub status: TaskStatus,

    /// Task priority
    pub priority: TaskPriority,

    /// Task dependencies (task IDs that must complete first)
    pub dependencies: Vec<TaskId>,

    /// Task result (if completed)
    pub result: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Workspace directory for the task
    pub workspace_dir: Option<String>,

    /// Whether the task is sandboxed
    pub sandboxed: bool,

    /// Whether network access is allowed
    pub allow_network: bool,

    /// Task creation timestamp
    pub created_at: DateTime<Utc>,

    /// Task start timestamp (if started)
    pub started_at: Option<DateTime<Utc>>,

    /// Task completion timestamp (if completed)
    pub completed_at: Option<DateTime<Utc>>,

    /// Node ID currently executing the task
    pub node_id: Option<NodeId>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Task execution level - four-tier decision mechanism
    pub level: TaskLevel,

    /// Ambiguity handling policy
    pub on_ambiguity: AmbiguityPolicy,

    /// Input file paths list
    pub inputs: Vec<String>,

    /// Expected outputs
    pub outputs: Vec<String>,

    /// Rollback commands (if task fails)
    pub rollback: Option<Vec<String>>,

    /// Idempotency flag - true if task can be safely retried
    pub idempotent: bool,

    /// Failure type for debt mechanism
    pub failure_type: Option<FailureType>,

    /// 关联的 Skill ID（可选）
    /// 如果设置，表示此任务通过 DAG Scheduler 调用该 Skill
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub skill_id: Option<String>,

    /// Skill 执行参数（JSON 格式）
    /// 传递给 Skill 的输入参数
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub skill_params: Option<serde_json::Value>,

    /// Skill 执行结果（JSON 格式）
    /// 任务执行成功后，Skill 的输出结果
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub skill_result: Option<serde_json::Value>,
}

impl Task {
    /// Create a new task with minimal required fields
    pub fn new(id: TaskId, title: String, group_name: String) -> Self {
        Self {
            id,
            parent_id: None,
            title,
            description: None,
            group_name,
            completion_criteria: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::default(),
            dependencies: Vec::new(),
            result: None,
            error: None,
            workspace_dir: None,
            sandboxed: true,
            allow_network: false,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            node_id: None,
            metadata: HashMap::new(),
            level: TaskLevel::Mechanical { retry: 3 },
            on_ambiguity: AmbiguityPolicy::AutoBest,
            inputs: Vec::new(),
            outputs: Vec::new(),
            rollback: None,
            idempotent: false,
            failure_type: None,
            skill_id: None,
            skill_params: None,
            skill_result: None,
        }
    }

    /// Returns true if all dependencies are satisfied
    pub fn dependencies_satisfied(&self, completed_tasks: &[TaskId]) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }

    /// 创建调用指定 Skill 的任务
    pub fn for_skill(skill_id: impl Into<String>) -> Self {
        let skill_id_str = skill_id.into();
        Self::new(
            format!("skill-{}", skill_id_str),
            format!("Execute skill {}", skill_id_str),
            "skill".to_string()
        ).with_skill(skill_id_str)
    }

    /// 设置要调用的 Skill
    pub fn with_skill(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }

    /// 设置 Skill 参数
    pub fn with_skill_params(mut self, params: impl Serialize) -> Self {
        self.skill_params = serde_json::to_value(params).ok();
        self
    }

    /// 设置 Skill 执行结果
    pub fn with_skill_result(mut self, result: impl Serialize) -> Self {
        self.skill_result = serde_json::to_value(result).ok();
        self
    }

    /// 检查此任务是否关联 Skill
    pub fn is_skill_task(&self) -> bool {
        self.skill_id.is_some()
    }

    /// 获取 Skill ID（如果存在）
    pub fn skill_id(&self) -> Option<&str> {
        self.skill_id.as_deref()
    }
}

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub task_id: TaskId,

    /// Whether the task succeeded
    pub success: bool,

    /// Result output
    pub output: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Timestamp of completion
    pub completed_at: DateTime<Utc>,
}

/// Memory entry categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemoryCategory {
    /// Execution records
    Execution,
    /// Result data
    Result,
    /// Error information
    Error,
    /// Context information
    Context,
    /// Skill experience
    Skill,
}

/// Memory domain (public/private)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemoryDomain {
    /// Private encrypted memory
    Private,
    /// Public shared memory
    Public,
}

/// Debt entry for failure accumulation mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtEntry {
    /// Task ID that caused the debt
    pub task_id: TaskId,
    /// DAG run ID
    pub dag_run_id: String,
    /// Type of failure
    pub failure_type: FailureType,
    /// Error message
    pub error_message: String,
    /// When the debt was created
    pub created_at: DateTime<Utc>,
    /// Whether the debt has been resolved
    pub resolved: bool,
}

/// Skill 任务 = 关联了 Skill 的 Task
pub type SkillTask = Task;

/// Skill 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 输出结果
    pub output: Option<serde_json::Value>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
    }

    #[test]
    fn test_task_dependencies() {
        let task = Task::new("task-1".into(), "Test".into(), "test".into());
        assert!(task.dependencies_satisfied(&[]));
        assert!(task.dependencies_satisfied(&["task-2".into()]));
    }
}
