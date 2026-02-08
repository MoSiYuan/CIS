//! # Enhanced Session Manager
//!
//! Provides multi-session management with persistence and agent multiplexing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agent::AgentType;
use crate::error::{CisError, Result};
use crate::network::agent_session::{SessionId, SessionState};

/// Default session timeout (1 hour of inactivity)
const DEFAULT_SESSION_TIMEOUT: Duration = Duration::from_secs(3600);

/// Default checkpoint interval (5 minutes)
const DEFAULT_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(300);

/// Session metadata for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSession {
    /// Session ID
    pub id: String,
    /// Target DID
    pub target_did: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Project path
    pub project_path: Option<PathBuf>,
    /// Terminal size
    pub terminal_size: (u16, u16),
    /// Session state
    pub state: SessionState,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    /// Whether this is a resumed session
    pub is_resumed: bool,
    /// Session metadata
    pub metadata: HashMap<String, String>,
}

impl PersistentSession {
    /// Create from session components
    pub fn new(
        id: String,
        target_did: String,
        agent_type: AgentType,
        project_path: Option<PathBuf>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            target_did,
            agent_type,
            project_path,
            terminal_size: (80, 24),
            state: SessionState::Initial,
            created_at: now,
            last_activity: now,
            is_resumed: false,
            metadata: HashMap::new(),
        }
    }

    /// Check if session has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.last_activity);
        elapsed.num_seconds() > timeout.as_secs() as i64
    }
}

/// Session checkpoint for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    /// Session ID
    pub session_id: String,
    /// Checkpoint timestamp
    pub timestamp: DateTime<Utc>,
    /// Terminal scrollback buffer (last N lines)
    pub scrollback: Vec<String>,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Working directory
    pub working_dir: PathBuf,
    /// Command history
    pub command_history: Vec<String>,
}

/// Agent switch event
#[derive(Debug, Clone)]
pub enum AgentSwitchEvent {
    /// Before switching agent
    BeforeSwitch {
        session_id: SessionId,
        from: AgentType,
        to: AgentType,
    },
    /// After switching agent
    AfterSwitch {
        session_id: SessionId,
        new_agent: AgentType,
    },
}

/// Enhanced Session Manager
pub struct EnhancedSessionManager {
    /// Active sessions (in-memory)
    pub sessions: Arc<RwLock<HashMap<String, ManagedSession>>>,
    /// Currently active session ID
    active_session: Arc<RwLock<Option<String>>>,
    /// Session store for persistence
    pub store: Arc<RwLock<SessionStore>>,
    /// Agent switch event sender
    agent_switch_tx: mpsc::Sender<AgentSwitchEvent>,
    /// Checkpoint interval
    checkpoint_interval: Duration,
    /// Session timeout
    session_timeout: Duration,
    /// Cleanup task handle
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Managed session wrapper
#[derive(Debug, Clone)]
pub struct ManagedSession {
    /// Session metadata
    pub metadata: PersistentSession,
    /// Session start time
    pub started_at: Instant,
    /// Last activity
    pub last_activity: Instant,
    /// Agent switch history
    pub agent_history: Vec<(DateTime<Utc>, AgentType)>,
}

impl ManagedSession {
    /// Create new managed session
    pub fn new(metadata: PersistentSession) -> Self {
        let now = Instant::now();
        Self {
            metadata,
            started_at: now,
            last_activity: now,
            agent_history: Vec::new(),
        }
    }

    /// Record activity
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
        self.metadata.last_activity = Utc::now();
    }

