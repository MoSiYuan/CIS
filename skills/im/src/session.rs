//! IM 会话管理
//!
//! 提供会话的创建、查询、更新和删除功能。

use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::db::ImDatabase;
use crate::types::{Conversation, ConversationType, UserId};
use crate::error::{ImError, Result};

/// 会话管理器
pub struct SessionManager {
    db: Arc<ImDatabase>,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new(db: Arc<ImDatabase>) -> Self {
        Self { db }
    }

    /// 创建一对一私聊会话
    pub async fn create_direct_session(
        &self,
        user1: UserId,
        user2: UserId,
    ) -> Result<Conversation> {
        // 检查是否已存在
        if let Some(existing) = self.find_direct_session(&user1, &user2).await? {
            return Ok(existing);
        }

        let now = Utc::now();
        let session = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Direct,
            name: None,
            participants: vec![user1, user2],
            created_at: now,
            updated_at: now,
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };

        self.db.create_conversation(&session).await?;
        Ok(session)
    }

    /// 创建群组会话
    pub async fn create_group_session(
        &self,
        name: String,
        participants: Vec<UserId>,
    ) -> Result<Conversation> {
        if participants.len() < 2 {
            return Err(ImError::InvalidMessage(
                "Group session requires at least 2 participants".to_string()
            ));
        }

        let now = Utc::now();
        let session = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Group,
            name: Some(name),
            participants,
            created_at: now,
            updated_at: now,
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({
                "group_settings": {
                    "max_members": 500,
                    "invite_only": true,
                }
            }),
        };

        self.db.create_conversation(&session).await?;
        Ok(session)
    }

    /// 创建频道会话
    pub async fn create_channel_session(
        &self,
        name: String,
        owner: UserId,
    ) -> Result<Conversation> {
        let now = Utc::now();
        let session = Conversation {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Channel,
            name: Some(name),
            participants: vec![owner],
            created_at: now,
            updated_at: now,
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({
                "channel_settings": {
                    "public": true,
                    "readonly": false,
                }
            }),
        };

        self.db.create_conversation(&session).await?;
        Ok(session)
    }

    /// 查找一对一会话
    async fn find_direct_session(
        &self,
        user1: &str,
        user2: &str,
    ) -> Result<Option<Conversation>> {
        let sessions = self.list_user_sessions(user1).await?;
        
        for session in sessions {
            if session.conversation_type == ConversationType::Direct {
                let has_both = session.participants.contains(&user1.to_string()) 
                    && session.participants.contains(&user2.to_string());
                if has_both && session.participants.len() == 2 {
                    return Ok(Some(session));
                }
            }
        }
        
        Ok(None)
    }

    /// 获取会话
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Conversation>> {
        self.db.get_conversation(session_id).await
    }

    /// 列出用户的所有会话
    pub async fn list_user_sessions(&self, user_id: &str) -> Result<Vec<Conversation>> {
        self.db.list_conversations(user_id).await
    }

    /// 添加参与者
    pub async fn add_participant(
        &self,
        session_id: &str,
        user_id: UserId,
    ) -> Result<()> {
        let mut session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        if !session.participants.contains(&user_id) {
            session.participants.push(user_id);
            session.updated_at = Utc::now();
            self.db.create_conversation(&session).await?;
        }

        Ok(())
    }

    /// 移除参与者
    pub async fn remove_participant(
        &self,
        session_id: &str,
        user_id: &str,
    ) -> Result<()> {
        let mut session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        session.participants.retain(|p| p != user_id);
        session.updated_at = Utc::now();
        self.db.create_conversation(&session).await?;

        Ok(())
    }

    /// 更新会话信息
    pub async fn update_session(
        &self,
        session_id: &str,
        name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<()> {
        let mut session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        if let Some(name) = name {
            session.name = Some(name);
        }
        if let Some(avatar) = avatar_url {
            session.avatar_url = Some(avatar);
        }
        session.updated_at = Utc::now();

        self.db.update_conversation(&session).await
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.db.delete_conversation(session_id).await
    }

    /// 检查用户是否在会话中
    pub async fn is_participant(&self, session_id: &str, user_id: &str) -> Result<bool> {
        let session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        Ok(session.participants.contains(&user_id.to_string()))
    }

    /// 获取会话的参与者列表
    pub async fn get_participants(&self, session_id: &str) -> Result<Vec<UserId>> {
        let session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        Ok(session.participants)
    }

    /// 更新会话最后消息时间
    pub async fn update_last_message_at(
        &self,
        session_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let mut session = self.db.get_conversation(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;

        session.last_message_at = Some(timestamp);
        session.updated_at = timestamp;

        self.db.update_conversation(&session).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_manager() -> (SessionManager, tempfile::TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(ImDatabase::open(&temp_dir.path().join("test.db")).unwrap());
        let manager = SessionManager::new(db);
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_direct_session() {
        let (manager, _temp) = setup_manager().await;

        let session = manager
            .create_direct_session("user1".to_string(), "user2".to_string())
            .await
            .unwrap();

        assert_eq!(session.conversation_type, ConversationType::Direct);
        assert_eq!(session.participants.len(), 2);
        assert!(session.participants.contains(&"user1".to_string()));
        assert!(session.participants.contains(&"user2".to_string()));
    }

    #[tokio::test]
    async fn test_create_group_session() {
        let (manager, _temp) = setup_manager().await;

        let session = manager
            .create_group_session(
                "Test Group".to_string(),
                vec!["user1".to_string(), "user2".to_string(), "user3".to_string()],
            )
            .await
            .unwrap();

        assert_eq!(session.conversation_type, ConversationType::Group);
        assert_eq!(session.name, Some("Test Group".to_string()));
        assert_eq!(session.participants.len(), 3);
    }

    #[tokio::test]
    async fn test_add_remove_participant() {
        let (manager, _temp) = setup_manager().await;

        let session = manager
            .create_group_session(
                "Test Group".to_string(),
                vec!["user1".to_string(), "user2".to_string()],
            )
            .await
            .unwrap();

        // 添加参与者
        manager
            .add_participant(&session.id, "user3".to_string())
            .await
            .unwrap();

        let participants = manager.get_participants(&session.id).await.unwrap();
        assert_eq!(participants.len(), 3);

        // 移除参与者
        manager.remove_participant(&session.id, "user2").await.unwrap();

        let participants = manager.get_participants(&session.id).await.unwrap();
        assert_eq!(participants.len(), 2);
        assert!(!participants.contains(&"user2".to_string()));
    }

    #[tokio::test]
    async fn test_list_user_sessions() {
        let (manager, _temp) = setup_manager().await;

        // 创建多个会话
        manager
            .create_direct_session("user1".to_string(), "user2".to_string())
            .await
            .unwrap();
        manager
            .create_direct_session("user1".to_string(), "user3".to_string())
            .await
            .unwrap();
        manager
            .create_group_session(
                "Group".to_string(),
                vec!["user1".to_string(), "user4".to_string()],
            )
            .await
            .unwrap();

        let sessions = manager.list_user_sessions("user1").await.unwrap();
        assert_eq!(sessions.len(), 3);
    }
}
