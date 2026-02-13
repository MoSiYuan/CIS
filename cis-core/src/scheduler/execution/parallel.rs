//! # 并行执行器
//!
//! 使用线程池或异步任务并行执行任务。
//!
//! ## 适用场景
//! - 大量独立任务
//! - 需要高吞吐量
//! - 任务间无依赖关系

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::future::join_all;
use tokio::sync::Mutex;

use super::{Executor, ExecutionResult, ExecutorStats};
use crate::error::Result;
use crate::types::Task;

/// 并行执行器配置
#[derive(Debug, Clone)]
pub struct ParallelExecutorConfig {
    /// 最大并发数
    pub max_concurrency: usize,
    /// 任务超时时间（秒）
    pub timeout_secs: Option<u64>,
}

impl Default for ParallelExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 10,
            timeout_secs: Some(300), // 默认 5 分钟超时
        }
    }
}

/// 并行执行器
///
/// 使用 tokio 异步任务并行执行任务。
pub struct ParallelExecutor {
    /// 配置
    config: ParallelExecutorConfig,
    /// 执行统计
    stats: Arc<Mutex<ExecutorStats>>,
}

impl ParallelExecutor {
    /// 创建新的并行执行器
    pub fn new(config: ParallelExecutorConfig) -> Self {
        Self {
            config,
            stats: Arc::new(Mutex::new(ExecutorStats {
                name: "parallel".to_string(),
                ..Default::default()
            })),
        }
    }

    /// 创建默认配置的执行器
    pub fn default_config() -> Self {
        Self::new(ParallelExecutorConfig::default())
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

    /// 执行单个任务（内部实现）
    async fn execute_task_internal(&self, task: Task) -> ExecutionResult {
        let start = Instant::now();

        tracing::debug!(
            task_id = %task.id,
            executor = "parallel",
            "Executing task"
        );

        // 应用超时
        let execute_future = self.execute_task_impl(&task);

        let result = match self.config.timeout_secs {
            Some(timeout) => {
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(timeout),
                    execute_future,
                )
                .await
                {
                    Ok(Ok(output)) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, true).await;
                        ExecutionResult::success(task.id, output, duration)
                    }
                    Ok(Err(e)) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, false).await;
                        ExecutionResult::failure(task.id, e.to_string(), duration)
                    }
                    Err(_) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, false).await;
                        ExecutionResult::failure(
                            task.id,
                            format!("Execution timeout after {} seconds", timeout),
                            duration,
                        )
                    }
                }
            }
            None => match execute_future.await {
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
            },
        };

        result
    }

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

impl Default for ParallelExecutor {
    fn default() -> Self {
        Self::default_config()
    }
}

#[async_trait]
impl Executor for ParallelExecutor {
    /// 执行单个任务
    async fn execute(&self, task: Task) -> Result<ExecutionResult> {
        Ok(self.execute_task_internal(task).await)
    }

    /// 批量执行任务（并行执行）
    async fn execute_batch(&self, tasks: Vec<Task>) -> Result<Vec<ExecutionResult>> {
        tracing::debug!(
            count = tasks.len(),
            max_concurrency = self.config.max_concurrency,
            "Executing batch of tasks"
        );

        // 使用信号量限制并发数
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrency));
        let mut futures = Vec::new();

        for task in tasks {
            let semaphore = semaphore.clone();
            let executor = self.clone_task_executor();

            let future = async move {
                let _permit = semaphore.acquire().await.unwrap();
                executor.execute_task_internal(task).await
            };

            futures.push(future);
        }

        // 并行执行所有任务
        let results = join_all(futures).await;

        Ok(results)
    }

    fn name(&self) -> &str {
        "parallel"
    }

    fn stats(&self) -> ExecutorStats {
        // 使用 try_lock 以避免在 trait 方法中阻塞
        self.stats.try_lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| ExecutorStats {
                name: "parallel".to_string(),
                ..Default::default()
            })
    }
}

impl ParallelExecutor {
    /// 克隆任务执行器（用于并发执行）
    fn clone_task_executor(&self) -> TaskExecutorHandle {
        TaskExecutorHandle {
            config: self.config.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// 任务执行器句柄
///
/// 用于在并发任务中共享执行器状态。
#[derive(Clone)]
struct TaskExecutorHandle {
    config: ParallelExecutorConfig,
    stats: Arc<Mutex<ExecutorStats>>,
}

impl TaskExecutorHandle {
    /// 更新统计信息
    async fn update_stats(&self, duration_secs: f64, success: bool) {
        let mut stats = self.stats.lock().await;
        stats.total_executed += 1;
        if success {
            stats.succeeded += 1;
        } else {
            stats.failed += 1;
        }

        let total = stats.total_executed as f64;
        let current_avg = stats.avg_duration_secs;
        stats.avg_duration_secs = (current_avg * (total - 1.0) + duration_secs) / total;
    }

    /// 执行任务
    async fn execute_task_internal(&self, task: Task) -> super::ExecutionResult {
        let start = Instant::now();

        tracing::debug!(
            task_id = %task.id,
            executor = "parallel",
            "Executing task"
        );

        // 应用超时
        let execute_future = self.execute_task_impl(&task);

        let result = match self.config.timeout_secs {
            Some(timeout) => {
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(timeout),
                    execute_future,
                )
                .await
                {
                    Ok(Ok(output)) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, true).await;
                        super::ExecutionResult::success(task.id, output, duration)
                    }
                    Ok(Err(e)) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, false).await;
                        super::ExecutionResult::failure(task.id, e.to_string(), duration)
                    }
                    Err(_) => {
                        let duration = start.elapsed().as_secs_f64();
                        self.update_stats(duration, false).await;
                        super::ExecutionResult::failure(
                            task.id,
                            format!("Execution timeout after {} seconds", timeout),
                            duration,
                        )
                    }
                }
            }
            None => match execute_future.await {
                Ok(output) => {
                    let duration = start.elapsed().as_secs_f64();
                    self.update_stats(duration, true).await;
                    super::ExecutionResult::success(task.id, output, duration)
                }
                Err(e) => {
                    let duration = start.elapsed().as_secs_f64();
                    self.update_stats(duration, false).await;
                    super::ExecutionResult::failure(task.id, e.to_string(), duration)
                }
            },
        };

        result
    }

    /// 实际任务执行实现
    async fn execute_task_impl(&self, task: &Task) -> Result<serde_json::Value> {
        if let Some(skill_id) = &task.skill {
            tracing::debug!(skill_id = %skill_id, "Executing skill");

            return Ok(serde_json::json!({
                "skill": skill_id,
                "status": "executed",
                "message": "Skill execution not yet implemented"
            }));
        }

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
    async fn test_parallel_executor() {
        let executor = ParallelExecutor::default_config();
        let task = create_test_task("1");

        let result = executor.execute(task).await.unwrap();

        assert_eq!(result.task_id, "1");
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_parallel_executor_batch() {
        let executor = ParallelExecutor::default_config();
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
    async fn test_parallel_executor_stats() {
        let executor = ParallelExecutor::default_config();
        let tasks = vec![
            create_test_task("1"),
            create_test_task("2"),
        ];

        executor.execute_batch(tasks).await.unwrap();
        let stats = executor.stats();

        assert_eq!(stats.total_executed, 2);
        assert_eq!(stats.succeeded, 2);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.name, "parallel");
    }
}
