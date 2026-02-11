//! OpenCode AI Provider 实现

use super::{AiProvider, AiError, ConversationContext, Message, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::fs;

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
    /// Session 持久化目录
    pub session_dir: Option<PathBuf>,
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
            session_dir: None,
        }
    }
}

/// OpenCode Session - 支持真实的多轮对话
/// 
/// 使用 OpenCode CLI 的 session 功能实现持久化的多轮对话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeSession {
    session_id: String,
    history: VecDeque<Message>,
    #[serde(skip)]
    config: OpenCodeConfig,
}

impl OpenCodeSession {
    /// 创建新的 Session
    pub fn new(session_id: impl Into<String>, config: OpenCodeConfig) -> Self {
        Self {
            session_id: session_id.into(),
            history: VecDeque::new(),
            config,
        }
    }

    /// 从持久化存储加载 Session
    pub async fn load(
        session_id: impl Into<String>,
        config: OpenCodeConfig,
    ) -> Result<Self> {
        let session_id = session_id.into();
        
        if let Some(ref session_dir) = config.session_dir {
            let session_file = session_dir.join(format!("{}.json", session_id));
            
            if session_file.exists() {
                let content = fs::read_to_string(&session_file).await
                    .map_err(AiError::Io)?;
                let mut session: OpenCodeSession = serde_json::from_str(&content)
                    .map_err(|e| AiError::InvalidResponse(format!("Failed to parse session: {}", e)))?;
                session.config = config;
                return Ok(session);
            }
        }
        
        Ok(Self::new(session_id, config))
    }

    /// 发送消息并获取回复（使用 opencode continue）
    pub async fn chat(&mut self, message: &str) -> Result<String> {
        // 确保 session 已初始化（首次对话使用 opencode init）
        if self.history.is_empty() {
            self.init_session(message).await
        } else {
            self.continue_session(message).await
        }
    }

    /// 初始化新 session（首次对话）
    async fn init_session(&mut self, message: &str) -> Result<String> {
        let mut cmd = Command::new("opencode");
        cmd.arg("init")
           .arg("-c")
           .arg(&self.session_id)
           .arg("--model")
           .arg(&self.config.model)
           .arg("--")
           .arg(message)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ref work_dir) = self.config.work_dir {
            cmd.current_dir(work_dir);
        }

        let output = cmd.output().await.map_err(AiError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(format!(
                "OpenCode init failed: {}",
                stderr
            )));
        }

        let response = Self::parse_json_output(&output.stdout)?;
        
        // 更新历史记录
        self.history.push_back(Message::user(message));
        self.history.push_back(Message::assistant(&response));
        
        // 持久化 session
        self.persist().await?;
        
        Ok(response)
    }

    /// 继续已有 session
    async fn continue_session(&mut self, message: &str) -> Result<String> {
        let mut cmd = Command::new("opencode");
        cmd.arg("continue")
           .arg("-c")
           .arg(&self.session_id)
           .arg("--")
           .arg(message)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ref work_dir) = self.config.work_dir {
            cmd.current_dir(work_dir);
        }

        let output = cmd.output().await.map_err(AiError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(format!(
                "OpenCode continue failed: {}",
                stderr
            )));
        }

        let response = Self::parse_json_output(&output.stdout)?;
        
        // 更新历史记录
        self.history.push_back(Message::user(message));
        self.history.push_back(Message::assistant(&response));
        
        // 限制历史记录长度，防止过长
        while self.history.len() > 100 {
            self.history.pop_front();
        }
        
        // 持久化 session
        self.persist().await?;
        
        Ok(response)
    }

    /// 持久化 session 到磁盘
    async fn persist(&self) -> Result<()> {
        if let Some(ref session_dir) = self.config.session_dir {
            // 确保目录存在
            fs::create_dir_all(session_dir).await.map_err(AiError::Io)?;
            
            let session_file = session_dir.join(format!("{}.json", self.session_id));
            let content = serde_json::to_string_pretty(self)
                .map_err(|e| AiError::InvalidResponse(format!("Failed to serialize session: {}", e)))?;
            
            fs::write(&session_file, content).await.map_err(AiError::Io)?;
        }
        
        Ok(())
    }

    /// 获取 session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 获取历史记录
    pub fn history(&self) -> &VecDeque<Message> {
        &self.history
    }

    /// 清空历史记录
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 获取历史记录的消息数量
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

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

pub struct OpenCodeProvider {
    config: OpenCodeConfig,
}

impl OpenCodeProvider {
    pub fn new(config: OpenCodeConfig) -> Self {
        Self { config }
    }

    /// 创建新的 session
    pub fn create_session(&self, session_id: impl Into<String>) -> OpenCodeSession {
        OpenCodeSession::new(session_id, self.config.clone())
    }

    /// 加载已有 session
    pub async fn load_session(&self, session_id: impl Into<String>) -> Result<OpenCodeSession> {
        OpenCodeSession::load(session_id, self.config.clone()).await
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
        OpenCodeSession::parse_json_output(&output.stdout)
    }

    async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
        // 使用 session 实现真正的多轮对话
        let session_id = format!("cis-auto-{}", uuid::Uuid::new_v4());
        let mut session = OpenCodeSession::new(session_id, self.config.clone());

        // 构建包含系统提示和历史的初始消息
        let mut full_message = String::new();
        
        if !system.is_empty() {
            full_message.push_str(&format!("System: {}\n\n", system));
        }

        // 添加历史消息（除了最后一条用户消息）
        if messages.len() > 1 {
            for msg in &messages[..messages.len() - 1] {
                match msg.role {
                    super::Role::System => {
                        full_message.push_str(&format!("System: {}\n", msg.content));
                    }
                    super::Role::User => {
                        full_message.push_str(&format!("User: {}\n", msg.content));
                    }
                    super::Role::Assistant => {
                        full_message.push_str(&format!("Assistant: {}\n", msg.content));
                    }
                }
            }
        }

        // 最后一条消息作为当前输入
        if let Some(last_msg) = messages.last() {
            full_message.push_str(&last_msg.content);
        }

        session.chat(&full_message).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_session_creation() {
        let config = OpenCodeConfig::default();
        let session = OpenCodeSession::new("test-session", config);
        
        assert_eq!(session.session_id(), "test-session");
        assert_eq!(session.history_len(), 0);
    }

    #[test]
    fn test_opencode_session_history() {
        let config = OpenCodeConfig::default();
        let mut session = OpenCodeSession::new("test-session", config);
        
        session.history.push_back(Message::user("Hello"));
        session.history.push_back(Message::assistant("Hi there!"));
        
        assert_eq!(session.history_len(), 2);
        
        session.clear_history();
        assert_eq!(session.history_len(), 0);
    }

    #[tokio::test]
    async fn test_parse_json_output() {
        let json_output = r#"{"type":"content","content":"Hello World"}
{"type":"content","content":"How are you?"}
"#;

        let result = OpenCodeSession::parse_json_output(json_output.as_bytes());
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Hello World"));
        assert!(content.contains("How are you?"));
    }

    #[tokio::test]
    async fn test_provider_create_session() {
        let provider = OpenCodeProvider::default();
        let session = provider.create_session("my-session");
        
        assert_eq!(session.session_id(), "my-session");
    }
}
