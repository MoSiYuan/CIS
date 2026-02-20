use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type TaskId = String;
pub type NodeId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TaskLevel {
    Mechanical {
        retry: u8,
    },
    Recommended {
        default_action: Action,
        timeout_secs: u16,
    },
    Confirmed,
    Arbitrated {
        stakeholders: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Action {
    Execute,
    Skip,
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum FailureType {
    Ignorable,
    Blocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum AmbiguityPolicy {
    AutoBest,
    Suggest { default: Action, timeout_secs: u16 },
    Ask,
    Escalate,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_run(&self) -> bool {
        matches!(self, Self::Pending | Self::Blocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Default)]
pub enum TaskPriority {
    Urgent = 4,
    High = 3,
    #[default]
    Medium = 2,
    Low = 1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub description: Option<String>,
    pub group_name: String,
    pub completion_criteria: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub dependencies: Vec<TaskId>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub workspace_dir: Option<String>,
    pub sandboxed: bool,
    pub allow_network: bool,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub node_id: Option<NodeId>,
    pub metadata: HashMap<String, String>,
    pub level: TaskLevel,
    pub on_ambiguity: AmbiguityPolicy,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub rollback: Option<Vec<String>>,
    pub idempotent: bool,
    pub failure_type: Option<FailureType>,
    pub skill_id: Option<String>,
    pub skill_params: Option<serde_json::Value>,
    pub skill_result: Option<serde_json::Value>,
}

impl Task {
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

    pub fn dependencies_satisfied(&self, completed_tasks: &[TaskId]) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }

    pub fn for_skill(skill_id: impl Into<String>) -> Self {
        let skill_id_str = skill_id.into();
        Self::new(
            format!("skill-{}", skill_id_str),
            format!("Execute skill {}", skill_id_str),
            "skill".to_string(),
        )
        .with_skill(skill_id_str)
    }

    pub fn with_skill(mut self, skill_id: impl Into<String>) -> Self {
        self.skill_id = Some(skill_id.into());
        self
    }

    pub fn with_skill_params(mut self, params: impl Serialize) -> Self {
        self.skill_params = serde_json::to_value(params).ok();
        self
    }

    pub fn with_skill_result(mut self, result: impl Serialize) -> Self {
        self.skill_result = serde_json::to_value(result).ok();
        self
    }

    pub fn is_skill_task(&self) -> bool {
        self.skill_id.is_some()
    }

    pub fn skill_id(&self) -> Option<&str> {
        self.skill_id.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: TaskId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub completed_at: DateTime<Utc>,
}
