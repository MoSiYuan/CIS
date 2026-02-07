//! # Session Events
//!
//! Event system for Agent Cluster sessions using tokio broadcast channels.
//! Supports CLI/GUI/API layers subscribing to session updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::agent::AgentType;
use crate::agent::cluster::SessionId;

/// Session lifecycle events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// New session created
    Created {
        session_id: SessionId,
        summary: SessionSummary,
        timestamp: DateTime<Utc>,
    },
    /// Session output updated
    OutputUpdated {
        session_id: SessionId,
        data: String,
        timestamp: DateTime<Utc>,
    },
    /// Session state changed
    StateChanged {
        session_id: SessionId,
        old_state: SessionState,
        new_state: SessionState,
        timestamp: DateTime<Utc>,
    },
    /// Session completed (success)
    Completed {
        session_id: SessionId,
        output: String,
        exit_code: i32,
        timestamp: DateTime<Utc>,
    },
    /// Session failed
    Failed {
        session_id: SessionId,
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Session blocked (waiting for human intervention)
    Blocked {
        session_id: SessionId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    /// Session recovered from blocked state
    Recovered {
        session_id: SessionId,
        timestamp: DateTime<Utc>,
    },
    /// User attached to session
    Attached {
        session_id: SessionId,
        user: String,
        timestamp: DateTime<Utc>,
    },
    /// User detached from session
    Detached {
        session_id: SessionId,
        user: String,
        timestamp: DateTime<Utc>,
    },
    /// Session killed
    Killed {
        session_id: SessionId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl SessionEvent {
    /// Get session ID from event
    pub fn session_id(&self) -> &SessionId {
        match self {
            SessionEvent::Created { session_id, .. } => session_id,
            SessionEvent::OutputUpdated { session_id, .. } => session_id,
            SessionEvent::StateChanged { session_id, .. } => session_id,
            SessionEvent::Completed { session_id, .. } => session_id,
            SessionEvent::Failed { session_id, .. } => session_id,
            SessionEvent::Blocked { session_id, .. } => session_id,
            SessionEvent::Recovered { session_id, .. } => session_id,
            SessionEvent::Attached { session_id, .. } => session_id,
            SessionEvent::Detached { session_id, .. } => session_id,
            SessionEvent::Killed { session_id, .. } => session_id,
        }
    }

    /// Get timestamp from event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            SessionEvent::Created { timestamp, .. } => *timestamp,
            SessionEvent::OutputUpdated { timestamp, .. } => *timestamp,
            SessionEvent::StateChanged { timestamp, .. } => *timestamp,
            SessionEvent::Completed { timestamp, .. } => *timestamp,
            SessionEvent::Failed { timestamp, .. } => *timestamp,
            SessionEvent::Blocked { timestamp, .. } => *timestamp,
            SessionEvent::Recovered { timestamp, .. } => *timestamp,
            SessionEvent::Attached { timestamp, .. } => *timestamp,
            SessionEvent::Detached { timestamp, .. } => *timestamp,
            SessionEvent::Killed { timestamp, .. } => *timestamp,
        }
    }
}

/// Session state (for events)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Initial spawning state
    Spawning,
    /// Running but not attached
    RunningDetached,
    /// Someone is attached
    Attached { user: String },
    /// Blocked waiting for human intervention
    Blocked { reason: String },
    /// Completed successfully
    Completed { output: String, exit_code: i32 },
    /// Failed with error
    Failed { error: String },
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Spawning => write!(f, "spawning"),
            SessionState::RunningDetached => write!(f, "running"),
            SessionState::Attached { user } => write!(f, "attached({})", user),
            SessionState::Blocked { reason } => write!(f, "blocked: {}", reason),
            SessionState::Completed { exit_code, .. } => write!(f, "completed({})", exit_code),
            SessionState::Failed { error } => write!(f, "failed: {}", error),
        }
    }
}

/// Session summary for listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Full session ID
    pub id: String,
    /// Short display ID (run_id[:8]:task_id)
    pub short_id: String,
    /// DAG run ID
    pub dag_run_id: String,
    /// Task ID
    pub task_id: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Current state string
    pub state: String,
    /// Runtime duration
    pub runtime_secs: u64,
    /// Output preview (last N lines)
    pub output_preview: String,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Event broadcaster for session events
#[derive(Debug, Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<SessionEvent>,
}

impl EventBroadcaster {
    /// Create new broadcaster with capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.sender.subscribe()
    }

    /// Send event (broadcast to all subscribers)
    pub fn send(&self, event: SessionEvent) -> Result<usize, broadcast::error::SendError<SessionEvent>> {
        self.sender.send(event)
    }

    /// Get number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_broadcaster() {
        let broadcaster = EventBroadcaster::new(10);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();

        let session_id = SessionId::new("run-123", "task-456");
        let event = SessionEvent::Created {
            session_id: session_id.clone(),
            summary: SessionSummary {
                id: session_id.to_string(),
                short_id: session_id.short(),
                dag_run_id: "run-123".to_string(),
                task_id: "task-456".to_string(),
                agent_type: AgentType::OpenCode,
                state: "spawning".to_string(),
                runtime_secs: 0,
                output_preview: String::new(),
                created_at: Utc::now(),
            },
            timestamp: Utc::now(),
        };

        // Send event
        let sent = broadcaster.send(event.clone()).unwrap();
        assert_eq!(sent, 2); // 2 receivers

        // Both receivers should get the event
        assert_eq!(rx1.try_recv().unwrap().session_id(), &session_id);
        assert_eq!(rx2.try_recv().unwrap().session_id(), &session_id);
    }
}
