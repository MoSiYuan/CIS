//! # CIS Feishu IM Skill
//!
//! 飞书即时通讯集成 Skill，支持：
//! - Webhook 消息接收
//! - AI 对话响应（使用 cis-core::ai 或 cis-skill-sdk::ai）
//! - 多轮对话上下文管理
//! - 严格分离 IM 数据库和记忆数据库
//!
//! ## 架构原则
//!
//! 遵循 CIS 第一性原理：
//! - **本地主权**: IM 数据库独立于记忆数据库
//! - **零轮询**: Webhook 被动触发
//! - **可配置**: 支持 Claude/Kimi AI Provider
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use cis_feishu_im::FeishuImSkill;
//! use cis_skill_sdk::Skill;
//!
//! let mut skill = FeishuImSkill::new();
//! skill.init(config)?;
//!
//! // 启动 Webhook 服务器
//! skill.start_webhook().await?;
//! ```

#![cfg_attr(feature = "wasm", no_std)]

// 模块声明
mod config;
mod context;
mod webhook;
mod feishu;

// WASM 模式使用 core
#[cfg(feature = "wasm")]
extern crate core;

use cis_skill_sdk::{
    im::{self, ImMessage, MessageContent, SessionType},
    Result, Skill, SkillContext, SkillConfig, Event, Error as SdkError,
};
use cis_core::ai::{self, AiProvider, AiProviderFactory, ClaudeCliProvider, KimiCodeProvider, Message as CoreMessage, ProviderType};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

// 导出核心类型
pub use config::{FeishuImConfig, TriggerMode, ContextConfig, expand_path};
pub use context::ConversationContext;
pub use webhook::WebhookServer;

/// Feishu IM Skill 错误
#[derive(Error, Debug)]
pub enum FeishuImError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("Webhook 错误: {0}")]
    Webhook(String),

    #[error("AI 调用错误: {0}")]
    Ai(String),

    #[error("数据库错误: {0}")]
    Database(String),

    #[error("飞书 API 错误: {0}")]
    FeishuApi(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

// 转换为 SDK Error
impl From<FeishuImError> for SdkError {
    fn from(err: FeishuImError) -> Self {
        SdkError::Other(err.to_string())
    }
}

/// 从 MessageContent 提取文本
fn extract_text_from_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text { text } => text.clone(),
        MessageContent::Markdown { text } => text.clone(),
        MessageContent::Html { html } => html.clone(),
        _ => String::new(),
    }
}

/// Feishu IM Skill
///
/// 实现 `cis_skill_sdk::Skill` trait
pub struct FeishuImSkill {
    /// 配置
    config: FeishuImConfig,

    /// 对话上下文管理
    context: Arc<ConversationContext>,

    /// AI Provider
    ai_provider: Option<Arc<dyn AiProvider>>,

    /// Webhook 服务器
    webhook_server: Option<WebhookServer>,

    /// 运行状态
    running: Arc<RwLock<bool>>,
}

