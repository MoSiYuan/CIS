//! # Remote Agent Session Management
//!
//! Provides remote Agent session functionality over Matrix WebSocket (port 6767).
//!
//! ## Features
//!
//! - Spawn local Agent (claude/kimi) via PTY
//! - Forward PTY I/O through WebSocket binary frames
//! - Support multiple concurrent sessions
//! - Sandbox/scope restriction based on project
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     Agent Session Manager                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
//! │  │ AgentSession │  │ AgentSession │  │ AgentSession │  ...      │
//! │  │   (UUID)     │  │   (UUID)     │  │   (UUID)     │           │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │
//! │         │                 │                 │                   │
//! │    ┌────┴────┐       ┌────┴────┐       ┌────┴────┐              │
//! │    │  PTY    │       │  PTY    │       │  PTY    │              │
//! │    │ Claude  │       │  Kimi   │       │  Shell  │              │
//! │    └────┬────┘       └────┬────┘       └────┬────┘              │
//! │         │                 │                 │                   │
//! │         └─────────────────┴─────────────────┘                   │
//! │                           │                                     │
//! │                    ┌──────┴──────┐                             │
//! │                    │ PtyForwarder│                             │
//! │                    │ (WebSocket) │                             │
//! │                    └─────────────┘                             │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Protocol
//!
//! ### Control Messages (JSON)
//! - `session_start`: Initiate new session
//! - `session_end`: Terminate session
//! - `resize`: Resize terminal
//!
//! ### Data Messages (Binary)
//! Format: `[session_id: 16 bytes][data: variable]`

use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::agent::AgentType;
use crate::error::{CisError, Result};
use crate::identity::DIDManager;
use crate::network::acl::{AclResult, NetworkAcl};
use crate::sandbox::SandboxConfig;

/// Default WebSocket port for agent sessions
pub const AGENT_SESSION_PORT: u16 = 6767;

/// Default terminal size
const DEFAULT_TERMINAL_COLS: u16 = 80;
const DEFAULT_TERMINAL_ROWS: u16 = 24;

/// Session ID type (UUID)
pub type SessionId = Uuid;

/// Control messages for agent session protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionControlMessage {
    /// Start a new session
    SessionStart {
        /// Target DID to connect to
        target_did: String,
        /// Agent type to spawn
        agent_type: AgentType,
        /// Project path for sandbox scope
        project_path: Option<PathBuf>,
        /// Initial terminal columns
        cols: Option<u16>,
        /// Initial terminal rows
        rows: Option<u16>,
    },
    /// End a session
    SessionEnd {
        /// Session ID to terminate
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
    /// Session started acknowledgment
    SessionStarted {
        /// Assigned session ID
        session_id: String,
        /// Status message
        message: String,
    },
    /// Error response
    Error {
        /// Error message
        message: String,
    },
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Initial state
    Initial,
    /// Connecting to target
    Connecting,
    /// Session is active
    Active,
    /// Session is closing
    Closing,
    /// Session is closed
    Closed,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Initial => write!(f, "initial"),
            SessionState::Connecting => write!(f, "connecting"),
            SessionState::Active => write!(f, "active"),
            SessionState::Closing => write!(f, "closing"),
            SessionState::Closed => write!(f, "closed"),
        }
    }
}

/// Information about an agent session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session ID
    pub id: SessionId,
    /// Session state
    pub state: SessionState,
    /// Agent type
    pub agent_type: AgentType,
    /// Target DID
    pub target_did: String,
    /// Project path (for sandbox scope)
    pub project_path: Option<PathBuf>,
    /// Terminal size
    pub terminal_size: (u16, u16),
    /// Session creation timestamp
    pub created_at: i64,
    /// Last activity timestamp
    pub last_activity: i64,
}

