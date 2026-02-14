//! # 任务执行器
//!
//! 定义统一的任务执行接口，支持多种执行策略。

pub mod sync;
pub mod parallel;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{Task, TaskStatus};

pub use sync::SyncExecutor;
pub use parallel::ParallelExecutor;

/// 任务执行结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    /// 任务 ID
    pub task_id: String,
    /// 执行状态
    pub status: TaskStatus,
    /// 输出
    pub output: serde_json::Value,
    /// 执行时长（秒）
    pub duration_secs: f64,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(task_id: String, output: serde_json::Value, duration_secs: f64) -> Self {
        Self {
            task_id,
            status: TaskStatus::Completed,
            output,
            duration_secs,
            error: None,
        }
    }

    /// 创建失败结果
    pub fn failure(task_id: String, error: String, duration_secs: f64) -> Self {
        Self {
            task_id,
            status: TaskStatus::Failed,
            output: serde_json::Value::Null,
            duration_secs,
            error: Some(error),
        }
    }

    /// 判断是否成功
    pub fn is_success(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    /// 判断是否失败
    pub fn is_failure(&self) -> bool {
        matches!(self.status, TaskStatus::Failed)
    }
}

/// 任务执行器 Trait
///
/// 定义统一的任务执行接口。
#[async_trait]
pub trait Executor: Send + Sync {
    /// 执行单个任务
    async fn execute(&self, task: Task) -> Result<ExecutionResult>;

    /// 批量执行任务（可并行）
    ///
    /// 默认实现为顺序执行，并行执行器可覆盖此方法。
    async fn execute_batch(&self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();
        for task in tasks {
            let result = self.execute(task).await?;
            results.push(result);
        }
        Ok(results)
    }

    /// 获取执行器名称
    fn name(&self) -> &str {
        "executor"
    }

    /// 获取执行器统计信息
    fn stats(&self) -> ExecutorStats {
        ExecutorStats::default()
    }
}

/// 执行器统计信息
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ExecutorStats {
    /// 执行器名称
    pub name: String,
    /// 总执行任务数
    pub total_executed: usize,
    /// 成功任务数
    pub succeeded: usize,
    /// 失败任务数
    pub failed: usize,
    /// 平均执行时长（秒）
    pub avg_duration_secs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskLevel};

    fn create_test_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            title: format!("Task {}", id),
            description: None,
            status: TaskStatus::Pending,
            priority: crate::types::TaskPriority::Medium,
            level: TaskLevel::mechanical_default(),
            group: "test".to_string(),
            skill: None,
            input: serde_json::Value::Null,
            output: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            dependencies: vec![],
        }
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult::success("task-1".to_string(), serde_json::json!("output"), 1.5);
        assert!(result.is_success());
        assert!(!result.is_failure());
        assert_eq!(result.task_id, "task-1");
        assert_eq!(result.duration_secs, 1.5);
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult::failure("task-1".to_string(), "Error".to_string(), 0.5);
        assert!(!result.is_success());
        assert!(result.is_failure());
        assert_eq!(result.error, Some("Error".to_string()));
    }
}
