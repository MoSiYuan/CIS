//! # WebSocket Federation Server
//!
//! WebSocket server for CIS federation (BMI - Between Machine Interface).
//!
//! ## Features
//!
//! - Accept WebSocket connections from peers
//! - Noise protocol handshake (placeholder)
//! - DID authentication
//! - Event forwarding

use std::net::SocketAddr;
use std::sync::Arc;

use futures::StreamExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::matrix::store::MatrixStore;
use crate::identity::DIDManager;

use super::protocol::{
    AckMessage, AuthMessage, ErrorCode, ErrorMessage, HandshakeMessage, WsMessage,
    PROTOCOL_VERSION, WS_PATH,
};
use super::tunnel::{TunnelManager, TunnelState};

/// WebSocket federation server
pub struct WebSocketServer {
    /// Server configuration
    config: WsServerConfig,
    /// Tunnel manager for connection management
    tunnel_manager: Arc<TunnelManager>,
    /// Matrix store for event persistence
    store: Arc<MatrixStore>,
    /// This node's DID
    node_did: String,
    /// DID manager for authentication
    did_manager: Arc<DIDManager>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl std::fmt::Debug for WebSocketServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketServer")
            .field("config", &self.config)
            .field("tunnel_manager", &self.tunnel_manager)
            .field("node_did", &self.node_did)
            .finish_non_exhaustive()
    }
}

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct WsServerConfig {
    /// Server name / DID
    pub server_name: String,
    /// Port to listen on
    pub port: u16,
    /// Bind address
    pub bind_address: String,
    /// Require authentication
    pub require_auth: bool,
    /// Maximum connections
    pub max_connections: usize,
}

impl Default for WsServerConfig {
    fn default() -> Self {
        Self {
            server_name: "cis.local".to_string(),
            port: 6768,
            bind_address: "0.0.0.0".to_string(),
            require_auth: true,
            max_connections: 100,
        }
    }
}

impl WsServerConfig {
    /// Create new config with server name
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            ..Default::default()
        }
    }

    /// Set port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set bind address
    pub fn with_bind_address(mut self, addr: impl Into<String>) -> Self {
        self.bind_address = addr.into();
        self
    }

    /// Set authentication requirement
    pub fn with_auth(mut self, require: bool) -> Self {
        self.require_auth = require;
        self
    }
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(
        config: WsServerConfig,
        tunnel_manager: Arc<TunnelManager>,
        store: Arc<MatrixStore>,
        node_did: impl Into<String>,
        did_manager: Arc<DIDManager>,
    ) -> Self {
        Self {
            config,
            tunnel_manager,
            store,
            node_did: node_did.into(),
            did_manager,
            shutdown_tx: None,
        }
    }

    /// Run the WebSocket server
    pub async fn run(&mut self) -> Result<(), WsServerError> {
        let addr = SocketAddr::from((
            self.config
                .bind_address
                .parse::<std::net::IpAddr>()
                .map_err(|e| WsServerError::ConfigError(format!("Invalid bind address: {}", e)))?,
            self.config.port,
        ));

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| WsServerError::BindError(e.to_string()))?;

        info!(
            "WebSocket federation server starting on ws://{}{}",
            addr, WS_PATH
        );

        // Start tunnel manager maintenance
        let tm = self.tunnel_manager.clone();
        tokio::spawn(async move {
            tm.start_maintenance().await;
        });

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            self.handle_connection(stream, peer_addr).await;
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("WebSocket server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle incoming WebSocket connection
    async fn handle_connection(&self, stream: TcpStream, peer_addr: SocketAddr) {
        let tunnel_manager = self.tunnel_manager.clone();
        let store = self.store.clone();
        let config = self.config.clone();
        let node_did = self.node_did.clone();
        let did_manager = self.did_manager.clone();

        tokio::spawn(async move {
            debug!("New WebSocket connection from {}", peer_addr);

            // Accept WebSocket upgrade
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    error!("WebSocket handshake failed for {}: {}", peer_addr, e);
                    return;
                }
            };

            // Handle the connection
            let handler = ConnectionHandler::new(
                peer_addr,
                ws_stream,
                tunnel_manager,
                store,
                config,
                node_did,
                did_manager,
            );

            if let Err(e) = handler.run().await {
                error!("Connection handler error for {}: {:?}", peer_addr, e);
            }
        });
    }

    /// Shutdown the server
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
        self.tunnel_manager.shutdown().await;
    }

    /// Get server port
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Get server name
    pub fn server_name(&self) -> &str {
        &self.config.server_name
    }
}

