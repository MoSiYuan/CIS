# TASK 7.1: Agent Trait 实现

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 13+

---

## 任务概述

实现 Agent trait 和相关基础设施，支持多种 Agent 类型。

## 工作内容

### 1. 定义 Agent Trait

```rust
// crates/cis-traits/src/agent.rs
#[cfg(feature = "agent")]
pub mod agent {
    use async_trait::async_trait;
    
    /// Agent 类型枚举
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum AgentType {
        Receptionist,
        Coder,
        Doc,
        Debugger,
        Custom(&'static str),
    }
    
    /// Agent 运行时类型
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuntimeType {
        Local,      // 本地 LLM
        Remote,     // 远程 API
        Hybrid,     // 混合
    }
    
    /// 模型偏好
    #[derive(Debug, Clone)]
    pub struct ModelPreference {
        pub primary: String,
        pub fallbacks: Vec<String>,
        pub parameters: HashMap<String, Value>,
    }
    
    /// Agent 核心 trait
    #[async_trait]
    pub trait Agent: Send + Sync + Lifecycle + Named {
        /// 获取 Agent 类型
        fn agent_type(&self) -> AgentType;
        
        /// 获取运行时类型
        fn runtime_type(&self) -> RuntimeType;
        
        /// 执行一轮对话
        async fn turn(&mut self, user_message: &str) -> anyhow::Result<String>;
        
        /// 检查是否能处理某类任务
        fn can_handle(&self, task_type: &TaskType) -> bool;
        
        /// 获取记忆命名空间
        fn memory_namespace(&self) -> String;
        
        /// 获取模型偏好
        fn model_preference(&self) -> ModelPreference;
    }
    
    /// 命名 trait
    pub trait Named {
        fn name(&self) -> &str;
        fn set_name(&mut self, name: impl Into<String>);
    }
}
```

### 2. 实现 AgentPool

```rust
// crates/cis-core/src/agent/pool.rs
#[cfg(feature = "multi-agent")]
pub struct AgentPool {
    agents: RwLock<HashMap<AgentType, Vec<Box<dyn Agent>>>>,
    factories: RwLock<HashMap<AgentType, AgentFactory>>,
    max_per_type: usize,
}

#[cfg(feature = "multi-agent")]
impl AgentPool {
    pub fn new(max_per_type: usize) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            factories: RwLock::new(HashMap::new()),
            max_per_type,
        }
    }
    
    pub fn register(&self, agent_type: AgentType, factory: AgentFactory) {
        self.factories.write().insert(agent_type, factory);
    }
    
    pub async fn acquire(&self, agent_type: AgentType) -> Result<Box<dyn Agent>, PoolError> {
        // 1. 尝试从池中获取空闲 Agent
        if let Some(agent) = self.try_get_idle(&agent_type).await {
            return Ok(agent);
        }
        
        // 2. 检查是否达到上限
        let count = self.count(&agent_type).await;
        if count >= self.max_per_type {
            return Err(PoolError::PoolExhausted(agent_type));
        }
        
        // 3. 创建新 Agent
        let factory = self.factories.read()
            .get(&agent_type)
            .cloned()
            .ok_or(PoolError::FactoryNotFound(agent_type))?;
        
        let agent = factory.create().await?;
        Ok(agent)
    }
    
    pub async fn release(&self, agent: Box<dyn Agent>) {
        // 归还 Agent 到池中
        let agent_type = agent.agent_type();
        self.agents.write()
            .entry(agent_type)
            .or_default()
            .push(agent);
    }
}
```

### 3. 配置 Feature Flags

```toml
# crates/cis-traits/Cargo.toml
[features]
default = []
agent = ["dep:async-trait"]
multi-agent = ["agent"]
```

```toml
# crates/cis-core/Cargo.toml
[features]
default = []
multi-agent = ["cis-traits/multi-agent", "agent-pool", "receptionist", "worker-agents"]
agent-pool = []
receptionist = ["agent-pool"]
worker-agents = ["agent-pool"]
```

## 验收标准

- [ ] Agent trait 定义完整
- [ ] AgentPool 实现可用
- [ ] 支持多种 Agent 类型
- [ ] Feature flag 控制编译
- [ ] 单元测试覆盖

## 依赖

- Task 6.2 (v1.2.0 发布)
- Task 1.3 (cis-traits)

## 阻塞

- Task 7.2 (Receptionist Agent)
- Task 7.3 (Worker Agents)

---
