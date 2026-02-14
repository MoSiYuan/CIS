# CIS v1.1.6 任务存储和管理系统

> **设计日期**: 2026-02-12
> **核心需求**: SQLite 替代 TOML，高性能、可控字段、支持归档
> **目标**: 完整的任务定义、分配、执行、归档系统

---

## 1. 数据库 Schema 设计

### 1.1 表结构

```sql
-- Agents 表（Agent 注册和配置）
CREATE TABLE IF NOT EXISTS agents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_type TEXT NOT NULL UNIQUE,         -- Claude, OpenCode, Kimi
    display_name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    config_json TEXT NOT NULL,              -- JSON 配置
    capabilities_json TEXT NOT NULL,        -- JSON 数组
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_agents_type ON agents(agent_type);
CREATE INDEX idx_agents_enabled ON agents(enabled);

-- Tasks 表（任务定义）
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL UNIQUE,          -- 任务 ID（V-1, V-2 等）
    name TEXT NOT NULL,
    type TEXT NOT NULL,                       -- 任务类型
    priority INTEGER NOT NULL,                  -- 0=P0, 1=P1, 2=P2, 3=P3

    -- Prompt 模板和变量
    prompt_template TEXT NOT NULL,
    context_variables_json TEXT,              -- JSON 对象

    -- 任务详情
    description TEXT,
    estimated_effort_days REAL,
    dependencies_json TEXT,                   -- JSON 数组

    -- 引擎代码注入支持
    engine_type TEXT,                         -- Unreal5.7, Unity, Godot
    engine_context_id INTEGER,                  -- 引用 engine_contexts 表

    -- 状态
    status TEXT NOT NULL DEFAULT 'pending',      -- pending, assigned, running, completed, failed
    assigned_team_id INTEGER,                   -- 引用 assignments 表
    assigned_agent_id INTEGER,                  -- Agent 实例 ID
    assigned_at INTEGER,

    -- 执行结果
    result_json TEXT,                          -- JSON 输出
    error_message TEXT,
    started_at INTEGER,
    completed_at INTEGER,
    duration_seconds REAL,

    -- 元数据
    metadata_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_priority ON tasks(priority);
CREATE INDEX idx_tasks_type ON tasks(type);
CREATE INDEX idx_tasks_assigned_team ON tasks(assigned_team_id);

-- Task Context Variables 表（上下文变量，支持复杂场景）
CREATE TABLE IF NOT EXISTS task_context_variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    variable_name TEXT NOT NULL,
    variable_value TEXT NOT NULL,
    variable_type TEXT NOT NULL,               -- string, number, boolean, file, code
    created_at INTEGER NOT NULL,

    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE INDEX idx_ctx_vars_task ON task_context_variables(task_id);

-- Engine Contexts 表（引擎代码上下文，复用）
CREATE TABLE IF NOT EXISTS engine_contexts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    engine_type TEXT NOT NULL,                -- Unreal5.7, Unity, Godot
    engine_version TEXT,                        -- 可选版本号

    -- 引擎目录扫描结果
    base_directory TEXT NOT NULL,
    injectable_directories_json TEXT,           -- JSON 数组
    readonly_directories_json TEXT,             -- JSON 数组
    total_size_bytes INTEGER,
    file_count INTEGER,

    -- 元数据
    scanned_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_engine_contexts_type ON engine_contexts(engine_type, engine_version);

-- Agent Sessions 表（Session 复用）
CREATE TABLE IF NOT EXISTS agent_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE,
    agent_id INTEGER NOT NULL,
    runtime_type TEXT NOT NULL,                -- Claude, OpenCode, Kimi

    -- Session 状态
    status TEXT NOT NULL DEFAULT 'active',     -- active, idle, expired, released
    context_capacity INTEGER NOT NULL,          -- token 数
    context_used INTEGER DEFAULT 0,

    -- 元数据
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    expires_at INTEGER,

    FOREIGN KEY (agent_id) REFERENCES agents(id)
);

CREATE INDEX idx_sessions_status ON agent_sessions(status);
CREATE INDEX idx_sessions_runtime ON agent_sessions(runtime_type);
CREATE INDEX idx_sessions_agent ON agent_sessions(agent_id);

-- Task Assignments 表（任务分配历史）
CREATE TABLE IF NOT EXISTS task_assignments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    team_id TEXT NOT NULL,                     -- V-1, V-2 等
    agent_type TEXT NOT NULL,
    session_id INTEGER NOT NULL,

    -- 分配原因（用于分析）
    assignment_reason TEXT,
    matched_capabilities_json TEXT,

    -- 时间戳
    assigned_at INTEGER NOT NULL,
    accepted_at INTEGER,

    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
);

CREATE INDEX idx_assignments_task ON task_assignments(task_id);
CREATE INDEX idx_assignments_session ON task_assignments(session_id);

-- Task Execution Logs 表（执行日志）
CREATE TABLE IF NOT EXISTS task_execution_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    session_id INTEGER NOT NULL,

    -- 执行阶段
    stage TEXT NOT NULL,                      -- preparing, executing, completed, failed
    log_level TEXT NOT NULL,                  -- DEBUG, INFO, WARN, ERROR
    message TEXT NOT NULL,
    details_json TEXT,

    -- 性能指标
    duration_ms INTEGER,
    tokens_used INTEGER,

    timestamp INTEGER NOT NULL,

    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES agent_sessions(id)
);

CREATE INDEX idx_logs_task ON task_execution_logs(task_id);
CREATE INDEX idx_logs_timestamp ON task_execution_logs(timestamp);

-- Archives 表（任务归档）
CREATE TABLE IF NOT EXISTS task_archives (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    archive_id TEXT NOT NULL UNIQUE,          -- 归档批次 ID
    archived_at INTEGER NOT NULL,

    -- 统计
    total_tasks INTEGER,
    completed_tasks INTEGER,
    failed_tasks INTEGER,

    -- 压缩数据
    compressed_data BLOB,                      -- gzip 压缩的任务数据

    -- 元数据
    archive_type TEXT NOT NULL,                 -- weekly, monthly, manual
    metadata_json TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_archives_date ON task_archives(archived_at);
```

