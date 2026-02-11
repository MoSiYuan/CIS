//! Claude Code Provider

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::agent::security::CommandWhitelist;
use crate::error::{CisError, Result};

/// Claude Code Provider
pub struct ClaudeProvider {
    config: AgentConfig,
    /// 命令白名单验证器
    whitelist: CommandWhitelist,
}

impl ClaudeProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            whitelist: CommandWhitelist::default(),
        }
    }

    /// 使用自定义白名单创建 Provider
    pub fn with_whitelist(config: AgentConfig, whitelist: CommandWhitelist) -> Self {
        Self { config, whitelist }
    }

    /// 从配置文件加载白名单
    pub fn with_whitelist_file(config: AgentConfig, path: &str) -> Result<Self> {
        let whitelist = CommandWhitelist::from_file(path)?;
        Ok(Self { config, whitelist })
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(AgentConfig::default())
    }

    /// 验证命令是否允许执行
    fn validate_command(&self, command: &str, args: &[&str]) -> Result<()> {
        match self.whitelist.validate_with_explanation(command, args) {
            Ok(result) => {
                if result.requires_confirmation {
                    // 危险命令，记录警告日志
                    tracing::warn!(
                        "Dangerous command requires confirmation: {} {:?}",
                        command, args
                    );
                }
                Ok(())
            }
            Err(e) => {
                // 向用户解释拒绝原因
                tracing::error!("Command rejected by whitelist: {}", e);
                Err(CisError::execution(format!(
                    "Security: {}. This command violates the security policy. If you need to execute this command, please contact your administrator to update the command whitelist configuration.",
                    e
                )))
            }
        }
    }

    /// 构建 claude 命令
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("claude");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // 非交互模式：直接输出结果并退出
        cmd.arg("--print");

        // 模型选择
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        // 注意：Claude CLI 不支持 --max-tokens 参数
        // 该参数在 API 中有效，但 CLI 版本不支持

        // 添加系统提示词
        if let Some(ref system) = req.system_prompt {
            cmd.arg("--system-prompt").arg(system);
        }

        cmd
    }
}

#[async_trait]
impl AgentProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    async fn available(&self) -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);

        // 添加 prompt
        cmd.arg(&req.prompt);

        let output = cmd.output().await?;

        let content = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(AgentResponse {
            content,
            token_usage: None, // Claude CLI 不提供 token 统计
            metadata: [("exit_code".to_string(), serde_json::json!(output.status.code()))]
                .into_iter()
                .collect(),
        })
    }

    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);

        // 流式输出格式
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg(&req.prompt);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            // 尝试解析 JSON 流输出
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                // 提取内容字段
                if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                    if tx.send(content.to_string()).await.is_err() {
                        break;
                    }
                } else if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
                    if tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
            } else {
                // 非 JSON 行，直接发送
                if tx.send(line).await.is_err() {
                    break;
                }
            }
        }

        let status = child.wait().await?;

        Ok(AgentResponse {
            content: String::new(), // 流式模式下内容已发送
            token_usage: None,
            metadata: [("exit_code".to_string(), serde_json::json!(status.code()))]
                .into_iter()
                .collect(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streaming: true,
            tool_calling: false, // Claude CLI 暂不支持
            multimodal: true,
            max_context_length: Some(200_000),
            supported_models: vec![
                "claude-sonnet-4-20250514".to_string(),
                "claude-opus-4-20250514".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_provider_creation() {
        let config = AgentConfig::default();
        let provider = ClaudeProvider::new(config);
        assert_eq!(provider.name(), "claude");
    }

    #[test]
    fn test_claude_capabilities() {
        let config = AgentConfig::default();
        let provider = ClaudeProvider::new(config);
        let caps = provider.capabilities();
        assert!(caps.streaming);
        assert!(caps.max_context_length.is_some());
    }
}
