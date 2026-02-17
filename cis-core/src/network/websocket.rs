//! # WebSocket Network Module
//!
//! WebSocket client and server implementations for CIS network communication.
//!
//! ## Features
//!
//! - WebSocket connection management
//! - Message framing and serialization
//! - Connection pooling
//! - Automatic reconnection with exponential backoff
//! - Heartbeat keep-alive mechanism
//! - Concurrent connection handling

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::network::NetworkError;

/// Default WebSocket port
pub const DEFAULT_WS_PORT: u16 = 6768;

/// Default connection timeout
pub const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Default heartbeat interval
pub const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum reconnection attempts
pub const MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsNetworkMessage {
    /// Handshake initiation
    Handshake {
        /// Protocol version
        version: u32,
        /// Node identifier
        node_id: String,
        /// Timestamp
        timestamp: u64,
    },
    /// Data payload
    Data {
        /// Message payload
        payload: Vec<u8>,
        /// Message sequence number
        sequence: u64,
    },
    /// Heartbeat ping
    Ping {
        /// Ping ID
        ping_id: u64,
        /// Timestamp
        timestamp: u64,
    },
    /// Heartbeat pong
    Pong {
        /// Ping ID being responded to
        ping_id: u64,
        /// Timestamp
        timestamp: u64,
    },
    /// Error message
    Error {
        /// Error code
        code: ErrorCode,
        /// Error message
        message: String,
    },
    /// Close message
    Close {
        /// Close reason
        reason: String,
    },
}

/// Error codes for WebSocket errors
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Invalid message format
    InvalidFormat,
    /// Authentication failed
    AuthFailed,
    /// Rate limited
    RateLimited,
    /// Internal error
    InternalError,
    /// Connection closed
    ConnectionClosed,
    /// Timeout
    Timeout,
}

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Initial connecting state
    Connecting,
    /// Handshake in progress
    Handshaking,
    /// Connected and ready
    Connected,
    /// Reconnecting
    Reconnecting,
    /// Disconnected
    Disconnected,
    /// Error state
    Error,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Handshaking => write!(f, "handshaking"),
            ConnectionState::Connected => write!(f, "connected"),
            ConnectionState::Reconnecting => write!(f, "reconnecting"),
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Error => write!(f, "error"),
        }
    }
}

/// WebSocket connection configuration
#[derive(Debug, Clone)]
pub struct WsConnectionConfig {
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Enable auto reconnect
    pub auto_reconnect: bool,
    /// Maximum reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Reconnect interval (base for exponential backoff)
    pub reconnect_interval: Duration,
}

impl Default for WsConnectionConfig {
    fn default() -> Self {
        Self {
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            heartbeat_interval: DEFAULT_HEARTBEAT_INTERVAL,
            auto_reconnect: true,
            max_reconnect_attempts: MAX_RECONNECT_ATTEMPTS,
            reconnect_interval: Duration::from_secs(1),
        }
    }
}

impl WsConnectionConfig {
    /// Create new default config
    pub fn new() -> Self {
        Self::default()
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set heartbeat interval
    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// Set auto reconnect
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }

    /// Set max reconnect attempts
    pub fn with_max_reconnect_attempts(mut self, max: u32) -> Self {
        self.max_reconnect_attempts = max;
        self
    }
}

/// WebSocket connection handle
pub struct WsConnection {
    /// Connection ID
    pub id: String,
    /// Connection state
    state: RwLock<ConnectionState>,
    /// Message sender
    sender: mpsc::UnboundedSender<WsNetworkMessage>,
    /// Last activity timestamp
    last_activity: RwLock<std::time::Instant>,
    /// Reconnect attempt count
    reconnect_attempts: RwLock<u32>,
}

impl WsConnection {
    /// Create a new connection handle
    pub fn new(id: impl Into<String>, sender: mpsc::UnboundedSender<WsNetworkMessage>) -> Self {
        Self {
            id: id.into(),
            state: RwLock::new(ConnectionState::Connecting),
            sender,
            last_activity: RwLock::new(std::time::Instant::now()),
            reconnect_attempts: RwLock::new(0),
        }
    }

