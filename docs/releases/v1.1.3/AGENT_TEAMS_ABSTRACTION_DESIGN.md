# CIS Agent Teams 抽象架构设计

**目标**: 设计统一抽象，让 Claude 和 OpenCode 都支持多 Agent 任务结构，DAG 可自由选择 Agent 工具

---

## 1. 当前架构分析

### 1.1 现有组件

```
┌─────────────────────────────────────────────────────────────────┐
│                      CIS 当前架构                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────┐    ┌──────────────────┐                  │
│  │   Agent Cluster  │    │   DAG Scheduler  │                  │
│  │   (节点内多Agent) │    │   (任务编排)      │                  │
│  │                  │    │                  │                  │
│  │  - OpenCode      │    │  - Task 定义      │                  │
│  │  - Claude        │    │  - 依赖管理       │                  │
│  │  - Kimi          │    │  - 状态机         │                  │
│  └────────┬─────────┘    └────────┬─────────┘                  │
│           │                       │                             │
│           └───────────┬───────────┘                             │
│                       ▼                                         │
│  ┌──────────────────────────────────────────┐                  │
│  │        Agent Provider (单一执行)          │                  │
│  │                                          │                  │
│  │   execute(prompt) -> Result              │                  │
│  │                                          │                  │
│  └──────────────────────────────────────────┘                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 当前问题

1. **Agent 是单执行模式**: 每次调用 `execute(prompt)`，没有持续状态
2. **没有多 Agent 协作**: 多个 Agent 之间无法通信协作
3. **DAG 无法指定 Agent**: Task 结构缺少 `agent_type` 字段
4. **Claude/OpenCode 能力不均等**: 只有 Claude 有原生 Agent Teams 支持

---

## 2. 目标架构设计

### 2.1 核心抽象

```rust
/// Agent Runtime 抽象 - 统一 Claude 和 OpenCode 的多 Agent 能力
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Runtime 类型
    fn runtime_type(&self) -> RuntimeType;
    
    /// 创建 Agent 实例
    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn Agent>>;
    
    /// 获取所有活跃 Agent
    async fn list_agents(&self) -> Vec<AgentInfo>;
    
    /// 广播消息给所有 Agent
    async fn broadcast(&self, message: AgentMessage) -> Result<()>;
}

/// Agent 实例抽象
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent ID
    fn agent_id(&self) -> &str;
    
    /// 发送消息给这个 Agent
    async fn send(&self, message: AgentMessage) -> Result<()>;
    
    /// 接收消息（从 Mailbox）
    async fn recv(&self) -> Result<AgentMessage>;
    
    /// 执行任务
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult>;
    
    /// 获取 Agent 状态
    async fn status(&self) -> AgentStatus;
}

/// 消息结构（Mailbox 语义）
pub struct AgentMessage {
    pub from: String,
    pub to: Option<String>,  // None = 广播
    pub message_type: MessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}
```

### 2.2 两种 Runtime 实现

```
┌─────────────────────────────────────────────────────────────────┐
│                    Agent Runtime 抽象层                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────┐    ┌─────────────────────────┐    │
│  │   ClaudeAgentRuntime    │    │  OpenCodeAgentRuntime   │    │
│  │                         │    │                         │    │
│  │  使用 Claude Code CLI   │    │  使用 OpenCode CLI      │    │
│  │  的 headless/agent 模式  │    │  的多进程模式            │    │
│  │                         │    │                         │    │
│  │  - claude agent create  │    │  - opencode run --agent │    │
│  │  - claude agent message │    │  - 自定义 Agent 协议      │    │
│  │  - claude agent list    │    │                         │    │
│  └──────────┬──────────────┘    └──────────┬──────────────┘    │
│             │                              │                    │
│             └──────────────┬───────────────┘                    │
│                            ▼                                    │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Mailbox / Message Router                     │  │
│  │                                                           │  │
│  │   节点内: tokio::broadcast (内存)                         │  │
│  │   跨节点: Matrix Federation (7676)                        │  │
│  │                                                           │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. 详细设计

