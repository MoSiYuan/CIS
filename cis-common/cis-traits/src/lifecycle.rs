use async_trait::async_trait;
use std::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

#[async_trait]
pub trait Lifecycle: Send + Sync {
    async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn stop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn shutdown(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn is_running(&self) -> bool;
    async fn health_check(&self) -> HealthStatus;
}

pub trait Named {
    fn name(&self) -> &str;
}
