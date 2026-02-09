//! OpenCode Agent Provider
//!
//! 实现 AgentProvider trait，支持 OpenCode CLI

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::debug;

use crate::agent::{AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider};
use crate::error::Result;

/// OpenCode Agent Provider
pub struct OpenCodeProvider {
    config: AgentConfig,
}

impl OpenCodeProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(AgentConfig::default())
    }

    /// 构建 opencode 命令
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("opencode");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // OpenCode 使用 "run" 子命令
        cmd.arg("run");

        // 设置模型
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        // 设置输出格式为 JSON
        cmd.arg("--format").arg("json");

        // 设置系统提示词（通过环境变量）
        if let Some(ref system) = req.system_prompt {
            cmd.env("OPENCODE_SYSTEM_PROMPT", system);
        }

        cmd
    }

    /// 解析 JSON 格式输出
    fn parse_json_output(&self, stdout: &[u8]) -> Result<String> {
        let output = String::from_utf8_lossy(stdout);

        // OpenCode JSON 输出是事件流，每行一个 JSON 对象
        let mut content_parts = Vec::new();

        for line in output.lines() {
            if line.is_empty() {
                continue;
            }

            // 尝试解析 JSON 事件
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                // 提取 content 字段
                if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                    content_parts.push(content.to_string());
                }
                // 也可能是 text 字段
                else if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                    content_parts.push(text.to_string());
                }
                // 也可能是 message 字段
                else if let Some(message) = event.get("message").and_then(|m| m.as_str()) {
                    content_parts.push(message.to_string());
                }
            } else {
                // 如果不是 JSON，直接添加
                content_parts.push(line.to_string());
            }
        }

        // 合并所有内容
        let content = content_parts.join("\n");

        // 如果没有提取到任何内容，返回原始输出
        if content.is_empty() {
            Ok(output.to_string())
        } else {
            Ok(content)
        }
    }
}

#[async_trait]
impl AgentProvider for OpenCodeProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn available(&self) -> bool {
        Command::new("opencode")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);

        // 添加 prompt - 直接传递，不使用 -- 分隔符
        cmd.arg(&req.prompt);

        debug!("Executing OpenCode command: opencode run --model {:?} --format json <prompt>",
               self.config.model);

        let output = cmd.output().await?;

        // 解析 JSON 输出
        let content = self.parse_json_output(&output.stdout)?;

        debug!("OpenCode response length: {} bytes", content.len());

        Ok(AgentResponse {
            content,
            token_usage: None, // OpenCode CLI 暂不提供 token 统计
            metadata: [
                ("exit_code".to_string(), serde_json::json!(output.status.code())),
                ("agent_type".to_string(), serde_json::json!("opencode")),
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
        let mut cmd = self.build_command(&req);

        // 添加 prompt - 直接传递，不使用 -- 分隔符
        cmd.arg(&req.prompt);

        debug!("Executing OpenCode stream command");

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            // 解析 JSON 事件
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                // 提取并发送内容
                if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                    if tx.send(content.to_string()).await.is_err() {
                        break;
                    }
                }
                // 也可能是 text 字段
                else if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                    if tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
            } else {
                // 如果不是 JSON，直接发送原始行
                if tx.send(line).await.is_err() {
                    break;
                }
            }
        }

        let status = child.wait().await?;

        debug!("OpenCode stream completed with exit code: {:?}", status.code());

        Ok(AgentResponse {
            content: String::new(), // 流式模式下内容已通过 tx 发送
            token_usage: None,
            metadata: [
                ("exit_code".to_string(), serde_json::json!(status.code())),
                ("agent_type".to_string(), serde_json::json!("opencode")),
            ]
            .into_iter()
            .collect(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streaming: true,
            tool_calling: false,
            multimodal: true,
            max_context_length: Some(200_000),
            supported_models: vec![
                "opencode/big-pickle".to_string(),
                "opencode/glm-4.7-free".to_string(),
                "opencode/gpt-5-nano".to_string(),
                "opencode/kimi-k2.5-free".to_string(),
                "opencode/minimax-m2.1-free".to_string(),
                "opencode/trinity-large-preview-free".to_string(),
                "anthropic/claude-3-opus-20240229".to_string(),
                "anthropic/claude-3-sonnet-20240229".to_string(),
                "openai/gpt-4".to_string(),
                "openai/gpt-4-turbo".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_output() {
        let provider = OpenCodeProvider::default();

        // 测试 JSON 事件流
        let json_output = r#"{"type":"content","content":"Hello World"}
{"type":"content","content":"How are you?"}
"#;

        let result = provider.parse_json_output(json_output.as_bytes());
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Hello World"));
        assert!(content.contains("How are you?"));
    }

    #[test]
    fn test_parse_mixed_output() {
        let provider = OpenCodeProvider::default();

        // 测试混合输出（JSON + 纯文本）
        let mixed_output = r#"Some text
{"type":"content","content":"JSON content"}
More text
"#;

        let result = provider.parse_json_output(mixed_output.as_bytes());
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Some text"));
        assert!(content.contains("JSON content"));
    }
}
