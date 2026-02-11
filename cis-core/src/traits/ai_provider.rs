//! # AI Provider Trait
//!
//! AI 服务的抽象接口，定义大语言模型交互的基本操作。
//!
//! ## 设计原则
//!
//! - **统一接口**: 无论底层是 Claude、GPT 还是其他模型，提供一致的调用方式
//! - **异步优先**: 所有 IO 操作都是异步的
//! - **错误处理**: 每个方法返回 Result，便于错误传播
//! - **类型安全**: 使用强类型参数和返回值
//!
//! ## 使用示例
//!
//! ### 文本补全
//!
//! ```rust,no_run
//! use cis_core::traits::{AiProvider, CompletionRequest};
//!
//! # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
//! // 简单补全
//! let request = CompletionRequest::new("What is Rust?");
//! let response = provider.complete(request).await?;
//! println!("{}", response.text);
//! # Ok(())
//! # }
//! ```
//!
//! ### 向量嵌入
//!
//! ```rust,no_run
//! use cis_core::traits::{AiProvider, EmbeddingRequest};
//!
//! # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
//! // 单个文本嵌入
//! let request = EmbeddingRequest::new(vec!["Hello world".to_string()]);
//! let response = provider.embedding(request).await?;
//! println!("Embedding dimension: {}", response.embeddings[0].len());
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use crate::error::{CisError, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Token 使用信息
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// 输入 token 数量
    pub prompt_tokens: u64,
    /// 输出 token 数量
    pub completion_tokens: u64,
    /// 总 token 数量
    pub total_tokens: u64,
}

/// 模型信息
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// 模型标识符
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型提供商
    pub provider: String,
    /// 最大上下文长度
    pub max_context_length: usize,
    /// 是否支持函数调用
    pub supports_function_calling: bool,
    /// 是否支持嵌入
    pub supports_embedding: bool,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
}

/// 补全请求
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    /// 提示文本
    pub prompt: String,
    /// 系统消息（可选）
    pub system: Option<String>,
    /// 对话历史
    pub messages: Vec<(String, String)>, // (role, content)
    /// 温度参数 (0.0 - 2.0)
    pub temperature: Option<f32>,
    /// 最大生成 token 数
    pub max_tokens: Option<u32>,
    /// 停止序列
    pub stop_sequences: Vec<String>,
    /// 是否流式输出
    pub stream: bool,
    /// 额外参数
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl CompletionRequest {
    /// 创建新的补全请求
    ///
    /// # Arguments
    /// * `prompt` - 提示文本
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::traits::CompletionRequest;
    ///
    /// let request = CompletionRequest::new("What is Rust?");
    /// ```
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            system: None,
            messages: Vec::new(),
            temperature: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
            stream: false,
            extra_params: HashMap::new(),
        }
    }

    /// 设置系统消息
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// 添加对话消息
    pub fn with_message(mut self, role: impl Into<String>, content: impl Into<String>) -> Self {
        self.messages.push((role.into(), content.into()));
        self
    }

    /// 设置温度参数
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature.clamp(0.0, 2.0));
        self
    }

    /// 设置最大 token 数
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 设置停止序列
    pub fn with_stop_sequences(mut self, stops: Vec<String>) -> Self {
        self.stop_sequences = stops;
        self
    }

    /// 设置是否流式输出
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// 添加额外参数
    pub fn with_extra_param(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.extra_params.insert(key.into(), value);
        self
    }
}

/// 补全响应
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// 生成的文本
    pub text: String,
    /// Token 使用信息
    pub usage: Option<TokenUsage>,
    /// 使用的模型
    pub model: String,
    /// 完成原因
    pub finish_reason: Option<String>,
    /// 响应 ID
    pub id: String,
    /// 创建时间戳
    pub created_at: u64,
}

/// 嵌入请求
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    /// 输入文本列表
    pub texts: Vec<String>,
    /// 模型标识（可选，使用默认模型）
    pub model: Option<String>,
    /// 编码格式
    pub encoding_format: String,
    /// 维度（可选，用于降维）
    pub dimensions: Option<usize>,
}

impl EmbeddingRequest {
    /// 创建新的嵌入请求
    ///
    /// # Arguments
    /// * `texts` - 要嵌入的文本列表
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::traits::EmbeddingRequest;
    ///
    /// let request = EmbeddingRequest::new(vec![
    ///     "Hello world".to_string(),
    ///     "Rust programming".to_string(),
    /// ]);
    /// ```
    pub fn new(texts: Vec<String>) -> Self {
        Self {
            texts,
            model: None,
            encoding_format: "float".to_string(),
            dimensions: None,
        }
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置维度
    pub fn with_dimensions(mut self, dimensions: usize) -> Self {
        self.dimensions = Some(dimensions);
        self
    }
}

/// 嵌入响应
#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    /// 嵌入向量列表（与输入一一对应）
    pub embeddings: Vec<Vec<f32>>,
    /// Token 使用信息
    pub usage: Option<TokenUsage>,
    /// 使用的模型
    pub model: String,
    /// 嵌入维度
    pub dimension: usize,
    /// 响应 ID
    pub id: String,
}

