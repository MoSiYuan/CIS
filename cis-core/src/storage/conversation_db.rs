//! # Conversation Database
//!
//! 对话数据存储管理，支持对话历史和消息持久化。

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::error::Result;

/// 对话记录
#[derive(Debug, Clone)]
pub struct Conversation {
    /// 对话唯一ID
    pub id: String,
    /// 所属会话ID
    pub session_id: String,
    /// 关联项目路径
    pub project_path: Option<String>,
    /// 对话摘要
    pub summary: Option<String>,
    /// 话题标签
    pub topics: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 对话消息
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    /// 消息唯一ID
    pub id: String,
    /// 所属对话ID
    pub conversation_id: String,
    /// 角色 (user/assistant/system)
    pub role: String,
    /// 消息内容
    pub content: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
}

/// 对话数据库管理
pub struct ConversationDb {
    conn: Connection,
}

impl ConversationDb {
    /// 打开对话数据库
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// 创建数据库表结构
    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                project_path TEXT,
                summary TEXT,
                topics TEXT,
                created_at INTEGER,
                updated_at INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversation_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_conv_session ON conversations(session_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_conv_project ON conversations(project_path)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_msg_conv ON conversation_messages(conversation_id)",
            [],
        )?;

        Ok(())
    }

    /// 保存对话记录（插入或更新）
    pub fn save_conversation(&self, conv: &Conversation) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversations (id, session_id, project_path, summary, topics, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
             summary = excluded.summary,
             topics = excluded.topics,
             updated_at = excluded.updated_at",
            rusqlite::params![
                conv.id,
                conv.session_id,
                conv.project_path,
                conv.summary,
                serde_json::to_string(&conv.topics)?,
                conv.created_at.timestamp(),
                conv.updated_at.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 获取单个对话记录
    pub fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, project_path, summary, topics, created_at, updated_at
             FROM conversations WHERE id = ?1",
        )?;

        let result = stmt.query_row([id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                session_id: row.get(1)?,
                project_path: row.get(2)?,
                summary: row.get(3)?,
                topics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_default(),
                updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap_or_default(),
            })
        });

        match result {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 根据项目路径列出对话记录
    pub fn list_conversations_by_project(
        &self,
        project_path: &str,
        limit: usize,
    ) -> Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, project_path, summary, topics, created_at, updated_at
             FROM conversations WHERE project_path = ?1 ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(rusqlite::params![project_path, limit as i32], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                session_id: row.get(1)?,
                project_path: row.get(2)?,
                summary: row.get(3)?,
                topics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_default(),
                updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap_or_default(),
            })
        })?;

        let conversations: Vec<_> = rows.into_iter().flatten().collect();
        Ok(conversations)
    }

    /// 根据会话ID列出对话记录
    pub fn list_conversations_by_session(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, project_path, summary, topics, created_at, updated_at
             FROM conversations WHERE session_id = ?1 ORDER BY updated_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(rusqlite::params![session_id, limit as i32], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                session_id: row.get(1)?,
                project_path: row.get(2)?,
                summary: row.get(3)?,
                topics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_default(),
                updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap_or_default(),
            })
        })?;

        let mut conversations = Vec::new();
        for row in rows {
            if let Ok(conv) = row {
                conversations.push(conv);
            }
        }
        Ok(conversations)
    }

    /// 保存消息
    pub fn save_message(&self, msg: &ConversationMessage) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversation_messages (id, conversation_id, role, content, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
             content = excluded.content",
            rusqlite::params![
                msg.id,
                msg.conversation_id,
                msg.role,
                msg.content,
                msg.timestamp.timestamp(),
            ],
        )?;
        Ok(())
    }

    /// 获取对话的所有消息
    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<ConversationMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content, timestamp
             FROM conversation_messages 
             WHERE conversation_id = ?1 
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map([conversation_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                timestamp: DateTime::from_timestamp(row.get(4)?, 0).unwrap_or_default(),
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            if let Ok(msg) = row {
                messages.push(msg);
            }
        }
        Ok(messages)
    }

    /// 删除对话及其消息
    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        // 先删除关联的消息
        self.conn.execute(
            "DELETE FROM conversation_messages WHERE conversation_id = ?1",
            [id],
        )?;
        // 再删除对话
        self.conn.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
        Ok(())
    }

    /// 获取底层数据库连接
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 更新对话摘要
    pub fn update_summary(&self, id: &str, summary: &str) -> Result<()> {
        let updated_at = Utc::now().timestamp();
        self.conn.execute(
            "UPDATE conversations SET summary = ?1, updated_at = ?2 WHERE id = ?3",
            [summary, &updated_at.to_string(), id],
        )?;
        Ok(())
    }

    /// 更新对话话题
    pub fn update_topics(&self, id: &str, topics: &[String]) -> Result<()> {
        let updated_at = Utc::now().timestamp();
        let topics_json = serde_json::to_string(topics)?;
        self.conn.execute(
            "UPDATE conversations SET topics = ?1, updated_at = ?2 WHERE id = ?3",
            [&topics_json, &updated_at.to_string(), id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_conversation(id: &str) -> Conversation {
        Conversation {
            id: id.to_string(),
            session_id: "session-001".to_string(),
            project_path: Some("/test/project".to_string()),
            summary: Some("Test conversation".to_string()),
            topics: vec!["test".to_string(), "rust".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_conversation_db() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("conversations.db");
        let db = ConversationDb::open(&db_path).unwrap();

        // 测试保存对话
        let conv = create_test_conversation("conv-001");
        db.save_conversation(&conv).unwrap();

        // 测试获取对话
        let retrieved = db.get_conversation("conv-001").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "conv-001");
        assert_eq!(retrieved.session_id, "session-001");

        // 测试按项目路径列出
        let convs = db.list_conversations_by_project("/test/project", 10).unwrap();
        assert_eq!(convs.len(), 1);

        // 测试保存消息
        let msg = ConversationMessage {
            id: "msg-001".to_string(),
            conversation_id: "conv-001".to_string(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            timestamp: Utc::now(),
        };
        db.save_message(&msg).unwrap();

        // 测试获取消息
        let messages = db.get_messages("conv-001").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");

        // 测试删除对话
        db.delete_conversation("conv-001").unwrap();
        let retrieved = db.get_conversation("conv-001").unwrap();
        assert!(retrieved.is_none());
    }
}
