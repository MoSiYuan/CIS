# 单节点多 GLM Agent 协调设计

## 场景澄清

```
┌─────────────────────────────────────────────────────────────┐
│                    云端 CIS 节点 (单节点)                      │
│                                                              │
│   ┌─────────┐   ┌─────────┐   ┌─────────┐                  │
│   │ GLM-1   │   │ GLM-2   │   │ GLM-3   │   ← 多个 Agent   │
│   └────┬────┘   └────┬────┘   └────┬────┘     (用户/项目)   │
│        │             │             │                        │
│        └─────────────┼─────────────┘                        │
│                      ▼                                       │
│        ┌─────────────────────────┐                          │
│        │   HTTP API :6767        │                          │
│        └──────────┬──────────────┘                          │
│                   │                                          │
│        ┌──────────┴──────────┐                              │
│        │  单例 Coordinator    │  ← 节点内唯一                 │
│        │  (本地进程)          │                              │
│        └──────────┬──────────┘                              │
│                   │                                          │
│        ┌──────────┴──────────┐                              │
│        │   SQLite (本地)      │                              │
│        └─────────────────────┘                              │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**关键约束**：
- 单节点（一个 cis-node 进程）
- 多 GLM Agent 连接（通过 HTTP API）
- 任务推送到这个节点执行

---

## 问题场景

### 场景 1：多 Agent 推送同一 DAG
```
GLM-1 ──▶ POST /dag/publish (backup_daily)
            ↓
GLM-2 ──▶ POST /dag/publish (backup_daily)  ← 重复！
            ↓
数据库：两个相同的 DAG 记录
```

### 场景 2：多 Agent 查询/确认竞争
```
GLM-1 ──▶ GET /dag/backup/status
            ↓
GLM-2 ──▶ POST /dag/backup/confirm  ← 同时操作！
            ↓
GLM-1 ──▶ POST /dag/backup/confirm  ← 重复确认！
```

### 场景 3：任务执行争抢
```
假设 DAG 已确认，开始执行：

GLM-1 ──▶ task-1 执行 ──▶ 完成
GLM-2 ──▶ task-1 执行 ──▶ 同时执行同一个任务！
```

---

## 解决方案：本地单例 Coordinator

### 架构

```rust
/// 节点内单例 Coordinator
/// 通过 SQLite + 文件锁保证唯一性
pub struct LocalCoordinator {
    node_id: String,
    db: Connection,
    lock_file: FileLock,  // 确保只有一个 Coordinator 实例
}

impl LocalCoordinator {
    /// 获取或创建 Coordinator（单例）
    pub fn instance() -> Arc<LocalCoordinator> {
        static INSTANCE: OnceCell<Arc<LocalCoordinator>> = OnceCell::new();
        
        INSTANCE.get_or_init(|| {
            // 1. 尝试获取文件锁
            let lock = FileLock::acquire("/tmp/cis-coordinator.lock")
                .expect("Another Coordinator is running");
            
            // 2. 初始化
            Arc::new(LocalCoordinator::new(lock))
        }).clone()
    }
}
```

### 核心机制

#### 1. DAG 去重（基于 ID + 内容哈希）
```rust
impl LocalCoordinator {
    pub fn publish_dag(&self, req: PublishDagRequest) -> Result<DagResponse> {
        // 1. 检查 dag_id 是否已存在
        if let Some(existing) = self.find_dag(&req.dag_id)? {
            // 2. 内容相同？返回已存在
            if existing.content_hash == req.content_hash() {
                return Ok(DagResponse::AlreadyExists(existing.dag_id));
            }
            // 3. 内容不同？报错（需要修改 dag_id）
            return Err(DagError::IdConflict);
        }
        
        // 4. 创建新 DAG
        let dag = DagDefinition::new(req);
        self.save_dag(&dag)?;
        
        Ok(DagResponse::Created(dag.dag_id))
    }
}
```

#### 2. 状态机 + 乐观锁
```rust
pub struct DagRun {
    run_id: String,
    dag_id: String,
    status: DagStatus,  // Pending → Running → Completed/Failed
    version: i64,       // 乐观锁版本
}