    /// Get current state
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Set state
    pub async fn set_state(&self, state: ConnectionState) {
        *self.state.write().await = state;
    }

    /// Send a message
    pub fn send(&self, message: WsNetworkMessage) -> Result<(), NetworkError> {
        self.sender
            .send(message)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send message"
            )))
    }

    /// Update last activity
    pub async fn update_activity(&self) {
        *self.last_activity.write().await = std::time::Instant::now();
    }

    /// Get time since last activity
    pub async fn idle_duration(&self) -> Duration {
        self.last_activity.read().await.elapsed()
    }

    /// Increment reconnect attempts
    pub async fn increment_reconnect_attempts(&self) -> u32 {
        let mut attempts = self.reconnect_attempts.write().await;
        *attempts += 1;
        *attempts
    }

    /// Reset reconnect attempts
    pub async fn reset_reconnect_attempts(&self) {
        *self.reconnect_attempts.write().await = 0;
    }

    /// Get reconnect attempts
    pub async fn reconnect_attempts(&self) -> u32 {
        *self.reconnect_attempts.read().await
    }

    /// Check if should attempt reconnection
    pub async fn should_reconnect(&self, max_attempts: u32) -> bool {
        let attempts = self.reconnect_attempts().await;
        attempts < max_attempts
    }
}

/// WebSocket client for connecting to remote nodes
pub struct WsClient {
    /// Node ID
    node_id: String,
    /// Configuration
    config: WsConnectionConfig,
    /// Active connections
    connections: Arc<RwLock<HashMap<String, Arc<WsConnection>>>>,
}

impl WsClient {
    /// Create a new WebSocket client
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            config: WsConnectionConfig::default(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with custom config
    pub fn with_config(node_id: impl Into<String>, config: WsConnectionConfig) -> Self {
        Self {
            node_id: node_id.into(),
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to a WebSocket server
    pub async fn connect(&self, url: impl Into<String>) -> Result<Arc<WsConnection>, NetworkError> {
        let url = url.into();
        let url_clone = url.clone();
        
        // Attempt connection with timeout
        let connect_result = timeout(
            self.config.connection_timeout,
            connect_async(&url_clone)
        ).await;

        let (ws_stream, _) = match connect_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return Err(NetworkError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    format!("WebSocket connect failed: {}", e)
                )));
            }
            Err(_) => {
                return Err(NetworkError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Connection timeout"
                )));
            }
        };

        info!("WebSocket connected to {}", url);

        // Create connection handler
        let (conn, handle) = self.create_connection_handler(ws_stream, &url).await?;
        
        // Store connection
        {
            let mut conns = self.connections.write().await;
            conns.insert(url.clone(), conn.clone());
        }

        // Spawn handler task
        tokio::spawn(handle.run());

        Ok(conn)
    }

    /// Connect with automatic reconnection
    pub async fn connect_with_reconnect(
        &self,
        url: impl Into<String>,
    ) -> Result<Arc<WsConnection>, NetworkError> {
        let url = url.into();
        let mut last_error = None;

        for attempt in 1..=self.config.max_reconnect_attempts {
            match self.connect(&url).await {
                Ok(conn) => {
                    info!("Successfully connected to {} on attempt {}", url, attempt);
                    conn.reset_reconnect_attempts().await;
                    return Ok(conn);
                }
                Err(e) => {
                    warn!("Connection attempt {} to {} failed: {:?}", attempt, url, e);
                    last_error = Some(e);

                    if attempt < self.config.max_reconnect_attempts {
                        let delay = self.config.reconnect_interval * (1 << attempt.min(5));
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| NetworkError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Max reconnection attempts exceeded"
        ))))
    }

    /// Disconnect from a server
    pub async fn disconnect(&self, url: &str) -> Option<Arc<WsConnection>> {
        let mut conns = self.connections.write().await;
        conns.remove(url)
    }

    /// Get connection by URL
    pub async fn get_connection(&self, url: &str) -> Option<Arc<WsConnection>> {
        let conns = self.connections.read().await;
        conns.get(url).cloned()
    }

    /// Get all connections
    pub async fn get_all_connections(&self) -> Vec<Arc<WsConnection>> {
        let conns = self.connections.read().await;
        conns.values().cloned().collect()
    }

    /// Create connection handler
    async fn create_connection_handler<S>(
        &self,
        ws_stream: WebSocketStream<S>,
        url: &str,
    ) -> Result<(Arc<WsConnection>, ConnectionHandler<S>), NetworkError>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (sender, receiver) = mpsc::unbounded_channel();
        let connection = Arc::new(WsConnection::new(url, sender));
        
        let handler = ConnectionHandler::new(
            ws_stream,
            connection.clone(),
            receiver,
            self.config.clone(),
        );

        Ok((connection, handler))
    }
}

