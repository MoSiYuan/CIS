//! Claude CLI AI Provider 实现

use super::{AiProvider, AiError, Message, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Claude CLI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    pub work_dir: Option<PathBuf>,
}

fn default_model() -> String { "claude-sonnet-4-20250514".to_string() }
fn default_max_tokens() -> usize { 4096 }
fn default_temperature() -> f32 { 0.7 }

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            work_dir: None,
        }
    }
}

pub struct ClaudeCliProvider {
    config: ClaudeConfig,
}

impl ClaudeCliProvider {
    pub fn new(config: ClaudeConfig) -> Self { Self { config } }
}

impl Default for ClaudeCliProvider {
    fn default() -> Self { Self::new(ClaudeConfig::default()) }
}

#[async_trait]
impl AiProvider for ClaudeCliProvider {
    fn name(&self) -> &str { "claude-cli" }
    
    async fn available(&self) -> bool {
        match Command::new("claude").arg("--version").output().await {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    async fn chat(&self, prompt: &str) -> Result<String> {
        let mut cmd = Command::new("claude");
        cmd.arg("--model").arg(&self.config.model)
           .arg("--max-tokens").arg(self.config.max_tokens.to_string())
           .arg("--temperature").arg(self.config.temperature.to_string())
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
        
        Ok(String::from_utf8(output.stdout)?)
    }
    
    async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
        let mut cmd = Command::new("claude");
        cmd.arg("--model").arg(&self.config.model)
           .arg("--system").arg(system);
        
        for msg in messages {
            match msg.role {
                super::Role::User => { cmd.arg("--user").arg(&msg.content); }
                super::Role::Assistant => { cmd.arg("--assistant").arg(&msg.content); }
                _ => {}
            }
        }
        
        cmd.stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let output: std::process::Output = cmd.output().await.map_err(AiError::Io)?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(stderr.to_string()));
        }
        
        Ok(String::from_utf8(output.stdout)?)
    }
    
    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value> {
        let full_prompt = format!(
            "{}\n\nPlease respond with valid JSON matching this schema:\n{}\n\nRespond ONLY with the JSON object, no markdown formatting.",
            prompt, schema
        );
        
        let response = self.chat(&full_prompt).await?;
        
        // 简单提取 JSON
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
}
