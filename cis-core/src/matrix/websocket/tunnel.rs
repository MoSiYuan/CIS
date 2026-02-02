//! # WebSocket Tunnel Management
//!
//! Connection tunnel management for WebSocket federation.
//!
//! ## Features
//!
//! - Connection pooling
//! - Heartbeat keep-alive (5 second interval)
//! - Automatic reconnection
//! - Message broadcast

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::protocol::{AckMessage, EventMessage, PingMessage, PongMessage, WsMessage};
use crate::matrix::federation::types::CisMatrixEvent;

/// Default heartbeat interval (5 seconds)
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// Connection timeout (30 seconds)
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum missed heartbeats before considering connection dead
const MAX_MISSED_HEARTBEATS: u32 = 3;

/// Tunnel state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TunnelState {
    /// Initial connecting state
    Connecting,
    /// Handshake in progress
    Handshaking,
    /// Authenticating
    Authenticating,
    /// Ready for communication
    Ready,
    /// Reconnecting
    Reconnecting,
    /// Disconnected
    Disconnected,
    /// Error state
    Error,
}

/// WebSocket tunnel for peer communication
#[derive(Debug)]
pub struct Tunnel {
    /// Remote node ID
    pub node_id: String,
    /// Current state
    state: RwLock<TunnelState>,
    /// Message sender channel
    sender: mpsc::UnboundedSender<WsMessage>,
    /// Last activity timestamp
    last_activity: RwLock<Instant>,
    /// Missed heartbeat count
    missed_heartbeats: AtomicU64,
    /// Connection established timestamp
    connected_at: RwLock<Option<Instant>>,
    /// Statistics
    stats: RwLock<TunnelStats>,
}

/// Tunnel statistics
#[derive(Debug, Clone, Default)]
pub struct TunnelStats {
    /// Messages sent
    pub messages_sent: u64,
    /// Messages received
    pub messages_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Reconnect count
    pub reconnect_count: u32,
    /// Last error (if any)
    pub last_error: Option<String>,
}

impl Tunnel {
    /// Create a new tunnel
    pub fn new(node_id: impl Into<String>, sender: mpsc::UnboundedSender<WsMessage>) -> Self {
        Self {
            node_id: node_id.into(),
            state: RwLock::new(TunnelState::Connecting),
            sender,
            last_activity: RwLock::new(Instant::now()),
            missed_heartbeats: AtomicU64::new(0),
            connected_at: RwLock::new(None),
            stats: RwLock::new(TunnelStats::default()),
        }
    }

    /// Send a message through the tunnel
    pub async fn send(&self, message: WsMessage) -> Result<(), TunnelError> {
        self.update_activity().await;
        self.sender.send(message).map_err(|_| TunnelError::SendError)
    }

    /// Send an event message
    pub async fn send_event(&self, event: &CisMatrixEvent) -> Result<(), TunnelError> {
        let event_data = serde_json::to_vec(event)
            .map_err(|e| TunnelError::SerializationError(e.to_string()))?;
        let event_len = event_data.len();

        let message = EventMessage::new(
            format!("evt-{}", uuid::Uuid::new_v4()),
            event_data,
            &event.event_type,
            &event.sender,
        );

        let ws_message = WsMessage::Event(message);
        self.send(ws_message).await?;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.messages_sent += 1;
        stats.bytes_sent += event_len as u64;

        Ok(())
    }

    /// Send ping
    pub async fn send_ping(&self, ping_id: u64) -> Result<(), TunnelError> {
        let ping = WsMessage::Ping(PingMessage::new(ping_id));
        self.send(ping).await
    }

    /// Send pong
    pub async fn send_pong(&self, ping_id: u64) -> Result<(), TunnelError> {
        let pong = WsMessage::Pong(PongMessage::new(ping_id));
        self.send(pong).await
    }

    /// Send acknowledgment
    pub async fn send_ack(&self, message_id: &str, success: bool) -> Result<(), TunnelError> {
        let ack = if success {
            AckMessage::success(message_id)
        } else {
            AckMessage::failed(message_id, "processing failed")
        };
        self.send(WsMessage::Ack(ack)).await
    }

