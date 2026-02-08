//! # Agent Session
//!
//! Manages a single Agent session with PTY support.
//! Reuses core logic from `network::agent_session` but adapted for local DAG execution.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::agent::{AgentType, AgentCommandConfig};
use crate::agent::cluster::events::{EventBroadcaster, SessionEvent, SessionState};
use crate::agent::cluster::SessionId;
use crate::error::{CisError, Result};

/// Default terminal size
#[allow(dead_code)]
const DEFAULT_COLS: u16 = 80;
#[allow(dead_code)]
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
    #[allow(dead_code)]
    max_buffer_lines: usize,
    /// Event broadcaster
    event_broadcaster: EventBroadcaster,
    /// Attached user (if any)
    attached_user: Arc<RwLock<Option<String>>>,
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
        let config = AgentCommandConfig::from_agent_type(self.agent_type)
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
    pub async fn mark_completed(&self, output: &str, exit_code: i32) {
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

    /// Mark as failed
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
    pub async fn detach(&self, user: &str) -> Result<()> {
        *self.attached_user.write().await = None;
        self.set_state(SessionState::RunningDetached).await;
        
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
        matches!(
            self.get_state().await,
            SessionState::Completed { .. } | SessionState::Failed { .. }
        )
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        // Note: We can't use async Mutex in Drop, so we rely on async shutdown() 
        // being called before drop. This is a fallback safety net.
        // The process will be killed when the Child handle is dropped anyway.
    }
}
