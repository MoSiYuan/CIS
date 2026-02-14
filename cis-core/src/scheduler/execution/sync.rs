//! # 同步执行器
//!
//! 单线程顺序执行任务。
//!
//! ## 适用场景
//! - 测试环境
//! - 单机任务
//! - 需要严格顺序执行的任务

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::{Executor, ExecutionResult, ExecutorStats};
use crate::error::{CisError, Result};
use crate::types::Task;

/// 同步执行器
///
/// 单线程顺序执行任务，记录统计信息。
pub struct SyncExecutor {
    /// 执行统计
    stats: Arc<Mutex<ExecutorStats>>,
}

impl SyncExecutor {
    /// 创建新的同步执行器
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(ExecutorStats {
                name: "sync".to_string(),
                ..Default::default()
            })),
        }
    }

    /// 更新统计信息
    async fn update_stats(&self, duration_secs: f64, success: bool) {
        let mut stats = self.stats.lock().await;
        stats.total_executed += 1;
        if success {
            stats.succeeded += 1;
        } else {
            stats.failed += 1;
        }

        // 更新平均时长
        let total = stats.total_executed as f64;
        let current_avg = stats.avg_duration_secs;
        stats.avg_duration_secs = (current_avg * (total - 1.0) + duration_secs) / total;
    }
}

impl Default for SyncExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Executor for SyncExecutor {
    /// 执行单个任务
    async fn execute(&self, task: Task) -> Result<ExecutionResult> {
        let start = Instant::now();

        tracing::debug!(
            task_id = %task.id,
            executor = "sync",
            "Executing task"
        );

        // TODO: 实际执行逻辑
        // 这里需要调用 skill 或 agent 执行任务
        // 暂时返回成功结果
        let result = match self.execute_task_impl(&task).await {
            Ok(output) => {
                let duration = start.elapsed().as_secs_f64();
                self.update_stats(duration, true).await;
                ExecutionResult::success(task.id, output, duration)
            }
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                self.update_stats(duration, false).await;
                ExecutionResult::failure(task.id, e.to_string(), duration)
            }
        };

        Ok(result)
    }

    /// 批量执行任务（顺序执行）
    async fn execute_batch(&self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        let mut results = Vec::new();

        for task in tasks {
            let result = self.execute(task).await?;
            results.push(result);
        }

        Ok(results)
    }

    fn name(&self) -> &str {
        "sync"
    }

    fn stats(&self) -> ExecutorStats {
        // 使用 try_lock 以避免在 trait 方法中阻塞
        // 如果获取锁失败，返回默认统计
        self.stats.try_lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| ExecutorStats {
                name: "sync".to_string(),
                ..Default::default()
            })
    }
}

impl SyncExecutor {
    /// 实际任务执行实现
    ///
    /// TODO: 集成 skill/agent 执行逻辑
    async fn execute_task_impl(&self, task: &Task) -> Result<serde_json::Value> {
        // 如果任务指定了 skill，调用 skill 执行
        if let Some(skill_id) = &task.skill {
            tracing::debug!(skill_id = %skill_id, "Executing skill");

            // TODO: 调用 SkillExecutor
            // return skill_executor.execute_skill(skill_id, &task.input).await;

            return Ok(serde_json::json!({
                "skill": skill_id,
                "status": "executed",
                "message": "Skill execution not yet implemented"
            }));
        }

        // 默认返回任务信息
        Ok(serde_json::json!({
            "task_id": task.id,
            "title": task.title,
            "status": "completed",
            "message": "Task executed successfully"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskLevel, TaskPriority};

    fn create_test_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            title: format!("Test Task {}", id),
            description: Some("Test description".to_string()),
            status: crate::types::TaskStatus::Pending,
            priority: TaskPriority::Medium,
            level: TaskLevel::mechanical_default(),
            group: "test".to_string(),
            skill: None,
            input: serde_json::json!({"key": "value"}),
            output: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            dependencies: vec![],
        }
    }

    #[tokio::test]
    async fn test_sync_executor() {
        let executor = SyncExecutor::new();
        let task = create_test_task("1");

        let result = executor.execute(task).await.unwrap();

        assert_eq!(result.task_id, "1");
        assert!(result.is_success());
        assert!(result.duration_secs >= 0.0);
    }

    #[tokio::test]
    async fn test_sync_executor_batch() {
        let executor = SyncExecutor::new();
        let tasks = vec![
            create_test_task("1"),
            create_test_task("2"),
            create_test_task("3"),
        ];

        let results = executor.execute_batch(tasks).await.unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_success()));
    }

    #[tokio::test]
    async fn test_executor_stats() {
        let executor = SyncExecutor::new();
        let task = create_test_task("1");

        executor.execute(task).await.unwrap();
        let stats = executor.stats();

        assert_eq!(stats.total_executed, 1);
        assert_eq!(stats.succeeded, 1);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.name, "sync");
    }
}
