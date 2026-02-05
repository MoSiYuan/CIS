# DAG Skill 触发机制设计

## 核心问题澄清

### Q: DAG怎么被触发？
**A: DAG本身就是一种Skill**，通过 `!dag` 命令或HTTP接口触发。

```
5cloud接收任务 ──▶ 推送到目标节点 ──▶ 节点的DAG Skill ──▶ 按作用域分配agent-worker
```

---

## 1. DAG作为Skill的定义

### 1.1 Skill Manifest 定义

```yaml
# skills/dag-executor/skill.yaml
skill:
  id: dag-executor
  name: DAG Executor
  type: native
  
  # 触发方式
  triggers:
    - type: command
      pattern: "!dag"
    - type: http
      path: "/api/v1/dag/execute"
    - type: matrix_event
      event_type: "io.cis.dag.execute"
  
  # 执行配置
  execution:
    # 是否单例（全局只有一个实例运行）
    singleton: false
    
    # 作用域隔离级别
    scope_isolation: project  # project / user / dag_type / none
    
    # Worker生命周期
    worker:
      idle_timeout: 300  # 5分钟空闲退出
      max_concurrent: 4  # 每个worker最大并发任务
```

### 1.2 DAG定义中的作用域声明

```rust
/// DAG定义时指定作用域
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagDefinition {
    pub dag_id: String,
    pub tasks: Vec<DagTask>,
    
    // === 调度相关 ===
    /// 目标节点（可选，不指定则任意节点可认领）
    pub target_node: Option<String>,
    
    /// 作用域（决定worker隔离级别）
    pub scope: DagScope,
    
    /// 优先级
    pub priority: TaskPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DagScope {
    /// 全局共享worker
    Global,
    
    /// 按项目隔离（推荐）
    Project { 
        project_id: String,
        // 是否复用现有worker
        reuse_worker: bool,
    },
    
    /// 按用户隔离
    User { 
        user_id: String,
    },
    
    /// 按DAG类型隔离
    Type { 
        dag_type: String,  // "backup", "deploy", "test"
    },
}

impl DagScope {
    /// 生成worker标识
    pub fn worker_id(&self) -> String {
        match self {
            DagScope::Global => "worker-global".to_string(),
            DagScope::Project { project_id, .. } => format!("worker-project-{}", project_id),
            DagScope::User { user_id } => format!("worker-user-{}", user_id),
            DagScope::Type { dag_type } => format!("worker-type-{}", dag_type),
        }
    }
    
    /// 从DAG内容自动推断作用域
    pub fn infer_from_dag(dag: &DagDefinition) -> Self {
        // 优先级：显式指定 > 从任务推断 > 默认值
        
        // 1. 检查是否有project相关任务
        let has_project_tasks = dag.tasks.iter().any(|t| {
            t.command.contains("--project") || 
            t.env.contains_key("PROJECT_ID")
        });
        
        if has_project_tasks {
            // 从任务中提取project_id
            let project_id = dag.tasks.iter()
                .find_map(|t| t.env.get("PROJECT_ID"))
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            
            return DagScope::Project { 
                project_id, 
                reuse_worker: true 
            };
        }
        
        // 2. 根据DAG类型推断
        if dag.dag_id.contains("backup") {
            return DagScope::Type { dag_type: "backup".to_string() };
        }
        if dag.dag_id.contains("deploy") {
            return DagScope::Type { dag_type: "deploy".to_string() };
        }
        
        // 3. 默认全局
        DagScope::Global
    }
}
```

---

## 2. 作用域的确定方式

### 2.1 方式一：发布时显式指定（推荐）

```json
// GLM推送时显式指定
{
  "dag_id": "backup-proj-a",
  "scope": {
    "type": "Project",
    "project_id": "proj-a",
    "reuse_worker": true
  },
  "target_node": "node-1",
  "tasks": [...]
}
```

### 2.2 方式二：从任务环境变量推断

