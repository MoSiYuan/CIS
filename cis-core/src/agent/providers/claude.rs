//! Claude Code Provider

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::error::Result;

/// Claude Code Provider
pub struct ClaudeProvider {
    config: AgentConfig,
}

impl ClaudeProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    pub fn default() -> Self {
        Self::new(AgentConfig::default())
    }

    /// 构建 claude 命令
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("claude");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // 基础参数
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(max_tokens) = self.config.max_tokens {
            cmd.arg("--max-tokens").arg(max_tokens.to_string());
        }

        // 添加系统提示词
        if let Some(ref system) = req.system_prompt {
            cmd.arg("--system").arg(system);
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
        cmd.arg("--").arg(&req.prompt);

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

        // 启用流式输出
        cmd.arg("--stream");
        cmd.arg("--").arg(&req.prompt);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if tx.send(line).await.is_err() {
                break;
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