/// Connection handler for individual WebSocket connections
struct ConnectionHandler {
    /// Remote address
    peer_addr: SocketAddr,
    /// WebSocket stream (wrapped in Option for ownership transfer)
    ws_stream: Option<WebSocketStream<TcpStream>>,
    /// Tunnel manager
    tunnel_manager: Arc<TunnelManager>,
    /// Matrix store
    store: Arc<MatrixStore>,
    /// Server config
    config: WsServerConfig,
    /// This node's DID
    node_did: String,
    /// DID manager for authentication
    did_manager: Arc<DIDManager>,
    /// Remote node ID (set after auth)
    remote_node_id: Option<String>,
    /// Authenticated flag
    authenticated: bool,
    /// Message channel sender
    msg_tx: mpsc::UnboundedSender<WsMessage>,
    /// Message channel receiver
    msg_rx: mpsc::UnboundedReceiver<WsMessage>,
}

impl ConnectionHandler {
    /// Create a new connection handler
    fn new(
        peer_addr: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        tunnel_manager: Arc<TunnelManager>,
        store: Arc<MatrixStore>,
        config: WsServerConfig,
        node_did: String,
        did_manager: Arc<DIDManager>,
    ) -> Self {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        Self {
            peer_addr,
            ws_stream: Some(ws_stream),
            tunnel_manager,
            store,
            config,
            node_did,
            did_manager,
            remote_node_id: None,
            authenticated: false,
            msg_tx,
            msg_rx,
        }
    }

    /// Run the connection handler
    async fn run(mut self) -> Result<(), WsServerError> {
        // Take ownership of the stream
        let ws_stream = self.ws_stream.take()
            .ok_or_else(|| WsServerError::Internal("WebSocket stream already taken".to_string()))?;
        
        // Split the WebSocket stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Start handshake
        self.send_handshake().await?;

        // Start handshake
        self.send_handshake().await?;

        // Message processing loop
        loop {
            tokio::select! {
                // Receive from WebSocket
                Some(msg_result) = ws_receiver.next() => {
                    match msg_result {
                        Ok(msg) => {
                            if let Err(e) = self.handle_message(msg).await {
                                warn!("Error handling message from {}: {:?}", self.peer_addr, e);
                            }
                        }
                        Err(e) => {
                            error!("WebSocket error from {}: {}", self.peer_addr, e);
                            break;
                        }
                    }
                }

                // Send to WebSocket
                Some(msg) = self.msg_rx.recv() => {
                    let ws_msg = Message::Text(
                        serde_json::to_string(&msg)
                            .map_err(|e| WsServerError::SerializationError(e.to_string()))?
                    );
                    use futures::SinkExt;
                    if let Err(e) = ws_sender.send(ws_msg).await {
                        error!("Failed to send to {}: {}", self.peer_addr, e);
                        break;
                    }
                }

                else => break,
            }
        }

        // Cleanup
        if let Some(node_id) = &self.remote_node_id {
            self.tunnel_manager.remove_tunnel(node_id).await;
        }

        Ok(())
    }
    