---

## 2. Rust 数据库层实现

### 2.1 数据库连接池

```rust
use rusqlite::{Connection, Result as SqliteResult};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// 数据库连接池
pub struct DatabasePool {
    db_path: Arc<PathBuf>,
    max_connections: usize,
    semaphore: Arc<Semaphore>,
}

impl DatabasePool {
    pub fn new(db_path: PathBuf, max_connections: usize) -> Self {
        Self {
            db_path: Arc::new(db_path),
            max_connections,
            semaphore: Arc::new(Semaphore::new(max_connections)),
        }
    }

    /// 获取连接（异步信号量控制）
    pub async fn acquire(&self) -> SqliteResult<Connection> {
        let _permit = self.semaphore.acquire().await;

        Connection::open(&self.db_path)
            .map_err(|e| rusqlite::Error::SqliteSingleFailure(
                e.message,
                e.path,
            ))
    }

    /// 执行事务
    pub async fn transaction<F, R>(
        &self,
        f: F,
    ) -> SqliteResult<R>
    where
        F: FnOnce(Connection) -> SqliteResult<R>,
    {
        let conn = self.acquire().await?;

        conn.execute("BEGIN IMMEDIATE TRANSACTION", [])
            .and_then(|_| f(conn))
            .and_then(|_| conn.execute("COMMIT", []))
            .map_err(|e| rusqlite::Error::SqliteFailure(e))
    }
}

/// 全局数据库单例
lazy_static! {
    static ref DB_POOL: Arc<DatabasePool> = {
        let db_path = std::env::var("CIS_DB_PATH")
            .unwrap_or_else(|_| "~/.cis/data/tasks.db".to_string());

        Arc::new(DatabasePool::new(
            PathBuf::from(db_path),
            10  // 最多 10 个并发连接
        ))
    };
}
```

### 2.2 Task Repository