### 3.1 ClaudeAgentRuntime 实现

```rust
/// Claude Code CLI 的 Agent 模式实现
pub struct ClaudeAgentRuntime {
    /// 工作目录
    work_dir: PathBuf,
    /// 活跃 Agent 表
    agents: Arc<RwLock<HashMap<String, ClaudeAgentHandle>>>,
    /// 事件广播
    event_bus: EventBroadcaster,
}

impl AgentRuntime for ClaudeAgentRuntime {
    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn Agent>> {
        // 使用 claude agent create 命令
        let output = Command::new("claude")
            .args(&["agent", "create", 
                "--name", &config.name,
                "--system-prompt", &config.system_prompt,
                "--work-dir", &config.work_dir.to_string_lossy()
            ])
            .output()
            .await?;
            
        let agent_id = parse_agent_id(&output.stdout)?;
        
        // 启动 Agent 进程（无头模式）
        let process = Command::new("claude")
            .args(&["agent", "run", &agent_id, "--headless"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        
        let agent = ClaudeAgent::new(agent_id, process, self.event_bus.clone());
        self.agents.write().await.insert(agent_id.clone(), agent.clone());
        
        Ok(Box::new(agent))
    }
}

/// 单个 Claude Agent 实例
pub struct ClaudeAgent {
    agent_id: String,
    process: Child,
    inbox: mpsc::Receiver<AgentMessage>,
    event_bus: EventBroadcaster,
}

impl Agent for ClaudeAgent {
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        // 通过 stdin 发送任务
        let request = serde_json::to_string(&task)?;
        self.process.stdin.write_all(request.as_bytes()).await?;
        
        // 从 stdout 读取结果
        let mut stdout = self.process.stdout;
        let mut buffer = String::new();
        stdout.read_to_string(&mut buffer).await?;
        
        let result: TaskResult = serde_json::from_str(&buffer)?;
        Ok(result)
    }
    
    async fn send(&self, message: AgentMessage) -> Result<()> {
        // 发送到 Agent 的 stdin
        let msg = serde_json::to_string(&message)?;
        self.process.stdin.write_all(msg.as_bytes()).await?;
        Ok(())
    }
}
```

### 3.2 OpenCodeAgentRuntime 实现

```rust
/// OpenCode 的多进程 Agent 实现
pub struct OpenCodeAgentRuntime {
    work_dir: PathBuf,
    agents: Arc<RwLock<HashMap<String, OpenCodeAgentHandle>>>,
    event_bus: EventBroadcaster,
}

impl AgentRuntime for OpenCodeAgentRuntime {
    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn Agent>> {
        let agent_id = format!("opencode-agent-{}", uuid::Uuid::new_v4());
        
        // OpenCode 使用独立的进程 + Socket/Pipe 通信
        let socket_path = self.work_dir.join(format!("{}.sock", agent_id));
        
        // 启动 OpenCode Agent 进程
        let process = Command::new("opencode")
            .args(&[
                "run", 
                "--agent-mode",
                "--socket", &socket_path.to_string_lossy(),
                "--model", &config.model.unwrap_or_default(),
            ])
            .env("OPENCODE_AGENT_ID", &agent_id)
            .spawn()?;
        
        // 等待 Socket 就绪
        wait_for_socket(&socket_path).await?;
        
        let agent = OpenCodeAgent::new(agent_id, socket_path, process, self.event_bus.clone());
        self.agents.write().await.insert(agent.agent_id.clone(), agent.clone());
        
        Ok(Box::new(agent))
    }
}

/// OpenCode Agent 通过 Unix Socket 通信
pub struct OpenCodeAgent {
    agent_id: String,
    socket_path: PathBuf,
    process: Child,
    event_bus: EventBroadcaster,
}

impl Agent for OpenCodeAgent {
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        // 通过 Unix Socket 发送任务
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        
        let request = serde_json::to_vec(&task)?;
        stream.write_all(&request).await?;
        stream.shutdown().await?;
        
        // 读取响应
        let mut buffer = Vec::new();
        stream.read_to_end(&mut buffer).await?;
        
        let result: TaskResult = serde_json::from_slice(&buffer)?;
        Ok(result)
    }
    
    async fn send(&self, message: AgentMessage) -> Result<()> {
        // OpenCode 支持消息推送
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let msg = serde_json::to_vec(&message)?;
        stream.write_all(&msg).await?;
        Ok(())
    }
}
```

