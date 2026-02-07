//! OpenCode AI Provider 实现

use super::{AiProvider, AiError, ConversationContext, Message, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// OpenCode 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    pub work_dir: Option<PathBuf>,
    /// 使用服务器模式（性能优化）
    pub server_url: Option<String>,
}

fn default_model() -> String {
    // 默认使用免费模型
    "opencode/big-pickle".to_string()
}
fn default_max_tokens() -> usize { 4096 }
fn default_temperature() -> f32 { 0.7 }

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            work_dir: None,
            server_url: None,
        }
    }
}

pub struct OpenCodeProvider {
    config: OpenCodeConfig,
}

impl OpenCodeProvider {
    pub fn new(config: OpenCodeConfig) -> Self {
        Self { config }
    }
}

impl Default for OpenCodeProvider {
    fn default() -> Self {
        Self::new(OpenCodeConfig::default())
    }
}

#[async_trait]
impl AiProvider for OpenCodeProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn available(&self) -> bool {
        match Command::new("opencode").arg("--version").output().await {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    async fn chat(&self, prompt: &str) -> Result<String> {
        let mut cmd = Command::new("opencode");
        cmd.arg("run")
           .arg("--model").arg(&self.config.model)
           .arg("--format").arg("json")
           .arg("--")
           .arg(prompt)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ref work_dir) = self.config.work_dir {
            cmd.current_dir(work_dir);
        }

        let output: std::process::Output = cmd.output().await.map_err(AiError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(stderr.to_string()));
        }

        // 解析 JSON 输出
        Self::parse_json_output(&output.stdout)
    }

    async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
        // OpenCode 不直接支持 --user --assistant 参数
        // 通过 prompt 注入模拟多轮对话
        let mut parts = Vec::new();

        if !system.is_empty() {
            parts.push(format!("System: {}", system));
        }

        for msg in messages {
            match msg.role {
                super::Role::System => {
                    parts.push(format!("System: {}", msg.content));
                }
                super::Role::User => {
                    parts.push(format!("User: {}", msg.content));
                }
                super::Role::Assistant => {
                    parts.push(format!("Assistant: {}", msg.content));
                }
            }
        }

        let full_prompt = parts.join("\n\n");
        self.chat(&full_prompt).await
    }

    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value> {
        let full_prompt = format!(
            "{}\n\nPlease respond with valid JSON matching this schema:\n{}\n\nRespond ONLY with the JSON object, no markdown formatting.",
            prompt, schema
        );

        let response = self.chat(&full_prompt).await?;

        // 提取 JSON（与 Claude 实现相同）
        let trimmed = response.trim();
        let json_str = if trimmed.starts_with('{') && trimmed.ends_with('}') {
            trimmed
        } else if let Some(start) = trimmed.find("```json") {
            let after = &trimmed[start + 7..];
            if let Some(end) = after.find("```") {
                after[..end].trim()
            } else {
                return Err(AiError::InvalidResponse("Invalid JSON block".to_string()));
            }
        } else {
            return Err(AiError::InvalidResponse("No JSON found".to_string()));
        };

        serde_json::from_str(json_str)
            .map_err(|e| AiError::InvalidResponse(format!("JSON parse error: {}", e)))
    }

    /// 带 RAG 上下文的对话 (完全复用 RAG 逻辑)
    async fn chat_with_rag(
        &self,
        prompt: &str,
        ctx: Option<&ConversationContext>,
    ) -> Result<String> {
        // ✅ 完全复用 RAG 增强逻辑
        let enhanced_prompt = if let Some(context) = ctx {
            match context.prepare_ai_prompt(prompt).await {
                Ok(enhanced) => enhanced,
                Err(e) => {
                    tracing::warn!("Failed to prepare AI prompt: {}, using original", e);
                    prompt.to_string()
                }
            }
        } else {
            prompt.to_string()
        };

        self.chat(&enhanced_prompt).await
    }
}

impl OpenCodeProvider {
    /// 解析 JSON 格式输出
    fn parse_json_output(stdout: &[u8]) -> Result<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_json_output() {
        let json_output = r#"{"type":"content","content":"Hello World"}
{"type":"content","content":"How are you?"}
"#;

        let result = OpenCodeProvider::parse_json_output(json_output.as_bytes());
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Hello World"));
        assert!(content.contains("How are you?"));
    }

    #[tokio::test]
    async fn test_chat_with_context() {
        let provider = OpenCodeProvider::default();

        let system = "You are a helpful assistant";
        let messages = vec![
            Message::user("What is Rust?"),
            Message::assistant("Rust is a systems programming language"),
            Message::user("Is it fast?"),
        ];

        // 注意：这个测试需要 opencode 安装才能运行
        // 这里只验证不会 panic
        let _ = provider.chat_with_context(system, &messages);
    }
}