    /// Send handshake message
    async fn send_handshake(&mut self) -> Result<(), WsServerError> {
        debug!("Starting handshake with {}", self.peer_addr);

        // Send handshake message
        let handshake = HandshakeMessage::new(
            PROTOCOL_VERSION,
            vec![], // Noise payload would go here
            &self.node_did,
        );

        let msg = WsMessage::Handshake(handshake);
        self.msg_tx
            .send(msg)
            .map_err(|_| WsServerError::SendError)?;

        Ok(())
    }

    /// Handle incoming WebSocket message
    async fn handle_message(&mut self, msg: Message) -> Result<(), WsServerError> {
        match msg {
            Message::Text(text) => {
                let ws_msg: WsMessage = serde_json::from_str(&text)
                    .map_err(|e| WsServerError::SerializationError(e.to_string()))?;
                self.handle_ws_message(ws_msg).await
            }
            Message::Binary(bin) => {
                // Try to parse as JSON first, could be Protobuf in future
                let ws_msg: WsMessage = serde_json::from_slice(&bin)
                    .map_err(|e| WsServerError::SerializationError(e.to_string()))?;
                self.handle_ws_message(ws_msg).await
            }
            Message::Ping(_data) => {
                // Pong is handled automatically by tokio-tungstenite
                debug!("Received ping from {}", self.peer_addr);
                Ok(())
            }
            Message::Pong(_) => {
                debug!("Received pong from {}", self.peer_addr);
                Ok(())
            }
            Message::Close(_) => {
                info!("Connection closed by {}", self.peer_addr);
                Err(WsServerError::ConnectionClosed)
            }
            Message::Frame(_) => Ok(()), // Shouldn't happen
        }
    }

    /// Handle parsed WebSocket message
    async fn handle_ws_message(&mut self, msg: WsMessage) -> Result<(), WsServerError> {
        match msg {
            WsMessage::Handshake(handshake) => {
                debug!("Received handshake from {}", handshake.server_name);

                // Validate protocol version
                if handshake.version != PROTOCOL_VERSION {
                    return self
                        .send_error(
                            ErrorCode::VersionMismatch,
                            format!("Protocol version mismatch: expected {}", PROTOCOL_VERSION),
                        )
                        .await;
                }

                // Store remote node ID
                self.remote_node_id = Some(handshake.server_name.clone());

                // Register tunnel
                let tunnel = self
                    .tunnel_manager
                    .register_tunnel(&handshake.server_name, self.msg_tx.clone())
                    .await;
                tunnel.set_state(TunnelState::Handshaking).await;

                Ok(())
            }

            WsMessage::Auth(auth) => {
                debug!("Received auth from {}", auth.did);

                if self.config.require_auth {
                    if let Err(e) = self.verify_auth(&auth).await {
                        return self
                            .send_error(ErrorCode::AuthFailed, e.to_string())
                            .await;
                    }
                }

                self.authenticated = true;

                // Update tunnel state
                if let Some(ref node_id) = self.remote_node_id {
                    if let Some(tunnel) = self.tunnel_manager.get_tunnel(node_id).await {
                        tunnel.set_state(TunnelState::Ready).await;
                    }
                }

                // Send success ack
                let ack = WsMessage::Ack(AckMessage::success("auth"));
                self.msg_tx.send(ack).map_err(|_| WsServerError::SendError)
            }

            WsMessage::Event(event) => {
                if self.config.require_auth && !self.authenticated {
                    return self
                        .send_error(ErrorCode::Unauthorized, "Not authenticated")
                        .await;
                }

                // Handle event
                if let Some(ref node_id) = self.remote_node_id {
                    self.tunnel_manager.handle_event(node_id, event).await;
                }

                Ok(())
            }

            WsMessage::Ping(ping) => {
                // Send pong response
                let pong = WsMessage::Pong(super::protocol::PongMessage::new(ping.ping_id));
                self.msg_tx.send(pong).map_err(|_| WsServerError::SendError)
            }

            WsMessage::Pong(pong) => {
                if let Some(ref node_id) = self.remote_node_id {
                    self.tunnel_manager.handle_pong(node_id, pong.ping_id).await;
                }
                Ok(())
            }

            WsMessage::Ack(ack) => {
                debug!("Received ack: {:?}", ack);
                Ok(())
            }

            WsMessage::Error(err) => {
                warn!("Received error from peer: {:?}", err);
                Ok(())
            }

            WsMessage::SyncRequest(request) => {
                debug!("Received sync request for room: {:?}, since: {:?}", 
                    request.room_id, request.since_event_id);
                
                // 检查认证
                if self.config.require_auth && !self.authenticated {
                    return self
                        .send_error(ErrorCode::Unauthorized, "Not authenticated")
                        .await;
                }

                // 处理同步请求
                match self.handle_sync_request(&request).await {
                    Ok(response) => {
                        self.msg_tx.send(WsMessage::SyncResponse(response))
                            .map_err(|_| WsServerError::SendError)
                    }
                    Err(e) => {
                        warn!("Failed to handle sync request: {}", e);
                        self.send_error(ErrorCode::InternalError, format!("Sync failed: {}", e))
                            .await
                    }
                }
            }

            WsMessage::SyncResponse(response) => {
                debug!("Received sync response with {} events for room: {}", 
                    response.events.len(), response.room_id);
                
                // 处理同步响应，保存接收到的事件
                if let Some(ref node_id) = self.remote_node_id {
                    if let Err(e) = self.handle_sync_response(node_id, &response).await {
                        warn!("Failed to handle sync response from {}: {}", node_id, e);
                    }
                }
                Ok(())
            }
        }
    }

