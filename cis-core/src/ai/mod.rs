//! AI Provider 模块
//!
//! 提供统一的 AI 调用接口，支持 Claude CLI（默认）和 Kimi Code

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

mod claude;
mod kimi;

pub use claude::{ClaudeCliProvider, ClaudeConfig};
pub use kimi::{KimiCodeProvider, KimiConfig};

/// AI Provider 错误
#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider not available: {0}")]
    NotAvailable(String),
    
    #[error("CLI execution failed: {0}")]
    CliError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub type Result<T> = std::result::Result<T, AiError>;

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// AI Provider 统一接口
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;
    
    /// 检查是否可用（CLI 工具是否安装）
    async fn available(&self) -> bool;
    
    /// 简单对话
    async fn chat(&self, prompt: &str) -> Result<String>;
    
    /// 带上下文的对话
    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[Message],
    ) -> Result<String>;
    
    /// 生成结构化数据（JSON）
    async fn generate_json(
        &self,
        prompt: &str,
        schema: &str,
    ) -> Result<serde_json::Value>;
}

/// AI Provider 工厂
pub struct AiProviderFactory;

impl AiProviderFactory {
    /// 创建默认 Provider（Claude CLI）
    pub fn default_provider() -> Box<dyn AiProvider> {
        Box::new(ClaudeCliProvider::default())
    }
    
    /// 根据配置创建 Provider
    pub fn from_config(config: AiProviderConfig) -> Box<dyn AiProvider> {
        match config.provider_type {
            ProviderType::Claude => {
                Box::new(ClaudeCliProvider::new(config.claude.unwrap_or_default()))
            }
            ProviderType::Kimi => {
                Box::new(KimiCodeProvider::new(config.kimi.unwrap_or_default()))
            }
        }
    }
}

/// Provider 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Claude,
    Kimi,
}

/// AI Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    #[serde(default)]
    pub provider_type: ProviderType,
    
    pub claude: Option<ClaudeConfig>,
    pub kimi: Option<KimiConfig>,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: ProviderType::Claude,
            claude: Some(ClaudeConfig::default()),
            kimi: None,
        }
    }
}