impl SessionInfo {
    /// Create new session info
    fn new(id: SessionId, agent_type: AgentType, target_did: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id,
            state: SessionState::Initial,
            agent_type,
            target_did,
            project_path: None,
            terminal_size: (DEFAULT_TERMINAL_COLS, DEFAULT_TERMINAL_ROWS),
            created_at: now,
            last_activity: now,
        }
    }

    /// Update last activity timestamp
    fn touch(&mut self) {
        self.last_activity = chrono::Utc::now().timestamp();
    }
}

/// A single agent session
///
/// Manages a PTY with a local Agent process and forwards I/O to WebSocket
pub struct AgentSession {
    /// Session ID
    id: SessionId,
    /// Session information
    info: Arc<RwLock<SessionInfo>>,
    /// Agent process handle
    process_handle: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    /// Output channel receiver (from PTY reader thread)
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    /// Input channel sender (to PTY writer)
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// Forwarder thread handle
    forwarder_handle: Option<JoinHandle<()>>,
    /// Sandbox configuration
    sandbox_config: SandboxConfig,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl AgentSession {
    /// Create a new agent session
    pub fn new(
        agent_type: AgentType,
        target_did: String,
        project_path: Option<PathBuf>,
        sandbox_config: SandboxConfig,
    ) -> Self {
        let id = Uuid::new_v4();
        let mut info = SessionInfo::new(id, agent_type, target_did);
        info.project_path = project_path.clone();

        Self {
            id,
            info: Arc::new(RwLock::new(info)),
            process_handle: None,
            output_rx: None,
            input_tx: None,
            forwarder_handle: None,
            sandbox_config,
            shutdown_tx: None,
        }
    }

    /// Get session ID
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// Get session info
    pub async fn info(&self) -> SessionInfo {
        self.info.read().await.clone()
    }

    /// Start the agent session
    ///
    /// Spawns the agent in a PTY and starts the I/O forwarder
    pub async fn start(&mut self, cols: u16, rows: u16) -> Result<()> {
        let agent_type = self.info.read().await.agent_type;

        info!(
            "Starting agent session {} with agent type {:?}",
            self.id, agent_type
        );

        // Update state
        {
            let mut info = self.info.write().await;
            info.state = SessionState::Connecting;
            info.terminal_size = (cols, rows);
        }

        // Create PTY and spawn agent
        self.spawn_agent_in_pty(agent_type, cols, rows).await?;

        // Update state
        {
            let mut info = self.info.write().await;
            info.state = SessionState::Active;
            info.touch();
        }

        info!("Agent session {} started successfully", self.id);
        Ok(())
    }

    /// Spawn agent process in PTY
    async fn spawn_agent_in_pty(&mut self, agent_type: AgentType, cols: u16, rows: u16) -> Result<()> {
        let pty_system = NativePtySystem::default();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| CisError::execution(format!("Failed to open PTY: {}", e)))?;

        // Get command for agent type
        let cmd = self.build_agent_command(agent_type)?;

        info!("Spawning agent process: {:?}", cmd);

        // Spawn process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| CisError::execution(format!("Failed to spawn agent: {}", e)))?;

        self.process_handle = Some(child);

        // Take ownership of master PTY for I/O
        let master = pair.master;
        
        // Create channels for PTY I/O
        let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        
        self.output_rx = Some(output_rx);
        self.input_tx = Some(input_tx);

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let session_id = self.id;
        let info = self.info.clone();

        // Spawn reader/writer thread
        let handle = tokio::task::spawn_blocking(move || {
            info!("PTY I/O thread started for session {}", session_id);

            // Get writer for sending input to PTY
            let mut writer = master.take_writer().ok();
            
            // Get reader for receiving output from PTY
            let mut reader = master.try_clone_reader().ok();

            let mut buf = vec![0u8; 4096];

            loop {
                // Check for shutdown signal (non-blocking)
                match shutdown_rx.try_recv() {
                    Ok(_) | Err(mpsc::error::TryRecvError::Disconnected) => {
                        info!("PTY I/O thread shutting down for session {}", session_id);
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
                }

                // Read from PTY if reader is available
                if let Some(ref mut r) = reader {
                    match r.read(&mut buf) {
                        Ok(n) => {
                            if n > 0 {
                                let data = buf[..n].to_vec();
                                if output_tx.send(data).is_err() {
                                    warn!("PTY output channel closed for session {}", session_id);
                                    break;
                                }
                                // Update activity
                                if let Ok(mut info) = info.try_write() {
                                    info.touch();
                                }
                            } else {
                                debug!("PTY EOF for session {}", session_id);
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available
                        }
                        Err(e) => {
                            warn!("PTY read error for session {}: {}", session_id, e);
                            break;
                        }
                    }
                }

                // Write to PTY if writer is available
                if let Some(ref mut w) = writer {
                    match input_rx.try_recv() {
                        Ok(data) => {
                            if let Err(e) = w.write_all(&data) {
                                warn!("PTY write error for session {}: {}", session_id, e);
                                break;
                            }
                            if let Err(e) = w.flush() {
                                warn!("PTY flush error for session {}: {}", session_id, e);
                                break;
                            }
                            if let Ok(mut info) = info.try_write() {
                                info.touch();
                            }
                        }
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            debug!("PTY input channel closed for session {}", session_id);
                            break;
                        }
                        Err(mpsc::error::TryRecvError::Empty) => {}
                    }
                }

                // Small sleep to prevent busy-waiting
                std::thread::sleep(std::time::Duration::from_millis(5));
            }

            info!("PTY I/O thread stopped for session {}", session_id);
        });

        self.forwarder_handle = Some(handle);
        Ok(())
    }

    /// Build command for agent type
    fn build_agent_command(&self, agent_type: AgentType) -> Result<CommandBuilder> {
        let (cmd_name, project_path) = match agent_type {
            AgentType::Claude => {
                let path = self.info.blocking_read().project_path.clone();
                ("claude".to_string(), path)
            }
            AgentType::Kimi => {
                let path = self.info.blocking_read().project_path.clone();
                ("kimi".to_string(), path)
            }
            AgentType::Aider => {
                let path = self.info.blocking_read().project_path.clone();
                ("aider".to_string(), path)
            }
            AgentType::OpenCode => {
                let path = self.info.blocking_read().project_path.clone();
                ("opencode".to_string(), path)
            }
            AgentType::Custom => {
                return Err(CisError::configuration(
                    "Custom agent type not supported for remote sessions",
                ));
            }
        };

        let mut cmd = CommandBuilder::new(cmd_name);
        
        // Set working directory if project path is specified
        if let Some(path) = project_path {
            cmd.cwd(path.clone());
            cmd.env("CIS_PROJECT_PATH", path.to_string_lossy().as_ref());
        }
        
        // Set environment variables for sandbox scope
        cmd.env("CIS_SANDBOX_STRICT", "true");

        Ok(cmd)
    }

    /// Send data to PTY (from WebSocket)
    pub fn send_to_pty(&self, data: Vec<u8>) -> Result<()> {
        if let Some(ref tx) = self.input_tx {
            tx.send(data)
                .map_err(|_| CisError::execution("PTY input channel closed"))?;
            Ok(())
        } else {
            Err(CisError::execution("PTY not initialized"))
        }
    }

    /// Try to receive data from PTY (to WebSocket) - non-blocking
    pub fn try_receive_from_pty(&mut self) -> Option<Vec<u8>> {
        if let Some(ref mut rx) = self.output_rx {
            match rx.try_recv() {
                Ok(data) => Some(data),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Resize terminal
    ///
    /// Note: portable-pty doesn't support resizing after creation,
    /// so this is a placeholder for future implementation.
    pub async fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let mut info = self.info.write().await;
        info.terminal_size = (cols, rows);
        info.touch();

        debug!("Terminal resize requested for session {} to {}x{} (not implemented)", self.id, cols, rows);
        Ok(())
    }

    /// Shutdown the session
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down agent session {}", self.id);

        // Update state
        {
            let mut info = self.info.write().await;
            info.state = SessionState::Closing;
        }

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // Wait for forwarder to stop
        if let Some(handle) = self.forwarder_handle.take() {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        }

        // Kill process
        if let Some(mut handle) = self.process_handle.take() {
            let _ = handle.kill();
        }

        // Drop channels
        self.output_rx = None;
        self.input_tx = None;

        // Update state
        {
            let mut info = self.info.write().await;
            info.state = SessionState::Closed;
        }

        info!("Agent session {} shut down", self.id);
        Ok(())
    }

    /// Check if session is active
    pub async fn is_active(&self) -> bool {
        let info = self.info.read().await;
        info.state == SessionState::Active
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        // Try to clean up resources synchronously
        if let Some(mut handle) = self.process_handle.take() {
            let _ = handle.kill();
        }
    }
}

/// Manages all active agent sessions
pub struct SessionManager {
    /// Active sessions
    sessions: Arc<RwLock<HashMap<SessionId, Arc<RwLock<AgentSession>>>>>,
    /// Network ACL for verification
    acl: Arc<RwLock<NetworkAcl>>,
    /// DID manager
    did_manager: Arc<DIDManager>,
    /// Default sandbox config
    default_sandbox: SandboxConfig,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(
        acl: Arc<RwLock<NetworkAcl>>,
        did_manager: Arc<DIDManager>,
        default_sandbox: SandboxConfig,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            acl,
            did_manager,
            default_sandbox,
        }
    }

    /// Create a new session
    ///
    /// Checks if target is verified and in whitelist before creating
    pub async fn create_session(
        &self,
        target_did: &str,
        agent_type: AgentType,
        project_path: Option<PathBuf>,
    ) -> Result<SessionId> {
        // Verify target is allowed
        self.verify_target(target_did).await?;

        // Create sandbox config based on project
        let sandbox_config = self.build_sandbox_config(project_path.as_ref()).await?;

        // Create session
        let session = AgentSession::new(
            agent_type,
            target_did.to_string(),
            project_path,
            sandbox_config,
        );
        let session_id = session.id();

        // Store session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, Arc::new(RwLock::new(session)));
        }

        info!("Created agent session {} for target {}", session_id, target_did);
        Ok(session_id)
    }