    /// Verify DID authentication
    /// 
    /// Validates:
    /// 1. DID format is valid
    /// 2. Public key matches DID
    /// 3. Signature is valid
    /// 4. Timestamp is within acceptable window (prevent replay)
    async fn verify_auth(&self, auth: &AuthMessage) -> Result<(), WsServerError> {
        // 1. Validate DID format
        if !DIDManager::is_valid_did(&auth.did) {
            return Err(WsServerError::AuthFailed(
                format!("Invalid DID format: {}", auth.did)
            ));
        }
        
        // 2. Parse DID to extract node_id and pubkey_short
        let (node_id, pubkey_short) = DIDManager::parse_did(&auth.did)
            .ok_or_else(|| WsServerError::AuthFailed("Failed to parse DID".to_string()))?;
        
        // 3. Verify public key matches DID
        let pub_key_hex = hex::encode(&auth.public_key);
        if !pub_key_hex.starts_with(&pubkey_short) {
            return Err(WsServerError::AuthFailed(
                format!("Public key mismatch: expected to start with {}", pubkey_short)
            ));
        }
        
        // 4. Parse verifying key
        let verifying_key = DIDManager::verifying_key_from_hex(&pub_key_hex)
            .map_err(|e| WsServerError::AuthFailed(
                format!("Invalid public key: {}", e)
            ))?;
        
        // 5. Verify timestamp (prevent replay attacks)
        // Accept messages within 5 minutes of current time
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| WsServerError::AuthFailed("System time error".to_string()))?
            .as_secs();
        
        let time_diff = if auth.timestamp > current_time {
            auth.timestamp - current_time
        } else {
            current_time - auth.timestamp
        };
        
        if time_diff > 300 { // 5 minutes
            return Err(WsServerError::AuthFailed(
                format!("Timestamp too old: {} seconds difference", time_diff)
            ));
        }
        
        // 6. Verify signature
        // The challenge response should be a signature of: did + timestamp
        let challenge_data = format!("{}:{}", auth.did, auth.timestamp);
        
        let signature = ed25519_dalek::Signature::from_slice(&auth.challenge_response)
            .map_err(|e| WsServerError::AuthFailed(
                format!("Invalid signature format: {}", e)
            ))?;
        
