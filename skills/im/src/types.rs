//! IM Skill 类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 消息 ID
pub type MessageId = String;

/// 会话 ID
pub type ConversationId = String;

/// 用户 ID
pub type UserId = String;

/// 消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum MessageContent {
    /// 文本消息
    #[serde(rename = "text")]
    Text { text: String },
    
    /// 图片消息
    #[serde(rename = "image")]
    Image { 
        url: String, 
        width: Option<u32>, 
        height: Option<u32>,
        alt_text: Option<String>,
    },
    
    /// 文件消息
    #[serde(rename = "file")]
    File { 
        name: String, 
        url: String, 
        size: u64,
        mime_type: Option<String>,
    },
    
    /// 语音消息
    #[serde(rename = "voice")]
    Voice { 
        url: String, 
        duration_secs: u32,
    },
    
    /// 引用回复
    #[serde(rename = "reply")]
    Reply { 
        reply_to: MessageId,
        content: Box<MessageContent>,
    },
}

/// 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub sender_id: UserId,
    pub content: MessageContent,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub read_by: Vec<UserId>,
    pub metadata: serde_json::Value,
}

impl Message {
    pub fn new(
        conversation_id: ConversationId,
        sender_id: UserId,
        content: MessageContent,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            conversation_id,
            sender_id,
            content,
            created_at: Utc::now(),
            updated_at: None,
            read_by: Vec::new(),
            metadata: serde_json::Value::Null,
        }
    }
    
    pub fn mark_read(&mut self, user_id: UserId) {
        if !self.read_by.contains(&user_id) {
            self.read_by.push(user_id);
        }
    }
}

/// 会话类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationType {
    /// 一对一聊天
    Direct,
    /// 群组聊天
    Group,
    /// 频道（广播）
    Channel,
}

/// 会话结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub conversation_type: ConversationType,
    pub name: Option<String>,
    pub participants: Vec<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub avatar_url: Option<String>,
    pub metadata: serde_json::Value,
}

/// 用户资料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// 用户状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    #[default]
    Offline,
    Online,
    Away,
    Busy,
    Invisible,
}

/// IM Skill 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImConfig {
    pub max_message_length: usize,
    pub max_file_size: u64,
    pub message_retention_days: i64,
    pub enable_reactions: bool,
    pub enable_editing: bool,
    pub enable_deletion: bool,
}

impl Default for ImConfig {
    fn default() -> Self {
        Self {
            max_message_length: 4096,
            max_file_size: 100 * 1024 * 1024, // 100MB
            message_retention_days: 365,
            enable_reactions: true,
            enable_editing: true,
            enable_deletion: true,
        }
    }
}

impl MessageContent {
    /// 获取内容类型字符串
    pub fn content_type(&self) -> &'static str {
        match self {
            MessageContent::Text { .. } => "text",
            MessageContent::Image { .. } => "image",
            MessageContent::File { .. } => "file",
            MessageContent::Voice { .. } => "voice",
            MessageContent::Reply { .. } => "reply",
        }
    }
    
    /// 获取文本内容（如果是文本消息）
    pub fn text_content(&self) -> Option<&str> {
        match self {
            MessageContent::Text { text } => Some(text),
            MessageContent::Reply { content, .. } => content.text_content(),
            _ => None,
        }
    }
}