    /// Get current state
    pub async fn state(&self) -> TunnelState {
        *self.state.read().await
    }

    /// Set state
    pub async fn set_state(&self, state: TunnelState) {
        *self.state.write().await = state;
        if state == TunnelState::Ready {
            *self.connected_at.write().await = Some(Instant::now());
        }
    }

    /// Update last activity timestamp
    pub async fn update_activity(&self) {
        *self.last_activity.write().await = Instant::now();
        self.missed_heartbeats.store(0, Ordering::SeqCst);
    }

    /// Get last activity timestamp
    pub async fn last_activity(&self) -> Instant {
        *self.last_activity.read().await
    }

    /// Increment missed heartbeats
    pub fn increment_missed_heartbeats(&self) -> u64 {
        self.missed_heartbeats.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get missed heartbeat count
    pub fn missed_heartbeats(&self) -> u64 {
        self.missed_heartbeats.load(Ordering::SeqCst)
    }

    /// Check if tunnel is healthy
    pub async fn is_healthy(&self) -> bool {
        let state = self.state().await;
        state == TunnelState::Ready && self.missed_heartbeats() < MAX_MISSED_HEARTBEATS as u64
    }

    /// Check if tunnel is ready for sending
    pub async fn is_ready(&self) -> bool {
        self.state().await == TunnelState::Ready
    }

    /// Get connection duration
    pub async fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.read().await.map(|t| t.elapsed())
    }

    /// Get statistics
    pub async fn stats(&self) -> TunnelStats {
        self.stats.read().await.clone()
    }

    /// Update received stats
    pub async fn record_received(&self, bytes: usize) {
        let mut stats = self.stats.write().await;
        stats.messages_received += 1;
        stats.bytes_received += bytes as u64;
    }

    /// Record error
    pub async fn record_error(&self, error: impl Into<String>) {
        let mut stats = self.stats.write().await;
        stats.last_error = Some(error.into());
    }
}

