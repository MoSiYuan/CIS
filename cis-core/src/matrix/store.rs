//! # Matrix Event Store
//!
//! SQLite-based storage for Matrix events and metadata.
//!
//! ## Schema
//!
//! - `matrix_events`: Matrix protocol events
//! - `cis_event_meta`: CIS-specific event metadata
//! - `matrix_users`: Local user accounts (simplified auth)
//! - `matrix_devices`: Device registrations
//! - `matrix_tokens`: Access tokens
//! - `matrix_rooms`: Room metadata (with federate flag)
//! - `matrix_room_members`: Room membership tracking

use rusqlite::{Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

use super::error::{MatrixError, MatrixResult};

/// Room 创建选项
#[derive(Debug, Clone)]
pub struct RoomOptions {
    /// Room ID
    pub room_id: String,
    /// 创建者
    pub creator: String,
    /// Room 名称
    pub name: Option<String>,
    /// Room 主题
    pub topic: Option<String>,
    /// 是否联邦同步
    pub federate: bool,
    /// 创建时间戳
    pub created_at: i64,
}

impl RoomOptions {
    /// 创建新的 Room 选项
    pub fn new(room_id: impl Into<String>, creator: impl Into<String>) -> Self {
        Self {
            room_id: room_id.into(),
            creator: creator.into(),
            name: None,
            topic: None,
            federate: false,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// 设置 Room 名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置 Room 主题
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// 设置联邦同步标记
    pub fn with_federate(mut self, federate: bool) -> Self {
        self.federate = federate;
        self
    }
}

/// Matrix message/event representation
#[derive(Debug, Clone)]
pub struct MatrixMessage {
    pub id: i64,
    pub room_id: String,
    pub event_id: String,
    pub sender: String,
    pub event_type: String,
    pub content: String,
    pub origin_server_ts: i64,
    pub unsigned: Option<String>,
    pub state_key: Option<String>,
}

/// Federation event representation for storing received federation events
#[derive(Debug, Clone)]
pub struct FederationEvent {
    pub id: i64,
    pub event_id: String,
    pub sender: String,
    pub room_id: String,
    pub event_type: String,
    pub content: String,
    pub origin_server_ts: i64,
    pub received_at: i64,
    pub processed: bool,
}

/// Matrix room information
#[derive(Debug, Clone)]
pub struct MatrixRoom {
    pub room_id: String,
    pub creator: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub created_at: i64,
}

/// Matrix event store
#[derive(Debug)]
pub struct MatrixStore {
    db: Arc<Mutex<Connection>>,
}

impl MatrixStore {
    /// Open or create a Matrix store at the given path
    pub fn open(path: &str) -> MatrixResult<Self> {
        let conn = Connection::open(path)
            .map_err(|e| MatrixError::Store(format!("Failed to open database: {}", e)))?;

        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };

        store.init_schema()?;

        Ok(store)
    }

    /// Open an in-memory store (for testing)
    pub fn open_in_memory() -> MatrixResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| MatrixError::Store(format!("Failed to open in-memory database: {}", e)))?;

        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };

        store.init_schema()?;

        Ok(store)
    }

    /// Initialize database schema
    fn init_schema(&self) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // Matrix events table - stores actual Matrix protocol events
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT UNIQUE NOT NULL,
                room_id TEXT NOT NULL,
                sender TEXT NOT NULL,
                event_type TEXT NOT NULL,
                content_json TEXT,
                received_at INTEGER,
                federate INTEGER DEFAULT 0,
                origin_server_ts INTEGER,
                unsigned TEXT,
                state_key TEXT
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_events table: {}", e)))?;

        // CIS event metadata - additional CIS-specific metadata for events
        db.execute(
            "CREATE TABLE IF NOT EXISTS cis_event_meta (
                event_id TEXT PRIMARY KEY,
                room_id TEXT NOT NULL,
                source_node TEXT,
                verified BOOLEAN DEFAULT 0,
                encrypted BOOLEAN DEFAULT 0,
                category TEXT,
                tags TEXT,
                created_at INTEGER DEFAULT (unixepoch())
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create cis_event_meta table: {}", e)))?;

        // Local users table (simplified auth for Phase 0)
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_users (
                user_id TEXT PRIMARY KEY,
                display_name TEXT,
                avatar_url TEXT,
                created_at INTEGER DEFAULT (unixepoch())
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_users table: {}", e)))?;

        // Devices table
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_devices (
                device_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                display_name TEXT,
                last_seen INTEGER,
                ip_address TEXT,
                created_at INTEGER DEFAULT (unixepoch()),
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_devices table: {}", e)))?;

        // Access tokens table
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_tokens (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                device_id TEXT,
                created_at INTEGER DEFAULT (unixepoch()),
                expires_at INTEGER,
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id),
                FOREIGN KEY (device_id) REFERENCES matrix_devices(device_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_tokens table: {}", e)))?;

        // Sync tokens table (for incremental sync)
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_sync_tokens (
                user_id TEXT PRIMARY KEY,
                since TEXT NOT NULL,
                updated_at INTEGER DEFAULT (unixepoch())
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create sync_tokens table: {}", e)))?;

        // Rooms table (Phase 1) - with federate flag
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_rooms (
                room_id TEXT PRIMARY KEY,
                creator TEXT NOT NULL,
                name TEXT,
                topic TEXT,
                federate INTEGER DEFAULT 0,
                created_at INTEGER DEFAULT (unixepoch()),
                FOREIGN KEY (creator) REFERENCES matrix_users(user_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_rooms table: {}", e)))?;

        // 迁移：如果旧表没有 federate 列，添加它
        let _ = db.execute(
            "ALTER TABLE matrix_rooms ADD COLUMN federate INTEGER DEFAULT 0",
            [],
        );

        // Room members table (Phase 1) - tracks membership state
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_room_members (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                room_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                membership TEXT NOT NULL DEFAULT 'join',
                display_name TEXT,
                avatar_url TEXT,
                joined_at INTEGER DEFAULT (unixepoch()),
                updated_at INTEGER DEFAULT (unixepoch()),
                UNIQUE(room_id, user_id),
                FOREIGN KEY (room_id) REFERENCES matrix_rooms(room_id),
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create matrix_room_members table: {}", e)))?;

        // DID 身份与信任网络
        db.execute(
            "CREATE TABLE IF NOT EXISTS did_trust (
                trustor TEXT,
                trustee TEXT,
                trust_level INTEGER CHECK(trust_level IN (0,1,2)),
                PRIMARY KEY (trustor, trustee)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create did_trust table: {}", e)))?;

        // 网络节点状态（WebSocket 联邦视图）
        db.execute(
            "CREATE TABLE IF NOT EXISTS network_peers (
                node_id TEXT PRIMARY KEY,
                endpoint_ws TEXT,
                status INTEGER,
                last_seen INTEGER,
                rtt_ms INTEGER
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create network_peers table: {}", e)))?;

        // 断线同步队列
        db.execute(
            "CREATE TABLE IF NOT EXISTS pending_sync (
                target_node TEXT,
                room_id TEXT,
                since_event_id TEXT,
                priority INTEGER,
                PRIMARY KEY (target_node, room_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create pending_sync table: {}", e)))?;

        // Create indexes
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_events_room ON matrix_events(room_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_events_sender ON matrix_events(sender)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_events_ts ON matrix_events(origin_server_ts)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_devices_user ON matrix_devices(user_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_room_members_room ON matrix_room_members(room_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_room_members_user ON matrix_room_members(user_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_matrix_room_members_membership ON matrix_room_members(membership)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        // Federation events table for deduplication and cleanup
        db.execute(
            "CREATE TABLE IF NOT EXISTS federation_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT UNIQUE NOT NULL,
                sender TEXT NOT NULL,
                room_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                content TEXT NOT NULL,
                origin_server_ts INTEGER NOT NULL,
                received_at INTEGER DEFAULT (unixepoch()),
                processed BOOLEAN DEFAULT FALSE
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create federation_events table: {}", e)))?;

        // Indexes for federation_events table
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_events_event_id ON federation_events(event_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create federation_events index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_events_room ON federation_events(room_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create federation_events room index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_events_received_at ON federation_events(received_at)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create federation_events received_at index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_events_processed ON federation_events(processed)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create federation_events processed index: {}", e)))?;

        Ok(())
    }

    /// Create a new user if not exists
    pub fn ensure_user(&self, user_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT OR IGNORE INTO matrix_users (user_id) VALUES (?1)",
            [user_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to create user: {}", e)))?;

        Ok(())
    }

    /// Store an access token
    pub fn store_token(&self, token: &str, user_id: &str, device_id: Option<&str>) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT INTO matrix_tokens (token, user_id, device_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![token, user_id, device_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to store token: {}", e)))?;

        Ok(())
    }

    /// Validate an access token and return user_id
    pub fn validate_token(&self, token: &str) -> MatrixResult<Option<String>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<String>, rusqlite::Error> = db.query_row(
            "SELECT user_id FROM matrix_tokens WHERE token = ?1 AND (expires_at IS NULL OR expires_at > unixepoch())",
            [token],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(user_id) => Ok(user_id),
            Err(e) => Err(MatrixError::Store(format!("Failed to validate token: {}", e))),
        }
    }

    /// Register or update a device
    pub fn register_device(&self, device_id: &str, user_id: &str, display_name: Option<&str>) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT INTO matrix_devices (device_id, user_id, display_name) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(device_id) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, display_name)",
            rusqlite::params![device_id, user_id, display_name],
        ).map_err(|e| MatrixError::Store(format!("Failed to register device: {}", e)))?;

        Ok(())
    }

    /// Check if a user exists
    pub fn user_exists(&self, user_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM matrix_users WHERE user_id = ?1",
            [user_id],
            |row| row.get(0),
        ).map_err(|e| MatrixError::Store(format!("Failed to check user existence: {}", e)))?;

        Ok(count > 0)
    }

    /// Register a new user
    pub fn register_user(&self, user_id: &str, _access_token: &str, device_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // Insert user
        db.execute(
            "INSERT INTO matrix_users (user_id, display_name, created_at) 
             VALUES (?1, ?1, unixepoch())",
            [user_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to register user: {}", e)))?;

        // Store access token in matrix_devices (add token column if needed)
        db.execute(
            "INSERT INTO matrix_devices (device_id, user_id, display_name, last_seen) 
             VALUES (?1, ?2, 'Element Client', unixepoch())
             ON CONFLICT(device_id) DO UPDATE SET
             last_seen = unixepoch()",
            [device_id, user_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to store device token: {}", e)))?;

        Ok(())
    }

    /// Get database connection for direct access
    pub fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db.lock().expect("Failed to lock database")
    }

    // ==================== Phase 1: Room and Message APIs ====================

    /// Get all rooms a user has joined
    pub fn get_joined_rooms(&self, user_id: &str) -> MatrixResult<Vec<String>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT room_id FROM matrix_room_members 
             WHERE user_id = ?1 AND membership = 'join'"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let room_ids: Result<Vec<String>, rusqlite::Error> = stmt
            .query_map([user_id], |row| row.get(0))
            .map_err(|e| MatrixError::Store(format!("Failed to query rooms: {}", e)))?
            .collect();

        room_ids.map_err(|e| MatrixError::Store(format!("Failed to collect rooms: {}", e)))
    }

    /// Get messages for a room since a given timestamp
    pub fn get_room_messages(
        &self,
        room_id: &str,
        since_ts: i64,
        limit: usize,
    ) -> MatrixResult<Vec<MatrixMessage>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT id, room_id, event_id, sender, event_type, content_json AS content, 
                    origin_server_ts, unsigned, state_key
             FROM matrix_events 
             WHERE room_id = ?1 AND origin_server_ts > ?2
             ORDER BY origin_server_ts ASC
             LIMIT ?3"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let messages = stmt
            .query_map(
                rusqlite::params![room_id, since_ts, limit],
                |row| {
                    Ok(MatrixMessage {
                        id: row.get(0)?,
                        room_id: row.get(1)?,
                        event_id: row.get(2)?,
                        sender: row.get(3)?,
                        event_type: row.get(4)?,
                        content: row.get(5)?,
                        origin_server_ts: row.get(6)?,
                        unsigned: row.get(7)?,
                        state_key: row.get(8)?,
                    })
                },
            )
            .map_err(|e| MatrixError::Store(format!("Failed to query messages: {}", e)))?;

        let mut result = Vec::new();
        for msg in messages {
            result.push(msg.map_err(|e| MatrixError::Store(format!("Failed to read message: {}", e)))?);
        }

        Ok(result)
    }

    /// Save a Matrix event (flexible version for Phase 1)
    #[allow(clippy::too_many_arguments)]
    pub fn save_event(
        &self,
        room_id: &str,
        event_id: &str,
        sender: &str,
        event_type: &str,
        content: &str,
        origin_server_ts: i64,
        unsigned: Option<&str>,
        state_key: Option<&str>,
    ) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT INTO matrix_events 
             (room_id, event_id, sender, event_type, content_json, origin_server_ts, unsigned, state_key) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(event_id) DO UPDATE SET
             content_json = excluded.content_json,
             origin_server_ts = excluded.origin_server_ts",
            rusqlite::params![
                room_id,
                event_id,
                sender,
                event_type,
                content,
                origin_server_ts,
                unsigned,
                state_key,
            ],
        ).map_err(|e| MatrixError::Store(format!("Failed to save event: {}", e)))?;

        Ok(())
    }

    /// Create a new room (原始方法，保持兼容)
    pub fn create_room(
        &self,
        room_id: &str,
        creator: &str,
        name: Option<&str>,
        topic: Option<&str>,
    ) -> MatrixResult<()> {
        self.create_room_with_opts(&RoomOptions {
            room_id: room_id.to_string(),
            creator: creator.to_string(),
            name: name.map(|s| s.to_string()),
            topic: topic.map(|s| s.to_string()),
            federate: false,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    /// 创建 Room（带 federate 标记）
    pub fn create_room_with_opts(&self, opts: &RoomOptions) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // Ensure creator exists
        db.execute(
            "INSERT OR IGNORE INTO matrix_users (user_id) VALUES (?1)",
            [&opts.creator],
        ).map_err(|e| MatrixError::Store(format!("Failed to ensure user: {}", e)))?;

        // Insert room with federate flag
        db.execute(
            "INSERT INTO matrix_rooms (room_id, creator, name, topic, federate, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(room_id) DO UPDATE SET
             name = excluded.name,
             topic = excluded.topic,
             federate = excluded.federate",
            rusqlite::params![
                opts.room_id,
                opts.creator,
                opts.name,
                opts.topic,
                opts.federate as i32,
                opts.created_at,
            ],
        ).map_err(|e| MatrixError::Store(format!("Failed to create room: {}", e)))?;

        Ok(())
    }

    /// 获取 Room 的 federate 标记
    pub fn is_room_federate(&self, room_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<i32>, rusqlite::Error> = db.query_row(
            "SELECT federate FROM matrix_rooms WHERE room_id = ?1",
            [room_id],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(Some(federate)) => Ok(federate != 0),
            Ok(None) => Ok(false), // Room 不存在时默认不联邦
            Err(e) => Err(MatrixError::Store(format!("Failed to check federate: {}", e))),
        }
    }

    /// 列出所有 federate=true 的 Room
    pub fn list_federate_rooms(&self) -> MatrixResult<Vec<String>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT room_id FROM matrix_rooms WHERE federate = 1"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let rooms = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| MatrixError::Store(format!("Failed to query rooms: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| MatrixError::Store(format!("Failed to collect rooms: {}", e)))?;

        Ok(rooms)
    }

    /// 设置 Room 的 federate 标记
    pub fn set_room_federate(&self, room_id: &str, federate: bool) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "UPDATE matrix_rooms SET federate = ?1 WHERE room_id = ?2",
            rusqlite::params![federate as i32, room_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to set federate: {}", e)))?;

        Ok(())
    }

    /// Add a user to a room
    pub fn join_room(&self, room_id: &str, user_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // Ensure user exists
        db.execute(
            "INSERT OR IGNORE INTO matrix_users (user_id) VALUES (?1)",
            [user_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to ensure user: {}", e)))?;

        // Add or update membership
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        db.execute(
            "INSERT INTO matrix_room_members (room_id, user_id, membership, joined_at, updated_at) 
             VALUES (?1, ?2, 'join', ?3, ?3)
             ON CONFLICT(room_id, user_id) DO UPDATE SET
             membership = 'join',
             updated_at = excluded.updated_at",
            rusqlite::params![room_id, user_id, now],
        ).map_err(|e| MatrixError::Store(format!("Failed to join room: {}", e)))?;

        Ok(())
    }

    /// Check if a user is in a room
    pub fn is_user_in_room(&self, room_id: &str, user_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<i64>, rusqlite::Error> = db.query_row(
            "SELECT 1 FROM matrix_room_members 
             WHERE room_id = ?1 AND user_id = ?2 AND membership = 'join'",
            rusqlite::params![room_id, user_id],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(MatrixError::Store(format!("Failed to check membership: {}", e))),
        }
    }

    /// Check if a room exists
    pub fn room_exists(&self, room_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<i64>, rusqlite::Error> = db.query_row(
            "SELECT 1 FROM matrix_rooms WHERE room_id = ?1",
            [room_id],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(MatrixError::Store(format!("Failed to check room: {}", e))),
        }
    }

    /// Check if an event exists by event_id
    pub fn event_exists(&self, event_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<i64>, rusqlite::Error> = db.query_row(
            "SELECT 1 FROM matrix_events WHERE event_id = ?1",
            [event_id],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(MatrixError::Store(format!("Failed to check event: {}", e))),
        }
    }

    /// Save a raw event (for federation sync)
    pub fn save_raw_event(
        &self,
        room_id: &str,
        event_id: &str,
        event_type: &str,
        content: &serde_json::Value,
        origin_server_ts: i64,
    ) -> MatrixResult<()> {
        let content_str = content.to_string();
        
        // Extract sender from content if available, otherwise use empty
        let sender = content.get("sender")
            .and_then(|s| s.as_str())
            .unwrap_or("@unknown:cis.local");
        
        // Extract unsigned if available
        let unsigned = content.get("unsigned").map(|u| u.to_string());
        
        // Extract state_key if available
        let state_key = content.get("state_key").and_then(|s| s.as_str());

        self.save_event(
            room_id,
            event_id,
            sender,
            event_type,
            &content_str,
            origin_server_ts,
            unsigned.as_deref(),
            state_key,
        )
    }

    /// Get room state (state events)
    pub fn get_room_state(&self, room_id: &str) -> MatrixResult<Vec<(String, String, String, String)>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT event_type, COALESCE(state_key, ''), sender, content
             FROM matrix_events 
             WHERE room_id = ?1 AND state_key IS NOT NULL
             ORDER BY origin_server_ts ASC"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let state = stmt
            .query_map([room_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| MatrixError::Store(format!("Failed to query state: {}", e)))?;

        let mut result = Vec::new();
        for event in state {
            result.push(event.map_err(|e| MatrixError::Store(format!("Failed to read state: {}", e)))?);
        }

        Ok(result)
    }

    /// Get events for a room since a specific event ID (for sync pagination)
    pub fn get_events_since_event_id(
        &self,
        room_id: &str,
        since_event_id: &str,
        limit: i64,
    ) -> MatrixResult<Vec<MatrixMessage>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // First, get the timestamp of the since_event_id
        let since_ts: Option<i64> = db.query_row(
            "SELECT origin_server_ts FROM matrix_events WHERE event_id = ?1 AND room_id = ?2",
            rusqlite::params![since_event_id, room_id],
            |row| row.get(0),
        ).optional().map_err(|e| MatrixError::Store(format!("Failed to query since event: {}", e)))?;

        // If the event doesn't exist, return empty (client will need to do full sync)
        let since_ts = match since_ts {
            Some(ts) => ts,
            None => {
                tracing::warn!("Since event {} not found in room {}, returning empty result", 
                    since_event_id, room_id);
                return Ok(Vec::new());
            }
        };

        // Get events after this timestamp
        let mut stmt = db.prepare(
            "SELECT id, room_id, event_id, sender, event_type, content_json, 
                    origin_server_ts, unsigned, state_key
             FROM matrix_events 
             WHERE room_id = ?1 AND origin_server_ts > ?2
             ORDER BY origin_server_ts ASC
             LIMIT ?3"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let messages = stmt
            .query_map(
                rusqlite::params![room_id, since_ts, limit],
                |row| {
                    Ok(MatrixMessage {
                        id: row.get(0)?,
                        room_id: row.get(1)?,
                        event_id: row.get(2)?,
                        sender: row.get(3)?,
                        event_type: row.get(4)?,
                        content: row.get(5)?,
                        origin_server_ts: row.get(6)?,
                        unsigned: row.get(7)?,
                        state_key: row.get(8)?,
                    })
                },
            )
            .map_err(|e| MatrixError::Store(format!("Failed to query messages: {}", e)))?;

        let mut result = Vec::new();
        for msg in messages {
            result.push(msg.map_err(|e| MatrixError::Store(format!("Failed to read message: {}", e)))?);
        }

        Ok(result)
    }

    // ==================== Federation Event Storage ====================

    /// Store a federation event with deduplication (ON CONFLICT IGNORE)
    ///
    /// Returns true if the event was inserted, false if it was a duplicate
    pub fn store_federation_event(
        &self,
        event_id: &str,
        sender: &str,
        room_id: &str,
        event_type: &str,
        content: &str,
        origin_server_ts: i64,
    ) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let rows_affected = db.execute(
            "INSERT OR IGNORE INTO federation_events 
             (event_id, sender, room_id, event_type, content, origin_server_ts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                event_id,
                sender,
                room_id,
                event_type,
                content,
                origin_server_ts,
            ],
        ).map_err(|e| MatrixError::Store(format!("Failed to store federation event: {}", e)))?;

        Ok(rows_affected > 0)
    }

    /// Check if a federation event exists by event_id
    pub fn federation_event_exists(&self, event_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<i64>, rusqlite::Error> = db.query_row(
            "SELECT 1 FROM federation_events WHERE event_id = ?1",
            [event_id],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(MatrixError::Store(format!("Failed to check federation event: {}", e))),
        }
    }

    /// Get a federation event by event_id
    pub fn get_federation_event(&self, event_id: &str) -> MatrixResult<Option<FederationEvent>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result = db.query_row(
            "SELECT id, event_id, sender, room_id, event_type, content, 
                    origin_server_ts, received_at, processed
             FROM federation_events WHERE event_id = ?1",
            [event_id],
            |row| {
                Ok(FederationEvent {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    sender: row.get(2)?,
                    room_id: row.get(3)?,
                    event_type: row.get(4)?,
                    content: row.get(5)?,
                    origin_server_ts: row.get(6)?,
                    received_at: row.get(7)?,
                    processed: row.get::<_, i32>(8)? != 0,
                })
            },
        ).optional();

        match result {
            Ok(event) => Ok(event),
            Err(e) => Err(MatrixError::Store(format!("Failed to get federation event: {}", e))),
        }
    }

    /// Mark a federation event as processed
    pub fn mark_federation_event_processed(&self, event_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "UPDATE federation_events SET processed = TRUE WHERE event_id = ?1",
            [event_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to mark event as processed: {}", e)))?;

        Ok(())
    }

    /// Get unprocessed federation events (for async processing)
    pub fn get_unprocessed_federation_events(&self, limit: usize) -> MatrixResult<Vec<FederationEvent>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT id, event_id, sender, room_id, event_type, content, 
                    origin_server_ts, received_at, processed
             FROM federation_events 
             WHERE processed = FALSE
             ORDER BY received_at ASC
             LIMIT ?1"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let events = stmt
            .query_map([limit as i64], |row| {
                Ok(FederationEvent {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    sender: row.get(2)?,
                    room_id: row.get(3)?,
                    event_type: row.get(4)?,
                    content: row.get(5)?,
                    origin_server_ts: row.get(6)?,
                    received_at: row.get(7)?,
                    processed: row.get::<_, i32>(8)? != 0,
                })
            })
            .map_err(|e| MatrixError::Store(format!("Failed to query unprocessed events: {}", e)))?;

        let mut result = Vec::new();
        for event in events {
            result.push(event.map_err(|e| MatrixError::Store(format!("Failed to read event: {}", e)))?);
        }

        Ok(result)
    }

    /// Cleanup expired federation events (older than retention_days)
    ///
    /// Returns the number of deleted events
    pub fn cleanup_expired_federation_events(&self, retention_days: i64) -> MatrixResult<usize> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // Delete events older than retention_days
        let rows_deleted = db.execute(
            "DELETE FROM federation_events 
             WHERE received_at < unixepoch() - (?1 * 86400)",
            [retention_days],
        ).map_err(|e| MatrixError::Store(format!("Failed to cleanup expired events: {}", e)))?;

        Ok(rows_deleted)
    }

    /// Get the count of federation events (optionally filtered by processed status)
    pub fn count_federation_events(&self, processed: Option<bool>) -> MatrixResult<i64> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let count: i64 = match processed {
            Some(p) => {
                db.query_row(
                    "SELECT COUNT(*) FROM federation_events WHERE processed = ?1",
                    [if p { 1 } else { 0 }],
                    |row| row.get(0),
                ).map_err(|e| MatrixError::Store(format!("Failed to count events: {}", e)))?
            }
            None => {
                db.query_row(
                    "SELECT COUNT(*) FROM federation_events",
                    [],
                    |row| row.get(0),
                ).map_err(|e| MatrixError::Store(format!("Failed to count events: {}", e)))?
            }
        };

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_creation() {
        let store = MatrixStore::open_in_memory().unwrap();
        // Store created successfully with schema initialized
        drop(store);
    }

    #[test]
    fn test_user_management() {
        let store = MatrixStore::open_in_memory().unwrap();
        
        store.ensure_user("@alice:cis.local").unwrap();
        // User created successfully
    }

    #[test]
    fn test_token_management() {
        let store = MatrixStore::open_in_memory().unwrap();
        
        // Create user first
        store.ensure_user("@bob:cis.local").unwrap();
        
        // Register device before storing token (foreign key constraint)
        store.register_device("DEVICE1", "@bob:cis.local", Some("Test Device")).unwrap();
        
        // Now store token with device_id
        store.store_token("test_token_123", "@bob:cis.local", Some("DEVICE1")).unwrap();
        
        let user_id = store.validate_token("test_token_123").unwrap();
        assert_eq!(user_id, Some("@bob:cis.local".to_string()));
        
        let invalid = store.validate_token("invalid_token").unwrap();
        assert_eq!(invalid, None);
    }

    #[test]
    fn test_room_operations() {
        let store = MatrixStore::open_in_memory().unwrap();
        
        // Create a user
        store.ensure_user("@alice:cis.local").unwrap();
        
        // Create a room
        let room_id = "!test123:cis.local";
        store.create_room(room_id, "@alice:cis.local", Some("Test Room"), Some("A test room")).unwrap();
        
        // Check room exists
        assert!(store.room_exists(room_id).unwrap());
        assert!(!store.room_exists("!nonexistent:cis.local").unwrap());
        
        // Join the room
        store.join_room(room_id, "@alice:cis.local").unwrap();
        
        // Check user is in room
        assert!(store.is_user_in_room(room_id, "@alice:cis.local").unwrap());
        
        // Get joined rooms
        let joined_rooms = store.get_joined_rooms("@alice:cis.local").unwrap();
        assert!(joined_rooms.contains(&room_id.to_string()));
    }

    #[test]
    fn test_save_and_get_events() {
        let store = MatrixStore::open_in_memory().unwrap();
        
        // Create user and room
        store.ensure_user("@alice:cis.local").unwrap();
        let room_id = "!test456:cis.local";
        store.create_room(room_id, "@alice:cis.local", None, None).unwrap();
        store.join_room(room_id, "@alice:cis.local").unwrap();
        
        // Save an event
        let event_id = "$event123";
        let content = r#"{"msgtype":"m.text","body":"Hello World"}"#;
        store.save_event(
            room_id,
            event_id,
            "@alice:cis.local",
            "m.room.message",
            content,
            1234567890,
            None,
            None,
        ).unwrap();
        
        // Get messages
        let messages = store.get_room_messages(room_id, 0, 10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].event_id, event_id);
        assert_eq!(messages[0].content, content);
    }

    // ==================== Federation Event Tests ====================

    #[test]
    fn test_store_federation_event() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Store a federation event
        let inserted = store.store_federation_event(
            "$fed_event1",
            "@sender:example.com",
            "!room:example.com",
            "m.room.message",
            r#"{"body":"Hello","msgtype":"m.text"}"#,
            1234567890,
        ).unwrap();

        assert!(inserted);

        // Verify the event exists
        let exists = store.federation_event_exists("$fed_event1").unwrap();
        assert!(exists);

        // Get the event
        let event = store.get_federation_event("$fed_event1").unwrap();
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_id, "$fed_event1");
        assert_eq!(event.sender, "@sender:example.com");
        assert_eq!(event.room_id, "!room:example.com");
        assert_eq!(event.event_type, "m.room.message");
        assert!(!event.processed);
    }

    #[test]
    fn test_federation_event_deduplication() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Store the same event twice
        let inserted1 = store.store_federation_event(
            "$dup_event",
            "@sender:example.com",
            "!room:example.com",
            "m.room.message",
            r#"{"body":"Hello"}"#,
            1234567890,
        ).unwrap();
        assert!(inserted1);

        let inserted2 = store.store_federation_event(
            "$dup_event",
            "@sender:example.com",
            "!room:example.com",
            "m.room.message",
            r#"{"body":"Hello"}"#,
            1234567890,
        ).unwrap();
        assert!(!inserted2); // Second insert should be ignored

        // Count should be 1
        let count = store.count_federation_events(None).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_mark_federation_event_processed() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Store an event
        store.store_federation_event(
            "$proc_event",
            "@sender:example.com",
            "!room:example.com",
            "m.room.message",
            r#"{"body":"Hello"}"#,
            1234567890,
        ).unwrap();

        // Verify not processed
        let event = store.get_federation_event("$proc_event").unwrap().unwrap();
        assert!(!event.processed);

        // Mark as processed
        store.mark_federation_event_processed("$proc_event").unwrap();

        // Verify processed
        let event = store.get_federation_event("$proc_event").unwrap().unwrap();
        assert!(event.processed);
    }

    #[test]
    fn test_get_unprocessed_federation_events() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Store multiple events
        for i in 0..5 {
            store.store_federation_event(
                &format!("$unproc_event{}", i),
                "@sender:example.com",
                "!room:example.com",
                "m.room.message",
                r#"{"body":"Hello"}"#,
                1234567890,
            ).unwrap();
        }

        // Mark some as processed
        store.mark_federation_event_processed("$unproc_event0").unwrap();
        store.mark_federation_event_processed("$unproc_event2").unwrap();

        // Get unprocessed events
        let unprocessed = store.get_unprocessed_federation_events(10).unwrap();
        assert_eq!(unprocessed.len(), 3);

        // Verify counts
        let total = store.count_federation_events(None).unwrap();
        let processed = store.count_federation_events(Some(true)).unwrap();
        let unproc_count = store.count_federation_events(Some(false)).unwrap();

        assert_eq!(total, 5);
        assert_eq!(processed, 2);
        assert_eq!(unproc_count, 3);
    }

    #[test]
    fn test_cleanup_expired_federation_events() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Store some events
        for i in 0..3 {
            store.store_federation_event(
                &format!("$cleanup_event{}", i),
                "@sender:example.com",
                "!room:example.com",
                "m.room.message",
                r#"{"body":"Hello"}"#,
                1234567890,
            ).unwrap();
        }

        let count_before = store.count_federation_events(None).unwrap();
        assert_eq!(count_before, 3);

        // Cleanup with 30 days retention (should not delete recent events)
        let deleted = store.cleanup_expired_federation_events(30).unwrap();
        assert_eq!(deleted, 0);

        // Count should still be 3
        let count_after = store.count_federation_events(None).unwrap();
        assert_eq!(count_after, 3);
    }

    #[test]
    fn test_federation_event_not_found() {
        let store = MatrixStore::open_in_memory().unwrap();

        // Check non-existent event
        let exists = store.federation_event_exists("$nonexistent").unwrap();
        assert!(!exists);

        let event = store.get_federation_event("$nonexistent").unwrap();
        assert!(event.is_none());
    }
}
