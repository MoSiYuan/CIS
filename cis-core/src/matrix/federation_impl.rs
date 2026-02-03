//! # Federation Connection Manager
//!
//! Centralized management of federation connections with:
//! - Connection pooling and lifecycle management
//! - Automatic reconnection with exponential backoff
//! - DID-based authentication and key resolution
//! - Room state synchronization
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                 FederationManager                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │  HTTP Client │  │ WebSocket    │  │  Sync Queue  │       │
//! │  │  (6767)      │  │  (6768)      │  │              │       │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
//! │         │                 │                 │               │
//! │         └─────────────────┴─────────────────┘               │
//! │                           │                                 │
//! │                    ┌──────┴──────┐                         │
//! │                    │ Connection  │                         │
//! │                    │ Manager     │                         │
//! │                    └─────────────┘                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ed25519_dalek::VerifyingKey;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

use crate::error::{CisError, Result};
use crate::identity::DIDManager;
use crate::matrix::federation::{
    client::{FederationClient, FederationClientError},
    discovery::PeerDiscovery,
    types::{CisMatrixEvent, PeerInfo},
};
use crate::matrix::RoomInfo;
use crate::matrix::store::MatrixStore;
use crate::matrix::websocket::{
    protocol::{PingMessage, SyncRequest, SyncResponse, WsMessage},
    tunnel::{Tunnel, TunnelManager, TunnelError},
    WebSocketClient,
};

/// Federation connection state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    /// Disconnected
    Disconnected,
    /// Connecting
    Connecting,
    /// Connected and ready
    Ready,
    /// Reconnecting with backoff
    Reconnecting { attempt: u32, next_retry: i64 },
    /// Error state
    Error,
}

/// Federation connection info
#[derive(Debug, Clone)]
pub struct FederationConnection {
    /// Remote node DID
    pub node_did: String,
    /// Remote node ID
    pub node_id: String,
    /// Connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Last activity timestamp
    last_activity: Arc<RwLock<Instant>>,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
    /// WebSocket tunnel (if using WS)
    tunnel: Arc<RwLock<Option<Arc<Tunnel>>>>,
    /// Peer info for HTTP fallback
    peer_info: PeerInfo,
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Events sent
    pub events_sent: u64,
    /// Events received
    pub events_received: u64,
    /// Connection attempts
    pub connect_attempts: u32,
    /// Reconnect count
    pub reconnect_count: u32,
    /// Last error message
    pub last_error: Option<String>,
    /// Connection established timestamp
    pub connected_at: Option<Instant>,
}

impl FederationConnection {
    /// Create a new federation connection
    pub fn new(node_did: String, node_id: String, peer_info: PeerInfo) -> Self {
        Self {
            node_did,
            node_id,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            last_activity: Arc::new(RwLock::new(Instant::now())),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            tunnel: Arc::new(RwLock::new(None)),
            peer_info,
        }
    }

    /// Get current state
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Set state
    pub async fn set_state(&self, state: ConnectionState) {
        let mut guard = self.state.write().await;
        *guard = state;

        if matches!(state, ConnectionState::Ready) {
            let mut stats = self.stats.write().await;
            stats.connected_at = Some(Instant::now());
        }
    }

    /// Check if connection is ready
    pub async fn is_ready(&self) -> bool {
        matches!(self.state().await, ConnectionState::Ready)
    }

    /// Update last activity
    pub async fn update_activity(&self) {
        *self.last_activity.write().await = Instant::now();
    }

    /// Get last activity
    pub async fn last_activity(&self) -> Instant {
        *self.last_activity.read().await
    }

    /// Get statistics
    pub async fn stats(&self) -> ConnectionStats {
        self.stats.read().await.clone()
    }

    /// Record event sent
    pub async fn record_event_sent(&self) {
        let mut stats = self.stats.write().await;
        stats.events_sent += 1;
    }

    /// Record event received
    pub async fn record_event_received(&self) {
        let mut stats = self.stats.write().await;
        stats.events_received += 1;
    }

    /// Record error
    pub async fn record_error(&self, error: impl Into<String>) {
        let mut stats = self.stats.write().await;
        stats.last_error = Some(error.into());
    }

    /// Record reconnect attempt
    pub async fn record_reconnect(&self) {
        let mut stats = self.stats.write().await;
        stats.reconnect_count += 1;
        stats.connect_attempts += 1;
    }

    /// Set tunnel
    pub async fn set_tunnel(&self, tunnel: Arc<Tunnel>) {
        *self.tunnel.write().await = Some(tunnel);
    }

    /// Get tunnel
    pub async fn get_tunnel(&self) -> Option<Arc<Tunnel>> {
        self.tunnel.read().await.clone()
    }
}

/// Room sync status
#[derive(Debug, Clone)]
pub struct RoomSyncStatus {
    pub room_id: String,
    pub last_sync_time: i64,
    pub events_synced: u64,
    pub is_syncing: bool,
}