/// Tunnel manager for managing multiple peer connections
#[derive(Debug)]
pub struct TunnelManager {
    /// Active tunnels
    tunnels: Arc<RwLock<HashMap<String, Arc<Tunnel>>>>,
    /// Heartbeat interval
    heartbeat_interval: Duration,
    /// Event channel for incoming events
    event_tx: mpsc::Sender<(String, CisMatrixEvent)>,
    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

impl TunnelManager {
    /// Create a new tunnel manager
    pub fn new(
        heartbeat_interval: Duration,
        event_tx: mpsc::Sender<(String, CisMatrixEvent)>,
    ) -> Self {
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_interval,
            event_tx,
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Create with default heartbeat interval
    pub fn with_event_channel(event_tx: mpsc::Sender<(String, CisMatrixEvent)>) -> Self {
        Self::new(HEARTBEAT_INTERVAL, event_tx)
    }

    /// Register a new tunnel
    pub async fn register_tunnel(
        &self,
        node_id: impl Into<String>,
        sender: mpsc::UnboundedSender<WsMessage>,
    ) -> Arc<Tunnel> {
        let node_id = node_id.into();
        let tunnel = Arc::new(Tunnel::new(&node_id, sender));

        let mut tunnels = self.tunnels.write().await;
        tunnels.insert(node_id.clone(), tunnel.clone());

        info!("Registered tunnel for node: {}", node_id);
        tunnel
    }

    /// Remove a tunnel
    pub async fn remove_tunnel(&self, node_id: &str) -> Option<Arc<Tunnel>> {
        let mut tunnels = self.tunnels.write().await;
        let removed = tunnels.remove(node_id);

        if removed.is_some() {
            info!("Removed tunnel for node: {}", node_id);
        }

        removed
    }

    /// Get a tunnel by node ID
    pub async fn get_tunnel(&self, node_id: &str) -> Option<Arc<Tunnel>> {
        let tunnels = self.tunnels.read().await;
        tunnels.get(node_id).cloned()
    }

    /// Get all tunnels
    pub async fn get_all_tunnels(&self) -> Vec<Arc<Tunnel>> {
        let tunnels = self.tunnels.read().await;
        tunnels.values().cloned().collect()
    }

    /// Get all ready tunnels
    pub async fn get_ready_tunnels(&self) -> Vec<Arc<Tunnel>> {
        let tunnels = self.tunnels.read().await;
        let mut ready = Vec::new();
        for tunnel in tunnels.values() {
            if tunnel.is_ready().await {
                ready.push(tunnel.clone());
            }
        }
        ready
    }

    /// Get tunnel count
    pub async fn tunnel_count(&self) -> usize {
        self.tunnels.read().await.len()
    }

    /// Broadcast an event to all ready tunnels
    pub async fn broadcast(&self, event: &CisMatrixEvent) -> HashMap<String, Result<(), TunnelError>> {
        let tunnels = self.get_ready_tunnels().await;
        let mut results = HashMap::new();

        for tunnel in tunnels {
            let result = tunnel.send_event(event).await;
            results.insert(tunnel.node_id.clone(), result);
        }

        results
    }

    /// Send event to specific node
    pub async fn send_to(
        &self,
        node_id: &str,
        event: &CisMatrixEvent,
    ) -> Result<(), TunnelError> {
        match self.get_tunnel(node_id).await {
            Some(tunnel) => tunnel.send_event(event).await,
            None => Err(TunnelError::TunnelNotFound(node_id.to_string())),
        }
    }

    /// Send raw WebSocket message to specific node
    pub async fn send_message(
        &self,
        node_id: &str,
        message: WsMessage,
    ) -> Result<(), TunnelError> {
        match self.get_tunnel(node_id).await {
            Some(tunnel) => tunnel.send(message).await,
            None => Err(TunnelError::TunnelNotFound(node_id.to_string())),
        }
    }

    /// Check if a node is connected (has a ready tunnel)
    pub async fn is_connected(&self, node_id: &str) -> bool {
        match self.get_tunnel(node_id).await {
            Some(tunnel) => tunnel.is_ready().await,
            None => false,
        }
    }

    /// Start maintenance tasks (heartbeat, timeout cleanup)
    pub async fn start_maintenance(&self) {
        let tunnels = self.tunnels.clone();
        let interval_duration = self.heartbeat_interval;

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        let mut heartbeat_timer = interval(interval_duration);
        let mut cleanup_timer = interval(Duration::from_secs(30));

        info!("Tunnel manager maintenance started");

        loop {
            tokio::select! {
                _ = heartbeat_timer.tick() => {
                    Self::send_heartbeats(&tunnels).await;
                }
                _ = cleanup_timer.tick() => {
                    Self::cleanup_dead_tunnels(&tunnels).await;
                }
                _ = shutdown_rx.recv() => {
                    info!("Tunnel manager maintenance shutting down");
                    break;
                }
            }
        }
    }

    /// Send heartbeats to all tunnels
    async fn send_heartbeats(tunnels: &Arc<RwLock<HashMap<String, Arc<Tunnel>>>>) {
        let tunnels_guard = tunnels.read().await;
        let mut ping_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for (node_id, tunnel) in tunnels_guard.iter() {
            if !tunnel.is_ready().await {
                continue;
            }

            // Check if we've missed too many heartbeats
            if tunnel.missed_heartbeats() >= MAX_MISSED_HEARTBEATS as u64 {
                warn!(
                    "Tunnel {} has missed {} heartbeats, marking for reconnection",
                    node_id,
                    tunnel.missed_heartbeats()
                );
                let _ = tunnel.set_state(TunnelState::Reconnecting).await;
                continue;
            }

            // Send ping
            match tunnel.send_ping(ping_id).await {
                Ok(_) => {
                    debug!("Sent ping {} to {}", ping_id, node_id);
                    tunnel.increment_missed_heartbeats();
                }
                Err(e) => {
                    warn!("Failed to send ping to {}: {:?}", node_id, e);
                    tunnel.increment_missed_heartbeats();
                }
            }
            ping_id += 1;
        }
    }

    /// Cleanup dead tunnels
    async fn cleanup_dead_tunnels(tunnels: &Arc<RwLock<HashMap<String, Arc<Tunnel>>>>) {
        let mut tunnels_guard = tunnels.write().await;
        let dead_nodes: Vec<String> = tunnels_guard
            .iter()
            .filter(|(_, tunnel)| {
                let missed = tunnel.missed_heartbeats();
                missed >= (MAX_MISSED_HEARTBEATS * 2) as u64
            })
            .map(|(node_id, _)| node_id.clone())
            .collect();

        for node_id in dead_nodes {
            warn!("Removing dead tunnel for node: {}", node_id);
            tunnels_guard.remove(&node_id);
        }
    }

    /// Shutdown maintenance
    pub async fn shutdown(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
    }

    /// Handle pong response
    pub async fn handle_pong(&self, node_id: &str, ping_id: u64) {
        if let Some(tunnel) = self.get_tunnel(node_id).await {
            tunnel.update_activity().await;
            debug!("Received pong {} from {}", ping_id, node_id);
        }
    }

    /// Handle incoming event
    pub async fn handle_event(&self, node_id: &str, event_msg: EventMessage) {
        if let Some(tunnel) = self.get_tunnel(node_id).await {
            tunnel.update_activity().await;
            tunnel.record_received(event_msg.event_data.len()).await;

            // Parse and forward event
            match serde_json::from_slice::<CisMatrixEvent>(&event_msg.event_data) {
                Ok(event) => {
                    let _ = self.event_tx.send((node_id.to_string(), event)).await;
                }
                Err(e) => {
                    error!("Failed to parse event from {}: {}", node_id, e);
                    tunnel.record_error(format!("Parse error: {}", e)).await;
                }
            }
        }
    }
}

/// Tunnel errors
#[derive(Debug, thiserror::Error, Clone)]
pub enum TunnelError {
    /// Tunnel not found
    #[error("Tunnel not found: {0}")]
    TunnelNotFound(String),