    /// Get duration since start
    pub fn duration(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Get idle duration
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

/// Session store for persistence
pub struct SessionStore {
    /// Database connection
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SessionStore {
    /// Create new session store
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = rusqlite::Connection::open(db_path)
            .map_err(|e| CisError::storage(format!("Failed to open session db: {}", e)))?;
        
        // Configure WAL mode
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        ).map_err(|e| CisError::storage(format!("Failed to configure WAL: {}", e)))?;
        
        let conn = Arc::new(Mutex::new(conn));
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Initialize database schema
    fn init_schema(conn: &Arc<Mutex<rusqlite::Connection>>) -> Result<()> {
        let conn = conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                target_did TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                project_path TEXT,
                terminal_cols INTEGER NOT NULL,
                terminal_rows INTEGER NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_activity TEXT NOT NULL,
                is_resumed INTEGER NOT NULL DEFAULT 0,
                metadata TEXT DEFAULT '{}'
            );

            CREATE TABLE IF NOT EXISTS session_checkpoints (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                scrollback TEXT NOT NULL,
                env_vars TEXT NOT NULL,
                working_dir TEXT NOT NULL,
                command_history TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_state ON sessions(state);
            CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON sessions(last_activity);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_session ON session_checkpoints(session_id);
            "#,
        )
        .map_err(|e| CisError::storage(format!("Failed to init session schema: {}", e)))?;

        Ok(())
    }

    /// Save session
    pub fn save_session(&self, session: &PersistentSession) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let agent_type_str = format!("{:?}", session.agent_type);
        let project_path_str = session.project_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let state_str = format!("{:?}", session.state);
        let created_at_str = session.created_at.to_rfc3339();
        let last_activity_str = session.last_activity.to_rfc3339();
        let metadata_str = serde_json::to_string(&session.metadata).unwrap_or_default();
        
        conn.execute(
            r#"
            INSERT INTO sessions (
                id, target_did, agent_type, project_path, terminal_cols, terminal_rows,
                state, created_at, last_activity, is_resumed, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                state = excluded.state,
                last_activity = excluded.last_activity,
                is_resumed = excluded.is_resumed,
                metadata = excluded.metadata
            "#,
            [
                &session.id as &dyn rusqlite::ToSql,
                &session.target_did,
                &agent_type_str,
                &project_path_str,
                &(session.terminal_size.0 as i64),
                &(session.terminal_size.1 as i64),
                &state_str,
                &created_at_str,
                &last_activity_str,
                &(session.is_resumed as i64),
                &metadata_str,
            ],
        )
        .map_err(|e| CisError::storage(format!("Failed to save session: {}", e)))?;

        Ok(())
    }

    /// Load session by ID
    pub fn load_session(&self, session_id: &str) -> Result<Option<PersistentSession>> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, target_did, agent_type, project_path, terminal_cols, terminal_rows,
                       state, created_at, last_activity, is_resumed, metadata
                FROM sessions
                WHERE id = ?1
                "#,
            )
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let session = stmt
            .query_row([session_id], |row| {
                let agent_type_str: String = row.get(2)?;
                let state_str: String = row.get(6)?;
                let metadata_str: String = row.get(10)?;
                let project_path_str: String = row.get(3)?;

                Ok(PersistentSession {
                    id: row.get(0)?,
                    target_did: row.get(1)?,
                    agent_type: Self::parse_agent_type(&agent_type_str),
                    project_path: if project_path_str.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(project_path_str))
                    },
                    terminal_size: (row.get::<_, i64>(4)? as u16, row.get::<_, i64>(5)? as u16),
                    state: Self::parse_session_state(&state_str),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    is_resumed: row.get::<_, i64>(9)? != 0,
                    metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                })
            })
            .optional()
            .map_err(|e| CisError::storage(format!("Failed to load session: {}", e)))?;

        Ok(session)
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<PersistentSession>> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, target_did, agent_type, project_path, terminal_cols, terminal_rows,
                       state, created_at, last_activity, is_resumed, metadata
                FROM sessions
                ORDER BY last_activity DESC
                "#,
            )
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let sessions = stmt
            .query_map([], |row| {
                let agent_type_str: String = row.get(2)?;
                let state_str: String = row.get(6)?;
                let metadata_str: String = row.get(10)?;
                let project_path_str: String = row.get(3)?;

                Ok(PersistentSession {
                    id: row.get(0)?,
                    target_did: row.get(1)?,
                    agent_type: Self::parse_agent_type(&agent_type_str),
                    project_path: if project_path_str.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(project_path_str))
                    },
                    terminal_size: (row.get::<_, i64>(4)? as u16, row.get::<_, i64>(5)? as u16),
                    state: Self::parse_session_state(&state_str),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    is_resumed: row.get::<_, i64>(9)? != 0,
                    metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                })
            })
            .map_err(|e| CisError::storage(format!("Failed to list sessions: {}", e)))?;

        sessions
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| CisError::storage(format!("Failed to collect sessions: {}", e)))
    }

    /// Delete session
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            [session_id],
        )
        .map_err(|e| CisError::storage(format!("Failed to delete session: {}", e)))?;

        Ok(())
    }

    /// Save checkpoint
    pub fn save_checkpoint(&self, checkpoint: &SessionCheckpoint) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let timestamp_str = checkpoint.timestamp.to_rfc3339();
        let scrollback_str = serde_json::to_string(&checkpoint.scrollback).unwrap_or_default();
        let env_vars_str = serde_json::to_string(&checkpoint.env_vars).unwrap_or_default();
        let working_dir_str = checkpoint.working_dir.to_string_lossy().to_string();
        let command_history_str = serde_json::to_string(&checkpoint.command_history).unwrap_or_default();
        
        conn.execute(
            r#"
            INSERT INTO session_checkpoints (
                session_id, timestamp, scrollback, env_vars, working_dir, command_history
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            [
                &checkpoint.session_id as &dyn rusqlite::ToSql,
                &timestamp_str,
                &scrollback_str,
                &env_vars_str,
                &working_dir_str,
                &command_history_str,
            ],
        )
        .map_err(|e| CisError::storage(format!("Failed to save checkpoint: {}", e)))?;

        Ok(())
    }

    /// Load latest checkpoint
    pub fn load_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionCheckpoint>> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let mut stmt = conn
            .prepare(
                r#"
                SELECT session_id, timestamp, scrollback, env_vars, working_dir, command_history
                FROM session_checkpoints
                WHERE session_id = ?1
                ORDER BY timestamp DESC
                LIMIT 1
                "#,
            )
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let checkpoint = stmt
            .query_row([session_id], |row| {
                let scrollback_str: String = row.get(2)?;
                let env_vars_str: String = row.get(3)?;
                let command_history_str: String = row.get(5)?;

                Ok(SessionCheckpoint {
                    session_id: row.get(0)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    scrollback: serde_json::from_str(&scrollback_str).unwrap_or_default(),
                    env_vars: serde_json::from_str(&env_vars_str).unwrap_or_default(),
                    working_dir: PathBuf::from(row.get::<_, String>(4)?),
                    command_history: serde_json::from_str(&command_history_str).unwrap_or_default(),
                })
            })
            .optional()
            .map_err(|e| CisError::storage(format!("Failed to load checkpoint: {}", e)))?;

        Ok(checkpoint)
    }

    /// Cleanup old sessions
    pub fn cleanup_old_sessions(&self, max_age: Duration) -> Result<usize> {
        let conn = self.conn.lock().map_err(|_| {
            CisError::storage("Failed to lock connection".to_string())
        })?;
        
        let cutoff = Utc::now() - chrono::Duration::seconds(max_age.as_secs() as i64);
        
        let affected = conn.execute(
            "DELETE FROM sessions WHERE last_activity < ?1 AND state = ?2",
            [&cutoff.to_rfc3339(), "Closed"],
        )
        .map_err(|e| CisError::storage(format!("Failed to cleanup sessions: {}", e)))?;

        Ok(affected)
    }

    /// Parse agent type from string
    fn parse_agent_type(s: &str) -> AgentType {
        match s.to_lowercase().as_str() {
            "claude" => AgentType::Claude,
            "kimi" => AgentType::Kimi,
            "aider" => AgentType::Aider,
            "opencode" => AgentType::OpenCode,
            _ => AgentType::Custom,
        }
    }

    /// Parse session state from string
    fn parse_session_state(s: &str) -> SessionState {
        match s.to_lowercase().as_str() {
            "initial" => SessionState::Initial,
            "connecting" => SessionState::Connecting,
            "active" => SessionState::Active,
            "closing" => SessionState::Closing,
            _ => SessionState::Closed,
        }
    }
}

