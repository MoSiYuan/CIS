use async_trait::async_trait;
use std::error::Error;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    Receptionist,
    Coder,
    Doc,
    Debugger,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeType {
    Claude,
    OpenCode,
    Kimi,
    Ollama,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentStatus {
    Created,
    Running,
    Busy,
    Idle,
    Shutdown,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPoolStats {
    pub total_agents: usize,
    pub available: usize,
    pub busy: usize,
    pub by_type: HashMap<AgentType, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<usize>,
    pub system_prompt: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "claude-3-sonnet".to_string(),
            temperature: 0.7,
            max_tokens: Some(4096),
            system_prompt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub total_turns: u64,
    pub successful_turns: u64,
    pub failed_turns: u64,
    pub average_response_time_ms: u64,
    pub memory_usage_bytes: u64,
}

impl Default for AgentMetrics {
    fn default() -> Self {
        Self {
            total_turns: 0,
            successful_turns: 0,
            failed_turns: 0,
            average_response_time_ms: 0,
            memory_usage_bytes: 0,
        }
    }
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    fn runtime_type(&self) -> RuntimeType;
    fn status(&self) -> AgentStatus;

    async fn turn(&mut self, user_message: &str) -> Result<String, Box<dyn Error + Send + Sync>>;
    async fn can_handle(&self, task_type: &str) -> bool;
    async fn configure(&mut self, config: AgentConfig) -> Result<(), Box<dyn Error + Send + Sync>>;
    
    async fn metrics(&self) -> AgentMetrics;
    async fn reset(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn shutdown(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[async_trait]
pub trait AgentPool: Send + Sync {
    async fn acquire(&self, agent_type: AgentType) -> Result<Box<dyn Agent>, Box<dyn Error + Send + Sync>>;
    async fn release(&self, agent: Box<dyn Agent>) -> Result<(), Box<dyn Error + Send + Sync>>;

    fn available_types(&self) -> Vec<AgentType>;
    async fn stats(&self) -> Result<AgentPoolStats, Box<dyn Error + Send + Sync>>;
    
    async fn register_agent(&self, agent: Box<dyn Agent>) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn unregister_agent(&self, agent_type: AgentType) -> Result<(), Box<dyn Error + Send + Sync>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub task_id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub priority: TaskPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}
