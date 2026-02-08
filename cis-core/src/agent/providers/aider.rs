//! Aider Provider

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::error::Result;

/// Aider Provider
/// 
/// Aider 是专门为编程设计的 AI 助手，支持多模型
pub struct AiderProvider {
    config: AgentConfig,
}

impl AiderProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(AgentConfig::default())
    }
}

#[async_trait]
impl AgentProvider for AiderProvider {
    fn name(&self) -> &str {
        "aider"
    }

    async fn available(&self) -> bool {
        Command::new("aider")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let mut cmd = Command::new("aider");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // Aider 参数
        cmd.arg("--no-pretty").arg("--yes-always");

        // 模型选择
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        // 非交互模式执行指令
        cmd.arg("--message").arg(&req.prompt);

        let output = cmd.output().await?;

        let content = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(AgentResponse {
            content,
            token_usage: None,
            metadata: [
                ("exit_code".to_string(), serde_json::json!(output.status.code())),
                ("model".to_string(), serde_json::json!(self.config.model)),
            ]
            .into_iter()
            .collect(),
        })
    }

    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse> {
        // Aider 流式输出通过 --verbose 或重定向实现
        let mut cmd = Command::new("aider");

        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        cmd.arg("--no-pretty")
            .arg("--yes-always")
            .arg("--verbose")
            .arg("--message")
            .arg(&req.prompt);

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
            content: String::new(),
            token_usage: None,
            metadata: [("exit_code".to_string(), serde_json::json!(status.code()))]
                .into_iter()
                .collect(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streaming: true,
            tool_calling: true, // Aider 支持代码编辑工具
            multimodal: false,
            max_context_length: Some(128_000),
            supported_models: vec![
                "gpt-4".to_string(),
                "gpt-4-turbo".to_string(),
                "claude-3-5-sonnet".to_string(),
                "deepseek".to_string(),
            ],
        }
    }
}
