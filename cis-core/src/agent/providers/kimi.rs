//! Kimi Code Provider

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::agent::security::CommandWhitelist;
use crate::error::{CisError, Result};

/// Kimi Code Provider
pub struct KimiProvider {
    #[allow(dead_code)]
    config: AgentConfig,
    /// 命令白名单验证器
    whitelist: CommandWhitelist,
}

impl KimiProvider {
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
