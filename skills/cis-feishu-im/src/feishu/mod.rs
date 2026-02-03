//! 飞书 API 集成模块
//!
//! 封装飞书 SDK 和数据结构

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 飞书消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    /// 消息 ID（全局唯一）
    pub message_id: String,

    /// 消息类型
    pub msg_type: String,

    /// 发送者信息
    pub sender: FeishuSender,

    /// 消息内容
    pub content: FeishuContent,

    /// 会话类型
    pub chat_type: Option<String>,

    /// 群聊 ID（如果是群聊）
    pub chat_id: Option<String>,

    /// 是否被 @
    pub is_at: bool,

    /// 时间戳
    pub timestamp: Option<u64>,

    /// 额外元数据
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 飞书发送者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSender {
    /// 用户 ID
    pub user_id: String,

    /// 用户名
    pub name: String,

    /// 头像 URL
    pub avatar_url: Option<String>,
}

/// 飞书消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuContent {
    /// 文本内容
    pub text: Option<String>,

    /// 富文本内容
    pub post: Option<FeishuPost>,

    /// 卡片内容
    pub card: Option<serde_json::Value>,

    /// 其他内容
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 飞书富文本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuPost {
    /// 富文本内容（多语言）
    pub zh_cn: FeishuPostContent,
}

/// 飞书富文本内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuPostContent {
    /// 标题
    pub title: Option<String>,

    /// 内容元素
    pub content: Vec<FeishuTextElement>,
}

/// 飞书文本元素
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tag")]
pub enum FeishuTextElement {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "a")]
    Link { text: String, href: String },

    #[serde(rename = "at")]
    At { user_id: String, name: String },

    #[serde(rename = "img")]
    Image { image_key: String },

    #[serde(rename = "media")]
    Media { file_key: String, image_key: Option<String> },
}

/// 飞书用户事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuUserEvent {
    /// 用户 ID 列表
    pub user_ids: Vec<String>,

    /// 群组 ID
    pub chat_id: String,

    /// 操作者
    pub operator_id: Option<String>,

    /// 时间戳
    pub timestamp: u64,
}

/// 飞书事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FeishuEvent {
    /// 收到消息
    #[serde(rename = "message")]
    MessageReceived(FeishuMessage),

    /// 用户加入群组
    #[serde(rename = "user_added")]
    UserJoined(FeishuUserEvent),

    /// 用户离开群组
    #[serde(rename = "user_removed")]
    UserLeft(FeishuUserEvent),

    /// 群组信息变更
    #[serde(rename = "group_updated")]
    GroupUpdated,
}

impl FeishuMessage {
    /// 提取用户消息文本
    pub fn extract_text(&self) -> String {
        if let Some(ref text) = self.content.text {
            return text.clone();
        }

        if let Some(ref post) = self.content.post {
            let mut result = String::new();
            for elem in &post.zh_cn.content {
                match elem {
                    FeishuTextElement::Text { text } => result.push_str(text),
                    FeishuTextElement::Link { text, .. } => result.push_str(text),
                    FeishuTextElement::At { name, .. } => result.push_str(&format!("@{}", name)),
                    FeishuTextElement::Image { .. } => result.push_str("[图片]"),
                    FeishuTextElement::Media { .. } => result.push_str("[媒体]"),
                }
            }
            return result;
        }

        String::new()
    }

    /// 获取会话 ID
    pub fn get_session_id(&self) -> String {
        if let Some(ref chat_id) = self.chat_id {
            return chat_id.clone();
        }

        // 私聊使用用户 ID
        format!("p2p:{}", self.sender.user_id)
    }

    /// 转换为 JSON Value
    pub fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// 从 JSON Value 解析
    pub fn from_json(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

/// 飞书 API 客户端
pub struct FeishuClient {
    app_id: String,
    app_secret: String,
    http_client: reqwest::Client,
}

impl FeishuClient {
    /// 创建新的飞书客户端
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            http_client: reqwest::Client::new(),
        }
    }