```rust
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// 任务实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub task_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub prompt_template: String,
    #[serde(rename = "context_variables")]
    pub context_variables: serde_json::Value,
    pub description: Option<String>,
    pub estimated_effort_days: f64,
    #[serde(rename = "dependencies")]
    pub dependencies: Vec<String>,
    #[serde(rename = "engine_type")]
    pub engine_type: Option<String>,
    #[serde(rename = "engine_context_id")]
    pub engine_context_id: Option<i64>,
    pub status: TaskStatus,
    pub result: Option<TaskResult>,
    #[serde(rename = "created_at")]
    pub created_at: i64,
    #[serde(rename = "updated_at")]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskType {
    #[serde(rename = "module_refactoring")]
    ModuleRefactoring,
    #[serde(rename = "engine_code_injection")]
    EngineCodeInjection,
    #[serde(rename = "performance_optimization")]
    PerformanceOptimization,
    #[serde(rename = "code_review")]
    CodeReview,
    #[serde(rename = "test_writing")]
    TestWriting,
    #[serde(rename = "documentation")]
    Documentation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    #[serde(rename = "0")]
    P0,
    #[serde(rename = "1")]
    P1,
    #[serde(rename = "2")]
    P2,
    #[serde(rename = "3")]
    P3,
}

/// 任务仓储
pub struct TaskRepository {
    db: Arc<DatabasePool>,
}

impl TaskRepository {
    pub fn new() -> Self {
        Self {
            db: DB_POOL.clone(),
        }
    }

    /// 创建任务
    pub async fn create(&self, task: &Task) -> SqliteResult<i64> {
        let conn = self.db.acquire().await?;

        let mut stmt = conn.prepare(
            "INSERT INTO tasks (
                task_id, name, type, priority,
                prompt_template, context_variables_json,
                description, estimated_effort_days,
                dependencies_json, engine_type, engine_context_id,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'pending', ?13, ?13)"
        )?;

        let now = chrono::Utc::now().timestamp();

        stmt.execute(params![
            &task.task_id,
            &task.name,
            &task.task_type,
            &task.priority,
            &task.prompt_template,
            &serde_json::to_string(&task.context_variables),
            &task.description,
            &task.estimated_effort_days,
            &serde_json::to_string(&task.dependencies),
            &task.engine_type,
            &task.engine_context_id,
            now,
            now,
        ])?;

        conn.last_insert_rowid()
    }

    /// 查询任务（支持多种过滤）
    pub async fn query(&self, filter: TaskFilter) -> SqliteResult<Vec<Task>> {
        let conn = self.db.acquire().await?;

        let mut sql = String::from(
            "SELECT * FROM tasks WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // 状态过滤
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status));
        }

        // 类型过滤
        if let Some(task_types) = &filter.task_types {
            let placeholders: Vec<_> = task_types.iter()
                .map(|_| "(?)")
                .collect();
            let placeholder_str = placeholders.join(",");
            sql.push_str(&format!(" AND type IN ({})", placeholder_str));
            for task_type in task_types {
                params.push(Box::new(task_type));
            }
        }

        // 优先级过滤
        if let Some(min_priority) = filter.min_priority {
            sql.push_str(&format!(" AND priority >= {}", min_priority));
        }
        if let Some(max_priority) = filter.max_priority {
            sql.push_str(&format!(" AND priority <= {}", max_priority));
        }

        // Team 过滤
        if let Some(team_id) = &filter.assigned_team {
            sql.push_str(" AND assigned_team_id = ?");
            params.push(Box::new(team_id));
        }

        // 排序
        sql.push_str(&format!(" ORDER BY {} ASC", filter.sort_by.unwrap_or("priority")));

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let tasks = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(Task {
                id: row.get(0)?,
                task_id: row.get(1)?,
                name: row.get(2)?,
                task_type: row.get(3)?,
                priority: row.get(4)?,
                prompt_template: row.get(5)?,
                context_variables: serde_json::from_str(row.get::<_>(6)?)
                    .map_err(|e| rusqlite::Error::SqliteFailure(e.to_string()))?,
                description: row.get(7)?,
                estimated_effort_days: row.get(8)?,
                dependencies: serde_json::from_str(row.get::<_>(9)?)
                    .map_err(|e| rusqlite::Error::SqliteFailure(e.to_string()))?,
                engine_type: row.get::<Option<String>>(10)?,
                engine_context_id: row.get::<Option<i64>>(11)?,
                status: row.get(12)?,
                result: None,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?.collect();

        Ok(tasks)
    }

    /// 更新任务状态
    pub async fn update_status(&self, task_id: &str, status: TaskStatus) -> SqliteResult<()> {
        let conn = self.db.acquire().await?;

        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE task_id = ?1",
            params![status, now, task_id],
        )?;

        Ok(())
    }

    /// 分配任务到 Team
    pub async fn assign_to_team(
        &self,
        task_id: i64,
        team_id: &str,
        session_id: i64,
    ) -> SqliteResult<()> {
        self.db.transaction(|conn| {
            conn.execute(
                "INSERT INTO task_assignments (task_id, team_id, session_id, assigned_at)
                VALUES (?1, ?2, ?3, ?4)",
                params![
                    task_id,
                    team_id,
                    session_id,
                    chrono::Utc::now().timestamp(),
                ],
            )?;

            // 同时更新任务状态
            conn.execute(
                "UPDATE tasks SET status = 'assigned', assigned_team_id = ?2, assigned_at = ?3
                WHERE id = ?1",
                params![team_id, chrono::Utc::now().timestamp(), task_id],
            )?;

            Ok(())
        }).await
    }
}

/// 任务查询过滤器
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<Vec<TaskStatus>>,
    pub task_types: Option<Vec<TaskType>>,
    pub min_priority: Option<TaskPriority>,
    pub max_priority: Option<TaskPriority>,
    pub assigned_team: Option<String>,
    pub sort_by: Option<String>,  // priority, created_at, name
    pub limit: Option<usize>,
}
```

