# CIS-DAG 最小化实现方案

> 目标：快速可用，兼容单机/集群，CLI可查询

---

## 核心设计原则

```
┌─────────────────────────────────────────────────────────────┐
│                    统一存储 + 统一入口                        │
│                                                              │
│   存储：SQLite（本地优先，Room同步可选）                       │
│   入口：HTTP API（单机） + Room消息（集群）                    │
│   查询：CLI直接读SQLite                                       │
│   并发：乐观锁版本号（最简单有效）                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 1. 统一存储层（SQLite）

### 表结构（最小化）

```sql
-- DAG定义表
CREATE TABLE dag_definitions (
    dag_id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,     -- SHA256(tasks_json) 用于去重
    tasks_json TEXT NOT NULL,       -- 任务定义JSON
    target_node TEXT,               -- 目标节点标识（可选）
    scope TEXT DEFAULT 'global',    -- 作用域：global/project-{x}/user-{y}
    status TEXT DEFAULT 'pending',  -- pending/running/completed/failed
    version INTEGER DEFAULT 1,      -- 乐观锁版本号
    owner_node TEXT,                -- 执行节点（认领后填写）
    created_by TEXT,                -- 创建者（GLM DID或CLI用户）
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- DAG运行实例表
CREATE TABLE dag_runs (
    run_id TEXT PRIMARY KEY,
    dag_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    current_task TEXT,              -- 当前执行的任务ID
    progress INTEGER DEFAULT 0,     -- 进度百分比
    result_json TEXT,               -- 执行结果
    error_message TEXT,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    FOREIGN KEY (dag_id) REFERENCES dag_definitions(dag_id)
);

-- 任务状态表（用于详细跟踪）
CREATE TABLE task_executions (
    execution_id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    status TEXT DEFAULT 'pending',  -- pending/running/completed/failed
    output TEXT,                    -- 标准输出
    error TEXT,                     -- 错误输出
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    FOREIGN KEY (run_id) REFERENCES dag_runs(run_id)
);

-- 索引
CREATE INDEX idx_dag_status ON dag_definitions(status);
CREATE INDEX idx_dag_scope ON dag_definitions(scope);
CREATE INDEX idx_dag_owner ON dag_definitions(owner_node);
CREATE INDEX idx_runs_dag ON dag_runs(dag_id);
CREATE INDEX idx_runs_status ON dag_runs(status);
```

### 存储位置

```rust
pub fn db_path() -> PathBuf {
    // 统一使用 CIS 数据目录
    cis_core::storage::paths::Paths::data_dir().join("dag.db")
}

// 单机：~/.local/share/cis/dag.db (Linux)
// 集群：每个节点本地一份，通过 Room 事件同步
```

---

## 2. 统一入口层

### 2.1 HTTP API（单机模式）

```rust
// cis-core/src/glm/mod.rs 简化版
pub struct GlmApiServer {
    db: Arc<Mutex<Connection>>,
}

impl GlmApiServer {
    // POST /api/v1/dag/publish
    pub async fn publish_dag(&self, req: PublishRequest) -> Result<DagResponse> {
        let db = self.db.lock().await;
        
        // 1. 检查去重（content_hash）
        if self.dag_exists(&req.content_hash)? {
            return Ok(DagResponse::AlreadyExists);
        }
        
        // 2. 创建DAG（本地执行）
        let dag = DagDefinition::new(req);
        self.save_dag(&db, &dag)?;
        
        // 3. 立即执行（单机模式）
        self.spawn_local_execution(&dag.dag_id);
        
        Ok(DagResponse::Created { dag_id: dag.dag_id })
    }
}
```

### 2.2 Room 消息（集群模式）

```rust
// cis-core/src/matrix/events/dag.rs（新增）

/// Room DAG 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DagRoomEvent {
    /// 广播新DAG
    #[serde(rename = "io.cis.dag.publish")]
    Publish {
        dag_id: String,
        content_hash: String,
        tasks_json: String,
        target_node: Option<String>,
        scope: String,
        timestamp: i64,
    },
    
    /// 认领DAG（CAS）
    #[serde(rename = "io.cis.dag.claim")]
    Claim {
        dag_id: String,
        node_id: String,
        timestamp: i64,
    },
    
    /// 状态更新
    #[serde(rename = "io.cis.dag.status")]
    StatusUpdate {
        dag_id: String,
        run_id: String,
        status: String,
        progress: i32,
        timestamp: i64,
    },
}

/// Room 事件处理器
pub struct DagRoomHandler {
    db: Arc<Mutex<Connection>>,
    node_id: String,
}