    /// Start a session
    pub async fn start_session(
        &self,
        session_id: SessionId,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        let session_arc = {
            let sessions = self.sessions.read().await;
            sessions
                .get(&session_id)
                .cloned()
                .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?
        };

        let mut session = session_arc.write().await;
        session.start(cols, rows).await
    }

    /// Verify target DID is allowed
    async fn verify_target(&self, target_did: &str) -> Result<()> {
        // Check ACL
        let acl = self.acl.read().await;
        match acl.check_did(target_did) {
            AclResult::Allowed => {
                debug!("Target DID {} is allowed", target_did);
                Ok(())
            }
            AclResult::Denied(reason) => {
                warn!("Target DID {} denied by ACL: {}", target_did, reason);
                Err(CisError::network(format!(
                    "Target not in whitelist: {}",
                    reason
                )))
            }
            AclResult::Quarantine => {
                warn!("Target DID {} is quarantined", target_did);
                Err(CisError::network("Target is quarantined".to_string()))
            }
        }
    }

    /// Build sandbox config for project
    async fn build_sandbox_config(
        &self,
        project_path: Option<&PathBuf>,
    ) -> Result<SandboxConfig> {
        let mut config = self.default_sandbox.clone();

        if let Some(path) = project_path {
            // Add project directory to whitelist
            config.add_allowed_dir(path.clone());

            // Also add common subdirectories
            config.add_allowed_dir(path.join(".cis"));
            config.add_allowed_dir(path.join("src"));
        }

        Ok(config)
    }

