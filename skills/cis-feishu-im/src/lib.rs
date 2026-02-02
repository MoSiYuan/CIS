//! # CIS Feishu IM Skill
//!
//! 飞书即时通讯集成 Skill，支持：
//! - **双模式运行**: 轮询 + Webhook，灵活配置
//! - **AI 对话**: 使用 cis-core::ai 或 cis-skill-sdk::ai
//! - **多轮对话**: 上下文管理
//! - **数据库分离**: IM 数据库独立于记忆数据库
//!
//! ## 架构原则
//!
//! 遵循 CIS 第一性原理：
//! - **本地主权**: IM 数据库独立于记忆数据库
//! - **灵活运行**: 支持轮询/Webhook/双模式
//! - **随时关机**: 关机即离线，开机即恢复
//! - **可配置**: 支持 Claude/Kimi AI Provider
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use cis_feishu_im::FeishuImSkill;
//!
//! let mut skill = FeishuImSkill::with_config(config);
//!
//! // 根据配置的 runtime_mode 自动启动
//! skill.start().await?;
//!
//! // 或者单独启动
//! skill.start_polling().await?;  // 仅轮询
//! skill.start_webhook().await?;  // 仅 Webhook
//! ```

#![cfg_attr(feature = "wasm", no_std)]

// 模块声明
mod config;
mod context;
mod error;
mod poller;
mod feishu_api;
mod session;
mod webhook;
mod feishu;

// WASM 模式使用 core
#[cfg(feature = "wasm")]
extern crate core;

use cis_skill_sdk::{
    Result, Skill, SkillContext, SkillConfig, Event, Error as SdkError,
};
use cis_core::ai::{self, AiProvider, ClaudeCliProvider, KimiCodeProvider, ProviderType};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

// 导出核心类型
pub use config::{FeishuImConfig, TriggerMode, RuntimeMode, ContextConfig, expand_path, WebhookConfig};
pub use context::ConversationContext;
pub use error::FeishuImError;
pub use feishu_api::FeishuApiClient;
pub use poller::{MessagePoller, PollingConfig};
pub use session::{FeishuSession, FeishuSessionManager, FeishuSessionType, FeishuSessionStatus};
pub use webhook::WebhookServer;

// 转换为 SDK Error
impl From<FeishuImError> for SdkError {
    fn from(err: FeishuImError) -> Self {
        SdkError::Other(err.to_string())
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

    /// 消息轮询器
    poller: Option<MessagePoller>,

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
            poller: None,
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
            poller: None,
            webhook_server: None,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动消息轮询
    pub async fn start_polling(&mut self) -> Result<()> {
        if *self.running.read().await {
            return Err(FeishuImError::Polling("轮询器已在运行".into()).into());
        }

        // 确保已初始化 AI Provider
        if self.ai_provider.is_none() {
            return Err(FeishuImError::Config("AI Provider 未初始化".into()).into());
        }

        // 创建轮询器
        let mut poller = MessagePoller::new(
            self.config.clone(),
            self.context.clone(),
            self.ai_provider.as_ref().unwrap().clone(),
        );

        // 启动轮询
        poller.start().await
            .map_err(|e| FeishuImError::Polling(e.to_string()))?;

        self.poller = Some(poller);
        *self.running.write().await = true;

        tracing::info!("✅ 消息轮询已启动");
        Ok(())
    }

    /// 停止消息轮询
    pub async fn stop_polling(&mut self) -> Result<()> {
        if let Some(mut poller) = self.poller.take() {
            poller.stop().await
                .map_err(|e| FeishuImError::Polling(e.to_string()))?;
            *self.running.write().await = false;
        }
        Ok(())
    }

    /// 启动 Webhook 服务器
    pub async fn start_webhook(&mut self) -> Result<()> {
        if self.webhook_server.is_some() {
            return Err(FeishuImError::Config("Webhook 服务器已在运行".into()).into());
        }

        // 确保已初始化 AI Provider
        if self.ai_provider.is_none() {
            return Err(FeishuImError::Config("AI Provider 未初始化".into()).into());
        }

        // 创建 Webhook 服务器
        let mut server = WebhookServer::new(
            self.config.clone(),
            self.context.clone(),
            self.ai_provider.as_ref().unwrap().clone(),
        );

        // 启动服务器
        server.start().await
            .map_err(|e| FeishuImError::Config(format!("Webhook 启动失败: {}", e)))?;

        self.webhook_server = Some(server);
        tracing::info!("✅ Webhook 服务器已启动");
        Ok(())
    }

    /// 停止 Webhook 服务器
    pub async fn stop_webhook(&mut self) -> Result<()> {
        if let Some(mut server) = self.webhook_server.take() {
            server.stop().await
                .map_err(|e| FeishuImError::Config(format!("Webhook 停止失败: {}", e)))?;
        }
        Ok(())
    }

    /// 根据 runtime_mode 自动启动服务
    pub async fn start(&mut self) -> Result<()> {
        match self.config.runtime_mode {
            RuntimeMode::PollingOnly => {
                self.start_polling().await?;
            }
            RuntimeMode::WebhookOnly => {
                self.start_webhook().await?;
            }
            RuntimeMode::Both => {
                self.start_polling().await?;
                self.start_webhook().await?;
            }
        }
        Ok(())
    }

    /// 停止所有服务
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_polling().await?;
        self.stop_webhook().await?;
        Ok(())
    }

    /// 检查运行状态
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 获取配置（只读）
    pub fn config(&self) -> &FeishuImConfig {
        &self.config
    }
}

/// 从 MessageContent 提取文本
fn extract_text_from_content(content: &cis_skill_sdk::im::MessageContent) -> String {
    match content {
        cis_skill_sdk::im::MessageContent::Text { text } => text.clone(),
        cis_skill_sdk::im::MessageContent::Markdown { text } => text.clone(),
        cis_skill_sdk::im::MessageContent::Html { html } => html.clone(),
        _ => String::new(),
    }
}

/// 实现 Skill trait
impl Skill for FeishuImSkill {
    fn name(&self) -> &str {
        "cis-feishu-im"
    }

    fn init(&mut self, skill_config: SkillConfig) -> Result<()> {
        // 只在配置为默认值时才从 SkillConfig 中加载
        if self.config.app_id.is_empty() && self.config.app_secret.is_empty() {
            self.config = skill_config
                .get::<FeishuImConfig>("feishu_im")
                .unwrap_or_default();
        }

        // 初始化 AI Provider（只在尚未初始化时）
        if self.ai_provider.is_none() {
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
        }

        tracing::info!("FeishuImSkill initialized: {:?}", self.config);
        Ok(())
    }

    fn handle_event(&self, _ctx: &dyn SkillContext, _event: Event) -> Result<()> {
        // 轮询模式下，事件处理由轮询器负责
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        tracing::info!("FeishuImSkill shutdown");
        Ok(())
    }
}
