# 分布式DAG协调架构设计

## 架构概览

```
┌─────────────────────────────────────────────────────────────────────┐
│                          CIS 集群                                    │
│                                                                      │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌────────┐ │
│  │ Node-1  │   │ Node-2  │   │ Node-3  │   │ Node-4  │   │5cloud  │ │
│  │(Worker) │   │(Worker) │   │(Worker) │   │(Worker) │   │(Entry) │ │
│  └────┬────┘   └────┬────┘   └────┬────┘   └────┬────┘   └───┬────┘ │
│       │             │             │             │            │      │
│       └─────────────┴─────────────┴─────────────┘            │      │
│                         │                                     │      │
│                         ▼                                     │      │
│              ┌─────────────────────┐                          │      │
│              │   Matrix Room       │◀─────────────────────────┘      │
│              │  (!tasks:example)   │                                 │
│              └─────────────────────┘                                 │
│                         │                                            │
└─────────────────────────┼────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     任务认领与执行流程                                │
│                                                                      │
│  阶段1: 任务广播                                                      │
│  ─────────────────                                                    │
│  5cloud ──▶ Room: "新DAG: backup_daily, target: node-1, scope: proj-a" │
│                                                                      │
│  阶段2: 节点认领 (Node-1)                                             │
│  ────────────────────────                                             │
│  Node-1: "我匹配target，我来认领"                                      │
│    ├── 写入公域记忆: "DAG backup_daily 归我执行"                       │
│    ├── 写入DAG表: status=PENDING, owner=node-1                        │
│    └── 尝试启动 singleton-agent-DAG                                   │
│                                                                      │
│  阶段3: 单例协调器启动 (Node-1 本地)                                   │
│  ───────────────────────────────────                                  │
│  singleton-agent-DAG:                                                  │
│    ├── 检查 scope=proj-a 是否已有 worker                               │
│    │   ├── 有 → 复用现有 worker                                        │
│    │   └── 无 → 启动新 agent-worker-proj-a                             │
│    └── 将DAG任务分配给 agent-worker                                    │
│                                                                      │
│  阶段4: 任务执行 (agent-worker-proj-a)                                 │
│  ─────────────────────────────────────                                │
│  agent-worker:                                                         │
│    ├── 从队列领取任务                                                  │
│    ├── 执行 (shell/skill)                                              │
│    ├── 更新状态到公域记忆                                              │
│    └── 完成后退出或等待新任务                                          │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 核心概念

### 1. 作用域 (Scope)
```rust
/// 任务执行的作用域，同一作用域只有一个worker
pub enum TaskScope {
    /// 项目级别隔离
    Project(String),      // e.g., "proj-a", "proj-b"
    
    /// 用户级别隔离  
    User(String),         // e.g., "user-123"
    
    /// DAG类型隔离
    DagType(String),      // e.g., "backup", "deploy", "test"
    
    /// 全局唯一（整个集群一个worker）
    Global,
}

/// 作用域决定了worker的启动策略
impl TaskScope {
    pub fn worker_name(&self) -> String {
        match self {
            TaskScope::Project(p) => format!("worker-project-{}", p),
            TaskScope::User(u) => format!("worker-user-{}", u),
            TaskScope::DagType(t) => format!("worker-type-{}", t),
            TaskScope::Global => "worker-global".to_string(),
        }
    }
}
```

### 2. 单例agent-DAG (Singleton Coordinator)
```rust
/// 每个节点本地只有一个，负责管理该节点的所有worker
pub struct SingletonDagCoordinator {
    node_id: String,
    
    /// 管理的workers: scope -> worker handle
    workers: HashMap<TaskScope, WorkerHandle>,
    
    /// 任务队列
    task_queue: Arc<Mutex<VecDeque<DagTask>>>,
}

impl SingletonDagCoordinator {
    /// 全局单例（节点内）
    pub fn instance() -> Arc<Self> { ... }
    
    /// 处理新DAG
    pub async fn handle_new_dag(&self, dag: DagDefinition) -> Result<()> {
        // 1. 确定作用域
        let scope = dag.get_scope();
        
        // 2. 检查是否已有worker
        if let Some(worker) = self.workers.get(&scope) {
            // 复用现有worker
            worker.submit_dag(dag).await?;
        } else {
            // 启动新worker
            let worker = self.spawn_worker(scope.clone()).await?;
            worker.submit_dag(dag).await?;
            self.workers.insert(scope, worker);
        }
        
        Ok(())
    }
    
    /// 启动worker进程/线程
    async fn spawn_worker(&self, scope: TaskScope) -> Result<WorkerHandle> {
        let worker_name = scope.worker_name();
        
        // 启动独立进程
        let child = Command::new("cis-agent-worker")
            .arg("--name", &worker_name)
            .arg("--scope", &scope.to_string())
            .arg("--node", &self.node_id)
            .spawn()?;
        
        Ok(WorkerHandle { process: child, scope })
    }
}
```

### 3. agent-worker (作用域执行器)
```rust
/// 特定作用域的任务执行器
/// 同一作用域全局只有一个实例运行
pub struct AgentWorker {
    name: String,
    scope: TaskScope,
    node_id: String,
    