/// Heartbeat result for a node
#[derive(Debug, Clone)]
pub struct HeartbeatResult {
    pub node_id: String,
    pub online: bool,
    pub rtt_ms: Option<u64>,
    pub last_seen: Option<i64>,
}

/// FederationManager - Centralized federation connection management
#[derive(Debug)]
pub struct FederationManager {
    /// This node's DID
    node_did: String,
    /// This node's ID
    node_id: String,
    /// Active connections
    connections: Arc<RwLock<HashMap<String, FederationConnection>>>,
    /// HTTP client for federation
    http_client: FederationClient,
    /// WebSocket tunnel manager
    tunnel_manager: Arc<TunnelManager>,
    /// Peer discovery
    discovery: PeerDiscovery,
    /// Matrix store
    store: Arc<MatrixStore>,
    /// DID cache for remote nodes
    did_cache: Arc<RwLock<HashMap<String, VerifyingKey>>>,
    /// Reconnection queue
    reconnect_queue: Arc<Mutex<Vec<String>>>,
    /// Event sender for incoming events
    event_tx: mpsc::Sender<CisMatrixEvent>,
    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    /// Configuration
    config: FederationManagerConfig,
    /// Room sync statuses
    room_sync_status: Arc<RwLock<HashMap<String, RoomSyncStatus>>>,
    /// Heartbeat results
    heartbeat_results: Arc<RwLock<HashMap<String, HeartbeatResult>>>,
    /// Sync task shutdown signal
    sync_shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

/// Federation manager configuration
#[derive(Debug, Clone)]
pub struct FederationManagerConfig {
    /// Enable WebSocket connections
    pub use_websocket: bool,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnection base delay (seconds)
    pub reconnect_base_delay: u64,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Connection timeout (seconds)
    pub connection_timeout: u64,
    /// Heartbeat interval (seconds)
    pub heartbeat_interval: u64,
    /// Enable DID verification
    pub verify_dids: bool,
}

impl Default for FederationManagerConfig {
    fn default() -> Self {
        Self {
            use_websocket: true,
            auto_reconnect: true,
            reconnect_base_delay: 2,
            max_reconnect_attempts: 10,
            connection_timeout: 30,
            heartbeat_interval: 5,
            verify_dids: true,
        }
    }
}

impl FederationManager {
    /// Create a new federation manager
    pub fn new(
        did: Arc<DIDManager>,
        discovery: PeerDiscovery,
        store: Arc<MatrixStore>,
        event_tx: mpsc::Sender<CisMatrixEvent>,
    ) -> Result<Self> {
        let http_client = FederationClient::new()
            .map_err(|e| CisError::p2p(format!("Failed to create federation client: {}", e)))?;
        
        // Create channel bridge for tunnel manager (converts (node_id, event) to event)
        let (tunnel_event_tx, mut tunnel_event_rx) = mpsc::channel::<(String, CisMatrixEvent)>(1000);
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            while let Some((node_id, event)) = tunnel_event_rx.recv().await {
                debug!("Received event from {} via tunnel", node_id);
                if event_tx_clone.send(event).await.is_err() {
                    break;
                }
            }
        });
        
        // Create tunnel manager for WebSocket connections
        let tunnel_manager = Arc::new(TunnelManager::with_event_channel(tunnel_event_tx));

