//! IM 即时通讯专用接口
//!
//! 为 IM Skill 开发提供统一的类型定义和 API。
//!
//! # 使用示例
//!
//! ```rust
//! use cis_skill_sdk::im::*;
//! use cis_skill_sdk::{Skill, SkillContext, Event, Result};
//!
//! pub struct MyImSkill;
//!
//! impl Skill for MyImSkill {
//!     fn name(&self) -> &str { "my-im-skill" }
//!     
//!     fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
//!         if let Event::Custom { name, data } = event {
//!             if name == "im:message" {
//!                 let msg: ImMessage = serde_json::from_value(data)?;
//!                 ctx.log_info(&format!("收到消息: {}", msg.content));
//!                 
//!                 // 回复消息
//!                 let reply = ImMessageBuilder::text("收到！")
//!                     .to(&msg.from)
//!                     .build();
//!                 
//!                 ctx.emit_event("im:send", 
//!                     &serde_json::to_vec(&reply)?)?;
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ==================== 消息类型 ====================

/// IM 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImMessage {
    /// 消息 ID（全局唯一）
    pub id: String,
    /// 发送者
    pub from: String,
    /// 接收者（用户或群组）
    pub to: String,
    /// 会话类型
    pub session_type: SessionType,
    /// 消息类型
    pub msg_type: MessageType,
    /// 消息内容（类型相关）
    pub content: MessageContent,
    /// 发送时间戳
    pub timestamp: u64,
    /// 引用/回复的消息 ID
    #[serde(default)]
    pub reply_to: Option<String>,
    /// @提及的用户列表
    #[serde(default)]
    pub mentions: Vec<String>,
    /// 额外元数据
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 会话类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    /// 单聊
    Private,
    /// 群聊
    Group,
    /// 频道
    Channel,
    /// 系统消息
    System,
}

/// 消息类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// 文本
    Text,
    /// 图片
    Image,
    /// 文件
    File,
    /// 语音
    Voice,
    /// 视频
    Video,
    /// 位置
    Location,
    /// 富文本（Markdown/HTML）
    RichText,
    /// 卡片/模板消息
    Card,
    /// 系统通知
    System,
    /// 撤回
    Recall,
    /// 自定义
    Custom,
}

/// 消息内容（根据消息类型变体）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessageContent {
    /// 纯文本
    Text { text: String },
    /// 图片
    Image { url: String, width: u32, height: u32, size: u64 },
    /// 文件
    File { name: String, url: String, size: u64, mime: String },
    /// 语音
    Voice { url: String, duration: u32 },
    /// 视频
    Video { url: String, duration: u32, cover: String },
    /// 位置
    Location { latitude: f64, longitude: f64, address: String },
    /// Markdown
    Markdown { text: String },
    /// HTML
    Html { html: String },
    /// 卡片消息
    Card { template: String, data: serde_json::Value },
    /// 系统通知
    System { code: String, params: HashMap<String, String> },
    /// 撤回
    Recall { original_id: String },
    /// 自定义
    Custom { payload: serde_json::Value },
}

// ==================== 消息构建器 ====================

/// IM 消息构建器
///
/// 提供流式 API 构建消息
///
/// # 示例
///
/// ```rust
/// use cis_skill_sdk::im::ImMessageBuilder;
///
/// let msg = ImMessageBuilder::text("Hello!")
///     .to("user123")
///     .in_private()
///     .build();
/// ```
pub struct ImMessageBuilder {
    msg: ImMessage,
}

impl ImMessageBuilder {
    /// 创建文本消息
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            msg: ImMessage {
                id: String::new(),
                from: String::new(),
                to: String::new(),
                session_type: SessionType::Private,
                msg_type: MessageType::Text,
                content: MessageContent::Text { text: content.into() },
                timestamp: 0,
                reply_to: None,
                mentions: vec![],
                metadata: HashMap::new(),
            },
        }
    }
    
    /// 创建 Markdown 消息
    pub fn markdown(text: impl Into<String>) -> Self {
        let mut builder = Self::text("");
        builder.msg.msg_type = MessageType::RichText;
        builder.msg.content = MessageContent::Markdown { text: text.into() };
        builder
    }
    
    /// 创建图片消息
    pub fn image(url: impl Into<String>) -> Self {
        let mut builder = Self::text("");
        builder.msg.msg_type = MessageType::Image;
        builder.msg.content = MessageContent::Image {
            url: url.into(),
            width: 0,
            height: 0,
            size: 0,
        };
        builder
    }
    
    /// 设置接收者
    pub fn to(mut self, to: impl Into<String>) -> Self {
        self.msg.to = to.into();
        self
    }
    
    /// 设置发送者
    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.msg.from = from.into();
        self
    }
    
    /// 单聊会话
    pub fn in_private(mut self) -> Self {
        self.msg.session_type = SessionType::Private;
        self
    }
    
    /// 群聊会话
    pub fn in_group(mut self) -> Self {
        self.msg.session_type = SessionType::Group;
        self
    }
    
    /// 频道会话
    pub fn in_channel(mut self) -> Self {
        self.msg.session_type = SessionType::Channel;
        self
    }
    
    /// 回复消息
    pub fn reply_to(mut self, msg_id: impl Into<String>) -> Self {
        self.msg.reply_to = Some(msg_id.into());
        self
    }
    
    /// @提及用户
    pub fn mention(mut self, user: impl Into<String>) -> Self {
        self.msg.mentions.push(user.into());
        self
    }
    
    /// 构建消息
    pub fn build(self) -> ImMessage {
        self.msg
    }
}

