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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::matrix::federation::types::{CisMatrixEvent, PeerInfo};
use crate::matrix::store::MatrixStore;

use super::protocol::{
    AckMessage, AuthMessage, ErrorCode, ErrorMessage, HandshakeMessage, WsMessage, PROTOCOL_VERSION,
    WS_PATH,
};
use super::tunnel::{Tunnel, TunnelManager, TunnelState};

/// WebSocket federation server
#[derive(Debug)]
pub struct WebSocketServer {
    /// Server configuration
    config: WsServerConfig,
    /// Tunnel manager for connection management
    tunnel_manager: Arc<TunnelManager>,
    /// Matrix store for event persistence
    store: Arc<MatrixStore>,
    /// This node's DID
    node_did: String,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
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
    ) -> Self {
        Self {
            config,
            tunnel_manager,
            store,
            node_did: node_did.into(),
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
    /// WebSocket stream
    ws_stream: WebSocketStream<TcpStream>,
    /// Tunnel manager
    tunnel_manager: Arc<TunnelManager>,
    /// Matrix store
    store: Arc<MatrixStore>,
    /// Server config
    config: WsServerConfig,
    /// This node's DID
    node_did: String,
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
    ) -> Self {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        Self {
            peer_addr,
            ws_stream,
            tunnel_manager,
            store,
            config,
            node_did,
            remote_node_id: None,
            authenticated: false,
            msg_tx,
            msg_rx,
        }
    }

    /// Run the connection handler
    async fn run(mut self) -> Result<(), WsServerError> {
        // Split the WebSocket stream
        let (mut ws_sender, mut ws_receiver) = self.ws_stream.split();

        // Start handshake
        self.perform_handshake().await?;

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

    /// Perform Noise handshake (simplified)
    async fn perform_handshake(&mut self) -> Result<(), WsServerError> {
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
            Message::Ping(data) => {
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
        }
    }

    /// Verify authentication (placeholder)
    async fn verify_auth(&self, _auth: &AuthMessage) -> Result<(), WsServerError> {
        // TODO: Implement actual DID verification
        // 1. Verify signature
        // 2. Check challenge response
        // 3. Validate DID
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

        Ok(WebSocketServer::new(config, tunnel_manager, Arc::new(store), node_did))
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
        let server = WebSocketServerBuilder::new()
            .config(WsServerConfig::new("builder.local"))
            .node_did("did:cis:builder")
            .build();

        assert!(server.is_ok());
        let server = server.unwrap();
        assert_eq!(server.server_name(), "builder.local");
    }
}
