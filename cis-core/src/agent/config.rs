//! Agent 命令配置
//!
//! 定义不同 Agent 的命令行参数配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use portable_pty::CommandBuilder;

use crate::agent::AgentType;
use crate::error::{CisError, Result};

/// Agent 运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentMode {
    /// 单次执行模式（默认）
    Single,
    /// 持久化模式（后台运行）
    Persistent,
    /// 服务模式（HTTP Server）
    Server,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::Single
    }
}

fn default_http_host() -> String {
    "127.0.0.1".to_string()
}

fn default_auto_serve() -> bool {
    true
}

/// Agent 命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandConfig {
    /// 命令名称
    pub command: String,
    /// 基础参数
    pub base_args: Vec<String>,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 是否需要 PTY
    pub requires_pty: bool,
    /// 是否支持流式输出
    pub supports_streaming: bool,

    // === 新增字段 ===
    /// 运行模式
    #[serde(default)]
    pub mode: AgentMode,
    /// HTTP 端口（Server 模式用）
    #[serde(default)]
    pub http_port: Option<u16>,
    /// HTTP 主机（Server 模式用）
    #[serde(default = "default_http_host")]
    pub http_host: String,
    /// 是否自动启动 serve
    #[serde(default = "default_auto_serve")]
    pub auto_serve: bool,
}

impl AgentCommandConfig {
    /// 创建 Claude 配置（单次模式）
    pub fn claude() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "claude".to_string());

        Self {
            command: "claude".to_string(),
            base_args: vec![
                "--dangerously-skip-permissions".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
            mode: AgentMode::Single,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    /// 创建 Kimi 配置（单次模式）
    pub fn kimi() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "kimi".to_string());

        Self {
            command: "kimi".to_string(),
            base_args: vec![
                "--dangerously-skip-permissions".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
            mode: AgentMode::Single,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    /// 创建 Aider 配置（单次模式）
    pub fn aider() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "aider".to_string());

        Self {
            command: "aider".to_string(),
            base_args: vec![],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
            mode: AgentMode::Single,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    /// 创建 OpenCode 配置（单次模式）
    pub fn opencode() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "opencode".to_string());

        Self {
            command: "opencode".to_string(),
            base_args: vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
            mode: AgentMode::Single,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    // === 新增：持久化模式配置 ===

    /// Claude 持久化模式（PTY）
    pub fn claude_persistent() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "claude".to_string());

        Self {
            command: "claude".to_string(),
            base_args: vec![
                "--dangerously-skip-permissions".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
            mode: AgentMode::Persistent,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    /// OpenCode 持久化模式（HTTP Server）
    pub fn opencode_persistent(port: Option<u16>) -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "opencode".to_string());
        env_vars.insert("OPENCODE_SERVER_MODE".to_string(), "true".to_string());

        Self {
            command: "opencode".to_string(),
            base_args: vec![
                "serve".to_string(),
            ],
            env_vars,
            requires_pty: false,  // Server 模式不需要 PTY
            supports_streaming: true,
            mode: AgentMode::Server,
            http_port: port,
            http_host: default_http_host(),
            auto_serve: true,
        }
    }

    /// OpenCode 单次模式（使用已有 server）
    pub fn opencode_single_with_server(url: &str) -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "opencode".to_string());
        env_vars.insert("OPENCODE_SERVER_URL".to_string(), url.to_string());

        Self {
            command: "opencode".to_string(),
            base_args: vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            env_vars,
            requires_pty: false,
            supports_streaming: true,
            mode: AgentMode::Single,
            http_port: None,
            http_host: default_http_host(),
            auto_serve: false,
        }
    }

    /// 从 AgentType 和 AgentMode 创建配置
    pub fn from_agent_type(agent_type: AgentType, mode: AgentMode) -> Option<Self> {
        match (agent_type, mode) {
            (AgentType::Claude, AgentMode::Single) => Some(Self::claude()),
            (AgentType::Claude, AgentMode::Persistent) => Some(Self::claude_persistent()),
            (AgentType::OpenCode, AgentMode::Single) => Some(Self::opencode()),
            (AgentType::OpenCode, AgentMode::Server) => Some(Self::opencode_persistent(None)),
            (AgentType::Kimi, AgentMode::Single) => Some(Self::kimi()),
            (AgentType::Aider, AgentMode::Single) => Some(Self::aider()),
            _ => None,
        }
    }

    /// 构建完整的 CommandBuilder
    pub fn build_command(&self, work_dir: &Path, session_id: &str) -> Result<CommandBuilder> {
        let mut cmd = CommandBuilder::new(&self.command);

        // 设置工作目录
        cmd.cwd(work_dir);

        // 设置环境变量
        cmd.env("CIS_PROJECT_PATH", work_dir.to_string_lossy().as_ref());
        cmd.env("CIS_SESSION_ID", session_id);

        // 添加配置中的环境变量
        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        // 添加基础参数
        for arg in &self.base_args {
            cmd.arg(arg);
        }

        Ok(cmd)
    }

    // === 辅助方法 ===

    /// 是否为持久化模式
    pub fn is_persistent(&self) -> bool {
        matches!(self.mode, AgentMode::Persistent | AgentMode::Server)
    }

    /// 是否为 Server 模式
    pub fn is_server(&self) -> bool {
        matches!(self.mode, AgentMode::Server)
    }

    /// 获取 HTTP URL（Server 模式）
    pub fn http_url(&self) -> Option<String> {
        if self.is_server() {
            let port = self.http_port?;
            Some(format!("http://{}:{}", self.http_host, port))
        } else {
            None
        }
    }

    /// 构建 serve 命令（OpenCode）
    pub fn build_serve_command(&self) -> Result<CommandBuilder> {
        if !self.is_server() {
            return Err(CisError::invalid_input("Not server mode"));
        }

        let mut cmd = CommandBuilder::new(&self.command);
        cmd.arg("serve");

        if let Some(port) = self.http_port {
            cmd.arg("--port");
            cmd.arg(port.to_string());
        }

        cmd.arg("--host");
        cmd.arg(&self.http_host);

        for (key, value) in &self.env_vars {
            cmd.env(key, value);
        }

        Ok(cmd)
    }
}