impl DagRoomHandler {
    /// 处理 Room 消息
    pub async fn handle_event(&self, event: DagRoomEvent) -> Result<()> {
        match event {
            DagRoomEvent::Publish { dag_id, content_hash, target_node, scope, .. } => {
                // 1. 检查是否匹配本节点
                if !self.should_claim(&target_node) {
                    return Ok(());
                }
                
                // 2. 尝试认领（CAS写入本地DB）
                if self.try_claim(&dag_id, &content_hash)? {
                    // 3. 广播认领成功
                    self.broadcast_claim(&dag_id).await?;
                    
                    // 4. 启动执行
                    self.spawn_execution(&dag_id);
                }
            }
            
            DagRoomEvent::Claim { dag_id, node_id, .. } => {
                // 记录其他节点的认领（用于状态同步）
                if node_id != self.node_id {
                    self.record_remote_claim(&dag_id, &node_id)?;
                }
            }
            
            DagRoomEvent::StatusUpdate { dag_id, status, progress, .. } => {
                // 更新本地状态（集群同步）
                self.update_status(&dag_id, &status, progress)?;
            }
        }
        
        Ok(())
    }
    
    /// CAS认领
    fn try_claim(&self, dag_id: &str, content_hash: &str) -> Result<bool> {
        let db = self.db.lock().await;
        
        // 尝试更新：如果status=pending且owner_node为空，则认领
        let rows = db.execute(
            "UPDATE dag_definitions 
             SET owner_node = ?1, status = 'running', updated_at = CURRENT_TIMESTAMP
             WHERE dag_id = ?2 
               AND content_hash = ?3
               AND status = 'pending'
               AND owner_node IS NULL",
            params![&self.node_id, dag_id, content_hash],
        )?;
        
        Ok(rows > 0)
    }
}
```

---

## 3. 统一执行层

### 3.1 本地执行器（单线程足够）

```rust
// cis-core/src/executor/local.rs（简化版）

pub struct LocalExecutor {
    db: Arc<Mutex<Connection>>,
    running: Arc<AtomicBool>,
}

impl LocalExecutor {
    /// 启动执行循环（单线程）
    pub async fn run(&self) {
        while self.running.load(Ordering::Relaxed) {
            // 1. 查询待执行DAG
            let pending = self.get_pending_dag().await;
            
            if let Some(dag) = pending {
                // 2. 执行
                self.execute_dag(&dag).await;
            } else {
                // 3. 无任务，休眠
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    /// 执行单个DAG
    async fn execute_dag(&self, dag: &DagDefinition) {
        // 1. 创建run记录
        let run_id = self.create_run(&dag.dag_id);
        
        // 2. 拓扑排序执行任务
        let tasks = dag.topological_sort();
        
        for (i, task) in tasks.iter().enumerate() {
            // 更新当前任务
            self.update_progress(&run_id, &task.id, i as i32 * 100 / tasks.len() as i32);
            
            // 执行任务
            let result = self.run_task(task).await;
            
            if result.is_err() {
                // 标记失败，停止后续任务
                self.mark_failed(&run_id, &task.id, &result.err().unwrap());
                return;
            }
        }
        
        // 3. 标记完成
        self.mark_completed(&run_id);
    }
    
    /// 执行单个任务（shell/skill）
    async fn run_task(&self, task: &DagTask) -> Result<(), String> {
        match task.task_type {
            TaskType::Shell => {
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&task.command)
                    .output()
                    .map_err(|e| e.to_string())?;
                
                if output.status.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            }
            TaskType::Skill => {
                // 调用本地skill
                self.invoke_skill(&task.command).await
            }
        }
    }
}
```

### 3.2 乐观锁并发控制

```rust
/// 更新DAG状态（乐观锁）
pub fn update_dag_status(
    db: &Connection,
    dag_id: &str,
    expected_version: i64,
    new_status: &str
) -> Result<bool> {
    let rows = db.execute(
        "UPDATE dag_definitions 
         SET status = ?1, version = version + 1, updated_at = CURRENT_TIMESTAMP
         WHERE dag_id = ?2 AND version = ?3",
        params![new_status, dag_id, expected_version],
    )?;
    
    Ok(rows > 0)
}

/// 使用示例（重试机制）
pub async fn update_with_retry(
    db: Arc<Mutex<Connection>>,
    dag_id: &str,
    new_status: &str
) -> Result<()> {
    for attempt in 0..3 {
        let db_guard = db.lock().await;
        
        // 获取当前版本
        let (current_status, version): (String, i64) = db_guard.query_row(
            "SELECT status, version FROM dag_definitions WHERE dag_id = ?1",
            [dag_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        
        // 检查状态转换是否合法
        if !is_valid_transition(&current_status, new_status) {
            return Err("Invalid state transition".into());
        }
        
        // 尝试更新
        if update_dag_status(&db_guard, dag_id, version, new_status)? {
            return Ok(());  // 成功
        }
        
        // 失败，短暂重试
        drop(db_guard);
        tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
    }
    
    Err("Update failed after 3 attempts".into())
}
```

---

## 4. CLI查询（直接读SQLite）

### 4.1 查询命令

```rust
// cis-node/src/commands/dag_query.rs（新增）

/// dag list - 列出所有DAG
pub fn list_dags(status: Option<&str>, scope: Option<&str>) -> Result<()> {
    let db = DagDb::open()?;
    
    let mut sql = "SELECT dag_id, scope, status, owner_node, created_at FROM dag_definitions".to_string();
    let mut conditions = vec![];
    
    if let Some(s) = status {
        conditions.push(format!("status = '{}'", s));
    }
    if let Some(s) = scope {
        conditions.push(format!("scope = '{}'", s));
    }
    
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    
    sql.push_str(" ORDER BY created_at DESC");
    
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,  // dag_id
            row.get::<_, String>(1)?,  // scope
            row.get::<_, String>(2)?,  // status
            row.get::<_, Option<String>>(3)?,  // owner_node
            row.get::<_, String>(4)?,  // created_at
        ))
    })?;
    