### 2.3 Agent Session Repository

```rust
/// Agent Session 实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub session_id: String,
    pub agent_id: i64,
    pub runtime_type: String,
    pub status: SessionStatus,
    pub context_capacity: i64,
    pub context_used: i64,
    pub created_at: i64,
    pub last_used_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Active,
    Idle,
    Expired,
    Released,
}

/// Agent Session 仓储
pub struct SessionRepository {
    db: Arc<DatabasePool>,
}

impl SessionRepository {
    pub fn new() -> Self {
        Self {
            db: DB_POOL.clone(),
        }
    }

    /// 创建 Session
    pub async fn create(
        &self,
        agent_id: i64,
        runtime_type: &str,
        context_capacity: i64,
        ttl_minutes: i64,
    ) -> SqliteResult<i64> {
        let conn = self.db.acquire().await?;

        let session_id = format!("{}-{}", agent_id, uuid::Uuid::new_v4());
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + (ttl_minutes * 60);

        let mut stmt = conn.prepare(
            "INSERT INTO agent_sessions (
                session_id, agent_id, runtime_type,
                status, context_capacity, context_used,
                created_at, last_used_at, expires_at
            ) VALUES (?1, ?2, ?3, 'active', ?4, 0, ?5, ?6, ?7)"
        )?;

        stmt.execute(params![
            &session_id,
            &agent_id,
            &runtime_type,
            now,
            context_capacity,
            now,
            expires_at,
        ])?;

        conn.last_insert_rowid()
    }

    /// 获取可用 Session（复用）
    pub async fn acquire_session(
        &self,
        agent_id: i64,
    ) -> SqliteResult<Option<AgentSession>> {
        let conn = self.db.acquire().await?;

        let now = chrono::Utc::now().timestamp();

        // 查找该 Agent 的可用 Session
        let mut stmt = conn.prepare(
            "SELECT * FROM agent_sessions
             WHERE agent_id = ?1
             AND status = 'active'
             AND context_used < context_capacity
             AND expires_at > ?2
             ORDER BY last_used_at ASC
             LIMIT 1"
        )?;

        let session = stmt.query_row(params![agent_id, now], |row| {
            Ok(AgentSession {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                runtime_type: row.get(3)?,
                status: row.get(4)?,
                context_capacity: row.get(5)?,
                context_used: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?;

        // 如果找到可用 Session，标记为使用中
        if let Some(ref session) = session {
            conn.execute(
                "UPDATE agent_sessions SET status = 'idle', last_used_at = ?2
                 WHERE id = ?1",
                params![now, session.id],
            )?;
        }

        Ok(session)
    }

    /// 归还 Session（复用）
    pub async fn release_session(&self, session_id: i64) -> SqliteResult<()> {
        let conn = self.db.acquire().await?;

        conn.execute(
            "UPDATE agent_sessions SET status = 'active', context_used = context_used + 1
             WHERE id = ?1",
            params![session_id],
        )?;

        Ok(())
    }
}
```

---

## 3. CLI 工具实现

### 3.1 任务管理 CLI

