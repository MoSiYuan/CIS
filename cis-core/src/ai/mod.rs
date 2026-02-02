//! AI Provider 模块
//!
//! 提供统一的 AI 调用接口，支持 Claude CLI（默认）和 Kimi Code
//! 同时提供 RAG (Retrieval Augmented Generation) 增强功能

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

mod claude;
mod kimi;

pub mod embedding;

pub use claude::{ClaudeCliProvider, ClaudeConfig};
pub use embedding::{
    create_embedding_service, cosine_similarity, filter_by_similarity,
    EmbeddingConfig, EmbeddingProvider, EmbeddingService as EmbeddingServiceTrait, LocalEmbeddingService,
    OpenAIEmbeddingService, DEFAULT_EMBEDDING_DIM, MIN_SIMILARITY_THRESHOLD,
};
pub use kimi::{KimiCodeProvider, KimiConfig};

/// AI Provider 错误
#[derive(Error, Debug)]
pub enum AiError {
    #[error("Provider not available: {0}")]
    NotAvailable(String),
    
    #[error("CLI execution failed: {0}")]
    CliError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub type Result<T> = std::result::Result<T, AiError>;

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// AI Provider 统一接口
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;
    
    /// 检查是否可用（CLI 工具是否安装）
    async fn available(&self) -> bool;
    
    /// 简单对话
    async fn chat(&self, prompt: &str) -> Result<String>;
    
    /// 带上下文的对话
    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[Message],
    ) -> Result<String>;
    
    /// 生成结构化数据（JSON）
    async fn generate_json(
        &self,
        prompt: &str,
        schema: &str,
    ) -> Result<serde_json::Value>;
}

/// AI Provider 工厂
pub struct AiProviderFactory;

impl AiProviderFactory {
    /// 创建默认 Provider（Claude CLI）
    pub fn default_provider() -> Box<dyn AiProvider> {
        Box::new(ClaudeCliProvider::default())
    }
    
    /// 根据配置创建 Provider
    pub fn from_config(config: AiProviderConfig) -> Box<dyn AiProvider> {
        match config.provider_type {
            ProviderType::Claude => {
                Box::new(ClaudeCliProvider::new(config.claude.unwrap_or_default()))
            }
            ProviderType::Kimi => {
                Box::new(KimiCodeProvider::new(config.kimi.unwrap_or_default()))
            }
        }
    }
}

/// Provider 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Claude,
    Kimi,
}

impl Default for ProviderType {
    fn default() -> Self {
        Self::Claude
    }
}

/// AI Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderConfig {
    #[serde(default)]
    pub provider_type: ProviderType,
    
    pub claude: Option<ClaudeConfig>,
    pub kimi: Option<KimiConfig>,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            provider_type: ProviderType::Claude,
            claude: Some(ClaudeConfig::default()),
            kimi: None,
        }
    }
}

// ============================================
// RAG (Retrieval Augmented Generation) Support
// ============================================

use crate::conversation::context::{ConversationContext, MessageRole};
use crate::vector::VectorStorage;

/// Completion Request
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// Prompt text
    pub prompt: String,
    /// System message
    pub system: Option<String>,
    /// Temperature
    pub temperature: Option<f32>,
    /// Max tokens
    pub max_tokens: Option<u32>,
}

impl Default for CompletionRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            system: None,
            temperature: None,
            max_tokens: None,
        }
    }
}

/// Completion Response
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// Generated text
    pub text: String,
    /// Token usage (if available)
    pub usage: Option<TokenUsage>,
    /// Model used
    pub model: Option<String>,
}

/// Token Usage
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// RAG增强的AI Provider
pub struct RagProvider<P: AiProvider> {
    inner: P,
    vector_storage: Arc<VectorStorage>,
    memory_top_k: usize,
}

impl<P: AiProvider> RagProvider<P> {
    /// 创建新的 RAG Provider
    pub fn new(inner: P, vector_storage: Arc<VectorStorage>) -> Self {
        Self {
            inner,
            vector_storage,
            memory_top_k: 5,
        }
    }
    
    /// 设置记忆检索数量
    pub fn with_memory_top_k(mut self, k: usize) -> Self {
        self.memory_top_k = k;
        self
    }
    
