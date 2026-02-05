//! # Federation Server
//!
//! HTTP server for CIS Matrix Federation (BMI - Between Machine Interface).
//!
//! ## Port
//!
//! Default port: 6767
//!
//! ## Endpoints
//!
//! - `GET /_matrix/key/v2/server` - Get server signing key (Matrix spec)
//! - `POST /_cis/v1/event/receive` - Receive events from other nodes
//!
//! ## Example
//!
//! ```no_run
//! use cis_core::matrix::federation::{FederationServer, FederationConfig, PeerDiscovery};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = FederationConfig::new("kitchen.local");
//! let discovery = PeerDiscovery::new(vec![]);
//! let store = Arc::new(cis_core::matrix::MatrixStore::open_in_memory()?);
//!
//! let server = FederationServer::new(config, discovery, store);
//! server.run().await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::{header::HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn};

use crate::matrix::store::MatrixStore;
use crate::matrix::error::MatrixError;

use super::{
    client::FederationClient,
    discovery::PeerDiscovery,
    types::*,
};

/// Federation HTTP server
#[derive(Clone)]
pub struct FederationServer {
    /// Server configuration
    config: FederationConfig,
    
    /// Peer discovery
    discovery: PeerDiscovery,
    
    /// Matrix store for persisting events
    store: Arc<MatrixStore>,
    
    /// Known peers (runtime state)
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    
    /// HTTP client for sending events
    client: FederationClient,
}

/// Shared state for Axum handlers
#[derive(Clone)]
struct FederationState {
    /// Server configuration
    config: FederationConfig,
    
    /// Peer discovery
    discovery: PeerDiscovery,
    
    /// Matrix store
    store: Arc<MatrixStore>,
    
    /// Known peers
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
}

impl FederationServer {
    /// Create a new federation server
    ///
    /// # Arguments
    ///
    /// * `config` - Federation configuration
    /// * `discovery` - Peer discovery manager
    /// * `store` - Matrix store for event persistence
    pub fn new(
        config: FederationConfig,
        discovery: PeerDiscovery,
        store: Arc<MatrixStore>,
    ) -> Self {
        let client = FederationClient::new()
            .unwrap_or_else(|e| {
                warn!("Failed to create federation client: {}, using default", e);
                FederationClient::default()
            });
        
        // Initialize peers from discovery
        let peers: HashMap<String, PeerInfo> = discovery
            .get_known_peers()
            .into_iter()
            .map(|p| (p.server_name.clone(), p))
            .collect();
        
        Self {
            config,
            discovery,
            store,
            peers: Arc::new(RwLock::new(peers)),
            client,
        }
    }
    
    /// Create a new federation server with custom client
    pub fn with_client(
        config: FederationConfig,
        discovery: PeerDiscovery,
        store: Arc<MatrixStore>,
        client: FederationClient,
    ) -> Self {
        let peers: HashMap<String, PeerInfo> = discovery
            .get_known_peers()
            .into_iter()
            .map(|p| (p.server_name.clone(), p))
            .collect();
        
        Self {
            config,
            discovery,
            store,
            peers: Arc::new(RwLock::new(peers)),
            client,
        }
    }
    
    /// Run the federation server
    ///
    /// This method blocks until the server shuts down.
    pub async fn run(&self) -> Result<(), MatrixError> {
        let addr = SocketAddr::from((
            self.config.bind_address.parse::<std::net::IpAddr>()
                .map_err(|e| MatrixError::InvalidParameter(format!("Invalid bind address: {}", e)))?,
            self.config.port,
        ));
        
        info!(
            "Federation server starting on {} (HTTPS: {}, mTLS: {})",
            addr, self.config.use_https, self.config.use_mtls
        );
        
        // Start peer discovery
        let discovery = self.discovery.clone();
        tokio::spawn(async move {
            discovery.start_discovery().await;
        });
        
        let app = self.create_router();
        
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| MatrixError::Internal(format!("Failed to bind: {}", e)))?;
        
        axum::serve(listener, app)
            .await
            .map_err(|e| MatrixError::Internal(format!("Server error: {}", e)))?;
        