    /// 发送文本消息
    pub async fn send_text(&self, chat_id: &str, text: &str) -> Result<(), FeishuApiError> {
        // TODO: 实现飞书 API 调用
        // POST /open-apis/im/v1/messages
        tracing::info!("发送文本消息到飞书: chat_id={}, text={}", chat_id, text);
        Ok(())
    }

    /// 发送卡片消息
    pub async fn send_card(&self, chat_id: &str, _card: serde_json::Value) -> Result<(), FeishuApiError> {
        // TODO: 实现飞书 API 调用
        tracing::info!("发送卡片消息到飞书: chat_id={}", chat_id);
        Ok(())
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, user_id: &str) -> Result<FeishuSender, FeishuApiError> {
        // TODO: 实现飞书 API 调用
        // GET /open-apis/contact/v3/users/:user_id
        tracing::info!("获取用户信息: user_id={}", user_id);
        Ok(FeishuSender {
            user_id: user_id.to_string(),
            name: "Unknown".to_string(),
            avatar_url: None,
        })
    }

    /// 获取群组信息
    pub async fn get_group_info(&self, chat_id: &str) -> Result<GroupInfo, FeishuApiError> {
        // TODO: 实现飞书 API 调用
        // GET /open-apis/im/v1/chats/:chat_id
        tracing::info!("获取群组信息: chat_id={}", chat_id);
        Ok(GroupInfo {
            chat_id: chat_id.to_string(),
            name: "Unknown Group".to_string(),
            owner: "Unknown".to_string(),
            member_count: 0,
        })
    }
}

/// 群组信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub chat_id: String,
    pub name: String,
    pub owner: String,
    pub member_count: u32,
}

/// 飞书 API 错误
#[derive(Debug, thiserror::Error)]
pub enum FeishuApiError {
    #[error("网络请求失败: {0}")]
    Request(#[from] reqwest::Error),

    #[error("API 返回错误: code={0}, msg={1}")]
    ApiError(i32, String),

    #[error("序列化错误: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("其他错误: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text() {
        let msg = FeishuMessage {
            message_id: "test".to_string(),
            msg_type: "text".to_string(),
            sender: FeishuSender {
                user_id: "user123".to_string(),
                name: "Test User".to_string(),
                avatar_url: None,
            },
            content: FeishuContent {
                text: Some("Hello, World!".to_string()),
                post: None,
                card: None,
                extra: HashMap::new(),
            },
            chat_type: Some("p2p".to_string()),
            chat_id: None,
            is_at: false,
            timestamp: None,
            extra: HashMap::new(),
        };

        assert_eq!(msg.extract_text(), "Hello, World!");
    }

    #[test]
    fn test_get_session_id() {
        let msg = FeishuMessage {
            message_id: "test".to_string(),
            msg_type: "text".to_string(),
            sender: FeishuSender {
                user_id: "user123".to_string(),
                name: "Test User".to_string(),
                avatar_url: None,
            },
            content: FeishuContent {
                text: Some("Hello".to_string()),
                post: None,
                card: None,
                extra: HashMap::new(),
            },
            chat_type: Some("p2p".to_string()),
            chat_id: None,
            is_at: false,
            timestamp: None,
            extra: HashMap::new(),
        };

        assert_eq!(msg.get_session_id(), "p2p:user123");
    }

    #[test]
    fn test_message_serialization() {
        let msg = FeishuMessage {
            message_id: "test".to_string(),
            msg_type: "text".to_string(),
            sender: FeishuSender {
                user_id: "user123".to_string(),
                name: "Test User".to_string(),
                avatar_url: None,
            },
            content: FeishuContent {
                text: Some("Hello".to_string()),
                post: None,
                card: None,
                extra: HashMap::new(),
            },
            chat_type: Some("p2p".to_string()),
            chat_id: None,
            is_at: false,
            timestamp: Some(1234567890),
            extra: HashMap::new(),
        };

        let json = msg.to_json().unwrap();
        let decoded = FeishuMessage::from_json(json).unwrap();

        assert_eq!(decoded.message_id, msg.message_id);
        assert_eq!(decoded.sender.user_id, msg.sender.user_id);
    }
}
