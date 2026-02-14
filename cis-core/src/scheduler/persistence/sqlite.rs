//! # SQLite 持久化
//!
//! 使用任务数据库提供任务持久化。
//!
//! ## 设计原则
//! - 复用 task 模块的数据库连接池
//! - 使用现有的 tasks 表，不创建新表
//! - 仅负责执行结果的持久化，任务管理由 TaskRepository 负责

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

use super::{Persistence, ExecutionResult};
use crate::error::{CisError, Result};
use crate::task::db::DatabasePool;
use crate::types::{Task, TaskStatus};

/// SQLite 持久化
///
/// 使用任务数据库提供持久化。
pub struct SqlitePersistence {
    /// 数据库连接池
    db_pool: Arc<DatabasePool>,
}

impl SqlitePersistence {
    /// 创建新的 SQLite 持久化实例
    pub fn new(db_pool: Arc<DatabasePool>) -> Self {
        Self { db_pool }
    }

    /// 保存执行结果到数据库
    async fn save_execution_impl(&self, result: &ExecutionResult) -> Result<()> {
        let task_id = result.task_id.clone();
        let status = result.status.clone();
        let output = serde_json::to_string(&result.output).unwrap_or_default();
        let error = result.error.clone().unwrap_or_default();
        let duration = result.duration_secs;

        let pool = self.db_pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            conn.execute(
                "UPDATE tasks SET status = ?1, output = ?2, updated_at = datetime('now') WHERE task_id = ?3",
                [&status.to_string(), &output, &task_id],
            )?;
            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        debug!(
            task_id = %task_id,
            status = %status,
            duration_secs = duration,
            "Saved execution result"
        );

        Ok(())
    }

    /// 加载任务状态
    async fn load_task_status_impl(&self, task_id: &str) -> Result<TaskStatus> {
        let task_id = task_id.to_string();
        let pool = self.db_pool.clone();

        let status: Option<String> = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare("SELECT status FROM tasks WHERE task_id = ?1")?;
            let status = stmt.query_row([&task_id], |row| row.get(0)).optional()?;
            Ok::<Option<String>, rusqlite::Error>(status)
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        match status {
            Some(s) => s.parse::<TaskStatus>()
                .map_err(|e| CisError::execution(format!("Invalid task status: {}", e))),
            None => Err(CisError::not_found(format!("Task not found: {}", task_id))),
        }
    }

    /// 保存任务
    async fn save_task_impl(&self, task: &Task) -> Result<()> {
        let task_json = serde_json::to_string(task)
            .map_err(|e| CisError::serialization(format!("Failed to serialize task: {}", e)))?;

        let pool = self.db_pool.clone();
        let id = task.id.clone();
        let title = task.title.clone();
        let status = task.status.to_string();
        let priority = task.priority.to_string();
        let group = task.group.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;

            conn.execute(
                "INSERT OR REPLACE INTO tasks (task_id, title, status, priority, group_name, task_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))",
                [&id, &title, &status, &priority, &group, &task_json],
            )?;
            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        debug!(task_id = %task.id, "Saved task to database");
        Ok(())
    }

    /// 加载任务
    async fn load_task_impl(&self, task_id: &str) -> Result<Option<Task>> {
        let task_id = task_id.to_string();
        let pool = self.db_pool.clone();

        let task_json: Option<String> = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare("SELECT task_json FROM tasks WHERE task_id = ?1")?;
            let json = stmt.query_row([&task_id], |row| row.get(0)).optional()?;
            Ok::<Option<String>, rusqlite::Error>(json)
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        match task_json {
            Some(json) => {
                let task: Task = serde_json::from_str(&json)
                    .map_err(|e| CisError::serialization(format!("Failed to deserialize task: {}", e)))?;
                Ok(Some(task))
            }
            None => Ok(None),
        }
    }

    /// 删除任务
    async fn delete_task_impl(&self, task_id: &str) -> Result<()> {
        let task_id = task_id.to_string();
        let pool = self.db_pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            conn.execute("DELETE FROM tasks WHERE task_id = ?1", [&task_id])?;
            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        debug!(task_id = %task_id, "Deleted task from database");
        Ok(())
    }

    /// 获取所有任务
    async fn get_all_tasks_impl(&self) -> Result<Vec<Task>> {
        let pool = self.db_pool.clone();

        let task_jsons: Vec<String> = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare("SELECT task_json FROM tasks ORDER BY created_at DESC")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            let mut jsons = Vec::new();
            for row in rows {
                jsons.push(row?);
            }
            Ok::<Vec<String>, rusqlite::Error>(jsons)
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        let mut tasks = Vec::new();
        for json in task_jsons {
            let task: Task = serde_json::from_str(&json)
                .map_err(|e| CisError::serialization(format!("Failed to deserialize task: {}", e)))?;
            tasks.push(task);
        }

        Ok(tasks)
    }

    /// 按状态获取任务
    async fn get_tasks_by_status_impl(&self, status: TaskStatus) -> Result<Vec<Task>> {
        let pool = self.db_pool.clone();
        let status_str = status.to_string();

        let task_jsons: Vec<String> = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare("SELECT task_json FROM tasks WHERE status = ?1 ORDER BY created_at DESC")?;
            let rows = stmt.query_map([&status_str], |row| row.get(0))?;
            let mut jsons = Vec::new();
            for row in rows {
                jsons.push(row?);
            }
            Ok::<Vec<String>, rusqlite::Error>(jsons)
        })
        .await
        .map_err(|e| CisError::execution(format!("Task join error: {}", e)))??;

        let mut tasks = Vec::new();
        for json in task_jsons {
            let task: Task = serde_json::from_str(&json)
                .map_err(|e| CisError::serialization(format!("Failed to deserialize task: {}", e)))?;
            tasks.push(task);
        }

        Ok(tasks)
    }
}

#[async_trait]
impl Persistence for SqlitePersistence {
    async fn save_execution(&self, result: &ExecutionResult) -> Result<()> {
        self.save_execution_impl(result).await
    }

    async fn load_task_status(&self, task_id: &str) -> Result<TaskStatus> {
        self.load_task_status_impl(task_id).await
    }

    async fn save_task(&self, task: &Task) -> Result<()> {
        self.save_task_impl(task).await
    }

    async fn load_task(&self, task_id: &str) -> Result<Option<Task>> {
        self.load_task_impl(task_id).await
    }

    async fn delete_task(&self, task_id: &str) -> Result<()> {
        self.delete_task_impl(task_id).await
    }

    async fn get_all_tasks(&self) -> Result<Vec<Task>> {
        self.get_all_tasks_impl().await
    }

    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>> {
        self.get_tasks_by_status_impl(status).await
    }

    fn backend_name(&self) -> &str {
        "sqlite"
    }

    async fn is_healthy(&self) -> bool {
        let pool = self.db_pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get();
            conn.is_ok()
        })
        .await
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::db::create_database_pool;
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
    async fn test_sqlite_persistence() {
        // 使用内存数据库进行测试
        let pool = Arc::new(
            create_database_pool(":memory:")
                .await
                .expect("Failed to create database pool")
        );

        let persistence = SqlitePersistence::new(pool);
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