impl LocalCoordinator {
    /// 确认 DAG（幂等）
    pub fn confirm_dag(&self, dag_id: &str, agent_id: &str) -> Result<()> {
        // 1. 幂等性：检查是否已确认
        if self.is_confirmed(dag_id)? {
            return Ok(());  // 已确认，直接返回
        }
        
        // 2. 乐观锁更新
        let rows = self.db.execute(
            "UPDATE dag_runs 
             SET status = 'Running', version = version + 1
             WHERE dag_id = ?1 AND status = 'Pending'",
            [dag_id],
        )?;
        
        if rows == 0 {
            // 已被其他 Agent 确认或状态不对
            return Err(DagError::InvalidState);
        }
        
        // 3. 触发执行
        self.spawn_dag_execution(dag_id);
        
        Ok(())
    }
}
```

#### 3. 任务队列 + 本地执行
```rust
pub struct TaskQueue {
    db: Connection,
}

impl TaskQueue {
    /// 提交任务（GLM 调用）
    pub fn submit(&self, task: Task) -> Result<()> {
        self.db.execute(
            "INSERT INTO task_queue (task_id, payload, status, submitted_by)
             VALUES (?1, ?2, 'Pending', ?3)",
            [&task.id, &task.to_json()?, &task.submitted_by],
        )?;
        
        // 通知本地执行器
        self.notify_executor();
        Ok(())
    }
    
    /// 领取任务（本地执行器调用）
    pub fn claim(&self) -> Result<Option<Task>> {
        // 原子性：UPDATE + SELECT
        self.db.execute(
            "UPDATE task_queue 
             SET status = 'Running', claimed_at = datetime('now')
             WHERE task_id = (
                 SELECT task_id FROM task_queue 
                 WHERE status = 'Pending' 
                 ORDER BY priority DESC, created_at ASC 
                 LIMIT 1
             )",
            [],
        )?;
        
        // 返回被领取的任务
        self.db.query_row(
            "SELECT payload FROM task_queue WHERE status = 'Running' 
             AND claimed_at > datetime('now', '-1 second')",
            [],
            |row| {
                let json: String = row.get(0)?;
                Ok(Task::from_json(&json))
            },
        ).optional()
    }
}
```

#### 4. 本地执行器（单线程/线程池）
```rust
pub struct LocalExecutor {
    queue: TaskQueue,
    worker_pool: ThreadPool,
}

impl LocalExecutor {
    pub fn run(&self) {
        loop {
            // 1. 从队列领取任务
            if let Some(task) = self.queue.claim().unwrap() {
                // 2. 提交到线程池执行
                self.worker_pool.spawn(move || {
                    let result = execute_task(&task);
                    
                    // 3. 更新结果
                    self.queue.complete(&task.id, result);
                });
            }
            
            // 4. 短暂休眠，避免忙等
            thread::sleep(Duration::from_millis(100));
        }
    }
}
```

---

## API 流程

### 完整交互流程

```
┌─────────┐                              ┌─────────────────┐
│  GLM-1  │                              │  CIS 节点        │
└────┬────┘                              └────────┬────────┘
     │                                            │
     │ 1. POST /dag/publish                       │
     │    {dag_id: "backup", tasks: [...]}       │
     ├────────────────────────────────────────────▶│
     │                                            │
     │    2. 检查 dag_id 存在？                      │
     │       ├── 存在 → 返回 AlreadyExists        │
     │       └── 不存在 → 创建 Pending DAG         │
     │                                            │
     │◀────────────────────────────────────────────┤
     │    {status: "created", confirm_url: "..."} │
     │                                            │
     │ 3. 用户确认后：                              │
     │    POST /dag/backup/confirm                │
     ├────────────────────────────────────────────▶│
     │                                            │
     │    4. 状态机：Pending → Running              │
     │       启动本地执行器                          │
     │                                            │
     │◀────────────────────────────────────────────┤
     │    {status: "confirmed", message: "..."}    │
     │                                            │
     │ 5. 轮询状态：                                │
     │    GET /dag/backup/status                  │
     ├────────────────────────────────────────────▶│
     │                                            │
     │    6. 返回执行进度                            │
     │◀────────────────────────────────────────────┤
     │    {status: "running", completed: 1/3}      │
