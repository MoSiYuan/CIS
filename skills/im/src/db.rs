//! IM 数据库完整实现

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::*;
use crate::error::{ImError, Result};

/// IM 数据库
pub struct ImDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ImDatabase {
    /// 打开数据库（同步版本，用于非异步上下文）
    pub fn open(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("im.db");
        std::fs::create_dir_all(data_dir)
            .map_err(|e| ImError::Database(format!("Failed to create data dir: {}", e)))?;
        let conn = Connection::open(&db_path)
            .map_err(|e| ImError::Database(format!("Failed to open database: {}", e)))?;
        
        let db = Self { 
            conn: Arc::new(Mutex::new(conn)) 
        };
        
        // 同步初始化表结构
        db.init_tables_sync()?;
        
        Ok(db)
    }
    
    /// 异步打开数据库（用于异步上下文）
    pub async fn open_async(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("im.db");
        std::fs::create_dir_all(data_dir)
            .map_err(|e| ImError::Database(format!("Failed to create data dir: {}", e)))?;
        let conn = Connection::open(&db_path)
            .map_err(|e| ImError::Database(format!("Failed to open database: {}", e)))?;
        
        let db = Self { 
            conn: Arc::new(Mutex::new(conn)) 
        };
        
        // 异步初始化表结构
        db.init_tables().await?;
        
        Ok(db)
    }
    