impl FeishuImSkill {
    /// 创建新的 Skill 实例
    pub fn new() -> Self {
        Self {
            config: FeishuImConfig::default(),
            context: Arc::new(ConversationContext::default()),
            ai_provider: None,
            webhook_server: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 使用指定配置创建
    pub fn with_config(config: FeishuImConfig) -> Self {
        let context_config = config.context_config.clone();
        Self {
            config,
            context: Arc::new(ConversationContext::new(context_config)),
            ai_provider: None,
            webhook_server: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动 Webhook 服务器
    pub async fn start_webhook(&mut self) -> Result<()> {
        if *self.running.read().await {
            return Err(FeishuImError::Webhook("服务器已在运行".into()).into());
        }

        // 创建 Webhook 服务器
        let mut server = WebhookServer::new(
            self.config.clone(),
            self.context.clone(),
            self.ai_provider.as_ref().unwrap().clone(),
        );

        // 启动服务器
        server.start().await.map_err(|e| FeishuImError::Webhook(e.to_string()))?;

        self.webhook_server = Some(server);
        *self.running.write().await = true;

        Ok(())
    }

    /// 停止 Webhook 服务器
    pub async fn stop_webhook(&mut self) -> Result<()> {
        if let Some(mut server) = self.webhook_server.take() {
            server.stop().await.map_err(|e| FeishuImError::Webhook(e.to_string()))?;
            *self.running.write().await = false;
        }
        Ok(())
    }

    /// 检查运行状态
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 处理飞书消息（内部方法）
    async fn handle_feishu_message(
        &self,
        ctx: &dyn SkillContext,
        feishu_msg: feishu::FeishuMessage,
    ) -> Result<()> {
        // 1. 转换为统一的 ImMessage
        let im_msg = self.convert_to_im_message(&feishu_msg)?;

        // 2. 检查是否应该响应
        if !self.should_respond(&feishu_msg) {
            ctx.log_debug(&format!("消息不满足触发条件，忽略: {:?}", feishu_msg));
            return Ok(());
        }

        // 3. 获取会话 ID
        let session_id = im_msg.to.clone();

        // 4. 获取对话历史
        let history = self.context.get_history(&session_id).await;

        // 5. 构建 AI 请求
        let mut messages = history;
        let user_text = extract_text_from_content(&im_msg.content);
        messages.push(CoreMessage::user(user_text.clone()));

        // 6. 调用 AI
        let system_prompt = self.build_system_prompt(&feishu_msg);
        let response = if let Some(provider) = &self.ai_provider {
            provider
                .chat_with_context(&system_prompt, &messages)
                .await
                .map_err(|e| FeishuImError::Ai(e.to_string()))?
        } else {
            // 使用 SDK 提供的 AI 接口
            let prompt = messages
                .iter()
                .map(|m| match m.role {
                    ai::Role::System => format!("System: {}\n", m.content),
                    ai::Role::User => format!("User: {}\n", m.content),
                    ai::Role::Assistant => format!("Assistant: {}\n", m.content),
                })
                .collect::<String>();

            ctx.ai_chat(&prompt).map_err(|e| FeishuImError::Ai(e.to_string()))?
        };

        // 7. 保存对话历史到 IM 数据库（不是记忆数据库！）
        self.context
            .add_message(&session_id, CoreMessage::user(user_text))
            .await;
        self.context
            .add_message(&session_id, CoreMessage::assistant(response.clone()))
            .await;

        // 8. 可选：同步重要信息到记忆数据库
        if self.config.context_config.sync_to_memory {
            self.sync_to_memory(ctx, &feishu_msg, &response).await?;
        }

        // 9. 发送回复到飞书
        self.send_reply(&feishu_msg, &response).await?;

        Ok(())
    }

    /// 转换飞书消息为 ImMessage
    fn convert_to_im_message(&self, msg: &feishu::FeishuMessage) -> Result<ImMessage> {
        let (msg_type, content) = match msg.msg_type.as_str() {
            "text" => (
                im::MessageType::Text,
                MessageContent::Text {
                    text: msg.content.text.clone().unwrap_or_default(),
                },
            ),
            "post" => (
                im::MessageType::RichText,
                MessageContent::Markdown {
                    text: msg.extract_text(),
                },
            ),
            _ => (
                im::MessageType::Text,
                MessageContent::Text {
                    text: msg.extract_text(),
                },
            ),
        };

        let session_type = match msg.chat_type.as_deref() {
            Some("p2p") => SessionType::Private,
            Some("group") => SessionType::Group,
            _ => SessionType::Private,
        };

        Ok(ImMessage {
            id: msg.message_id.clone(),
            from: msg.sender.user_id.clone(),
            to: msg.chat_id.clone().unwrap_or_else(|| format!("p2p:{}", msg.sender.user_id)),
            session_type,
            msg_type,
            content,
            timestamp: msg.timestamp.unwrap_or_else(|| chrono::Utc::now().timestamp() as u64),
            reply_to: None,
            mentions: vec![],
            metadata: Default::default(),
        })
    }

    /// 判断是否应该响应
    fn should_respond(&self, msg: &feishu::FeishuMessage) -> bool {
        match self.config.trigger_mode {
            TriggerMode::AtMentionOnly => msg.is_at,
            TriggerMode::PrivateAndAtMention => {
                msg.is_at || msg.chat_type.as_deref() == Some("p2p")
            }
            TriggerMode::All => true,
        }
    }

    /// 构建 AI 系统提示词
    fn build_system_prompt(&self, msg: &feishu::FeishuMessage) -> String {
        format!(
            "你是 CIS 飞书机器人助手，负责回答用户问题。\n\n\
             用户信息: {}\n\
             会话类型: {:?}\n\n\
             请用简洁、友好的语气回复。",
            msg.sender.name,
            msg.chat_type
        )
    }

    /// 同步重要信息到记忆数据库
    async fn sync_to_memory(
        &self,
        ctx: &dyn SkillContext,
        msg: &feishu::FeishuMessage,
        response: &str,
    ) -> Result<()> {
        // 提取关键词和摘要
        let text = msg.extract_text();

        // 简单判断是否重要（包含特定关键词）
        let important_keywords = ["记住", "重要", "笔记", "总结", "任务", "计划"];
        let is_important = important_keywords.iter().any(|kw| text.contains(kw));

        if is_important {
            // 存储到记忆数据库（通过 SkillContext）
            let key = format!("feishu:memory:{}:{}", msg.chat_type.as_deref().unwrap_or("p2p"), msg.message_id);
            let value = serde_json::to_vec(&(text, response)).unwrap();
            ctx.memory_set(&key, &value).map_err(|e| FeishuImError::Database(e.to_string()))?;

            ctx.log_info(&format!("重要信息已同步到记忆: {}", key));
        }

        Ok(())
    }

    /// 发送回复到飞书
    async fn send_reply(&self, original_msg: &feishu::FeishuMessage, reply: &str) -> Result<()> {
        // TODO: 实现飞书 API 调用
        // 使用 larkrs-client 发送消息
        tracing::info!("发送回复到飞书: chat_id={:?}", original_msg.chat_type);
        Ok(())
    }
}

impl Default for FeishuImSkill {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Skill Trait 实现 ====================

impl Skill for FeishuImSkill {
    fn name(&self) -> &str {
        "cis-feishu-im"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "飞书即时通讯集成 - AI 对话助手"
    }

    fn init(&mut self, config: SkillConfig) -> Result<()> {
        // 解析配置
        self.config = config
            .get::<FeishuImConfig>("feishu_im")
            .unwrap_or_default();

        // 初始化 AI Provider
        // 从配置创建 AI Provider
        let provider: Arc<dyn AiProvider> = match self.config.ai_provider.provider_type {
            ProviderType::Claude => {
                Arc::new(ClaudeCliProvider::new(
                    self.config.ai_provider.claude.clone().unwrap_or_default()
                ))
            }
            ProviderType::Kimi => {
                Arc::new(KimiCodeProvider::new(
                    self.config.ai_provider.kimi.clone().unwrap_or_default()
                ))
            }
        };
        self.ai_provider = Some(provider);

        tracing::info!("FeishuImSkill initialized: {:?}", self.config);
        Ok(())
    }

    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        // 同步处理事件（基础 Skill trait）
        match event {
            Event::Custom { name, data } => match name.as_str() {
                "feishu:message_received" => {
                    if let Ok(msg) = serde_json::from_value::<feishu::FeishuMessage>(data) {
                        let ctx_clone = ctx.clone(); // TODO: 实现克隆
                        // TODO: 异步处理
                        // tokio::spawn(async move {
                        //     self.handle_feishu_message(&ctx_clone, msg).await
                        // });
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        tracing::info!("FeishuImSkill shutdown");
        Ok(())
    }
}

// ==================== NativeSkill Trait 实现 (异步) ====================

#[cfg(feature = "native")]
use cis_skill_sdk::NativeSkill;

#[cfg(feature = "native")]
#[async_trait::async_trait]
impl NativeSkill for FeishuImSkill {
    fn name(&self) -> &str {
        "cis-feishu-im"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "飞书即时通讯集成 - AI 对话助手（Native 模式）"
    }

    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        // 解析配置
        self.config = config
            .get::<FeishuImConfig>("feishu_im")
            .unwrap_or_default();

        // 初始化 AI Provider
        // 从配置创建 AI Provider
        let provider: Arc<dyn AiProvider> = match self.config.ai_provider.provider_type {
            ProviderType::Claude => {
                Arc::new(ClaudeCliProvider::new(
                    self.config.ai_provider.claude.clone().unwrap_or_default()
                ))
            }
            ProviderType::Kimi => {
                Arc::new(KimiCodeProvider::new(
                    self.config.ai_provider.kimi.clone().unwrap_or_default()
                ))
            }
        };
        self.ai_provider = Some(provider);

        // 初始化对话上下文
        self.context = Arc::new(ConversationContext::new(self.config.context_config.clone()));

        tracing::info!("FeishuImSkill initialized (async): {:?}", self.config);
        Ok(())
    }

    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => {
                ctx.log_debug(&format!("收到事件: {}", name));

                match name.as_str() {
                    "feishu:message_received" => {
                        if let Ok(msg) = serde_json::from_value::<feishu::FeishuMessage>(data) {
                            self.handle_feishu_message(ctx, msg).await?;
                        }
                    }
                    "feishu:user_joined" => {
                        ctx.log_info(&format!("用户加入群组: {:?}", data));
                    }
                    "feishu:user_left" => {
                        ctx.log_info(&format!("用户离开群组: {:?}", data));
                    }
                    _ => {}
                }
            }
            Event::Init { config } => {
                ctx.log_info("FeishuImSkill 收到 Init 事件");
            }
            Event::Shutdown => {
                ctx.log_info("FeishuImSkill 收到 Shutdown 事件");
                // 停止 Webhook 服务器
                // Note: 需要 Arc<Mutex<Self>> 才能调用 stop_webhook
            }
            _ => {}
        }
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("FeishuImSkill shutdown (async)");
        Ok(())
    }
}

// ==================== WASM 导出 ====================

#[cfg(feature = "wasm")]
#[no_mangle]
pub extern "C" fn skill_init() -> i32 {
    0
}

#[cfg(feature = "wasm")]
#[no_mangle]
pub extern "C" fn skill_handle_event(_event_ptr: *const u8, _event_len: usize) -> i32 {
    0
}

#[cfg(feature = "wasm")]
#[no_mangle]
pub extern "C" fn skill_shutdown() -> i32 {
    0
}