impl Default for AgentCommandConfig {
    fn default() -> Self {
        Self::claude()  // 默认使用 Claude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_mode_default() {
        let mode: AgentMode = Default::default();
        assert_eq!(mode, AgentMode::Single);
    }

    #[test]
    fn test_agent_command_config_claude() {
        let config = AgentCommandConfig::claude();
        assert_eq!(config.command, "claude");
        assert!(config.base_args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(config.requires_pty);
        assert_eq!(config.mode, AgentMode::Single);
        assert!(!config.is_persistent());
        assert!(!config.is_server());
        assert!(config.http_url().is_none());
    }

    #[test]
    fn test_agent_command_config_kimi() {
        let config = AgentCommandConfig::kimi();
        assert_eq!(config.command, "kimi");
        assert!(config.requires_pty);
        assert_eq!(config.mode, AgentMode::Single);
    }

    #[test]
    fn test_agent_command_config_aider() {
        let config = AgentCommandConfig::aider();
        assert_eq!(config.command, "aider");
        assert!(config.requires_pty);
        assert_eq!(config.mode, AgentMode::Single);
    }

    #[test]
    fn test_agent_command_config_opencode() {
        let config = AgentCommandConfig::opencode();
        assert_eq!(config.command, "opencode");
        assert!(config.base_args.contains(&"run".to_string()));
        assert!(config.base_args.contains(&"--format".to_string()));
        assert!(config.base_args.contains(&"json".to_string()));
        assert_eq!(config.mode, AgentMode::Single);
    }

    #[test]
    fn test_claude_persistent() {
        let config = AgentCommandConfig::claude_persistent();
        assert_eq!(config.command, "claude");
        assert_eq!(config.mode, AgentMode::Persistent);
        assert!(config.is_persistent());
        assert!(!config.is_server());
        assert!(config.requires_pty);
        assert!(config.http_url().is_none());
    }

    #[test]
    fn test_opencode_persistent() {
        let config = AgentCommandConfig::opencode_persistent(Some(8080));
        assert_eq!(config.command, "opencode");
        assert_eq!(config.mode, AgentMode::Server);
        assert!(config.is_persistent());
        assert!(config.is_server());
        assert!(!config.requires_pty);
        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.http_host, "127.0.0.1");
        assert!(config.auto_serve);
        assert_eq!(config.http_url(), Some("http://127.0.0.1:8080".to_string()));
        assert!(config.env_vars.contains_key("OPENCODE_SERVER_MODE"));
    }

