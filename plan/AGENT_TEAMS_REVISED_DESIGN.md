# CIS Agent Teams 修正设计

**基于现有架构的深入分析**

---

## 1. 现有架构盘点

### 1.1 当前 Agent 调用层次

```
┌─────────────────────────────────────────────────────────────────┐
│                    CIS 现有 Agent 架构                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Layer 3: AgentProvider (单次执行)                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  ClaudeProvider::execute(req) -> Result<AgentResponse>   │  │
│  │  OpenCodeProvider::execute(req) -> Result<AgentResponse> │  │
│  │                                                          │  │
│  │  - 单次调用，无状态                                       │
│  │  - execute_stream 支持流式                                │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              ▲                                  │
│                              │ 调用                              │
│  Layer 2: Agent Cluster (并发执行)                               │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  AgentClusterExecutor                                    │  │
│  │       │                                                  │  │
│  │       ▼                                                  │  │
│  │  SessionManager (单例)                                    │  │
│  │  ├─ AgentSession (PTY + 进程)                             │  │
│  │  ├─ AgentSession (PTY + 进程)                             │  │
│  │  └─ AgentSession (PTY + 进程)                             │  │
│  │                                                          │  │
│  │  - 多 Agent 并发（每个 DAG Task 一个 Session）            │  │
│  │  - PTY 交互式会话                                         │
│  │  - EventBus (tokio::broadcast) 状态同步                   │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              ▲                                  │
│                              │ 创建                              │
│  Layer 1: AgentCommandConfig (命令构建)                          │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  AgentCommandConfig::claude()                            │  │
│  │      -> CommandBuilder("claude", ["--dangerously-skip...│  │
│  │                                                          │  │
│  │  AgentCommandConfig::opencode()                          │  │
│  │      -> CommandBuilder("opencode", ["run", "--format"..│  │
│  │                                                          │  │
│  │  - 仅配置命令行参数                                       │
│  │  - requires_pty: true                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 当前问题

| 问题 | 现状 | 目标 |
|------|------|------|
| **Agent 生命周期** | 单次执行（执行完即销毁） | 支持持久化 Agent，可复用 |
| **Agent 间通信** | 无（各自独立） | 支持 Mailbox 消息传递 |
| **DAG 选 Agent** | 只能选 default_agent | 每个 Task 可指定不同 Agent |
| **前后台切换** | 不支持 | OpenCode `-p` 模式支持 |
| **Claude Agent** | 简单调用 | 复用 Claude 内部 Agent 能力 |

---

## 2. 修正设计

### 2.1 核心思路

**不要重写，要扩展！**

```
现有架构                    扩展后
    │                          │
    ▼                          ▼
┌──────────────┐        ┌──────────────────┐
│ AgentSession │   ->   │ PersistentAgent  │
│   (单次)      │        │   (可复用)        │
└──────────────┘        └──────────────────┘
    │                          │
    │ 销毁                      │ 保留
    ▼                          ▼
  结束                    ┌──────────┐
                          │ 等待消息  │
                          └──────────┘
```

### 2.2 OpenCode `-p` 模式利用

OpenCode 的 `-p` 参数可以让进程后台运行并通过 socket 通信：

```rust
/// OpenCode 持久化 Agent
pub struct OpenCodePersistentAgent {
    agent_id: String,
    socket_path: PathBuf,
    pid: u32,
}

impl OpenCodePersistentAgent {
    /// 启动后台 Agent
    pub async fn start(config: AgentConfig) -> Result<Self> {
        let agent_id = format!("opencode-agent-{}", uuid::Uuid::new_v4());
        let socket_path = get_work_dir().join(format!("{}.sock", agent_id));
        
        // opencode run -p: 后台模式 + socket 通信
        let mut cmd = Command::new("opencode");
        cmd.arg("run")
            .arg("-p")  // 后台模式！
            .arg("--socket").arg(&socket_path)
            .arg("--model").arg(&config.model.unwrap_or_default());
        
        let child = cmd.spawn()?;
        
        // 等待 socket 就绪
        wait_for_socket(&socket_path).await?;
        
        Ok(Self {
            agent_id,
            socket_path,
            pid: child.id().unwrap(),
        })
    }
    
    /// 发送任务（复用连接）
    pub async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        
        // 发送任务
        let request = serde_json::to_vec(&task)?;
        stream.write_all(&request).await?;
        
