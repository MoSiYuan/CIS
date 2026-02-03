//! IM Skill - 即时通讯 Skill
//!
//! 完整的 IM Skill 实现，支持：
//! - 一对一和群组聊天
//! - 消息存储和历史查询
//! - Matrix Room 集成
//! - 联邦同步

pub mod db;
pub mod error;
pub mod handler;
pub mod message;
pub mod search;
pub mod session;
pub mod types;
pub mod matrix_adapter;

pub use db::ImDatabase;
pub use error::{ImError, Result};
pub use handler::*;
pub use message::MessageManager;
pub use search::ImMessageSearch;
pub use session::SessionManager;
pub use types::*;

use std::path::Path;
use std::sync::Arc;

/// IM Skill 主结构
pub struct ImSkill {
    db: Arc<ImDatabase>,
    config: ImConfig,
}

impl ImSkill {
    /// 创建新的 IM Skill
    pub fn new(db_path: &Path) -> Result<Self> {
        let db = ImDatabase::open(db_path)?;
        Ok(Self {
            db: Arc::new(db),
            config: ImConfig::default(),
        })
    }
    
    /// 使用自定义配置创建
    pub fn with_config(mut self, config: ImConfig) -> Self {
        self.config = config;
        self
    }
    
    /// 获取数据库引用
    pub fn db(&self) -> &Arc<ImDatabase> {
        &self.db
    }
    
    /// 发送消息
    pub async fn send_message(
        &self,
        conversation_id: &str,
        sender_id: &str,
        content: MessageContent,
    ) -> Result<Message> {
        // 验证消息长度
        let content_size = serde_json::to_string(&content).unwrap_or_default().len();
        if content_size > self.config.max_message_length {
            return Err(ImError::MessageTooLarge {
                size: content_size,
                max: self.config.max_message_length,
            });
        }
        
        // 验证会话存在
        if self.db.get_conversation(conversation_id).await?.is_none() {
            return Err(ImError::ConversationNotFound(conversation_id.to_string()));
        }
        
        let message = Message::new(
            conversation_id.to_string(),
            sender_id.to_string(),
            content,
        );
        
        self.db.save_message(&message).await?;
        
        Ok(message)
    }
    
    /// 获取消息历史
    pub async fn get_history(
        &self,
        conversation_id: &str,
        before: Option<chrono::DateTime<chrono::Utc>>,
        limit: usize,
    ) -> Result<Vec<Message>> {
        self.db.get_messages(conversation_id, before, limit).await
    }
    
    /// 创建会话
    pub async fn create_conversation(
        &self,
        conversation_type: ConversationType,
        name: Option<String>,
        participants: Vec<String>,
    ) -> Result<Conversation> {
        let now = chrono::Utc::now();
        let conversation = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type,
            name,
            participants,
            created_at: now,
            updated_at: now,
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::Value::Null,
        };
        
        self.db.create_conversation(&conversation).await?;
        
        Ok(conversation)
    }
    
    /// 获取会话
    pub async fn get_conversation(&self, conversation_id: &str) -> Result<Option<Conversation>> {
        self.db.get_conversation(conversation_id).await
    }
    
    /// 列出用户的会话
    pub async fn list_conversations(&self, user_id: &str) -> Result<Vec<Conversation>> {
        self.db.list_conversations(user_id).await
    }
    
    /// 标记已读
    pub async fn mark_read(&self, message_id: &str, user_id: &str) -> Result<()> {
        self.db.mark_message_read(message_id, user_id).await
    }
    
    /// 更新用户资料
    pub async fn update_user_profile(&self, profile: UserProfile) -> Result<()> {
        self.db.save_user_profile(&profile).await
    }
    
    /// 获取用户资料
    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        self.db.get_user_profile(user_id).await
    }
}

impl Default for ImSkill {
    fn default() -> Self {
        // 使用内存数据库作为默认
        let db = ImDatabase::open(Path::new(":memory:")).expect("Failed to open memory database");
        Self {
            db: Arc::new(db),
            config: ImConfig::default(),
        }
    }
}

/// Skill 元数据
pub const SKILL_NAME: &str = "im";
pub const SKILL_VERSION: &str = "0.1.0";
pub const SKILL_DESCRIPTION: &str = "Instant Messaging Skill with Matrix integration";
pub const SKILL_ROOM_ID: &str = "!im:cis.local";
pub const SKILL_FEDERATE: bool = true;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_create_conversation() {
        let temp_dir = TempDir::new().unwrap();
        let skill = ImSkill::new(&temp_dir.path().join("im.db")).unwrap();
        
        let conv = skill.create_conversation(
            ConversationType::Direct,
            Some("Test Chat".to_string()),
            vec!["user1".to_string(), "user2".to_string()],
        ).await.unwrap();
        
        assert_eq!(conv.conversation_type, ConversationType::Direct);
        assert_eq!(conv.name, Some("Test Chat".to_string()));
        assert_eq!(conv.participants.len(), 2);
    }
    
    #[tokio::test]
    async fn test_send_message() {
        let temp_dir = TempDir::new().unwrap();
        let skill = ImSkill::new(&temp_dir.path().join("im.db")).unwrap();
        
        // 先创建会话
        let conv = skill.create_conversation(
            ConversationType::Direct,
            None,
            vec!["user1".to_string()],
        ).await.unwrap();
        
        // 发送消息
        let msg = skill.send_message(
            &conv.id,
            "user1",
            MessageContent::Text { text: "Hello!".to_string() },
        ).await.unwrap();
        
        assert_eq!(msg.sender_id, "user1");
        assert!(matches!(msg.content, MessageContent::Text { .. }));
    }
    
    #[tokio::test]
    async fn test_message_too_large() {
        let temp_dir = TempDir::new().unwrap();
        let skill = ImSkill::new(&temp_dir.path().join("im.db")).unwrap()
            .with_config(ImConfig {
                max_message_length: 10,
                ..Default::default()
            });
        
        let conv = skill.create_conversation(
            ConversationType::Direct,
            None,
            vec!["user1".to_string()],
        ).await.unwrap();
        
        // 尝试发送超过限制的消息
        let result = skill.send_message(
            &conv.id,
            "user1",
            MessageContent::Text { text: "This is a very long message".to_string() },
        ).await;
        
        assert!(matches!(result, Err(ImError::MessageTooLarge { .. })));
    }
    
    #[tokio::test]
    async fn test_list_conversations() {
        let temp_dir = TempDir::new().unwrap();
        let skill = ImSkill::new(&temp_dir.path().join("im.db")).unwrap();
        
        // 创建两个会话
        skill.create_conversation(
            ConversationType::Direct,
            Some("Chat 1".to_string()),
            vec!["user1".to_string(), "user2".to_string()],
        ).await.unwrap();
        
        skill.create_conversation(
            ConversationType::Group,
            Some("Group Chat".to_string()),
            vec!["user1".to_string(), "user2".to_string(), "user3".to_string()],
        ).await.unwrap();
        
        // 列出 user1 的会话
        let conversations = skill.list_conversations("user1").await.unwrap();
        assert_eq!(conversations.len(), 2);
    }
}
