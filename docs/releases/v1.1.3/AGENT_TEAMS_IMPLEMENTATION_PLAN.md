# Agent Teams 实施计划

## 需要修改/新增的文件

### 1. 核心抽象层

```
cis-core/src/agent/teams/
├── mod.rs              # 导出 teams 模块
├── runtime.rs          # AgentRuntime trait + UnifiedMailbox
├── agent.rs            # Agent trait
├── message.rs          # AgentMessage, MessageType
├── claude/
│   ├── mod.rs          # ClaudeAgentRuntime
│   └── agent.rs        # ClaudeAgent
└── opencode/
    ├── mod.rs          # OpenCodeAgentRuntime
    └── agent.rs        # OpenCodeAgent
```

### 2. DAG 扩展

```
cis-core/src/scheduler/
├── mod.rs              # 扩展 DagNode 结构
└── multi_agent_executor.rs  # MultiAgentDagExecutor

cis-core/src/types.rs   # 扩展 Task 结构
```

### 3. 集成点

```
cis-core/src/agent/mod.rs          # 集成 runtime 注册
cis-core/src/skill/mod.rs          # Skill 支持 agent_runtime 配置
cis-node/src/commands/dag.rs       # CLI 支持 --agent-runtime 参数
```

## 关键改动点

### 1. DagNode 扩展

```rust
// cis-core/src/scheduler/mod.rs
pub struct DagNode {
    pub task_id: String,
    pub dependencies: Vec<String>,
    pub skill_id: Option<String>,
    
    // 新增
    pub agent_runtime: Option<RuntimeType>,
    pub agent_id: Option<String>,
    pub create_new_agent: bool,
    pub agent_config: Option<AgentConfig>,
    pub keep_agent: bool,
}
```

### 2. 创建 runtime trait

```rust
// cis-core/src/agent/teams/runtime.rs
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn runtime_type(&self) -> RuntimeType;
    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn Agent>>;
    async fn list_agents(&self) -> Vec<AgentInfo>;
}
```

### 3. 修改 DAG 执行器

```rust
// cis-core/src/scheduler/multi_agent_executor.rs
pub struct MultiAgentDagExecutor {
    scheduler: DagScheduler,
    runtimes: HashMap<RuntimeType, Box<dyn AgentRuntime>>,
    agent_pool: Arc<RwLock<HashMap<String, Box<dyn Agent>>>>,
}
```

## 快速验证步骤

1. **Week 1**: 定义 trait，实现内存 Mailbox
2. **Week 2**: 实现 ClaudeAgentRuntime（stub）
3. **Week 3**: 实现 OpenCodeAgentRuntime（stub）
4. **Week 4**: 集成 DAG，支持 YAML 配置

## 预期 API

```bash
# 执行 DAG，指定 Runtime
cis dag run workflow.yaml --agent-runtime claude
cis dag run workflow.yaml --agent-runtime opencode

# 混合 Runtime（YAML 中指定）
cis dag run multi-agent.yaml  # 自动根据 task 配置选择
```

## 依赖检查

- [ ] Claude Code CLI 支持 headless/agent 模式？
- [ ] OpenCode 支持 socket/agent 模式？
- [ ] 需要添加哪些 crate？
  - `async-trait` (可能已有)
  - `uuid` (可能已有)
