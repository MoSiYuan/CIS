//! Node Controller
//!
//! Handles all node-related operations and business logic

use std::collections::HashMap;

use cis_core::service::{
    NodeService,
    ListOptions,
    node_service::{
        NodeInfo, NodeStats, BindOptions, TrustLevel,
        NodeStatus as ServiceNodeStatus,
    }
};
use tracing::{info, warn, error};

/// Node Controller
///
/// Responsibilities:
/// - Encapsulate all node CRUD operations
/// - Handle service calls and error handling
/// - Provide a clean API for ViewModels
pub struct NodeController {
    node_service: NodeService,
}

impl NodeController {
    /// Create a new NodeController
    pub fn new(node_service: NodeService) -> Self {
        info!("Initializing NodeController");
        Self { node_service }
    }

    /// List all nodes with optional filters
    pub async fn list_nodes(&self, options: ListOptions) -> Result<Vec<NodeInfo>, String> {
        info!("NodeController: list_nodes");

        self.node_service
            .list(options)
            .await
            .map(|result| result.items)
            .map_err(|e| {
                error!("Failed to list nodes: {}", e);
                format!("Failed to list nodes: {}", e)
            })
    }

    /// Inspect a specific node
    pub async fn inspect_node(&self, node_id: &str) -> Result<NodeInfo, String> {
        info!("NodeController: inspect_node({})", node_id);

        self.node_service
            .inspect(node_id)
            .await
            .map_err(|e| {
                error!("Failed to inspect node '{}': {}", node_id, e);
                format!("Failed to inspect node '{}': {}", node_id, e)
            })
    }

    /// Ping a node to check if it's online
    pub async fn ping_node(&self, node_id: &str) -> Result<bool, String> {
        info!("NodeController: ping_node({})", node_id);

        self.node_service
            .ping(node_id)
            .await
            .map_err(|e| {
                error!("Failed to ping node '{}': {}", node_id, e);
                format!("Failed to ping node '{}': {}", node_id, e)
            })
    }

    /// Bind to a new node
    pub async fn bind_node(
        &self,
        endpoint: String,
        did: Option<String>,
        trust_level: TrustLevel,
        auto_sync: bool,
    ) -> Result<NodeInfo, String> {
        info!("NodeController: bind_node({}, {:?})", endpoint, did);

        let options = BindOptions {
            endpoint,
            did,
            trust_level,
            auto_sync,
        };

        self.node_service
            .bind(options)
            .await
            .map_err(|e| {
                error!("Failed to bind node: {}", e);
                format!("Failed to bind node: {}", e)
            })
    }

    /// Block a node
    pub async fn block_node(&self, node_id: &str) -> Result<(), String> {
        info!("NodeController: block_node({})", node_id);

        self.node_service
            .block(node_id)
            .await
            .map_err(|e| {
                error!("Failed to block node '{}': {}", node_id, e);
                format!("Failed to block node '{}': {}", node_id, e)
            })
    }

    /// Unblock a node
    pub async fn unblock_node(&self, node_id: &str) -> Result<(), String> {
        info!("NodeController: unblock_node({})", node_id);

        self.node_service
            .unblock(node_id)
            .await
            .map_err(|e| {
                error!("Failed to unblock node '{}': {}", node_id, e);
                format!("Failed to unblock node '{}': {}", node_id, e)
            })
    }

    /// Verify a node with DID
    pub async fn verify_node(&self, node_id: &str, did: &str) -> Result<(), String> {
        info!("NodeController: verify_node({}, {})", node_id, did);

        self.node_service
            .verify(node_id, did)
            .await
            .map_err(|e| {
                error!("Failed to verify node '{}': {}", node_id, e);
                format!("Failed to verify node '{}': {}", node_id, e)
            })
    }

    /// Get statistics for a node
    pub async fn get_node_stats(&self, node_id: &str) -> Result<NodeStats, String> {
        info!("NodeController: get_node_stats({})", node_id);

        self.node_service
            .stats(node_id)
            .await
            .map_err(|e| {
                error!("Failed to get stats for node '{}': {}", node_id, e);
                format!("Failed to get stats for node '{}': {}", node_id, e)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running NodeService
    // In a real scenario, you'd use a mock service

    #[tokio::test]
    async fn test_controller_creation() {
        // This test would fail if NodeService::new() fails
        // In production, you'd mock the service
    }
}
