//! # Agent Cluster Module
//!
//! DAG Agent Cluster implementation for CIS.
//!
//! ## Features
//!
//! - Multi-Agent concurrent execution within single DAG
//! - PTY-based interactive sessions (attach/detach)
//! - Blockage detection and auto-recovery
//! - Upstream context injection for downstream tasks
//! - CLI/GUI/API three-layer support
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     SessionManager (singleton)                   │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
//! │  │ AgentSession │  │ AgentSession │  │ AgentSession │  ...      │
//! │  │  + PTY       │  │  + PTY       │  │  + PTY       │           │
//! │  └──────────────┘  └──────────────┘  └──────────────┘           │
//! │                                                                  │
//! │  Event Bus (tokio::sync::broadcast)                             │
//! └─────────────────────────────────────────────────────────────────┘
//!                              │
//!              ┌───────────────┼───────────────┐
//!              ▼               ▼               ▼
//!           CLI (PTY)     GUI (WebSocket)   API (REST)
//! ```

use std::fmt;
use serde::{Deserialize, Serialize};

pub mod context;
pub mod events;
pub mod executor;
pub mod manager;
pub mod monitor;
pub mod session;

#[cfg(test)]
pub mod opencode_migration_test;

// Re-export main types
pub use context::{build_task_prompt, ContextEntry, ContextStore, OutputFormat};
pub use events::{SessionEvent, SessionState, SessionSummary};
pub use executor::{AgentClusterConfig, AgentClusterExecutor, ExecutionReport, ExecutionStats, TaskOutput};
pub use manager::{AttachHandle, SessionManager, SessionManagerConfig};
pub use monitor::{BlockageResult, DetectionStrategy, MonitorConfig, MonitorCoordinator, SessionMonitor};
pub use session::AgentSession;

/// Session ID - unique identifier for a DAG task session
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionId {
    /// DAG run ID
    pub dag_run_id: String,
    /// Task ID within the DAG
    pub task_id: String,
}

impl SessionId {
    /// Create new session ID
    pub fn new(dag_run_id: &str, task_id: &str) -> Self {
        Self {
            dag_run_id: dag_run_id.to_string(),
            task_id: task_id.to_string(),
        }
    }

    /// Get short display format: "run_id[:8]:task_id"
    pub fn short(&self) -> String {
        let run_short = if self.dag_run_id.len() > 8 {
            &self.dag_run_id[..8]
        } else {
            &self.dag_run_id
        };
        format!("{}:{}", run_short, self.task_id)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.dag_run_id, self.task_id)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        // Parse "run_id:task_id" format
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() == 2 {
            Self::new(parts[0], parts[1])
        } else {
            // Fallback: use whole string as run_id, empty task_id
            Self::new(s, "unknown")
        }
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

/// Parse session ID from string (handles both full and short formats)
pub fn parse_session_id(s: &str) -> crate::error::Result<SessionId> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(crate::error::CisError::invalid_input(
            format!("Invalid session ID format: {}. Expected 'run_id:task_id'", s)
        ));
    }
    Ok(SessionId::new(parts[0], parts[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_creation() {
        let id = SessionId::new("run-123", "task-456");
        assert_eq!(id.dag_run_id, "run-123");
        assert_eq!(id.task_id, "task-456");
    }

    #[test]
    fn test_session_id_short() {
        let id = SessionId::new("abcdef123456", "my-task");
        assert_eq!(id.short(), "abcdef12:my-task");

        let id = SessionId::new("short", "task");
        assert_eq!(id.short(), "short:task");
    }

    #[test]
    fn test_session_id_display() {
        let id = SessionId::new("run-123", "task-456");
        assert_eq!(id.to_string(), "run-123:task-456");
    }

    #[test]
    fn test_session_id_from_str() {
        let id = SessionId::from("run-123:task-456");
        assert_eq!(id.dag_run_id, "run-123");
        assert_eq!(id.task_id, "task-456");

        // Edge case: no colon
        let id = SessionId::from("only-run-id");
        assert_eq!(id.dag_run_id, "only-run-id");
        assert_eq!(id.task_id, "unknown");
    }

    #[test]
    fn test_parse_session_id() {
        let id = parse_session_id("run-123:task-456").unwrap();
        assert_eq!(id.dag_run_id, "run-123");
        assert_eq!(id.task_id, "task-456");

        // Invalid format
        assert!(parse_session_id("invalid").is_err());
    }
}