    println!("{:<20} {:<15} {:<10} {:<15} {}", "DAG ID", "Scope", "Status", "Owner", "Created");
    println!("{}", "-".repeat(80));
    
    for row in rows {
        let (id, scope, status, owner, created) = row?;
        println!("{:<20} {:<15} {:<10} {:<15} {}",
            truncate(&id, 20),
            truncate(&scope, 15),
            status,
            owner.as_deref().unwrap_or("-"),
            created
        );
    }
    
    Ok(())
}

/// dag status <dag-id> - 查看状态
pub fn dag_status(dag_id: &str) -> Result<()> {
    let db = DagDb::open()?;
    
    // 查询DAG定义
    let dag: (String, String, String, Option<String>, i64) = db.query_row(
        "SELECT dag_id, scope, status, owner_node, version FROM dag_definitions WHERE dag_id = ?1",
        [dag_id],
        |row| Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?
        )),
    )?;
    
    println!("DAG: {}", dag.0);
    println!("Scope: {}", dag.1);
    println!("Status: {}", dag.2);
    println!("Owner: {}", dag.3.as_deref().unwrap_or("unclaimed"));
    println!("Version: {}", dag.4);
    
    // 查询最近的run
    let runs = db.prepare(
        "SELECT run_id, status, progress, started_at, completed_at 
         FROM dag_runs WHERE dag_id = ?1 ORDER BY started_at DESC LIMIT 5"
    )?.query_map([dag_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    
    println!("\nRecent Runs:");
    for run in runs {
        let (run_id, status, progress, started, completed) = run?;
        println!("  {}: {} ({}%) - {}/{}",
            run_id, status, progress,
            started.as_deref().unwrap_or("-"),
            completed.as_deref().unwrap_or("-")
        );
    }
    
    Ok(())
}

/// dag logs <dag-id> - 查看日志
pub fn dag_logs(dag_id: &str, run_id: Option<&str>) -> Result<()> {
    let db = DagDb::open()?;
    
    let run_id = match run_id {
        Some(r) => r.to_string(),
        None => {
            // 获取最新的run
            db.query_row(
                "SELECT run_id FROM dag_runs WHERE dag_id = ?1 ORDER BY started_at DESC LIMIT 1",
                [dag_id],
                |row| row.get::<_, String>(0)
            )?
        }
    };
    
    // 查询任务执行日志
    let mut stmt = db.prepare(
        "SELECT task_id, status, output, error, started_at, completed_at
         FROM task_executions WHERE run_id = ?1 ORDER BY started_at"
    )?;
    
    let rows = stmt.query_map([&run_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    })?;
    
    println!("Run: {}\n", run_id);
    
    for row in rows {
        let (task_id, status, output, error, started, completed) = row?;
        println!("[{}] {} - {}", 
            started.as_deref().unwrap_or("?"),
            task_id, 
            status
        );
        if let Some(out) = output {
            println!("  stdout: {}", out);
        }
        if let Some(err) = error {
            println!("  stderr: {}", err);
        }
    }
    
    Ok(())
}
```

### 4.2 CLI命令注册

```rust
// cis-node/src/main.rs

/// DAG query commands
#[derive(Subcommand, Debug)]
pub enum DagCommands {
    /// List all DAGs
    List {
        #[arg(short, long)]
        status: Option<String>,
        #[arg(short, long)]
        scope: Option<String>,
    },
    /// Show DAG status
    Status { dag_id: String },
    /// Show DAG logs
    Logs { 
        dag_id: String,
        #[arg(short, long)]
        run: Option<String>,
    },
    /// Publish new DAG
    Publish { file: PathBuf },
}
```

---

## 5. 单机 vs 集群模式切换

### 5.1 配置

```toml
# ~/.cis/config.toml
[dag]
mode = "standalone"  # "standalone" | "cluster"
auto_execute = true  # 单机模式下自动执行

# 集群模式配置
[dag.cluster]
room_id = "!dag-tasks:matrix.org"
claim_timeout = 300  # 认领超时（秒）
```

### 5.2 运行时判断

```rust
pub struct DagManager {
    mode: DagMode,
    db: Arc<Mutex<Connection>>,
    executor: Option<LocalExecutor>,
    room_handler: Option<DagRoomHandler>,
}

impl DagManager {
    pub async fn new(config: &Config) -> Result<Self> {
        let db = Self::open_db()?;
        
        match config.dag.mode {
            DagMode::Standalone => {
                // 单机模式：启动本地执行器
                let executor = LocalExecutor::new(db.clone());
                tokio::spawn(executor.run());
                
                Ok(Self {
                    mode: DagMode::Standalone,
                    db,
                    executor: Some(executor),
                    room_handler: None,
                })
            }
            
            DagMode::Cluster => {
                // 集群模式：启动Room处理器
                let handler = DagRoomHandler::new(
                    db.clone(),
                    config.node_id.clone(),
                    config.dag.cluster.room_id.clone(),
                );
                
                Ok(Self {
                    mode: DagMode::Cluster,
                    db,
                    executor: None,
                    room_handler: Some(handler),
                })
            }
        }
    }
    
    /// 统一入口：发布DAG
    pub async fn publish(&self, dag: DagDefinition) -> Result<()> {
        // 1. 本地存储
        self.save_dag(&dag).await?;
        
        match self.mode {
            DagMode::Standalone => {
                // 单机：自动执行
                self.executor.as_ref().unwrap().schedule(&dag.dag_id);
            }
            
            DagMode::Cluster => {
                // 集群：广播到Room
                self.room_handler.as_ref().unwrap()
                    .broadcast_publish(&dag).await?;
            }
        }
        
        Ok(())
    }
}
```

---

## 6. 实现优先级

| 优先级 | 模块 | 文件 | 工作量 | 说明 |
|--------|------|------|--------|------|
| P0 | SQLite表结构 | `scheduler/persistence.rs` | 2h | 扩展现有表 |
| P0 | 乐观锁更新 | `scheduler/persistence.rs` | 2h | version字段 |
| P0 | 本地执行器 | `executor/local.rs` | 4h | 单线程执行 |
| P0 | CLI查询 | `commands/dag_query.rs` | 3h | list/status/logs |
| P1 | Room事件 | `matrix/events/dag.rs` | 4h | 集群模式 |
| P1 | CAS认领 | `scheduler/claim.rs` | 3h | 分布式认领 |
| P2 | 模式切换 | `dag/manager.rs` | 2h | 单机/集群切换 |

**总计：2-3天完成最小可用版本**

---

## 7. 使用示例

### 单机模式

```bash
# 1. 启动服务
cis glm start -b 127.0.0.1:6767

# 2. GLM推送DAG
curl -X POST http://localhost:6767/api/v1/dag/publish \
  -H "Authorization: Bearer did:cis:local:abc" \
  -d '{"dag_id":"backup","tasks":[...]}'

# 3. CLI查询
cis dag list
cis dag status backup
cis dag logs backup
```

### 集群模式

```bash
# 1. 配置集群模式
echo '[dag]
mode = "cluster"
room_id = "!tasks:cis.dev"
' >> ~/.cis/config.toml

# 2. 启动节点（自动加入Room）
cis node start

# 3. 从任意入口推送（5cloud/GLM）
# DAG通过Room广播，目标节点认领执行

# 4. 任意节点CLI查询（本地SQLite）
cis dag list --status running
cis dag status backup --scope project-a
```

这个最小化方案是否满足需求？需要我详细展开某个部分吗？
