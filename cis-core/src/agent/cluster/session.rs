//! # Agent Session
//!
//! Manages a single Agent session with PTY support.
//! Reuses core logic from `network::agent_session` but adapted for local DAG execution.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::agent::{AgentType, AgentCommandConfig, AgentMode};
use crate::agent::cluster::events::{EventBroadcaster, SessionEvent, SessionState};
use crate::agent::cluster::SessionId;
use crate::error::{CisError, Result};

/// Default terminal size
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;

/// Default output buffer size (max lines)
const DEFAULT_MAX_BUFFER_LINES: usize = 10000;

/// Output buffer with line limit
#[derive(Debug)]
pub struct OutputBuffer {
    /// Buffer lines
    lines: VecDeque<String>,
    /// Total bytes stored
    total_bytes: usize,
    /// Maximum lines to keep
    max_lines: usize,
}

impl OutputBuffer {
    /// Create new buffer with max lines limit
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_lines),
            total_bytes: 0,
            max_lines,
        }
    }

    /// Append data to buffer
    pub fn append(&mut self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        for line in text.lines() {
            self.lines.push_back(line.to_string());
            self.total_bytes += line.len();
        }
        // Trim if exceeds max lines
        while self.lines.len() > self.max_lines {
            if let Some(old) = self.lines.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(old.len());
            }
        }
    }

    /// Get all lines as string
    pub fn as_string(&self) -> String {
        self.lines.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    /// Get recent lines (last N)
    pub fn recent_lines(&self, n: usize) -> Vec<&str> {
        self.lines.iter().rev().take(n).map(|s| s.as_str()).collect()
    }

    /// Get preview (last few lines as single string)
    pub fn preview(&self, lines: usize) -> String {
        self.recent_lines(lines).into_iter().rev().collect::<Vec<_>>().join("\n")
    }

    /// Get total bytes
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if buffer contains keyword (case insensitive)
    pub fn contains_keyword(&self, keyword: &str) -> Option<String> {
        let keyword_lower = keyword.to_lowercase();
        for line in self.lines.iter().rev().take(20) {
            if line.to_lowercase().contains(&keyword_lower) {
                return Some(line.clone());
            }
        }
        None
    }
}

impl Default for OutputBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_BUFFER_LINES)
    }
}

/// Shared session internals (thread-safe wrapper)
struct SessionInternals {
    /// PTY master handle
    pty_master: Option<Box<dyn MasterPty + Send>>,
    /// Agent process handle
    process_handle: Option<Box<dyn Child + Send + Sync>>,
    /// Input channel sender (to PTY)
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    /// Output channel receiver (from PTY)
    output_rx: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// I/O thread handle
    io_handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for SessionInternals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionInternals")
            .field("pty_master", &self.pty_master.is_some())
            .field("process_handle", &self.process_handle.is_some())
            .field("input_tx", &self.input_tx.is_some())
            .field("output_rx", &self.output_rx.is_some())
            .field("shutdown_tx", &self.shutdown_tx.is_some())
            .field("io_handle", &self.io_handle.is_some())
            .finish()
    }
}

/// Agent Session internal state
#[derive(Debug)]
pub struct AgentSession {
    /// Session ID
    pub id: SessionId,
    /// Agent type
    pub agent_type: AgentType,
    /// Current state
    pub(crate) state: Arc<RwLock<SessionState>>,
    /// Shared internals (protected by Mutex for thread safety)
    internals: Arc<Mutex<SessionInternals>>,
    /// Output buffer
    output_buffer: Arc<RwLock<OutputBuffer>>,
    /// Work directory
    pub work_dir: PathBuf,
    /// Initial prompt
    pub prompt: String,
    /// Upstream context (from dependencies)
    pub upstream_context: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    last_activity: Arc<RwLock<DateTime<Utc>>>,
    /// Max buffer lines
    max_buffer_lines: usize,
    /// Event broadcaster
    event_broadcaster: EventBroadcaster,
    /// Attached user (if any)
    attached_user: Arc<RwLock<Option<String>>>,
    
    // === Persistence fields ===
    /// Whether this is a persistent session (does not auto-destroy after task completion)
    persistent: bool,
    
    /// Maximum idle time in seconds before auto-destroy (0 means no limit)
    max_idle_secs: u64,
}