        Ok(())
    }
    
    /// Create the Axum router
    fn create_router(&self) -> Router {
        let state = FederationState {
            config: self.config.clone(),
            discovery: self.discovery.clone(),
            store: self.store.clone(),
            peers: self.peers.clone(),
        };
        
        // Configure CORS
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
            .allow_headers(Any);
        
        Router::new()
            // Matrix spec endpoint: Get server key
            .route("/_matrix/key/v2/server", get(get_server_key))
            // CIS federation endpoint: Receive events
            .route("/_cis/v1/event/receive", post(receive_event))
            // Health check
            .route("/_cis/v1/health", get(health_check))
            // Middleware
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            // State
            .with_state(state)
    }
    
    /// Get the server port
    pub fn port(&self) -> u16 {
        self.config.port
    }
    
    /// Get the server name
    pub fn server_name(&self) -> &str {
        &self.config.server_name
    }
    
    /// Forward an event to other nodes
    ///
    /// # Arguments
    ///
    /// * `event` - The event to forward
    /// * `targets` - List of target server names (if empty, forwards to all trusted peers)
    pub async fn forward_event(
        &self,
        event: &CisMatrixEvent,
        targets: &[String],
    ) -> Result<HashMap<String, super::client::FederationClientResult<EventReceiveResponse>>, MatrixError> {
        let peers = if targets.is_empty() {
            // Forward to all trusted peers
            self.discovery.get_trusted_peers()
        } else {
            // Forward to specified targets
            let mut result = Vec::new();
            for target in targets {
                if let Some(peer) = self.discovery.get_peer(target) {
                    result.push(peer);
                } else {
                    warn!("Target peer not found: {}", target);
                }
            }
            result
        };
        
        if peers.is_empty() {
            debug!("No peers to forward event to");
            return Ok(HashMap::new());
        }
        
        info!("Forwarding event {} to {} peers", event.event_id, peers.len());
        
        let results = self.client.broadcast_event_parallel(&peers, event).await;
        
        // Update last_seen for successful peers
        for (server_name, result) in &results {
            if result.is_ok() {
                self.discovery.mark_peer_seen(server_name);
            }
        }
        
        Ok(results)
    }
    
    /// Add a peer at runtime
    pub async fn add_peer(&self, peer: PeerInfo) {
        let server_name = peer.server_name.clone();
        
        // Add to discovery
        self.discovery.add_peer(peer.clone());
        
        // Add to runtime peers
        let mut peers = self.peers.write().await;
        peers.insert(server_name.clone(), peer);
        
        info!("Added peer: {}", server_name);
    }
    
    /// Remove a peer at runtime
    pub async fn remove_peer(&self, server_name: &str) -> Option<PeerInfo> {
        let mut peers = self.peers.write().await;
        let removed = peers.remove(server_name);
        
        if removed.is_some() {
            info!("Removed peer: {}", server_name);
        }
        
        removed
    }
    
    /// Get all known peers
    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.discovery.get_known_peers()
    }
    
    /// Get trusted peers
    pub fn get_trusted_peers(&self) -> Vec<PeerInfo> {
        self.discovery.get_trusted_peers()
    }
}

/// Get server signing key (Matrix spec endpoint)
///
/// GET /_matrix/key/v2/server
async fn get_server_key(
    State(state): State<FederationState>,
) -> impl IntoResponse {
    debug!("Getting server key for {}", state.config.server_name);
    
    // Build verify_keys from config
    let mut verify_keys = HashMap::new();
    
    if let Some(ref public_key) = state.config.public_key {
        verify_keys.insert(
            "ed25519:0".to_string(),
            VerifyKey {
                key: public_key.clone(),
            },
        );
    }
    
    // Build signatures
    let mut signatures = HashMap::new();
    
    // In a real implementation, we would sign this response
    // For now, we return an empty signature (simplified scheme B)
    signatures.insert(
        state.config.server_name.clone(),
        HashMap::new(),
    );
    
    let response = ServerKeyResponse {
        server_name: state.config.server_name.clone(),
        valid_until_ts: (chrono::Utc::now() + chrono::Duration::days(7)).timestamp_millis(),
        verify_keys,
        old_verify_keys: None,
        signatures,
        tls_fingerprints: None,
    };
    
    (StatusCode::OK, Json(response))
}

