//! # 任务仓储
//!
//! 提供任务的 CRUD 操作和复杂查询功能。

use super::db::DatabasePool;
use super::models::*;
use rusqlite::{params, Connection};
use std::sync::Arc;

/// 任务仓储
pub struct TaskRepository {
    db: Arc<DatabasePool>,
}

impl TaskRepository {
    /// 创建新的任务仓储
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// 创建任务
    pub async fn create(&self, task: &TaskEntity) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let dependencies_json = serde_json::to_string(&task.dependencies)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let context_vars_json = serde_json::to_string(&task.context_variables)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let metadata_json = task.metadata.as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut stmt = conn.prepare(
            "INSERT INTO tasks (
                task_id, name, type, priority,
                prompt_template, context_variables_json,
                description, estimated_effort_days,
                dependencies_json, engine_type, engine_context_id,
                status, created_at, updated_at,
                assigned_team_id, assigned_agent_id, assigned_at,
                result_json, error_message, started_at, completed_at,
                duration_seconds, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"
        )?;

        stmt.execute(params![
            &task.task_id,
            &task.name,
            &task.task_type,
            &task.priority,
            &task.prompt_template,
            &context_vars_json,
            &task.description,
            &task.estimated_effort_days,
            &dependencies_json,
            &task.engine_type,
            &task.engine_context_id,
            &task.status,
            &task.created_at_ts,
            &task.updated_at_ts,
            &task.assigned_team_id,
            &task.assigned_agent_id,
            &task.assigned_at,
            &task.result,
            &task.error_message,
            &task.started_at,
            &task.completed_at,
            &task.duration_seconds,
            &metadata_json,
        ])?;