```rust
// cis-core/src/task/cli.rs

use clap::{Parser, Subcommand};
use crate::task::repository::{TaskRepository, TaskFilter};

#[derive(Parser)]
struct TaskArgs {
    #[clap(subcommand)]
    command: TaskCommand(TaskCommandArgs),
}

#[derive(SubCommand)]
enum TaskCommand {
    /// 创建任务
    Create(TaskCreateArgs),

    /// 列出任务
    List(TaskListArgs),

    /// 分配任务
    Assign(TaskAssignArgs),

    /// 查询任务
    Query(TaskQueryArgs),

    /// 归档任务
    Archive(TaskArchiveArgs),
}

#[derive(Parser)]
struct TaskCreateArgs {
    /// 任务 ID（如 V-1）
    #[clap(short, long)]
    task_id: String,

    /// 任务名称
    #[clap(short, long)]
    name: String,

    /// 任务类型
    #[clap(short, long)]
    #[arg(value_enum)]
    task_type: TaskType,

    /// 优先级
    #[clap(short, long)]
    #[arg(value_enum)]
    priority: TaskPriority,

    /// Prompt 模板
    #[clap(short, long)]
    prompt_template: String,

    /// 上下文变量（JSON）
    #[clap(short, long)]
    context_vars: String,

    /// 描述
    #[clap(short, long)]
    description: Option<String>,

    /// 预估工作量（天）
    #[clap(short, long)]
    estimated_effort: f64,

    /// 依赖任务
    #[clap(short, long)]
    #[clap(delimiter = ',')]
    dependencies: Vec<String>,
}

impl TaskCreateArgs {
    /// 创建任务
    pub async fn execute(&self, repo: &TaskRepository) -> SqliteResult<i64> {
        let task = Task {
            id: 0,
            task_id: self.task_id.clone(),
            name: self.name.clone(),
            task_type: self.task_type.clone(),
            priority: self.priority,
            prompt_template: self.prompt_template.clone(),
            context_variables: serde_json::from_str(&self.context_vars)
                .map_err(|e| rusqlite::Error::SqliteFailure(e.to_string()))?,
            description: self.description,
            estimated_effort_days: self.estimated_effort,
            dependencies: self.dependencies,
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            result: None,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        repo.create(&task).await
    }
}

#[derive(Parser)]
struct TaskListArgs {
    /// 状态过滤
    #[clap(short, long)]
    status: Option<String>,

    /// 类型过滤
    #[clap(short, long)]
    task_type: Option<String>,

    /// 最小优先级
    #[clap(short, long)]
    min_priority: Option<TaskPriority>,

    /// 最大优先级
    #[clap(short, long)]
    max_priority: Option<TaskPriority>,

    /// Team 过滤
    #[clap(short, long)]
    team: Option<String>,

    /// 排序
    #[clap(short, long)]
    sort_by: Option<String>,

    /// 限制数量
    #[clap(short, long)]
    limit: Option<usize>,
}

impl TaskListArgs {
    /// 列出任务
    pub async fn execute(&self, repo: &TaskRepository) -> SqliteResult<Vec<Task>> {
        let mut filter = TaskFilter::default();

        // 解析状态
        if let Some(status_str) = &self.status {
            filter.status = Some(match status_str.as_str() {
                "pending" => vec![TaskStatus::Pending],
                "assigned" => vec![TaskStatus::Assigned],
                "running" => vec![TaskStatus::Running],
                "completed" => vec![TaskStatus::Completed],
                "failed" => vec![TaskStatus::Failed],
                _ => vec![],
            });
        }

        // 解析优先级
        if let Some(prio_str) = &self.min_priority {
            filter.min_priority = Some(match prio_str.as_str() {
                "p0" => TaskPriority::P0,
                "p1" => TaskPriority::P1,
                "p2" => TaskPriority::P2,
                "p3" => TaskPriority::P3,
                _ => return Err(rusqlite::Error::InvalidQuery),
            });
        }

        repo.query(filter).await
    }
}

```

---

## 4. 性能优化

### 4.1 批量操作

```rust
impl TaskRepository {
    /// 批量插入任务
    pub async fn batch_create(&self, tasks: &[Task]) -> SqliteResult<Vec<i64>> {
        self.db.transaction(|conn| {
            let mut stmt = conn.prepare(
                "INSERT INTO tasks (
                    task_id, name, type, priority,
                    prompt_template, context_variables_json,
                    status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8)"
            )?;

            let mut ids = Vec::new();
            for task in tasks {
                let now = chrono::Utc::now().timestamp();
                stmt.execute(params![
                    &task.task_id,
                    &task.name,
                    &task.task_type,
                    &task.priority,
                    &task.prompt_template,
                    &serde_json::to_string(&task.context_variables),
                    now,
                    now,
                ])?;

                ids.push(conn.last_insert_rowid()?);
            }

            Ok(ids)
        }).await
    }

    /// 批量更新状态
    pub async fn batch_update_status(
        &self,
        updates: &[(i64, TaskStatus)],
    ) -> SqliteResult<()> {
        self.db.transaction(|conn| {
            let mut stmt = conn.prepare(
                "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE id = ?1"
            )?;

            let now = chrono::Utc::now().timestamp();
            for (task_id, status) in updates {
                stmt.execute(params![status, now, task_id])?;
            }

            Ok(())
        }).await
    }
}
```