/// AI Provider 抽象接口
///
/// 提供统一的大语言模型调用接口，支持文本补全和向量嵌入。
///
/// ## 实现要求
///
/// - 所有方法必须是线程安全的 (Send + Sync)
/// - 所有异步方法必须返回 Result 类型
/// - 实现应该处理超时和重试逻辑
///
/// ## 使用示例
///
/// ```rust,no_run
/// use cis_core::traits::{AiProvider, CompletionRequest, EmbeddingRequest};
///
/// # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
/// // 文本补全
/// let completion = provider.complete(
///     CompletionRequest::new("Explain async/await in Rust")
///         .with_temperature(0.7)
/// ).await?;
///
/// println!("Response: {}", completion.text);
///
/// // 获取嵌入
/// let embedding = provider.embedding(
///     EmbeddingRequest::new(vec!["Rust programming".to_string()])
/// ).await?;
///
/// println!("Embedding dimension: {}", embedding.dimension);
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 执行文本补全
    ///
    /// # Arguments
    /// * `request` - 补全请求参数
    ///
    /// # Returns
    /// * `Ok(CompletionResponse)` - 补全成功
    /// * `Err(CisError::Ai(_))` - AI 服务错误
    /// * `Err(CisError::InvalidInput(_))` - 请求参数无效
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::{AiProvider, CompletionRequest};
    ///
    /// # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
    /// let request = CompletionRequest::new("What is ownership in Rust?")
    ///     .with_system("You are a helpful Rust tutor.")
    ///     .with_temperature(0.7)
    ///     .with_max_tokens(500);
    ///
    /// match provider.complete(request).await {
    ///     Ok(response) => println!("{}", response.text),
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// 生成文本嵌入向量
    ///
    /// # Arguments
    /// * `request` - 嵌入请求参数
    ///
    /// # Returns
    /// * `Ok(EmbeddingResponse)` - 嵌入成功
    /// * `Err(CisError::Ai(_))` - AI 服务错误
    /// * `Err(CisError::InvalidInput(_))` - 请求参数无效（如文本过长）
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::{AiProvider, EmbeddingRequest};
    ///
    /// # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
    /// let request = EmbeddingRequest::new(vec![
    ///     "Machine learning is...".to_string(),
    ///     "Deep learning uses...".to_string(),
    /// ]);
    ///
    /// let response = provider.embedding(request).await?;
    /// 
    /// for (i, emb) in response.embeddings.iter().enumerate() {
    ///     println!("Text {}: {} dimensions", i, emb.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn embedding(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse>;

    /// 获取可用模型列表
    ///
    /// # Returns
    /// * `Ok(Vec<ModelInfo>)` - 模型列表
    /// * `Err(CisError::Ai(_))` - 获取失败
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::AiProvider;
    ///
    /// # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
    /// let models = provider.list_models().await?;
    /// for model in models {
    ///     println!("{}: {}", model.id, model.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// 检查 Provider 是否可用
    ///
    /// # Returns
    /// * `Ok(true)` - 服务可用
    /// * `Ok(false)` - 服务不可用（如 API 密钥未配置）
    /// * `Err(CisError::Ai(_))` - 检查过程中发生错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::AiProvider;
    ///
    /// # async fn example(provider: &dyn AiProvider) -> anyhow::Result<()> {
    /// match provider.health_check().await {
    ///     Ok(true) => println!("AI service is ready"),
    ///     Ok(false) => println!("AI service is unavailable"),
    ///     Err(e) => eprintln!("Health check failed: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn health_check(&self) -> Result<bool>;

    /// 获取默认模型信息
    ///
    /// # Returns
    /// * `Ok(ModelInfo)` - 默认模型信息
    /// * `Err(CisError::Ai(_))` - 获取失败
    fn default_model(&self) -> Result<ModelInfo>;
}

/// AiProvider 的 Arc 包装类型
pub type AiProviderRef = Arc<dyn AiProvider>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_builder() {
        let req = CompletionRequest::new("Hello")
            .with_system("You are helpful")
            .with_temperature(0.5)
            .with_max_tokens(100);

        assert_eq!(req.prompt, "Hello");
        assert_eq!(req.system, Some("You are helpful".to_string()));
        assert_eq!(req.temperature, Some(0.5));
        assert_eq!(req.max_tokens, Some(100));
    }

    #[test]
    fn test_embedding_request_builder() {
        let req = EmbeddingRequest::new(vec!["text1".to_string(), "text2".to_string()])
            .with_model("text-embedding-3-small")
            .with_dimensions(256);

        assert_eq!(req.texts.len(), 2);
        assert_eq!(req.model, Some("text-embedding-3-small".to_string()));
        assert_eq!(req.dimensions, Some(256));
    }

    #[test]
    fn test_temperature_clamping() {
        let req = CompletionRequest::new("Test").with_temperature(3.0);
        assert_eq!(req.temperature, Some(2.0));

        let req = CompletionRequest::new("Test").with_temperature(-0.5);
        assert_eq!(req.temperature, Some(0.0));
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }
}