        // 读取响应
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer).await?;
        
        let result: TaskResult = serde_json::from_slice(&buffer)?;
        Ok(result)
    }
    
    /// 前台attach（用于调试）
    pub async fn attach(&self) -> Result<()> {
        // opencode attach <pid>
        Command::new("opencode")
            .args(&["attach", &self.pid.to_string()])
            .status().await?;
        Ok(())
    }
    
    /// 后台detach（返回 socket 模式）
    pub async fn detach(&self) -> Result<()> {
        // 发送 detach 信号
        // 进程继续后台运行
        Ok(())
    }
}
```

### 2.3 Claude Agent 复用现有逻辑

Claude 没有 `-p` 模式，但可以利用现有的 AgentSession 架构：

```rust
/// Claude 持久化 Agent（复用 AgentSession）
pub struct ClaudePersistentAgent {
    session_id: SessionId,
    session_manager: &'static SessionManager,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl ClaudePersistentAgent {
    /// 创建持久化 Agent（不自动销毁）
    pub async fn start(
        manager: &'static SessionManager,
        config: AgentConfig,
    ) -> Result<Self> {
        let session_id = manager.create_session(
            "persistent",  // 特殊 run_id 表示持久化
            &config.name,
            AgentType::Claude,
            &config.system_prompt.unwrap_or_default(),
            &config.work_dir,
            "",  // no upstream context
        ).await?;
        
        // 获取 session 的 input_tx
        let input_tx = manager.get_input_tx(&session_id).await?;
        
        Ok(Self {
            session_id,
            session_manager: manager,
            input_tx,
        })
    }
    
    /// 执行任务（通过 PTY 发送命令）
    pub async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        // 构造任务命令
        let command = format!("\n{}\n", task.prompt);
        
        // 通过 PTY 发送
        self.input_tx.send(command.into_bytes())?;
        
        // 等待输出（通过 EventBus 订阅）
        let output = self.wait_for_output().await?;
        
        Ok(TaskResult {
            success: true,
            output,
            ..Default::default()
        })
    }
    
    /// 前台 attach（复用现有 attach 功能）
    pub async fn attach(&self) -> Result<AttachHandle> {
        self.session_manager.attach(&self.session_id).await
    }
    
    /// 销毁 Agent
    pub async fn destroy(self) -> Result<()> {
        self.session_manager.kill_session(&self.session_id, "User requested").await?;
        Ok(())
    }
}
```

### 2.4 统一 Agent Pool

```rust
/// Agent 池管理器
pub struct AgentPool {
    /// Runtime 注册表
    runtimes: HashMap<RuntimeType, Box<dyn AgentRuntime>>,
    
    /// 持久化 Agent 实例
    agents: Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>>,
    
    /// 最大并发数
    max_concurrent: usize,
}

#[async_trait]
pub trait PersistentAgent: Send + Sync {
    fn agent_id(&self) -> &str;
    fn runtime_type(&self) -> RuntimeType;
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult>;
    async fn attach(&self) -> Result<()>;
    async fn status(&self) -> AgentStatus;
}

impl AgentPool {
    /// 获取或创建 Agent
    pub async fn acquire(&self, config: AgentAcquireConfig) -> Result<AgentHandle> {
        // 1. 检查是否有可用 Agent
        if let Some(agent_id) = &config.reuse_agent_id {
            if let Some(agent) = self.agents.read().await.get(agent_id) {
                return Ok(AgentHandle::new(agent_id.clone(), self.agents.clone()));
            }
        }
        
        // 2. 创建新 Agent
        let runtime = self.runtimes.get(&config.runtime_type)
            .ok_or(Error::RuntimeNotFound)?;
        
        let agent = runtime.create_persistent(config).await?;
        let agent_id = agent.agent_id().to_string();
        
        self.agents.write().await.insert(agent_id.clone(), agent);
        
        Ok(AgentHandle::new(agent_id, self.agents.clone()))
    }
    
    /// 释放 Agent（根据策略决定是否销毁）
    pub async fn release(&self, handle: AgentHandle, keep: bool) -> Result<()> {
        if !keep {
            self.agents.write().await.remove(&handle.agent_id);
        }
        Ok(())
    }
}
```

---

## 3. DAG 扩展

### 3.1 Task 结构扩展

```rust
/// DAG 任务定义（扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTask {
    pub task_id: String,
    pub dependencies: Vec<String>,
    
    // 现有字段
    pub skill_id: Option<String>,
    pub level: TaskLevel,
    
    // === 新增：Agent 选择 ===
    /// 指定 Runtime 类型
    #[serde(default)]
    pub agent_runtime: Option<RuntimeType>,
    
    /// 复用已有 Agent ID
    #[serde(default)]
    pub reuse_agent: Option<String>,
    
    /// 是否保持 Agent（执行后不销毁）
    #[serde(default = "default_keep_agent")]
    pub keep_agent: bool,
    
    /// Agent 配置（创建新 Agent 时用）
    #[serde(default)]
    pub agent_config: Option<AgentConfig>,
}

fn default_keep_agent() -> bool {
    false
}
```

### 3.2 MultiAgentDagExecutor

```rust
/// 支持多 Agent 的 DAG 执行器
pub struct MultiAgentDagExecutor {
    scheduler: DagScheduler,
    agent_pool: AgentPool,
    context_store: ContextStore,
}

impl MultiAgentDagExecutor {
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        // 存储此 run 使用的 Agent
        let mut run_agents: HashMap<String, AgentHandle> = HashMap::new();
        
        loop {
            let ready_tasks = self.scheduler.get_ready_tasks(&run.run_id)?;
            
            for task_id in ready_tasks {
                let task = self.scheduler.get_task(&run.run_id, &task_id)?;
                
                // 获取或创建 Agent
                let agent = self.get_agent(&task, &mut run_agents).await?;
                
                // 执行任务
                let result = agent.execute(TaskRequest {
                    prompt: build_prompt(&task),
                    context: self.gather_context(run, &task).await?,
                }).await?;
                
                // 处理结果
                self.handle_result(run, &task_id, result).await?;
                
                // 释放 Agent
                let keep = task.keep_agent;
                self.agent_pool.release(agent, keep).await?;
            }
            
            if self.is_run_complete(run) {
                break;
            }
        }
        