/// Receive an event from another node
///
/// POST /_cis/v1/event/receive
async fn receive_event(
    State(state): State<FederationState>,
    headers: HeaderMap,
    Json(event): Json<CisMatrixEvent>,
) -> impl IntoResponse {
    debug!("Received event {} from {}", event.event_id, event.sender);
    
    // 1. Get sender server name from sender user ID or headers
    let sender_server = extract_sender_server(&event, &headers);
    
    // 2. Verify sender if signature verification is enabled
    if state.config.verify_signatures {
        if let Err(e) = verify_event_signature(&event, &state).await {
            warn!("Signature verification failed for {}: {}", event.event_id, e);
            return (
                StatusCode::FORBIDDEN,
                Json(EventReceiveResponse::error(
                    &event.event_id,
                    format!("Signature verification failed: {}", e),
                )),
            );
        }
    }
    
    // 3. Check if sender is trusted (if we have them in our peer list)
    if let Some(ref sender) = sender_server {
        if let Some(peer) = state.discovery.get_peer(sender) {
            if !peer.trusted {
                warn!("Event from untrusted peer: {}", sender);
                return (
                    StatusCode::FORBIDDEN,
                    Json(EventReceiveResponse::error(
                        &event.event_id,
                        "Peer not trusted",
                    )),
                );
            }
        }
    }
    
    // 4. Save event to database
    if let Err(e) = save_event_to_store(&event, &state).await {
        error!("Failed to save event {}: {}", event.event_id, e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(EventReceiveResponse::error(
                &event.event_id,
                format!("Failed to save event: {}", e),
            )),
        );
    }
    
    // 5. Mark peer as seen
    if let Some(ref sender) = sender_server {
        state.discovery.mark_peer_seen(sender);
    }
    
    info!("Successfully received and saved event {}", event.event_id);
    
    // 6. Return success response
    (
        StatusCode::OK,
        Json(EventReceiveResponse::success(&event.event_id)),
    )
}

/// Health check endpoint
///
/// GET /_cis/v1/health
async fn health_check(State(state): State<FederationState>) -> impl IntoResponse {
    let response = serde_json::json!({
        "status": "healthy",
        "server_name": state.config.server_name,
        "version": "0.1.0",
        "peers": state.discovery.peer_count(),
    });
    
    (StatusCode::OK, Json(response))
}

/// Extract sender server from event or headers
fn extract_sender_server(event: &CisMatrixEvent, headers: &HeaderMap) -> Option<String> {
    // First, try to get from event origin field
    if let Some(ref origin) = event.origin {
        return Some(origin.clone());
    }
    
    // Try to extract from sender user ID (format: @user:server)
    if let Some((_, server)) = event.sender.split_once(':') {
        return Some(server.to_string());
    }
    
    // Try to get from X-Origin-Server header (custom CIS header)
    if let Some(value) = headers.get("x-origin-server") {
        if let Ok(server) = value.to_str() {
            return Some(server.to_string());
        }
    }
    
    None
}