    #[test]
    fn test_opencode_persistent_without_port() {
        let config = AgentCommandConfig::opencode_persistent(None);
        assert_eq!(config.mode, AgentMode::Server);
        assert!(config.http_port.is_none());
        assert!(config.http_url().is_none()); // 没有端口时返回 None
    }

    #[test]
    fn test_opencode_single_with_server() {
        let config = AgentCommandConfig::opencode_single_with_server("http://localhost:9000");
        assert_eq!(config.command, "opencode");
        assert_eq!(config.mode, AgentMode::Single);
        assert!(!config.is_persistent());
        assert!(!config.is_server());
        assert!(!config.requires_pty);
        assert_eq!(config.env_vars.get("OPENCODE_SERVER_URL"), Some(&"http://localhost:9000".to_string()));
    }

    #[test]
    fn test_from_agent_type() {
        // Claude Single
        let claude_single = AgentCommandConfig::from_agent_type(AgentType::Claude, AgentMode::Single);
        assert!(claude_single.is_some());
        let config = claude_single.unwrap();
        assert_eq!(config.command, "claude");
        assert_eq!(config.mode, AgentMode::Single);

        // Claude Persistent
        let claude_persistent = AgentCommandConfig::from_agent_type(AgentType::Claude, AgentMode::Persistent);
        assert!(claude_persistent.is_some());
        let config = claude_persistent.unwrap();
        assert_eq!(config.mode, AgentMode::Persistent);

        // OpenCode Single
        let opencode_single = AgentCommandConfig::from_agent_type(AgentType::OpenCode, AgentMode::Single);
        assert!(opencode_single.is_some());
        let config = opencode_single.unwrap();
        assert_eq!(config.command, "opencode");
        assert_eq!(config.mode, AgentMode::Single);

        // OpenCode Server
        let opencode_server = AgentCommandConfig::from_agent_type(AgentType::OpenCode, AgentMode::Server);
        assert!(opencode_server.is_some());
        let config = opencode_server.unwrap();
        assert_eq!(config.mode, AgentMode::Server);

        // Kimi Single
        let kimi_single = AgentCommandConfig::from_agent_type(AgentType::Kimi, AgentMode::Single);
        assert!(kimi_single.is_some());
        assert_eq!(kimi_single.unwrap().command, "kimi");

        // Aider Single
        let aider_single = AgentCommandConfig::from_agent_type(AgentType::Aider, AgentMode::Single);
        assert!(aider_single.is_some());
        assert_eq!(aider_single.unwrap().command, "aider");

        // Unsupported combinations
        let kimi_persistent = AgentCommandConfig::from_agent_type(AgentType::Kimi, AgentMode::Persistent);
        assert!(kimi_persistent.is_none());

        let aider_server = AgentCommandConfig::from_agent_type(AgentType::Aider, AgentMode::Server);
        assert!(aider_server.is_none());

        let custom_single = AgentCommandConfig::from_agent_type(AgentType::Custom, AgentMode::Single);
        assert!(custom_single.is_none());
    }

    #[test]
    fn test_build_serve_command_success() {
        let config = AgentCommandConfig::opencode_persistent(Some(8080));
        let result = config.build_serve_command();
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_serve_command_failure() {
        let config = AgentCommandConfig::claude();
        let result = config.build_serve_command();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Not server mode"));
    }

    #[test]
    fn test_serde_backward_compatibility() {
        // 测试向后兼容：旧的序列化数据没有新字段时应该能正常反序列化
        let json = r#"{
            "command": "claude",
            "base_args": ["--dangerously-skip-permissions"],
            "env_vars": {},
            "requires_pty": true,
            "supports_streaming": true
        }"#;
        let config: AgentCommandConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.command, "claude");
        assert_eq!(config.mode, AgentMode::Single); // 默认值
        assert_eq!(config.http_host, "127.0.0.1"); // 默认值
        assert!(config.auto_serve); // 默认值
    }

    #[test]
    fn test_serde_full_config() {
        // 测试完整配置序列化/反序列化
        let config = AgentCommandConfig::opencode_persistent(Some(9000));
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentCommandConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, config.command);
        assert_eq!(deserialized.mode, config.mode);
        assert_eq!(deserialized.http_port, config.http_port);
        assert_eq!(deserialized.http_host, config.http_host);
        assert_eq!(deserialized.auto_serve, config.auto_serve);
    }
}
