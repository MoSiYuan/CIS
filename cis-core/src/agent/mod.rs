//! # Agent Provider 模块
//!
//! 提供统一的 LLM Agent 抽象接口，支持双向调用：
//! - CIS → Agent: CIS 调用外部 LLM Agent
//! - Agent → CIS: 外部 Agent 通过 CLI/API 调用 CIS
//!
//! ## Multi-Agent Architecture (Phase 7)
//!
//! This module includes:
//! - Receptionist Agent: Entry point that routes requests to appropriate workers
//! - Worker Agents: Specialized agents for different task types

pub mod receptionist;
pub mod worker;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::error::Result;

pub mod bridge;
pub mod builder;    // Builder 模式强制执行（P1.7.0 任务组 0.4）
pub mod cluster;
pub mod executor;   // 单个任务执行（P1.7.0 任务组 0.3）
pub mod config;
pub mod federation;
pub mod federation_client;
pub mod persistent;
pub mod process_detector;
pub mod providers;
pub mod security;
pub mod guard;
pub mod leak_detector;

pub use guard::{
    AgentGuard,
    GuardId,
    LeakDetector,
    LeakedGuard,
    GuardStats,
    GuardStatsSummary,
    AgentCleanupError,
};

pub use leak_detector::{AgentLeakDetector, LeakReport, LeakedAgent, LeakSeverity, LeakSummary};

pub use bridge::AgentBridgeSkill;
pub use builder::AgentTaskBuilder;  // Builder API（P1.7.0 任务组 0.4）
pub use cluster::{SessionManager, SessionId, SessionEvent, SessionState};
pub use executor::{AgentExecutor, AgentResult};  // Executor API（P1.7.0 任务组 0.3）
pub use config::{AgentCommandConfig, AgentMode};

// Multi-Agent exports (Phase 7)
pub use receptionist::{ReceptionistAgent, ReceptionistConfig, RoutingDecision};
pub use worker::{WorkerAgent, WorkerAgentConfig, WorkerType};

/// Agent 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    /// 主指令/Prompt
    pub prompt: String,
    /// 上下文信息
    pub context: AgentContext,
    /// 允许使用的 Skill 列表
    pub skills: Vec<String>,
    /// 系统提示词（覆盖默认）
    pub system_prompt: Option<String>,
    /// 会话历史
    pub history: Vec<AgentMessage>,
}

/// Agent 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Agent 上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// 工作目录
    pub work_dir: Option<PathBuf>,
    /// 允许访问的记忆前缀
    pub memory_access: Vec<String>,
    /// 项目配置
    pub project_config: Option<crate::project::ProjectConfig>,
    /// 额外上下文数据
    pub extra: HashMap<String, serde_json::Value>,
}

impl AgentContext {
    pub fn new() -> Self {
        Self {
            work_dir: None,
            memory_access: vec![],
            project_config: None,
            extra: HashMap::new(),
        }
    }

    pub fn with_work_dir(mut self, dir: PathBuf) -> Self {
        self.work_dir = Some(dir);
        self
    }

    pub fn with_memory_access(mut self, prefixes: Vec<String>) -> Self {
        self.memory_access = prefixes;
        self
    }
}

impl Default for AgentContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// 响应内容
    pub content: String,
    /// 使用的 Token 数（如果可用）
    pub token_usage: Option<TokenUsage>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

/// Agent Provider 统一接口
///
/// 所有 LLM Agent（Claude, Kimi, Aider, 等）实现此接口
#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;

    /// Provider 版本
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// 检查 Agent 是否可用
    async fn available(&self) -> bool;

    /// 执行指令（同步返回）
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse>;

    /// 流式执行
    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse>;

    /// 初始化（可选）
    async fn init(&mut self, _context: AgentContext) -> Result<()> {
        Ok(())
    }

    /// 获取 Agent 能力描述
    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::default()
    }
}

/// Agent 能力描述
#[derive(Debug, Clone, Default)]
pub struct AgentCapabilities {
    /// 是否支持流式输出
    pub streaming: bool,
    /// 是否支持工具调用
    pub tool_calling: bool,
    /// 是否支持多模态
    pub multimodal: bool,
    /// 最大上下文长度
    pub max_context_length: Option<usize>,
    /// 支持的模型列表
    pub supported_models: Vec<String>,
}

/// Agent Provider 工厂
pub struct AgentProviderFactory;