    /// Get session
    pub async fn get_session(
        &self,
        session_id: SessionId,
    ) -> Option<Arc<RwLock<AgentSession>>> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).cloned()
    }

    /// Send data to session PTY
    pub async fn send_to_session(&self, session_id: SessionId, data: Vec<u8>) -> Result<()> {
        if let Some(session_arc) = self.get_session(session_id).await {
            let session = session_arc.read().await;
            session.send_to_pty(data)
        } else {
            Err(CisError::not_found(format!("Session {} not found", session_id)))
        }
    }

    /// Try to receive data from session PTY (non-blocking)
    pub async fn try_receive_from_session(&self, session_id: SessionId) -> Result<Option<Vec<u8>>> {
        if let Some(session_arc) = self.get_session(session_id).await {
            let mut session = session_arc.write().await;
            Ok(session.try_receive_from_pty())
        } else {
            Err(CisError::not_found(format!("Session {} not found", session_id)))
        }
    }

    /// Resize session terminal
    pub async fn resize_session(&self, session_id: SessionId, cols: u16, rows: u16) -> Result<()> {
        if let Some(session_arc) = self.get_session(session_id).await {
            let mut session = session_arc.write().await;
            session.resize(cols, rows).await
        } else {
            Err(CisError::not_found(format!("Session {} not found", session_id)))
        }
    }

    /// End a session
    pub async fn end_session(&self, session_id: SessionId) -> Result<()> {
        // Remove from sessions first
        let session_arc = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&session_id)
        };

        if let Some(session_arc) = session_arc {
            let mut session = session_arc.write().await;
            session.shutdown().await
        } else {
            Err(CisError::not_found(format!("Session {} not found", session_id)))
        }
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        let mut infos = Vec::new();

        for (_, session_arc) in sessions.iter() {
            let session = session_arc.read().await;
            infos.push(session.info().await);
        }

        infos
    }

    /// Get session info
    pub async fn get_session_info(&self, session_id: SessionId) -> Option<SessionInfo> {
        if let Some(session_arc) = self.get_session(session_id).await {
            let session = session_arc.read().await;
            Some(session.info().await)
        } else {
            None
        }
    }

    /// Clean up inactive sessions
    pub async fn cleanup_inactive(&self, max_inactive_secs: i64) -> usize {
        let now = chrono::Utc::now().timestamp();
        let mut to_remove = Vec::new();

        {
            let sessions = self.sessions.read().await;
            for (id, session_arc) in sessions.iter() {
                let session = session_arc.read().await;
                let info = session.info().await;
                if now - info.last_activity > max_inactive_secs {
                    to_remove.push(*id);
                }
            }
        }

        let count = to_remove.len();
        for id in to_remove {
            let _ = self.end_session(id).await;
        }

        count
    }

    /// Shutdown all sessions
    pub async fn shutdown_all(&self) {
        let session_ids: Vec<SessionId> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };

        for id in session_ids {
            let _ = self.end_session(id).await;
        }
    }
}

