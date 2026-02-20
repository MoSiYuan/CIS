use async_trait::async_trait;
use std::error::Error;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    Receptionist,
    Coder,
    Doc,
    Debugger,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeType {
    Claude,
    OpenCode,
    Kimi,
    Ollama,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Created,
    Running,
    Busy,
    Idle,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct AgentPoolStats {
    pub total_agents: usize,
    pub available: usize,
    pub busy: usize,
    pub by_type: HashMap<AgentType, usize>,
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<usize>,
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn runtime_type(&self) -> RuntimeType;
    fn status(&self) -> AgentStatus;

    async fn turn(&mut self, user_message: &str) -> Result<String, Box<dyn Error + Send + Sync>>;
    async fn can_handle(&self, task_type: &str) -> bool;
    async fn configure(&mut self, config: AgentConfig) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
pub trait AgentPool: Send + Sync {
    async fn acquire(&self, agent_type: AgentType) -> Result<Box<dyn Agent>, Box<dyn Error + Send + Sync>>;
    async fn release(&self, agent: Box<dyn Agent>) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn available_types(&self) -> Vec<AgentType>;
    async fn stats(&self) -> Result<AgentPoolStats, Box<dyn Error + Send + Sync>>;
}
