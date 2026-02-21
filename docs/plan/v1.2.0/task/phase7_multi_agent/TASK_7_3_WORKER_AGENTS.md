# TASK 7.3: Worker Agents 实现

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 497a3d4
> **负责人**: TBD
> **周期**: Week 14-15

---

## 任务概述

实现具体的 Worker Agents：Coder、Doc、Debugger。

## 工作内容

### 1. Coder Agent

```rust
// crates/cis-core/src/agent/workers/coder.rs
#[cfg(feature = "worker-agents")]
pub struct CoderAgent {
    name: String,
    llm: Box<dyn LLM>,
    memory: Arc<dyn Memory>,
    tools: Vec<Box<dyn Tool>>,
    context: CoderContext,
}

#[cfg(feature = "worker-agents")]
impl Agent for CoderAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Coder
    }
    
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Remote  // 使用 Claude API
    }
    
    fn can_handle(&self, task_type: &TaskType) -> bool {
        matches!(task_type, 
            TaskType::CodeGeneration |
            TaskType::CodeReview |
            TaskType::Refactoring |
            TaskType::Debugging
        )
    }
    
    async fn turn(&mut self, user_message: &str) -> anyhow::Result<String> {
        // 1. 检索相关记忆（过往代码、项目结构）
        let memories = self.memory
            .recall(user_message, 5)
            .await?;
        
        // 2. 构建上下文
        let context = self.build_context(&memories).await?;
        
        // 3. 构建提示词
        let prompt = format!(
            "You are a coding assistant. Context: {}\n\nUser: {}",
            context, user_message
        );
        
        // 4. 调用 LLM
        let response = self.llm.complete(&prompt).await?;
        
        // 5. 存储交互
        self.store_interaction(user_message, &response).await?;
        
        Ok(response)
    }
}
```

### 2. Doc Agent

```rust
// crates/cis-core/src/agent/workers/doc.rs
#[cfg(feature = "worker-agents")]
pub struct DocAgent {
    name: String,
    llm: Box<dyn LLM>,
    memory: Arc<dyn Memory>,
}

#[cfg(feature = "worker-agents")]
impl Agent for DocAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Doc
    }
    
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Hybrid
    }
    
    fn can_handle(&self, task_type: &TaskType) -> bool {
        matches!(task_type,
            TaskType::Documentation |
            TaskType::CodeExplanation |
            TaskType::Tutorial
        )
    }
    
    async fn turn(&mut self, user_message: &str) -> anyhow::Result<String> {
        // 使用 OpenCode 模型生成文档
        // ...
    }
}
```

### 3. Debugger Agent

```rust
// crates/cis-core/src/agent/workers/debugger.rs
#[cfg(feature = "worker-agents")]
pub struct DebuggerAgent {
    name: String,
    llm: Box<dyn LLM>,
    memory: Arc<dyn Memory>,
    tool_registry: ToolRegistry,
}

#[cfg(feature = "worker-agents")]
impl Agent for DebuggerAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Debugger
    }
    
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Remote  // 使用 Kimi API
    }
    
    fn can_handle(&self, task_type: &TaskType) -> bool {
        matches!(task_type,
            TaskType::Debugging |
            TaskType::ErrorAnalysis |
            TaskType::LogAnalysis
        )
    }
    
    async fn turn(&mut self, user_message: &str) -> anyhow::Result<String> {
        // 使用 ReAct 模式进行调试
        let mut iterations = 0;
        let max_iterations = 10;
        
        loop {
            // 1. Thought
            let thought = self.llm.think(user_message).await?;
            
            // 2. Action
            if let Some(action) = self.parse_action(&thought) {
                let observation = self.tool_registry.execute(action).await?;
                
                // 3. 基于观察继续
                if self.is_resolved(&observation) {
                    return self.generate_solution(&observation).await;
                }
            } else {
                // 直接回答
                return Ok(thought);
            }
            
            iterations += 1;
            if iterations >= max_iterations {
                return Err(anyhow!("Max iterations reached"));
            }
        }
    }
}
```

### 4. Agent Factory

```rust
// crates/cis-core/src/agent/factory.rs
#[cfg(feature = "worker-agents")]
pub struct AgentFactory {
    creator: Box<dyn Fn() -> Box<dyn Agent> + Send + Sync>,
}

#[cfg(feature = "worker-agents")]
impl AgentFactory {
    pub fn coder(llm_config: LLMConfig, memory: Arc<dyn Memory>) -> Self {
        Self {
            creator: Box::new(move || {
                Box::new(CoderAgent::new(
                    llm_config.clone(),
                    memory.clone(),
                ))
            }),
        }
    }
    
    pub fn doc(llm_config: LLMConfig, memory: Arc<dyn Memory>) -> Self {
        // ...
    }
    
    pub fn debugger(llm_config: LLMConfig, memory: Arc<dyn Memory>) -> Self {
        // ...
    }
    
    pub async fn create(&self) -> Result<Box<dyn Agent>, FactoryError> {
        Ok((self.creator)())
    }
}
```

## 验收标准

- [ ] Coder Agent 可生成/审查代码
- [ ] Doc Agent 可生成文档
- [ ] Debugger Agent 可使用工具链调试
- [ ] 各 Agent 使用不同 LLM backend
- [ ] AgentPool 管理正常

## 依赖

- Task 7.1 (Agent trait)

## 阻塞

- Task 7.4 (DAG 编排)
- Task 7.5 (P2P 跨设备)

---
