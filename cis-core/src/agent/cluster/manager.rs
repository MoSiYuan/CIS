//! # Session Manager
//!
//! Manager for all Agent Cluster sessions.
//! Supports CLI/GUI/API layers through shared state and event broadcasting.
//!
//! ## 废弃警告
//!
//! `SessionManager::global()` 全局单例方法已废弃。
//! 请使用 `ServiceContainer` 进行依赖注入。
//!
//! ```rust
//! // [X] 废弃的方式
//! let manager = SessionManager::global();
//!
//! // [OK] 推荐的方式
//! let container = ServiceContainer::production(config).await?;
//! // 通过容器获取 SessionManager 实例
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{info, warn};

use crate::agent::AgentType;
use crate::agent::cluster::events::{EventBroadcaster, SessionEvent, SessionState, SessionSummary};
use crate::agent::cluster::session::AgentSession;
use crate::agent::cluster::SessionId;
use crate::error::{CisError, Result};

/// Default configuration values
const DEFAULT_MAX_BUFFER_LINES: usize = 10000;
const DEFAULT_BLOCKAGE_CHECK_INTERVAL_MS: u64 = 500;
const DEFAULT_MAX_SESSIONS: usize = 100;

/// Get default socket directory from environment or use default
fn default_socket_dir() -> std::path::PathBuf {
    std::env::var("CIS_SOCKET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("TMPDIR")
                .map(|tmp| std::path::PathBuf::from(tmp).join("cis").join("sessions"))
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/cis/sessions"))
        })
}

/// Session manager configuration
#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    /// Unix socket directory for attach
    pub socket_dir: std::path::PathBuf,
    /// Maximum output buffer lines per session
    pub max_buffer_lines: usize,
    /// Blockage detection keywords
    pub blockage_keywords: Vec<String>,
    /// Blockage check interval (milliseconds)
    pub blockage_check_interval_ms: u64,
    /// Default timeout for sessions (seconds)
    pub default_timeout_secs: u64,
    /// Maximum concurrent sessions
    pub max_sessions: usize,
    /// Enable blockage detection
    pub enable_blockage_detection: bool,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            socket_dir: default_socket_dir(),
            max_buffer_lines: DEFAULT_MAX_BUFFER_LINES,
            blockage_keywords: vec![
                "?".to_string(),
                "confirm".to_string(),
                "yes/no".to_string(),
                "y/n".to_string(),
                "enter to continue".to_string(),
                "press any key".to_string(),
                "authentication required".to_string(),
                "password:".to_string(),
                "merge conflict".to_string(),
                "rebase conflict".to_string(),
                "conflict:".to_string(),
                "error:".to_string(),
                "fatal:".to_string(),
            ],
            blockage_check_interval_ms: DEFAULT_BLOCKAGE_CHECK_INTERVAL_MS,
            default_timeout_secs: 3600,
            max_sessions: DEFAULT_MAX_SESSIONS,
            enable_blockage_detection: true,
        }
    }
}

/// Session Manager - for managing agent sessions
///
/// [WARNING] 注意: `global()` 方法已废弃，请使用依赖注入。
#[derive(Debug)]
pub struct SessionManager {
    /// Active sessions (Mutex for thread-safety with non-Sync AgentSession)
    sessions: Arc<Mutex<HashMap<SessionId, Arc<RwLock<AgentSession>>>>>,
    /// Configuration
    config: SessionManagerConfig,
    /// Event broadcaster
    event_broadcaster: EventBroadcaster,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<()>>>>,
}

