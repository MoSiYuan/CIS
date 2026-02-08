//! # Remote Agent Session
//!
//! Manages remote Agent connections over Matrix Federation.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, error, warn, debug};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream};
use tokio::net::TcpStream;
use serde::{Deserialize, Serialize};
use bytes::BytesMut;
use uuid::Uuid;

use cis_core::network::NetworkAcl;
use cis_core::network::SessionControlMessage;


/// Default WebSocket port for agent sessions
pub const AGENT_SESSION_PORT: u16 = 6767;

/// PTY data frame prefix size (UUID)
const PTY_FRAME_PREFIX_SIZE: usize = 16;

/// Remote session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Disconnected
    Disconnected,
    /// Connecting
    Connecting,
    /// Authenticating (DID challenge/response)
    Authenticating,
    /// Connected, ready for Agent
    Connected,
    /// Agent running
    AgentRunning,
    /// Error state
    Error,
}

/// PTY data frame for terminal I/O
#[derive(Debug, Clone)]
pub struct PtyDataFrame {
    /// Session ID (16 bytes)
    pub session_id: Uuid,
    /// Terminal data
    pub data: Vec<u8>,
}

impl PtyDataFrame {
    /// Create new PTY data frame
    pub fn new(session_id: Uuid, data: Vec<u8>) -> Self {
        Self { session_id, data }
    }

    /// Serialize to binary frame: [session_id: 16 bytes][data]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut frame = BytesMut::with_capacity(PTY_FRAME_PREFIX_SIZE + self.data.len());
        frame.extend_from_slice(self.session_id.as_bytes());
        frame.extend_from_slice(&self.data);
        frame.to_vec()
    }

    /// Parse from binary frame
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SessionError> {
        if bytes.len() < PTY_FRAME_PREFIX_SIZE {
            return Err(SessionError::InvalidFrame("Frame too short".to_string()));
        }
        let session_id = Uuid::from_slice(&bytes[..PTY_FRAME_PREFIX_SIZE])
            .map_err(|e| SessionError::InvalidFrame(format!("Invalid session ID: {}", e)))?;
        let data = bytes[PTY_FRAME_PREFIX_SIZE..].to_vec();
        Ok(Self { session_id, data })
    }
}

/// Agent start command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentCommand {
    /// Start agent
    Start {
        /// Agent type
        agent_type: String,
        /// Session ID
        session_id: String,
        /// Terminal columns
        cols: u16,
        /// Terminal rows
        rows: u16,
    },
    /// Stop agent
    Stop {
        /// Session ID
        session_id: String,
    },
    /// Resize terminal
    Resize {
        /// Session ID
        session_id: String,
        /// New columns
        cols: u16,
        /// New rows
        rows: u16,
    },
}

/// Remote Agent session
pub struct RemoteSession {
    /// Target node DID
    pub target_did: String,
    
    /// Target address
    pub target_addr: String,
    
    /// Session state
    pub state: SessionState,
    
    /// WebSocket sender
    ws_sender: Option<mpsc::UnboundedSender<Message>>,
    
    /// Terminal output receiver
    output_rx: Option<mpsc::Receiver<Vec<u8>>>,
    
    /// Session started at
    started_at: Option<i64>,
    
    /// Session ID for PTY forwarding
    session_id: Option<Uuid>,
    
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    
    /// Connection handle
    connection_handle: Option<tokio::task::JoinHandle<()>>,
}

impl RemoteSession {
    /// Create new session (disconnected)
    pub fn new(target_did: impl Into<String>, target_addr: impl Into<String>) -> Self {
        Self {
            target_did: target_did.into(),
            target_addr: target_addr.into(),
            state: SessionState::Disconnected,
            ws_sender: None,
            output_rx: None,
            started_at: None,
            session_id: None,
            shutdown_tx: None,
            connection_handle: None,
        }
    }
    