        conn.last_insert_rowid()
    }

    /// 批量创建任务
    pub async fn batch_create(&self, tasks: &[TaskEntity]) -> rusqlite::Result<Vec<i64>> {
        self.db.transaction(|conn| {
            let mut stmt = conn.prepare(
                "INSERT INTO tasks (
                    task_id, name, type, priority,
                    prompt_template, context_variables_json,
                    description, estimated_effort_days,
                    dependencies_json, engine_type, engine_context_id,
                    status, created_at, updated_at,
                    metadata_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"
            )?;

            let mut ids = Vec::new();
            for task in tasks {
                let dependencies_json = serde_json::to_string(&task.dependencies)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let context_vars_json = serde_json::to_string(&task.context_variables)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let metadata_json = task.metadata.as_ref()
                    .map(|m| serde_json::to_string(m))
                    .transpose()
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                stmt.execute(params![
                    &task.task_id,
                    &task.name,
                    &task.task_type,
                    &task.priority,
                    &task.prompt_template,
                    &context_vars_json,
                    &task.description,
                    &task.estimated_effort_days,
                    &dependencies_json,
                    &task.engine_type,
                    &task.engine_context_id,
                    &task.status,
                    &task.created_at_ts,
                    &task.updated_at_ts,
                    &metadata_json,
                ])?;

                ids.push(conn.last_insert_rowid()?);
            }

            Ok(ids)
        }).await
    }

    /// 根据 ID 获取任务
    pub async fn get_by_id(&self, id: i64) -> rusqlite::Result<Option<TaskEntity>> {
        let conn = self.db.acquire().await?;
        let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?1")?;

        let task = stmt.query_row(params![id], |row| Self::map_row(row)).ok();

        Ok(task)
    }

    /// 根据 task_id 获取任务
    pub async fn get_by_task_id(&self, task_id: &str) -> rusqlite::Result<Option<TaskEntity>> {
        let conn = self.db.acquire().await?;
        let mut stmt = conn.prepare("SELECT * FROM tasks WHERE task_id = ?1")?;

        let task = stmt.query_row(params![task_id], |row| Self::map_row(row)).ok();

        Ok(task)
    }

    /// 查询任务（支持多种过滤条件）
    pub async fn query(&self, filter: TaskFilter) -> rusqlite::Result<Vec<TaskEntity>> {
        let conn = self.db.acquire().await?;

        let mut sql = String::from("SELECT * FROM tasks WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // 状态过滤
        if let Some(statuses) = &filter.status {
            let placeholders: Vec<_> = statuses.iter().map(|_| "(?)").collect();
            sql.push_str(&format!(" AND status IN ({})", placeholders.join(",")));
            for status in statuses {
                params.push(Box::new(status));
            }
        }

        // 类型过滤
        if let Some(task_types) = &filter.task_types {
            let placeholders: Vec<_> = task_types.iter().map(|_| "(?)").collect();
            sql.push_str(&format!(" AND type IN ({})", placeholders.join(",")));
            for task_type in task_types {
                params.push(Box::new(task_type));
            }
        }

        // 优先级过滤
        if let Some(min_priority) = filter.min_priority {
            sql.push_str(&format!(" AND priority >= {}", min_priority.value()));
        }
        if let Some(max_priority) = filter.max_priority {
            sql.push_str(&format!(" AND priority <= {}", max_priority.value()));
        }

        // Team 过滤
        if let Some(team_id) = &filter.assigned_team {
            sql.push_str(" AND assigned_team_id = ?");
            params.push(Box::new(team_id));
        }

        // 引擎类型过滤
        if let Some(engine_type) = &filter.engine_type {
            sql.push_str(" AND engine_type = ?");
            params.push(Box::new(engine_type));
        }

        // 排序
        sql.push_str(&format!(
            " ORDER BY {} {}",
            filter.sort_by.as_str(),
            filter.sort_order.as_str()
        ));

        // 分页
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let tasks = stmt.query_map(param_refs.as_slice(), |row| Self::map_row(row))?.collect();

        tasks
    }

    /// 更新任务状态
    pub async fn update_status(
        &self,
        id: i64,
        status: TaskStatus,
        error_message: Option<String>,
    ) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        let mut sql = String::from("UPDATE tasks SET status = ?2, updated_at = ?3");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(status), Box::new(now)];

        if let Some(error) = error_message {
            sql.push_str(", error_message = ?");
            params.push(Box::new(error));
        }

        sql.push_str(" WHERE id = ?1");
        params.push(Box::new(id));

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.prepare(&sql)?.execute(param_refs.as_slice())?;

        Ok(())
    }

    /// 更新任务分配信息
    pub async fn update_assignment(
        &self,
        id: i64,
        team_id: Option<String>,
        agent_id: Option<i64>,
    ) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE tasks SET assigned_team_id = ?2, assigned_agent_id = ?3,
             assigned_at = ?4, status = 'assigned', updated_at = ?4
             WHERE id = ?1",
            params![id, team_id, agent_id, now],
        )?;

        Ok(())
    }

    /// 更新任务执行结果
    pub async fn update_result(
        &self,
        id: i64,
        result: &TaskResult,
        duration_seconds: f64,
    ) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();
        let result_json = serde_json::to_string(result)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        conn.execute(
            "UPDATE tasks SET result_json = ?2, duration_seconds = ?3,
             completed_at = ?4, status = 'completed', updated_at = ?4
             WHERE id = ?1",
            params![id, result_json, duration_seconds, now],
        )?;

        Ok(())
    }

    /// 标记任务为运行中
    pub async fn mark_running(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE tasks SET status = 'running', started_at = ?2, updated_at = ?2
             WHERE id = ?1",
            params![id, now],
        )?;

        Ok(())
    }

    /// 删除任务
    pub async fn delete(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 批量删除任务
    pub async fn batch_delete(&self, ids: &[i64]) -> rusqlite::Result<usize> {
        self.db.transaction(|conn| {
            let placeholders: Vec<_> = ids.iter().map(|_| "(?)").collect();
            let sql = format!("DELETE FROM tasks WHERE id IN ({})", placeholders.join(","));

            let mut stmt = conn.prepare(&sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();

            stmt.execute(param_refs.as_slice())
        }).await
    }

    /// 统计任务数量
    pub async fn count(&self, filter: TaskFilter) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let mut sql = String::from("SELECT COUNT(*) FROM tasks WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // 应用与 query 相同的过滤逻辑
        if let Some(statuses) = &filter.status {
            let placeholders: Vec<_> = statuses.iter().map(|_| "(?)").collect();
            sql.push_str(&format!(" AND status IN ({})", placeholders.join(",")));
            for status in statuses {
                params.push(Box::new(status));
            }
        }

        if let Some(min_priority) = filter.min_priority {
            sql.push_str(&format!(" AND priority >= {}", min_priority.value()));
        }
        if let Some(max_priority) = filter.max_priority {
            sql.push_str(&format!(" AND priority <= {}", max_priority.value()));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = stmt.query_row(param_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// 全文搜索任务
    pub async fn search(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<TaskEntity>> {
        let conn = self.db.acquire().await?;

        let sql = format!(
            "SELECT t.* FROM tasks t
             INNER JOIN tasks_fts fts ON t.id = fts.rowid
             WHERE tasks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        );

        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt.query_map(params![query, limit as i64], |row| Self::map_row(row))?.collect();

        tasks
    }

    /// 映射数据库行到 TaskEntity
    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<TaskEntity> {
        Ok(TaskEntity {
            id: row.get(0)?,
            task_id: row.get(1)?,
            name: row.get(2)?,
            task_type: row.get(3)?,
            priority: row.get(4)?,
            prompt_template: row.get(5)?,
            context_variables: serde_json::from_str(row.get::<_, String>(6)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(Box::new(e).into()))?,
            description: row.get(7)?,
            estimated_effort_days: row.get(8)?,
            dependencies: serde_json::from_str(row.get::<_, String>(9)?)
                .unwrap_or_else(|_| Vec::new()),
            engine_type: row.get(10)?,
            engine_context_id: row.get(11)?,
            status: row.get(12)?,
            assigned_team_id: row.get(13)?,
            assigned_agent_id: row.get(14)?,
            assigned_at: row.get(15)?,
            result: row.get::<_, Option<String>>(16)?.and_then(|s| {
                serde_json::from_str(&s).ok()
            }),
            error_message: row.get(17)?,
            started_at: row.get(18)?,
            completed_at: row.get(19)?,
            duration_seconds: row.get(20)?,
            metadata: row.get::<_, Option<String>>(21)?.and_then(|s| {
                serde_json::from_str(&s).ok()
            }),
            created_at_ts: row.get(22)?,
            updated_at_ts: row.get(23)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::db::create_database_pool;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_get_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let repo = TaskRepository::new(pool);

        let task = TaskEntity {
            id: 0,
            task_id: "test-1".to_string(),
            name: "Test Task".to_string(),
            task_type: TaskType::CodeReview,
            priority: TaskPriority::P0,
            prompt_template: "Review the code".to_string(),
            context_variables: serde_json::json!({}),
            description: Some("Test description".to_string()),
            estimated_effort_days: Some(5.0),
            dependencies: vec![],
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: None,
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        };

        let id = repo.create(&task).await.unwrap();
        assert!(id > 0);

        let retrieved = repo.get_by_id(id).await.unwrap().unwrap();
        assert_eq!(retrieved.task_id, "test-1");
        assert_eq!(retrieved.name, "Test Task");
        assert_eq!(retrieved.priority, TaskPriority::P0);
    }

    #[tokio::test]
    async fn test_query_with_filter() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let repo = TaskRepository::new(pool);

        // 创建多个任务
        for i in 0..3 {
            let task = TaskEntity {
                id: 0,
                task_id: format!("test-{}", i),
                name: format!("Task {}", i),
                task_type: TaskType::CodeReview,
                priority: if i == 0 { TaskPriority::P0 } else { TaskPriority::P1 },
                prompt_template: "Review".to_string(),
                context_variables: serde_json::json!({}),
                description: None,
                estimated_effort_days: None,
                dependencies: vec![],
                engine_type: None,
                engine_context_id: None,
                status: TaskStatus::Pending,
                assigned_team_id: None,
                assigned_agent_id: None,
                assigned_at: None,
                result: None,
                error_message: None,
                started_at: None,
                completed_at: None,
                duration_seconds: None,
                metadata: None,
                created_at_ts: chrono::Utc::now().timestamp(),
                updated_at_ts: chrono::Utc::now().timestamp(),
            };
            repo.create(&task).await.unwrap();
        }

        // 查询 P0 任务
        let filter = TaskFilter {
            min_priority: Some(TaskPriority::P0),
            max_priority: Some(TaskPriority::P0),
            ..Default::default()
        };

        let results = repo.query(filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].priority, TaskPriority::P0);
    }
}