/// Connection handler for managing a single WebSocket connection
struct ConnectionHandler<S> {
    /// WebSocket stream
    ws_stream: WebSocketStream<S>,
    /// Connection handle
    connection: Arc<WsConnection>,
    /// Message receiver
    receiver: mpsc::UnboundedReceiver<WsNetworkMessage>,
    /// Configuration
    config: WsConnectionConfig,
    /// Ping counter (reserved for future ping tracking)
    _ping_counter: u64,
}

impl<S> ConnectionHandler<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    /// Create a new connection handler
    fn new(
        ws_stream: WebSocketStream<S>,
        connection: Arc<WsConnection>,
        receiver: mpsc::UnboundedReceiver<WsNetworkMessage>,
        config: WsConnectionConfig,
    ) -> Self {
        Self {
            ws_stream,
            connection,
            receiver,
            config,
            _ping_counter: 0,
        }
    }

    /// Run the connection handler
    async fn run(self) {
        let (mut ws_sender, mut ws_receiver) = self.ws_stream.split();
        let mut heartbeat_interval = interval(self.config.heartbeat_interval);
        let connection = self.connection.clone();
        let mut receiver = self.receiver;
        let mut ping_counter = 0u64;

        connection.set_state(ConnectionState::Connected).await;
        connection.update_activity().await;

        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                Some(msg_result) = ws_receiver.next() => {
                    match msg_result {
                        Ok(msg) => {
                            if let Err(e) = Self::handle_ws_message(&connection, msg).await {
                                warn!("Error handling message: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                    }
                }

                // Handle outgoing messages
                Some(msg) = receiver.recv() => {
                    let json = match serde_json::to_string(&msg) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize message: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = ws_sender.send(Message::Text(json)).await {
                        error!("Failed to send message: {}", e);
                        break;
                    }
                }

                // Send heartbeat
                _ = heartbeat_interval.tick() => {
                    ping_counter += 1;
                    let ping = WsNetworkMessage::Ping {
                        ping_id: ping_counter,
                        timestamp: current_timestamp(),
                    };

                    let json = match serde_json::to_string(&ping) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize ping: {}", e);
                            continue;
                        }
                    };

                    if let Err(e) = ws_sender.send(Message::Text(json)).await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }

                    // Check idle timeout
                    if connection.idle_duration().await > Duration::from_secs(60) {
                        warn!("Connection idle timeout, closing");
                        break;
                    }
                }

                else => break,
            }
        }

        // Cleanup
        connection.set_state(ConnectionState::Disconnected).await;
        info!("Connection handler for {} stopped", connection.id);
    }

    /// Handle incoming WebSocket message
    async fn handle_ws_message(
        connection: &Arc<WsConnection>,
        msg: Message,
    ) -> Result<(), NetworkError> {
        match msg {
            Message::Text(text) => {
                let ws_msg: WsNetworkMessage = serde_json::from_str(&text)
                    .map_err(|e| NetworkError::VerificationFailed(format!("Parse error: {}", e)))?;

                match ws_msg {
                    WsNetworkMessage::Pong { .. } => {
                        connection.update_activity().await;
                    }
                    WsNetworkMessage::Ping { ping_id, .. } => {
                        // Send pong response
                        let pong = WsNetworkMessage::Pong {
                            ping_id,
                            timestamp: current_timestamp(),
                        };
                        let _ = connection.send(pong);
                    }
                    _ => {
                        connection.update_activity().await;
                    }
                }

                Ok(())
            }
            Message::Close(_) => {
                info!("Connection closed by remote");
                Err(NetworkError::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionReset,
                    "Connection closed"
                )))
            }
            _ => Ok(()),
        }
    }
}

