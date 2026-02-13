//! # 任务数据库 Schema 定义
//!
//! 定义所有表结构和索引。

use rusqlite::{Connection, Result as SqliteResult};

/// 初始化数据库表结构
pub fn initialize_schema(conn: &Connection) -> SqliteResult<()> {
    // 启用 WAL 模式以支持并发读写
    conn.execute("PRAGMA journal_mode=WAL", [])?;
    conn.execute("PRAGMA foreign_keys=ON", [])?;
    conn.execute("PRAGMA synchronous=NORMAL", [])?;
    conn.execute("PRAGMA busy_timeout=5000", [])?;

    // 创建所有表
    create_agents_table(conn)?;
    create_tasks_table(conn)?;
    create_task_context_variables_table(conn)?;
    create_engine_contexts_table(conn)?;
    create_agent_sessions_table(conn)?;
    create_task_assignments_table(conn)?;
    create_task_execution_logs_table(conn)?;
    create_task_archives_table(conn)?;

    // 创建全文搜索索引
    create_fulltext_search_index(conn)?;

    Ok(())
}

/// 创建 Agents 表
fn create_agents_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            agent_type TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            config_json TEXT NOT NULL,
            capabilities_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;

    // 创建索引
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agents_type ON agents(agent_type)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_agents_enabled ON agents(enabled)", [])?;

    Ok(())
}

/// 创建 Tasks 表
fn create_tasks_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            type TEXT NOT NULL,
            priority INTEGER NOT NULL,
            prompt_template TEXT NOT NULL,
            context_variables_json TEXT,
            description TEXT,
            estimated_effort_days REAL,
            dependencies_json TEXT,
            engine_type TEXT,
            engine_context_id INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            assigned_team_id INTEGER,
            assigned_agent_id INTEGER,
            assigned_at INTEGER,
            result_json TEXT,
            error_message TEXT,
            started_at INTEGER,
            completed_at INTEGER,
            duration_seconds REAL,
            metadata_json TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;

    // 创建索引
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_type ON tasks(type)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_assigned_team ON tasks(assigned_team_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_status_priority ON tasks(status, priority)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_type_status ON tasks(type, status)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_tasks_assigned_team_status ON tasks(assigned_team_id, status)", [])?;

    Ok(())
}

/// 创建 Task Context Variables 表
fn create_task_context_variables_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_context_variables (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            variable_name TEXT NOT NULL,
            variable_value TEXT NOT NULL,
            variable_type TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_ctx_vars_task ON task_context_variables(task_id)", [])?;

    Ok(())
}

/// 创建 Engine Contexts 表
fn create_engine_contexts_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS engine_contexts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            engine_type TEXT NOT NULL,
            engine_version TEXT,
            base_directory TEXT NOT NULL,
            injectable_directories_json TEXT,
            readonly_directories_json TEXT,
            total_size_bytes INTEGER,
            file_count INTEGER,
            scanned_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(engine_type, engine_version)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_engine_contexts_type ON engine_contexts(engine_type, engine_version)", [])?;

    Ok(())
}

/// 创建 Agent Sessions 表
fn create_agent_sessions_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL UNIQUE,
            agent_id INTEGER NOT NULL,
            runtime_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            context_capacity INTEGER NOT NULL,
            context_used INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            last_used_at INTEGER,
            expires_at INTEGER,
            FOREIGN KEY (agent_id) REFERENCES agents(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_status ON agent_sessions(status)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_runtime ON agent_sessions(runtime_type)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_sessions_agent ON agent_sessions(agent_id)", [])?;

    Ok(())
}

/// 创建 Task Assignments 表
fn create_task_assignments_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_assignments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            team_id TEXT NOT NULL,
            agent_type TEXT NOT NULL,
            session_id INTEGER NOT NULL,
            assignment_reason TEXT,
            matched_capabilities_json TEXT,
            assigned_at INTEGER NOT NULL,
            accepted_at INTEGER,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
            FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_assignments_task ON task_assignments(task_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_assignments_session ON task_assignments(session_id)", [])?;

    Ok(())
}

/// 创建 Task Execution Logs 表
fn create_task_execution_logs_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_execution_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            session_id INTEGER NOT NULL,
            stage TEXT NOT NULL,
            log_level TEXT NOT NULL,
            message TEXT NOT NULL,
            details_json TEXT,
            duration_ms INTEGER,
            tokens_used INTEGER,
            timestamp INTEGER NOT NULL,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
            FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_task ON task_execution_logs(task_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON task_execution_logs(timestamp)", [])?;

    Ok(())
}

