//! # Receptionist Agent
//!
//! The Receptionist Agent is the entry point for all user requests.
//! It analyzes the request and routes it to the appropriate Worker Agent.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use cis_traits::agent::{Agent, AgentConfig, AgentStatus, AgentType, RuntimeType, AgentMetrics};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceptionistConfig {
    pub max_workers: usize,
    pub timeout_secs: u64,
}

impl Default for ReceptionistConfig {
    fn default() -> Self {
        Self {
            max_workers: 10,
            timeout_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingDecision {
    Code(String),
    Documentation(String),
    Debug(String),
    Analysis(String),
    Unknown,
}

pub struct ReceptionistAgent {
    config: ReceptionistConfig,
    status: AgentStatus,
    metrics: AgentMetrics,
    worker_pool: Arc<RwLock<Vec<Box<dyn Agent>>>>,
}

impl ReceptionistAgent {
    pub fn new(config: ReceptionistConfig) -> Self {
        Self {
            config,
            status: AgentStatus::Created,
            metrics: AgentMetrics::default(),
            worker_pool: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn route(&self, request: &str) -> RoutingDecision {
        let request_lower = request.to_lowercase();
        
        if request_lower.contains("代码") || request_lower.contains("code") || request_lower.contains("写") || request_lower.contains("implement") {
            RoutingDecision::Code(request.to_string())
        } else if request_lower.contains("文档") || request_lower.contains("doc") || request_lower.contains("write") {
            RoutingDecision::Documentation(request.to_string())
        } else if request_lower.contains("调试") || request_lower.contains("debug") || request_lower.contains("error") || request_lower.contains("bug") {
            RoutingDecision::Debug(request.to_string())
        } else if request_lower.contains("分析") || request_lower.contains("analyze") || request_lower.contains("review") {
            RoutingDecision::Analysis(request.to_string())
        } else {
            RoutingDecision::Unknown
        }
    }
}

#[async_trait]
impl Agent for ReceptionistAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Receptionist
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Custom("receptionist".to_string())
    }

    fn status(&self) -> AgentStatus {
        self.status.clone()
    }

    async fn turn(&mut self, user_message: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.status = AgentStatus::Busy;
        
        let decision = self.route(user_message).await;
        let response = format!("Routed to: {:?}", decision);
        
        self.metrics.total_turns += 1;
        self.metrics.successful_turns += 1;
        self.status = AgentStatus::Idle;
        
        Ok(response)
    }

    async fn can_handle(&self, task_type: &str) -> bool {
        matches!(task_type, "routing" | "classification" | "reception")
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