    /// 本地任务队列
    local_queue: VecDeque<DagTask>,
    
    /// 执行状态
    status: WorkerStatus,
}

impl AgentWorker {
    pub async fn run(&mut self) {
        loop {
            // 1. 从队列取任务
            if let Some(task) = self.local_queue.pop_front() {
                // 2. 执行
                let result = self.execute_task(task).await;
                
                // 3. 更新公域记忆
                self.update_public_memory(&result).await;
            }
            
            // 4. 检查是否需要退出（空闲超时）
            if self.should_exit() {
                break;
            }
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    async fn execute_task(&self, task: DagTask) -> TaskResult {
        match task.task_type {
            TaskType::Shell => self.run_shell(&task).await,
            TaskType::Skill => self.run_skill(&task).await,
            TaskType::Matrix => self.send_matrix(&task).await,
        }
    }
}
```

---

## 分布式认领机制

### 问题：如何避免多节点同时认领？

#### 方案A：Room消息顺序 + 第一个响应
```rust
/// 认领协议
pub struct DagClaimProtocol {
    /// 广播认领意向
    pub async fn broadcast_intent(&self, dag_id: &str) {
        let msg = json!({
            "type": "dag_claim_intent",
            "dag_id": dag_id,
            "claimer": self.node_id,
            "timestamp": now(),
        });
        
        self.room.send(msg.to_string()).await;
    }
    
    /// 监听认领响应，第一个确认的获得执行权
    pub async fn wait_for_claim_result(&self, dag_id: &str) -> bool {
        let mut events = self.room.events();
        
        while let Some(event) = events.next().await {
            if let Ok(msg) = serde_json::from_str::<ClaimMessage>(&event.content) {
                if msg.dag_id == dag_id {
                    // 检查是否是自己第一个发送的
                    return msg.claimer == self.node_id;
                }
            }
        }
        
        false
    }
}
```

#### 方案B：公域记忆CAS（推荐）
```rust
/// 基于公域记忆的乐观锁认领
pub async fn claim_dag_via_memory(&self, dag_id: &str) -> Result<bool> {
    let claim_key = format!("dag:{}/claim", dag_id);
    
    // 1. 尝试写入认领信息
    let claim_info = json!({
        "node_id": self.node_id,
        "claimed_at": now(),
        "status": "claiming",
    });
    
    // 2. CAS操作：只有key不存在时才写入
    let result = self.memory_service.cas(
        &claim_key,
        None,  // 期望值：不存在
        Some(claim_info.to_string()),  // 新值
        MemoryDomain::Public,
    ).await?;
    
    if result.success {
        // 认领成功，现在写入DAG到本地
        self.persist_dag(dag_id).await?;
        
        // 更新状态为claimed
        self.memory_service.set(
            &claim_key,
            json!({"status": "claimed", "node_id": self.node_id}),
            MemoryDomain::Public,
        ).await?;
        
        Ok(true)
    } else {
        // 已被其他节点认领
        Ok(false)
    }
}
```

#### 方案C：SQLite分布式锁（基于Room）
```rust
/// 利用Room的CRDT特性实现分布式锁
/// 每个节点都有SQLite副本，通过Room同步
pub struct DistributedLock {
    room: MatrixRoom,
}

impl DistributedLock {
    /// 尝试获取锁
    pub async fn try_lock(&self, lock_name: &str) -> Result<LockGuard> {
        // 写入锁请求到Room
        let lock_req = LockRequest {
            name: lock_name.to_string(),
            node_id: self.node_id.clone(),
            timestamp: now(),
        };
        
        self.room.send(lock_req.to_json()).await?;
        
        // 等待同步（CRDT保证最终一致）
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 查询本地SQLite（已通过Room同步）
        let holder = self.db.query_row(
            "SELECT node_id FROM distributed_locks WHERE name = ?1",
            [lock_name],
        )?;
        
        if holder == self.node_id {
            Ok(LockGuard { name: lock_name.to_string() })
        } else {
            Err(LockError::AlreadyHeld(holder))
        }
    }
}
```

---

## 完整流程时序图

```
5cloud          Room           Node-1          Node-2       Node-3
  │              │               │               │            │
  │ 1. publish   │               │               │            │
  │──────────────▶               │               │            │
  │              │ 2. broadcast  │               │            │
  │              │───────────────┼───────────────┼────────────▶
  │              │               │               │            │
  │              │               │ 3. check target
  │              │               │      (match)  │            │
  │              │               │               │ 4. check target
  │              │               │               │   (no match)│
  │              │               │               │            │ 5. check target
  │              │               │               │            │   (no match)
  │              │               │               │            │
  │              │               │ 6. CAS claim  │               │
  │              │               │──────┐        │               │
  │              │               │      │ write public memory
  │              │               │◀─────┘        │               │
  │              │               │   success!    │               │
  │              │               │               │               │
  │              │               │ 7. persist DAG│               │
  │              │               │ (SQLite local)│               │
  │              │               │               │               │
  │              │               │ 8. start singleton
  │              │               │    coordinator              │
  │              │               │               │               │
  │              │               │ 9. check scope worker
  │              │               │   (proj-a not exist)        │
  │              │               │               │               │
  │              │               │ 10. spawn agent-worker-proj-a
  │              │               │     (new process)           │
  │              │               │               │               │
  │              │               │ 11. submit DAG to worker    │
  │              │               │               │               │
  │              │               │ 12. execute tasks           │
  │              │               │               │               │
  │              │ 13. status update (public memory)           │
  │              │◀──────────────┤               │               │
  │ 14. poll     │               │               │               │
  │◀─────────────┤               │               │               │
```

---

## 关键问题解答

### Q1: 同一作用域只有一个worker，如何强制保证？

**A**: 本地文件锁 + 进程名检查
```rust
pub async fn ensure_singleton_worker(scope: &TaskScope) -> Result<()> {
    let lock_file = format!("/tmp/cis-worker-{}.lock", scope.worker_name());
    
    // 1. 尝试获取文件锁
    let lock = try_lock_exclusive(&lock_file)?;
    
    // 2. 检查是否已有同名进程在运行
    let existing = pgrep(&format!("cis-agent-worker.*{}", scope.worker_name()))?;
    
    if existing && lock.is_none() {
        // 已有其他进程持有锁
        return Err("Worker already running in another process");
    }
    
    // 3. 启动worker，持有锁直到退出
    spawn_worker_process(scope, lock).await
}
```

### Q2: Node-1 崩溃了怎么办？

**A**: 租约过期 + 重新认领
```rust
// DAG认领时有租约时间
let claim = DagClaim {
    node_id: "node-1".to_string(),
    claimed_at: now(),
    lease_expires: now() + Duration::from_secs(300),  // 5分钟租约
};

// 其他节点定期扫描超时的DAG
if now() > claim.lease_expires {
    // 可以重新认领
    self.try_claim(dag_id).await?;
}
```

### Q3: agent-worker是进程还是线程？

**A**: 推荐独立进程，原因：
1. **隔离性**：worker崩溃不影响coordinator
2. **资源清理**：进程退出自动释放资源
3. **监控方便**：OS级别监控进程状态

```rust
// 启动worker进程
Command::new("cis-agent-worker")
    .arg("--scope", "proj-a")
    .arg("--parent-pid", parent_pid.to_string())  // 孤儿进程检测
    .spawn()?;
```

### Q4: 如何避免5cloud单点故障？

**A**: 多入口 + 负载均衡
```
┌─────────┐   ┌─────────┐   ┌─────────┐
│ cloud-1 │   │ cloud-2 │   │ cloud-3 │
└────┬────┘   └────┬────┘   └────┬────┘
     │             │             │
     └─────────────┴─────────────┘
                   │
              ┌─────────┐
              │  Room   │
              └─────────┘
```

任意cloud节点都可作为入口，通过Room广播到所有节点。

---

## 实现建议

### 组件划分
```
cis-core/src/
├── coordinator/
│   ├── mod.rs                    # Coordinator 模块
│   ├── singleton.rs              # SingletonDagCoordinator
│   ├── worker_pool.rs            # Worker管理
│   └── claim.rs                  # 分布式认领协议
│
├── worker/
│   ├── mod.rs                    # AgentWorker
│   ├── executor.rs               # 任务执行
│   └── lifecycle.rs              # 生命周期管理
│
└── protocol/
    └── dag_claim.rs              # 认领消息格式
```

### 配置示例
```toml
[coordinator]
enable_singleton = true
worker_idle_timeout = 300  # 5分钟空闲退出
claim_lease_duration = 300  # 5分钟租约

[worker]
max_concurrent_tasks = 4
scope_isolation = "project"  # project/user/dag_type/global
```

---

## 总结

### ✅ 这个设计的优点

1. **分布式入口**：5cloud作为入口，通过Room广播，可扩展多个入口
2. **智能认领**：基于target标签，特定节点执行特定任务
3. **单例保证**：同一作用域只有一个worker，避免资源竞争
4. **多项目隔离**：不同项目独立worker，互不干扰
5. **故障恢复**：租约过期后可重新认领

### ⚠️ 需要注意的点

1. **认领冲突**：需要CAS或分布式锁防止多节点同时认领
2. **Worker孤儿进程**：父进程崩溃时，worker需要自杀或被收养
3. **状态同步**：公域记忆的写入延迟可能影响状态查询
4. **作用域爆炸**：项目过多时，worker进程数可能过多

### 🎯 推荐实现路径

Phase 1: 基础认领
- 公域记忆CAS认领
- 本地SQLite存储DAG
- 简单worker线程（非进程）

Phase 2: 单例worker
- 文件锁保证单例
- worker进程化
- 租约过期检测

Phase 3: 多作用域
- 项目隔离
- 动态worker生命周期
- 负载均衡优化

这个设计是否满足你的需求？需要我详细展开某个部分吗？