impl AgentSession {
    /// Create a new agent session (does not start PTY yet)
    pub fn new(
        id: SessionId,
        agent_type: AgentType,
        work_dir: PathBuf,
        prompt: String,
        upstream_context: String,
        event_broadcaster: EventBroadcaster,
        max_buffer_lines: usize,
    ) -> Self {
        let now = Utc::now();
        let internals = SessionInternals {
            pty_master: None,
            process_handle: None,
            input_tx: None,
            output_rx: None,
            shutdown_tx: None,
            io_handle: None,
        };
        Self {
            id,
            agent_type,
            state: Arc::new(RwLock::new(SessionState::Spawning)),
            internals: Arc::new(Mutex::new(internals)),
            output_buffer: Arc::new(RwLock::new(OutputBuffer::new(max_buffer_lines))),
            work_dir,
            prompt,
            upstream_context,
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            max_buffer_lines,
            event_broadcaster,
            attached_user: Arc::new(RwLock::new(None)),
            persistent: false,
            max_idle_secs: 0,
        }
    }

    /// Start the agent session (spawn PTY and agent process)
    pub async fn start(&mut self, cols: u16, rows: u16) -> Result<()> {
        info!("Starting agent session {} with {:?}", self.id, self.agent_type);

        // Create PTY
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| CisError::execution(format!("Failed to open PTY: {}", e)))?;

        // Build agent command
        let cmd = self.build_agent_command()?;

