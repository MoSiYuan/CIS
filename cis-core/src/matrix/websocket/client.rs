//! # WebSocket Federation Client
//!
//! WebSocket client for connecting to peer CIS nodes.
//!
//! ## Features
//!
//! - Connect to remote nodes via WebSocket
//! - Noise protocol handshake
//! - DID authentication
//! - Automatic reconnection
//! - UDP hole punching support

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::matrix::federation::types::PeerInfo;

use super::protocol::{
    build_ws_url, AuthMessage, HandshakeMessage, WsMessage, PROTOCOL_VERSION,
    WS_PATH,
};
use super::tunnel::{Tunnel, TunnelError, TunnelManager, TunnelState};

/// WebSocket federation client
#[derive(Debug)]
pub struct WebSocketClient {
    /// This node's ID
    node_id: String,
    /// This node's DID
    node_did: String,
    /// Authentication key (for DID auth)
    auth_key: Option<Vec<u8>>,
}

/// Connection options
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// Connection timeout
    pub timeout: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnect interval
    pub reconnect_interval: Duration,
    /// Maximum reconnection attempts
    pub max_reconnects: u32,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(5),
            max_reconnects: 10,
        }
    }
}

impl ConnectOptions {
    /// Create default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set auto reconnect
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub fn new(node_id: impl Into<String>, node_did: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            node_did: node_did.into(),
            auth_key: None,
        }
    }

    /// Set authentication key
    pub fn with_auth_key(mut self, key: Vec<u8>) -> Self {
        self.auth_key = Some(key);
        self
    }

    /// Connect to a peer node
    pub async fn connect(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        let options = ConnectOptions::default();
        self.connect_with_options(peer, tunnel_manager, options).await
    }

    /// Connect with custom options
    pub async fn connect_with_options(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
        options: ConnectOptions,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        info!("Connecting to peer: {} at {}:{}", peer.server_name, peer.host, peer.port);

        // Build WebSocket URL
        let ws_url = if peer.port == 6768 {
            // Use WebSocket port
            build_ws_url(&peer.host, peer.port, peer.use_https)
        } else {
            // Fallback to HTTP port with ws path
            let scheme = if peer.use_https { "wss" } else { "ws" };
            format!("{}://{}:{}{}", scheme, peer.host, peer.port, WS_PATH)
        };

        debug!("WebSocket URL: {}", ws_url);

        // Attempt connection with timeout
        let connect_result = timeout(options.timeout, connect_async(&ws_url)).await;

        let (ws_stream, _) = match connect_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return Err(WsClientError::ConnectionError(format!(
                    "WebSocket connect failed: {}",
                    e
                )))
            }
            Err(_) => return Err(WsClientError::Timeout),
        };

        info!("WebSocket connected to {}", peer.server_name);

        // Setup connection handler
        let handler = ClientConnectionHandler::new(
            ws_stream,
            self.node_id.clone(),
            self.node_did.clone(),
            self.auth_key.clone(),
            tunnel_manager.clone(),
            peer.server_name.clone(),
            options,
        );

        // Start the connection handler
        let tunnel = handler.run().await?;

        Ok(tunnel)
    }

    /// Connect with automatic retry
    pub async fn connect_with_retry(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
        max_attempts: u32,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match self.connect(peer, tunnel_manager.clone()).await {
                Ok(tunnel) => {
                    info!("Successfully connected to {} on attempt {}", peer.server_name, attempt);
                    return Ok(tunnel);
                }
                Err(e) => {
                    warn!(
                        "Connection attempt {} to {} failed: {:?}",
                        attempt, peer.server_name, e
                    );
                    last_error = Some(e);

                    if attempt < max_attempts {
                        let delay = Duration::from_millis(500 * (1 << attempt.min(4)));
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(WsClientError::MaxRetriesExceeded))
    }

    /// Attempt UDP hole punching (placeholder)
    pub async fn punch_hole(&self, _target_node: &str) -> Result<SocketAddr, WsClientError> {
        // TODO: Implement UDP hole punching
        // 1. Contact relay/anchor server
        // 2. Get both peers' public endpoints
        // 3. Send UDP packets simultaneously
        // 4. Detect NAT type
        // 5. Establish direct connection or fallback to relay

        warn!("UDP hole punching not yet implemented");
        Err(WsClientError::NotImplemented("Hole punching".to_string()))
    }

    /// Check if a peer is reachable
    pub async fn health_check(&self, peer: &PeerInfo) -> bool {
        let ws_url = build_ws_url(&peer.host, peer.port, peer.use_https);

        match timeout(Duration::from_secs(5), connect_async(&ws_url)).await {
            Ok(Ok(_)) => true,
            _ => false,
        }
    }
}

/// Client connection handler
struct ClientConnectionHandler {
    /// WebSocket stream (wrapped in Option for ownership transfer)
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    /// This node's ID
    node_id: String,
    /// This node's DID
    node_did: String,
    /// Authentication key
    auth_key: Option<Vec<u8>>,
    /// Tunnel manager
    tunnel_manager: Arc<TunnelManager>,
    /// Remote node name
    remote_node: String,
    /// Connection options
    options: ConnectOptions,
    /// Message channel
    msg_tx: mpsc::UnboundedSender<WsMessage>,
    /// Message receiver
    msg_rx: mpsc::UnboundedReceiver<WsMessage>,
    /// Connection state
    state: ConnectionState,
    /// Ping ID counter
    ping_counter: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ConnectionState {
    Connecting,
    Handshaking,
    Authenticating,
    Ready,
    Error,
    Closed,
}

impl ClientConnectionHandler {
    /// Create a new client connection handler
    fn new(
        ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        node_id: String,
        node_did: String,
        auth_key: Option<Vec<u8>>,
        tunnel_manager: Arc<TunnelManager>,
        remote_node: String,
        options: ConnectOptions,
    ) -> Self {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        Self {
            ws_stream: Some(ws_stream),
            node_id,
            node_did,
            auth_key,
            tunnel_manager,
            remote_node,
            options,
            msg_tx,
            msg_rx,
            state: ConnectionState::Connecting,
            ping_counter: 0,
        }
    }

    /// Run the connection handler
    async fn run(mut self) -> Result<Arc<Tunnel>, WsClientError> {
        use futures::StreamExt;

        // Take ownership of the stream
        let ws_stream = self.ws_stream.take()
            .ok_or_else(|| WsClientError::ConnectionError("WebSocket stream already taken".to_string()))?;
        
        // Split the stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Register tunnel first
        let tunnel = self
            .tunnel_manager
            .register_tunnel(&self.remote_node, self.msg_tx.clone())
            .await;

        // Send handshake
        self.send_handshake().await?;
        self.state = ConnectionState::Handshaking;
        tunnel.set_state(TunnelState::Handshaking).await;

        // Wait for handshake response with timeout
        let handshake_result = timeout(self.options.timeout, async {
            while let Some(msg_result) = ws_receiver.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if let WsMessage::Handshake(_) = ws_msg {
                                    return Ok(ws_msg);
                                }
                            }
                        }
                    }
                    Err(e) => return Err(WsClientError::ConnectionError(e.to_string())),
                }
            }
            Err(WsClientError::ConnectionClosed)
        })
        .await;

        match handshake_result {
            Ok(Ok(_)) => {
                debug!("Handshake successful with {}", self.remote_node);
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(WsClientError::Timeout),
        }

        // Send authentication
        self.send_auth().await?;
        self.state = ConnectionState::Authenticating;
        tunnel.set_state(TunnelState::Authenticating).await;

        // Wait for auth response
        let auth_result = timeout(self.options.timeout, async {
            while let Some(msg_result) = ws_receiver.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                match ws_msg {
                                    WsMessage::Ack(ack) => {
                                        if ack.status == super::protocol::AckStatus::Success {
                                            return Ok(());
                                        } else {
                                            return Err(WsClientError::AuthFailed(
                                                ack.error.unwrap_or_default(),
                                            ));
                                        }
                                    }
                                    WsMessage::Error(err) => {
                                        return Err(WsClientError::AuthFailed(err.message));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => return Err(WsClientError::ConnectionError(e.to_string())),
                }
            }
            Err(WsClientError::ConnectionClosed)
        })
        .await;

        match auth_result {
            Ok(Ok(())) => {
                info!("Authenticated with {}", self.remote_node);
                self.state = ConnectionState::Ready;
                tunnel.set_state(TunnelState::Ready).await;
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(WsClientError::Timeout),
        }

        // Spawn message processing task
        let remote_node = self.remote_node.clone();
        let tunnel_manager = self.tunnel_manager.clone();
        let mut msg_rx = self.msg_rx;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg_result) = ws_receiver.next() => {
                        match msg_result {
                            Ok(msg) => {
                                if let Err(e) = Self::handle_incoming_message(
                                    &remote_node,
                                    &tunnel_manager,
                                    msg,
                                )
                                .await
                                {
                                    warn!("Error handling message: {:?}", e);
                                }
                            }
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }
                    Some(msg) = msg_rx.recv() => {
                        let ws_msg = Message::Text(
                            match serde_json::to_string(&msg) {
                                Ok(json) => json,
                                Err(e) => {
                                    error!("Failed to serialize message: {}", e);
                                    continue;
                                }
                            }
                        );
                        use futures::SinkExt;
                        if let Err(e) = ws_sender.send(ws_msg).await {
                            error!("Failed to send message: {}", e);
                            break;
                        }
                    }
                    else => break,
                }
            }

            // Cleanup
            tunnel_manager.remove_tunnel(&remote_node).await;
            debug!("Client connection handler for {} stopped", remote_node);
        });

        Ok(tunnel)
    }

    /// Send handshake message
    async fn send_handshake(&self) -> Result<(), WsClientError> {
        let handshake = HandshakeMessage::new(
            PROTOCOL_VERSION,
            vec![], // Noise payload would go here
            &self.node_did,
        );

        let msg = WsMessage::Handshake(handshake);
        self.msg_tx
            .send(msg)
            .map_err(|_| WsClientError::SendError)
    }

    /// Send authentication message
    async fn send_auth(&self) -> Result<(), WsClientError> {
        // Create challenge response (placeholder)
        let challenge_response = vec![0u8; 32]; // Would be signed challenge

        let auth = AuthMessage::new(
            &self.node_did,
            challenge_response,
            self.auth_key.clone().unwrap_or_default(),
        );

        let msg = WsMessage::Auth(auth);
        self.msg_tx
            .send(msg)
            .map_err(|_| WsClientError::SendError)
    }

    /// Handle incoming message
    async fn handle_incoming_message(
        remote_node: &str,
        tunnel_manager: &TunnelManager,
        msg: Message,
    ) -> Result<(), WsClientError> {
        match msg {
            Message::Text(text) => {
                let ws_msg: WsMessage = serde_json::from_str(&text)
                    .map_err(|e| WsClientError::SerializationError(e.to_string()))?;

                match ws_msg {
                    WsMessage::Ping(ping) => {
                        // Send pong
                        if let Some(tunnel) = tunnel_manager.get_tunnel(remote_node).await {
                            let _ = tunnel.send_pong(ping.ping_id);
                        }
                    }
                    WsMessage::Pong(pong) => {
                        tunnel_manager.handle_pong(remote_node, pong.ping_id).await;
                    }
                    WsMessage::Event(event) => {
                        tunnel_manager.handle_event(remote_node, event).await;
                    }
                    WsMessage::Error(err) => {
                        warn!("Received error from {}: {:?}", remote_node, err);
                    }
                    _ => {
                        debug!("Received message: {:?}", ws_msg);
                    }
                }
            }
            Message::Close(_) => {
                return Err(WsClientError::ConnectionClosed);
            }
            _ => {}
        }

        Ok(())
    }
}