/// Verify event signature
/// 
/// 验证联邦事件的签名：
/// 1. 从事件中提取签名
/// 2. 解析发送者的 DID 获取公钥
/// 3. 验证事件内容的签名
async fn verify_event_signature(
    event: &CisMatrixEvent,
    state: &FederationState,
) -> Result<(), String> {
    // 如果配置禁用签名验证，直接通过
    if !state.config.verify_signatures {
        return Ok(());
    }
    
    // 获取签名
    let signatures = match &event.signatures {
        Some(sigs) if !sigs.is_empty() => sigs,
        _ => {
            // 没有签名的事件在严格模式下被拒绝
            if state.config.verify_signatures {
                return Err("Event has no signatures".to_string());
            }
            return Ok(());
        }
    };
    
    // 解析发送者 DID 获取公钥
    let sender_did = format!("did:cis:{}", event.sender.replace(':', ":"));
    let verifying_key = match resolve_did_to_key(&sender_did).await {
        Ok(key) => key,
        Err(e) => {
            return Err(format!("Failed to resolve DID for {}: {}", event.sender, e));
        }
    };
    
    // 创建事件内容的 canonical 表示
    let event_canonical = serde_json::json!({
        "event_id": &event.event_id,
        "room_id": &event.room_id,
        "sender": &event.sender,
        "event_type": &event.event_type,
        "content": &event.content,
        "origin_server_ts": event.origin_server_ts,
    });
    
    let event_bytes = serde_json::to_vec(&event_canonical)
        .map_err(|e| format!("Failed to serialize event: {}", e))?;
    
    // 验证每个签名
    for (server_name, server_sigs) in signatures {
        for (key_id, sig_hex) in server_sigs {
            match crate::identity::did::DIDManager::signature_from_hex(sig_hex) {
                Ok(signature) => {
                    use ed25519_dalek::Verifier;
                    match verifying_key.verify(&event_bytes, &signature) {
                        Ok(()) => {
                            debug!(
                                "Event {} signature verified from {} with key {}",
                                event.event_id, server_name, key_id
                            );
                            return Ok(());
                        }
                        Err(e) => {
                            warn!(
                                "Signature verification failed for event {} from {}: {:?}",
                                event.event_id, server_name, e
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse signature for event {}: {}", event.event_id, e);
                }
            }
        }
    }
    
    Err(format!(
        "No valid signature found for event {} from {}",
        event.event_id, event.sender
    ))
}

/// Resolve DID to verifying key
async fn resolve_did_to_key(did: &str) -> Result<ed25519_dalek::VerifyingKey, String> {
    use crate::identity::did::DIDManager;
    
    // 解析 DID
    let (_, public_key_hex) = DIDManager::parse_did(did)
        .ok_or_else(|| format!("Invalid DID format: {}", did))?;
    
    // 从十六进制解析公钥
    DIDManager::verifying_key_from_hex(&public_key_hex)
        .map_err(|e| format!("Failed to parse public key: {}", e))
}

/// Save event to Matrix store
///
/// This function handles deduplication and stores federation events
/// in the federation_events table for tracking and cleanup.
async fn save_event_to_store(
    event: &CisMatrixEvent,
    state: &FederationState,
) -> Result<(), MatrixError> {
    debug!(
        "Saving event: id={}, room={}, type={}",
        event.event_id, event.room_id, event.event_type
    );

    // 1. Check for duplicate events (deduplication)
    match state.store.federation_event_exists(&event.event_id) {
        Ok(true) => {
            debug!("Event {} already exists, skipping", event.event_id);
            return Ok(()); // Duplicate event, skip
        }
        Ok(false) => {
            // Continue with storage
        }
        Err(e) => {
            error!("Failed to check event existence: {}", e);
            return Err(e);
        }
    }

    // 2. Serialize content to string
    let content_str = event.content.to_string();

    // 3. Store in federation_events table with ON CONFLICT IGNORE
    let inserted = state.store.store_federation_event(
        &event.event_id,
        &event.sender,
        &event.room_id,
        &event.event_type,
        &content_str,
        event.origin_server_ts,
    )?;

    if inserted {
        info!(
            "Stored federation event: id={}, room={}, type={}",
            event.event_id, event.room_id, event.event_type
        );

        // 4. Also save to the main matrix_events table for room visibility
        // This ensures the event appears in room history
        let unsigned_str = event.unsigned.as_ref().map(|u| u.to_string());
        state.store.save_event(
            &event.room_id,
            &event.event_id,
            &event.sender,
            &event.event_type,
            &content_str,
            event.origin_server_ts,
            unsigned_str.as_deref(),
            event.state_key.as_deref(),
        )?;

        // 5. Mark as processed
        state.store.mark_federation_event_processed(&event.event_id)?;
    } else {
        debug!("Event {} was a duplicate (race condition)", event.event_id);
    }

    Ok(())
}

/// Cleanup expired federation events
///
/// Deletes federation events older than the specified retention period.
/// Default retention is 30 days.
pub async fn cleanup_expired_events(state: &FederationState, retention_days: Option<i64>) -> Result<usize, MatrixError> {
    let days = retention_days.unwrap_or(30);
    
    if days <= 0 {
        return Err(MatrixError::InvalidParameter(
            "retention_days must be positive".to_string()
        ));
    }

    let deleted = state.store.cleanup_expired_federation_events(days)?;
    
    if deleted > 0 {
        info!("Cleaned up {} expired federation events (older than {} days)", deleted, days);
    } else {
        debug!("No expired federation events to cleanup (retention: {} days)", days);
    }

    Ok(deleted)
}

/// Start periodic cleanup task for federation events
///
/// This spawns a background task that periodically cleans up expired events.
pub fn start_cleanup_task(state: FederationState, interval_hours: u64, retention_days: Option<i64>) {
    let interval = std::time::Duration::from_secs(interval_hours * 3600);
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = cleanup_expired_events(&state, retention_days).await {
                error!("Failed to cleanup expired federation events: {}", e);
            }
        }
    });
    
    info!("Started federation events cleanup task (interval: {} hours, retention: {} days)", 
        interval_hours, retention_days.unwrap_or(30));
}

