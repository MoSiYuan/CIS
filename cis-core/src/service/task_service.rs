//! # Task Service
//!
//! 任务管理服务，提供任务的创建、分发、追踪等功能。

use super::{ListOptions, PaginatedResult, BatchResult};
use crate::error::{CisError, Result};
use crate::storage::db::CoreDb;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 任务摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub worker_id: Option<String>,
    pub dag_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// 任务详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    #[serde(flatten)]
    pub summary: TaskSummary,
    pub task_type: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub retries: u32,
    pub max_retries: u32,
    pub timeout: u64,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Scheduled,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Scheduled => write!(f, "scheduled"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
            TaskStatus::Retrying => write!(f, "retrying"),
        }
    }
}

impl From<crate::types::TaskStatus> for TaskStatus {
    fn from(status: crate::types::TaskStatus) -> Self {
        match status {
            crate::types::TaskStatus::Pending => TaskStatus::Pending,
            crate::types::TaskStatus::Running => TaskStatus::Running,
            crate::types::TaskStatus::Completed => TaskStatus::Completed,
            crate::types::TaskStatus::Failed => TaskStatus::Failed,
            crate::types::TaskStatus::Blocked => TaskStatus::Pending,
            crate::types::TaskStatus::Cancelled => TaskStatus::Cancelled,
        }
    }
}

impl From<TaskStatus> for crate::types::TaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Pending => crate::types::TaskStatus::Pending,
            TaskStatus::Scheduled => crate::types::TaskStatus::Pending,
            TaskStatus::Running => crate::types::TaskStatus::Running,
            TaskStatus::Completed => crate::types::TaskStatus::Completed,
            TaskStatus::Failed => crate::types::TaskStatus::Failed,
            TaskStatus::Cancelled => crate::types::TaskStatus::Cancelled,
            TaskStatus::Retrying => crate::types::TaskStatus::Pending,
        }
    }
}

/// 任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Critical,
    High,
    #[default]
    Normal,
    Low,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Critical => write!(f, "critical"),
            TaskPriority::High => write!(f, "high"),
            TaskPriority::Normal => write!(f, "normal"),
            TaskPriority::Low => write!(f, "low"),
        }
    }
}

impl From<crate::types::TaskPriority> for TaskPriority {
    fn from(priority: crate::types::TaskPriority) -> Self {
        match priority {
            crate::types::TaskPriority::Urgent => TaskPriority::Critical,
            crate::types::TaskPriority::High => TaskPriority::High,
            crate::types::TaskPriority::Medium => TaskPriority::Normal,
            crate::types::TaskPriority::Low => TaskPriority::Low,
        }
    }
}

impl From<TaskPriority> for crate::types::TaskPriority {
    fn from(priority: TaskPriority) -> Self {
        match priority {
            TaskPriority::Critical => crate::types::TaskPriority::Urgent,
            TaskPriority::High => crate::types::TaskPriority::High,
            TaskPriority::Normal => crate::types::TaskPriority::Medium,
            TaskPriority::Low => crate::types::TaskPriority::Low,
        }
    }
}

/// 创建任务选项
#[derive(Debug, Clone, Default)]
pub struct CreateTaskOptions {
    pub name: String,
    pub task_type: String,
    pub input: serde_json::Value,
    pub priority: TaskPriority,
    pub dag_id: Option<String>,
    pub worker_id: Option<String>,
    pub timeout: u64,
    pub max_retries: u32,
}

/// 队列状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStatus {
    pub pending: usize,
    pub running: usize,
    pub workers_online: usize,
    pub avg_wait_time: u64,
}

/// 任务服务
pub struct TaskService {
    data_dir: PathBuf,
}

impl TaskService {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("cis")
            .join("tasks");
        
        std::fs::create_dir_all(&data_dir)?;
        