### 3.3 统一 Mailbox 实现

```rust
/// 统一的 Mailbox 实现
/// 
/// 节点内：使用 tokio::broadcast（内存）
/// 跨节点：使用 Matrix Federation（网络）
pub struct UnifiedMailbox {
    /// 本地内存广播（节点内 Agent）
    local_bus: EventBroadcaster,
    
    /// Matrix Room（跨节点）
    room_id: Option<String>,
    matrix_client: Option<MatrixClient>,
    
    /// Agent 地址表
    agent_routes: Arc<RwLock<HashMap<String, RouteInfo>>>,
}

impl UnifiedMailbox {
    /// 发送消息
    pub async fn send(&self, message: AgentMessage) -> Result<()> {
        match &message.to {
            None => {
                // 广播
                self.broadcast(message).await
            }
            Some(agent_id) => {
                // 点对点
                let route = self.agent_routes.read().await.get(agent_id).cloned();
                match route {
                    Some(RouteInfo::Local) => {
                        // 同一节点，直接内存广播
                        self.local_bus.broadcast(message).await
                    }
                    Some(RouteInfo::Remote { node_id, room_id }) => {
                        // 跨节点，通过 Matrix
                        self.send_matrix_event(&room_id, message).await
                    }
                    None => Err(Error::AgentNotFound(agent_id.clone())),
                }
            }
        }
    }
    
    /// 广播消息
    async fn broadcast(&self, message: AgentMessage) -> Result<()> {
        // 1. 本地广播
        self.local_bus.broadcast(message.clone()).await?;
        
        // 2. 跨节点广播（如果启用联邦）
        if let (Some(room_id), Some(client)) = (&self.room_id, &self.matrix_client) {
            let event = CisMatrixEvent::new(
                generate_event_id(),
                room_id,
                &message.from,
                "m.agent.message",
                serde_json::to_value(&message)?,
            );
            client.send_event(event).await?;
        }
        
        Ok(())
    }
}
```

---

## 4. DAG 集成

### 4.1 扩展 Task 结构

```rust
/// DAG 节点定义（扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNode {
    /// 任务 ID
    pub task_id: String,
    
    /// 依赖的任务 ID 列表
    pub dependencies: Vec<String>,
    
    /// 执行的 Skill
    pub skill_id: Option<String>,
    
    /// ===== 新增：Agent 配置 =====
    /// 指定使用的 Agent Runtime
    pub agent_runtime: Option<RuntimeType>,  // Claude / OpenCode
    
    /// 指定具体的 Agent ID（复用已有 Agent）
    pub agent_id: Option<String>,
    
    /// 是否创建新 Agent
    pub create_new_agent: bool,
    
    /// Agent 配置（创建新 Agent 时使用）
    pub agent_config: Option<AgentConfig>,
    
    /// 任务完成后是否保留 Agent
    pub keep_agent: bool,
    
    /// 四级决策级别
    pub level: TaskLevel,
    
    /// 其他字段...
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}
```

### 4.2 DAG 执行器集成