```

---

## 并发控制策略

### 策略 1：SQLite 事务（单节点最优）
```rust
// SQLite 在单节点下是线程安全的
// 利用数据库事务天然的原子性

let tx = db.transaction()?;

// 检查
let exists: bool = tx.query_row(...)?;
if exists {
    return Ok(AlreadyExists);
}

// 插入
tx.execute("INSERT INTO dags ...", [...])?;

tx.commit()?;
// 事务保证原子性，无需额外锁
```

### 策略 2：状态机约束
```rust
// 非法操作直接拒绝
match (current_status, requested_action) {
    (Pending, Confirm) => Ok(()),
    (Running, Confirm) => Err("Already confirmed"),  // 幂等
    (Completed, _) => Err("DAG already completed"),  // 终态不可变
    _ => Err("Invalid state transition"),
}
```

### 策略 3：操作去重（Idempotency Key）
```rust
// GLM 端生成唯一 key，服务端记录
pub struct OperationLog {
    idempotency_key: String,  // GLM 生成的 UUID
    operation: String,        // "confirm"
    result: String,           // 缓存结果
}

// 相同 key 直接返回缓存结果
if let Some(cached) = self.find_by_key(&key)? {
    return Ok(cached.result);
}
```

---

## 实现建议

### 文件结构
```
cis-node/src/
├── main.rs
├── commands/
│   └── glm.rs          # HTTP API 端点
├── coordinator/
│   ├── mod.rs          # LocalCoordinator 单例
│   ├── dag_manager.rs  # DAG 生命周期管理
│   ├── task_queue.rs   # 任务队列
│   └── executor.rs     # 本地执行器
```

### 关键代码
```rust
// coordinator/mod.rs

use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

/// 节点级单例 Coordinator
pub struct LocalCoordinator {
    db: Arc<Mutex<Connection>>,
}

impl LocalCoordinator {
    /// 全局单例获取
    pub fn instance() -> Arc<Self> {
        static INSTANCE: Lazy<Arc<LocalCoordinator>> = Lazy::new(|| {
            Arc::new(LocalCoordinator::new())
        });
        INSTANCE.clone()
    }
    
    fn new() -> Self {
        let db = Connection::open(data_dir().join("cis.db"))
            .expect("Failed to open database");
        
        Self {
            db: Arc::new(Mutex::new(db)),
        }
    }
    
    /// 发布 DAG（GLM 调用）
    pub fn publish_dag(&self, req: PublishRequest) -> Result<DagResponse> {
        let db = self.db.lock().unwrap();
        // ... 去重逻辑
    }
    
    /// 确认 DAG（GLM 调用）
    pub fn confirm_dag(&self, dag_id: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        // ... 状态机 + 启动执行
    }
    
    /// 查询状态（GLM 调用）
    pub fn query_status(&self, dag_id: &str) -> Result<DagStatus> {
        let db = self.db.lock().unwrap();
        // ... 查询
    }
}
```

---

## 与分布式方案对比

| 维度 | 单节点单例 Coordinator | 分布式乐观锁 |
|------|------------------------|-------------|
| **适用范围** | 单节点多 Agent | 多节点集群 |
| **实现复杂度** | 低（SQLite 事务） | 中（版本号） |
| **并发控制** | 数据库事务 | 乐观锁 |
| **扩展性** | 垂直扩展（单机） | 水平扩展（多机） |
| **故障恢复** | 重启后从 DB 恢复 | 各节点自治 |
| **网络要求** | 无（本地进程） | 节点间通信 |

---

## 结论

### ✅ 这个方案是合理的！

**前提**：明确是**单节点多 Agent** 场景，不是分布式集群。

**核心优势**：
1. 简单：SQLite 事务天然原子性
2. 高效：本地进程，无网络开销
3. 可靠：单节点故障即服务故障（符合单节点假设）

**推荐实现**：
1. `LocalCoordinator` 单例（Arc + Mutex）
2. SQLite 事务保证原子性
3. 状态机约束非法操作
4. 可选：Idempotency Key 防止重复提交

### 下一步

是否开始实现 `LocalCoordinator`？
- [ ] 创建 coordinator 模块
- [ ] 实现 DAG 去重
- [ ] 实现状态机
- [ ] 集成到 GLM API 端点