/// WebSocket client errors
#[derive(Debug, thiserror::Error, Clone)]
pub enum WsClientError {
    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout
    #[error("Connection timeout")]
    Timeout,

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

    /// Not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Max retries exceeded
    #[error("Maximum retry attempts exceeded")]
    MaxRetriesExceeded,

    /// Tunnel error
    #[error("Tunnel error: {0}")]
    TunnelError(String),
}

impl From<TunnelError> for WsClientError {
    fn from(err: TunnelError) -> Self {
        WsClientError::TunnelError(err.to_string())
    }
}

/// WebSocket client builder
#[derive(Debug, Default)]
pub struct WebSocketClientBuilder {
    node_id: Option<String>,
    node_did: Option<String>,
    auth_key: Option<Vec<u8>>,
}

impl WebSocketClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set node ID
    pub fn node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Set node DID
    pub fn node_did(mut self, did: impl Into<String>) -> Self {
        self.node_did = Some(did.into());
        self
    }

    /// Set authentication key
    pub fn auth_key(mut self, key: Vec<u8>) -> Self {
        self.auth_key = Some(key);
        self
    }

    /// Build the client
    pub fn build(self) -> Result<WebSocketClient, WsClientError> {
        let node_id = self.node_id.ok_or_else(|| {
            WsClientError::ConnectionError("Node ID is required".to_string())
        })?;

        let node_did = self.node_did.unwrap_or_else(|| node_id.clone());

        let client = WebSocketClient::new(node_id, node_did);

        Ok(if let Some(key) = self.auth_key {
            client.with_auth_key(key)
        } else {
            client
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_options() {
        let opts = ConnectOptions::new().with_timeout(Duration::from_secs(60));
        assert_eq!(opts.timeout, Duration::from_secs(60));
        assert!(opts.auto_reconnect);
    }

    #[test]
    fn test_client_builder() {
        let client = WebSocketClientBuilder::new()
            .node_id("test-node")
            .node_did("did:cis:test")
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.node_id, "test-node");
        assert_eq!(client.node_did, "did:cis:test");
    }

    #[test]
    fn test_client_builder_missing_node_id() {
        let client = WebSocketClientBuilder::new().build();
        assert!(client.is_err());
    }
}