// ==================== 用户与群组 ====================

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub status: UserStatus,
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// 用户状态
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    #[default]
    Offline,
    Online,
    Away,
    Busy,
    DoNotDisturb,
}

/// 群组信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub owner: String,
    pub members: Vec<GroupMember>,
    #[serde(default)]
    pub settings: GroupSettings,
}

/// 群组成员
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub user_id: String,
    pub role: GroupRole,
    pub join_time: u64,
}

/// 群组角色
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupRole {
    #[default]
    Member,
    Admin,
    Owner,
}

/// 群组设置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupSettings {
    pub max_members: u32,
    pub invite_only: bool,
    pub mute_all: bool,
}

// ==================== 事件 ====================

/// IM 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum ImEvent {
    /// 收到消息
    MessageReceived(ImMessage),
    /// 消息已发送
    MessageSent { id: String, timestamp: u64 },
    /// 消息已读
    MessageRead { msg_id: String, by: String },
    /// 消息撤回
    MessageRecalled { msg_id: String, by: String },
    /// 用户上线
    UserOnline { user_id: String },
    /// 用户下线
    UserOffline { user_id: String, last_seen: u64 },
    /// 被邀请进群
    InvitedToGroup { group_id: String, by: String },
    /// 被移出群组
    RemovedFromGroup { group_id: String, by: String },
    /// 群组信息变更
    GroupUpdated { group_id: String, changes: HashMap<String, serde_json::Value> },
    /// 输入状态
    Typing { session_id: String, user_id: String },
    /// 会话未读数更新
    UnreadUpdate { session_id: String, count: u32 },
    /// 连接状态变更
    ConnectionStatus { connected: bool },
}

// ==================== API 请求/响应 ====================

/// 发送消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub message: ImMessage,
    #[serde(default)]
    pub options: SendOptions,
}

/// 发送选项
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendOptions {
    /// 是否需要送达确认
    #[serde(default)]
    pub need_ack: bool,
    /// 重试次数
    #[serde(default = "default_retry")]
    pub retry: u32,
    /// 超时时间（毫秒）
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_retry() -> u32 { 3 }
fn default_timeout() -> u64 { 30000 }

/// 发送消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub success: bool,
    pub message_id: Option<String>,
    pub error: Option<String>,
    pub timestamp: Option<u64>,
}

/// 获取历史消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetHistoryRequest {
    pub session_id: String,
    #[serde(default)]
    pub before: Option<u64>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 { 20 }

/// 历史消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetHistoryResponse {
    pub messages: Vec<ImMessage>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub session_type: SessionType,
    pub name: String,
    pub avatar: Option<String>,
    pub last_message: Option<ImMessage>,
    pub unread_count: u32,
    pub last_update: u64,
    pub pinned: bool,
    pub muted: bool,
}

// ==================== IM Context 扩展 ====================

/// IM Skill 上下文扩展
///
/// 为 SkillContext 添加 IM 相关便捷方法
pub trait ImContextExt {
    /// 发送消息
    fn im_send(&self, message: &ImMessage) -> crate::error::Result<SendMessageResponse>;
    
    /// 回复消息
    fn im_reply(&self, original: &ImMessage, content: MessageContent) -> crate::error::Result<()>;
    
    /// 获取会话列表
    fn im_get_sessions(&self) -> crate::error::Result<Vec<Session>>;
    
    /// 获取历史消息
    fn im_get_history(&self, req: &GetHistoryRequest) -> crate::error::Result<GetHistoryResponse>;
    
    /// 标记已读
    fn im_mark_read(&self, session_id: &str, msg_id: &str) -> crate::error::Result<()>;
    
    /// 撤回消息
    fn im_recall(&self, msg_id: &str) -> crate::error::Result<()>;
    
    /// 获取用户信息
    fn im_get_user(&self, user_id: &str) -> crate::error::Result<Option<User>>;
    
    /// 获取群组信息
    fn im_get_group(&self, group_id: &str) -> crate::error::Result<Option<Group>>;
}
