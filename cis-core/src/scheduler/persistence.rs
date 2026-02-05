//! DAG 持久化存储
//!
//! 将 DAG 运行状态和 Task 保存到 SQLite 数据库，支持重启后恢复。

use rusqlite::{Connection, OptionalExtension};

use crate::error::Result;
use crate::scheduler::{DagRun, DagRunStatus, DagSpec};
use crate::types::{Task, TaskStatus};

/// DAG 持久化存储
pub struct DagPersistence {
    db: Connection,
}

impl DagPersistence {
    /// 创建新的持久化实例
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // 创建 dag_specs 表 - 存储 DAG 规格定义
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dag_specs (
                dag_id TEXT PRIMARY KEY,
                target_node TEXT,
                scope_type TEXT NOT NULL,
                scope_id TEXT,
                content_hash TEXT,
                priority TEXT NOT NULL,
                spec_json TEXT NOT NULL,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // 创建 dag_runs 表 - 存储 DAG 运行实例
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dag_runs (
                run_id TEXT PRIMARY KEY,
                dag_id TEXT NOT NULL,
                status TEXT NOT NULL,
                dag_json TEXT NOT NULL,
                debts_json TEXT NOT NULL,
                target_node TEXT,
                scope_type TEXT NOT NULL,
                scope_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // 创建 tasks 表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                task_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                priority TEXT NOT NULL,
                group_name TEXT NOT NULL,
                task_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self { db: conn })
    }

    // ==================== DagSpec 存储 ====================