        if !DIDManager::verify(&verifying_key, challenge_data.as_bytes(), &signature) {
            return Err(WsServerError::AuthFailed(
                "Signature verification failed".to_string()
            ));
        }
        
        // 7. Update remote node ID from DID
        tracing::info!("DID authentication successful for {} (node_id: {})", auth.did, node_id);
        
        Ok(())
    }

    /// Send error message
    async fn send_error(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Result<(), WsServerError> {
        let error = WsMessage::Error(ErrorMessage::new(code, message));
        self.msg_tx
            .send(error)
            .map_err(|_| WsServerError::SendError)
    }

    /// Handle sync request - query events from store and return response
    async fn handle_sync_request(
        &self,
        request: &super::protocol::SyncRequest,
    ) -> Result<super::protocol::SyncResponse, String> {
        let room_id = &request.room_id;
        let limit = if request.limit == 0 { 100 } else { request.limit.min(1000) };
        
        // Get events from store
        let messages = if let Some(ref since_event_id) = request.since_event_id {
            // Get events since a specific event ID
            self.store.get_events_since_event_id(room_id, since_event_id, limit as i64)
                .map_err(|e| format!("Failed to get events: {}", e))?
        } else {
            // Get all events for the room (from beginning)
            self.store.get_room_messages(room_id, 0, limit)
                .map_err(|e| format!("Failed to get messages: {}", e))?
        };

        // Apply filter if provided
        let filter = request.filter.as_ref();
        let mut events: Vec<super::protocol::EventMessage> = Vec::new();
        let mut last_event_id: Option<String> = None;

        for msg in messages {
            // Apply event type filter
            if let Some(ref f) = filter {
                if !f.matches_event_type(&msg.event_type) {
                    continue;
                }
                if !f.matches_sender(&msg.sender) {
                    continue;
                }
            }

            let event_data = serde_json::json!({
                "event_id": msg.event_id,
                "room_id": msg.room_id,
                "sender": msg.sender,
                "type": msg.event_type,
                "content": serde_json::from_str::<serde_json::Value>(&msg.content)
                    .unwrap_or(serde_json::json!({})),
                "origin_server_ts": msg.origin_server_ts,
                "unsigned": msg.unsigned.as_ref()
                    .and_then(|u| serde_json::from_str::<serde_json::Value>(u).ok()),
                "state_key": msg.state_key,
            });

            let event_msg = super::protocol::EventMessage::new(
                &msg.event_id,
                event_data.to_string().into_bytes(),
                &msg.event_type,
                &msg.sender,
            )
            .with_room_id(room_id);

            last_event_id = Some(msg.event_id.clone());
            events.push(event_msg);
        }

        // Determine next_batch token
        let has_more = events.len() == limit;
        let next_batch = if has_more {
            last_event_id.clone()
        } else {
            None
        };

        // Build response
        let mut response = super::protocol::SyncResponse::new(room_id, events)
            .with_has_more(has_more);
        
        if let Some(batch) = next_batch {
            response = response.with_next_batch(batch);
        }

        info!("Sync request for room {}: returned {} events, has_more: {}", 
            room_id, response.events.len(), response.has_more);

        Ok(response)
    }

    /// Handle sync response - save received events to store
    async fn handle_sync_response(
        &self,
        node_id: &str,
        response: &super::protocol::SyncResponse,
    ) -> Result<(), String> {
        let room_id = &response.room_id;
        let mut saved_count = 0;
        let mut failed_count = 0;

        for event_msg in &response.events {
            // Parse event data
            let event_data: serde_json::Value = match serde_json::from_slice(&event_msg.event_data) {
                Ok(data) => data,
                Err(e) => {
                    warn!("Failed to parse event data from {}: {}", node_id, e);
                    failed_count += 1;
                    continue;
                }
            };

            // Extract event fields
            let event_id = event_data.get("event_id")
                .and_then(|v| v.as_str())
                .unwrap_or(&event_msg.message_id);
            let sender = event_data.get("sender")
                .and_then(|v| v.as_str())
                .unwrap_or(&event_msg.sender);
            let event_type = event_data.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or(&event_msg.event_type);
            let origin_server_ts = event_data.get("origin_server_ts")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

            // Check if event already exists
            match self.store.event_exists(event_id) {
                Ok(true) => {
                    debug!("Event {} already exists, skipping", event_id);
                    continue;
                }
                Ok(false) => {}
                Err(e) => {
                    warn!("Failed to check event existence: {}", e);
                }
            }

            // Save event to store
            let content = event_data.get("content")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "{}".to_string());
            let unsigned = event_data.get("unsigned").map(|v| v.to_string());
            let state_key = event_data.get("state_key").and_then(|v| v.as_str());

            if let Err(e) = self.store.save_event(
                room_id,
                event_id,
                sender,
                event_type,
                &content,
                origin_server_ts,
                unsigned.as_deref(),
                state_key,
            ) {
                warn!("Failed to save event {}: {}", event_id, e);
                failed_count += 1;
                continue;
            }

            // Parse as CisMatrixEvent and forward to event channel
            if let Ok(cis_event) = serde_json::from_value::<crate::matrix::federation::types::CisMatrixEvent>(event_data.clone()) {
                let _ = self.tunnel_manager.send_event_to_channel(node_id.to_string(), cis_event).await;
            }

            saved_count += 1;
        }

        info!("Sync response from {}: saved {} events, {} failed", 
            node_id, saved_count, failed_count);

        Ok(())
    }
}