        Ok(self.build_report(run).await?)
    }
    
    async fn get_agent(
        &self,
        task: &DagTask,
        run_agents: &mut HashMap<String, AgentHandle>,
    ) -> Result<AgentHandle> {
        // 1. 复用同 run 的 Agent
        if let Some(agent_id) = &task.reuse_agent {
            if let Some(agent) = run_agents.get(agent_id) {
                return Ok(agent.clone());
            }
        }
        
        // 2. 从 Pool 获取
        let config = AgentAcquireConfig {
            runtime_type: task.agent_runtime.unwrap_or_default(),
            reuse_agent_id: task.reuse_agent.clone(),
            agent_config: task.agent_config.clone(),
        };
        
        let agent = self.agent_pool.acquire(config).await?;
        
        // 3. 记录到 run_agents
        run_agents.insert(agent.agent_id.clone(), agent.clone());
        
        Ok(agent)
    }
}
```

---

## 4. 使用示例

### 4.1 YAML 配置

```yaml
# multi-agent-workflow.yaml
dag:
  name: "代码生成+审查+修复"
  
  tasks:
    # Task 1: 用 Claude 生成代码
    - task_id: "generate"
      name: "生成代码"
      agent_runtime: "claude"
      agent_config:
        name: "code-generator"
        system_prompt: "你是专业 Rust 开发者"
      keep_agent: true  # 保持 Agent 供后续使用
      skill_id: "generate-code"
      
    # Task 2: 用 OpenCode 审查
    - task_id: "review"
      name: "代码审查"
      agent_runtime: "opencode"
      agent_config:
        model: "opencode/glm-4.7-free"
      dependencies: ["generate"]
      skill_id: "review-code"
      
    # Task 3: 复用 Claude Agent 修复代码
    - task_id: "fix"
      name: "修复代码"
      agent_runtime: "claude"
      reuse_agent: "code-generator"  # 复用 Task 1 的 Agent
      dependencies: ["review"]
      skill_id: "fix-code"
      keep_agent: false  # 完成后销毁
```

### 4.2 Rust API

```rust
/// 执行多 Agent DAG
async fn run_multi_agent_dag() -> Result<()> {
    // 1. 创建 Agent Pool
    let pool = AgentPool::new()
        .with_runtime(RuntimeType::Claude, ClaudeRuntime::new())
        .with_runtime(RuntimeType::OpenCode, OpenCodeRuntime::new());
    
    // 2. 创建执行器
    let executor = MultiAgentDagExecutor::new(
        DagScheduler::new(),
        pool,
    );
    
    // 3. 加载并执行
    let dag: DagDefinition = load_yaml("multi-agent-workflow.yaml").await?;
    let report = executor.execute(dag).await?;
    
    println!("完成: {} 任务成功", report.completed);
    Ok(())
}
```

### 4.3 CLI 使用

```bash
# 执行多 Agent DAG
cis dag run multi-agent-workflow.yaml

# 查看运行中的 Agent
cis agent list --dag-run <run-id>

# Attach 到特定 Agent（调试）
cis agent attach <agent-id>

# Detach（返回后台）
# Ctrl+B, D (tmux 风格)
```

---

## 5. 实施计划

### Phase 1: 基础抽象 (Week 1)
- [ ] 定义 `PersistentAgent` trait
- [ ] 实现 `AgentPool`
- [ ] 扩展 `AgentCommandConfig` 支持持久化模式

### Phase 2: OpenCode `-p` 模式 (Week 2)
- [ ] 实现 `OpenCodePersistentAgent`
- [ ] 调研 `-p` 模式的具体行为和 socket 协议
- [ ] 实现 attach/detach 功能

### Phase 3: Claude 持久化 (Week 3)
- [ ] 扩展 `AgentSession` 支持持久化模式
- [ ] 实现 `ClaudePersistentAgent`
- [ ] 复用现有 attach 功能

### Phase 4: DAG 集成 (Week 4)
- [ ] 扩展 `DagTask` 结构
- [ ] 实现 `MultiAgentDagExecutor`
- [ ] 支持 YAML 配置

### Phase 5: 跨节点 (Week 5-6)
- [ ] Agent 跨节点发现
- [ ] Matrix Federation 传输
- [ ] 端到端测试

---

## 6. 关键决策

| 决策 | 选择 | 原因 |
|------|------|------|
| OpenCode 模式 | `-p` + socket | 原生支持前后台切换 |
| Claude 模式 | PTY + EventBus | 复用现有架构 |
| Agent 通信 | Socket / PTY | 简单可靠 |
| 跨节点 | Matrix Federation | 复用 CIS 基础设施 |
| 生命周期 | DAG 控制 | 灵活可控 |

---

**设计完成 - 基于现有架构的修正版本**