    /// 保存 DAG 规格
    pub fn save_spec(&self, spec: &DagSpec) -> Result<()> {
        let spec_json = serde_json::to_string(spec)?;
        let (scope_type, scope_id) = spec.scope.to_db_fields();
        
        self.db.execute(
            "INSERT OR REPLACE INTO dag_specs 
             (dag_id, target_node, scope_type, scope_id, content_hash, priority, 
              spec_json, version, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                spec.dag_id,
                spec.target_node,
                scope_type,
                scope_id,
                spec.content_hash(),
                format!("{:?}", spec.priority),
                spec_json,
                spec.version,
                chrono::Utc::now().to_rfc3339(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// 加载 DAG 规格
    pub fn load_spec(&self, dag_id: &str) -> Result<Option<DagSpec>> {
        let mut stmt = self
            .db
            .prepare("SELECT spec_json FROM dag_specs WHERE dag_id = ?1")?;

        let spec_json: Option<String> = stmt.query_row([dag_id], |row| row.get(0)).optional()?;

        match spec_json {
            Some(json) => {
                let spec: DagSpec = serde_json::from_str(&json)?;
                Ok(Some(spec))
            }
            None => Ok(None),
        }
    }

    /// 删除 DAG 规格
    pub fn delete_spec(&self, dag_id: &str) -> Result<()> {
        self.db.execute("DELETE FROM dag_specs WHERE dag_id = ?1", [dag_id])?;
        Ok(())
    }

    /// 保存 DAG 运行（简化版 - 不需要 spec 时）
    pub fn save_run_simple(&self, run: &DagRun) -> Result<()> {
        let dag_json = run.to_json()?;
        let debts_json = serde_json::to_string(&run.debts)?;
        let status_str = format!("{:?}", run.status);

        self.db.execute(
            "INSERT OR REPLACE INTO dag_runs 
             (run_id, dag_id, status, dag_json, debts_json, target_node, scope_type, scope_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                run.run_id,
                "",
                status_str,
                dag_json,
                debts_json,
                Option::<String>::None,
                "Global",
                Option::<String>::None,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// 保存 DAG 运行（完整版 - 带 spec）
    pub fn save_run(&self, run: &DagRun, spec: &DagSpec) -> Result<()> {
        let dag_json = run.to_json()?;
        let debts_json = serde_json::to_string(&run.debts)?;
        let status_str = format!("{:?}", run.status);
        let (scope_type, scope_id) = spec.scope.to_db_fields();

        self.db.execute(
            "INSERT OR REPLACE INTO dag_runs 
             (run_id, dag_id, status, dag_json, debts_json, target_node, scope_type, scope_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                run.run_id,
                spec.dag_id,
                status_str,
                dag_json,
                debts_json,
                spec.target_node,
                scope_type,
                scope_id,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// 加载 DAG 运行
    pub fn load_run(&self, run_id: &str) -> Result<Option<DagRun>> {
        let mut stmt = self
            .db
            .prepare("SELECT dag_json FROM dag_runs WHERE run_id = ?1")?;

        let dag_json: Option<String> = stmt.query_row([run_id], |row| row.get(0)).optional()?;

        match dag_json {
            Some(json) => {
                let run = DagRun::from_json(&json)?;
                Ok(Some(run))
            }
            None => Ok(None),
        }
    }

    /// 列出所有运行
    pub fn list_runs(&self) -> Result<Vec<(String, DagRunStatus, String)>> {
        let mut stmt = self
            .db
            .prepare("SELECT run_id, status, updated_at FROM dag_runs ORDER BY updated_at DESC")?;

        let runs = stmt.query_map([], |row| {
            let run_id: String = row.get(0)?;
            let status_str: String = row.get(1)?;
            let updated_at: String = row.get(2)?;

            let status = match status_str.as_str() {
                "Running" => DagRunStatus::Running,
                "Paused" => DagRunStatus::Paused,
                "Completed" => DagRunStatus::Completed,
                "Failed" => DagRunStatus::Failed,
                _ => DagRunStatus::Running,
            };

            Ok((run_id, status, updated_at))
        })?;

        let result: std::result::Result<Vec<_>, _> = runs.collect();
        Ok(result?)
    }

    /// 删除运行记录
    pub fn delete_run(&self, run_id: &str) -> Result<()> {
        self.db
            .execute("DELETE FROM dag_runs WHERE run_id = ?1", [run_id])?;
        Ok(())
    }

    /// 获取数据库连接（用于高级操作）
    pub fn connection(&self) -> &Connection {
        &self.db
    }

    // ==================== Task 存储 ====================

    /// 保存 Task
    pub fn save_task(&self, task: &Task) -> Result<()> {
        let task_json = serde_json::to_string(task)?;
        
        self.db.execute(
            "INSERT OR REPLACE INTO tasks 
             (task_id, title, status, priority, group_name, task_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                task.id,
                task.title,
                format!("{:?}", task.status),
                format!("{:?}", task.priority),
                task.group_name,
                task_json,
                task.created_at.to_rfc3339(),
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        
        Ok(())
    }

    /// 加载 Task
    pub fn load_task(&self, task_id: &str) -> Result<Option<Task>> {
        let mut stmt = self
            .db
            .prepare("SELECT task_json FROM tasks WHERE task_id = ?1")?;
        
        let task_json: Option<String> = stmt.query_row([task_id], |row| row.get(0)).optional()?;
        
        match task_json {
            Some(json) => {
                let task: Task = serde_json::from_str(&json)?;
                Ok(Some(task))
            }
            None => Ok(None),
        }
    }

    /// 列出所有 Tasks
    pub fn list_tasks(&self, status_filter: Option<TaskStatus>) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        
        match status_filter {
            Some(status) => {
                let status_str = format!("{:?}", status);
                let mut stmt = self.db.prepare(
                    "SELECT task_json FROM tasks WHERE status = ?1 ORDER BY created_at DESC"
                )?;
                let rows = stmt.query_map([status_str], |row| {
                    let json: String = row.get(0)?;
                    let task: Task = serde_json::from_str(&json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(task)
                })?;
                
                for row in rows {
                    tasks.push(row?);
                }
            }
            None => {
                let mut stmt = self.db.prepare(
                    "SELECT task_json FROM tasks ORDER BY created_at DESC"
                )?;
                let rows = stmt.query_map([], |row| {
                    let json: String = row.get(0)?;
                    let task: Task = serde_json::from_str(&json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                    Ok(task)
                })?;
                
                for row in rows {
                    tasks.push(row?);
                }
            }
        };
        
        Ok(tasks)
    }

    /// 删除 Task
    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        self.db.execute("DELETE FROM tasks WHERE task_id = ?1", [task_id])?;
        Ok(())
    }

    /// 更新 Task 状态
    pub fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<bool> {
        let task = self.load_task(task_id)?;
        
        match task {
            Some(mut task) => {
                task.status = status;
                self.save_task(&task)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::TaskDag;
    use tempfile::NamedTempFile;

    #[test]
    fn test_persistence_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();

        // Create a simple DAG run
        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        let run = DagRun::new(dag);
        let run_id = run.run_id.clone();

        // Save
        persistence.save_run_simple(&run).unwrap();

        // Load
        let loaded = persistence.load_run(&run_id).unwrap().unwrap();
        assert_eq!(loaded.run_id, run_id);
        assert_eq!(loaded.dag.node_count(), 1);
    }

    #[test]
    fn test_persistence_list_runs() {
        let temp_file = NamedTempFile::new().unwrap();
        let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();

        // Create and save multiple runs
        for i in 0..3 {
            let mut dag = TaskDag::new();
            dag.add_node(format!("task{}", i), vec![]).unwrap();
            dag.initialize();

            let run = DagRun::new(dag);
            persistence.save_run_simple(&run).unwrap();
        }

        // List runs
        let runs = persistence.list_runs().unwrap();
        assert_eq!(runs.len(), 3);
    }

    #[test]
    fn test_persistence_delete_run() {
        let temp_file = NamedTempFile::new().unwrap();
        let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();

        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        let run = DagRun::new(dag);
        let run_id = run.run_id.clone();

        persistence.save_run_simple(&run).unwrap();
        assert!(persistence.load_run(&run_id).unwrap().is_some());

        persistence.delete_run(&run_id).unwrap();
        assert!(persistence.load_run(&run_id).unwrap().is_none());
    }
}