        // Spawn agent process
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| CisError::execution(format!("Failed to spawn agent: {}", e)))?;

        // Store in internals
        {
            let mut internals = self.internals.lock().await;
            internals.process_handle = Some(child);
            internals.pty_master = Some(pair.master);
        }

        // Start I/O thread
        self.start_io_thread().await?;

        // Send initial prompt after a short delay
        let input_tx = self.internals.lock().await.input_tx.clone();
        let prompt = format!("{}\n", self.prompt);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Some(tx) = input_tx {
                let _ = tx.send(prompt.into_bytes());
            }
        });

        // Update state
        *self.state.write().await = SessionState::RunningDetached;
        
        // Broadcast state change
        let _ = self.event_broadcaster.send(SessionEvent::StateChanged {
            session_id: self.id.clone(),
            old_state: SessionState::Spawning,
            new_state: SessionState::RunningDetached,
            timestamp: Utc::now(),
        });

        info!("Agent session {} started successfully", self.id);
        Ok(())
    }

    /// Build command for agent type (重构版本)
    fn build_agent_command(&self) -> Result<CommandBuilder> {
        // 获取 Agent 配置
        let config = AgentCommandConfig::from_agent_type(self.agent_type, AgentMode::Single)
            .ok_or_else(|| CisError::configuration(
                format!("Agent type {:?} not supported for cluster sessions", self.agent_type)
            ))?;

        // 构建命令
        let cmd = config.build_command(&self.work_dir, &self.id.to_string())?;

        debug!("Built agent command: {:?} with args {:?}", config.command, config.base_args);

        Ok(cmd)
    }

    /// Start I/O thread for PTY communication
    async fn start_io_thread(&mut self) -> Result<()> {
        let master = {
            let mut internals = self.internals.lock().await;
            internals.pty_master.take().ok_or_else(|| {
                CisError::execution("PTY master not initialized")
            })?
        };

        // Create channels
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (output_tx, output_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

        // Store channels in internals
        {
            let mut internals = self.internals.lock().await;
            internals.input_tx = Some(input_tx);
            internals.output_rx = Some(output_rx);
            internals.shutdown_tx = Some(shutdown_tx);
        }

        let session_id = self.id.clone();
        let output_buffer = self.output_buffer.clone();
        let _state = self.state.clone();
        let last_activity = self.last_activity.clone();
        let event_broadcaster = self.event_broadcaster.clone();

        // Spawn blocking I/O thread
        let handle = tokio::task::spawn_blocking(move || {
            info!("PTY I/O thread started for session {}", session_id);

            let mut writer = master.take_writer().ok();
            let mut reader = master.try_clone_reader().ok();
            let mut buf = vec![0u8; 4096];

            loop {
                // Check shutdown signal
                match shutdown_rx.try_recv() {
                    Ok(_) | Err(mpsc::error::TryRecvError::Disconnected) => {
                        info!("PTY I/O thread shutting down for session {}", session_id);
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
                }

                // Read from PTY
                if let Some(ref mut r) = reader {
                    match r.read(&mut buf) {
                        Ok(0) => {
                            debug!("PTY EOF for session {}", session_id);
                            break;
                        }
                        Ok(n) => {
                            let data = buf[..n].to_vec();
                            
                            // Update output buffer
                            if let Ok(mut buffer) = output_buffer.try_write() {
                                buffer.append(&data);
                            }
                            
                            // Send to channel
                            if output_tx.send(data.clone()).is_err() {
                                warn!("Output channel closed for session {}", session_id);
                                break;
                            }

                            // Update activity
                            if let Ok(mut activity) = last_activity.try_write() {
                                *activity = Utc::now();
                            }

                            // Broadcast output event
                            let _ = event_broadcaster.send(SessionEvent::OutputUpdated {
                                session_id: session_id.clone(),
                                data: String::from_utf8_lossy(&data).to_string(),
                                timestamp: Utc::now(),
                            });
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

                // Write to PTY
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
                            if let Ok(mut activity) = last_activity.try_write() {
                                *activity = Utc::now();
                            }
                        }
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            debug!("Input channel closed for session {}", session_id);
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

        // Store I/O handle
        {
            let mut internals = self.internals.lock().await;
            internals.io_handle = Some(handle);
        }
        Ok(())
    }

    /// Send input to PTY
    pub fn send_input(&self, data: &[u8]) -> Result<()> {
        // Use try_lock to avoid blocking in non-async context
        if let Ok(internals) = self.internals.try_lock() {
            if let Some(ref tx) = internals.input_tx {
                tx.send(data.to_vec())
                    .map_err(|_| CisError::execution("Input channel closed"))?;
                return Ok(());
            }
        }
        Err(CisError::execution("Session not started"))
    }

    /// Try receive output (non-blocking)
    pub fn try_receive_output(&self) -> Option<Vec<u8>> {
        // Use try_lock to avoid blocking in non-async context
        if let Ok(mut internals) = self.internals.try_lock() {
            if let Some(ref mut rx) = internals.output_rx {
                match rx.try_recv() {
                    Ok(data) => return Some(data),
                    Err(_) => return None,
                }
            }
        }
        None
    }

    /// Get current state
    pub async fn get_state(&self) -> SessionState {
        self.state.read().await.clone()
    }

    /// Set state (internal use)
    pub async fn set_state(&self, new_state: SessionState) {
        let old_state = self.state.read().await.clone();
        *self.state.write().await = new_state.clone();
        
        let _ = self.event_broadcaster.send(SessionEvent::StateChanged {
            session_id: self.id.clone(),
            old_state,
            new_state,
            timestamp: Utc::now(),
        });
    }

    /// Mark as blocked
    pub async fn mark_blocked(&self, reason: &str) {
        self.set_state(SessionState::Blocked {
            reason: reason.to_string(),
        }).await;
        
        let _ = self.event_broadcaster.send(SessionEvent::Blocked {
            session_id: self.id.clone(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        });
    }

    /// Mark as recovered from blocked
    pub async fn mark_recovered(&self) {
        self.set_state(SessionState::RunningDetached).await;
        
        let _ = self.event_broadcaster.send(SessionEvent::Recovered {
            session_id: self.id.clone(),
            timestamp: Utc::now(),
        });
    }

    /// Mark as completed
    /// 
    /// For persistent sessions, transitions to Idle state instead of Completed.
    pub async fn mark_completed(&self, output: &str, exit_code: i32) {
        if self.persistent && exit_code == 0 {
            // Persistent session with success: go to Idle
            self.mark_idle().await;
        } else {
            // Non-persistent or failed: go to Completed/Failed
            let state = SessionState::Completed {
                output: output.to_string(),
                exit_code,
            };
            self.set_state(state.clone()).await;

            let _ = self.event_broadcaster.send(SessionEvent::Completed {
                session_id: self.id.clone(),
                output: output.to_string(),
                exit_code,
                timestamp: Utc::now(),
            });
        }
    }

    /// Mark as failed
    /// 
    /// Note: Even persistent sessions transition to Failed state on errors.
    pub async fn mark_failed(&self, error: &str) {
        let state = SessionState::Failed {
            error: error.to_string(),
        };
        self.set_state(state.clone()).await;

        let _ = self.event_broadcaster.send(SessionEvent::Failed {
            session_id: self.id.clone(),
            error: error.to_string(),
            timestamp: Utc::now(),
        });
    }

    /// Attach user
    pub async fn attach(&self, user: &str) -> Result<()> {
        *self.attached_user.write().await = Some(user.to_string());
        self.set_state(SessionState::Attached {
            user: user.to_string(),
        }).await;
        
        let _ = self.event_broadcaster.send(SessionEvent::Attached {
            session_id: self.id.clone(),
            user: user.to_string(),
            timestamp: Utc::now(),
        });
        Ok(())
    }

    /// Detach user
    /// 
    /// For persistent sessions in Idle state, stays in Idle.
    /// Otherwise transitions to RunningDetached.
    pub async fn detach(&self, user: &str) -> Result<()> {
        *self.attached_user.write().await = None;
        
        // Only transition to RunningDetached if not already in Idle
        let current_state = self.state.read().await.clone();
        if !matches!(current_state, SessionState::Idle) {
            self.set_state(SessionState::RunningDetached).await;
        }

        let _ = self.event_broadcaster.send(SessionEvent::Detached {
            session_id: self.id.clone(),
            user: user.to_string(),
            timestamp: Utc::now(),
        });
        Ok(())
    }

    /// Get attached user
    pub async fn attached_user(&self) -> Option<String> {
        self.attached_user.read().await.clone()
    }

    /// Get output buffer content
    pub async fn get_output(&self) -> String {
        self.output_buffer.read().await.as_string()
    }

    /// Get output buffer preview
    pub async fn get_output_preview(&self, lines: usize) -> String {
        self.output_buffer.read().await.preview(lines)
    }

    /// Check for blockage keywords in output
    pub async fn check_blockage(&self, keywords: &[String]) -> Option<String> {
        let buffer = self.output_buffer.read().await;
        for keyword in keywords {
            if let Some(line) = buffer.contains_keyword(keyword) {
                return Some(format!("{}: {}", keyword, line));
            }
        }
        None
    }

    // === Persistence methods ===

    /// Set persistent mode
    pub fn set_persistent(&mut self, persistent: bool) {
        self.persistent = persistent;
    }

    /// Check if session is persistent
    pub fn is_persistent(&self) -> bool {
        self.persistent
    }

    /// Set maximum idle time in seconds (0 means no limit)
    pub fn set_max_idle_secs(&mut self, secs: u64) {
        self.max_idle_secs = secs;
    }

    /// Get maximum idle time in seconds
    pub fn max_idle_secs(&self) -> u64 {
        self.max_idle_secs
    }

    /// Update last activity timestamp
    pub async fn touch(&self) {
        *self.last_activity.write().await = Utc::now();
    }

    /// Check if session should be auto-destroyed
    /// 
    /// For non-persistent sessions: returns true if in terminal state
    /// For persistent sessions: returns true only if idle timeout exceeded
    pub async fn should_auto_destroy(&self) -> bool {
        if self.persistent {
            // Persistent mode: check idle timeout
            if self.max_idle_secs > 0 {
                let last_activity = *self.last_activity.read().await;
                let idle_time = Utc::now().signed_duration_since(last_activity);
                if idle_time.num_seconds() > self.max_idle_secs as i64 {
                    return true; // Idle timeout exceeded
                }
            }
            false // Don't destroy persistent sessions
        } else {
            // Non-persistent mode: destroy if in terminal state
            let state = self.state.read().await.clone();
            state.is_terminal()
        }
    }

    /// Send input to PTY (async version for persistent mode)
    /// 
    /// This is the async version that automatically appends a newline
    /// and updates the activity timestamp.
    pub async fn send_input_async(&self, input: &str) -> Result<()> {
        let input_tx = {
            let internals = self.internals.lock().await;
            internals.input_tx.clone()
        };

        if let Some(tx) = input_tx {
            // Add newline and send
            let input_with_newline = format!("{}\n", input);
            tx.send(input_with_newline.into_bytes())
                .map_err(|_| CisError::execution("Failed to send input to session"))?;

            // Update activity timestamp
            self.touch().await;

            Ok(())
        } else {
            Err(CisError::execution("Session not started"))
        }
    }

    /// Get current output content (non-destructive read)
    /// 
    /// Returns the full content of the output buffer as a string.
    pub async fn get_output_content(&self) -> String {
        self.output_buffer.read().await.as_string()
    }

    /// Wait for specific output pattern with timeout
    /// 
    /// Polls the output buffer until the pattern is found or timeout is reached.
    /// Returns the full output content when pattern is found.
    pub async fn wait_for_output(&self, pattern: &str, timeout: Duration) -> Result<String> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            let output = self.get_output_content().await;
            if output.contains(pattern) {
                return Ok(output);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(CisError::execution(format!(
            "Timeout waiting for pattern: {}",
            pattern
        )))
    }

    /// Mark session as idle (for persistent sessions after task completion)
    /// 
    /// Only transitions to Idle state if the session is persistent.
    /// For non-persistent sessions, the original terminal state is preserved.
    pub async fn mark_idle(&self) {
        if self.persistent {
            let old_state = self.state.read().await.clone();
            *self.state.write().await = SessionState::Idle;
            self.touch().await;

            let _ = self.event_broadcaster.send(SessionEvent::StateChanged {
                session_id: self.id.clone(),
                old_state,
                new_state: SessionState::Idle,
                timestamp: Utc::now(),
            });
        }
    }

    /// Mark session as paused
    pub async fn mark_paused(&self) {
        let old_state = self.state.read().await.clone();
        *self.state.write().await = SessionState::Paused;

        let _ = self.event_broadcaster.send(SessionEvent::StateChanged {
            session_id: self.id.clone(),
            old_state,
            new_state: SessionState::Paused,
            timestamp: Utc::now(),
        });
    }

    /// Resume from paused state
    /// 
    /// Transitions back to RunningDetached if currently paused.
    pub async fn resume(&self) -> Result<()> {
        let current_state = self.state.read().await.clone();
        
        if !matches!(current_state, SessionState::Paused) {
            return Err(CisError::execution(format!(
                "Cannot resume from state: {:?}",
                current_state
            )));
        }

        *self.state.write().await = SessionState::RunningDetached;
        self.touch().await;

        let _ = self.event_broadcaster.send(SessionEvent::StateChanged {
            session_id: self.id.clone(),
            old_state: SessionState::Paused,
            new_state: SessionState::RunningDetached,
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// Shutdown the session
    pub async fn shutdown(&mut self, reason: &str) -> Result<()> {
        info!("Shutting down session {}: {}", self.id, reason);

        // Get internals and cleanup
        let mut internals = self.internals.lock().await;
        
        // Send shutdown signal
        if let Some(tx) = internals.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // Wait for I/O thread to stop
        if let Some(handle) = internals.io_handle.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // Kill process
        if let Some(mut handle) = internals.process_handle.take() {
            let _ = handle.kill();
        }

        // Drop channels
        internals.input_tx = None;
        internals.output_rx = None;
        internals.pty_master = None;
        drop(internals);

        // Broadcast killed event
        let _ = self.event_broadcaster.send(SessionEvent::Killed {
            session_id: self.id.clone(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        });

        Ok(())
    }

    /// Get runtime duration
    pub fn runtime(&self) -> Duration {
        Utc::now().signed_duration_since(self.created_at).to_std().unwrap_or_default()
    }

    /// Check if session is in terminal state
    pub async fn is_terminal(&self) -> bool {
        self.get_state().await.is_terminal()
    }

    /// Check if session is in active state (can be reused)
    pub async fn is_active(&self) -> bool {
        self.get_state().await.is_active()
    }

    /// Check if session can accept new tasks
    pub async fn can_accept_task(&self) -> bool {
        self.get_state().await.can_accept_task()
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        // Note: We can't use async Mutex in Drop, so we rely on async shutdown() 
        // being called before drop. This is a fallback safety net.
        // The process will be killed when the Child handle is dropped anyway.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_session() -> AgentSession {
        let event_broadcaster = EventBroadcaster::new(100);
        AgentSession::new(
            SessionId::new("test-run", "test-task"),
            AgentType::OpenCode,
            PathBuf::from("/tmp"),
            "test prompt".to_string(),
            "".to_string(),
            event_broadcaster,
            1000,
        )
    }

    #[test]
    fn test_persistent_mode_setters() {
        let mut session = create_test_session();

        // Default values
        assert!(!session.is_persistent());
        assert_eq!(session.max_idle_secs(), 0);

        // Set persistent mode
        session.set_persistent(true);
        assert!(session.is_persistent());

        session.set_persistent(false);
        assert!(!session.is_persistent());

        // Set max idle seconds
        session.set_max_idle_secs(3600);
        assert_eq!(session.max_idle_secs(), 3600);

        session.set_max_idle_secs(0);
        assert_eq!(session.max_idle_secs(), 0);
    }

    #[tokio::test]
    async fn test_should_auto_destroy_non_persistent() {
        let session = create_test_session();

        // Non-persistent session should not auto-destroy initially
        assert!(!session.should_auto_destroy().await);

        // Mark as completed
        session
            .mark_completed("output", 0)
            .await;
        assert!(session.should_auto_destroy().await);
    }

    #[tokio::test]
    async fn test_should_auto_destroy_persistent() {
        let mut session = create_test_session();
        session.set_persistent(true);

        // Persistent session should not auto-destroy even after completion
        session.mark_completed("output", 0).await;
        assert!(!session.should_auto_destroy().await);

        // But should auto-destroy if idle timeout is exceeded
        session.set_max_idle_secs(1);
        // Wait for timeout
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(session.should_auto_destroy().await);
    }

    #[tokio::test]
    async fn test_touch_updates_activity() {
        let session = create_test_session();

        let before = *session.last_activity.read().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        session.touch().await;
        let after = *session.last_activity.read().await;

        assert!(after > before);
    }

    #[tokio::test]
    async fn test_session_state_helpers() {
        assert!(SessionState::Idle.is_active());
        assert!(SessionState::RunningDetached.is_active());
        assert!(!SessionState::Completed {
            output: "".to_string(),
            exit_code: 0,
        }
        .is_active());
        assert!(!SessionState::Failed {
            error: "".to_string(),
        }
        .is_active());

        assert!(SessionState::Idle.can_accept_task());
        assert!(!SessionState::RunningDetached.can_accept_task());
        assert!(!SessionState::Completed {
            output: "".to_string(),
            exit_code: 0,
        }
        .can_accept_task());

        assert!(SessionState::Completed {
            output: "".to_string(),
            exit_code: 0,
        }
        .is_terminal());
        assert!(SessionState::Failed {
            error: "".to_string(),
        }
        .is_terminal());
        assert!(SessionState::Killed.is_terminal());
        assert!(!SessionState::Idle.is_terminal());
        assert!(!SessionState::RunningDetached.is_terminal());
    }

    #[tokio::test]
    async fn test_mark_idle() {
        let mut session = create_test_session();
        
        // Non-persistent session should not go to Idle
        session.mark_idle().await;
        assert!(!matches!(session.get_state().await, SessionState::Idle));

        // Persistent session should go to Idle
        session.set_persistent(true);
        session.mark_idle().await;
        assert!(matches!(session.get_state().await, SessionState::Idle));
    }

    #[tokio::test]
    async fn test_pause_and_resume() {
        let session = create_test_session();

        // Start session first
        session.set_state(SessionState::RunningDetached).await;

        // Pause
        session.mark_paused().await;
        assert!(matches!(session.get_state().await, SessionState::Paused));

        // Resume
        session.resume().await.unwrap();
        assert!(matches!(session.get_state().await, SessionState::RunningDetached));

        // Cannot resume from non-paused state
        assert!(session.resume().await.is_err());
    }

    #[tokio::test]
    async fn test_can_accept_task() {
        let session = create_test_session();
        
        // Initially cannot accept task (Spawning state)
        assert!(!session.can_accept_task().await);

        // Set to Idle
        session.set_state(SessionState::Idle).await;
        assert!(session.can_accept_task().await);

        // Other states cannot accept
        session.set_state(SessionState::RunningDetached).await;
        assert!(!session.can_accept_task().await);
    }
}
