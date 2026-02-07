//! Agent 命令配置
//!
//! 定义不同 Agent 的命令行参数配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use portable_pty::CommandBuilder;

use crate::agent::AgentType;
use crate::error::{CisError, Result};

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
}

impl AgentCommandConfig {
    /// 创建 Claude 配置
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
        }
    }

    /// 创建 Kimi 配置
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
        }
    }

    /// 创建 Aider 配置
    pub fn aider() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "aider".to_string());

        Self {
            command: "aider".to_string(),
            base_args: vec![],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
        }
    }

    /// 创建 OpenCode 配置
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
        }
    }

    /// 从 AgentType 创建配置
    pub fn from_agent_type(agent_type: AgentType) -> Option<Self> {
        match agent_type {
            AgentType::Claude => Some(Self::claude()),
            AgentType::Kimi => Some(Self::kimi()),
            AgentType::Aider => Some(Self::aider()),
            AgentType::OpenCode => Some(Self::opencode()),
            AgentType::Custom => None,
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
    fn test_agent_command_config_claude() {
        let config = AgentCommandConfig::claude();
        assert_eq!(config.command, "claude");
        assert!(config.base_args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(config.requires_pty);
    }

    #[test]
    fn test_agent_command_config_opencode() {
        let config = AgentCommandConfig::opencode();
        assert_eq!(config.command, "opencode");
        assert!(config.base_args.contains(&"run".to_string()));
        assert!(config.base_args.contains(&"--format".to_string()));
        assert!(config.base_args.contains(&"json".to_string()));
    }

    #[test]
    fn test_from_agent_type() {
        let claude_config = AgentCommandConfig::from_agent_type(AgentType::Claude);
        assert!(claude_config.is_some());
        assert_eq!(claude_config.unwrap().command, "claude");

        let opencode_config = AgentCommandConfig::from_agent_type(AgentType::OpenCode);
        assert!(opencode_config.is_some());
        assert_eq!(opencode_config.unwrap().command, "opencode");

        let custom_config = AgentCommandConfig::from_agent_type(AgentType::Custom);
        assert!(custom_config.is_none());
    }
}
