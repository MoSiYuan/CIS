//! # 内存持久化
//!
//! 基于内存的任务持久化实现，主要用于测试。

use std::sync::Arc;
use std::collections::HashMap;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{Persistence, ExecutionResult};
use crate::error::Result;
use crate::types::{Task, TaskStatus};

/// 内存持久化
///
/// 使用内存存储任务数据，适用于测试和开发环境。
pub struct MemoryPersistence {
    /// 任务存储
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl MemoryPersistence {
    /// 创建新的内存持久化实例
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryPersistence {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Persistence for MemoryPersistence {
    /// 保存执行结果
    async fn save_execution(&self, result: &ExecutionResult) -> Result<()> {
        let mut tasks = self.tasks.write().await;

        if let Some(task) = tasks.get_mut(&result.task_id) {
            task.status = result.status.clone();
            task.output = Some(result.output.clone());
            task.updated_at = chrono::Utc::now();
        }

        tracing::debug!(
            task_id = %result.task_id,
            status = %result.status,
            "Saved execution result to memory"
        );

        Ok(())
    }

    /// 加载任务状态
    async fn load_task_status(&self, task_id: &str) -> Result<TaskStatus> {
        let tasks = self.tasks.read().await;

        tasks
            .get(task_id)
            .map(|task| task.status.clone())
            .ok_or_else(|| crate::error::CisError::not_found(format!("Task not found: {}", task_id)))
    }

    /// 保存任务
    async fn save_task(&self, task: &Task) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task.clone());
        Ok(())
    }

    /// 加载任务
    async fn load_task(&self, task_id: &str) -> Result<Option<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(task_id).cloned())
    }

    /// 删除任务
    async fn delete_task(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(task_id);
        Ok(())
    }

    /// 获取所有任务
    async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        let tasks = self.tasks.read().await;
        let mut task_list: Vec<Task> = tasks.values().cloned().collect();
        task_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(task_list)
    }

    /// 按状态获取任务
    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>> {
        let tasks = self.tasks.read().await;
        let mut task_list: Vec<Task> = tasks
            .values()
            .filter(|task| task.status == status)
            .cloned()
            .collect();
        task_list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(task_list)
    }

    fn backend_name(&self) -> &str {
        "memory"
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
            status: TaskStatus::Pending,
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
    async fn test_memory_persistence_save_and_load() {
        let persistence = MemoryPersistence::new();
        let task = create_test_task("1");

        persistence.save_task(&task).await.unwrap();

        let loaded = persistence.load_task("1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "1");
    }

    #[tokio::test]
    async fn test_memory_persistence_get_all() {
        let persistence = MemoryPersistence::new();

        persistence.save_task(&create_test_task("1")).await.unwrap();
        persistence.save_task(&create_test_task("2")).await.unwrap();
        persistence.save_task(&create_test_task("3")).await.unwrap();

        let all = persistence.get_all_tasks().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_memory_persistence_get_by_status() {
        let persistence = MemoryPersistence::new();

        let mut task1 = create_test_task("1");
        task1.status = TaskStatus::Pending;
        persistence.save_task(&task1).await.unwrap();

        let mut task2 = create_test_task("2");
        task2.status = TaskStatus::Completed;
        persistence.save_task(&task2).await.unwrap();

        let pending = persistence.get_tasks_by_status(TaskStatus::Pending).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "1");

        let completed = persistence.get_tasks_by_status(TaskStatus::Completed).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "2");
    }

    #[tokio::test]
    async fn test_memory_persistence_delete() {
        let persistence = MemoryPersistence::new();
        let task = create_test_task("1");

        persistence.save_task(&task).await.unwrap();
        assert!(persistence.load_task("1").await.unwrap().is_some());

        persistence.delete_task("1").await.unwrap();
        assert!(persistence.load_task("1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_memory_persistence_save_execution() {
        let persistence = MemoryPersistence::new();
        let task = create_test_task("1");

        persistence.save_task(&task).await.unwrap();

        let execution_result = ExecutionResult::success(
            "1".to_string(),
            serde_json::json!({"result": "success"}),
            1.5,
        );

        persistence.save_execution(&execution_result).await.unwrap();

        let loaded = persistence.load_task("1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().status, TaskStatus::Completed);
    }
}