impl SessionManager {
    /// Create new session manager with config
    pub fn new(config: SessionManagerConfig) -> Self {
        let event_broadcaster = EventBroadcaster::new(1024);
        
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            config,
            event_broadcaster,
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Get global singleton instance (DEPRECATED)
    ///
    /// [WARNING] 警告: 此方法已废弃，将在 v1.2.0 中移除。
    /// 请使用 `ServiceContainer` 进行依赖注入。
    #[deprecated(
        since = "1.1.4",
        note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
    )]
    #[allow(deprecated)]
    pub fn global() -> &'static Self {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<SessionManager> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new(SessionManagerConfig::default()))
    }

    /// Initialize the session manager (start background tasks)
    pub async fn init(&self) -> Result<()> {
        info!("Initializing SessionManager...");

        // Create socket directory if needed
        if !self.config.socket_dir.exists() {
            std::fs::create_dir_all(&self.config.socket_dir)
                .map_err(|e| CisError::execution(format!("Failed to create socket dir: {}", e)))?;
        }

        // Start blockage detection task if enabled
        if self.config.enable_blockage_detection {
            self.start_blockage_detection().await;
        }

        info!("SessionManager initialized");
        Ok(())
    }

    /// Start blockage detection background task
    async fn start_blockage_detection(&self) {
        let sessions = self.sessions.clone();
        let keywords = self.config.blockage_keywords.clone();
        let interval = self.config.blockage_check_interval_ms;
        let _event_broadcaster = self.event_broadcaster.clone();

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        tokio::spawn(async move {
            info!("Blockage detection task started");
            let mut interval = tokio::time::interval(Duration::from_millis(interval));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let sessions_guard = sessions.lock().await;
                        for (id, session_arc) in sessions_guard.iter() {
                            let session = session_arc.read().await;
                            
                            // Only check running sessions
                            match session.get_state().await {
                                SessionState::RunningDetached | SessionState::Attached { .. } => {
                                    if let Some(reason) = session.check_blockage(&keywords).await {
                                        warn!("Blockage detected in session {}: {}", id, reason);
                                        session.mark_blocked(&reason).await;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Blockage detection task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        dag_run_id: &str,
        task_id: &str,
        agent_type: AgentType,
        prompt: &str,
        work_dir: &Path,
        upstream_context: &str,
    ) -> Result<SessionId> {
        // Check max sessions limit
        let session_count = self.sessions.lock().await.len();
        if session_count >= self.config.max_sessions {
            return Err(CisError::execution(format!(
                "Maximum session limit reached ({}/{})",
                session_count, self.config.max_sessions
            )));
        }

        let session_id = SessionId::new(dag_run_id, task_id);

        // Check if session already exists
        if self.sessions.lock().await.contains_key(&session_id) {
            return Err(CisError::invalid_input(format!(
                "Session {} already exists",
                session_id
            )));
        }

        info!("Creating session {} for task {} (agent: {:?})", 
            session_id.short(), task_id, agent_type);

        // Create session
        let session = AgentSession::new(
            session_id.clone(),
            agent_type,
            work_dir.to_path_buf(),
            prompt.to_string(),
            upstream_context.to_string(),
            self.event_broadcaster.clone(),
            self.config.max_buffer_lines,
        );

        let session_arc = Arc::new(RwLock::new(session));

        // Start session
        {
            let mut session = session_arc.write().await;
            session.start(80, 24).await?;
        }

        // Store session
        self.sessions.lock().await.insert(session_id.clone(), session_arc);

        // Broadcast creation event
        let summary = self.get_session_summary(&session_id).await?;
        let _ = self.event_broadcaster.send(SessionEvent::Created {
            session_id: session_id.clone(),
            summary,
            timestamp: Utc::now(),
        });

        info!("Session {} created successfully", session_id.short());
        Ok(session_id)
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &SessionId) -> Option<Arc<RwLock<AgentSession>>> {
        self.sessions.lock().await.get(session_id).cloned()
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<SessionSummary> {
        let mut summaries = Vec::new();
        let sessions_guard = self.sessions.lock().await;

        for (id, session_arc) in sessions_guard.iter() {
            if let Ok(summary) = self.build_session_summary(id, session_arc).await {
                summaries.push(summary);
            }
        }

        summaries
    }

    /// List sessions by DAG run ID
    pub async fn list_sessions_by_dag(&self, dag_run_id: &str) -> Vec<SessionSummary> {
        let mut summaries = Vec::new();
        let sessions_guard = self.sessions.lock().await;

        for (id, session_arc) in sessions_guard.iter() {
            if id.dag_run_id == dag_run_id {
                if let Ok(summary) = self.build_session_summary(id, session_arc).await {
                    summaries.push(summary);
                }
            }
        }

        summaries
    }

    /// Get session summary
    async fn get_session_summary(&self, session_id: &SessionId) -> Result<SessionSummary> {
        let sessions_guard = self.sessions.lock().await;
        let session_arc = sessions_guard
            .get(session_id)
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;
        
        self.build_session_summary(session_id, session_arc).await
    }

    /// Build session summary from session
    async fn build_session_summary(
        &self,
        id: &SessionId,
        session_arc: &Arc<RwLock<AgentSession>>,
    ) -> Result<SessionSummary> {
        let session = session_arc.read().await;
        let state = session.get_state().await;
        let runtime = session.runtime();
        let preview = session.get_output_preview(3).await;

        Ok(SessionSummary {
            id: id.to_string(),
            short_id: id.short(),
            dag_run_id: id.dag_run_id.clone(),
            task_id: id.task_id.clone(),
            agent_type: session.agent_type,
            state: state.to_string(),
            runtime_secs: runtime.as_secs(),
            output_preview: preview,
            created_at: session.created_at,
        })
    }

    /// Attach to a session
    pub async fn attach_session(&self, session_id: &SessionId, user: &str) -> Result<AttachHandle> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        
        // Check if already attached by someone else
        if let Some(current_user) = session.attached_user().await {
            if current_user != user {
                return Err(CisError::invalid_input(format!(
                    "Session {} is already attached by {}",
                    session_id, current_user
                )));
            }
        }

        // Mark as attached
        drop(session);
        let session = session_arc.write().await;
        session.attach(user).await?;

        info!("User {} attached to session {}", user, session_id.short());

        // Create attach handle using session's public methods
        let handle = AttachHandle::new(
            session_id.clone(),
            session_arc.clone(),
        ).await?;

        Ok(handle)
    }

    /// Detach from a session
    pub async fn detach_session(&self, session_id: &SessionId, user: &str) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.write().await;
        session.detach(user).await?;

        info!("User {} detached from session {}", user, session_id.short());
        Ok(())
    }

    /// Mark session as blocked
    pub async fn mark_blocked(&self, session_id: &SessionId, reason: &str) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        session.mark_blocked(reason).await;

        info!("Session {} marked as blocked: {}", session_id.short(), reason);
        Ok(())
    }

    /// Mark session as recovered
    pub async fn mark_recovered(&self, session_id: &SessionId) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        session.mark_recovered().await;

        info!("Session {} marked as recovered", session_id.short());
        Ok(())
    }

    /// Mark session as completed
    pub async fn mark_completed(&self, session_id: &SessionId, output: &str, exit_code: i32) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        session.mark_completed(output, exit_code).await;

        info!("Session {} marked as completed (exit: {})", session_id.short(), exit_code);
        Ok(())
    }

    /// Mark session as failed
    pub async fn mark_failed(&self, session_id: &SessionId, error: &str) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        session.mark_failed(error).await;

        info!("Session {} marked as failed: {}", session_id.short(), error);
        Ok(())
    }

    /// Get session output
    pub async fn get_output(&self, session_id: &SessionId) -> Result<String> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        Ok(session.get_output().await)
    }

    /// Send input to session
    pub async fn send_input(&self, session_id: &SessionId, data: &[u8]) -> Result<()> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        session.send_input(data)?;
        Ok(())
    }

    /// Get session state
    pub async fn get_state(&self, session_id: &SessionId) -> Result<SessionState> {
        let session_arc = self.get_session(session_id)
            .await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        let session = session_arc.read().await;
        Ok(session.get_state().await)
    }

    /// Kill a session
    pub async fn kill_session(&self, session_id: &SessionId, reason: &str) -> Result<()> {
        info!("Killing session {}: {}", session_id.short(), reason);

        // Remove from sessions map first
        let session_arc = {
            let mut sessions_guard = self.sessions.lock().await;
            sessions_guard.remove(session_id)
        };

        if let Some(session_arc) = session_arc {
            let mut session = session_arc.write().await;
            session.shutdown(reason).await?;
        } else {
            return Err(CisError::not_found(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    /// Kill all sessions for a DAG run
    pub async fn kill_all_by_dag(&self, dag_run_id: &str, reason: &str) -> Result<usize> {
        info!("Killing all sessions for DAG run {}", dag_run_id);

        let to_kill: Vec<SessionId> = {
            let sessions_guard = self.sessions.lock().await;
            sessions_guard
                .iter()
                .filter(|(id, _)| id.dag_run_id == dag_run_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        let count = to_kill.len();
        for session_id in to_kill {
            let _ = self.kill_session(&session_id, reason).await;
        }

        Ok(count)
    }

    /// Subscribe to session events
    pub fn subscribe_events(&self) -> broadcast::Receiver<SessionEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Get number of active sessions
    pub async fn session_count(&self) -> usize {
        self.sessions.lock().await.len()
    }

    /// Get number of active sessions for a DAG run
    pub async fn session_count_by_dag(&self, dag_run_id: &str) -> usize {
        self.sessions
            .lock()
            .await
            .iter()
            .filter(|(id, _)| id.dag_run_id == dag_run_id)
            .count()
    }

    /// Shutdown all sessions and cleanup
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down SessionManager...");

        // Signal background tasks to stop
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(()).await;
        }

        // Kill all sessions
        let session_ids: Vec<SessionId> = {
            let mut sessions_guard = self.sessions.lock().await;
            let ids = sessions_guard.keys().cloned().collect();
            sessions_guard.clear();
            ids
        };

        for session_id in session_ids {
            let _ = self.kill_session(&session_id, "SessionManager shutdown").await;
        }

        info!("SessionManager shutdown complete");
        Ok(())
    }
}

/// Attach handle for interacting with a session
pub struct AttachHandle {
    /// Session ID
    pub session_id: SessionId,
    /// Session reference
    session: Arc<RwLock<AgentSession>>,
}

impl AttachHandle {
    /// Create new attach handle
    async fn new(session_id: SessionId, session: Arc<RwLock<AgentSession>>) -> Result<Self> {
        // Verify session is ready for attach
        let s = session.read().await;
        if s.get_state().await == SessionState::Spawning {
            return Err(CisError::execution("Session still spawning"));
        }
        drop(s);

        Ok(Self {
            session_id,
            session,
        })
    }

    /// Send input to the session
    pub fn send_input(&self, data: &[u8]) -> Result<()> {
        // Use try_lock to avoid blocking in non-async context
        if let Ok(session) = self.session.try_read() {
            session.send_input(data)
        } else {
            Err(CisError::execution("Session locked"))
        }
    }

    /// Try receive output (non-blocking)
    pub fn try_receive_output(&self) -> Option<Vec<u8>> {
        // Use try_lock to avoid blocking in non-async context
        if let Ok(session) = self.session.try_read() {
            session.try_receive_output()
        } else {
            None
        }
    }

    /// Get current state
    pub async fn get_state(&self) -> SessionState {
        let session = self.session.read().await;
        session.get_state().await
    }

    /// Get session output
    pub async fn get_output(&self) -> String {
        let session = self.session.read().await;
        session.get_output().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_session_manager_creation() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_global_singleton() {
        let manager1 = SessionManager::global();
        let manager2 = SessionManager::global();
        // Both should point to the same instance
        assert!(std::ptr::eq(manager1, manager2));
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        let mut rx = manager.subscribe_events();

        // This just tests that subscription works
        // Actual events would be sent during session operations
        assert!(rx.try_recv().is_err()); // Empty channel
    }
}
