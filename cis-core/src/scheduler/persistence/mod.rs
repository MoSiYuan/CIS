//! # 任务持久化
//!
//! 定义任务持久化接口，支持多种存储后端。

pub mod sqlite;
pub mod memory;

use async_trait::async_trait;

use crate::error::Result;
use crate::types::{Task, TaskStatus};

pub use sqlite::SqlitePersistence;
pub use memory::MemoryPersistence;

// ExecutionResult 定义在本模块中以避免循环依赖
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: serde_json::Value,
    pub duration_secs: f64,
    pub error: Option<String>,
}

/// 任务持久化 Trait
///
/// 定义任务状态和执行结果的持久化接口。
#[async_trait]
pub trait Persistence: Send + Sync {
    /// 保存执行结果
    async fn save_execution(&self, result: &ExecutionResult) -> Result<()>;

    /// 加载任务状态
    async fn load_task_status(&self, task_id: &str) -> Result<TaskStatus>;

    /// 保存任务
    async fn save_task(&self, task: &Task) -> Result<()>;

    /// 加载任务
    async fn load_task(&self, task_id: &str) -> Result<Option<Task>>;

    /// 删除任务
    async fn delete_task(&self, task_id: &str) -> Result<()>;

    /// 获取所有任务
    async fn get_all_tasks(&self) -> Result<Vec<Task>>;

    /// 按状态获取任务
    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>>;

    /// 获取持久化后端名称
    fn backend_name(&self) -> &str {
        "persistence"
    }

    /// 检查连接是否有效
    async fn is_healthy(&self) -> bool {
        true
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
    async fn test_memory_persistence() {
        let persistence = MemoryPersistence::new();
        let task = create_test_task("1");

        // 保存任务
        persistence.save_task(&task).await.unwrap();

        // 加载任务
        let loaded = persistence.load_task("1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, "1");

        // 获取所有任务
        let all = persistence.get_all_tasks().await.unwrap();
        assert_eq!(all.len(), 1);

        // 删除任务
        persistence.delete_task("1").await.unwrap();
        let loaded = persistence.load_task("1").await.unwrap();
        assert!(loaded.is_none());
    }
}