impl AgentProviderFactory {
    /// 根据配置创建 Provider
    pub fn create(config: &AgentConfig) -> Result<Box<dyn AgentProvider>> {
        match config.provider_type {
            AgentType::Claude => Ok(Box::new(providers::ClaudeProvider::new(config.clone()))),
            AgentType::Kimi => Ok(Box::new(providers::KimiProvider::new(config.clone()))),
            AgentType::Aider => Ok(Box::new(providers::AiderProvider::new(config.clone()))),
            AgentType::OpenCode => Ok(Box::new(providers::OpenCodeProvider::new(config.clone()))),
            AgentType::Custom => {
                // 自定义 Provider 通过插件机制加载
                Err(crate::error::CisError::configuration(
                    "Custom agent provider not implemented yet"
                ))
            }
        }
    }

    /// 创建默认 Provider
    pub async fn default_provider() -> Result<Box<dyn AgentProvider>> {
        // 尝试按优先级创建：Claude → OpenCode → Kimi → Aider
        let claude = providers::ClaudeProvider::default();
        if claude.available().await {
            return Ok(Box::new(claude));
        }

        let opencode = providers::OpenCodeProvider::default();
        if opencode.available().await {
            return Ok(Box::new(opencode));
        }

        let kimi = providers::KimiProvider::default();
        if kimi.available().await {
            return Ok(Box::new(kimi));
        }

        let aider = providers::AiderProvider::default();
        if aider.available().await {
            return Ok(Box::new(aider));
        }

        Err(crate::error::CisError::configuration(
            "No AI agent available. Please install Claude Code, OpenCode, Kimi, or Aider."
        ))
    }
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider_type: AgentType,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub timeout_secs: Option<u64>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider_type: AgentType::Claude,
            model: None,
            api_key: None,
            base_url: None,
            timeout_secs: Some(300),
            max_tokens: Some(4096),
            temperature: Some(0.7),
        }
    }
}

/// Agent 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Kimi,
    Aider,
    OpenCode,
    Custom,
}

impl AgentType {
    /// 获取命令名称
    pub fn command_name(&self) -> Option<&'static str> {
        match self {
            AgentType::Claude => Some("claude"),
            AgentType::Kimi => Some("kimi"),
            AgentType::Aider => Some("aider"),
            AgentType::OpenCode => Some("opencode"),
            AgentType::Custom => None,
        }
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::Claude => "Claude Code",
            AgentType::Kimi => "Kimi Code",
            AgentType::Aider => "Aider",
            AgentType::OpenCode => "OpenCode",
            AgentType::Custom => "Custom",
        }
    }

    /// 是否支持 PTY 交互
    pub fn supports_pty(&self) -> bool {
        match self {
            AgentType::Claude | AgentType::Kimi | AgentType::Aider | AgentType::OpenCode => true,
            AgentType::Custom => false,
        }
    }

    /// 从字符串解析（用于配置文件）
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude" => Some(AgentType::Claude),
            "kimi" => Some(AgentType::Kimi),
            "aider" => Some(AgentType::Aider),
            "opencode" => Some(AgentType::OpenCode),
            _ => None,
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for AgentType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(AgentType::Claude),
            "kimi" => Ok(AgentType::Kimi),
            "aider" => Ok(AgentType::Aider),
            "opencode" => Ok(AgentType::OpenCode),
            "custom" => Ok(AgentType::Custom),
            _ => Err(format!("Invalid agent type: {}", s)),
        }
    }
}

/// Agent 管理器
pub struct AgentManager {
    providers: std::sync::Mutex<HashMap<String, Box<dyn AgentProvider>>>,
    default: std::sync::Mutex<String>,
}

impl AgentManager {
    pub fn new() -> Self {
        Self {
            providers: std::sync::Mutex::new(HashMap::new()),
            default: std::sync::Mutex::new("claude".to_string()),
        }
    }

    /// 注册 Provider
    pub fn register(&self, name: impl Into<String>, provider: Box<dyn AgentProvider>) {
        if let Ok(mut providers) = self.providers.lock() {
            providers.insert(name.into(), provider);
        }
    }

    /// 获取 Provider
    pub fn get(&self, _name: &str) -> Option<Box<dyn AgentProvider>> {
        // 由于 trait object 不能 Clone，这里返回 None
        // 实际使用时应该通过其他方式获取引用
        None
    }

    /// 获取默认 Provider 名称
    pub fn default_name(&self) -> String {
        self.default.lock().map(|d| d.clone()).unwrap_or_else(|_| "claude".to_string())
    }

    /// 设置默认 Provider
    pub fn set_default(&self, name: impl Into<String>) {
        if let Ok(mut default) = self.default.lock() {
            *default = name.into();
        }
    }

    /// 列出所有 Providers
    pub fn list(&self) -> Vec<String> {
        self.providers.lock()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}