```json
// 任务中包含PROJECT_ID，自动推断
{
  "dag_id": "backup-daily",
  "tasks": [
    {
      "id": "backup",
      "type": "shell",
      "command": "./backup.sh",
      "env": {
        "PROJECT_ID": "proj-a",  // 自动提取
        "BACKUP_TARGET": "/data/proj-a"
      }
    }
  ]
}
// 推断结果：scope = Project { project_id: "proj-a" }
```

### 2.3 方式三：从DAG ID前缀推断

```rust
// dag_id 命名约定
// "proj-a-backup-daily" -> scope = Project { project_id: "proj-a" }
// "backup-global" -> scope = Global
// "user-123-task" -> scope = User { user_id: "123" }

pub fn parse_scope_from_id(dag_id: &str) -> DagScope {
    let parts: Vec<&str> = dag_id.split('-').collect();
    
    if parts.len() >= 2 && parts[0] == "proj" {
        return DagScope::Project { 
            project_id: parts[1].to_string(),
            reuse_worker: true,
        };
    }
    
    if parts.len() >= 2 && parts[0] == "user" {
        return DagScope::User { 
            user_id: parts[1].to_string(),
        };
    }
    
    DagScope::Global
}
```

### 2.4 方式四：发布者身份推断

```rust
// 根据发布者（GLM Agent）的身份推断
pub fn infer_from_creator(creator_did: &str) -> DagScope {
    // 假设GLM Agent DID格式：did:cis:agent:{user_id}:{random}
    let parts: Vec<&str> = creator_did.split(':').collect();
    if parts.len() >= 4 {
        return DagScope::User { 
            user_id: parts[3].to_string(),
        };
    }
    DagScope::Global
}
```

---

## 3. 完整触发流程

### 3.1 流程图

```
┌──────────────────────────────────────────────────────────────────────┐
│                           触发流程                                     │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  5cloud (入口)                                                        │
│     │                                                                │
│     │ 1. 接收DAG (HTTP/Room)                                          │
│     │    dag_id: "proj-a-backup"                                       │
│     │    scope: 未指定                                                │
│     ▼                                                                │
│  ┌─────────────────┐                                                  │
│  │ 推断作用域       │                                                  │
│  │ ─────────────── │                                                  │
│  │ • 检查显式scope  │ 无                                               │
│  │ • 检查env PROJECT_ID: "proj-a" ✓                                  │
│  │ • 推断: scope = Project { project_id: "proj-a" }                  │
│  └────────┬────────┘                                                  │
│           │                                                          │
│           │ 2. 确定目标节点                                            │
│           │    target_node: "node-1" (显式指定)                        │
│           ▼                                                          │
│  ┌─────────────────┐                                                  │
│  │ 推送DAG          │                                                  │
│  │ ─────────────── │                                                  │
│  │ 方式A: HTTP直接   │ ──▶ POST http://node-1:6767/api/v1/dag/execute  │
│  │ 方式B: Room广播   │ ──▶ Room: !tasks:cis "新DAG: proj-a-backup"     │
│  └────────┬────────┘                                                  │
│           │                                                          │
│           ▼                                                          │
│  Node-1 (目标节点)                                                    │
│     │                                                                │
│     │ 3. DAG Skill接收                                                │
│     │    "!dag execute proj-a-backup"                                 │
│     ▼                                                                │
│  ┌──────────────────────────────────────────┐                       │
│  │ DAG Skill (dag-executor)                  │                       │
│  │ ─────────────────────────                 │                       │
│  │                                           │                       │
│  │ 4. 解析DAG得到scope: Project {            │                       │
│  │      project_id: "proj-a"                 │                       │
│  │    }                                      │                       │
│  │                                           │                       │
│  │ 5. 计算worker_id: "worker-project-proj-a" │                       │
│  │                                           │                       │
│  │ 6. 检查worker是否存在？                   │                       │
│  │    ├── 存在 → 复用                        │                       │
│  │    └── 不存在 → 启动新worker              │                       │
│  │                                           │                       │
│  │ 7. 发送DAG到worker的队列                  │                       │
│  │    worker.submit(dag)                     │                       │
│  └────────┬──────────────────────────────────┘                       │
│           │                                                          │
│           ▼                                                          │
│  ┌──────────────────────────────────────────┐                       │
│  │ agent-worker-project-proj-a (进程/线程)   │                       │
│  │ ─────────────────────────────────────     │                       │
│  │                                           │                       │
│  │ 8. 从队列取DAG                            │                       │
│  │ 9. 拓扑排序执行任务                        │                       │
│  │    task-1 (shell) ──▶ task-2 (skill)      │                       │
│  │ 10. 更新状态到SQLite                       │                       │
│  │ 11. 广播进度到Room（可选）                  │                       │
│  │                                           │                       │
│  └──────────────────────────────────────────┘                       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.2 代码实现

```rust
// cis-core/src/skill/dag_executor/mod.rs