```rust
/// 支持多 Agent 的 DAG 执行器
pub struct MultiAgentDagExecutor {
    /// DAG 调度器
    scheduler: DagScheduler,
    
    /// Agent Runtime 注册表
    runtimes: HashMap<RuntimeType, Box<dyn AgentRuntime>>,
    
    /// Agent 实例池
    agent_pool: Arc<RwLock<HashMap<String, Box<dyn Agent>>>>,
    
    /// Mailbox
    mailbox: UnifiedMailbox,
}

impl MultiAgentDagExecutor {
    /// 执行单个任务
    async fn execute_task(&self, run_id: &str, node: &DagNode) -> Result<TaskResult> {
        // 1. 获取或创建 Agent
        let agent = self.get_or_create_agent(node).await?;
        
        // 2. 准备任务输入
        let task_request = TaskRequest {
            task_id: node.task_id.clone(),
            prompt: self.build_prompt(node).await?,
            context: self.gather_context(run_id, node).await?,
            tools: self.available_tools(node).await?,
        };
        
        // 3. 执行
        let result = agent.execute(task_request).await?;
        
        // 4. 清理（如果不保留）
        if !node.keep_agent {
            if let Some(agent_id) = &node.agent_id {
                self.agent_pool.write().await.remove(agent_id);
            }
        }
        
        Ok(result)
    }
    
    /// 获取或创建 Agent
    async fn get_or_create_agent(&self, node: &DagNode) -> Result<Box<dyn Agent>> {
        // 1. 如果指定了已有 Agent ID，直接获取
        if let Some(agent_id) = &node.agent_id {
            if let Some(agent) = self.agent_pool.read().await.get(agent_id) {
                return Ok(agent.clone());
            }
        }
        
        // 2. 获取 Runtime
        let runtime_type = node.agent_runtime.unwrap_or(RuntimeType::Claude);
        let runtime = self.runtimes.get(&runtime_type)
            .ok_or(Error::RuntimeNotFound(runtime_type))?;
        
        // 3. 创建新 Agent
        let config = node.agent_config.clone()
            .unwrap_or_else(|| AgentConfig::default_for(runtime_type));
        
        let agent = runtime.create_agent(config).await?;
        
        // 4. 加入池
        self.agent_pool.write().await.insert(agent.agent_id().to_string(), agent.clone());
        
        Ok(agent)
    }
}
```

---

## 5. 使用示例

### 5.1 DAG 定义（YAML）

```yaml
# multi-agent-workflow.yaml
dag:
  name: "多 Agent 代码审查"
  description: "Claude 写代码，OpenCode 审查"
  
  tasks:
    # Task 1: 使用 Claude 生成代码
    - task_id: "generate-code"
      name: "生成代码"
      agent_runtime: "claude"
      create_new_agent: true
      agent_config:
        name: "code-generator"
        system_prompt: "你是一个专业的 Rust 开发者"
      keep_agent: false
      skill_id: "generate-rust-code"
      inputs:
        - "{{ global.requirement }}"
      outputs:
        - "generated_code.rs"
      
    # Task 2: 使用 OpenCode 审查代码
    - task_id: "review-code"
      name: "代码审查"
      agent_runtime: "opencode"
      create_new_agent: true
      agent_config:
        model: "opencode/glm-4.7-free"
      dependencies:
        - "generate-code"
      skill_id: "review-rust-code"
      inputs:
        - "{{ tasks.generate-code.outputs[0] }}"
      outputs:
        - "review_report.md"
        
    # Task 3: 使用同一 Claude 实例优化代码
    - task_id: "refine-code"
      name: "优化代码"
      agent_runtime: "claude"
      # 复用第一个任务的 Agent
      agent_id: "code-generator"
      create_new_agent: false
      dependencies:
        - "review-code"
      skill_id: "refine-rust-code"
```

### 5.2 Rust 代码示例

