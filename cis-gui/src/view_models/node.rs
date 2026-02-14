//! Node ViewModel
//!
//! Manages node list, status, and operations

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{info, warn};

use cis_core::service::{NodeService, ListOptions};
use cis_core::service::node_service::{NodeInfo, BindOptions, TrustLevel};

use crate::node_manager::{ManagedNode, NodeStatus, TrustState};
use super::{ViewModel, ViewModelState};

/// Result type for node refresh operations
#[derive(Debug, Clone)]
pub enum NodeRefreshResult {
    Success(Vec<ManagedNode>),
    Error(String),
}

/// Node ViewModel
///
/// Responsibilities:
/// - Manage node list and status
/// - Handle node operations (ping, block, verify, etc.)
/// - Auto-refresh node list periodically
/// - Provide node data to UI
pub struct NodeViewModel {
    /// Node data (thread-safe)
    nodes: Arc<RwLock<Vec<ManagedNode>>>,

    /// Demo/fallback nodes
    demo_nodes: Vec<ManagedNode>,

    /// Core service
    node_service: Option<NodeService>,

    /// Async runtime handle
    runtime_handle: tokio::runtime::Handle,

    /// Refresh state
    last_refresh: Arc<RwLock<Instant>>,
    refresh_interval: Duration,
    is_refreshing: Arc<AtomicBool>,

    /// Refresh result channel
    refresh_tx: tokio::sync::mpsc::Sender<NodeRefreshResult>,
    refresh_rx: Arc<RwLock<tokio::sync::mpsc::Receiver<NodeRefreshResult>>>,

    /// View model state
    state: ViewModelState,

    /// Whether to use real nodes (vs demo nodes)
    use_real_nodes: Arc<AtomicBool>,
}

