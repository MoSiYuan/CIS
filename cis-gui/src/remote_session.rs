//! # Remote Agent Session
//!
//! Manages remote Agent connections over Matrix Federation.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

use cis_core::network::NetworkAcl;

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

/// Remote Agent session
pub struct RemoteSession {
    /// Target node DID
    pub target_did: String,
    
    /// Target address
    pub target_addr: String,
    
    /// Session state
    pub state: SessionState,
    
    /// WebSocket sender
    ws_sender: Option<mpsc::UnboundedSender<String>>,
    
    /// Terminal output receiver
    output_rx: Option<mpsc::Receiver<Vec<u8>>>,
    
    /// Session started at
    started_at: Option<i64>,
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
        }
    }
    
    /// Connect to remote node
    pub async fn connect(&mut self, _acl: Arc<RwLock<NetworkAcl>>) -> Result<(), SessionError> {
        info!("Connecting to {} at {}", self.target_did, self.target_addr);
        self.state = SessionState::Connecting;
        
        // TODO: Establish WebSocket connection to target
        // TODO: Perform DID challenge/response
        // TODO: Spawn Agent process on remote
        
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
        
        // TODO: Send agent start command
        // TODO: Setup PTY forwarding
        
        self.state = SessionState::AgentRunning;
        Ok(())
    }
    
    /// Send input to remote Agent
    pub async fn send_input(&self, data: &[u8]) -> Result<(), SessionError> {
        if self.state != SessionState::AgentRunning {
            return Err(SessionError::NotRunning);
        }
        
        if let Some(ref sender) = self.ws_sender {
            // TODO: Wrap in PTY data frame
            let _ = sender.send(String::from_utf8_lossy(data).to_string());
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
        
        // TODO: Send disconnect
        // TODO: Close WebSocket
        
        self.state = SessionState::Disconnected;
        self.ws_sender = None;
        self.output_rx = None;
    }
    
    /// Get session duration
    pub fn duration(&self) -> Option<std::time::Duration> {
        self.started_at.map(|start| {
            let now = chrono::Utc::now().timestamp();
            std::time::Duration::from_secs((now - start) as u64)
        })
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
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
