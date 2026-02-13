//! # 任务数据模型
//!
//! 定义任务相关的所有数据结构。

use rusqlite::{types::FromSql, types::FromSqlResult, types::ToSql, types::ToSqlOutput};
use serde::{Deserialize, Serialize};

/// 任务实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntity {
    pub id: i64,
    pub task_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub prompt_template: String,
    #[serde(rename = "context_variables")]
    pub context_variables: serde_json::Value,
    pub description: Option<String>,
    pub estimated_effort_days: Option<f64>,
    #[serde(rename = "dependencies")]
    pub dependencies: Vec<String>,
    #[serde(rename = "engine_type")]
    pub engine_type: Option<String>,
    #[serde(rename = "engine_context_id")]
    pub engine_context_id: Option<i64>,
    pub status: TaskStatus,
    pub assigned_team_id: Option<String>,
    pub assigned_agent_id: Option<i64>,
    pub assigned_at: Option<i64>,
    pub result: Option<TaskResult>,
    pub error_message: Option<String>,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_seconds: Option<f64>,
    pub metadata: Option<serde_json::Value>,
    #[serde(rename = "created_at")]
    pub created_at_ts: i64,
    #[serde(rename = "updated_at")]
    pub updated_at_ts: i64,
}

impl TaskEntity {
    /// 获取创建时间
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(self.created_at_ts, 0).unwrap_or_default()
    }

    /// 获取更新时间
    pub fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(self.updated_at_ts, 0).unwrap_or_default()
    }
}

/// 任务类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskType {
    #[serde(rename = "module_refactoring")]
    ModuleRefactoring,
    #[serde(rename = "engine_code_injection")]
    EngineCodeInjection,
    #[serde(rename = "performance_optimization")]
    PerformanceOptimization,
    #[serde(rename = "code_review")]
    CodeReview,
    #[serde(rename = "test_writing")]
    TestWriting,
    #[serde(rename = "documentation")]
    Documentation,
}

impl ToSql for TaskType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            TaskType::ModuleRefactoring => "module_refactoring",
            TaskType::EngineCodeInjection => "engine_code_injection",
            TaskType::PerformanceOptimization => "performance_optimization",
            TaskType::CodeReview => "code_review",
            TaskType::TestWriting => "test_writing",
            TaskType::Documentation => "documentation",
        };
        Ok(ToSqlOutput::from(s.to_string()))
    }
}

impl FromSql for TaskType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "module_refactoring" => Ok(TaskType::ModuleRefactoring),
            "engine_code_injection" => Ok(TaskType::EngineCodeInjection),
            "performance_optimization" => Ok(TaskType::PerformanceOptimization),
            "code_review" => Ok(TaskType::CodeReview),
            "test_writing" => Ok(TaskType::TestWriting),
            "documentation" => Ok(TaskType::Documentation),
            _ => Err(rusqlite::types::FromSqlError::Other(Box::new(format!(
                "Unknown task type: {}",
                s
            )))),
        })
    }
}

/// 任务优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Copy)]
pub enum TaskPriority {
    #[serde(rename = "0")]
    P0,
    #[serde(rename = "1")]
    P1,
    #[serde(rename = "2")]
    P2,
    #[serde(rename = "3")]
    P3,
}

impl TaskPriority {
    /// 获取优先级数值
    pub fn value(&self) -> i32 {
        match self {
            TaskPriority::P0 => 0,
            TaskPriority::P1 => 1,
            TaskPriority::P2 => 2,
            TaskPriority::P3 => 3,
        }
    }

    /// 从数值创建优先级
    pub fn from_value(value: i32) -> Option<Self> {
        match value {
            0 => Some(TaskPriority::P0),
            1 => Some(TaskPriority::P1),
            2 => Some(TaskPriority::P2),
            3 => Some(TaskPriority::P3),
            _ => None,
        }
    }
}

impl ToSql for TaskPriority {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.value()))
    }
}

impl FromSql for TaskPriority {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64().and_then(|v| {
            TaskPriority::from_value(v as i32)
                .ok_or_else(|| rusqlite::types::FromSqlError::Other(Box::new(format!("Invalid priority: {}", v))))
        })
    }
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "assigned")]
    Assigned,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl ToSql for TaskStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Assigned => "assigned",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        };
        Ok(ToSqlOutput::from(s.to_string()))
    }
}

impl FromSql for TaskStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "pending" => Ok(TaskStatus::Pending),
            "assigned" => Ok(TaskStatus::Assigned),
            "running" => Ok(TaskStatus::Running),
            "completed" => Ok(TaskStatus::Completed),
            "failed" => Ok(TaskStatus::Failed),
            _ => Err(rusqlite::types::FromSqlError::Other(Box::new(format!(
                "Unknown task status: {}",
                s
            )))),
        })
    }
}

/// 任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub success: bool,
    pub output: Option<String>,
    pub artifacts: Vec<String>,
    pub exit_code: Option<i32>,
}