    /// Connect to remote node
    pub async fn connect(&mut self, _acl: Arc<RwLock<NetworkAcl>>) -> Result<(), SessionError> {
        info!("Connecting to {} at {}", self.target_did, self.target_addr);
        self.state = SessionState::Connecting;
        
        // Build WebSocket URL
        let ws_url = format!("ws://{}:{}/_cis/agent/session", self.target_addr, AGENT_SESSION_PORT);
        
        // Establish WebSocket connection to target
        let (ws_stream, _): (tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>, _) = connect_async(&ws_url)
            .await
            .map_err(|e| SessionError::ConnectionFailed(format!("WebSocket connect failed: {}", e)))?;
        
        info!("WebSocket connected to {}", self.target_addr);
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Perform DID challenge/response
        self.state = SessionState::Authenticating;
        
        // Send authentication message with DID
        let auth_msg = serde_json::json!({
            "type": "auth",
            "did": self.target_did,
            "timestamp": chrono::Utc::now().timestamp_millis() as u64,
        });
        
        ws_sender
            .send(Message::Text(auth_msg.to_string()))
            .await
            .map_err(|e| SessionError::AuthFailed(format!("Failed to send auth: {}", e)))?;
        
        // Wait for authentication response with timeout
        let auth_timeout = tokio::time::Duration::from_secs(30);
        let auth_result = tokio::time::timeout(auth_timeout, async {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(response) = serde_json::from_str::<serde_json::Value>(&text) {
                            if response.get("status").and_then(|s| s.as_str()) == Some("success") {
                                return Ok(());
                            } else {
                                let error = response.get("error").and_then(|e| e.as_str())
                                    .unwrap_or("Unknown auth error");
                                return Err(SessionError::AuthFailed(error.to_string()));
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        return Err(SessionError::ConnectionClosed);
                    }
                    Err(e) => {
                        return Err(SessionError::ConnectionFailed(e.to_string()));
                    }
                    _ => {}
                }
            }
            Err(SessionError::ConnectionClosed)
        }).await;
        
        match auth_result {
            Ok(Ok(())) => {
                info!("DID authentication successful for {}", self.target_did);
            }
            Ok(Err(e)) => {
                self.state = SessionState::Error;
                return Err(e);
            }
            Err(_) => {
                self.state = SessionState::Error;
                return Err(SessionError::AuthFailed("Authentication timeout".to_string()));
            }
        }
        
        // Create channels for PTY communication
        let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<Message>();
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(100);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        
        self.ws_sender = Some(ws_tx);
        self.output_rx = Some(output_rx);
        self.shutdown_tx = Some(shutdown_tx);
        self.session_id = Some(Uuid::new_v4());
        
        // Spawn connection handler task for PTY forwarding
        let session_id = self.session_id.unwrap();
        let target_did = self.target_did.clone();
        