```rust
/// 创建并执行多 Agent DAG
async fn execute_multi_agent_workflow() -> Result<()> {
    // 1. 创建 Runtime
    let claude_runtime = ClaudeAgentRuntime::new().await?;
    let opencode_runtime = OpenCodeAgentRuntime::new().await?;
    
    // 2. 创建执行器
    let mut executor = MultiAgentDagExecutor::new(DagScheduler::new())
        .with_runtime(RuntimeType::Claude, claude_runtime)
        .with_runtime(RuntimeType::OpenCode, opencode_runtime);
    
    // 3. 加载 DAG
    let dag_def: DagDefinition = load_dag("multi-agent-workflow.yaml").await?;
    let run_id = executor.create_run(dag_def).await?;
    
    // 4. 执行
    let report = executor.execute(&run_id).await?;
    
    println!("执行完成: {} tasks completed", report.completed);
    Ok(())
}
```

---

## 6. 跨节点 Agent Teams

### 6.1 场景

```
Node A (Claude)                    Node B (OpenCode)
┌─────────────────────┐           ┌─────────────────────┐
│  Agent: code-gen    │◄─────────►│  Agent: reviewer    │
│  Runtime: Claude    │  Matrix   │  Runtime: OpenCode  │
│                     │  Room     │                     │
│  !code-gen:cis.local│           │  !reviewer:cis.local│
└─────────────────────┘           └─────────────────────┘
         │                                  │
         └──────────┬───────────────────────┘
                    ▼
           ┌─────────────────┐
           │  Unified Mailbox │
           │  (跨节点消息路由)  │
           └─────────────────┘
```

### 6.2 配置

```rust
// 启用跨节点联邦
let mailbox = UnifiedMailbox::new()
    .with_federation(MatrixClient::new("!agent-team:cis.local"))
    .await?;

// Agent A 发送消息给 Agent B
let message = AgentMessage {
    from: "code-gen@node-a".to_string(),
    to: Some("reviewer@node-b".to_string()),
    message_type: MessageType::TaskRequest,
    payload: json!({
        "code": "...",
        "request": "请审查这段代码"
    }),
    timestamp: Utc::now(),
};

mailbox.send(message).await?;
```

---

## 7. 实施路线图

### Phase 1: 基础抽象（Week 1）
- [ ] 定义 `AgentRuntime` 和 `Agent` trait
- [ ] 实现 `UnifiedMailbox`
- [ ] 扩展 `Task` 结构支持 Agent 配置

### Phase 2: Claude Runtime（Week 2）
- [ ] 实现 `ClaudeAgentRuntime`
- [ ] 调研 Claude Code headless/agent 模式
- [ ] 实现 stdin/stdout 通信协议

### Phase 3: OpenCode Runtime（Week 3）
- [ ] 实现 `OpenCodeAgentRuntime`
- [ ] 实现 Unix Socket 通信
- [ ] 统一两种 Runtime 的行为

### Phase 4: DAG 集成（Week 4）
- [ ] 扩展 `DagNode` 结构
- [ ] 实现 `MultiAgentDagExecutor`
- [ ] 支持 YAML 配置

### Phase 5: 跨节点（Week 5-6）
- [ ] 集成 Matrix Federation
- [ ] 实现跨节点 Agent 发现
- [ ] 端到端测试

---

## 8. 关键决策

| 决策 | 选择 | 原因 |
|------|------|------|
| 通信协议 | JSON-RPC over stdin/stdout (Claude) / Unix Socket (OpenCode) | 简单、可调试 |
| Agent 生命周期 | DAG 控制（create/destroy） | 资源管理清晰 |
| 跨节点传输 | Matrix Federation | 复用 CIS 基础设施 |
| 消息格式 | JSON | 通用、易扩展 |

---

## 9. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| Claude headless 模式不稳定 | 同时支持 interactive 模式 fallback |
| OpenCode 不支持多 Agent | 使用多进程模拟 |
| 跨节点延迟高 | 本地缓存 + 异步消息 |
| Agent 状态不一致 | 定期心跳检查 |

---

**设计完成**