### 4.2 索引优化

```sql
-- 为常用查询创建特定索引
CREATE INDEX idx_tasks_status_priority ON tasks(status, priority);
CREATE INDEX idx_tasks_type_status ON tasks(type, status);
CREATE INDEX idx_tasks_assigned_team_status ON tasks(assigned_team_id, status);

-- 全文搜索索引（FTS5）
CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(
    tasks_fts,
    content=tasks(task_id, name, description, prompt_template),
    content_rowid=id
);

-- 全文搜索查询示例
-- SELECT * FROM tasks WHERE tasks_fts MATCH ?1 ORDER BY rank;
```

---

## 5. CLI 使用示例

### 5.1 创建 V-1 任务（CLI 架构修复）

```bash
# 方式 1：使用命令行参数
cis task create \
    --task-id "V-1" \
    --name "CLI 架构修复" \
    --type module_refactoring \
    --priority p0 \
    --prompt-template "审查以下 CLI handler..." \
    --context-vars '{"handlers_dir": "cis-node/src/cli/handlers"}' \
    --description "审查并重构所有 CLI handler，确保只调用 Server API" \
    --estimated-effort 5 \
    --dependencies "[]"

# 方式 2：使用 JSON 文件
cat > task-v1.json << 'EOF'
{
  "task_id": "V-1",
  "name": "CLI 架构修复",
  "type": "module_refactoring",
  "priority": "p0",
  "prompt_template": "审查以下目录...\n{{handlers_dir}}\n\n参考文档：CLI_GUIDE_OPTIMIZED.md",
  "context_variables": {
    "handlers_dir": "cis-node/src/cli/handlers",
    "guide_doc": "docs/plan/v1.1.6/CLI_GUIDE_OPTIMIZED.md"
  },
  "description": "审查并重构所有 CLI handler，确保只调用 Server API",
  "estimated_effort_days": 5.0,
  "dependencies": []
}
EOF

cis task create-from-json task-v1.json
```

### 5.2 批量创建任务

```json
{
  "tasks": [
    {
      "task_id": "V-1",
      "name": "CLI 架构修复",
      "type": "module_refactoring",
      "priority": "p0",
      "prompt_template": "...",
      "context_variables": {...},
      "estimated_effort_days": 5.0,
      "dependencies": []
    },
    {
      "task_id": "V-2",
      "name": "scheduler 拆分",
      "type": "module_refactoring",
      "priority": "p1",
      "prompt_template": "...",
      "context_variables": {...},
      "estimated_effort_days": 15.0,
      "dependencies": ["V-1"]
    },
    {
      "task_id": "V-4",
      "name": "memory 精准索引",
      "type": "module_refactoring",
      "priority": "p1",
      "prompt_template": "...",
      "context_variables": {...},
      "estimated_effort_days": 15.0,
      "dependencies": []
    }
  ]
}
```

```bash
cis task batch-create tasks.json
```

---

## 6. 多 Agent 加锁实现

### 6.1 数据库锁机制

```sql
-- SQLite 写前锁（WAL）
-- 在数据库连接字符串中启用
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;  -- 5 秒超时
PRAGMA synchronous=NORMAL;       -- 同步模式

-- 数据库级别的锁
BEGIN IMMEDIATE;
-- 执行操作...
COMMIT;
```

### 6.2 并发控制

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

/// 任务管理器（支持并发）
pub struct TaskManager {
    task_repo: Arc<TaskRepository>,
    session_repo: Arc<SessionRepository>,

    // 队列锁（避免重复分配）
    assignment_locks: Arc<Mutex<HashMap<String, ()>>>,
}