/// WebSocket server errors
#[derive(Debug, thiserror::Error)]
pub enum WsServerError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Bind error
    #[error("Failed to bind: {0}")]
    BindError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Send error
    #[error("Failed to send message")]
    SendError,

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// WebSocket server builder
#[derive(Debug, Default)]
pub struct WebSocketServerBuilder {
    config: Option<WsServerConfig>,
    store_path: Option<String>,
    node_did: Option<String>,
    did_manager: Option<Arc<DIDManager>>,
}

impl WebSocketServerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set server configuration
    pub fn config(mut self, config: WsServerConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set store path
    pub fn store_path(mut self, path: impl Into<String>) -> Self {
        self.store_path = Some(path.into());
        self
    }

    /// Set node DID
    pub fn node_did(mut self, did: impl Into<String>) -> Self {
        self.node_did = Some(did.into());
        self
    }

    /// Set DID manager
    pub fn did_manager(mut self, did_manager: Arc<DIDManager>) -> Self {
        self.did_manager = Some(did_manager);
        self
    }

    /// Build the server
    pub fn build(self) -> Result<WebSocketServer, WsServerError> {
        let config = self.config.unwrap_or_default();
        let node_did = self.node_did.unwrap_or_else(|| config.server_name.clone());

        let store = if let Some(path) = self.store_path {
            MatrixStore::open(&path).map_err(|e| WsServerError::Internal(e.to_string()))?
        } else {
            MatrixStore::open_in_memory().map_err(|e| WsServerError::Internal(e.to_string()))?
        };

        // Create event channel
        let (event_tx, _event_rx) = mpsc::channel(1000);

        let tunnel_manager = Arc::new(TunnelManager::with_event_channel(event_tx));
        
        // DID manager is required for authentication
        let did_manager = self.did_manager
            .ok_or_else(|| WsServerError::ConfigError("DID manager is required".to_string()))?;

        Ok(WebSocketServer::new(config, tunnel_manager, Arc::new(store), node_did, did_manager))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_server_config() {
        let config = WsServerConfig::new("test.local").with_port(9999);
        assert_eq!(config.server_name, "test.local");
        assert_eq!(config.port, 9999);
    }

    #[test]
    fn test_server_builder() {
        let did_manager = Arc::new(DIDManager::generate("builder").unwrap());
        let server = WebSocketServerBuilder::new()
            .config(WsServerConfig::new("builder.local"))
            .node_did("did:cis:builder")
            .did_manager(did_manager)
            .build();

        assert!(server.is_ok());
        let server = server.unwrap();
        assert_eq!(server.server_name(), "builder.local");
    }
    
    #[test]
    fn test_did_verification() {
        use super::super::protocol::AuthMessage;
        
        // Create a DID manager for the "client"
        let client_did_manager = DIDManager::generate("client-node").unwrap();
        let client_did = client_did_manager.did().to_string();
        let client_pubkey = client_did_manager.public_key_hex();
        
        // Create challenge data
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let challenge_data = format!("{}:{}", client_did, timestamp);
        
        // Sign the challenge
        let signature = client_did_manager.sign(challenge_data.as_bytes());
        
        // Create auth message
        let auth = AuthMessage::new(
            &client_did,
            signature.to_bytes().to_vec(),
            hex::decode(&client_pubkey).unwrap(),
        );
        
        // Verify DID format
        assert!(DIDManager::is_valid_did(&auth.did));
        
        // Parse DID
        let (node_id, pubkey_short) = DIDManager::parse_did(&auth.did).unwrap();
        assert_eq!(node_id, "client-node");
        assert!(client_pubkey.starts_with(&pubkey_short));
        
        // Verify signature
        let verifying_key = DIDManager::verifying_key_from_hex(&client_pubkey).unwrap();
        assert!(DIDManager::verify(&verifying_key, challenge_data.as_bytes(), &signature));
    }

    #[tokio::test]
    async fn test_sync_request_handling() {
        use super::super::protocol::{SyncRequest, SyncFilter, EventMessage};
        
        // Create store with test data
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        
        // Create test room and events
        let room_id = "!test-room:cis.local";
        store.ensure_user("@alice:cis.local").unwrap();
        store.create_room(room_id, "@alice:cis.local", Some("Test Room"), None).unwrap();
        
        // Save some test events
        for i in 0..5 {
            let event_id = format!("$event{}", i);
            store.save_event(
                room_id,
                &event_id,
                "@alice:cis.local",
                "m.room.message",
                &format!("{{\"body\":\"Message {}\"}}", i),
                1000 + i as i64 * 100,
                None,
                None,
            ).unwrap();
        }
        
        // Create connection handler
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();
        let (event_tx, _event_rx) = mpsc::channel(100);
        let tunnel_manager = Arc::new(TunnelManager::with_event_channel(event_tx));
        let did_manager = Arc::new(DIDManager::generate("test").unwrap());
        
        let config = WsServerConfig::new("test.local").with_auth(false);
        
        let handler = ConnectionHandler {
            peer_addr: "127.0.0.1:12345".parse().unwrap(),
            ws_stream: None,
            tunnel_manager,
            store,
            config,
            node_did: "did:cis:test".to_string(),
            did_manager,
            remote_node_id: Some("remote-node".to_string()),
            authenticated: true,
            msg_tx,
            msg_rx,
        };
        
        // Test sync request without since (get all events)
        let request = SyncRequest::new(room_id, None, 10);
        let response = handler.handle_sync_request(&request).await.unwrap();
        
        assert_eq!(response.room_id, room_id);
        assert_eq!(response.events.len(), 5);
        assert!(!response.has_more);
        assert!(response.next_batch.is_some());
        
        // Test sync request with since (pagination)
        let since_event = "$event2";
        let request = SyncRequest::new(room_id, Some(since_event.to_string()), 10);
        let response = handler.handle_sync_request(&request).await.unwrap();
        
        // Should get events after $event2 (i.e., $event3, $event4)
        assert!(response.events.len() >= 2);
        
        // Test sync request with filter
        let filter = SyncFilter::new()
            .with_event_types(vec!["m.room.message".to_string()]);
        let request = SyncRequest::new(room_id, None, 10).with_filter(filter);
        let response = handler.handle_sync_request(&request).await.unwrap();
        
        assert_eq!(response.events.len(), 5); // All events are m.room.message
    }

    #[tokio::test]
    async fn test_sync_response_handling() {
        use super::super::protocol::{SyncResponse, EventMessage};
        use crate::matrix::federation::types::CisMatrixEvent;
        
        // Create store
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        store.ensure_user("@bob:cis.local").unwrap();
        
        // Create connection handler
        let (msg_tx, _msg_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::channel(100);
        let tunnel_manager = Arc::new(TunnelManager::with_event_channel(event_tx));
        let did_manager = Arc::new(DIDManager::generate("test").unwrap());
        
        let config = WsServerConfig::new("test.local").with_auth(false);
        
        let handler = ConnectionHandler {
            peer_addr: "127.0.0.1:12346".parse().unwrap(),
            ws_stream: None,
            tunnel_manager,
            store: store.clone(),
            config,
            node_did: "did:cis:test".to_string(),
            did_manager,
            remote_node_id: Some("remote-node".to_string()),
            authenticated: true,
            msg_tx,
            msg_rx: mpsc::unbounded_channel().1,
        };
        
        // Create test events
        let room_id = "!sync-room:cis.local";
        let events: Vec<EventMessage> = (0..3)
            .map(|i| {
                let event_data = serde_json::json!({
                    "event_id": format!("$sync-event{}", i),
                    "room_id": room_id,
                    "sender": "@bob:cis.local",
                    "type": "m.room.message",
                    "content": {
                        "msgtype": "m.text",
                        "body": format!("Sync message {}", i)
                    },
                    "origin_server_ts": chrono::Utc::now().timestamp_millis(),
                });
                EventMessage::new(
                    format!("$sync-event{}", i),
                    event_data.to_string().into_bytes(),
                    "m.room.message",
                    "@bob:cis.local",
                )
                .with_room_id(room_id)
            })
            .collect();
        
        // Create sync response
        let response = SyncResponse::new(room_id, events).with_has_more(false);
        
        // Handle sync response
        handler.handle_sync_response("remote-node", &response).await.unwrap();
        
        // Verify events were saved
        let messages = store.get_room_messages(room_id, 0, 10).unwrap();
        assert_eq!(messages.len(), 3);
        
        // Verify events were forwarded to channel
        let mut received_count = 0;
        while let Ok((node_id, _event)) = event_rx.try_recv() {
            assert_eq!(node_id, "remote-node");
            received_count += 1;
        }
        assert_eq!(received_count, 3);
    }

    #[test]
    fn test_sync_filter_matching() {
        use super::super::protocol::SyncFilter;
        
        // Test empty filter (matches everything)
        let filter = SyncFilter::new();
        assert!(filter.matches_event_type("m.room.message"));
        assert!(filter.matches_sender("@alice:cis.local"));
        
        // Test event type filter
        let filter = SyncFilter::new()
            .with_event_types(vec!["m.room.message".to_string(), "m.room.member".to_string()]);
        assert!(filter.matches_event_type("m.room.message"));
        assert!(filter.matches_event_type("m.room.member"));
        assert!(!filter.matches_event_type("m.room.create"));
        
        // Test exclusion filter
        let filter = SyncFilter {
            not_event_types: Some(vec!["m.room.message".to_string()]),
            ..Default::default()
        };
        assert!(!filter.matches_event_type("m.room.message"));
        assert!(filter.matches_event_type("m.room.member"));
        
        // Test sender filter
        let filter = SyncFilter::new()
            .with_senders(vec!["@alice:cis.local".to_string()]);
        assert!(filter.matches_sender("@alice:cis.local"));
        assert!(!filter.matches_sender("@bob:cis.local"));
    }
}
