//! # Worker Agent
//!
//! Worker Agents handle specific types of tasks assigned by the Receptionist.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use cis_traits::agent::{Agent, AgentConfig, AgentStatus, AgentType, RuntimeType, AgentMetrics};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerType {
    Coder,
    Documenter,
    Debugger,
    Analyzer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAgentConfig {
    pub worker_type: WorkerType,
    pub model: String,
    pub temperature: f32,
}

impl Default for WorkerAgentConfig {
    fn default() -> Self {
        Self {
            worker_type: WorkerType::Coder,
            model: "claude-3-sonnet".to_string(),
            temperature: 0.7,
        }
    }
}

pub struct WorkerAgent {
    config: WorkerAgentConfig,
    status: AgentStatus,
    metrics: AgentMetrics,
}

impl WorkerAgent {
    pub fn new(config: WorkerAgentConfig) -> Self {
        Self {
            config,
            status: AgentStatus::Created,
            metrics: AgentMetrics::default(),
        }
    }
}

#[async_trait]
impl Agent for WorkerAgent {
    fn agent_type(&self) -> AgentType {
        match self.config.worker_type {
            WorkerType::Coder => AgentType::Coder,
            WorkerType::Documenter => AgentType::Doc,
            WorkerType::Debugger => AgentType::Debugger,
            WorkerType::Analyzer => AgentType::Custom("analyzer".to_string()),
        }
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Claude
    }

    fn status(&self) -> AgentStatus {
        self.status.clone()
    }

    async fn turn(&mut self, user_message: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.status = AgentStatus::Busy;
        
        let response = format!("[{:?}] Processed: {}", self.config.worker_type, user_message);
        
        self.metrics.total_turns += 1;
        self.metrics.successful_turns += 1;
        self.status = AgentStatus::Idle;
        
        Ok(response)
    }

    async fn can_handle(&self, task_type: &str) -> bool {
        let worker_type_str = format!("{:?}", self.config.worker_type).to_lowercase();
        task_type.to_lowercase().contains(&worker_type_str)
    }

    async fn configure(&mut self, config: AgentConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    
    async fn metrics(&self) -> AgentMetrics {
        self.metrics.clone()
    }
    
    async fn reset(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.metrics = AgentMetrics::default();
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.status = AgentStatus::Shutdown;
        Ok(())
    }
}
