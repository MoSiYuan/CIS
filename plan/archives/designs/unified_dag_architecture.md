# CIS 统一 DAG 执行架构设计

## 核心观点

**不是两套独立方案，而是三层统一架构**

```
┌─────────────────────────────────────────────────────────────────┐
│                     Unified DagExecutor                          │
│                    (单一入口，多种后端)                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐          │
│   │   Local     │   │   Matrix    │   │   Hybrid    │          │
│   │  Executor   │   │  Executor   │   │  Executor   │          │
│   │  (API直连)   │   │  (Room事件) │   │  (智能混合) │          │
│   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘          │
│          │                 │                 │                 │
│          └─────────────────┼─────────────────┘                 │
│                            │                                   │
│              ┌─────────────▼──────────────┐                   │
│              │     SessionManager         │                   │
│              │     (Agent Cluster)        │                   │
│              └────────────────────────────┘                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 架构分层

### Layer 1: 统一执行器 (UnifiedDagExecutor)

**职责**: 用户唯一入口，路由到不同后端

```rust
pub struct UnifiedDagExecutor {
    /// 执行模式
    mode: ExecutionMode,
    /// 本地执行器
    local: Option<AgentClusterExecutor>,
    /// Matrix 执行器
    matrix: Option<MatrixDagExecutor>,
    /// 节点 ID
    node_id: String,
}

pub enum ExecutionMode {
    /// 本地执行（直接 API）
    Local,
    /// Matrix Room 事件驱动
    Matrix { room_id: String },
    /// 混合模式（本地执行 + Matrix 广播）
    Hybrid { room_id: String },
    /// 联邦模式（可接收远程任务）
    Federation { coordinator: Option<String> },
}
```

### Layer 2: 执行后端 (Executor Backend)

#### 2.1 Local Executor (API 直连)

```rust
pub struct LocalExecutor {
    session_manager: &'static SessionManager,
    context_store: ContextStore,
    max_workers: usize,
}

impl LocalExecutor {
    /// 直接创建 session 并执行
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        // 已有实现：直接调用 SessionManager
        // 1. 创建 AgentSession
        // 2. 启动 PTY
        // 3. 监控状态
    }
}
```

**适用场景**: 
- 单机开发
- CI/CD 流水线
- 不需要跨节点协调

#### 2.2 Matrix Executor (Room 事件)

```rust
pub struct MatrixExecutor {
    /// 监听的 Room ID
    room_id: RoomId,
    /// Matrix Nucleus
    nucleus: Arc<MatrixNucleus>,
    /// 本地任务队列（从 Room 接收）
    task_queue: mpsc::Receiver<MatrixDagTask>,
    /// 是否作为协调器
    is_coordinator: bool,
}

impl MatrixExecutor {
    /// 启动 Room 监听
    pub async fn start_listening(&self) -> Result<()> {
        // 1. 加入 Room
        // 2. 监听 cis.dag.task.assigned 事件
        // 3. 认领分配给本节点的任务
        // 4. 执行并广播结果
    }
    
    /// 广播任务分配
    pub async fn broadcast_task_assignment(
        &self,
        run_id: &str,
        task_id: &str,
        target_node: &str,
    ) -> Result<()> {
        let event = MatrixEvent::new(
            self.room_id.clone(),
            EventId::generate(),
            self.user_id(),
            "cis.dag.task.assigned",
            json!({
                "run_id": run_id,
                "task_id": task_id,
                "assigned_to": target_node,
                "timestamp": Utc::now(),
            }),
        );
        
        self.nucleus.send_event(event).await
    }
}
```

**适用场景**:
- 分布式 DAG 执行
- 多节点协作
- 需要故障转移

#### 2.3 Hybrid Executor (推荐)

```rust
pub struct HybridExecutor {
    /// 本地执行器（实际执行任务）
    local: LocalExecutor,
    /// Matrix 广播器（状态同步）
    broadcaster: DagMatrixBroadcaster,
    /// 是否启用远程任务接收
    accept_remote: bool,
}

impl HybridExecutor {
    /// 执行本地任务 + 广播状态
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        // 1. 本地执行
        let report = self.local.execute_run(run).await?;
        
        // 2. 广播关键状态到 Matrix Room
        self.broadcaster.broadcast_run_completed(&report).await?;
        
        Ok(report)
    }
    
    /// 接收远程任务分配
    pub async fn handle_remote_assignment(&self, event: MatrixEvent) -> Result<()> {
        // 1. 验证任务分配给本节点
        // 2. 创建本地 session 执行
        // 3. 广播执行结果
    }
}
```

**适用场景** (推荐默认):
- 本地执行性能好
- Matrix 广播用于监控和协调
- 支持分布式扩展

### Layer 3: SessionManager (统一会话层)

无论上层使用哪种模式，最终都通过 **SessionManager** 管理 AgentSession：

```rust
/// 全局单例，所有 Executor 共享
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<SessionId, Arc<RwLock<AgentSession>>>>>,
    event_broadcaster: EventBroadcaster,  // 本地广播
    matrix_broadcaster: Option<MatrixBroadcaster>,  // Matrix 广播（可选）
}
```

---

## 执行流程对比

### 场景 1: 本地开发 (Local Mode)

```
User
 │
 │ cis dag run dag.toml
 │
 ▼
LocalExecutor
 │
 ├─► SessionManager::create_session()
 │
 ├─► AgentSession::start()
 │       ├─► PTY + Claude Process
 │       └─► 监控循环
 │
 └─► 直接返回结果