    /// Send error
    #[error("Failed to send message")]
    SendError,

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout
    #[error("Operation timeout")]
    Timeout,

    /// Not ready
    #[error("Tunnel not ready")]
    NotReady,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tunnel_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let tunnel = Tunnel::new("test-node", tx);

        assert_eq!(tunnel.node_id, "test-node");
        assert!(matches!(tunnel.state().await, TunnelState::Connecting));
    }

    #[tokio::test]
    async fn test_tunnel_state_changes() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let tunnel = Tunnel::new("test-node", tx);

        tunnel.set_state(TunnelState::Ready).await;
        assert!(tunnel.is_ready().await);
        assert!(tunnel.is_healthy().await);

        tunnel.set_state(TunnelState::Error).await;
        assert!(!tunnel.is_ready().await);
    }

    #[tokio::test]
    async fn test_tunnel_manager() {
        let (event_tx, _event_rx) = mpsc::channel(100);
        let manager = TunnelManager::with_event_channel(event_tx);

        // Register a tunnel
        let (ws_tx, _ws_rx) = mpsc::unbounded_channel();
        let tunnel = manager.register_tunnel("node-1", ws_tx).await;

        assert_eq!(manager.tunnel_count().await, 1);

        // Get tunnel
        let retrieved = manager.get_tunnel("node-1").await;
        assert!(retrieved.is_some());

        // Remove tunnel
        manager.remove_tunnel("node-1").await;
        assert_eq!(manager.tunnel_count().await, 0);
    }

    #[tokio::test]
    async fn test_tunnel_stats() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let tunnel = Tunnel::new("test-node", tx);

        tunnel.record_received(100).await;
        tunnel.record_received(200).await;

        let stats = tunnel.stats().await;
        assert_eq!(stats.messages_received, 2);
        assert_eq!(stats.bytes_received, 300);
    }
}
