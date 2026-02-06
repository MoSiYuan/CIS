//! # Room Store Types
//!
//! Type definitions for per-room SQLite storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Room event filter
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Filter by event types
    pub event_types: Option<Vec<String>>,
    /// Filter by senders
    pub senders: Option<Vec<String>>,
    /// Start timestamp (inclusive)
    pub since: Option<i64>,
    /// End timestamp (inclusive)
    pub until: Option<i64>,
    /// Content contains text (simple LIKE query)
    pub contains: Option<String>,
    /// Filter by event ID prefix
    pub event_id_prefix: Option<String>,
}

impl EventFilter {
    /// Create empty filter (matches all)
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by event type
    pub fn with_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.event_types
            .get_or_insert_with(Vec::new)
            .push(event_type.into());
        self
    }

    /// Filter by sender
    pub fn with_sender(mut self, sender: impl Into<String>) -> Self {
        self.senders
            .get_or_insert_with(Vec::new)
            .push(sender.into());
        self
    }

    /// Filter by time range
    pub fn with_time_range(mut self, since: i64, until: i64) -> Self {
        self.since = Some(since);
        self.until = Some(until);
        self
    }

    /// Filter by content contains
    pub fn with_contains(mut self, text: impl Into<String>) -> Self {
        self.contains = Some(text.into());
        self
    }

    /// Build SQL WHERE clause and parameters
    pub(crate) fn build_where(&self) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref types) = self.event_types {
            let placeholders: Vec<_> = types.iter().map(|_| "?").collect();
            clauses.push(format!("event_type IN ({})", placeholders.join(",")));
            for t in types {
                params.push(Box::new(t.clone()));
            }
        }

        if let Some(ref senders) = self.senders {
            let placeholders: Vec<_> = senders.iter().map(|_| "?").collect();
            clauses.push(format!("sender IN ({})", placeholders.join(",")));
            for s in senders {
                params.push(Box::new(s.clone()));
            }
        }

        if let Some(since) = self.since {
            clauses.push("origin_server_ts >= ?".to_string());
            params.push(Box::new(since));
        }

        if let Some(until) = self.until {
            clauses.push("origin_server_ts <= ?".to_string());
            params.push(Box::new(until));
        }

        if let Some(ref contains) = self.contains {
            clauses.push("content LIKE ?".to_string());
            params.push(Box::new(format!("%{}%", contains)));
        }

        if let Some(ref prefix) = self.event_id_prefix {
            clauses.push("event_id LIKE ?".to_string());
            params.push(Box::new(format!("{}%", prefix)));
        }

        let where_clause = if clauses.is_empty() {
            "1=1".to_string()
        } else {
            clauses.join(" AND ")
        };

        (where_clause, params)
    }
}

/// Pagination parameters
#[derive(Debug, Clone)]
pub struct Pagination {
    /// Maximum number of results
    pub limit: usize,
    /// Return events before this event ID (exclusive)
    pub before: Option<String>,
    /// Return events after this event ID (exclusive)
    pub after: Option<String>,
    /// Sort direction
    pub direction: SortDirection,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            before: None,
            after: None,
            direction: SortDirection::Backward,
        }
    }
}

impl Pagination {
    /// Create new pagination with limit
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }

    /// Set before cursor
    pub fn before(mut self, event_id: impl Into<String>) -> Self {
        self.before = Some(event_id.into());
        self
    }

    /// Set after cursor
    pub fn after(mut self, event_id: impl Into<String>) -> Self {
        self.after = Some(event_id.into());
        self
    }

    /// Set forward direction (oldest first)
    pub fn forward(mut self) -> Self {
        self.direction = SortDirection::Forward;
        self
    }

    /// Set backward direction (newest first, default)
    pub fn backward(mut self) -> Self {
        self.direction = SortDirection::Backward;
        self
    }
}

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Oldest first
    Forward,
    /// Newest first
    Backward,
}

/// Paginated events result
#[derive(Debug, Clone)]
pub struct PaginatedEvents {
    /// Events in this page
    pub events: Vec<StoredEvent>,
    /// Pagination token for next page
    pub next_token: Option<String>,
    /// Pagination token for previous page
    pub prev_token: Option<String>,
    /// Total count (if available)
    pub total_count: Option<usize>,
}