impl NodeViewModel {
    /// Create a new NodeViewModel
    pub fn new(
        node_service: Option<NodeService>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Self {
        info!("Initializing NodeViewModel");

        // Create demo/fallback nodes
        let demo_nodes = vec![
            ManagedNode {
                id: "munin".to_string(),
                name: "Munin-macmini".to_string(),
                did: Some("did:cis:munin:abc123".to_string()),
                address: "192.168.1.100:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(12),
            },
            ManagedNode {
                id: "hugin".to_string(),
                name: "Hugin-pc".to_string(),
                did: Some("did:cis:hugin:def456".to_string()),
                address: "192.168.1.105:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(8),
            },
            ManagedNode {
                id: "seed".to_string(),
                name: "seed.cis.dev".to_string(),
                did: Some("did:cis:seed:ghi789".to_string()),
                address: "seed.cis.dev:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(45),
            },
            ManagedNode {
                id: "unknown".to_string(),
                name: "unknown-device".to_string(),
                did: None,
                address: "192.168.1.200:7676".to_string(),
                status: NodeStatus::Offline,
                trust_state: TrustState::Pending,
                last_seen: Some("5m ago".to_string()),
                latency_ms: None,
            },
        ];

        let (refresh_tx, refresh_rx) = tokio::sync::mpsc::channel(10);

        Self {
            nodes: Arc::new(RwLock::new(Vec::new())),
            demo_nodes,
            node_service,
            runtime_handle,
            last_refresh: Arc::new(RwLock::new(Instant::now())),
            refresh_interval: Duration::from_secs(5),
            is_refreshing: Arc::new(AtomicBool::new(false)),
            refresh_tx,
            refresh_rx: Arc::new(RwLock::new(refresh_rx)),
            state: ViewModelState::new(),
            use_real_nodes: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get current node list
    pub async fn get_nodes(&self) -> Vec<ManagedNode> {
        if self.use_real_nodes.load(Ordering::SeqCst) {
            let nodes = self.nodes.read().await;
            nodes.clone()
        } else {
            self.demo_nodes.clone()
        }
    }

    /// Check for refresh results from background task
    pub async fn check_refresh_results(&self) {
        let mut rx = self.refresh_rx.write().await;
        while let Ok(result) = rx.try_recv() {
            match result {
                NodeRefreshResult::Success(nodes) => {
                    *self.nodes.write().await = nodes;
                    self.use_real_nodes.store(true, Ordering::SeqCst);
                    info!("Node refresh successful");
                    self.state.mark_dirty();
                }
                NodeRefreshResult::Error(err) => {
                    warn!("Node refresh failed: {}", err);
                }
            }
        }
    }

    /// Trigger async node refresh
    pub fn refresh_nodes(&self) {
        if self.node_service.is_none() {
            info!("NodeService not available, using demo data");
            return;
        }

        if self.is_refreshing.load(Ordering::SeqCst) {
            return; // Already refreshing
        }

        self.is_refreshing.store(true, Ordering::SeqCst);

        let tx = self.refresh_tx.clone();
        let handle = self.runtime_handle.clone();
        let is_refreshing = Arc::clone(&self.is_refreshing);

        handle.spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new();
            if rt.is_err() {
                let _ = tx.try_send(NodeRefreshResult::Error(
                    "Failed to create runtime".to_string()
                ));
                is_refreshing.store(false, Ordering::SeqCst);
                return;
            }
            let rt = rt.unwrap();

            rt.block_on(async {
                let node_service = NodeService::new();
                let options = ListOptions::default();

                match node_service {
                    Ok(service) => {
                        match service.list(options).await {
                            Ok(result) => {
                                let nodes: Vec<ManagedNode> = result
                                    .items
                                    .iter()
                                    .map(|summary| {
                                        use cis_core::service::node_service::NodeStatus as ServiceNodeStatus;

                                        let status = match summary.status {
                                            ServiceNodeStatus::Online => NodeStatus::Online,
                                            ServiceNodeStatus::Offline => NodeStatus::Offline,
                                            ServiceNodeStatus::Blacklisted => NodeStatus::Offline,
                                            _ => NodeStatus::Offline,
                                        };

                                        let trust_state = match summary.status {
                                            ServiceNodeStatus::Blacklisted => TrustState::Blocked,
                                            ServiceNodeStatus::Online => TrustState::Verified,
                                            ServiceNodeStatus::Offline => TrustState::Verified,
                                            _ => TrustState::Unknown,
                                        };

                                        let last_seen = Some(
                                            chrono::Local::now()
                                                .format("%Y-%m-%d %H:%M")
                                                .to_string()
                                        );

                                        ManagedNode {
                                            id: summary.id.clone(),
                                            name: summary.name.clone(),
                                            did: if summary.did.is_empty() {
                                                None
                                            } else {
                                                Some(summary.did.clone())
                                            },
                                            address: summary.endpoint.clone(),
                                            status,
                                            trust_state,
                                            last_seen,
                                            latency_ms: None,
                                        }
                                    })
                                    .collect();

                                let _ = tx.try_send(NodeRefreshResult::Success(nodes));
                            }
                            Err(e) => {
                                let _ = tx.try_send(NodeRefreshResult::Error(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(NodeRefreshResult::Error(e.to_string()));
                    }
                }
            });

            is_refreshing.store(false, Ordering::SeqCst);
        });
    }

    /// Check if it's time to refresh
    pub async fn should_refresh(&self) -> bool {
        let last = *self.last_refresh.read().await;
        last.elapsed() > self.refresh_interval
    }

    /// Ping a node
    pub async fn ping_node(&self, node_id: &str) -> Result<bool, String> {
        if let Some(ref service) = self.node_service {
            service
                .ping(node_id)
                .await
                .map_err(|e| format!("Failed to ping node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Block a node
    pub async fn block_node(&self, node_id: &str) -> Result<(), String> {
        info!("Blocking node: {}", node_id);

        if let Some(ref service) = self.node_service {
            service
                .block(node_id)
                .await
                .map_err(|e| format!("Failed to block node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Unblock a node
    pub async fn unblock_node(&self, node_id: &str) -> Result<(), String> {
        info!("Unblocking node: {}", node_id);

        if let Some(ref service) = self.node_service {
            service
                .unblock(node_id)
                .await
                .map_err(|e| format!("Failed to unblock node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Verify a node with DID
    pub async fn verify_node(&self, node_id: &str, did: &str) -> Result<(), String> {
        info!("Verifying node {} with DID: {}", node_id, did);

        if let Some(ref service) = self.node_service {
            service
                .verify(node_id, did)
                .await
                .map_err(|e| format!("Failed to verify node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Inspect a node
    pub async fn inspect_node(&self, node_id: &str) -> Result<NodeInfo, String> {
        if let Some(ref service) = self.node_service {
            service
                .inspect(node_id)
                .await
                .map_err(|e| format!("Failed to inspect node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Bind to a new node
    pub async fn bind_node(&self, endpoint: &str, did: Option<String>) -> Result<NodeInfo, String> {
        if let Some(ref service) = self.node_service {
            let options = BindOptions {
                endpoint: endpoint.to_string(),
                did,
                trust_level: TrustLevel::Limited,
                auto_sync: false,
            };

            service
                .bind(options)
                .await
                .map_err(|e| format!("Failed to bind node: {}", e))
        } else {
            Err("NodeService not available".to_string())
        }
    }

    /// Get demo nodes
    pub fn demo_nodes(&self) -> &[ManagedNode] {
        &self.demo_nodes
    }
}

impl ViewModel for NodeViewModel {
    fn name(&self) -> &str {
        "NodeViewModel"
    }

    fn needs_refresh(&self) -> bool {
        self.state.is_dirty()
    }

    fn mark_dirty(&self) {
        self.state.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_vm_creation() {
        let handle = tokio::runtime::Runtime::new()
            .unwrap()
            .handle()
            .clone();

        let vm = NodeViewModel::new(None, handle);
        assert_eq!(vm.name(), "NodeViewModel");
        assert!(!vm.demo_nodes().is_empty());
    }

    #[tokio::test]
    async fn test_get_demo_nodes() {
        let handle = tokio::runtime::Runtime::new()
            .unwrap()
            .handle()
            .clone();

        let vm = NodeViewModel::new(None, handle);
        let nodes = vm.get_nodes().await;
        assert_eq!(nodes.len(), 4); // Should have 4 demo nodes
    }
}