impl EnhancedSessionManager {
    /// Create new session manager
    pub fn new(store_path: &Path) -> Result<Self> {
        let store = Arc::new(RwLock::new(SessionStore::new(store_path)?));
        let (agent_switch_tx, _agent_switch_rx) = mpsc::channel(100);

        Ok(Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            active_session: Arc::new(RwLock::new(None)),
            store,
            agent_switch_tx,
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
            session_timeout: DEFAULT_SESSION_TIMEOUT,
            cleanup_handle: None,
            shutdown_tx: None,
        })
    }

    /// Start background tasks (cleanup, checkpointing)
    pub async fn start(&mut self) -> Result<()> {
        self.start_cleanup_task().await;
        self.start_checkpoint_task().await;
        info!("Session manager started");
        Ok(())
    }

    /// Start cleanup task
    async fn start_cleanup_task(&mut self) {
        let sessions = self.sessions.clone();
        let store = self.store.clone();
        let timeout = self.session_timeout;
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let handle = tokio::spawn(async move {
            let interval = tokio::time::Duration::from_secs(60);
            let mut ticker = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        // Cleanup timed out sessions
                        let to_remove = {
                            let sessions_guard = sessions.read().await;
                            let mut expired = Vec::new();
                            
                            for (id, session) in sessions_guard.iter() {
                                if session.idle_duration() > timeout {
                                    warn!("Session {} idle timeout, scheduling for removal", id);
                                    expired.push(id.clone());
                                }
                            }
                            expired
                        };

                        // Remove expired sessions
                        for id in to_remove {
                            let mut sessions_guard = sessions.write().await;
                            if let Some(session) = sessions_guard.get_mut(&id) {
                                session.metadata.state = SessionState::Closed;
                            }
                            drop(sessions_guard);

                            // Persist state change
                            let store_guard = store.read().await;
                            if let Some(session) = sessions.read().await.get(&id) {
                                let _ = store_guard.save_session(&session.metadata);
                            }
                        }

                        // Cleanup old sessions in database
                        let store_guard = store.read().await;
                        let _ = store_guard.cleanup_old_sessions(Duration::from_secs(7 * 24 * 3600));
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Session cleanup task shutting down");
                        break;
                    }
                }
            }
        });

        self.cleanup_handle = Some(handle);
    }

    /// Start checkpoint task
    async fn start_checkpoint_task(&self) {
        let sessions = self.sessions.clone();
        let store = self.store.clone();
        let interval = self.checkpoint_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                let active_sessions = {
                    let guard = sessions.read().await;
                    guard
                        .iter()
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>()
                };

                for session_id in active_sessions {
                    // Create checkpoint
                    let checkpoint = SessionCheckpoint {
                        session_id: session_id.clone(),
                        timestamp: Utc::now(),
                        scrollback: Vec::new(),
                        env_vars: HashMap::new(),
                        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                        command_history: Vec::new(),
                    };

                    let store_guard = store.read().await;
                    if let Err(e) = store_guard.save_checkpoint(&checkpoint) {
                        warn!("Failed to save checkpoint for session {}: {}", session_id, e);
                    } else {
                        debug!("Checkpoint saved for session {}", session_id);
                    }

                    // Update last activity
                    let mut guard = sessions.write().await;
                    if let Some(session) = guard.get_mut(&session_id) {
                        session.touch();
                    }
                }
            }
        });
    }

    /// Create new session
    pub async fn create_session(
        &self,
        target_did: impl Into<String>,
        agent_type: AgentType,
        project_path: Option<PathBuf>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let target_did = target_did.into();

        info!(
            "Creating new session {} for target {} with agent {:?}",
            id, target_did, agent_type
        );

        let metadata = PersistentSession::new(
            id.clone(),
            target_did.clone(),
            agent_type,
            project_path.clone(),
        );

        // Persist session
        {
            let store = self.store.read().await;
            store.save_session(&metadata)?;
        }

        // Create managed session
        let managed = ManagedSession::new(metadata);

        // Store in memory
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(id.clone(), managed);
        }

        // Set as active
        {
            let mut active = self.active_session.write().await;
            *active = Some(id.clone());
        }

        info!("Session {} created successfully", id);
        Ok(id)
    }

    /// Resume a session from persistence
    pub async fn resume_session(&self, session_id: &str) -> Result<ManagedSession> {
        info!("Attempting to resume session {}", session_id);

        // Load from database
        let metadata = {
            let store = self.store.read().await;
            store
                .load_session(session_id)?
                .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?
        };

        // Check if session can be resumed
        if metadata.state == SessionState::Closed {
            return Err(CisError::invalid_input(
                format!("Session {} is closed and cannot be resumed", session_id)
            ));
        }

        // Check for timeout
        if metadata.is_timed_out(self.session_timeout) {
            return Err(CisError::invalid_input(
                format!("Session {} has timed out", session_id)
            ));
        }

        // Create new managed session
        let mut metadata = metadata;
        metadata.is_resumed = true;
        metadata.state = SessionState::Connecting;

        let managed = ManagedSession::new(metadata.clone());

        // Store in memory
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.to_string(), managed);
        }

        // Update persistence
        {
            let store = self.store.read().await;
            store.save_session(&metadata)?;
        }

        // Set as active
        {
            let mut active = self.active_session.write().await;
            *active = Some(session_id.to_string());
        }

        info!("Session {} resumed successfully", session_id);

        // Return a copy
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| CisError::not_found("Failed to retrieve resumed session".to_string()))
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<PersistentSession> {
        let store = self.store.read().await;
        store.list_sessions().unwrap_or_default()
    }

    /// List active (in-memory) sessions
    pub async fn list_active_sessions(&self) -> Vec<(String, SessionState)> {
        let sessions = self.sessions.read().await;
        sessions
            .iter()
            .map(|(id, s)| (id.clone(), s.metadata.state))
            .collect()
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<ManagedSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Get active session
    pub async fn get_active_session(&self) -> Option<String> {
        let active = self.active_session.read().await;
        active.clone()
    }

    /// Switch to a different session
    pub async fn switch_session(&self, session_id: &str) -> Result<()> {
        // Verify session exists
        let sessions = self.sessions.read().await;
        if !sessions.contains_key(session_id) {
            // Try to resume from persistence
            drop(sessions);
            self.resume_session(session_id).await?;
        } else {
            drop(sessions);
        }

        // Set as active
        let mut active = self.active_session.write().await;
        *active = Some(session_id.to_string());

        info!("Switched to session {}", session_id);
        Ok(())
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()> {
        info!("Closing session {}", session_id);

        // Remove from memory
        let mut sessions = self.sessions.write().await;
        if let Some(mut session) = sessions.remove(session_id) {
            session.metadata.state = SessionState::Closed;
        }
        drop(sessions);

        // Update persistence
        let store = self.store.read().await;
        if let Some(mut metadata) = store.load_session(session_id)? {
            metadata.state = SessionState::Closed;
            store.save_session(&metadata)?;
        }

        // Clear active if this was the active session
        let mut active = self.active_session.write().await;
        if active.as_deref() == Some(session_id) {
            *active = None;
        }

        info!("Session {} closed", session_id);
        Ok(())
    }

    /// Kill a session (force close)
    pub async fn kill_session(&self, session_id: &str) -> Result<()> {
        warn!("Force killing session {}", session_id);
        self.close_session(session_id).await
    }

    /// Switch agent in a session
    pub async fn switch_agent(
        &self,
        session_id: &str,
        new_agent: AgentType,
    ) -> Result<()> {
        info!(
            "Switching agent in session {} to {:?}",
            session_id, new_agent
        );

        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let old_agent = session.metadata.agent_type;

        if old_agent == new_agent {
            return Ok(());
        }

        // Send before-switch event
        let _ = self
            .agent_switch_tx
            .send(AgentSwitchEvent::BeforeSwitch {
                session_id: Uuid::parse_str(session_id).unwrap_or_else(|_| Uuid::new_v4()),
                from: old_agent,
                to: new_agent,
            })
            .await;

        // Update agent type
        session.metadata.agent_type = new_agent;
        session.agent_history.push((Utc::now(), new_agent));

        session.metadata.state = SessionState::Initial;
        session.touch();

        // Persist changes
        let metadata = session.metadata.clone();
        drop(sessions);

        let store = self.store.read().await;
        store.save_session(&metadata)?;

        // Send after-switch event
        let _ = self
            .agent_switch_tx
            .send(AgentSwitchEvent::AfterSwitch {
                session_id: Uuid::parse_str(session_id).unwrap_or_else(|_| Uuid::new_v4()),
                new_agent,
            })
            .await;

        info!(
            "Agent switched in session {} from {:?} to {:?}",
            session_id, old_agent, new_agent
        );

        Ok(())
    }

    /// Update session activity
    pub async fn touch_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        session.touch();
        Ok(())
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: &str) -> Result<SessionStats> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        Ok(SessionStats {
            duration_secs: session.duration().as_secs(),
            idle_secs: session.idle_duration().as_secs(),
            agent_switches: session.agent_history.len(),
            state: session.metadata.state,
        })
    }

    /// Shutdown the session manager
    pub async fn shutdown(&mut self) {
        info!("Shutting down session manager");

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // Wait for cleanup task
        if let Some(handle) = self.cleanup_handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // Close all sessions
        let session_ids: Vec<String> = {
            let sessions = self.sessions.read().await;
            sessions.keys().cloned().collect()
        };

        for id in session_ids {
            if let Err(e) = self.close_session(&id).await {
                warn!("Failed to close session {}: {}", id, e);
            }
        }

        info!("Session manager shutdown complete");
    }
}