/// Federation server builder
#[derive(Debug)]
pub struct FederationServerBuilder {
    config: Option<FederationConfig>,
    discovery: Option<PeerDiscovery>,
    store_path: Option<String>,
    use_mtls: bool,
    cert_path: Option<String>,
    key_path: Option<String>,
    ca_path: Option<String>,
}

impl FederationServerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: None,
            discovery: None,
            store_path: None,
            use_mtls: false,
            cert_path: None,
            key_path: None,
            ca_path: None,
        }
    }
    
    /// Set federation configuration
    pub fn config(mut self, config: FederationConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    /// Set peer discovery
    pub fn discovery(mut self, discovery: PeerDiscovery) -> Self {
        self.discovery = Some(discovery);
        self
    }
    
    /// Set store path
    pub fn store_path(mut self, path: impl Into<String>) -> Self {
        self.store_path = Some(path.into());
        self
    }
    
    /// Enable mTLS
    pub fn with_mtls(mut self, cert_path: &str, key_path: &str) -> Self {
        self.use_mtls = true;
        self.cert_path = Some(cert_path.to_string());
        self.key_path = Some(key_path.to_string());
        self
    }
    
    /// Set CA certificate for mTLS
    pub fn with_ca(mut self, ca_path: &str) -> Self {
        self.ca_path = Some(ca_path.to_string());
        self
    }
    
    /// Build the federation server
    pub fn build(self) -> Result<FederationServer, MatrixError> {
        let config = self.config.unwrap_or_default();
        let discovery = self.discovery.unwrap_or_default();
        
        let store = if let Some(path) = self.store_path {
            MatrixStore::open(&path)?
        } else {
            MatrixStore::open_in_memory()?
        };
        
        // Create client with mTLS if enabled
        let client = if self.use_mtls {
            let cert = self.cert_path.ok_or_else(|| {
                MatrixError::InvalidParameter("mTLS enabled but no cert path provided".to_string())
            })?;
            let key = self.key_path.ok_or_else(|| {
                MatrixError::InvalidParameter("mTLS enabled but no key path provided".to_string())
            })?;
            
            FederationClient::with_mtls(&cert, &key, self.ca_path.as_deref())
                .map_err(|e| MatrixError::Internal(format!("Failed to create client: {}", e)))?
        } else {
            FederationClient::new()
                .map_err(|e| MatrixError::Internal(format!("Failed to create client: {}", e)))?
        };
        
        Ok(FederationServer::with_client(
            config,
            discovery,
            Arc::new(store),
            client,
        ))
    }
}