/// WebSocket server for accepting incoming connections
///
/// TODO: P1-13 - Server infrastructure for incoming WebSocket connections
/// Currently unused but reserved for future peer-to-peer listening functionality
pub struct WsServer {
    /// Bind address
    bind_addr: String,
    /// Port
    port: u16,
    /// Configuration
    config: WsConnectionConfig,
    /// Connection handler callback
    connection_handler: Arc<dyn Fn(Arc<WsConnection>) + Send + Sync>,
}

impl WsServer {
    /// Create a new WebSocket server
    pub fn new(bind_addr: impl Into<String>, port: u16) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            port,
            config: WsConnectionConfig::default(),
            connection_handler: Arc::new(|_| {}),
        }
    }

    /// Set connection handler
    pub fn on_connection<F>(mut self, handler: F) -> Self
    where
        F: Fn(Arc<WsConnection>) + Send + Sync + 'static,
    {
        self.connection_handler = Arc::new(handler);
        self
    }

    /// Run the server
    pub async fn run(&self) -> Result<(), NetworkError> {
        let addr = format!("{}:{}", self.bind_addr, self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(NetworkError::Io)?;

        info!("WebSocket server listening on ws://{}", addr);

        let connections = Arc::new(RwLock::new(HashMap::<String, Arc<WsConnection>>::new()));

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let handler = self.connection_handler.clone();
                    let conns = connections.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, peer_addr, handler, conns).await {
                            error!("Connection handler error for {}: {:?}", peer_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle incoming connection
    async fn handle_connection(
        stream: TcpStream,
        peer_addr: std::net::SocketAddr,
        handler: Arc<dyn Fn(Arc<WsConnection>) + Send + Sync>,
        connections: Arc<RwLock<HashMap<String, Arc<WsConnection>>>>,
    ) -> Result<(), NetworkError> {
        debug!("New WebSocket connection from {}", peer_addr);

        let ws_stream = accept_async(stream).await
            .map_err(|e| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!("WebSocket handshake failed: {}", e)
            )))?;

        let (sender, receiver) = mpsc::unbounded_channel();
        let conn_id = format!("{}-{}", peer_addr, uuid::Uuid::new_v4());
        let connection = Arc::new(WsConnection::new(&conn_id, sender));

        // Store connection
        {
            let mut conns = connections.write().await;
            conns.insert(conn_id.clone(), connection.clone());
        }

        // Call connection handler
        handler(connection.clone());

        // Create and run connection handler
        let conn_handler = ConnectionHandler::new(
            ws_stream,
            connection.clone(),
            receiver,
            WsConnectionConfig::default(),
        );

        conn_handler.run().await;

        // Remove connection on disconnect
        {
            let mut conns = connections.write().await;
            conns.remove(&conn_id);
        }

        Ok(())
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Unit Tests ====================

    #[test]
    fn test_ws_network_message_serialization() {
        let ping = WsNetworkMessage::Ping {
            ping_id: 1,
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("ping"));
        assert!(json.contains("1"));

        let decoded: WsNetworkMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, WsNetworkMessage::Ping { ping_id: 1, .. }));
    }

    #[test]
    fn test_ws_network_message_data() {
        let data = WsNetworkMessage::Data {
            payload: vec![1, 2, 3, 4, 5],
            sequence: 42,
        };

        let json = serde_json::to_string(&data).unwrap();
        let decoded: WsNetworkMessage = serde_json::from_str(&json).unwrap();

        match decoded {
            WsNetworkMessage::Data { payload, sequence } => {
                assert_eq!(payload, vec![1, 2, 3, 4, 5]);
                assert_eq!(sequence, 42);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_error_code_serialization() {
        let error = WsNetworkMessage::Error {
            code: ErrorCode::AuthFailed,
            message: "Authentication failed".to_string(),
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("auth_failed"));

        let decoded: WsNetworkMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            WsNetworkMessage::Error { code, message } => {
                assert!(matches!(code, ErrorCode::AuthFailed));
                assert_eq!(message, "Authentication failed");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(format!("{}", ConnectionState::Connecting), "connecting");
        assert_eq!(format!("{}", ConnectionState::Connected), "connected");
        assert_eq!(format!("{}", ConnectionState::Disconnected), "disconnected");
    }

    #[test]
    fn test_ws_connection_config_default() {
        let config = WsConnectionConfig::default();
        assert_eq!(config.connection_timeout, DEFAULT_CONNECTION_TIMEOUT);
        assert_eq!(config.heartbeat_interval, DEFAULT_HEARTBEAT_INTERVAL);
        assert!(config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, MAX_RECONNECT_ATTEMPTS);
    }

    #[test]
    fn test_ws_connection_config_builder() {
        let config = WsConnectionConfig::new()
            .with_connection_timeout(Duration::from_secs(60))
            .with_heartbeat_interval(Duration::from_secs(10))
            .with_auto_reconnect(false)
            .with_max_reconnect_attempts(5);

        assert_eq!(config.connection_timeout, Duration::from_secs(60));
        assert_eq!(config.heartbeat_interval, Duration::from_secs(10));
        assert!(!config.auto_reconnect);
        assert_eq!(config.max_reconnect_attempts, 5);
    }

    #[tokio::test]
    async fn test_ws_connection_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        assert_eq!(conn.id, "test-conn");
        assert!(matches!(conn.state().await, ConnectionState::Connecting));
        assert_eq!(conn.reconnect_attempts().await, 0);
    }

    #[tokio::test]
    async fn test_ws_connection_state_changes() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        conn.set_state(ConnectionState::Connected).await;
        assert!(matches!(conn.state().await, ConnectionState::Connected));

        conn.set_state(ConnectionState::Reconnecting).await;
        assert!(matches!(conn.state().await, ConnectionState::Reconnecting));
    }

    #[tokio::test]
    async fn test_ws_connection_reconnect_attempts() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        assert_eq!(conn.increment_reconnect_attempts().await, 1);
        assert_eq!(conn.increment_reconnect_attempts().await, 2);
        assert_eq!(conn.reconnect_attempts().await, 2);

        conn.reset_reconnect_attempts().await;
        assert_eq!(conn.reconnect_attempts().await, 0);
    }

    #[tokio::test]
    async fn test_ws_connection_should_reconnect() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        // Should reconnect when attempts < max
        assert!(conn.should_reconnect(3).await);

        conn.increment_reconnect_attempts().await;
        conn.increment_reconnect_attempts().await;
        conn.increment_reconnect_attempts().await;

        // Should not reconnect when attempts >= max
        assert!(!conn.should_reconnect(3).await);
    }

    #[tokio::test]
    async fn test_ws_connection_activity() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        // Initially should have some elapsed time (very small)
        tokio::time::sleep(Duration::from_millis(10)).await;
        let idle = conn.idle_duration().await;
        assert!(idle >= Duration::from_millis(10));

        // Update activity
        conn.update_activity().await;
        let idle_after = conn.idle_duration().await;
        assert!(idle_after < Duration::from_millis(5));
    }

    #[tokio::test]
    async fn test_ws_connection_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let conn = WsConnection::new("test-conn", tx);

        let ping = WsNetworkMessage::Ping {
            ping_id: 1,
            timestamp: 12345,
        };

        conn.send(ping.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert!(matches!(received, WsNetworkMessage::Ping { ping_id: 1, .. }));
    }

    #[test]
    fn test_ws_client_creation() {
        let client = WsClient::new("test-node");
        assert_eq!(client.node_id, "test-node");
    }

    #[test]
    fn test_ws_client_with_config() {
        let config = WsConnectionConfig::new()
            .with_auto_reconnect(false);
        
        let client = WsClient::with_config("test-node", config);
        assert_eq!(client.node_id, "test-node");
    }

    #[tokio::test]
    async fn test_ws_client_connection_management() {
        let client = WsClient::new("test-node");
        
        // Initially no connections
        assert_eq!(client.get_all_connections().await.len(), 0);

        // Simulate adding a connection (we can't actually connect without a server)
        let (tx, _rx) = mpsc::unbounded_channel();
        let conn = Arc::new(WsConnection::new("ws://test:1234", tx));
        
        {
            let mut conns = client.connections.write().await;
            conns.insert("ws://test:1234".to_string(), conn);
        }

        assert_eq!(client.get_all_connections().await.len(), 1);
        
        let retrieved = client.get_connection("ws://test:1234").await;
        assert!(retrieved.is_some());

        // Disconnect
        let removed = client.disconnect("ws://test:1234").await;
        assert!(removed.is_some());
        assert_eq!(client.get_all_connections().await.len(), 0);
    }

    #[test]
    fn test_error_code_variants() {
        let codes = vec![
            ErrorCode::InvalidFormat,
            ErrorCode::AuthFailed,
            ErrorCode::RateLimited,
            ErrorCode::InternalError,
            ErrorCode::ConnectionClosed,
            ErrorCode::Timeout,
        ];

        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let decoded: ErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(code, decoded);
        }
    }

    #[test]
    fn test_ws_network_message_all_variants() {
        // Test all message types
        let messages = vec![
            WsNetworkMessage::Handshake {
                version: 1,
                node_id: "test-node".to_string(),
                timestamp: 12345,
            },
            WsNetworkMessage::Data {
                payload: vec![1, 2, 3],
                sequence: 1,
            },
            WsNetworkMessage::Ping {
                ping_id: 1,
                timestamp: 12345,
            },
            WsNetworkMessage::Pong {
                ping_id: 1,
                timestamp: 12345,
            },
            WsNetworkMessage::Error {
                code: ErrorCode::InternalError,
                message: "test error".to_string(),
            },
            WsNetworkMessage::Close {
                reason: "test close".to_string(),
            },
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).unwrap();
            let decoded: WsNetworkMessage = serde_json::from_str(&json).unwrap();
            // Just ensure serialization round-trip works
            assert!(std::mem::discriminant(&msg) == std::mem::discriminant(&decoded));
        }
    }

    #[tokio::test]
    async fn test_concurrent_connections() {
        use tokio::sync::Mutex;
        
        let client = Arc::new(WsClient::new("test-node"));
        let counter = Arc::new(Mutex::new(0));
        
        // Simulate multiple concurrent "connections"
        let mut handles = vec![];
        
        for i in 0..10 {
            let client = client.clone();
            let counter = counter.clone();
            
            let handle = tokio::spawn(async move {
                let (tx, _rx) = mpsc::unbounded_channel();
                let conn = Arc::new(WsConnection::new(format!("conn-{}", i), tx));
                
                {
                    let mut conns = client.connections.write().await;
                    conns.insert(format!("ws://test:{}", i), conn);
                }
                
                let mut count = counter.lock().await;
                *count += 1;
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await.unwrap();
        }
        
        assert_eq!(*counter.lock().await, 10);
        assert_eq!(client.get_all_connections().await.len(), 10);
    }
}