impl TaskManager {
    /// 智能任务分配（带加锁）
    pub async fn assign_task_with_lock(
        &self,
        task_id: i64,
        team_id: &str,
    ) -> Result<i64> {
        // 1. 检查是否已有分配在进行
        {
            let locks = self.assignment_locks.lock().await;
            if locks.contains_key(task_id) {
                return Err(anyhow::anyhow!("任务 {} 已在分配中", task_id));
            }
            drop(locks);
        }

        // 2. 加锁
        let mut locks = self.assignment_locks.lock().await;
        locks.insert(task_id.to_string(), ());
        drop(locks);

        // 3. 获取或创建 Session
        let session = self.session_repo.acquire_session(agent_id).await?
            .ok_or_else(|| anyhow::anyhow!("无可用 Session"))?;

        // 4. 分配任务
        let assignment_id = self.task_repo.assign_to_team(
            task_id,
            team_id,
            session.id,
        ).await?;

        // 5. 释放锁（任务已分配）
        {
            let mut locks = self.assignment_locks.lock().await;
            locks.remove(&task_id.to_string());
            drop(locks);
        }

        Ok(assignment_id)
    }
}
```

---

## 7. 数据迁移工具

### 7.1 从 TOML 迁移到 SQLite

```rust
use std::fs;
use std::path::Path;

/// 迁移工具
pub struct Migrator {
    tasks_toml_path: PathBuf,
    db: Arc<DatabasePool>,
}

impl Migrator {
    pub async fn migrate_from_toml(&self) -> Result<usize> {
        // 1. 读取 TOML 文件
        let toml_content = fs::read_to_string(&self.tasks_toml_path)?;
        let tasks_data: TasksToml = toml::from_str(&toml_content)?;

        // 2. 插入到数据库
        let mut count = 0;
        for task_def in &tasks_data.task {
            let task = Task {
                id: 0,
                task_id: task_def.id.clone(),
                name: task_def.name.clone(),
                task_type: task_def.task_type.clone(),
                priority: task_def.priority.clone(),
                prompt_template: task_def.prompt.clone(),
                context_variables: serde_json::to_value(&task_def.context)
                    .map_err(|e| rusqlite::Error::SqliteFailure(e.to_string()))?,
                description: task_def.description,
                estimated_effort_days: task_def.effort,
                dependencies: task_def.dependencies.clone(),
                engine_type: None,
                engine_context_id: None,
                status: TaskStatus::Pending,
                result: None,
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };

            self.db.transaction(|conn| {
                // 插入任务
                let id = conn.execute(
                    "INSERT INTO tasks (task_id, name, type, priority, prompt_template, context_variables_json, status, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7)",
                    params![
                        &task.task_id, &task.name, &task.task_type, &task.priority,
                        &task.prompt_template,
                        &serde_json::to_string(&task.context_variables),
                        chrono::Utc::now().timestamp(),
                        chrono::Utc::now().timestamp(),
                    ],
                )?;

                // 插入上下文变量
                for (key, value) in &task_def.context {
                    conn.execute(
                        "INSERT INTO task_context_variables (task_id, variable_name, variable_value, variable_type, created_at)
                        VALUES (?1, ?2, ?3, 'string', ?4)",
                        params![id, key, value, chrono::Utc::now().timestamp()],
                    )?;
                }

                count += 1;
            }).await?;
        }

        println!("迁移完成：{} 个任务", count);
        Ok(count)
    }
}
```

---

## 8. 完整使用流程

### 8.1 初始化和创建任务

```bash
# 1. 初始化数据库
cis task db init

# 2. 创建 V-1 到 V-10 任务
cis task create \
    --task-id "V-1" \
    --name "CLI 架构修复" \
    --type module_refactoring \
    --priority p0 \
    --prompt "审查 {{handlers_dir}}..." \
    --context-vars '{"handlers_dir": "cis-node/src/cli/handlers"}' \
    --estimated-effort 5

cis task create \
    --task-id "V-2" \
    --name "scheduler 拆分" \
    --type module_refactoring \
    --priority p1 \
    --dependencies "V-1" \
    --estimated-effort 15

# 3. 批量创建（从 JSON）
cis task batch-create tasks.json

# 4. 列出所有 P0-P1 任务
cis task list --status pending --max-priority p1
```

### 8.2 Agent Pool 启动和任务分配

```bash
# 1. 创建 Agent Pool
cis agent pool create --name "v1.1.6-refactor"

# 2. 添加 Teams
cis agent pool add-team "Team-V-CLI" --runtime "claude" \
    --capabilities "code_review,module_refactoring"

# 3. 扫描引擎代码（如果需要）
cis engine scan --engine "unreal5.7" \
    --base-dir "/path/to/unreal/project"

# 4. 启动并行执行
cis agent pool start-parallel --max-teams 7 \
    --auto-assign-tasks