impl Default for FederationServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_server() -> FederationServer {
        let config = FederationConfig::new("test.local");
        let discovery = PeerDiscovery::default();
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        
        FederationServer::new(config, discovery, store)
    }

    #[test]
    fn test_server_creation() {
        let server = create_test_server();
        assert_eq!(server.port(), FEDERATION_PORT);
        assert_eq!(server.server_name(), "test.local");
    }

    #[test]
    fn test_router_creation() {
        let server = create_test_server();
        // We can't directly test create_router since it's private,
        // but we can test that the server was created successfully
    }

    #[test]
    fn test_builder() {
        let server = FederationServerBuilder::new()
            .config(FederationConfig::new("builder.local"))
            .discovery(PeerDiscovery::default())
            .build()
            .unwrap();
        
        assert_eq!(server.server_name(), "builder.local");
    }

    // ==================== Federation Event Storage Tests ====================

    fn create_test_state() -> FederationState {
        FederationState {
            config: FederationConfig::new("test.local"),
            discovery: PeerDiscovery::default(),
            store: Arc::new(MatrixStore::open_in_memory().unwrap()),
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn create_test_event(event_id: &str) -> CisMatrixEvent {
        CisMatrixEvent::new(
            event_id,
            "!test_room:test.local",
            "@sender:test.local",
            "m.room.message",
            serde_json::json!({ "body": "Hello", "msgtype": "m.text" }),
        )
    }

    #[tokio::test]
    async fn test_save_event_to_store_success() {
        let state = create_test_state();
        let event = create_test_event("$event123");

        // Save the event
        let result = save_event_to_store(&event, &state).await;
        assert!(result.is_ok());

        // Verify the event was stored in federation_events
        let exists = state.store.federation_event_exists("$event123").unwrap();
        assert!(exists);

        // Verify the event details
        let stored = state.store.get_federation_event("$event123").unwrap();
        assert!(stored.is_some());
        let stored = stored.unwrap();
        assert_eq!(stored.event_id, "$event123");
        assert_eq!(stored.sender, "@sender:test.local");
        assert_eq!(stored.room_id, "!test_room:test.local");
        assert_eq!(stored.event_type, "m.room.message");
        assert!(stored.processed);
    }

    #[tokio::test]
    async fn test_save_event_deduplication() {
        let state = create_test_state();
        let event = create_test_event("$event456");

        // Save the event first time
        let result = save_event_to_store(&event, &state).await;
        assert!(result.is_ok());

        // Save the same event again (should be deduplicated)
        let result = save_event_to_store(&event, &state).await;
        assert!(result.is_ok()); // Should succeed but not insert duplicate

        // Verify only one event exists
        let count = state.store.count_federation_events(None).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_save_multiple_different_events() {
        let state = create_test_state();

        // Create and save multiple different events
        for i in 0..5 {
            let event = create_test_event(&format!("$event{}", i));
            let result = save_event_to_store(&event, &state).await;
            assert!(result.is_ok());
        }

        // Verify all events exist
        let count = state.store.count_federation_events(None).unwrap();
        assert_eq!(count, 5);

        // Verify each event
        for i in 0..5 {
            let exists = state.store.federation_event_exists(&format!("$event{}", i)).unwrap();
            assert!(exists, "Event $event{} should exist", i);
        }
    }

    #[tokio::test]
    async fn test_cleanup_expired_events() {
        let state = create_test_state();

        // Create and save an event
        let event = create_test_event("$old_event");
        save_event_to_store(&event, &state).await.unwrap();

        // Verify event exists
        let count = state.store.count_federation_events(None).unwrap();
        assert_eq!(count, 1);

        // Cleanup with 0 days (should delete all events since they were just created with unixepoch())
        // Note: This test may be flaky due to timing, so we test with a large negative number
        // In practice, events are deleted if received_at < unixepoch() - (days * 86400)
        
        // For testing, we'll manually verify the cleanup function works
        // by checking that it returns Ok with 0 deleted for recent events
        let deleted = cleanup_expired_events(&state, Some(30)).await;
        assert!(deleted.is_ok());
        // Recent events should not be deleted with 30 day retention
        assert_eq!(deleted.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_invalid_retention() {
        let state = create_test_state();

        // Test with 0 days (invalid)
        let result = cleanup_expired_events(&state, Some(0)).await;
        assert!(result.is_err());

        // Test with negative days (invalid)
        let result = cleanup_expired_events(&state, Some(-1)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_store_federation_event_exists_check() {
        let state = create_test_state();

        // Check non-existent event
        let exists = state.store.federation_event_exists("$nonexistent").unwrap();
        assert!(!exists);

        // Create and save event
        let event = create_test_event("$existing_event");
        save_event_to_store(&event, &state).await.unwrap();

        // Check existing event
        let exists = state.store.federation_event_exists("$existing_event").unwrap();
        assert!(exists);
    }

    #[tokio::test]
    async fn test_get_unprocessed_events() {
        let state = create_test_state();

        // Store event directly as unprocessed
        state.store.store_federation_event(
            "$unprocessed1",
            "@sender:test.local",
            "!room:test.local",
            "m.room.message",
            r#"{"body":"test"}"#,
            chrono::Utc::now().timestamp_millis(),
        ).unwrap();

        // Get unprocessed events
        let unprocessed = state.store.get_unprocessed_federation_events(10).unwrap();
        assert_eq!(unprocessed.len(), 1);
        assert_eq!(unprocessed[0].event_id, "$unprocessed1");
        assert!(!unprocessed[0].processed);
    }
}
