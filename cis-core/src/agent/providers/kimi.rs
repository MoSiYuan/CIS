//! Kimi Code Provider

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::error::Result;

/// Kimi Code Provider
pub struct KimiProvider {
    #[allow(dead_code)]
    config: AgentConfig,
}

impl KimiProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(AgentConfig::default())
    }
}

#[async_trait]
impl AgentProvider for KimiProvider {
    fn name(&self) -> &str {
        "kimi"
    }

    async fn available(&self) -> bool {
        Command::new("kimi")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let mut cmd = Command::new("kimi");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // Kimi CLI 参数 - 直接传递 prompt，不使用 -- 分隔符
        cmd.arg("chat").arg("--no-stream").arg(&req.prompt);

        let output = cmd.output().await?;

        let content = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(AgentResponse {
            content,
            token_usage: None,
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
        let mut cmd = Command::new("kimi");

        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // 流式模式 - 直接传递 prompt，不使用 -- 分隔符
        cmd.arg("chat").arg("--stream").arg(&req.prompt);

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
            tool_calling: false,
            multimodal: false,
            max_context_length: Some(128_000),
            supported_models: vec![
                "kimi-k2".to_string(),
            ],
        }
    }
}