pub struct DagSkill {
    node_id: String,
    db: Arc<Mutex<Connection>>,
    workers: Arc<Mutex<HashMap<String, WorkerHandle>>>,
}

impl Skill for DagSkill {
    fn skill_id(&self) -> &str {
        "dag-executor"
    }
    
    /// 处理触发（HTTP/Room/CLI）
    async fn handle_trigger(&self, trigger: SkillTrigger) -> Result<SkillResponse> {
        match trigger {
            SkillTrigger::Http { body, .. } => {
                let dag: DagDefinition = serde_json::from_str(&body)?;
                self.execute_dag(dag).await
            }
            
            SkillTrigger::MatrixEvent { event } => {
                let dag_event: DagRoomEvent = serde_json::from_str(&event.content)?;
                match dag_event {
                    DagRoomEvent::Execute { dag_id, tasks, scope, .. } => {
                        let dag = DagDefinition {
                            dag_id,
                            tasks,
                            scope: scope.unwrap_or_else(|| DagScope::Global),
                            ..Default::default()
                        };
                        self.execute_dag(dag).await
                    }
                    _ => Ok(SkillResponse::ignored()),
                }
            }
            
            SkillTrigger::Cli { args } => {
                // CLI直接执行本地DAG文件
                let dag_file = std::fs::read_to_string(&args[0])?;
                let dag: DagDefinition = serde_json::from_str(&dag_file)?;
                self.execute_dag(dag).await
            }
        }
    }
}

impl DagSkill {
    /// 核心：执行DAG
    async fn execute_dag(&self, dag: DagDefinition) -> Result<SkillResponse> {
        // 1. 确定作用域（如果未指定则推断）
        let scope = match dag.scope {
            DagScope::Global if dag.dag_id.contains("proj-") => {
                DagScope::infer_from_dag(&dag)
            }
            _ => dag.scope,
        };
        
        let worker_id = scope.worker_id();
        
        // 2. 获取或创建worker
        let worker = self.get_or_create_worker(&worker_id, &scope).await?;
        
        // 3. 提交DAG到worker
        worker.submit(dag).await?;
        
        Ok(SkillResponse::success(format!(
            "DAG submitted to worker {}", worker_id
        )))
    }
    
    /// 获取或创建worker
    async fn get_or_create_worker(
        &self,
        worker_id: &str,
        scope: &DagScope
    ) -> Result<WorkerHandle> {
        let mut workers = self.workers.lock().await;
        
        if let Some(worker) = workers.get(worker_id) {
            if worker.is_alive() {
                return Ok(worker.clone());
            }
            // Worker已死，移除
            workers.remove(worker_id);
        }
        
        // 创建新worker
        let worker = self.spawn_worker(worker_id, scope).await?;
        workers.insert(worker_id.to_string(), worker.clone());
        
        Ok(worker)
    }
    