/// 创建 Task Archives 表
fn create_task_archives_table(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_archives (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            archive_id TEXT NOT NULL UNIQUE,
            archived_at INTEGER NOT NULL,
            total_tasks INTEGER,
            completed_tasks INTEGER,
            failed_tasks INTEGER,
            compressed_data BLOB,
            archive_type TEXT NOT NULL,
            metadata_json TEXT,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_archives_date ON task_archives(archived_at)", [])?;

    Ok(())
}

/// 创建全文搜索索引
fn create_fulltext_search_index(conn: &Connection) -> SqliteResult<()> {
    // 使用 FTS5 全文搜索
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
            tasks_fts,
            content=tasks,
            content_rowid=id
        )",
        [],
    )?;

    // 创建 FTS5 内容表映射
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS tasks_ai AFTER INSERT ON tasks BEGIN
            INSERT INTO tasks_fts(rowid, task_id, name, description, prompt_template)
            VALUES (new.id, new.task_id, new.name, new.description, new.prompt_template);
        END",
        [],
    )?;

    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS tasks_ad AFTER DELETE ON tasks BEGIN
            INSERT INTO tasks_fts(tasks_fts, rowid, task_id, name, description, prompt_template)
            VALUES ('delete', old.id, old.task_id, old.name, old.description, old.prompt_template);
        END",
        [],
    )?;

    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS tasks_au AFTER UPDATE ON tasks BEGIN
            INSERT INTO tasks_fts(rowid, task_id, name, description, prompt_template)
            VALUES (new.id, new.task_id, new.name, new.description, new.prompt_template);
        END",
        [],
    )?;

    Ok(())
}

/// 清理数据库（VACUUM）
pub fn vacuum_database(conn: &Connection) -> SqliteResult<()> {
    conn.execute("VACUUM", [])
}

/// 获取数据库统计信息
pub fn get_database_stats(conn: &Connection) -> SqliteResult<DatabaseStats> {
    let total_tasks: i64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
    let completed_tasks: i64 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE status = 'completed'", [], |row| row.get(0))?;
    let pending_tasks: i64 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE status = 'pending'", [], |row| row.get(0))?;
    let total_sessions: i64 = conn.query_row("SELECT COUNT(*) FROM agent_sessions", [], |row| row.get(0))?;
    let active_sessions: i64 = conn.query_row("SELECT COUNT(*) FROM agent_sessions WHERE status = 'active'", [], |row| row.get(0))?;

    Ok(DatabaseStats {
        total_tasks,
        completed_tasks,
        pending_tasks,
        total_sessions,
        active_sessions,
    })
}

/// 数据库统计信息
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub pending_tasks: i64,
    pub total_sessions: i64,
    pub active_sessions: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_initialize_schema() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        initialize_schema(&conn).unwrap();

        // 验证表已创建
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(table_count >= 8); // 至少有 8 个表
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        initialize_schema(&conn).unwrap();

        // 验证外键约束已启用
        let fk_enabled: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
        assert_eq!(fk_enabled, 1);
    }

    #[test]
    fn test_wal_mode_enabled() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        initialize_schema(&conn).unwrap();

        // 验证 WAL 模式已启用
        let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0)).unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    #[test]
    fn test_fulltext_search_index() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        initialize_schema(&conn).unwrap();

        // 插入测试任务
        conn.execute(
            "INSERT INTO tasks (task_id, name, type, priority, prompt_template, status, created_at, updated_at)
            VALUES ('test-1', 'CLI 架构修复', 'module_refactoring', 0, 'Test prompt', 'pending', 12345, 12345)",
            [],
        ).unwrap();

        // 使用全文搜索
        let result: Option<String> = conn
            .query_row(
                "SELECT task_id FROM tasks_fts WHERE tasks_fts MATCH 'CLI'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(result, Some("test-1".to_string()));
    }

    #[test]
    fn test_database_stats() {
        let temp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(temp_file.path()).unwrap();

        initialize_schema(&conn).unwrap();

        // 插入测试数据
        conn.execute(
            "INSERT INTO tasks (task_id, name, type, priority, prompt_template, status, created_at, updated_at)
            VALUES ('test-1', 'Task 1', 'module_refactoring', 0, 'Test', 'pending', 12345, 12345)",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO tasks (task_id, name, type, priority, prompt_template, status, created_at, updated_at)
            VALUES ('test-2', 'Task 2', 'module_refactoring', 1, 'Test', 'completed', 12345, 12345)",
            [],
        ).unwrap();

        let stats = get_database_stats(&conn).unwrap();
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.completed_tasks, 1);
        assert_eq!(stats.pending_tasks, 1);
    }
}