        Ok(Self { data_dir })
    }

    /// 获取日志目录
    fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// 获取任务日志文件路径
    fn task_log_file(&self, task_id: &str) -> PathBuf {
        self.logs_dir().join(format!("{}.log", task_id))
    }

    /// 列出任务
    pub async fn list(&self, options: ListOptions) -> Result<PaginatedResult<TaskSummary>> {
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        // 构建基础查询
        let mut query = String::from(
            "SELECT id, title, status, priority, node_id, group_name, created_at, started_at, completed_at 
             FROM tasks WHERE 1=1"
        );
        
        // 应用过滤器
        if !options.all {
            // 默认只显示活跃任务
            query.push_str(" AND status IN ('pending', 'running', 'scheduled')");
        }
        
        // 应用 filters
        for (key, value) in &options.filters {
            match key.as_str() {
                "status" => {
                    query.push_str(&format!(" AND status = '{}'", value.to_lowercase()));
                }
                "priority" => {
                    query.push_str(&format!(" AND priority = '{}'", value.to_lowercase()));
                }
                "worker_id" | "node_id" => {
                    query.push_str(&format!(" AND node_id = '{}'", value));
                }
                "dag_id" | "group_name" => {
                    query.push_str(&format!(" AND group_name = '{}'", value));
                }
                _ => {}
            }
        }
        
        // 应用排序
        if let Some(sort_by) = &options.sort_by {
            let order = if options.sort_desc { "DESC" } else { "ASC" };
            match sort_by.as_str() {
                "created" => query.push_str(&format!(" ORDER BY created_at {}", order)),
                "updated" => query.push_str(&format!(" ORDER BY completed_at {}", order)),
                "priority" => query.push_str(&format!(" ORDER BY priority {}", order)),
                _ => query.push_str(" ORDER BY created_at DESC"),
            }
        } else {
            query.push_str(" ORDER BY created_at DESC");
        }
        
        // 执行查询
        let mut stmt = conn.prepare(&query)
            .map_err(|e| CisError::storage(format!("Query failed: {}", e)))?;
        
        let tasks: Vec<TaskSummary> = stmt.query_map([], |row| {
            let status_str: String = row.get(2)?;
            let priority_int: i32 = row.get(3)?;
            let created_at_i64: i64 = row.get(6)?;
            let started_at_i64: Option<i64> = row.get(7)?;
            let completed_at_i64: Option<i64> = row.get(8)?;
            
            Ok(TaskSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                status: parse_task_status(&status_str),
                priority: parse_task_priority(priority_int),
                worker_id: row.get(4)?,
                dag_id: row.get(5)?,
                created_at: timestamp_to_datetime(created_at_i64),
                started_at: started_at_i64.map(timestamp_to_datetime),
                finished_at: completed_at_i64.map(timestamp_to_datetime),
            })
        })
        .map_err(|e| CisError::storage(format!("Query failed: {}", e)))?
        .filter_map(|r| r.ok())
        .collect();
        
        let total = tasks.len();
        
        // 应用限制
        let items: Vec<TaskSummary> = if let Some(limit) = options.limit {
            tasks.into_iter().take(limit).collect()
        } else {
            tasks
        };
        
        Ok(PaginatedResult::new(items, total))
    }

    /// 查看任务详情
    pub async fn inspect(&self, id: &str) -> Result<TaskInfo> {
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        let mut stmt = conn.prepare(
            "SELECT id, title, status, priority, node_id, group_name, created_at, started_at, completed_at,
                    description, result, error, metadata
             FROM tasks WHERE id = ?1"
        ).map_err(|e| CisError::storage(format!("Query failed: {}", e)))?;
        
        let row_result = stmt.query_row([id], |row| {
            let status_str: String = row.get(2)?;
            let priority_int: i32 = row.get(3)?;
            let created_at_i64: i64 = row.get(6)?;
            let started_at_i64: Option<i64> = row.get(7)?;
            let completed_at_i64: Option<i64> = row.get(8)?;
            let metadata_str: Option<String> = row.get(12)?;
            
            let metadata: HashMap<String, serde_json::Value> = metadata_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            
            let summary = TaskSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                status: parse_task_status(&status_str),
                priority: parse_task_priority(priority_int),
                worker_id: row.get(4)?,
                dag_id: row.get(5)?,
                created_at: timestamp_to_datetime(created_at_i64),
                started_at: started_at_i64.map(timestamp_to_datetime),
                finished_at: completed_at_i64.map(timestamp_to_datetime),
            };
            
            let _description: Option<String> = row.get(9)?;
            let result: Option<String> = row.get(10)?;
            let error: Option<String> = row.get(11)?;
            
            // 从 metadata 中提取额外字段
            let task_type: String = metadata.get("task_type")
                .and_then(|v: &serde_json::Value| v.as_str())
                .unwrap_or("default")
                .to_string();
            
            let input: serde_json::Value = metadata.get("input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            
            let output: Option<serde_json::Value> = result.map(|r| serde_json::json!({"result": r}));
            
            let retries: u32 = metadata.get("retries")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(0) as u32;
            
            let max_retries: u32 = metadata.get("max_retries")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(3) as u32;
            
            let timeout: u64 = metadata.get("timeout")
                .and_then(|v: &serde_json::Value| v.as_u64())
                .unwrap_or(300);
            
            Ok(TaskInfo {
                summary,
                task_type,
                input,
                output,
                error,
                retries,
                max_retries,
                timeout,
                metadata,
            })
        });
        
        match row_result {
            Ok(info) => Ok(info),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(CisError::not_found(format!("Task '{}' not found", id)))
            }
            Err(e) => Err(CisError::storage(format!("Query failed: {}", e))),
        }
    }

    /// 创建任务
    pub async fn create(&self, options: CreateTaskOptions) -> Result<TaskInfo> {
        let id = generate_task_id();
        let now = Utc::now().timestamp();
        
        let mut metadata = HashMap::new();
        metadata.insert("task_type".to_string(), serde_json::json!(options.task_type));
        metadata.insert("input".to_string(), options.input.clone());
        metadata.insert("timeout".to_string(), serde_json::json!(options.timeout));
        metadata.insert("max_retries".to_string(), serde_json::json!(options.max_retries));
        metadata.insert("retries".to_string(), serde_json::json!(0));
        
        let summary = TaskSummary {
            id: id.clone(),
            name: options.name.clone(),
            status: TaskStatus::Pending,
            priority: options.priority,
            worker_id: options.worker_id.clone(),
            dag_id: options.dag_id.clone(),
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
        };
        
        let info = TaskInfo {
            summary: summary.clone(),
            task_type: options.task_type,
            input: options.input,
            output: None,
            error: None,
            retries: 0,
            max_retries: options.max_retries,
            timeout: options.timeout,
            metadata: metadata.clone(),
        };
        
        // 保存到数据库
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        conn.execute(
            "INSERT INTO tasks (id, title, status, priority, node_id, group_name, created_at, metadata, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                options.name,
                "pending",
                task_priority_to_int(options.priority),
                options.worker_id.unwrap_or_default(),
                options.dag_id.unwrap_or_default(),
                now,
                serde_json::to_string(&metadata).unwrap_or_default(),
                format!("Task type: {}", info.task_type),
            ],
        ).map_err(|e| CisError::storage(format!("Failed to create task: {}", e)))?;
        
        // 创建空日志文件
        std::fs::create_dir_all(self.logs_dir())?;
        let log_file = self.task_log_file(&id);
        std::fs::write(&log_file, format!("[{}] Task created: {}\n", Utc::now(), id))?;
        
        Ok(info)
    }

    /// 分发任务
    pub async fn dispatch(&self, id: &str, worker_id: Option<&str>) -> Result<()> {
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        // 检查任务是否存在且处于可分发状态
        let status: String = conn.query_row(
            "SELECT status FROM tasks WHERE id = ?1",
            [id],
            |row| row.get(0),
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CisError::not_found(format!("Task '{}' not found", id))
            }
            _ => CisError::storage(format!("Query failed: {}", e)),
        })?;
        
        if status != "pending" && status != "retrying" {
            return Err(CisError::invalid_input(
                format!("Task '{}' cannot be dispatched from status '{}'", id, status)
            ));
        }
        
        // 更新任务状态为 scheduled
        let now = Utc::now().timestamp();
        let worker = worker_id.unwrap_or("");
        
        conn.execute(
            "UPDATE tasks SET status = ?1, node_id = ?2, started_at = ?3 WHERE id = ?4",
            rusqlite::params!["scheduled", worker, now, id],
        ).map_err(|e| CisError::storage(format!("Failed to dispatch task: {}", e)))?;
        
        // 记录日志
        let log_file = self.task_log_file(id);
        let log_entry = format!(
            "[{}] Task dispatched to worker: {}\n",
            Utc::now(),
            worker_id.unwrap_or("default")
        );
        if log_file.exists() {
            std::fs::write(&log_file, log_entry)?;
        }
        
        Ok(())
    }

    /// 取消任务
    pub async fn cancel(&self, id: &str) -> Result<()> {
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        // 检查任务是否存在
        let status: String = conn.query_row(
            "SELECT status FROM tasks WHERE id = ?1",
            [id],
            |row| row.get(0),
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CisError::not_found(format!("Task '{}' not found", id))
            }
            _ => CisError::storage(format!("Query failed: {}", e)),
        })?;
        
        // 检查是否可取消
        if status == "completed" || status == "cancelled" {
            return Err(CisError::invalid_input(
                format!("Task '{}' is already in terminal state '{}'", id, status)
            ));
        }
        
        // 更新任务状态为 cancelled
        let now = Utc::now().timestamp();
        
        conn.execute(
            "UPDATE tasks SET status = ?1, completed_at = ?2 WHERE id = ?3",
            rusqlite::params!["cancelled", now, id],
        ).map_err(|e| CisError::storage(format!("Failed to cancel task: {}", e)))?;
        
        // 记录日志
        let log_file = self.task_log_file(id);
        let log_entry = format!("[{}] Task cancelled\n", Utc::now());
        if log_file.exists() {
            std::fs::write(&log_file, log_entry)?;
        }
        
        Ok(())
    }

    /// 重试失败的任务
    pub async fn retry(&self, id: &str) -> Result<TaskInfo> {
        let core_db = self.get_core_db()?;
        
        {
            let db = core_db.lock()
                .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
            
            let conn = db.conn();
            
            // 检查任务是否存在
            let (status, metadata_str): (String, Option<String>) = conn.query_row(
                "SELECT status, metadata FROM tasks WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CisError::not_found(format!("Task '{}' not found", id))
                }
                _ => CisError::storage(format!("Query failed: {}", e)),
            })?;
            
            if status != "failed" && status != "cancelled" {
                return Err(CisError::invalid_input(
                    format!("Task '{}' with status '{}' cannot be retried", id, status)
                ));
            }
            
            // 更新 metadata 中的 retries 计数
            let mut metadata: HashMap<String, serde_json::Value> = metadata_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            
            let retries = metadata.get("retries")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32 + 1;
            
            metadata.insert("retries".to_string(), serde_json::json!(retries));
            metadata.insert("previous_error".to_string(), serde_json::json!("Task retried"));
            
            // 重置任务状态
            conn.execute(
                "UPDATE tasks SET status = ?1, started_at = NULL, completed_at = NULL, 
                 result = NULL, error = NULL, metadata = ?2 WHERE id = ?3",
                rusqlite::params!["retrying", serde_json::to_string(&metadata).unwrap_or_default(), id],
            ).map_err(|e| CisError::storage(format!("Failed to retry task: {}", e)))?;
        }
        
        // 记录日志
        let log_file = self.task_log_file(id);
        let log_entry = format!("[{}] Task marked for retry\n", Utc::now());
        if log_file.exists() {
            std::fs::write(&log_file, log_entry)?;
        }
        
        // 返回更新后的任务信息
        self.inspect(id).await
    }

    /// 删除任务
    pub async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let core_db = self.get_core_db()?;
        
        {
            let db = core_db.lock()
                .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
            
            let conn = db.conn();
            
            // 检查任务是否存在
            let status: String = conn.query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                [id],
                |row| row.get(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CisError::not_found(format!("Task '{}' not found", id))
                }
                _ => CisError::storage(format!("Query failed: {}", e)),
            })?;
            
            if !force && (status == "running" || status == "scheduled") {
                return Err(CisError::invalid_input(
                    format!("Task '{}' is active (status: {}). Use force=true to remove.", id, status)
                ));
            }
            
            // 删除任务
            conn.execute("DELETE FROM tasks WHERE id = ?1", [id])
                .map_err(|e| CisError::storage(format!("Failed to delete task: {}", e)))?;
        }
        
        // 删除日志文件
        let log_file = self.task_log_file(id);
        if log_file.exists() {
            let _ = std::fs::remove_file(log_file);
        }
        
        Ok(())
    }

    /// 获取任务日志
    pub async fn logs(&self, id: &str, tail: usize) -> Result<Vec<String>> {
        // 首先检查任务是否存在
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        let exists: bool = conn.query_row(
            "SELECT 1 FROM tasks WHERE id = ?1",
            [id],
            |_| Ok(true),
        ).unwrap_or(false);
        
        if !exists {
            return Err(CisError::not_found(format!("Task '{}' not found", id)));
        }
        
        // 读取日志文件
        let log_file = self.task_log_file(id);
        if !log_file.exists() {
            return Ok(vec![]);
        }
        
        let contents = tokio::fs::read_to_string(log_file).await?;
        let lines: Vec<String> = contents.lines().map(String::from).collect();
        
        let start = if lines.len() > tail { lines.len() - tail } else { 0 };
        Ok(lines[start..].to_vec())
    }

    /// 查看队列状态
    pub async fn queue_status(&self) -> Result<QueueStatus> {
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        let pending: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status IN ('pending', 'retrying')",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let running: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status IN ('running', 'scheduled')",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // 计算平均等待时间（简化实现，基于 pending 任务的创建时间）
        let avg_wait: Option<i64> = conn.query_row(
            "SELECT AVG(?1 - created_at) FROM tasks WHERE status IN ('pending', 'retrying')",
            [Utc::now().timestamp()],
            |row| row.get(0),
        ).unwrap_or(None);
        
        // 获取在线 worker 数量（通过统计不同 node_id）
        let workers: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT node_id) FROM tasks WHERE node_id IS NOT NULL AND node_id != ''",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        Ok(QueueStatus {
            pending: pending as usize,
            running: running as usize,
            workers_online: workers as usize,
            avg_wait_time: avg_wait.unwrap_or(0) as u64,
        })
    }

    /// 清理已完成的任务
    pub async fn prune(&self, max_age_hours: u32) -> Result<usize> {
        let cutoff = Utc::now().timestamp() - (max_age_hours as i64 * 3600);
        
        let core_db = self.get_core_db()?;
        let db = core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let conn = db.conn();
        
        // 获取要删除的任务 ID
        let mut stmt = conn.prepare(
            "SELECT id FROM tasks WHERE status IN ('completed', 'failed', 'cancelled') 
             AND completed_at < ?1"
        ).map_err(|e| CisError::storage(format!("Query failed: {}", e)))?;
        
        let task_ids: Vec<String> = stmt.query_map([cutoff], |row| row.get(0))
            .map_err(|e| CisError::storage(format!("Query failed: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();
        
        let count = task_ids.len();
        
        if count > 0 {
            // 删除数据库记录
            conn.execute(
                "DELETE FROM tasks WHERE status IN ('completed', 'failed', 'cancelled') 
                 AND completed_at < ?1",
                [cutoff],
            ).map_err(|e| CisError::storage(format!("Failed to prune tasks: {}", e)))?;
            
            // 删除日志文件
            for id in &task_ids {
                let log_file = self.task_log_file(id);
                if log_file.exists() {
                    let _ = std::fs::remove_file(log_file);
                }
            }
        }
        
        Ok(count)
    }

    /// 批量操作任务
    pub async fn batch_cancel(&self, ids: &[String]) -> Result<BatchResult> {
        let mut result = BatchResult::new();
        for id in ids {
            match self.cancel(id).await {
                Ok(()) => result.add_success(id),
                Err(e) => result.add_failure(id, e.to_string()),
            }
        }
        Ok(result)
    }

    /// 获取核心数据库引用
    fn get_core_db(&self) -> Result<Arc<Mutex<CoreDb>>> {
        // 直接使用 CoreDb::open() 获取数据库连接
        let core_db = CoreDb::open()?;
        Ok(Arc::new(Mutex::new(core_db)))
    }
}

impl Default for TaskService {
    fn default() -> Self {
        Self::new().expect("Failed to create TaskService")
    }
}

/// 生成任务 ID
fn generate_task_id() -> String {
    format!(
        "task-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

/// 解析任务状态
fn parse_task_status(status: &str) -> TaskStatus {
    match status.to_lowercase().as_str() {
        "pending" => TaskStatus::Pending,
        "scheduled" => TaskStatus::Scheduled,
        "running" => TaskStatus::Running,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        "retrying" => TaskStatus::Retrying,
        _ => TaskStatus::Pending,
    }
}

/// 解析任务优先级
fn parse_task_priority(priority: i32) -> TaskPriority {
    match priority {
        4 => TaskPriority::Critical,
        3 => TaskPriority::High,
        2 => TaskPriority::Normal,
        1 => TaskPriority::Low,
        _ => TaskPriority::Normal,
    }
}

/// 任务优先级转换为整数
fn task_priority_to_int(priority: TaskPriority) -> i32 {
    match priority {
        TaskPriority::Critical => 4,
        TaskPriority::High => 3,
        TaskPriority::Normal => 2,
        TaskPriority::Low => 1,
    }
}

/// 时间戳转换为 DateTime
fn timestamp_to_datetime(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now())
}