/// 任务查询过滤器
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<Vec<TaskStatus>>,
    pub task_types: Option<Vec<TaskType>>,
    pub min_priority: Option<TaskPriority>,
    pub max_priority: Option<TaskPriority>,
    pub assigned_team: Option<String>,
    pub engine_type: Option<String>,
    pub sort_by: TaskSortBy,
    pub sort_order: TaskSortOrder,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 排序字段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSortBy {
    Priority,
    CreatedAt,
    UpdatedAt,
    Name,
    EstimatedEffort,
}

impl TaskSortBy {
    pub fn as_str(&self) -> &str {
        match self {
            TaskSortBy::Priority => "priority",
            TaskSortBy::CreatedAt => "created_at",
            TaskSortBy::UpdatedAt => "updated_at",
            TaskSortBy::Name => "name",
            TaskSortBy::EstimatedEffort => "estimated_effort_days",
        }
    }
}

/// 排序方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSortOrder {
    Asc,
    Desc,
}

impl TaskSortOrder {
    pub fn as_str(&self) -> &str {
        match self {
            TaskSortOrder::Asc => "ASC",
            TaskSortOrder::Desc => "DESC",
        }
    }
}

/// Agent Session 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionEntity {
    pub id: i64,
    pub session_id: String,
    pub agent_id: i64,
    pub runtime_type: String,
    pub status: SessionStatus,
    pub context_capacity: i64,
    pub context_used: i64,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: i64,
}

/// Session 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "released")]
    Released,
}

impl ToSql for SessionStatus {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            SessionStatus::Active => "active",
            SessionStatus::Idle => "idle",
            SessionStatus::Expired => "expired",
            SessionStatus::Released => "released",
        };
        Ok(ToSqlOutput::from(s.to_string()))
    }
}

impl FromSql for SessionStatus {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "active" => Ok(SessionStatus::Active),
            "idle" => Ok(SessionStatus::Idle),
            "expired" => Ok(SessionStatus::Expired),
            "released" => Ok(SessionStatus::Released),
            _ => Err(rusqlite::types::FromSqlError::Other(Box::new(format!(
                "Unknown session status: {}",
                s
            )))),
        })
    }
}

/// Agent 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntity {
    pub id: i64,
    pub agent_type: String,
    pub display_name: String,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub capabilities: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 任务执行日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionLog {
    pub id: i64,
    pub task_id: i64,
    pub session_id: i64,
    pub stage: ExecutionStage,
    pub log_level: LogLevel,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub duration_ms: Option<i64>,
    pub tokens_used: Option<i64>,
    pub timestamp: i64,
}

/// 执行阶段
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStage {
    #[serde(rename = "preparing")]
    Preparing,
    #[serde(rename = "executing")]
    Executing,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

impl ToSql for ExecutionStage {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            ExecutionStage::Preparing => "preparing",
            ExecutionStage::Executing => "executing",
            ExecutionStage::Completed => "completed",
            ExecutionStage::Failed => "failed",
        };
        Ok(ToSqlOutput::from(s.to_string()))
    }
}

impl FromSql for ExecutionStage {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "preparing" => Ok(ExecutionStage::Preparing),
            "executing" => Ok(ExecutionStage::Executing),
            "completed" => Ok(ExecutionStage::Completed),
            "failed" => Ok(ExecutionStage::Failed),
            _ => Err(rusqlite::types::FromSqlError::Other(Box::new(format!(
                "Unknown execution stage: {}",
                s
            )))),
        })
    }
}

/// 日志级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogLevel {
    #[serde(rename = "DEBUG")]
    Debug,
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

impl ToSql for LogLevel {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let s = match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        };
        Ok(ToSqlOutput::from(s.to_string()))
    }
}

impl FromSql for LogLevel {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_str().and_then(|s| match s {
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(rusqlite::types::FromSqlError::Other(Box::new(format!(
                "Unknown log level: {}",
                s
            )))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::P0 < TaskPriority::P1);
        assert!(TaskPriority::P1 < TaskPriority::P2);
        assert!(TaskPriority::P2 < TaskPriority::P3);
    }

    #[test]
    fn test_task_priority_value() {
        assert_eq!(TaskPriority::P0.value(), 0);
        assert_eq!(TaskPriority::P1.value(), 1);
        assert_eq!(TaskPriority::P2.value(), 2);
        assert_eq!(TaskPriority::P3.value(), 3);
    }

    #[test]
    fn test_task_type_serialize() {
        let types = vec![
            TaskType::ModuleRefactoring,
            TaskType::EngineCodeInjection,
            TaskType::PerformanceOptimization,
            TaskType::CodeReview,
            TaskType::TestWriting,
            TaskType::Documentation,
        ];

        for task_type in types {
            let json = serde_json::to_string(&task_type).unwrap();
            let deserialized: TaskType = serde_json::from_str(&json).unwrap();
            assert_eq!(task_type, deserialized);
        }
    }
}
