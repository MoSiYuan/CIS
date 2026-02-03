//! Kimi Code AI Provider 实现

use super::{AiProvider, AiError, ConversationContext, Message, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Kimi Code 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KimiConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
}

fn default_model() -> String { "kimi-k2".to_string() }
fn default_max_tokens() -> usize { 4096 }

impl Default for KimiConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            max_tokens: default_max_tokens(),
        }
    }
}

pub struct KimiCodeProvider {
    config: KimiConfig,
}

impl KimiCodeProvider {
    pub fn new(config: KimiConfig) -> Self { Self { config } }
}

impl Default for KimiCodeProvider {
    fn default() -> Self { Self::new(KimiConfig::default()) }
}

#[async_trait]
impl AiProvider for KimiCodeProvider {
    fn name(&self) -> &str { "kimi-code" }
    
    async fn available(&self) -> bool {
        match Command::new("kimi").arg("--version").output().await {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    async fn chat(&self, prompt: &str) -> Result<String> {
        let mut cmd = Command::new("kimi");
        cmd.arg("chat")
           .arg("--model").arg(&self.config.model)
           .arg("--max-tokens").arg(self.config.max_tokens.to_string())
           .arg("--no-stream")
           .arg("--")
           .arg(prompt)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let output: std::process::Output = cmd.output().await.map_err(AiError::Io)?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(stderr.to_string()));
        }
        
        Ok(String::from_utf8(output.stdout)?)
    }
    
    async fn chat_with_context(&self, _system: &str, messages: &[Message]) -> Result<String> {
        // Kimi CLI 可能不支持系统消息，构建为 user message
        let conversation: Vec<String> = messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect();
        
        let prompt = conversation.join("\n\n");
        self.chat(&prompt).await
    }
    
    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value> {
        let full_prompt = format!(
            "{}\n\nPlease respond with valid JSON matching this schema:\n{}\n\nRespond ONLY with the JSON object.",
            prompt, schema
        );
        
        let response = self.chat(&full_prompt).await?;
        
        let trimmed = response.trim();
        let json_str = if trimmed.starts_with('{') && trimmed.ends_with('}') {
            trimmed
        } else if let Some(start) = trimmed.find("```") {
            let after = &trimmed[start + 3..];
            if let Some(end) = after.find("```") {
                let content = after[..end].trim();
                if content.starts_with("json") {
                    content[4..].trim()
                } else {
                    content
                }
            } else {
                return Err(AiError::InvalidResponse("Invalid JSON block".to_string()));
            }
        } else {
            return Err(AiError::InvalidResponse("No JSON found".to_string()));
        };
        
        serde_json::from_str(json_str)
            .map_err(|e| AiError::InvalidResponse(format!("JSON parse error: {}", e)))
    }

    /// 带 RAG 上下文的对话 (CVI-011)
    async fn chat_with_rag(
        &self,
        prompt: &str,
        ctx: Option<&ConversationContext>,
    ) -> Result<String> {
        // 如果提供了上下文，构建增强 Prompt
        let enhanced_prompt = if let Some(context) = ctx {
            match context.prepare_ai_prompt(prompt).await {
                Ok(enhanced) => enhanced,
                Err(e) => {
                    // 如果构建增强 Prompt 失败，回退到原始 Prompt
                    tracing::warn!("Failed to prepare AI prompt: {}, using original", e);
                    prompt.to_string()
                }
            }
        } else {
            prompt.to_string()
        };

        // 使用简单的 chat 方法发送增强后的 Prompt
        self.chat(&enhanced_prompt).await
    }
}