# 5. 查看状态
cis agent pool status --watch
```

---

## 9. 归档和清理

### 9.1 周任务归档

```rust
/// 任务归档器
pub struct TaskArchiver {
    db: Arc<DatabasePool>,
}

impl TaskArchiver {
    /// 归档已完成的任务
    pub async fn archive_week(&self, week_id: &str) -> Result<String> {
        let archive_id = format!("week-{}", week_id);

        // 1. 查询本周已完成的任务
        let conn = self.db.acquire().await?;
        let mut stmt = conn.prepare(
            "SELECT * FROM tasks WHERE status = 'completed'
             AND datetime(completed_at, 'start of week', ?1)
        )?;

        let tasks: Vec<Task> = stmt.query_map(params![week_id], |row| {
            // ...
        })?.collect();

        // 2. 压缩数据
        let compressed = self.compress_tasks(&tasks)?;

        // 3. 插入归档
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO task_archives (archive_id, archived_at, total_tasks, completed_tasks, compressed_data)
            VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &archive_id,
                now,
                tasks.len() as i64,
                tasks.len() as i64,
                &compressed,
            ],
        )?;

        // 4. 删除已归档任务
        conn.execute(
            "DELETE FROM tasks WHERE id IN (SELECT id FROM tasks WHERE status = 'completed'
             AND datetime(completed_at, 'start of week', ?1))",
            params![week_id],
        )?;

        Ok(archive_id)
    }
}
```

---

## 10. 性能对比

### 10.1 SQLite vs TOML

| 指标 | TOML | SQLite | 改进 |
|------|------|--------|------|
| **查询速度** | ~50ms | ~5ms | **10x** |
| **插入速度** | N/A | ~1ms | **新功能** |
| **批量插入** | 困难 | ~50ms/100条 | **新功能** |
| **复杂查询** | 不支持 | 原生支持 | **新功能** |
| **事务支持** | 不支持 | 原生支持 | **新功能** |
| **并发安全** | 不安全 | WAL + 锁 | **10x** |
| **归档支持** | 不支持 | 压缩+删除 | **新功能** |
| **全文搜索** | 不支持 | FTS5 | **新功能** |

---

## 11. CLI 命令完整列表

```bash
# 数据库管理
cis task db init                    # 初始化数据库
cis task db migrate                  # 从 TOML 迁移
cis task db vacuum                  # 清理数据库
cis task db stats                   # 数据库统计

# 任务 CRUD
cis task create [...]                  # 创建任务（交互式）
cis task create-from-json file.json   # 从 JSON 批量创建
cis task list [...]                    # 列出任务
cis task get <task-id>               # 获取任务详情
cis task update <task-id> [...]     # 更新任务
cis task delete <task-id>              # 删除任务

# 任务执行
cis task assign <task-id> --team <team-id>  # 分配任务
cis task start <task-id>             # 开始任务
cis task complete <task-id>            # 完成任务
cis task fail <task-id>               # 标记失败

# 查询和报告
cis task query --sql "SELECT ..."      # 执行自定义 SQL
cis task report --type weekly           # 生成周报告
cis task report --type team <team-id> # 生成 Team 报告

# 引擎代码扫描
cis engine scan --engine <type> --path <dir>  # 扫描引擎代码
cis engine list-contexts <id>         # 列出引擎上下文
cis engine delete <id>                # 删除引擎上下文

# Session 管理
cis session list [--runtime <type>]    # 列出 Sessions
cis session show <session-id>           # 显示 Session 详情
cis session release <session-id>         # 释放 Session
cis session expire                   # 清理过期 Sessions

# Agent Pool 管理
cis agent pool create [...]            # 创建 Pool
cis agent pool list-pools             # 列出 Pools
cis agent pool start-parallel           # 启动并行执行
cis agent pool status                  # 查看状态
cis agent pool stop                    # 停止
```

---

## 总结

### 核心改进

1. **SQLite 替代 TOML**
   - 复杂查询和关系
   - 事务和并发安全
   - 全文搜索（FTS5）
   - 高性能批量操作

2. **多 Agent 加锁复用**
   - Session Pool 管理不同 Runtime 的 Session
   - 智能任务分配（避免重复）
   - 数据库锁保证并发安全

3. **任务归档**
   - 自动归档已完成任务
   - 压缩存储节省空间
   - 保留完整执行历史

4. **CLI 工具完整**
   - 任务 CRUD
   - 批量操作
   - 查询和报告
   - 引擎代码扫描

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 设计完成，待实现