/// Stored event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Database ID
    pub id: i64,
    /// Matrix event ID
    pub event_id: String,
    /// Room ID
    pub room_id: String,
    /// Sender
    pub sender: String,
    /// Event type
    pub event_type: String,
    /// Content (JSON string)
    pub content: String,
    /// Origin server timestamp (milliseconds)
    pub origin_server_ts: i64,
    /// Received at timestamp (milliseconds)
    pub received_at: i64,
}

impl StoredEvent {
    /// Parse content as JSON
    pub fn content_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::from_str(&self.content)
    }

    /// Get received time as DateTime
    pub fn received_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.received_at)
            .unwrap_or_else(|| Utc::now())
    }

    /// Get origin server time as DateTime
    pub fn origin_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.origin_server_ts)
            .unwrap_or_else(|| Utc::now())
    }
}

/// Room statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStats {
    /// Total number of events
    pub total_events: usize,
    /// Earliest event timestamp
    pub earliest_event: Option<i64>,
    /// Latest event timestamp
    pub latest_event: Option<i64>,
    /// Event count by type
    pub event_type_counts: HashMap<String, usize>,
    /// Database file size in bytes
    pub db_size_bytes: usize,
    /// Room creation time (first event)
    pub created_at: Option<i64>,
}

/// Room metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMetadata {
    /// Room ID
    pub room_id: String,
    /// Room name (if set)
    pub name: Option<String>,
    /// Room topic (if set)
    pub topic: Option<String>,
    /// Creator
    pub creator: String,
    /// Created at
    pub created_at: i64,
    /// Last activity
    pub last_activity: i64,
    /// Is archived
    pub is_archived: bool,
    /// Federation enabled
    pub federate: bool,
}

/// Room info (for listing)
#[derive(Debug, Clone)]
pub struct RoomInfo {
    /// Room ID
    pub room_id: String,
    /// Metadata
    pub metadata: RoomMetadata,
    /// Statistics
    pub stats: RoomStats,
}

/// Sync position for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPosition {
    /// Node ID
    pub node_id: String,
    /// Last synced event ID
    pub last_event_id: String,
    /// Last synced timestamp
    pub last_timestamp: i64,
    /// Updated at
    pub updated_at: i64,
}

/// Room store configuration
#[derive(Debug, Clone)]
pub struct RoomStoreConfig {
    /// Base path for room databases
    pub base_path: std::path::PathBuf,
    /// Maximum events per query
    pub max_query_limit: usize,
    /// WAL checkpoint interval (events)
    pub wal_checkpoint_interval: usize,
    /// Auto-archive after inactivity (days)
    pub auto_archive_days: Option<u32>,
}

impl Default for RoomStoreConfig {
    fn default() -> Self {
        Self {
            base_path: crate::storage::Paths::data_dir().join("rooms"),
            max_query_limit: 1000,
            wal_checkpoint_interval: 1000,
            auto_archive_days: Some(30),
        }
    }
}

/// Room event (for broadcast)
#[derive(Debug, Clone)]
pub enum RoomEvent {
    /// New event stored
    EventStored(StoredEvent),
    /// Room archived
    RoomArchived { room_id: String },
    /// Room deleted
    RoomDeleted { room_id: String },
    /// Sync position updated
    SyncPositionUpdated { node_id: String, event_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_filter_build() {
        let filter = EventFilter::new()
            .with_event_type("m.room.message")
            .with_sender("@alice:cis.local")
            .with_contains("hello");

        let (sql, params) = filter.build_where();
        assert!(sql.contains("event_type IN"));
        assert!(sql.contains("sender IN"));
        assert!(sql.contains("content LIKE"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_pagination() {
        let pag = Pagination::new(100)
            .before("$event123")
            .forward();

        assert_eq!(pag.limit, 100);
        assert_eq!(pag.before, Some("$event123".to_string()));
        assert_eq!(pag.direction, SortDirection::Forward);
    }
}