    /// 构建RAG增强的提示
    pub async fn build_rag_prompt(
        &self,
        user_input: &str,
        conversation: Option<&ConversationContext>,
    ) -> crate::error::Result<String> {
        let mut context_parts = Vec::new();
        
        // 1. 检索相关记忆
        let memories = self.vector_storage.search_memory(
            user_input, 
            self.memory_top_k, 
            Some(0.7)
        ).await.map_err(|e| crate::error::CisError::vector(format!("Memory search failed: {}", e)))?;
        
        if !memories.is_empty() {
            context_parts.push("## 相关记忆\n".to_string());
            for m in memories {
                // Get the actual memory value
                let content = format!("[{}]", m.key);
                context_parts.push(format!("- [{}] {}", m.category.unwrap_or_default(), content));
            }
            context_parts.push("".to_string());
        }
        
        // 2. 检索相关技能
        let skills = self.vector_storage.search_skills(
            user_input, 
            None, 
            3, 
            Some(0.6)
        ).await.map_err(|e| crate::error::CisError::vector(format!("Skill search failed: {}", e)))?;
        
        if !skills.is_empty() {
            context_parts.push("## 可用技能\n".to_string());
            for s in skills {
                context_parts.push(format!("- {}: {}", s.skill_name, s.skill_id));
            }
            context_parts.push("".to_string());
        }
        
        // 3. 对话历史（如果可用）
        if let Some(conv) = conversation {
            let relevant = conv.retrieve_relevant_history(user_input, 3).await
                .map_err(|e| crate::error::CisError::conversation(format!("History retrieval failed: {}", e)))?;
            
            if !relevant.is_empty() {
                context_parts.push("## 相关对话历史\n".to_string());
                for msg in relevant {
                    let role = match msg.role {
                        MessageRole::User => "用户",
                        MessageRole::Assistant => "助手",
                        MessageRole::System => "系统",
                        MessageRole::Tool => "工具",
                    };
                    context_parts.push(format!("{}: {}", role, msg.content));
                }
                context_parts.push("".to_string());
            }
        }
        
        // 4. 组合提示
        let prompt = if context_parts.is_empty() {
            user_input.to_string()
        } else {
            format!(
                "{context}\n\n## 用户输入\n{input}",
                context = context_parts.join("\n"),
                input = user_input
            )
        };
        
        Ok(prompt)
    }
    
    /// RAG增强的完成请求
    pub async fn complete_with_rag(
        &self,
        request: CompletionRequest,
        conversation: Option<&ConversationContext>,
    ) -> crate::error::Result<CompletionResponse> {
        let enhanced_prompt = self.build_rag_prompt(&request.prompt, conversation).await?;
        
        // Use the inner provider's chat method
        let system = request.system.as_deref().unwrap_or("You are a helpful assistant.");
        let response_text = self.inner.chat_with_context(
            system,
            &[Message::user(enhanced_prompt)]
        ).await.map_err(|e| crate::error::CisError::ai(format!("AI request failed: {}", e)))?;
        
        Ok(CompletionResponse {
            text: response_text,
            usage: None,
            model: Some(self.inner.name().to_string()),
        })
    }
}

// 为RagProvider实现AiProvider trait - 注意：这个实现直接代理到inner provider
// 实际的RAG功能通过 complete_with_rag 方法提供
#[async_trait]
impl<P: AiProvider> AiProvider for RagProvider<P> {
    async fn chat(&self, prompt: &str) -> Result<String> {
        self.inner.chat(prompt).await
    }
    
    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[Message],
    ) -> Result<String> {
        self.inner.chat_with_context(system, messages).await
    }
    
    async fn generate_json(
        &self,
        prompt: &str,
        schema: &str,
    ) -> Result<serde_json::Value> {
        self.inner.generate_json(prompt, schema).await
    }
    
    fn name(&self) -> &str {
        self.inner.name()
    }
    
    async fn available(&self) -> bool {
        self.inner.available().await
    }
}

/// RAG Provider Builder
pub struct RagProviderBuilder {
    vector_storage: Option<Arc<VectorStorage>>,
    memory_top_k: usize,
}

impl RagProviderBuilder {
    /// 创建新的 Builder
    pub fn new() -> Self {
        Self {
            vector_storage: None,
            memory_top_k: 5,
        }
    }
    
    /// 设置向量存储
    pub fn with_vector_storage(mut self, storage: Arc<VectorStorage>) -> Self {
        self.vector_storage = Some(storage);
        self
    }
    
    /// 设置记忆检索数量
    pub fn with_memory_top_k(mut self, k: usize) -> Self {
        self.memory_top_k = k;
        self
    }
    
    /// 构建 RAG Provider
    pub fn build<P: AiProvider>(self, inner: P) -> Result<RagProvider<P>> {
        let storage = self.vector_storage
            .ok_or_else(|| AiError::NotAvailable("Vector storage not provided".to_string()))?;
        
        Ok(RagProvider {
            inner,
            vector_storage: storage,
            memory_top_k: self.memory_top_k,
        })
    }
}

impl Default for RagProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert!(matches!(msg.role, Role::User));
        assert_eq!(msg.content, "Hello");
        
        let msg = Message::system("You are an AI");
        assert!(matches!(msg.role, Role::System));
    }
    
    #[test]
    fn test_completion_request_default() {
        let req = CompletionRequest::default();
        assert!(req.prompt.is_empty());
        assert!(req.system.is_none());
    }
    
    #[test]
    fn test_rag_provider_builder() {
        // Just test the builder structure without actual storage
        let builder = RagProviderBuilder::new()
            .with_memory_top_k(10);
        
        assert_eq!(builder.memory_top_k, 10);
    }
}