```

### 场景 2: 分布式协作 (Matrix Mode)

```
Coordinator Node                    Worker Node A
 │                                      │
 │ 1. Create Room                       │
 │ 2. Broadcast: Task-1 → Node A        │
 │─────────────────────────────────────>│
 │                                      │
 │                                      │ 3. 接收任务分配
 │                                      │ 4. LocalExecutor::execute()
 │                                      │
 │ 5. 广播: Task-1 Running              │
 │<─────────────────────────────────────│
 │                                      │
 │                                      │ 6. 执行中...
 │                                      │
 │ 7. 广播: Task-1 Completed            │
 │<─────────────────────────────────────│
 │                                      │
 │ 8. Broadcast: Task-2 → Node B        │
 │─────────────────────────────────────┼──────────────────────► Worker Node B
```

### 场景 3: 混合模式 (Hybrid Mode - 推荐)

```
User/CLI
 │
 │ cis dag execute --hybrid --room !dag-status:cis.local
 │
 ▼
HybridExecutor
 │
 ├─► LocalExecutor::execute_run()  (本地执行，高性能)
 │       │
 │       ├─► SessionManager::create_session()
 │       │
 │       └─► 监控状态变更
 │
 ├─► DagMatrixBroadcaster::broadcast_task_status()
 │       │
 │       ├─► 本地订阅者 (GUI/CLI) 实时更新
 │       └─► 远程节点 同步状态
 │
 └─► 返回执行结果
```

---

## 配置示例

### config.toml

```toml
[dag]
# 默认执行模式
execution_mode = "hybrid"  # local | matrix | hybrid | federation

# 本地执行配置
[dag.local]
max_workers = 4
default_agent = "claude"

# Matrix 配置
[dag.matrix]
room_prefix = "!dag-"
home_server = "cis.local"
broadcast_interval_ms = 500

# 混合模式配置
[dag.hybrid]
# 本地执行，Matrix 广播
broadcast_status = true
broadcast_output = false  # 大输出不广播，通过 API 获取
accept_remote_tasks = true  # 是否接受远程分配的任务
```

### CLI 使用

```bash
# 1. 纯本地模式（快速，单机）
$ cis dag execute --local dag.toml

# 2. Matrix 模式（纯事件驱动，适合分布式）
$ cis dag execute --matrix --room !dag-project:cis.local dag.toml

# 3. 混合模式（推荐：本地执行 + Matrix 广播）
$ cis dag execute --hybrid --room !dag-status:cis.local dag.toml

# 4. 作为协调器，分配任务到多个节点
$ cis dag execute --federation --workers node-a,node-b,node-c dag.toml
```

---

## 核心整合点

### 1. SessionManager 扩展

```rust
impl SessionManager {
    /// 创建 session（所有模式共用）
    pub async fn create_session(...) -> Result<SessionId> {
        // ... 创建 AgentSession ...
        
        // 可选：广播到 Matrix
        if let Some(ref matrix) = self.matrix_broadcaster {
            matrix.broadcast_session_created(&session_id).await?;
        }
        
        Ok(session_id)
    }
    
    /// 状态变更时广播
    pub async fn update_state(&self, id: &SessionId, state: SessionState) {
        // 本地广播
        self.event_broadcaster.send(SessionEvent::StateChanged { ... });
        
        // Matrix 广播（如果启用）
        if let Some(ref matrix) = self.matrix_broadcaster {
            matrix.broadcast_state_changed(id, &state).await;
        }
    }
}
```

### 2. 事件映射

| 本地事件 | Matrix 事件 | 说明 |
|----------|-------------|------|
| `SessionEvent::Created` | `cis.dag.session.created` | Session 创建 |
| `SessionEvent::StateChanged` | `cis.dag.session.state` | 状态变更 |
| `SessionEvent::OutputUpdated` | `cis.dag.session.output` | 输出更新（采样）|
| `SessionEvent::Blocked` | `cis.dag.session.blocked` | 卡点通知 |
| `SessionEvent::Completed` | `cis.dag.session.completed` | 完成通知 |

### 3. 向后兼容

```rust
/// 现有代码无需修改
pub async fn existing_function() {
    // 直接使用 SessionManager，不关心上层模式
    let manager = SessionManager::global();
    let session = manager.create_session(...).await?;
    // ...
}
```

---

## 推荐演进路径

### Phase 1: 当前状态 ✅
- LocalExecutor 完成（Agent Cluster）
- SessionManager 全局单例
- CLI 直接调用

### Phase 2: 添加 Matrix 广播 (1-2 天)
```rust
// 在 SessionManager 中添加可选的 Matrix 广播
pub struct SessionManager {
    // ... 现有字段 ...
    matrix_broadcaster: Option<MatrixBroadcaster>,  // 新增
}

// Hybrid 模式 = Local 执行 + Matrix 广播
```

### Phase 3: Matrix Executor (2-3 天)
- Room 监听
- 任务分配/认领
- 跨节点执行

### Phase 4: 联邦协调器 (3-5 天)
- 智能任务分配
- 故障转移
- 负载均衡

---

## 总结

| 模式 | 执行位置 | 状态广播 | 适用场景 |
|------|----------|----------|----------|
| **Local** | 本地 | 本地广播 | 单机开发、CI/CD |
| **Matrix** | 分布式 | Room 事件 | 纯分布式、无中心节点 |
| **Hybrid** ⭐ | 本地 | 本地 + Room | 推荐默认，兼顾性能和可观测性 |
| **Federation** | 智能分配 | Room 事件 | 大规模集群、自动故障转移 |

**核心设计原则**:
1. **SessionManager 是唯一的**，所有模式共享 AgentSession 管理
2. **Matrix 是可选的增强**，不是替代
3. **Hybrid 是推荐默认**，本地执行性能好，Matrix 用于监控
4. **向后兼容**，现有代码无需修改即可工作
