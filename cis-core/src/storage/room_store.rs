//! # Room Store
//!
//! Per-room SQLite storage for Matrix events.
//! Each room has its own SQLite database file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

use crate::error::{CisError, Result};
use crate::matrix::nucleus::RoomId;

use super::room_types::{
    EventFilter, PaginatedEvents, Pagination, RoomEvent, RoomStats,
    SortDirection, StoredEvent, SyncPosition,
};

/// Schema version for migrations
const SCHEMA_VERSION: i32 = 1;

/// Room storage - one SQLite database per room
pub struct RoomStore {
    /// Room ID
    room_id: RoomId,
    /// Database connection
    db: Arc<Mutex<Connection>>,
    /// Database file path
    path: PathBuf,
    /// Event broadcaster
    event_tx: broadcast::Sender<RoomEvent>,
    /// Event counter for WAL checkpoint
    event_count: Arc<Mutex<usize>>,
    /// WAL checkpoint interval
    checkpoint_interval: usize,
}

impl std::fmt::Debug for RoomStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoomStore")
            .field("room_id", &self.room_id)
            .field("path", &self.path)
            .finish()
    }
}

impl RoomStore {
    /// Open or create room store at given path
    pub async fn open(room_id: RoomId, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                CisError::storage(format!("Failed to create room directory: {}", e))
            })?;
        }

        // Open database in blocking task
        let path_clone = path.clone();
        let db = tokio::task::spawn_blocking(move || {
            Connection::open(&path_clone).map_err(|e| {
                CisError::storage(format!("Failed to open room database: {}", e))
            })
        })
        .await
        .map_err(|e| CisError::execution(format!("DB open task failed: {}", e)))??;

        // Initialize schema
        let store = Self {
            room_id,
            db: Arc::new(Mutex::new(db)),
            path,
            event_tx: broadcast::channel(1024).0,
            event_count: Arc::new(Mutex::new(0)),
            checkpoint_interval: 1000,
        };

        store.init_schema().await?;

        info!("RoomStore opened for {}", store.room_id);
        Ok(store)
    }

    /// Get room ID
    pub fn room_id(&self) -> &RoomId {
        &self.room_id
    }

    /// Get database path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Subscribe to room events
    pub fn subscribe(&self) -> broadcast::Receiver<RoomEvent> {
        self.event_tx.subscribe()
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<()> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();

            // Enable WAL mode
            conn.pragma_update(None, "journal_mode", "WAL")?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;

            // Create events table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_id TEXT NOT NULL UNIQUE,
                    room_id TEXT NOT NULL,
                    sender TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    content TEXT NOT NULL,
                    origin_server_ts INTEGER NOT NULL,
                    received_at INTEGER NOT NULL
                )",
                [],
            )?;

            // Create indexes
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_events_sender ON events(sender)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(origin_server_ts)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_events_type_timestamp ON events(event_type, origin_server_ts)",
                [],
            )?;

            // Create metadata table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS room_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                )",
                [],
            )?;

            // Create sync positions table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS sync_positions (
                    node_id TEXT PRIMARY KEY,
                    last_event_id TEXT NOT NULL,
                    last_timestamp INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )",
                [],
            )?;

            // Insert schema version
            conn.execute(
                "INSERT OR REPLACE INTO room_metadata (key, value) VALUES ('schema_version', ?)",
                params![SCHEMA_VERSION.to_string()],
            )?;

            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Schema init failed: {}", e)))??;

        Ok(())
    }

    /// Store a single event
    pub async fn store_event(&self, event: &StoredEvent) -> Result<()> {
        // Clone for storage and broadcast
        let event_for_storage = event.clone();
        let event_for_broadcast = event.clone();
        
        let db = self.db.clone();
        let event_tx = self.event_tx.clone();
        let counter = self.event_count.clone();
        let checkpoint_interval = self.checkpoint_interval;

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();

            conn.execute(
                "INSERT OR REPLACE INTO events 
                 (event_id, room_id, sender, event_type, content, origin_server_ts, received_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    event_for_storage.event_id,
                    event_for_storage.room_id,
                    event_for_storage.sender,
                    event_for_storage.event_type,
                    event_for_storage.content,
                    event_for_storage.origin_server_ts,
                    event_for_storage.received_at
                ],
            )?;

            // Check WAL checkpoint
            let mut count = counter.blocking_lock();
            *count += 1;
            if *count >= checkpoint_interval {
                conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;
                *count = 0;
            }

            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Store event failed: {}", e)))??;

        // Broadcast event after successful storage
        let _ = event_tx.send(RoomEvent::EventStored(event_for_broadcast));

        Ok(())
    }

    /// Store multiple events in a transaction
    pub async fn store_events(&self, events: &[StoredEvent]) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let db = self.db.clone();
        let events: Vec<_> = events.to_vec();
        let event_tx = self.event_tx.clone();

        // Clone for broadcasting after move
        let events_for_broadcast: Vec<_> = events.iter().cloned().collect();
        
        let count = tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let tx = conn.unchecked_transaction()?;

            let mut inserted = 0;
            for event in events {
                match tx.execute(
                    "INSERT OR REPLACE INTO events 
                     (event_id, room_id, sender, event_type, content, origin_server_ts, received_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        event.event_id,
                        event.room_id,
                        event.sender,
                        event.event_type,
                        event.content,
                        event.origin_server_ts,
                        event.received_at
                    ],
                ) {
                    Ok(_) => inserted += 1,
                    Err(e) => warn!("Failed to insert event {}: {}", event.event_id, e),
                }
            }

            tx.commit()?;

            // WAL checkpoint
            conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;

            Ok::<usize, CisError>(inserted)
        })
        .await
        .map_err(|e| CisError::execution(format!("Store events failed: {}", e)))??;

        // Broadcast events
        for event in events_for_broadcast {
            let _ = event_tx.send(RoomEvent::EventStored(event));
        }

        Ok(count)
    }

    /// Query events with filter and pagination
    pub async fn query_events(
        &self,
        filter: EventFilter,
        pagination: Pagination,
    ) -> Result<PaginatedEvents> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();

            // Build WHERE clause
            let (where_clause, params) = filter.build_where();

            // Determine sort order
            let order = match pagination.direction {
                SortDirection::Forward => "ASC",
                SortDirection::Backward => "DESC",
            };

            // Build query
            let mut query = format!(
                "SELECT id, event_id, room_id, sender, event_type, content, origin_server_ts, received_at
                 FROM events WHERE {} ORDER BY origin_server_ts {}",
                where_clause, order
            );

            // Add cursor conditions
            if let Some(ref before) = pagination.before {
                query.push_str(&format!(
                    " AND origin_server_ts < (SELECT origin_server_ts FROM events WHERE event_id = '{}')",
                    before
                ));
            }
            if let Some(ref after) = pagination.after {
                query.push_str(&format!(
                    " AND origin_server_ts > (SELECT origin_server_ts FROM events WHERE event_id = '{}')",
                    after
                ));
            }

            // Add limit
            query.push_str(&format!(" LIMIT {}", pagination.limit + 1)); // +1 to check has_more

            // Execute query
            let mut stmt = conn.prepare(&query)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();

            let events: std::result::Result<Vec<StoredEvent>, rusqlite::Error> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    Ok(StoredEvent {
                        id: row.get(0)?,
                        event_id: row.get(1)?,
                        room_id: row.get(2)?,
                        sender: row.get(3)?,
                        event_type: row.get(4)?,
                        content: row.get(5)?,
                        origin_server_ts: row.get(6)?,
                        received_at: row.get(7)?,
                    })
                })?.collect();
            let events = events?;

            // Check if has more
            let has_more = events.len() > pagination.limit;
            let events: Vec<_> = events.into_iter().take(pagination.limit).collect();

            // Generate tokens
            let next_token = if has_more {
                events.last().map(|e| e.event_id.clone())
            } else {
                None
            };

            let prev_token = if pagination.before.is_some() || pagination.after.is_some() {
                events.first().map(|e| e.event_id.clone())
            } else {
                None
            };

            Ok(PaginatedEvents {
                events,
                next_token,
                prev_token,
                total_count: None, // Could add COUNT(*) query if needed
            })
        })
        .await
        .map_err(|e| CisError::execution(format!("Query events failed: {}", e)))?
    }

    /// Get latest N events
    pub async fn get_latest_events(&self, n: usize) -> Result<Vec<StoredEvent>> {
        self.query_events(
            EventFilter::new(),
            Pagination::new(n).backward(),
        )
        .await
        .map(|r| r.events)
    }

    /// Get events since a specific event ID
    pub async fn get_events_since(
        &self,
        since_event_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<StoredEvent>> {
        let filter = if let Some(event_id) = since_event_id {
            // Get timestamp of the reference event
            let ts = self.get_event_timestamp(event_id).await?;
            EventFilter::new().with_time_range(ts.unwrap_or(0), i64::MAX)
        } else {
            EventFilter::new()
        };

        self.query_events(filter, Pagination::new(limit).forward())
            .await
            .map(|r| r.events)
    }

    /// Get event timestamp by ID
    async fn get_event_timestamp(&self, event_id: &str) -> Result<Option<i64>> {
        let db = self.db.clone();
        let event_id = event_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let ts: Option<i64> = conn
                .query_row(
                    "SELECT origin_server_ts FROM events WHERE event_id = ?1",
                    params![event_id],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(ts)
        })
        .await
        .map_err(|e| CisError::execution(format!("Get timestamp failed: {}", e)))?
    }

    /// Get event by ID
    pub async fn get_event(&self, event_id: &str) -> Result<Option<StoredEvent>> {
        let db = self.db.clone();
        let event_id = event_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let event: Option<StoredEvent> = conn
                .query_row(
                    "SELECT id, event_id, room_id, sender, event_type, content, origin_server_ts, received_at
                     FROM events WHERE event_id = ?1",
                    params![event_id],
                    |row| {
                        Ok(StoredEvent {
                            id: row.get(0)?,
                            event_id: row.get(1)?,
                            room_id: row.get(2)?,
                            sender: row.get(3)?,
                            event_type: row.get(4)?,
                            content: row.get(5)?,
                            origin_server_ts: row.get(6)?,
                            received_at: row.get(7)?,
                        })
                    },
                )
                .optional()?;
            Ok(event)
        })
        .await
        .map_err(|e| CisError::execution(format!("Get event failed: {}", e)))?
    }

    /// Get room statistics
    pub async fn get_stats(&self) -> Result<RoomStats> {
        let db = self.db.clone();
        let db_path = self.path.clone(); // Clone path before move

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();

            // Total count
            let total_events: usize = conn
                .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;

            // Time range
            let (earliest, latest): (Option<i64>, Option<i64>) = conn.query_row(
                "SELECT MIN(origin_server_ts), MAX(origin_server_ts) FROM events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            // Event type counts - collect results before dropping conn
            let mut stmt = conn.prepare("SELECT event_type, COUNT(*) FROM events GROUP BY event_type")?;
            let type_counts: std::result::Result<HashMap<String, usize>, rusqlite::Error> = stmt
                .query_map([], |row| {
                    let event_type: String = row.get(0)?;
                    let count: usize = row.get(1)?;
                    Ok((event_type, count))
                })?.collect();
            let type_counts = type_counts?;
            drop(stmt); // Explicitly drop stmt before conn

            // DB file size - use stored path since conn.path() is unreliable
            drop(conn); // Release lock before file operation
            let db_size = std::fs::metadata(&db_path)
                .map(|m| m.len() as usize)
                .unwrap_or(0);

            Ok(RoomStats {
                total_events,
                earliest_event: earliest,
                latest_event: latest,
                event_type_counts: type_counts,
                db_size_bytes: db_size,
                created_at: earliest,
            })
        })
        .await
        .map_err(|e| CisError::execution(format!("Get stats failed: {}", e)))?
    }

    /// Update sync position for a node
    pub async fn update_sync_position(
        &self,
        node_id: &str,
        last_event_id: &str,
    ) -> Result<()> {
        let db = self.db.clone();
        let node_id_clone = node_id.to_string();
        let last_event_id_clone = last_event_id.to_string();
        let now = Utc::now().timestamp_millis();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();

            // Get timestamp of the event
            let ts: i64 = conn.query_row(
                "SELECT origin_server_ts FROM events WHERE event_id = ?1",
                params![last_event_id_clone],
                |row| row.get(0),
            )?;

            conn.execute(
                "INSERT OR REPLACE INTO sync_positions 
                 (node_id, last_event_id, last_timestamp, updated_at) 
                 VALUES (?1, ?2, ?3, ?4)",
                params![node_id_clone, last_event_id_clone, ts, now],
            )?;

            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Update sync position failed: {}", e)))??;

        let _ = self
            .event_tx
            .send(RoomEvent::SyncPositionUpdated {
                node_id: node_id.to_string(),
                event_id: last_event_id.to_string(),
            });

        Ok(())
    }

    /// Get sync position for a node
    pub async fn get_sync_position(&self, node_id: &str) -> Result<Option<SyncPosition>> {
        let db = self.db.clone();
        let node_id = node_id.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            let pos: Option<SyncPosition> = conn
                .query_row(
                    "SELECT node_id, last_event_id, last_timestamp, updated_at 
                     FROM sync_positions WHERE node_id = ?1",
                    params![node_id],
                    |row| {
                        Ok(SyncPosition {
                            node_id: row.get(0)?,
                            last_event_id: row.get(1)?,
                            last_timestamp: row.get(2)?,
                            updated_at: row.get(3)?,
                        })
                    },
                )
                .optional()?;
            Ok(pos)
        })
        .await
        .map_err(|e| CisError::execution(format!("Get sync position failed: {}", e)))?
    }

    /// Close the store
    pub async fn close(&self) -> Result<()> {
        let db = self.db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = db.blocking_lock();
            // Checkpoint before close
            conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])?;
            Ok::<(), CisError>(())
        })
        .await
        .map_err(|e| CisError::execution(format!("Close failed: {}", e)))??;

        info!("RoomStore closed for {}", self.room_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (RoomStore, TempDir) {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.db");
        let room_id = RoomId::new("!test:cis.local");
        let store = RoomStore::open(room_id, path).await.unwrap();
        (store, temp)
    }

    fn create_test_event(event_id: &str, event_type: &str) -> StoredEvent {
        StoredEvent {
            id: 0,
            event_id: event_id.to_string(),
            room_id: "!test:cis.local".to_string(),
            sender: "@alice:cis.local".to_string(),
            event_type: event_type.to_string(),
            content: r#"{"body":"test"}"#.to_string(),
            origin_server_ts: Utc::now().timestamp_millis(),
            received_at: Utc::now().timestamp_millis(),
        }
    }

    #[tokio::test]
    async fn test_store_and_query() {
        let (store, _temp) = create_test_store().await;

        // Store event
        let event = create_test_event("$event1", "m.room.message");
        store.store_event(&event).await.unwrap();

        // Query
        let result = store.query_events(EventFilter::new(), Pagination::new(10)).await.unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].event_id, "$event1");
    }

    #[tokio::test]
    async fn test_filter_by_type() {
        let (store, _temp) = create_test_store().await;

        // Store different types
        store.store_event(&create_test_event("$e1", "m.room.message")).await.unwrap();
        store.store_event(&create_test_event("$e2", "m.room.member")).await.unwrap();
        store.store_event(&create_test_event("$e3", "m.room.message")).await.unwrap();

        // Filter
        let filter = EventFilter::new().with_event_type("m.room.message");
        let result = store.query_events(filter, Pagination::new(10)).await.unwrap();
        assert_eq!(result.events.len(), 2);
    }

    #[tokio::test]
    async fn test_stats() {
        let (store, _temp) = create_test_store().await;

        store.store_event(&create_test_event("$e1", "m.room.message")).await.unwrap();
        store.store_event(&create_test_event("$e2", "m.room.member")).await.unwrap();

        let stats = store.get_stats().await.unwrap();
        assert_eq!(stats.total_events, 2);
        assert!(stats.event_type_counts.contains_key("m.room.message"));
        assert!(stats.event_type_counts.contains_key("m.room.member"));
    }
}
