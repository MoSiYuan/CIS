//! IM Skill 数据库层
//!
//! 管理消息、会话和用户的存储

use rusqlite::{Connection, Row};
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::*;
use crate::error::{ImError, Result};

/// IM 数据库
pub struct ImDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl ImDatabase {
    /// 打开数据库并初始化表结构
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path)
            .map_err(|e| ImError::Database(format!("Failed to open database: {}", e)))?;
        
        // 在同步上下文中初始化表结构
        Self::init_schema_sync(&mut conn)?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// 同步初始化表结构
    fn init_schema_sync(conn: &mut Connection) -> Result<()> {
        // 会话表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                conversation_type TEXT NOT NULL,
                name TEXT,
                participants TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_message_at TEXT,
                avatar_url TEXT,
                metadata TEXT
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 消息表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT,
                read_by TEXT,
                metadata TEXT,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 用户资料表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_profiles (
                user_id TEXT PRIMARY KEY,
                display_name TEXT,
                avatar_url TEXT,
                status TEXT NOT NULL,
                last_seen_at TEXT
            )",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 索引
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_conversation 
             ON messages(conversation_id, created_at)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_sender 
             ON messages(sender_id)",
            [],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    // === 会话操作 ===
    
    /// 创建或更新会话
    pub async fn create_conversation(&self, conversation: &Conversation) -> Result<()> {
        let participants_json = serde_json::to_string(&conversation.participants)
            .map_err(|e| ImError::Serialization(e.to_string()))?;
        
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO conversations 
             (id, conversation_type, name, participants, created_at, updated_at, 
              last_message_at, avatar_url, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
             name = excluded.name,
             participants = excluded.participants,
             updated_at = excluded.updated_at,
             last_message_at = excluded.last_message_at,
             avatar_url = excluded.avatar_url",
            rusqlite::params![
                conversation.id,
                format!("{:?}", conversation.conversation_type).to_lowercase(),
                conversation.name,
                participants_json,
                conversation.created_at.to_rfc3339(),
                conversation.updated_at.to_rfc3339(),
                conversation.last_message_at.map(|t| t.to_rfc3339()),
                conversation.avatar_url,
                serde_json::to_string(&conversation.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取会话
    pub async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_type, name, participants, created_at, updated_at,
             last_message_at, avatar_url, metadata
             FROM conversations WHERE id = ?1"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let result = stmt.query_row([id], |row| {
            Self::row_to_conversation(row)
        });
        
        match result {
            Ok(conv) => Ok(Some(conv)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImError::Database(e.to_string())),
        }
    }
    
    /// 列出用户参与的会话
    pub async fn list_conversations(&self, user_id: &str) -> Result<Vec<Conversation>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_type, name, participants, created_at, updated_at,
             last_message_at, avatar_url, metadata
             FROM conversations 
             WHERE participants LIKE ?1
             ORDER BY last_message_at DESC NULLS LAST"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let pattern = format!("%{}%", user_id);
        let conversations: Result<Vec<_>> = stmt
            .query_map([pattern], |row| Self::row_to_conversation(row))?
            .map(|r| r.map_err(|e| ImError::Database(e.to_string())))
            .collect();
        
        conversations
    }
    
    /// 更新会话
    pub async fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        let participants_json = serde_json::to_string(&conversation.participants)
            .map_err(|e| ImError::Serialization(e.to_string()))?;
        
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE conversations SET
             name = ?2,
             participants = ?3,
             updated_at = ?4,
             last_message_at = ?5,
             avatar_url = ?6,
             metadata = ?7
             WHERE id = ?1",
            rusqlite::params![
                conversation.id,
                conversation.name,
                participants_json,
                conversation.updated_at.to_rfc3339(),
                conversation.last_message_at.map(|t| t.to_rfc3339()),
                conversation.avatar_url,
                serde_json::to_string(&conversation.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 删除会话
    pub async fn delete_conversation(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            [id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    // === 消息操作 ===
    
    /// 保存消息
    pub async fn save_message(&self, message: &Message) -> Result<()> {
        let content_json = serde_json::to_string(&message.content)
            .map_err(|e| ImError::Serialization(e.to_string()))?;
        
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, sender_id, content_type, content, created_at,
              updated_at, read_by, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
             content = excluded.content,
             updated_at = excluded.updated_at,
             read_by = excluded.read_by",
            rusqlite::params![
                message.id,
                message.conversation_id,
                message.sender_id,
                message.content.content_type(),
                content_json,
                message.created_at.to_rfc3339(),
                message.updated_at.map(|t| t.to_rfc3339()),
                serde_json::to_string(&message.read_by).unwrap_or_default(),
                serde_json::to_string(&message.metadata).unwrap_or_default(),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        // 更新会话的 last_message_at
        conn.execute(
            "UPDATE conversations SET last_message_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![message.created_at.to_rfc3339(), message.conversation_id],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取消息
    pub async fn get_message(&self, id: &str) -> Result<Option<Message>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, sender_id, content_type, content,
             created_at, updated_at, read_by, metadata
             FROM messages WHERE id = ?1"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let result = stmt.query_row([id], |row| Self::row_to_message(row));
        
        match result {
            Ok(msg) => Ok(Some(msg)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImError::Database(e.to_string())),
        }
    }
    
    /// 获取消息列表
    pub async fn get_messages(
        &self,
        conversation_id: &str,
        before: Option<DateTime<Utc>>,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let conn = self.conn.lock().await;
        
        let messages: Result<Vec<Message>> = if let Some(before_ts) = before {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, sender_id, content_type, content,
                 created_at, updated_at, read_by, metadata
                 FROM messages 
                 WHERE conversation_id = ?1 AND created_at < ?2
                 ORDER BY created_at DESC
                 LIMIT ?3"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![conversation_id, before_ts.to_rfc3339(), limit],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, sender_id, content_type, content,
                 created_at, updated_at, read_by, metadata
                 FROM messages 
                 WHERE conversation_id = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2"
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            let rows = stmt.query_map(
                rusqlite::params![conversation_id, limit],
                Self::row_to_message,
            ).map_err(|e| ImError::Database(e.to_string()))?;
            
            rows.map(|r| r.map_err(|e| ImError::Database(e.to_string()))).collect()
        };
        
        let mut messages = messages?;
        // 反转回时间顺序
        messages.reverse();
        Ok(messages)
    }
    
    /// 标记消息已读
    pub async fn mark_message_read(&self, message_id: &str, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
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
        
        Ok(())
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
    
    // === 用户资料操作 ===
    
    /// 保存用户资料
    pub async fn save_user_profile(&self, profile: &UserProfile) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO user_profiles 
             (user_id, display_name, avatar_url, status, last_seen_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(user_id) DO UPDATE SET
             display_name = excluded.display_name,
             avatar_url = excluded.avatar_url,
             status = excluded.status,
             last_seen_at = excluded.last_seen_at",
            rusqlite::params![
                profile.user_id,
                profile.display_name,
                profile.avatar_url,
                format!("{:?}", profile.status).to_lowercase(),
                profile.last_seen_at.map(|t| t.to_rfc3339()),
            ],
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// 获取用户资料
    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT user_id, display_name, avatar_url, status, last_seen_at
             FROM user_profiles WHERE user_id = ?1"
        ).map_err(|e| ImError::Database(e.to_string()))?;
        
        let result = stmt.query_row([user_id], |row| Self::row_to_user_profile(row));
        
        match result {
            Ok(profile) => Ok(Some(profile)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ImError::Database(e.to_string())),
        }
    }
    
    // === Helper 方法 ===
    
    fn row_to_conversation(row: &Row) -> std::result::Result<Conversation, rusqlite::Error> {
        let participants_json: String = row.get(3)?;
        let participants: Vec<String> = serde_json::from_str(&participants_json)
            .map_err(|_| rusqlite::Error::InvalidColumnType(3, "participants".to_string(), rusqlite::types::Type::Text))?;
        
        let created_at_str: String = row.get(4)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(4, "created_at".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let updated_at_str: String = row.get(5)?;
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(5, "updated_at".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let last_message_at = row.get::<_, Option<String>>(6)?.map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidColumnType(6, "last_message_at".to_string(), rusqlite::types::Type::Text))
        }).transpose()?;
        
        let metadata_json: Option<String> = row.get(8)?;
        let metadata = metadata_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        Ok(Conversation {
            id: row.get(0)?,
            conversation_type: Self::parse_conversation_type(&row.get::<_, String>(1)?),
            name: row.get(2)?,
            participants,
            created_at,
            updated_at,
            last_message_at,
            avatar_url: row.get(7)?,
            metadata,
        })
    }
    
    fn row_to_message(row: &Row) -> std::result::Result<Message, rusqlite::Error> {
        let content_json: String = row.get(4)?;
        let content: MessageContent = serde_json::from_str(&content_json)
            .map_err(|_| rusqlite::Error::InvalidColumnType(4, "content".to_string(), rusqlite::types::Type::Text))?;
        
        let created_at_str: String = row.get(5)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|_| rusqlite::Error::InvalidColumnType(5, "created_at".to_string(), rusqlite::types::Type::Text))?
            .with_timezone(&Utc);
        
        let updated_at = row.get::<_, Option<String>>(6)?.map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidColumnType(6, "updated_at".to_string(), rusqlite::types::Type::Text))
        }).transpose()?;
        
        let read_by_json: Option<String> = row.get(7)?;
        let read_by = read_by_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        let metadata_json: Option<String> = row.get(8)?;
        let metadata = metadata_json.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        
        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            sender_id: row.get(2)?,
            content,
            created_at,
            updated_at,
            read_by,
            metadata,
        })
    }
    
    fn row_to_user_profile(row: &Row) -> std::result::Result<UserProfile, rusqlite::Error> {
        let status_str: String = row.get(3)?;
        let status = Self::parse_user_status(&status_str);
        
        let last_seen_at = row.get::<_, Option<String>>(4)?.map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| rusqlite::Error::InvalidColumnType(4, "last_seen_at".to_string(), rusqlite::types::Type::Text))
        }).transpose()?;
        
        Ok(UserProfile {
            user_id: row.get(0)?,
            display_name: row.get(1)?,
            avatar_url: row.get(2)?,
            status,
            last_seen_at,
        })
    }
    
    fn parse_conversation_type(s: &str) -> ConversationType {
        match s {
            "direct" => ConversationType::Direct,
            "group" => ConversationType::Group,
            "channel" => ConversationType::Channel,
            _ => ConversationType::Direct,
        }
    }
    
    fn parse_user_status(s: &str) -> UserStatus {
        match s {
            "online" => UserStatus::Online,
            "away" => UserStatus::Away,
            "busy" => UserStatus::Busy,
            "invisible" => UserStatus::Invisible,
            _ => UserStatus::Offline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_database_operations() {
        let temp_dir = TempDir::new().unwrap();
        let db = ImDatabase::open(&temp_dir.path().join("test.db")).unwrap();
        
        // 测试创建会话
        let conversation = Conversation {
            id: "conv-1".to_string(),
            conversation_type: ConversationType::Direct,
            name: Some("Test Chat".to_string()),
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::Value::Null,
        };
        
        db.create_conversation(&conversation).await.unwrap();
        
        // 测试获取会话
        let retrieved = db.get_conversation("conv-1").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, Some("Test Chat".to_string()));
        
        // 测试保存消息
        let message = Message::new(
            "conv-1".to_string(),
            "user1".to_string(),
            MessageContent::Text { text: "Hello!".to_string() },
        );
        
        db.save_message(&message).await.unwrap();
        
        // 测试获取消息
        let messages = db.get_messages("conv-1", None, 10).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].sender_id, "user1");
        
        // 测试标记已读
        db.mark_message_read(&message.id, "user2").await.unwrap();
        
        let retrieved_msg = db.get_message(&message.id).await.unwrap();
        assert!(retrieved_msg.is_some());
        let retrieved_msg = retrieved_msg.unwrap();
        assert!(retrieved_msg.read_by.contains(&"user2".to_string()));
    }
}