    /// 启动worker进程
    async fn spawn_worker(&self, worker_id: &str, scope: &DagScope) -> Result<WorkerHandle> {
        // 使用文件锁保证单例
        let lock_file = format!("/tmp/cis-worker-{}.lock", worker_id);
        let lock = FileLock::acquire(&lock_file)?;
        
        // 启动子进程
        let child = Command::new("cis-agent-worker")
            .arg("--worker-id", worker_id)
            .arg("--scope", &scope.to_string())
            .arg("--parent-pid", &std::process::id().to_string())
            .spawn()?;
        
        Ok(WorkerHandle {
            id: worker_id.to_string(),
            process: child,
            _lock: lock,
        })
    }
}
```

---

## 4. 5cloud推送的具体实现

### 4.1 HTTP推送（明确目标节点）

```rust
// 5cloud节点收到GLM请求后，推送到指定节点
pub async fn forward_to_target_node(
    &self,
    dag: DagDefinition,
    target_node: &str
) -> Result<()> {
    // 1. 解析目标节点地址
    let node_endpoint = self.resolve_node(target_node).await?;
    
    // 2. 直接HTTP推送
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/v1/dag/execute", node_endpoint))
        .header("Authorization", format!("Bearer {}", self.auth_token))
        .json(&dag)
        .send()
        .await?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("Target node returned error: {}", response.status()))
    }
}
```

### 4.2 Room广播（目标节点自选）

```rust
// 通过Room广播，任意节点可认领
pub async fn broadcast_via_room(&self, dag: DagDefinition) -> Result<()> {
    let event = DagRoomEvent::Execute {
        dag_id: dag.dag_id.clone(),
        tasks: dag.tasks.clone(),
        scope: Some(dag.scope.clone()),
        target_node: dag.target_node.clone(),
        timestamp: now(),
    };
    
    // 发送到Matrix Room
    self.matrix_room.send(
        serde_json::to_string(&event)?,
        MessageType::Text,
    ).await?;
    
    Ok(())
}

// 节点端接收处理
impl DagSkill {
    async fn handle_room_event(&self, event: RoomMessage) -> Result<()> {
        let dag_event: DagRoomEvent = serde_json::from_str(&event.content)?;
        
        // 检查是否匹配本节点
        if let Some(ref target) = dag_event.target_node {
            if target != &self.node_id {
                // 不匹配的节点忽略
                return Ok(());
            }
        }
        
        // 匹配，执行DAG
        self.execute_dag(dag_event.to_dag()).await
    }
}
```

---

## 5. 作用域确定流程总结

```
DAG发布
   │
   ├── 1. 显式指定 scope: { "type": "Project", "project_id": "proj-a" }
   │      └── 直接使用 ✓
   │
   ├── 2. 未指定，从任务env推断
   │      └── tasks[0].env.PROJECT_ID = "proj-a"
   │      └── scope = Project { project_id: "proj-a" } ✓
   │
   ├── 3. 未指定，从dag_id推断
   │      └── dag_id = "proj-a-backup-daily"
   │      └── scope = Project { project_id: "proj-a" } ✓
   │
   └── 4. 都未匹配
          └── scope = Global (默认)
```

---

## 6. 关键问答

### Q1: 一个节点可以有多个worker吗？
**A**: 可以，不同作用域对应不同worker：
```
node-1:
  ├── worker-project-proj-a  (处理proj-a的所有DAG)
  ├── worker-project-proj-b  (处理proj-b的所有DAG)
  └── worker-type-backup     (处理所有backup类型DAG)
```

### Q2: 相同作用域的DAG怎么排队？
**A**: 同一个worker内部有队列：
```rust
pub struct Worker {
    scope: DagScope,
    queue: VecDeque<DagDefinition>,  // 同scope的DAG排队
    current: Option<DagExecution>,
}
```

### Q3: 如何限制worker数量？
**A**: 配置最大worker数，LRU淘汰：
```rust
if workers.len() >= MAX_WORKERS {
    // 找空闲最久的worker关闭
    let oldest = workers.values()
        .filter(|w| w.is_idle())
        .min_by_key(|w| w.last_active());
    
    if let Some(w) = oldest {
        w.shutdown().await?;
        workers.remove(&w.id);
    }
}
```

### Q4: CLI怎么查询worker状态？
**A**: 直接读SQLite：
```bash
cis dag workers                    # 列出所有workercis dag worker worker-project-proj-a status  # 查看具体worker
```

这个设计是否清晰？需要我展开哪部分代码？