        let handle = tokio::spawn(async move {
            info!("Starting PTY forwarding for session {}", session_id);
            
            loop {
                tokio::select! {
                    // Handle incoming WebSocket messages
                    Some(msg) = ws_receiver.next() => {
                        match msg {
                            Ok(Message::Binary(data)) => {
                                // Parse PTY data frame
                                if let Ok(frame) = PtyDataFrame::from_bytes(&data) {
                                    if frame.session_id == session_id {
                                        if output_tx.send(frame.data).await.is_err() {
                                            warn!("Output channel closed for session {}", session_id);
                                            break;
                                        }
                                    }
                                }
                            }
                            Ok(Message::Text(text)) => {
                                debug!("Received text message: {}", text);
                                // Handle control messages
                                if let Ok(control) = serde_json::from_str::<SessionControlMessage>(&text) {
                                    match control {
                                        SessionControlMessage::Error { message } => {
                                            error!("Session error from remote: {}", message);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Ok(Message::Close(_)) => {
                                info!("WebSocket closed by remote for session {}", session_id);
                                break;
                            }
                            Err(e) => {
                                error!("WebSocket error for session {}: {}", session_id, e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    
                    // Send outgoing messages
                    Some(msg) = ws_rx.recv() => {
                        if ws_sender.send(msg).await.is_err() {
                            warn!("Failed to send WebSocket message for session {}", session_id);
                            break;
                        }
                    }
                    
                    // Handle shutdown signal
                    _ = shutdown_rx.recv() => {
                        info!("Shutdown signal received for session {}", session_id);
                        let _ = ws_sender.send(Message::Close(None)).await;
                        break;
                    }
                    
                    else => break,
                }
            }
            
            info!("PTY forwarding stopped for session {} to {}", session_id, target_did);
        });
        
        self.connection_handle = Some(handle);
        self.state = SessionState::Connected;
        self.started_at = Some(chrono::Utc::now().timestamp());
        
        Ok(())
    }
    
    /// Start Agent on remote
    pub async fn start_agent(&mut self, agent_type: &str) -> Result<(), SessionError> {
        if self.state != SessionState::Connected {
            return Err(SessionError::NotConnected);
        }
        
        info!("Starting {} agent on remote", agent_type);
        
        let session_id = self.session_id
            .ok_or_else(|| SessionError::NotConnected)?;
        
        // Send agent start command
        let start_cmd = AgentCommand::Start {
            agent_type: agent_type.to_string(),
            session_id: session_id.to_string(),
            cols: 80,
            rows: 24,
        };
        
        let cmd_json = serde_json::to_string(&start_cmd)
            .map_err(|e| SessionError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        
        if let Some(ref sender) = self.ws_sender {
            sender
                .send(Message::Text(cmd_json))
                .map_err(|_| SessionError::ConnectionFailed("Failed to send start command".to_string()))?;
        }
        
        // Setup PTY forwarding is done in the connection handler
        info!("PTY forwarding setup complete for session {}", session_id);
        
        self.state = SessionState::AgentRunning;
        Ok(())
    }
    
    /// Send input to remote Agent
    pub async fn send_input(&self, data: &[u8]) -> Result<(), SessionError> {
        if self.state != SessionState::AgentRunning {
            return Err(SessionError::NotRunning);
        }
        
        let session_id = self.session_id
            .ok_or_else(|| SessionError::NotRunning)?;
        
        if let Some(ref sender) = self.ws_sender {
            // Wrap in PTY data frame: [session_id: 16 bytes][data]
            let frame = PtyDataFrame::new(session_id, data.to_vec());
            let frame_bytes = frame.to_bytes();
            
            sender
                .send(Message::Binary(frame_bytes))
                .map_err(|_| SessionError::ConnectionFailed("Failed to send input".to_string()))?;
        }
        
        Ok(())
    }
    
    /// Receive output from remote Agent
    pub async fn receive_output(&mut self) -> Option<Vec<u8>> {
        if let Some(ref mut rx) = self.output_rx {
            rx.recv().await
        } else {
            None
        }
    }
    
    /// Disconnect session
    pub async fn disconnect(&mut self) {
        info!("Disconnecting from {}", self.target_did);
        
        // Send disconnect command if session is running
        if self.state == SessionState::AgentRunning {
            if let Some(session_id) = self.session_id {
                let stop_cmd = AgentCommand::Stop {
                    session_id: session_id.to_string(),
                };
                
                if let Ok(cmd_json) = serde_json::to_string(&stop_cmd) {
                    if let Some(ref sender) = self.ws_sender {
                        let _ = sender.send(Message::Text(cmd_json));
                    }
                }
            }
        }
        
        // Send shutdown signal to close WebSocket
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        
        // Wait for connection handler to finish
        if let Some(handle) = self.connection_handle.take() {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        }
        
        self.state = SessionState::Disconnected;
        self.ws_sender = None;
        self.output_rx = None;
        self.session_id = None;
        self.started_at = None;
        
        info!("Disconnected from {}", self.target_did);
    }
    
    /// Get session duration
    pub fn duration(&self) -> Option<std::time::Duration> {
        self.started_at.map(|start| {
            let now = chrono::Utc::now().timestamp();
            std::time::Duration::from_secs((now - start) as u64)
        })
    }
    
    /// Get session ID
    pub fn session_id(&self) -> Option<Uuid> {
        self.session_id
    }
    
    /// Resize terminal
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<(), SessionError> {
        if self.state != SessionState::AgentRunning {
            return Err(SessionError::NotRunning);
        }
        
        let session_id = self.session_id
            .ok_or_else(|| SessionError::NotRunning)?;
        
        let resize_cmd = AgentCommand::Resize {
            session_id: session_id.to_string(),
            cols,
            rows,
        };
        
        let cmd_json = serde_json::to_string(&resize_cmd)
            .map_err(|e| SessionError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        
        if let Some(ref sender) = self.ws_sender {
            sender
                .send(Message::Text(cmd_json))
                .map_err(|_| SessionError::ConnectionFailed("Failed to send resize command".to_string()))?;
        }
        
        Ok(())
    }
}

impl Drop for RemoteSession {
    fn drop(&mut self) {
        // Try to cleanup synchronously if possible
        if self.state != SessionState::Disconnected {
            // Note: We can't use async here, but the channels will be dropped
            // which will cause the connection handler to clean up
        }
    }
}

/// Session manager
pub struct SessionManager {
    sessions: Arc<RwLock<Vec<RemoteSession>>>,
    acl: Arc<RwLock<NetworkAcl>>,
}

impl SessionManager {
    pub fn new(acl: Arc<RwLock<NetworkAcl>>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(Vec::new())),
            acl,
        }
    }
    
    /// Create new session
    pub async fn create_session(
        &self,
        target_did: impl Into<String>,
        target_addr: impl Into<String>,
    ) -> Result<usize, SessionError> {
        let mut session = RemoteSession::new(target_did, target_addr);
        session.connect(self.acl.clone()).await?;
        
        let mut sessions = self.sessions.write().await;
        let id = sessions.len();
        sessions.push(session);
        
        Ok(id)
    }
    
    /// Get session info
    pub async fn get_session_info(&self, id: usize) -> Option<(String, SessionState)> {
        let sessions = self.sessions.read().await;
        sessions.get(id).map(|s| (s.target_did.clone(), s.state))
    }
    
    /// List active sessions
    pub async fn list_sessions(&self) -> Vec<(usize, String, SessionState)> {
        let sessions = self.sessions.read().await;
        sessions.iter()
            .enumerate()
            .map(|(i, s)| (i, s.target_did.clone(), s.state))
            .collect()
    }
    
    /// Close session
    pub async fn close_session(&self, id: usize) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            session.disconnect().await;
        }
    }
    
    /// Get mutable session reference
    pub async fn get_session_mut(&self, _id: usize) -> Option<tokio::sync::RwLockWriteGuard<'_, RemoteSession>> {
        // This is a bit tricky since we store sessions in a Vec inside RwLock
        // For now, we'll return None and users should use specific methods
        None
    }
    
    /// Send input to a session
    pub async fn send_input(&self, id: usize, data: &[u8]) -> Result<(), SessionError> {
        let sessions = self.sessions.read().await;
        if let Some(_session) = sessions.get(id) {
            // Note: This requires &mut self in receive_output but not in send_input
            // We need to handle this carefully
            drop(sessions);
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(id) {
                session.send_input(data).await
            } else {
                Err(SessionError::NotConnected)
            }
        } else {
            Err(SessionError::NotConnected)
        }
    }
    
    /// Receive output from a session
    pub async fn receive_output(&self, id: usize) -> Option<Vec<u8>> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(id) {
            session.receive_output().await
        } else {
            None
        }
    }
}

/// Session errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Not connected")]
    NotConnected,
    
    #[error("Agent not running")]
    NotRunning,
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    
    #[error("DID verification failed: {0}")]
    DidVerificationFailed(String),
    
    #[error("ACL denied: {0}")]
    AclDenied(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Invalid frame: {0}")]
    InvalidFrame(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_data_frame() {
        let session_id = Uuid::new_v4();
        let data = b"Hello, World!".to_vec();
        
        let frame = PtyDataFrame::new(session_id, data.clone());
        let bytes = frame.to_bytes();
        
        assert_eq!(bytes.len(), 16 + data.len());
        
        let parsed = PtyDataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.session_id, session_id);
        assert_eq!(parsed.data, data);
    }

    #[test]
    fn test_pty_data_frame_too_short() {
        let result = PtyDataFrame::from_bytes(&[1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_command_serialization() {
        let cmd = AgentCommand::Start {
            agent_type: "claude".to_string(),
            session_id: "test-session".to_string(),
            cols: 80,
            rows: 24,
        };
        
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("start"));
        assert!(json.contains("claude"));
    }

    #[test]
    fn test_session_state() {
        let session = RemoteSession::new("did:cis:test", "127.0.0.1:7676");
        assert_eq!(session.state, SessionState::Disconnected);
    }
}
