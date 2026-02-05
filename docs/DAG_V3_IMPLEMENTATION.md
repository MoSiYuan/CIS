# DAG v3 实现总结

## 任务完成清单

### ✅ Task 1.1: DagScope 枚举与 DagSpec 结构
**文件**: `cis-core/src/scheduler/mod.rs`

```rust
pub enum DagScope {
    Global,
    Project { project_id: String, reuse_worker: bool },
    User { user_id: String },
    Type { dag_type: String },
}

pub struct DagSpec {
    pub dag_id: String,
    pub tasks: Vec<DagTaskSpec>,
    pub target_node: Option<String>,
    pub scope: DagScope,
    pub priority: TaskPriority,
    pub version: i64,
}
```

### ✅ Task 1.2: SQLite 表结构扩展
**文件**: `cis-core/src/scheduler/persistence.rs`

新增表：
- `dag_specs`: 存储 DAG 规格（含 scope_type, scope_id, target_node, version）
- 扩展 `dag_runs`: 添加 scope 相关字段

新增方法：
- `save_spec()` / `load_spec()` / `delete_spec()`
- `save_run_simple()` / `save_run()`
- `DagScope::to_db_fields()` / `from_db_fields()`

### ✅ Task 1.3: LocalExecutor 本地执行器
**文件**: `cis-core/src/scheduler/local_executor.rs`

```rust
pub struct LocalExecutor {
    workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
    node_id: String,
    worker_binary: String,
}

impl LocalExecutor {
    pub async fn execute(&self, spec: &DagSpec) -> Result<String>;
    async fn ensure_worker(&self, worker_id: &str, scope: &DagScope) -> Result<String>;
    async fn spawn_worker(&self, worker_id: &str, scope: &DagScope) -> Result<String>;
    async fn dispatch_task(&self, worker_id: &str, room_id: &str, run_id: &str, task: &DagTaskSpec) -> Result<()>;
}
```

### ✅ Task 1.4: dag-executor Skill
**文件**: `skills/dag-executor/src/{lib,worker,error}.rs`

```rust
pub struct DagExecutorSkill {
    worker_manager: WorkerManager,
    nucleus: Mutex<Option<Arc<MatrixNucleus>>>,
    node_id: String,
    worker_binary: String,
}

#[async_trait]
impl Skill for DagExecutorSkill {
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()>;
    async fn execute_dag(&self, spec: DagSpec) -> Result<String>;
}
```

### ✅ Task 2.1: Worker 子命令
**文件**: `cis-node/src/commands/worker.rs`

```bash
# 启动 worker
cis-node worker start --worker-id <ID> --room <ROOM> --scope <SCOPE> --parent-node <NODE>

# 停止 worker
cis-node worker stop <worker-id>

# 查看状态
cis-node worker status [worker-id]
```

### ✅ Task 2.2: Matrix 事件发送集成
**文件**: `skills/dag-executor/src/lib.rs`

- `dispatch_task()` 使用 `nucleus.send_event()` 发送 Task 到 Worker Room
- 依赖 `ruma` crate 处理 Matrix 事件格式

### ✅ Task 2.3: GLM API 接入 dag-executor
**文件**: `cis-core/src/glm/mod.rs`, `cis-core/src/skill/manager.rs`

- `GlmApiState` 持有 `SkillManager`
- `publish_dag` / `confirm_dag` 调用 `dag-executor` skill
- `SkillManager::send_event()` 发送 `dag:execute` 事件

## 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                        用户层                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   GLM API    │  │     CLI      │  │   Matrix Room    │  │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘  │
└─────────┼─────────────────┼───────────────────┼────────────┘
          │                 │                   │
          ▼                 ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                     cis-core 层                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │               GLM API Server                        │    │
│  │  ┌──────────────┐  ┌──────────────┐                │    │
│  │  │  publish_dag │  │  query_dag   │                │    │
│  │  └──────┬───────┘  └──────────────┘                │    │
│  │         │                                          │    │
│  │         ▼                                          │    │
│  │  ┌────────────────────────────────────┐           │    │
│  │  │  skill_manager.send_event(         │           │    │
│  │  │    "dag-executor",                 │           │    │
│  │  │    Event::Custom {                 │           │    │
│  │  │      name: "dag:execute",          │           │    │
│  │  │      data: dag_spec                │           │    │
│  │  │    }                               │           │    │
│  │  └────────────────────────────────────┘           │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                 │
│  ┌────────────────────────┼─────────────────────────────┐  │
│  │    SkillManager        │                             │  │
│  │  ┌─────────────────────┼─────────────────────────┐   │  │
│  │  │ dag-executor skill  │ (Arc<dyn Skill>)        │   │  │
│  │  │ ┌───────────────────▼───────────────────────┐ │   │  │
│  │  │ │ DagExecutorSkill                            │ │   │  │
│  │  │ │ ├─ execute_dag()                            │ │   │  │
│  │  │ │ ├─ ensure_worker()                          │ │   │  │
│  │  │ │ └─ dispatch_task() ──► MatrixNucleus       │ │   │  │
│  │  │ └─────────────────────────────────────────────┘ │   │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────┘  │
│                          │                                    │
│  ┌───────────────────────┼────────────────────────────────┐  │
│  │   Matrix Nucleus      │                                │  │
│  │  ┌────────────────────┼────────────────────────────┐   │  │
│  │  │ send_event()       │                            │   │  │
│  │  │  └─► Room: !worker-{scope}:{node}               │   │  │
│  │  └────────────────────┼────────────────────────────┘   │  │
│  └───────────────────────┼────────────────────────────────┘  │
└──────────────────────────┼────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                     Worker 进程层                            │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  cis-node worker start                                  │ │
│  │   ──► Join Room: !worker-project-xxx:node1             │ │
│  │   ──► Listen events                                     │ │
│  │   ──► Execute tasks                                     │ │
│  │   ──► Report results                                    │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 使用流程

### 1. 发布 DAG
```bash
# HTTP API
curl -X POST http://localhost:6767/api/v1/dag/publish \
  -H "Authorization: Bearer did:cis:node1:abc..." \
  -d '{
    "dag_id": "proj-abc-deploy",
    "tasks": [...],
    "scope": {"type": "project", "project_id": "abc"}
  }'
```

### 2. 确认执行
```bash
curl -X POST http://localhost:6767/api/v1/dag/proj-abc-deploy/confirm \
  -H "Authorization: Bearer did:cis:node1:abc..."
# 返回: {"dag_id": "proj-abc-deploy", "run_id": "dag-run-xxx-uuid"}
```

### 3. Worker 自动创建
- `dag-executor` skill 根据 `scope` 生成 `worker_id`
- 检查 Worker 是否存在，不存在则启动 `cis-node worker`
- Worker 加入 Room: `!worker-project-abc:node1`

### 4. Task 分发执行
- `dispatch_task()` 发送 Matrix 事件到 Worker Room
- Worker 接收事件，执行 Task
- Worker 报告结果

## Worker 隔离策略

| Scope | Worker ID | 隔离级别 |
|-------|-----------|---------|
| Global | `worker-global` | 单节点共享 |
| Project | `worker-project-{id}` | 每项目独立 |
| User | `worker-user-{id}` | 每用户独立 |
| Type | `worker-type-{type}` | 每类型独立 |

## 后续优化方向

1. **Worker 进程监控**: 健康检查、自动重启
2. **Task 结果收集**: Worker 执行完成后回传结果
3. **DAG 状态查询**: 通过 run_id 查询执行状态
4. **Worker 资源限制**: CPU/内存限制
5. **Worker 日志聚合**: 集中收集 Worker 日志