/// Agent session server
///
/// Accepts WebSocket connections for remote agent sessions
pub struct AgentSessionServer {
    /// Bind address
    bind_address: String,
    /// Port to listen on
    port: u16,
    /// Session manager
    session_manager: Arc<SessionManager>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl AgentSessionServer {
    /// Create new server
    pub fn new(
        bind_address: impl Into<String>,
        port: u16,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            bind_address: bind_address.into(),
            port,
            session_manager,
            shutdown_tx: None,
        }
    }

    /// Run the server
    pub async fn run(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.bind_address, self.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| CisError::network(format!("Failed to bind: {}", e)))?;

        info!("Agent session server listening on ws://{}", addr);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            let session_manager = self.session_manager.clone();
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(stream, peer_addr, session_manager).await {
                                    error!("Connection error from {}: {}", peer_addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Agent session server shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle incoming connection
    async fn handle_connection(
        stream: TcpStream,
        peer_addr: std::net::SocketAddr,
        session_manager: Arc<SessionManager>,
    ) -> Result<()> {
        info!("New agent session connection from {}", peer_addr);

        // Accept WebSocket
        let ws_stream = accept_async(stream)
            .await
            .map_err(|e| CisError::network(format!("WebSocket handshake failed: {}", e)))?;

        // Handle the WebSocket connection
        handle_websocket_connection(ws_stream, session_manager).await
    }

    /// Shutdown the server
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
        // End all sessions
        self.session_manager.shutdown_all().await;
    }
}

/// Handle a WebSocket connection for agent sessions
async fn handle_websocket_connection(
    ws_stream: WebSocketStream<TcpStream>,
    session_manager: Arc<SessionManager>,
) -> Result<()> {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut current_session: Option<SessionId> = None;
    
    // Channel for PTY output to be sent to WebSocket
    let (pty_out_tx, mut pty_out_rx) = mpsc::unbounded_channel::<(SessionId, Vec<u8>)>();

    info!("Agent session WebSocket handler started");

    loop {
        tokio::select! {
            // Receive from WebSocket
            Some(msg_result) = ws_receiver.next() => {
                match msg_result {
                    Ok(msg) => {
                        match handle_ws_message(
                            msg,
                            &session_manager,
                            &mut ws_sender,
                            &mut current_session,
                            &pty_out_tx,
                        ).await {
                            Ok(should_continue) => {
                                if !should_continue {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Error handling WebSocket message: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }

            // Send PTY output to WebSocket
            Some((session_id, data)) = pty_out_rx.recv() => {
                // Build binary frame: [session_id: 16 bytes][data]
                let mut frame = BytesMut::with_capacity(16 + data.len());
                frame.extend_from_slice(session_id.as_bytes());
                frame.extend_from_slice(&data);

                if let Err(e) = ws_sender.send(Message::Binary(frame.to_vec())).await {
                    warn!("Failed to send PTY output: {}", e);
                    break;
                }
            }

            else => break,
        }
    }

    // Cleanup
    if let Some(session_id) = current_session {
        let _ = session_manager.end_session(session_id).await;
    }

    info!("Agent session WebSocket handler stopped");
    Ok(())
}

/// Handle a WebSocket message
/// Returns Ok(true) to continue, Ok(false) to stop, Err on error
async fn handle_ws_message(
    msg: Message,
    session_manager: &Arc<SessionManager>,
    ws_sender: &mut futures::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    current_session: &mut Option<SessionId>,
    pty_out_tx: &mpsc::UnboundedSender<(SessionId, Vec<u8>)>,
) -> Result<bool> {
    match msg {
        Message::Text(text) => {
            // Parse as control message
            let control: SessionControlMessage =
                serde_json::from_str(&text).map_err(|e| {
                    CisError::serialization(format!("Invalid control message: {}", e))
                })?;
            handle_control_message(
                control,
                session_manager,
                ws_sender,
                current_session,
                pty_out_tx,
            ).await
        }
        Message::Binary(data) => {
            // Parse binary frame: [session_id: 16 bytes][data]
            if data.len() < 16 {
                return Err(CisError::invalid_input(
                    "Binary frame too short (need at least 16 bytes for session ID)",
                ));
            }

            let session_id = Uuid::from_slice(&data[..16])
                .map_err(|e| CisError::invalid_input(format!("Invalid session ID: {}", e)))?;

            let payload = data[16..].to_vec();

            // Forward to PTY
            session_manager
                .send_to_session(session_id, payload)
                .await?;
            
            Ok(true)
        }
        Message::Ping(_) => {
            // Pong is automatic
            Ok(true)
        }
        Message::Pong(_) => Ok(true),
        Message::Close(_) => {
            info!("WebSocket closed by peer");
            Ok(false)
        }
        Message::Frame(_) => Ok(true),
    }
}

/// Handle a control message
/// Returns Ok(true) to continue, Ok(false) to stop, Err on error
async fn handle_control_message(
    msg: SessionControlMessage,
    session_manager: &Arc<SessionManager>,
    ws_sender: &mut futures::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    current_session: &mut Option<SessionId>,
    pty_out_tx: &mpsc::UnboundedSender<(SessionId, Vec<u8>)>,
) -> Result<bool> {
    match msg {
        SessionControlMessage::SessionStart {
            target_did,
            agent_type,
            project_path,
            cols,
            rows,
        } => {
            // End current session if any
            if let Some(session_id) = current_session.take() {
                let _ = session_manager.end_session(session_id).await;
            }

            // Create new session
            let session_id = session_manager
                .create_session(&target_did, agent_type, project_path)
                .await?;

            // Start session with terminal size
            let cols = cols.unwrap_or(DEFAULT_TERMINAL_COLS);
            let rows = rows.unwrap_or(DEFAULT_TERMINAL_ROWS);
            session_manager.start_session(session_id, cols, rows).await?;

            *current_session = Some(session_id);

            // Send acknowledgment
            let ack = SessionControlMessage::SessionStarted {
                session_id: session_id.to_string(),
                message: format!("Session started with {:?}", agent_type),
            };
            let ack_json = serde_json::to_string(&ack)?;
            ws_sender
                .send(Message::Text(ack_json))
                .await
                .map_err(|e| CisError::network(format!("Failed to send ack: {}", e)))?;

            // Start PTY output polling task
            start_pty_output_polling(session_id, session_manager.clone(), pty_out_tx.clone());

            Ok(true)
        }
        SessionControlMessage::SessionEnd { session_id } => {
            let id = Uuid::parse_str(&session_id)
                .map_err(|e| CisError::invalid_input(format!("Invalid session ID: {}", e)))?;

            if *current_session == Some(id) {
                *current_session = None;
            }

            session_manager.end_session(id).await?;
            Ok(true)
        }
        SessionControlMessage::Resize {
            session_id,
            cols,
            rows,
        } => {
            let id = Uuid::parse_str(&session_id)
                .map_err(|e| CisError::invalid_input(format!("Invalid session ID: {}", e)))?;

            session_manager.resize_session(id, cols, rows).await?;
            Ok(true)
        }
        _ => {
            // Ignore other messages (they're responses)
            Ok(true)
        }
    }
}

/// Start PTY output polling task
fn start_pty_output_polling(
    session_id: SessionId,
    session_manager: Arc<SessionManager>,
    pty_out_tx: mpsc::UnboundedSender<(SessionId, Vec<u8>)>,
) {
    tokio::spawn(async move {
        loop {
            // Check if session is still active
            match session_manager.get_session_info(session_id).await {
                Some(info) => {
                    if info.state != SessionState::Active {
                        break;
                    }
                }
                None => break,
            }

            // Try to receive output
            match session_manager.try_receive_from_session(session_id).await {
                Ok(Some(data)) => {
                    if pty_out_tx.send((session_id, data)).is_err() {
                        break;
                    }
                }
                Ok(None) => {
                    // No data available, sleep briefly
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_control_message_serialization() {
        let msg = SessionControlMessage::SessionStart {
            target_did: "did:cis:target:abc123".to_string(),
            agent_type: AgentType::Claude,
            project_path: Some(PathBuf::from("/workspace")),
            cols: Some(120),
            rows: Some(40),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("session_start"));
        assert!(json.contains("did:cis:target:abc123"));

        let decoded: SessionControlMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            SessionControlMessage::SessionStart { target_did, .. } => {
                assert_eq!(target_did, "did:cis:target:abc123");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_binary_frame_format() {
        let session_id = Uuid::new_v4();
        let data = b"Hello, PTY!";

        let mut frame = BytesMut::with_capacity(16 + data.len());
        frame.extend_from_slice(session_id.as_bytes());
        frame.extend_from_slice(data);

        assert_eq!(frame.len(), 16 + data.len());

        // Parse back
        let parsed_id = Uuid::from_slice(&frame[..16]).unwrap();
        let parsed_data = &frame[16..];

        assert_eq!(parsed_id, session_id);
        assert_eq!(parsed_data, data);
    }
}
