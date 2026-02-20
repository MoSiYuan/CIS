use async_trait::async_trait;
use cis_traits::{Lifecycle, HealthStatus, Named};
use std::error::Error;
use std::sync::{Arc, RwLock};
use super::protocol::Message;

pub struct P2pNetwork {
    name: String,
    running: RwLock<bool>,
    peers: RwLock<Vec<String>>,
}

impl P2pNetwork {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            running: RwLock::new(false),
            peers: RwLock::new(Vec::new()),
        }
    }

    pub async fn dial(&self, peer_id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut peers = self.peers.write().unwrap();
        if !peers.contains(&peer_id.to_string()) {
            peers.push(peer_id.to_string());
        }
        Ok(())
    }

    pub async fn broadcast(&self, _message: Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    pub fn connected_peers(&self) -> Vec<String> {
        self.peers.read().unwrap().clone()
    }
}

#[async_trait]
impl Lifecycle for P2pNetwork {
    async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut running = self.running.write().unwrap();
        *running = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut running = self.running.write().unwrap();
        *running = false;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.stop().await
    }

    fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }

    async fn health_check(&self) -> HealthStatus {
        if self.is_running() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy { reason: "Not running".to_string() }
        }
    }
}

impl Named for P2pNetwork {
    fn name(&self) -> &str {
        &self.name
    }
}