impl Drop for EnhancedSessionManager {
    fn drop(&mut self) {
        // Send shutdown signal if not already done
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    /// Session duration in seconds
    pub duration_secs: u64,
    /// Idle time in seconds
    pub idle_secs: u64,
    /// Number of agent switches
    pub agent_switches: usize,
    /// Current state
    pub state: SessionState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (EnhancedSessionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("sessions.db");
        let manager = EnhancedSessionManager::new(&db_path).unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_session() {
        let (manager, _temp) = create_test_manager();

        let session_id = manager
            .create_session("did:cis:test", AgentType::Claude, None)
            .await
            .unwrap();

        assert!(!session_id.is_empty());

        let session = manager.get_session(&session_id).await;
        assert!(session.is_some());

        let active = manager.get_active_session().await;
        assert_eq!(active, Some(session_id));
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (manager, _temp) = create_test_manager();

        manager
            .create_session("did:cis:test1", AgentType::Claude, None)
            .await
            .unwrap();
        manager
            .create_session("did:cis:test2", AgentType::Kimi, None)
            .await
            .unwrap();

        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_switch_session() {
        let (manager, _temp) = create_test_manager();

        let id1 = manager
            .create_session("did:cis:test1", AgentType::Claude, None)
            .await
            .unwrap();
        let id2 = manager
            .create_session("did:cis:test2", AgentType::Kimi, None)
            .await
            .unwrap();

        manager.switch_session(&id1).await.unwrap();
        assert_eq!(manager.get_active_session().await, Some(id1));

        manager.switch_session(&id2).await.unwrap();
        assert_eq!(manager.get_active_session().await, Some(id2));
    }

    #[tokio::test]
    async fn test_switch_agent() {
        let (manager, _temp) = create_test_manager();

        let session_id = manager
            .create_session("did:cis:test", AgentType::Claude, None)
            .await
            .unwrap();

        manager
            .switch_agent(&session_id, AgentType::Kimi)
            .await
            .unwrap();

        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.metadata.agent_type, AgentType::Kimi);
        assert_eq!(session.agent_history.len(), 1);
    }

    #[tokio::test]
    async fn test_close_session() {
        let (manager, _temp) = create_test_manager();

        let session_id = manager
            .create_session("did:cis:test", AgentType::Claude, None)
            .await
            .unwrap();

        manager.close_session(&session_id).await.unwrap();

        let session = manager.get_session(&session_id).await;
        assert!(session.is_none());

        let persisted = {
            let store = manager.store.read().await;
            store.load_session(&session_id).unwrap()
        };
        assert!(persisted.is_some());
        assert_eq!(persisted.unwrap().state, SessionState::Closed);
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("sessions.db");

        let session_id = {
            let manager = EnhancedSessionManager::new(&db_path).unwrap();
            let id = manager
                .create_session("did:cis:test", AgentType::Claude, None)
                .await
                .unwrap();
            id
        };

        // Create new manager with same database
        let manager = EnhancedSessionManager::new(&db_path).unwrap();
        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session_id);

        // Resume session
        let resumed = manager.resume_session(&session_id).await;
        assert!(resumed.is_ok());
        assert!(resumed.unwrap().metadata.is_resumed);
    }

    #[tokio::test]
    async fn test_session_stats() {
        let (manager, _temp) = create_test_manager();

        let session_id = manager
            .create_session("did:cis:test", AgentType::Claude, None)
            .await
            .unwrap();

        let stats = manager.get_session_stats(&session_id).await.unwrap();
        assert_eq!(stats.agent_switches, 0);
        assert_eq!(stats.state, SessionState::Initial);

        manager
            .switch_agent(&session_id, AgentType::Kimi)
            .await
            .unwrap();

        let stats = manager.get_session_stats(&session_id).await.unwrap();
        assert_eq!(stats.agent_switches, 1);
    }

    #[test]
    fn test_persistent_session_timeout() {
        let mut session = PersistentSession::new(
            "test".to_string(),
            "did:cis:test".to_string(),
            AgentType::Claude,
            None,
        );
        
        // 刚创建的会话不应该超时
        assert!(!session.is_timed_out(Duration::from_secs(3600)));
        
        // 修改 last_activity 为很久以前
        session.last_activity = Utc::now() - chrono::Duration::seconds(7200);
        
        // 现在应该超时了
        assert!(session.is_timed_out(Duration::from_secs(3600)));
    }
}