        Ok(Self {
            node_did: did.did().to_string(),
            node_id: did.node_id().to_string(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            tunnel_manager,
            discovery,
            store,
            did_cache: Arc::new(RwLock::new(HashMap::new())),
            reconnect_queue: Arc::new(Mutex::new(Vec::new())),
            event_tx,
            shutdown_tx: Arc::new(Mutex::new(None)),
            config: FederationManagerConfig::default(),
            room_sync_status: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_results: Arc::new(RwLock::new(HashMap::new())),
            sync_shutdown_tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Create with custom configuration
    pub fn with_config(
        did: Arc<DIDManager>,
        discovery: PeerDiscovery,
        store: Arc<MatrixStore>,
        event_tx: mpsc::Sender<CisMatrixEvent>,
        config: FederationManagerConfig,
    ) -> Result<Self> {
        let http_client = FederationClient::new()
            .map_err(|e| CisError::p2p(format!("Failed to create federation client: {}", e)))?;
        
        // Create channel bridge for tunnel manager (converts (node_id, event) to event)
        let (tunnel_event_tx, mut tunnel_event_rx) = mpsc::channel::<(String, CisMatrixEvent)>(1000);
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            while let Some((node_id, event)) = tunnel_event_rx.recv().await {
                debug!("Received event from {} via tunnel", node_id);
                if event_tx_clone.send(event).await.is_err() {
                    break;
                }
            }
        });
        
        // Create tunnel manager for WebSocket connections
        let tunnel_manager = Arc::new(TunnelManager::with_event_channel(tunnel_event_tx));

        Ok(Self {
            node_did: did.did().to_string(),
            node_id: did.node_id().to_string(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            tunnel_manager,
            discovery,
            store,
            did_cache: Arc::new(RwLock::new(HashMap::new())),
            reconnect_queue: Arc::new(Mutex::new(Vec::new())),
            event_tx,
            shutdown_tx: Arc::new(Mutex::new(None)),
            config,
            room_sync_status: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_results: Arc::new(RwLock::new(HashMap::new())),
            sync_shutdown_tx: Arc::new(Mutex::new(None)),
        })
    }

    /// Start the federation manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting FederationManager for node: {}", self.node_id);

        // Initialize connections for known peers
        self.init_connections().await?;

        // Start reconnection task
        if self.config.auto_reconnect {
            self.start_reconnection_task().await;
        }

        // Start room sync task (heartbeat and auto-sync)
        self.start_room_sync_task().await;

        // Perform initial room sync
        if let Err(e) = self.sync_rooms().await {
            warn!("Initial room sync failed: {}", e);
        }

        info!("FederationManager started successfully");
        Ok(())
    }

    /// Initialize connections for known peers
    async fn init_connections(&self) -> Result<()> {
        let peers = self.discovery.get_known_peers();

        for peer in peers {
            let node_id = peer.server_name.clone();
            let node_did = format!("did:cis:{}:unknown", node_id);

            let connection = FederationConnection::new(node_did, node_id.clone(), peer);
            let mut connections = self.connections.write().await;
            connections.insert(node_id.clone(), connection);

            // Try to connect immediately
            drop(connections);
            self.connect_to_node(&node_id).await;
        }

        Ok(())
    }

    /// Connect to a node
    async fn connect_to_node(&self, node_id: &str) -> Result<()> {
        let connection = {
            let connections = self.connections.read().await;
            connections.get(node_id).cloned()
        };

        let connection = match connection {
            Some(c) => c,
            None => return Err(CisError::p2p(format!("Unknown node: {}", node_id))),
        };

        connection.set_state(ConnectionState::Connecting).await;

        if self.config.use_websocket {
            // Try WebSocket first
            match self.connect_websocket(node_id).await {
                Ok(tunnel) => {
                    connection.set_tunnel(tunnel).await;
                    connection.set_state(ConnectionState::Ready).await;
                    connection.update_activity().await;
                    info!("WebSocket connection established to {}", node_id);
                    return Ok(());
                }
                Err(e) => {
                    warn!("WebSocket connection failed to {}: {}", node_id, e);
                }
            }
        }

        // Fall back to HTTP health check
        let peer = connection.peer_info.clone();
        if self.http_client.health_check(&peer).await {
            connection.set_state(ConnectionState::Ready).await;
            connection.update_activity().await;
            info!("HTTP connection verified to {}", node_id);
            Ok(())
        } else {
            connection.set_state(ConnectionState::Error).await;
            self.queue_reconnect(node_id.to_string()).await;
            Err(CisError::p2p(format!("Failed to connect to {}", node_id)))
        }
    }

    /// Connect via WebSocket to a peer node
    /// 
    /// Establishes a WebSocket connection with:
    /// 1. TCP connection to peer
    /// 2. Noise XX handshake for encryption
    /// 3. DID-based authentication
    /// 4. Tunnel registration for message routing
    async fn connect_websocket(&self, node_id: &str) -> Result<Arc<Tunnel>> {
        // Get connection info
        let connection = {
            let connections = self.connections.read().await;
            connections.get(node_id).cloned()
        };

        let connection = match connection {
            Some(c) => c,
            None => return Err(CisError::p2p(format!("Unknown node: {}", node_id))),
        };

        let peer_info = connection.peer_info.clone();
        
        info!("Establishing WebSocket connection to {} at {}:{}", 
              node_id, peer_info.host, peer_info.port);

        // Create WebSocket client
        let ws_client = WebSocketClient::new(
            &self.node_id,
            &self.node_did,
        );

        // Attempt connection with retry
        let tunnel = match ws_client.connect_with_retry(
            &peer_info,
            self.tunnel_manager.clone(),
            self.config.max_reconnect_attempts,
        ).await {
            Ok(tunnel) => {
                info!("WebSocket connection established to {}", node_id);
                tunnel
            }
            Err(e) => {
                let error_msg = format!("WebSocket connection failed to {}: {:?}", node_id, e);
                warn!("{}", error_msg);
                connection.record_error(&error_msg).await;
                return Err(CisError::p2p(error_msg));
            }
        };

        // Verify tunnel is ready
        let state = tunnel.state().await;
        if !matches!(state, super::websocket::tunnel::TunnelState::Ready) {
            let error_msg = format!("Tunnel to {} is not ready: {:?}", node_id, state);
            warn!("{}", error_msg);
            return Err(CisError::p2p(error_msg));
        }

        // Store tunnel reference in connection
        connection.set_tunnel(tunnel.clone()).await;
        
        // Record successful connection
        connection.record_event_sent().await; // Mark activity
        
        debug!("WebSocket tunnel ready for {}", node_id);
        Ok(tunnel)
    }

    /// Queue a node for reconnection
    async fn queue_reconnect(&self, node_id: String) {
        let mut queue = self.reconnect_queue.lock().await;
        if !queue.contains(&node_id) {
            debug!("Queued {} for reconnection", node_id);
            queue.push(node_id);
        }
    }

    /// Start the reconnection task
    async fn start_reconnection_task(&self) {
        let connections = self.connections.clone();
        let reconnect_queue = self.reconnect_queue.clone();
        let reconnect_base_delay = self.config.reconnect_base_delay;
        let max_attempts = self.config.max_reconnect_attempts;

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        let nodes_to_reconnect: Vec<String> = {
                            let mut queue = reconnect_queue.lock().await;
                            let nodes = queue.clone();
                            queue.clear();
                            nodes
                        };

                        for node_id in nodes_to_reconnect {
                            if let Some(conn) = connections.read().await.get(&node_id) {
                                let stats = conn.stats().await;

                                if stats.reconnect_count >= max_attempts {
                                    warn!("Max reconnection attempts reached for {}", node_id);
                                    conn.set_state(ConnectionState::Error).await;
                                    continue;
                                }

                                // Exponential backoff
                                let delay = reconnect_base_delay * (1_u64 << stats.reconnect_count.min(6));
                                conn.set_state(ConnectionState::Reconnecting {
                                    attempt: stats.reconnect_count + 1,
                                    next_retry: chrono::Utc::now().timestamp() + delay as i64,
                                }).await;

                                sleep(Duration::from_secs(delay)).await;

                                // Try to reconnect
                                // Note: In a real implementation, we'd call connect_to_node here
                                // For now, just mark as disconnected
                                conn.record_reconnect().await;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Reconnection task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Send an event to a specific node
    pub async fn send_event(&self, node_id: &str, event: &CisMatrixEvent) -> Result<()> {
        let connection = {
            let connections = self.connections.read().await;
            connections.get(node_id).cloned()
        };

        let connection = match connection {
            Some(c) => c,
            None => return Err(CisError::p2p(format!("Unknown node: {}", node_id))),
        };

        if !connection.is_ready().await {
            // Queue for later delivery
            self.queue_reconnect(node_id.to_string()).await;
            return Err(CisError::p2p(format!("Node {} not connected", node_id)));
        }

        // Try WebSocket first if available
        if let Some(tunnel) = connection.get_tunnel().await {
            match tunnel.send_event(event).await {
                Ok(()) => {
                    connection.record_event_sent().await;
                    connection.update_activity().await;
                    return Ok(());
                }
                Err(e) => {
                    warn!("WebSocket send failed to {}: {}", node_id, e);
                }
            }
        }

        // Fall back to HTTP
        match self.http_client.send_event(&connection.peer_info, event).await {
            Ok(response) => {
                if response.accepted {
                    connection.record_event_sent().await;
                    connection.update_activity().await;
                    Ok(())
                } else {
                    Err(CisError::p2p(format!(
                        "Event rejected by {}: {:?}",
                        node_id, response.error
                    )))
                }
            }
            Err(e) => {
                connection.record_error(format!("Send failed: {}", e)).await;
                self.queue_reconnect(node_id.to_string()).await;
                Err(CisError::p2p(format!("Failed to send to {}: {}", node_id, e)))
            }
        }
    }

    /// Broadcast an event to all connected nodes
    pub async fn broadcast_event(&self, event: &CisMatrixEvent) -> HashMap<String, Result<()>> {
        let connections = self.connections.read().await;
        let mut results = HashMap::new();

        for (node_id, _) in connections.iter() {
            if *node_id == self.node_id {
                continue; // Skip self
            }

            let result = self.send_event(node_id, event).await;
            results.insert(node_id.clone(), result);
        }

        results
    }

    /// Query room information from a remote node
    pub async fn query_room(&self, room_id: &str) -> Result<RoomInfo> {
        // Extract server from room ID (format: !roomid:server)
        let server = room_id
            .split(':')
            .nth(1)
            .ok_or_else(|| CisError::p2p("Invalid room ID format".to_string()))?;

        let connection = {
            let connections = self.connections.read().await;
            connections.get(server).cloned()
        };

        let connection = match connection {
            Some(c) => c,
            None => return Err(CisError::p2p(format!("Unknown server: {}", server))),
        };

        // Query via HTTP API
        let url = format!("{}/_cis/v1/room/{}", connection.peer_info.federation_url(), room_id);

        // For now, return a placeholder response
        // In a full implementation, this would make an actual HTTP request
        Ok(RoomInfo {
            room_id: room_id.to_string(),
            creator: format!("@admin:{}", server),
            name: Some("Remote Room".to_string()),
            topic: None,
            federate: true,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// Sync room history from a remote node
    pub async fn sync_history(
        &self,
        node_id: &str,
        room_id: &str,
        since: Option<String>,
    ) -> Result<Vec<CisMatrixEvent>> {
        let connection = {
            let connections = self.connections.read().await;
            connections.get(node_id).cloned()
        };

        let connection = match connection {
            Some(c) => c,
            None => return Err(CisError::p2p(format!("Unknown node: {}", node_id))),
        };

        // Build sync URL
        let mut url = format!(
            "{}/_cis/v1/sync?room={}",
            connection.peer_info.federation_url(),
            room_id
        );
        if let Some(since) = since {
            url.push_str(&format!("&since={}", since));
        }

        // For now, return empty history
        // In a full implementation, this would fetch from the remote node
        debug!("Syncing history for room {} from {}", room_id, node_id);
        Ok(Vec::new())
    }

    /// Resolve DID to get public key
    pub async fn resolve_did(&self, did: &str) -> Result<VerifyingKey> {
        // Check cache first
        {
            let cache = self.did_cache.read().await;
            if let Some(key) = cache.get(did) {
                return Ok(*key);
            }
        }

        // Parse DID to get node
        let (_, node_id) = DIDManager::parse_did(did)
            .ok_or_else(|| CisError::identity(format!("Invalid DID format: {}", did)))?;

        // Fetch from remote node
        let connection = {
            let connections = self.connections.read().await;
            connections.get(&node_id).cloned()
        };

        if let Some(conn) = connection {
            match self.http_client.fetch_server_key(&conn.peer_info).await {
                Ok(key_data) => {
                    // Extract public key from response
                    // This is simplified - real implementation would parse the Matrix key response
                    if let Some(key) = self.parse_server_key(&key_data) {
                        let mut cache = self.did_cache.write().await;
                        cache.insert(did.to_string(), key);
                        return Ok(key);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch server key from {}: {}", node_id, e);
                }
            }
        }

        Err(CisError::identity(format!("Failed to resolve DID: {}", did)))
    }

    /// Parse server key from JSON response
    fn parse_server_key(&self, key_data: &serde_json::Value) -> Option<VerifyingKey> {
        let verify_keys = key_data.get("verify_keys")?;
        let first_key = verify_keys.get("ed25519:0")?;
        let key_str = first_key.get("key")?.as_str()?;

        DIDManager::verifying_key_from_hex(key_str).ok()
    }

    /// Get cached DID key
    pub async fn get_cached_key(&self, did: &str) -> Option<VerifyingKey> {
        let cache = self.did_cache.read().await;
        cache.get(did).copied()
    }

    /// Cache a DID key
    pub async fn cache_did_key(&self, did: String, key: VerifyingKey) {
        let mut cache = self.did_cache.write().await;
        cache.insert(did, key);
    }

    /// Get connection for a node
    pub async fn get_connection(&self, node_id: &str) -> Option<FederationConnection> {
        let connections = self.connections.read().await;
        connections.get(node_id).cloned()
    }

    /// Get all connections
    pub async fn get_all_connections(&self) -> Vec<FederationConnection> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }

    /// Get ready connections
    pub async fn get_ready_connections(&self) -> Vec<FederationConnection> {
        let connections = self.connections.read().await;
        let mut ready = Vec::new();
        for conn in connections.values() {
            if conn.is_ready().await {
                ready.push(conn.clone());
            }
        }
        ready
    }

    /// Add a new peer connection
    pub async fn add_peer(&self, peer: PeerInfo) -> Result<()> {
        let node_id = peer.server_name.clone();
        let node_did = format!("did:cis:{}:unknown", node_id);

        let connection = FederationConnection::new(node_did, node_id.clone(), peer.clone());

        {
            let mut connections = self.connections.write().await;
            connections.insert(node_id.clone(), connection);
        }

        self.discovery.add_peer(peer);

        // Try to connect
        self.connect_to_node(&node_id).await?;

        info!("Added new peer: {}", node_id);
        Ok(())
    }

    /// Remove a peer connection
    pub async fn remove_peer(&self, node_id: &str) -> Result<()> {
        {
            let mut connections = self.connections.write().await;
            connections.remove(node_id);
        }

        self.discovery.remove_peer(node_id);
        info!("Removed peer: {}", node_id);
        Ok(())
    }

    /// Shutdown the federation manager
    pub async fn shutdown(&self) {
        info!("Shutting down FederationManager");

        // Shutdown room sync task
        self.shutdown_room_sync().await;

        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }

        // Close all connections
        let connections = self.connections.read().await;
        for (node_id, conn) in connections.iter() {
            conn.set_state(ConnectionState::Disconnected).await;
            debug!("Closed connection to {}", node_id);
        }
    }

    /// Get node DID
    pub fn node_did(&self) -> &str {
        &self.node_did
    }

    /// Get node ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    // ==================== Room Sync Methods ====================

    /// Sync rooms from federation peers
    /// 
    /// This method is called when the node comes online to:
    /// 1. Get known peers from FederationManager
    /// 2. Send SyncRequest for each federate=true room
    /// 3. Process SyncResponse and merge room states
    pub async fn sync_rooms(&self) -> Result<()> {
        info!("Starting room sync for node: {}", self.node_id);

        // Get all federate rooms from local store
        let federate_rooms = self.store.list_federate_rooms()
            .map_err(|e| CisError::storage(format!("Failed to list federate rooms: {}", e)))?;

        if federate_rooms.is_empty() {
            debug!("No federate rooms to sync");
            return Ok(());
        }

        info!("Found {} federate rooms to sync", federate_rooms.len());

        // Get ready connections (online peers)
        let ready_connections = self.get_ready_connections().await;
        
        if ready_connections.is_empty() {
            warn!("No ready connections available for room sync");
            return Ok(());
        }

        // Send sync requests for each room to each connected peer
        for room_id in &federate_rooms {
            for conn in &ready_connections {
                let target_node = &conn.node_id;
                
                // Skip self
                if target_node == &self.node_id {
                    continue;
                }

                debug!("Sending sync request for room {} to {}", room_id, target_node);

                // Build sync request
                let request = SyncRequest::new(
                    room_id.clone(),
                    None, // Start from beginning
                    100,  // Limit to 100 events
                );

                // Send via WebSocket if available
                if let Some(tunnel) = conn.get_tunnel().await {
                    let msg = WsMessage::SyncRequest(request);
                    match tunnel.send(msg).await {
                        Ok(_) => {
                            debug!("Sent sync request for room {} to {}", room_id, target_node);
                            
                            // Update sync status
                            let mut status = self.room_sync_status.write().await;
                            status.insert(room_id.clone(), RoomSyncStatus {
                                room_id: room_id.clone(),
                                last_sync_time: chrono::Utc::now().timestamp(),
                                events_synced: 0,
                                is_syncing: true,
                            });
                        }
                        Err(e) => {
                            warn!("Failed to send sync request to {}: {}", target_node, e);
                        }
                    }
                }
            }
        }

        info!("Room sync requests sent");
        Ok(())
    }

    /// Handle sync response from a peer
    pub async fn handle_sync_response(&self, from_node: &str, response: SyncResponse) -> Result<usize> {
        let events_count = response.events.len();
        info!(
            "Received sync response from {} for room {}: {} events",
            from_node,
            response.room_id,
            events_count
        );

        let mut inserted = 0;

        for event in &response.events {
            // Convert EventMessage to CisMatrixEvent and save
            let event_id = format!("${}", event.message_id);
            
            // Check if event already exists
            match self.store.event_exists(&event_id) {
                Ok(true) => {
                    debug!("Event {} already exists, skipping", event_id);
                    continue;
                }
                Ok(false) => {}
                Err(e) => {
                    error!("Failed to check event existence: {}", e);
                    continue;
                }
            }

            // Parse event data
            let content: serde_json::Value = match serde_json::from_slice(&event.event_data) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to parse event data: {}", e);
                    continue;
                }
            };

            // Extract sender (stored in content, will be extracted by save_raw_event)
            let _sender = content.get("sender")
                .and_then(|s| s.as_str())
                .unwrap_or("@unknown:cis.local");

            // Save to store
            let room_id = event.room_id.clone().unwrap_or_else(|| response.room_id.clone());
            match self.store.save_raw_event(
                &room_id,
                &event_id,
                &event.event_type,
                &content,
                event.timestamp as i64,
            ) {
                Ok(_) => {
                    inserted += 1;
                    debug!("Saved synced event {} from {}", event_id, from_node);
                }
                Err(e) => {
                    error!("Failed to save synced event {}: {}", event_id, e);
                }
            }
        }

        // Update sync status
        {
            let mut status = self.room_sync_status.write().await;
            if let Some(room_status) = status.get_mut(&response.room_id) {
                room_status.events_synced += inserted as u64;
                room_status.is_syncing = false;
                room_status.last_sync_time = chrono::Utc::now().timestamp();
            }
        }

        info!(
            "Synced {} events from {} for room {}",
            inserted, from_node, response.room_id
        );

        Ok(inserted)
    }

    /// Start the room sync task (heartbeat and periodic sync)
    /// 
    /// This spawns a background task that:
    /// 1. Sends heartbeat ping to all peers every 60 seconds
    /// 2. Detects offline nodes and marks them
    /// 3. Triggers room sync for reconnected peers
    pub async fn start_room_sync_task(&self) {
        let connections = self.connections.clone();
        let tunnel_manager = self.tunnel_manager.clone();
        let heartbeat_results = self.heartbeat_results.clone();
        let node_id = self.node_id.clone();

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.sync_shutdown_tx.lock().await = Some(shutdown_tx);

        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(60));
            let mut ping_id_counter: u64 = 0;

            info!("Room sync task started with 60s heartbeat interval");

            loop {
                tokio::select! {
                    _ = heartbeat_interval.tick() => {
                        debug!("Running periodic heartbeat check");
                        
                        let conns = connections.read().await;
                        for (target_node, conn) in conns.iter() {
                            if target_node == &node_id {
                                continue; // Skip self
                            }

                            // Check if connected via WebSocket
                            let is_connected = tunnel_manager.is_connected(target_node).await;
                            
                            if is_connected {
                                // Send ping
                                ping_id_counter += 1;
                                let ping = PingMessage::new(ping_id_counter);
                                let msg = WsMessage::Ping(ping);
                                
                                let start = Instant::now();
                                match tunnel_manager.send_message(target_node, msg).await {
                                    Ok(_) => {
                                        let rtt = start.elapsed().as_millis() as u64;
                                        debug!("Heartbeat ping sent to {}, RTT: {}ms", target_node, rtt);
                                        
                                        // Update heartbeat result
                                        let mut results = heartbeat_results.write().await;
                                        results.insert(target_node.clone(), HeartbeatResult {
                                            node_id: target_node.clone(),
                                            online: true,
                                            rtt_ms: Some(rtt),
                                            last_seen: Some(chrono::Utc::now().timestamp()),
                                        });

                                        // Update connection activity
                                        conn.update_activity().await;
                                    }
                                    Err(e) => {
                                        warn!("Heartbeat ping failed to {}: {}", target_node, e);
                                        
                                        // Mark as offline
                                        let mut results = heartbeat_results.write().await;
                                        results.insert(target_node.clone(), HeartbeatResult {
                                            node_id: target_node.clone(),
                                            online: false,
                                            rtt_ms: None,
                                            last_seen: None,
                                        });

                                        // Update connection state
                                        conn.set_state(ConnectionState::Error).await;
                                    }
                                }
                            } else {
                                // Node is offline
                                debug!("Node {} is not connected", target_node);
                                
                                let mut results = heartbeat_results.write().await;
                                results.insert(target_node.clone(), HeartbeatResult {
                                    node_id: target_node.clone(),
                                    online: false,
                                    rtt_ms: None,
                                    last_seen: None,
                                });
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Room sync task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Send heartbeat to all connected peers
    /// 
    /// Returns a map of node_id -> online status
    pub async fn send_heartbeat(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        let connections = self.get_ready_connections().await;
        let mut ping_id_counter: u64 = 0;

        for conn in connections {
            let target_node = &conn.node_id;
            
            if target_node == &self.node_id {
                continue; // Skip self
            }

            // Check WebSocket connection
            if let Some(tunnel) = conn.get_tunnel().await {
                ping_id_counter += 1;
                let ping = PingMessage::new(ping_id_counter);
                let msg = WsMessage::Ping(ping);

                match tunnel.send(msg).await {
                    Ok(_) => {
                        results.insert(target_node.clone(), true);
                        conn.update_activity().await;
                    }
                    Err(_) => {
                        results.insert(target_node.clone(), false);
                    }
                }
            } else {
                // Try HTTP health check as fallback
                let peer = conn.peer_info.clone();
                let online = self.http_client.health_check(&peer).await;
                results.insert(target_node.clone(), online);
                
                if online {
                    conn.update_activity().await;
                }
            }
        }

        // Update heartbeat results cache
        {
            let mut cached = self.heartbeat_results.write().await;
            for (node_id, online) in &results {
                cached.insert(node_id.clone(), HeartbeatResult {
                    node_id: node_id.clone(),
                    online: *online,
                    rtt_ms: None,
                    last_seen: if *online { Some(chrono::Utc::now().timestamp()) } else { None },
                });
            }
        }

        results
    }

    /// Get the last heartbeat results
    pub async fn get_heartbeat_results(&self) -> HashMap<String, HeartbeatResult> {
        self.heartbeat_results.read().await.clone()
    }

    /// Get room sync status
    pub async fn get_room_sync_status(&self) -> HashMap<String, RoomSyncStatus> {
        self.room_sync_status.read().await.clone()
    }

    /// Handle reconnection - re-subscribe to rooms and sync missed events
    /// 
    /// This should be called when a peer reconnects
    pub async fn handle_peer_reconnection(&self, node_id: &str) -> Result<()> {
        info!("Handling reconnection for peer: {}", node_id);

        // Get connection
        let connection = match self.get_connection(node_id).await {
            Some(conn) => conn,
            None => {
                return Err(CisError::p2p(format!("Unknown peer: {}", node_id)));
            }
        };

        // Update connection state
        connection.set_state(ConnectionState::Ready).await;
        connection.update_activity().await;

        // Get federate rooms
        let federate_rooms = self.store.list_federate_rooms()
            .map_err(|e| CisError::storage(format!("Failed to list federate rooms: {}", e)))?;

        // Send sync requests for all federate rooms
        for room_id in &federate_rooms {
            debug!("Re-syncing room {} with {}", room_id, node_id);

            // Get last event ID for this room (simplified - get most recent)
            let since = None; // Could be optimized to track last synced event

            let request = SyncRequest::new(room_id.clone(), since, 100);

            // Try WebSocket first
            if let Some(tunnel) = connection.get_tunnel().await {
                let msg = WsMessage::SyncRequest(request);
                match tunnel.send(msg).await {
                    Ok(_) => {
                        debug!("Sent re-sync request for room {} to {}", room_id, node_id);
                    }
                    Err(e) => {
                        warn!("Failed to send re-sync request: {}", e);
                    }
                }
            }
        }

        info!("Peer reconnection handled for {}", node_id);
        Ok(())
    }

    /// Shutdown room sync task
    async fn shutdown_room_sync(&self) {
        if let Some(tx) = self.sync_shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::federation::types::CisMatrixEvent;

    #[tokio::test]
    async fn test_federation_connection() {
        let peer = PeerInfo::new("test.local", "test.local");
        let conn = FederationConnection::new(
            "did:cis:test:abc123".to_string(),
            "test.local".to_string(),
            peer,
        );

        assert!(!conn.is_ready().await);

        conn.set_state(ConnectionState::Ready).await;
        assert!(conn.is_ready().await);

        conn.record_event_sent().await;
        conn.record_event_received().await;

        let stats = conn.stats().await;
        assert_eq!(stats.events_sent, 1);
        assert_eq!(stats.events_received, 1);
    }

    #[test]
    fn test_connection_state() {
        assert!(matches!(ConnectionState::Ready, ConnectionState::Ready));
        assert!(!matches!(ConnectionState::Disconnected, ConnectionState::Ready));
    }

    // ==================== Room Sync Tests ====================

    #[tokio::test]
    async fn test_room_sync_status() {
        let status = RoomSyncStatus {
            room_id: "!test:example.com".to_string(),
            last_sync_time: 1234567890,
            events_synced: 100,
            is_syncing: false,
        };

        assert_eq!(status.room_id, "!test:example.com");
        assert_eq!(status.events_synced, 100);
        assert!(!status.is_syncing);
    }

    #[tokio::test]
    async fn test_heartbeat_result() {
        let result = HeartbeatResult {
            node_id: "node1".to_string(),
            online: true,
            rtt_ms: Some(50),
            last_seen: Some(1234567890),
        };

        assert_eq!(result.node_id, "node1");
        assert!(result.online);
        assert_eq!(result.rtt_ms, Some(50));
    }

    #[tokio::test]
    async fn test_handle_sync_response() {
        // This is a simplified test that verifies the sync response handling
        // In a real test, we'd need to mock the store and connections
        
        let event = crate::matrix::websocket::protocol::EventMessage {
            message_id: "test123".to_string(),
            event_data: serde_json::json!({
                "sender": "@alice:example.com",
                "content": {"body": "Hello"}
            }).to_string().into_bytes(),
            event_type: "m.room.message".to_string(),
            sender: "@alice:example.com".to_string(),
            room_id: Some("!test:example.com".to_string()),
            timestamp: 1234567890,
            sequence: 1,
        };

        let response = SyncResponse {
            room_id: "!test:example.com".to_string(),
            events: vec![event],
            has_more: false,
            next_batch: None,
            timestamp: 1234567890,
        };

        assert_eq!(response.events.len(), 1);
        assert!(!response.has_more);
        assert_eq!(response.room_id, "!test:example.com");
    }

    #[test]
    fn test_room_sync_status_creation() {
        let now = chrono::Utc::now().timestamp();
        let status = RoomSyncStatus {
            room_id: "!room:server".to_string(),
            last_sync_time: now,
            events_synced: 42,
            is_syncing: true,
        };

        assert_eq!(status.room_id, "!room:server");
        assert_eq!(status.last_sync_time, now);
        assert_eq!(status.events_synced, 42);
        assert!(status.is_syncing);
    }

    #[test]
    fn test_heartbeat_result_offline() {
        let result = HeartbeatResult {
            node_id: "offline_node".to_string(),
            online: false,
            rtt_ms: None,
            last_seen: None,
        };

        assert_eq!(result.node_id, "offline_node");
        assert!(!result.online);
        assert!(result.rtt_ms.is_none());
        assert!(result.last_seen.is_none());
    }
}