    async fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        
        // 会话表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                session_type TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_message_at TEXT,
                avatar_url TEXT,
                metadata TEXT
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 参与者表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS participants (
                session_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                display_name TEXT,
                role TEXT DEFAULT 'member',
                joined_at TEXT NOT NULL,
                PRIMARY KEY (session_id, user_id),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 消息表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT DEFAULT 'sent',
                reply_to TEXT,
                read_by TEXT,
                metadata TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
                FOREIGN KEY (reply_to) REFERENCES messages(id)
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 已读状态表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS read_status (
                session_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                last_read_message_id TEXT,
                updated_at TEXT,
                PRIMARY KEY (session_id, user_id),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session_time 
             ON messages(session_id, timestamp DESC)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_id)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_participants_user ON participants(user_id)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 同步初始化表（用于 open 方法）
    fn init_tables_sync(&self) -> Result<()> {
        // 使用阻塞锁获取连接
        let conn = self.conn.try_lock()
            .map_err(|_| ImError::Database("Failed to acquire lock".to_string()))?;
        
        // 会话表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                session_type TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_message_at TEXT,
                avatar_url TEXT,
                metadata TEXT
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 参与者表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS participants (
                session_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                display_name TEXT,
                role TEXT DEFAULT 'member',
                joined_at TEXT NOT NULL,
                PRIMARY KEY (session_id, user_id),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 消息表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                status TEXT DEFAULT 'sent',
                reply_to TEXT,
                read_by TEXT,
                metadata TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
                FOREIGN KEY (reply_to) REFERENCES messages(id)
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 已读状态表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS read_status (
                session_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                last_read_message_id TEXT,
                updated_at TEXT,
                PRIMARY KEY (session_id, user_id),
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session_time 
             ON messages(session_id, timestamp DESC)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_id)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_participants_user ON participants(user_id)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    // ===== 会话操作 =====
    
    /// 创建或更新会话
    pub async fn create_session(&self, session: &Conversation) -> Result<()> {
        let conn = self.conn.lock().await;
        
        // 插入会话
        conn.execute(
            "INSERT INTO sessions (id, session_type, title, created_at, updated_at, 
                                  last_message_at, avatar_url, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
             title = excluded.title,
             updated_at = excluded.updated_at,
             last_message_at = excluded.last_message_at,
             avatar_url = excluded.avatar_url,
             metadata = excluded.metadata",
            rusqlite::params![
                session.id,
                format!("{:?}", session.conversation_type).to_lowercase(),
                session.name,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                session.last_message_at.map(|t| t.to_rfc3339()),
                session.avatar_url,
                serde_json::to_string(&session.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 先删除该会话的所有旧参与者记录（确保参与者列表与 session.participants 一致）
        conn.execute(
            "DELETE FROM participants WHERE session_id = ?1",
            rusqlite::params![session.id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 插入参与者
        for user_id in &session.participants {
            conn.execute(
                "INSERT INTO participants (session_id, user_id, role, joined_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(session_id, user_id) DO UPDATE SET
                 role = excluded.role",
                rusqlite::params![
                    session.id,
                    user_id,
                    "member",
                    session.created_at.to_rfc3339(),
                ],
            ).map_err(|e| ImError::Database(e.to_string()))?;
        }
        
        Ok(())
    }
    
    /// 获取会话
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Conversation>> {
        let conn = self.conn.lock().await;
        
        let session = conn.query_row(
            "SELECT id, session_type, title, created_at, updated_at, 
                    last_message_at, avatar_url, metadata
             FROM sessions WHERE id = ?1",
            [session_id],
            Self::row_to_conversation,
        ).optional().map_err(|e| ImError::Database(e.to_string()))?;
        
        if let Some(mut session) = session {
            // 加载参与者
            session.participants = Self::get_participants_internal_sync(&conn, &session.id)?;
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
    
    /// 列出用户的会话
    pub async fn list_sessions(&self, user_id: &str, limit: usize, offset: usize) 
        -> Result<Vec<Conversation>> 
    {
        let conn = self.conn.lock().await;
        
        let mut stmt = conn.prepare(
            "SELECT s.id, s.session_type, s.title, s.created_at, s.updated_at,
                    s.last_message_at, s.avatar_url, s.metadata
             FROM sessions s
             JOIN participants p ON s.id = p.session_id
             WHERE p.user_id = ?1
             ORDER BY s.updated_at DESC
             LIMIT ?2 OFFSET ?3"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let sessions: Result<Vec<_>> = stmt
            .query_map(
                rusqlite::params![user_id, limit as i64, offset as i64],
                Self::row_to_conversation,
            )
            .map_err(|e| ImError::Database(e.to_string()))?
            .map(|r| r.map_err(|e| ImError::Database(e.to_string())))
            .collect();
        
        let mut sessions = sessions?;
        
        // 加载每个会话的参与者
        for session in &mut sessions {
            session.participants = Self::get_participants_internal_sync(&conn, &session.id)?;
        }
        
        Ok(sessions)
    }
    
    /// 更新会话
    pub async fn update_session(&self, session: &Conversation) -> Result<()> {
        let conn = self.conn.lock().await;
        
        conn.execute(
            "UPDATE sessions SET
             title = ?2,
             updated_at = ?3,
             last_message_at = ?4,
             avatar_url = ?5,
             metadata = ?6
             WHERE id = ?1",
            rusqlite::params![
                session.id,
                session.name,
                session.updated_at.to_rfc3339(),
                session.last_message_at.map(|t| t.to_rfc3339()),
                session.avatar_url,
                serde_json::to_string(&session.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        
        conn.execute(
            "DELETE FROM sessions WHERE id = ?1",
            [session_id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    // ===== 消息操作 =====
    
    /// 保存消息
    pub async fn save_message(&self, message: &Message) -> Result<()> {
        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| ImError::Serialization(e.to_string()))?;
        
        let conn = self.conn.lock().await;
        
        // 提取 reply_to
        let reply_to = match &message.content {
            MessageContent::Reply { reply_to, .. } => Some(reply_to.as_str()),
            _ => None,
        };
        
        conn.execute(
            "INSERT INTO messages (id, session_id, sender_id, content_type, content, 
                                  timestamp, status, reply_to, read_by, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
             status = excluded.status,
             content = excluded.content,
             read_by = excluded.read_by",
            rusqlite::params![
                message.id,
                message.conversation_id,
                message.sender_id,
                message.content.content_type(),
                content_json,
                message.created_at.to_rfc3339(),
                "sent",
                reply_to,
                serde_json::to_string(&message.read_by).unwrap_or_default(),
                serde_json::to_string(&message.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 更新会话时间
        conn.execute(
            "UPDATE sessions SET updated_at = ?1, last_message_at = ?1 WHERE id = ?2",
            rusqlite::params![message.created_at.to_rfc3339(), message.conversation_id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取单条消息
    pub async fn get_message(&self, message_id: &str) -> Result<Option<Message>> {
        let conn = self.conn.lock().await;
        
        let message = conn.query_row(
            "SELECT id, session_id, sender_id, content_type, content, timestamp, 
                    status, reply_to, read_by, metadata
             FROM messages WHERE id = ?1",
            [message_id],
            Self::row_to_message,
        ).optional().map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(message)
    }
    
    /// 获取会话消息历史
    pub async fn get_messages(&self, session_id: &str, before: Option<DateTime<Utc>>, limit: usize) 
        -> Result<Vec<Message>> 
    {
        let conn = self.conn.lock().await;
        
        let messages: Result<Vec<Message>> = if let Some(before_time) = before {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, sender_id, content_type, content, timestamp,
                        status, reply_to, read_by, metadata
                 FROM messages 
                 WHERE session_id = ?1 AND timestamp < ?2
                 ORDER BY timestamp DESC
                 LIMIT ?3"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![session_id, before_time.to_rfc3339(), limit as i64],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, sender_id, content_type, content, timestamp,
                        status, reply_to, read_by, metadata
                 FROM messages 
                 WHERE session_id = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![session_id, limit as i64],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        };
        
        let mut messages = messages?;
        // 反转回时间正序
        messages.reverse();
        Ok(messages)
    }
    
    /// 搜索消息
    pub async fn search_messages(&self, query: &str, session_id: Option<&str>, limit: usize) 
        -> Result<Vec<Message>> 
    {
        let conn = self.conn.lock().await;
        
        let pattern = format!("%{}%", query);
        
        let messages: Result<Vec<Message>> = if let Some(sid) = session_id {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, sender_id, content_type, content, timestamp,
                        status, reply_to, read_by, metadata
                 FROM messages 
                 WHERE session_id = ?1 AND content LIKE ?2
                 ORDER BY timestamp DESC
                 LIMIT ?3"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![sid, pattern, limit as i64],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, sender_id, content_type, content, timestamp,
                        status, reply_to, read_by, metadata
                 FROM messages 
                 WHERE content LIKE ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![pattern, limit as i64],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        };
        
        messages
    }
    
    /// 删除消息
    pub async fn delete_message(&self, message_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        
        conn.execute(
            "DELETE FROM messages WHERE id = ?1",
            [message_id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    // ===== 已读状态 =====
    
    /// 标记消息已读
    pub async fn mark_as_read(&self, session_id: &str, user_id: &str, message_id: &str) 
        -> Result<()> 
    {
        let conn = self.conn.lock().await;
        
        // 更新消息的 read_by
        let mut stmt = conn.prepare(
            "SELECT read_by FROM messages WHERE id = ?1"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let read_by_json: String = stmt.query_row([message_id], |row| row.get(0))
            .map_err(|e| ImError::Database(e.to_string()))?;
        
        let mut read_by: Vec<String> = serde_json::from_str(&read_by_json)
            .unwrap_or_default();
        
        if !read_by.contains(&user_id.to_string()) {
            read_by.push(user_id.to_string());
            
            conn.execute(
                "UPDATE messages SET read_by = ?1 WHERE id = ?2",
                rusqlite::params![serde_json::to_string(&read_by).unwrap(), message_id],
            ).map_err(|e| ImError::Database(e.to_string()))?;
        }
        
        // 更新已读状态表
        conn.execute(
            "INSERT INTO read_status (session_id, user_id, last_read_message_id, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(session_id, user_id) DO UPDATE SET
             last_read_message_id = excluded.last_read_message_id,
             updated_at = excluded.updated_at",
            rusqlite::params![
                session_id,
                user_id,
                message_id,
                Utc::now().to_rfc3339(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取未读数
    pub async fn get_unread_count(&self, session_id: &str, user_id: &str) -> Result<u64> {
        let conn = self.conn.lock().await;
        
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages m
             LEFT JOIN read_status r ON m.session_id = r.session_id AND r.user_id = ?2
             WHERE m.session_id = ?1 
             AND (r.last_read_message_id IS NULL OR m.timestamp > 
                  (SELECT timestamp FROM messages WHERE id = r.last_read_message_id))
             AND m.sender_id != ?2",
            rusqlite::params![session_id, user_id],
            |row| row.get(0),
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(count as u64)
    }
    
    /// 标记消息已读（旧接口兼容）
    pub async fn mark_message_read(&self, message_id: &str, user_id: &str) -> Result<()> {
        // 获取消息所属会话
        let conn = self.conn.lock().await;
        
        let session_id: String = conn.query_row(
            "SELECT session_id FROM messages WHERE id = ?1",
            [message_id],
            |row| row.get(0),
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        drop(conn); // 释放锁
        
        self.mark_as_read(&session_id, user_id, message_id).await
    }
    
    /// 创建或更新会话（旧接口兼容）
    pub async fn create_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.create_session(conversation).await
    }
    
    /// 获取会话（旧接口兼容）
    pub async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        self.get_session(id).await
    }
    
    /// 列出会话（旧接口兼容）
    pub async fn list_conversations(&self, user_id: &str) -> Result<Vec<Conversation>> {
        self.list_sessions(user_id, 100, 0).await
    }
    
    /// 更新会话（旧接口兼容）
    pub async fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.update_session(conversation).await
    }
    
    /// 删除会话（旧接口兼容）
    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        self.delete_session(id).await
    }
    
    /// 保存用户资料
    pub async fn save_user_profile(&self, profile: &UserProfile) -> Result<()> {
        // 用户资料存储在 participants 表中
        let conn = self.conn.lock().await;
        
        conn.execute(
            "INSERT INTO participants (session_id, user_id, display_name, role, joined_at)
             VALUES ('__global__', ?1, ?2, 'user', ?3)
             ON CONFLICT(session_id, user_id) DO UPDATE SET
             display_name = excluded.display_name",
            rusqlite::params![
                profile.user_id,
                profile.display_name,
                Utc::now().to_rfc3339(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取用户资料
    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let conn = self.conn.lock().await;
        
        let profile = conn.query_row(
            "SELECT user_id, display_name FROM participants 
             WHERE session_id = '__global__' AND user_id = ?1",
            [user_id],
            |row| {
                Ok(UserProfile {
                    user_id: row.get(0)?,
                    display_name: row.get(1)?,
                    avatar_url: None,
                    status: UserStatus::Offline,
                    last_seen_at: None,
                })
            },
        ).optional().map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(profile)
    }
    
    // ===== 辅助方法 =====
    
    fn get_participants_internal_sync(
        conn: &Connection, 
        session_id: &str
    ) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT user_id FROM participants WHERE session_id = ?1"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let participants: Result<Vec<String>> = stmt
            .query_map([session_id], |row| row.get(0))
            .map_err(|e| ImError::Database(e.to_string()))?
            .map(|r| r.map_err(|e| ImError::Database(e.to_string())))
            .collect();
        
        participants
    }
    
    fn row_to_conversation(row: &rusqlite::Row) -> std::result::Result<Conversation, rusqlite::Error> {
        let session_type_str: String = row.get(1)?;
        let conversation_type = match session_type_str.as_str() {
            "direct" => ConversationType::Direct,
            "group" => ConversationType::Group,
            "channel" => ConversationType::Channel,
            _ => ConversationType::Direct,
        };
        
        let created_at_str: String = row.get(3)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(3, "created_at".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let updated_at_str: String = row.get(4)?;
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(4, "updated_at".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let last_message_at = row.get::<_, Option<String>>(5)?.map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidColumnType(5, "last_message_at".to_string(), rusqlite::types::Type::Text))
        }).transpose()?;
        
        let metadata_json: Option<String> = row.get(7)?;
        let metadata = metadata_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        Ok(Conversation {
            id: row.get(0)?,
            conversation_type,
            name: row.get(2)?,
            participants: vec![], // 单独加载
            created_at,
            updated_at,
            last_message_at,
            avatar_url: row.get(6)?,
            metadata,
        })
    }
    
    fn row_to_message(row: &rusqlite::Row) -> std::result::Result<Message, rusqlite::Error> {
        let content_json: String = row.get(4)?;
        let content: MessageContent = serde_json::from_str(&content_json)
            .map_err(|_| rusqlite::Error::InvalidColumnType(4, "content".to_string(), rusqlite::types::Type::Text))?;
        
        let timestamp_str: String = row.get(5)?;
        let created_at = DateTime::parse_from_rfc3339(&timestamp_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(5, "timestamp".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let read_by_json: Option<String> = row.get(8)?;
        let read_by = read_by_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        let metadata_json: Option<String> = row.get(9)?;
        let metadata = metadata_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            sender_id: row.get(2)?,
            content,
            created_at,
            updated_at: None,
            read_by,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_session_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = ImDatabase::open(temp_dir.path()).unwrap();
        
        // 测试创建会话
        let session = Conversation {
            id: "session-1".to_string(),
            conversation_type: ConversationType::Group,
            name: Some("Test Session".to_string()),
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };
        
        db.create_session(&session).await.unwrap();
        
        // 测试获取会话
        let retrieved = db.get_session("session-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, Some("Test Session".to_string()));
        assert_eq!(retrieved.participants.len(), 2);
        
        // 测试列出会话
        let sessions = db.list_sessions("user1", 10, 0).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }
    
    #[tokio::test]
    async fn test_message_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = ImDatabase::open(temp_dir.path()).unwrap();
        
        // 创建会话
        let session = Conversation {
            id: "session-1".to_string(),
            conversation_type: ConversationType::Direct,
            name: None,
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };
        db.create_session(&session).await.unwrap();
        
        // 测试保存消息
        let message = Message::new(
            "session-1".to_string(),
            "user1".to_string(),
            MessageContent::Text { text: "Hello!".to_string() },
        );
        
        db.save_message(&message).await.unwrap();
        
        // 测试获取消息
        let retrieved = db.get_message(&message.id).await.unwrap();
        assert!(retrieved.is_some());
        
        // 测试获取消息列表
        let messages = db.get_messages("session-1", None, 10).await.unwrap();
        assert_eq!(messages.len(), 1);
        
        // 测试搜索消息
        let results = db.search_messages("Hello", Some("session-1"), 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }
    
    #[tokio::test]
    async fn test_read_status() {
        let temp_dir = TempDir::new().unwrap();
        let db = ImDatabase::open(temp_dir.path()).unwrap();
        
        // 创建会话和消息
        let session = Conversation {
            id: "session-1".to_string(),
            conversation_type: ConversationType::Direct,
            name: None,
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };
        db.create_session(&session).await.unwrap();
        
        let message = Message::new(
            "session-1".to_string(),
            "user1".to_string(),
            MessageContent::Text { text: "Test".to_string() },
        );
        db.save_message(&message).await.unwrap();
        
        // 测试未读数
        let count = db.get_unread_count("session-1", "user2").await.unwrap();
        assert_eq!(count, 1);
        
        // 测试标记已读
        db.mark_as_read("session-1", "user2", &message.id).await.unwrap();
        
        // 再次检查未读数
        let count = db.get_unread_count("session-1", "user2").await.unwrap();
        assert_eq!(count, 0);
    }
}
