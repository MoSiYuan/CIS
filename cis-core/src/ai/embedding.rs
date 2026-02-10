//! # Embedding Service
//!
//! 提供文本向量化的统一接口，支持本地模型 (Nomic Embed Text v1.5) 和云端 API (OpenAI) 降级。
//!
//! ## 特性
//!
//! - 本地嵌入: 使用 ONNX Runtime 运行 Nomic Embed v1.5 (768维)
//! - 云端降级: OpenAI text-embedding-3-small 备用
//! - 延迟初始化: 首次调用时加载模型（约2秒）
//! - 批量处理: 支持批量向量化提升性能

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{CisError, Result};

/// 默认嵌入维度 (Nomic Embed v1.5)
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// 最小相似度阈值
pub const MIN_SIMILARITY_THRESHOLD: f32 = 0.6;

/// Embedding Service 统一接口
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// 单个文本向量化
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量文本向量化
    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// 获取嵌入维度
    fn dimension(&self) -> usize {
        DEFAULT_EMBEDDING_DIM
    }
}

/// Embedding Service 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmbeddingProvider {
    /// 本地 ONNX Runtime
    Local,
    /// OpenAI API
    OpenAI,
    /// 自动选择（优先本地）
    #[default]
    Auto,
}

/// Embedding Service 配置
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub provider: EmbeddingProvider,
    pub openai_api_key: Option<String>,
    pub openai_base_url: String,
    pub model_path: Option<String>,
    pub normalize: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: EmbeddingProvider::Auto,
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            openai_base_url: "https://api.openai.com/v1".to_string(),
            model_path: None,
            normalize: true,
        }
    }
}

/// 本地嵌入服务 (使用 FastEmbed)
///
/// 使用 fastembed-rs 本地运行嵌入模型，支持 Nomic Embed v1.5 等。
/// 首次加载会自动下载模型（如果需要），后续调用是毫秒级。
#[cfg(feature = "vector")]
pub struct LocalEmbeddingService {
    inner: crate::ai::embedding_fastembed::FastEmbedService,
}

impl LocalEmbeddingService {
    /// 使用默认模型创建服务 (Nomic Embed Text v1.5)
    pub async fn new() -> Result<Self> {
        let inner = crate::ai::embedding_fastembed::FastEmbedService::new().await?;
        Ok(Self { inner })
    }

    /// 使用配置创建服务
    pub async fn with_config(_config: &EmbeddingConfig) -> Result<Self> {
        // 目前使用默认模型，后续可根据配置选择
        Self::new().await
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.inner.embed(text).await
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.inner.batch_embed(texts).await
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

/// OpenAI Embedding Service
///
/// 调用 OpenAI API 获取文本嵌入，作为本地模型的降级方案。
pub struct OpenAIEmbeddingService {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    model: String,
    normalize: bool,
}

impl OpenAIEmbeddingService {
    /// 从环境变量创建服务
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| CisError::configuration("OPENAI_API_KEY not set"))?;
        Self::new(api_key)
    }

    /// 使用指定 API key 创建服务
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            client: reqwest::Client::new(),
            model: "text-embedding-3-small".to_string(),
            normalize: true,
        })
    }

    /// 使用配置创建服务
    pub fn with_config(config: &EmbeddingConfig) -> Result<Self> {
        let api_key = config
            .openai_api_key
            .clone()
            .ok_or_else(|| CisError::configuration("OpenAI API key not provided"))?;

        Ok(Self {
            api_key,
            base_url: config.openai_base_url.clone(),
            client: reqwest::Client::new(),
            model: "text-embedding-3-small".to_string(),
            normalize: config.normalize,
        })
    }

    fn normalize_vec(&self, mut vec: Vec<f32>) -> Vec<f32> {
        if !self.normalize {
            return vec;
        }
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut vec {
                *x /= norm;
            }
        }
        vec
    }
}

/// OpenAI Embedding API 响应
#[derive(Debug, serde::Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<OpenAIEmbeddingData>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAIEmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[async_trait]
impl EmbeddingService for OpenAIEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let response = self
            .client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "input": text,
                "model": self.model,
            }))
            .send()
            .await
            .map_err(|e| CisError::other(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text: String = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CisError::other(format!("OpenAI API error: {}", error_text)));
        }

        let result: OpenAIEmbeddingResponse = response
            .json::<OpenAIEmbeddingResponse>()
            .await
            .map_err(|e| CisError::other(format!("Failed to parse OpenAI response: {}", e)))?;

        result
            .data
            .into_iter()
            .next()
            .map(|d| self.normalize_vec(d.embedding))
            .ok_or_else(|| CisError::other("No embedding in response"))
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let response: reqwest::Response = self
            .client
            .post(format!("{}/embeddings", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "input": texts,
                "model": self.model,
            }))
            .send()
            .await
            .map_err(|e| CisError::other(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text: String = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(CisError::other(format!("OpenAI API error: {}", error_text)));
        }

        let result: OpenAIEmbeddingResponse = response
            .json::<OpenAIEmbeddingResponse>()
            .await
            .map_err(|e| CisError::other(format!("Failed to parse OpenAI response: {}", e)))?;

        let mut embeddings: Vec<_> = result
            .data
            .into_iter()
            .map(|d| (d.index, self.normalize_vec(d.embedding)))
            .collect();

        // 按原始顺序排序
        embeddings.sort_by_key(|(idx, _)| *idx);

        Ok(embeddings.into_iter().map(|(_, emb)| emb).collect())
    }

    fn dimension(&self) -> usize {
        // text-embedding-3-small 输出 1536 维
        1536
    }
}

/// 创建 Embedding Service 工厂函数
///
/// 根据配置自动选择本地或云端服务。
/// 
/// 注意：此函数不检查 Claude CLI，如需 Agent-first 策略，
/// 请使用 `create_embedding_service_with_fallback()` 函数。
///
/// 优先级（传统方式）：
/// 1. 如果配置指定 Local，尝试本地模型
/// 2. 如果配置指定 OpenAI，使用 OpenAI API
/// 3. 如果配置指定 Auto，先尝试本地模型，失败则降级到 OpenAI
/// 创建 embedding service (异步版本)
pub async fn create_embedding_service(config: Option<&EmbeddingConfig>) -> Result<Arc<dyn EmbeddingService>> {
    let config = config.cloned().unwrap_or_default();

    match config.provider {
        EmbeddingProvider::Local => {
            let service = LocalEmbeddingService::with_config(&config).await?;
            Ok(Arc::new(service))
        }
        EmbeddingProvider::OpenAI => {
            let service = OpenAIEmbeddingService::with_config(&config)?;
            Ok(Arc::new(service))
        }
        EmbeddingProvider::Auto => {
            // 先尝试本地模型 (FastEmbed 会自动下载)
            match LocalEmbeddingService::with_config(&config).await {
                Ok(service) => {
                    tracing::info!("Using local FastEmbed model (Nomic Embed Text v1.5)");
                    Ok(Arc::new(service))
                }
                Err(e) => {
                    tracing::warn!("Failed to load local embedding model: {}", e);
                    // 尝试 OpenAI 降级
                    if config.openai_api_key.is_some() {
                        tracing::info!("Falling back to OpenAI embedding service");
                        let service = OpenAIEmbeddingService::with_config(&config)?;
                        Ok(Arc::new(service))
                    } else {
                        Err(e)
                    }
                }
            }
        }
    }
}

/// 同步版本 (用于兼容性，内部会 block_on)
pub fn create_embedding_service_sync(config: Option<&EmbeddingConfig>) -> Result<Arc<dyn EmbeddingService>> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| CisError::configuration(format!("Failed to create runtime: {}", e)))?;
    rt.block_on(create_embedding_service(config))
}

/// 计算两个向量的余弦相似度
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot_product / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// 计算向量与查询的相似度并过滤
pub fn filter_by_similarity(
    query: &[f32],
    candidates: &[(String, Vec<f32>)],
    threshold: f32,
    limit: usize,
) -> Vec<(String, f32)> {
    let mut results: Vec<_> = candidates
        .iter()
        .filter_map(|(id, vec)| {
            let sim = cosine_similarity(query, vec);
            if sim >= threshold {
                Some((id.clone(), sim))
            } else {
                None
            }
        })
        .collect();

    // 按相似度降序排序
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    results
}

/// Claude CLI Embedding Service
///
/// 使用 FastEmbed 本地模型生成文本嵌入。
/// 这是一个实际的嵌入服务实现，使用 Nomic Embed Text v1.5 模型。
#[cfg(feature = "vector")]
pub struct ClaudeCliEmbeddingService {
    inner: crate::ai::embedding_fastembed::FastEmbedService,
}

#[cfg(not(feature = "vector"))]
pub struct ClaudeCliEmbeddingService;

#[cfg(feature = "vector")]
impl Default for ClaudeCliEmbeddingService {
    fn default() -> Self {
        // 在同步上下文中无法使用 async，返回一个占位符
        panic!("Use ClaudeCliEmbeddingService::new() async method instead")
    }
}

#[cfg(not(feature = "vector"))]
impl Default for ClaudeCliEmbeddingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "vector")]
impl ClaudeCliEmbeddingService {
    /// 使用默认模型创建服务 (Nomic Embed Text v1.5)
    pub async fn new() -> Self {
        match crate::ai::embedding_fastembed::FastEmbedService::new().await {
            Ok(inner) => Self { inner },
            Err(e) => panic!("Failed to initialize embedding service: {}", e),
        }
    }
}

#[cfg(not(feature = "vector"))]
impl ClaudeCliEmbeddingService {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "vector")]
#[async_trait]
impl EmbeddingService for ClaudeCliEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.inner.embed(text).await
    }
    
    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.inner.batch_embed(texts).await
    }
    
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

#[cfg(not(feature = "vector"))]
#[async_trait]
impl EmbeddingService for ClaudeCliEmbeddingService {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(CisError::configuration("vector feature not enabled"))
    }
    
    async fn batch_embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(CisError::configuration("vector feature not enabled"))
    }
    
    fn dimension(&self) -> usize {
        0
    }
}

/// SQL Fallback Embedding Service
///
/// 使用 FastEmbed 本地模型的嵌入服务。
/// 作为最终的回退方案，确保始终有可用的嵌入功能。
#[cfg(feature = "vector")]
pub struct SqlFallbackEmbeddingService {
    inner: crate::ai::embedding_fastembed::FastEmbedService,
}

#[cfg(not(feature = "vector"))]
pub struct SqlFallbackEmbeddingService;

#[cfg(feature = "vector")]
impl Default for SqlFallbackEmbeddingService {
    fn default() -> Self {
        // 在同步上下文中无法使用 async，返回一个占位符
        // 实际使用应该通过 new() async 方法
        panic!("Use SqlFallbackEmbeddingService::new() async method instead")
    }
}

#[cfg(not(feature = "vector"))]
impl Default for SqlFallbackEmbeddingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "vector")]
impl SqlFallbackEmbeddingService {
    /// 创建新的 SQL Fallback 嵌入服务（使用 FastEmbed）
    pub async fn new() -> Result<Self> {
        let inner = crate::ai::embedding_fastembed::FastEmbedService::new().await?;
        Ok(Self { inner })
    }
}

#[cfg(not(feature = "vector"))]
impl SqlFallbackEmbeddingService {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "vector")]
#[async_trait]
impl EmbeddingService for SqlFallbackEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.inner.embed(text).await
    }
    
    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.inner.batch_embed(texts).await
    }
    
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}

#[cfg(not(feature = "vector"))]
#[async_trait]
impl EmbeddingService for SqlFallbackEmbeddingService {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(CisError::configuration("vector feature not enabled"))
    }
    
    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(CisError::configuration("vector feature not enabled"))
    }
    
    fn dimension(&self) -> usize {
        0
    }
}

/// Embedding Service 创建函数（支持所有选项）
/// 
/// 优先级（从高到低）：
/// 1. 本地模型（Nomic Embed v1.5）- 最高优先级
/// 2. OpenAI API（需要 API Key）
/// 3. SQL Fallback（FastEmbed 本地模型）
pub async fn create_embedding_service_with_fallback(
    config: Option<&EmbeddingConfig>,
    init_config: &crate::ai::embedding_init::EmbeddingInitConfig,
) -> Result<Arc<dyn EmbeddingService>> {
    use crate::ai::embedding_init::EmbeddingInitOption;
    
    match init_config.option {
        EmbeddingInitOption::DownloadLocalModel | EmbeddingInitOption::Skip => {
            // 本地模型优先策略 (FastEmbed)
            // 1. 首先尝试 FastEmbed 本地模型 (自动下载)
            tracing::info!("Initializing FastEmbed with Nomic Embed Text v1.5");
            match LocalEmbeddingService::new().await {
                Ok(service) => {
                    tracing::info!("Using FastEmbed local model (Nomic Embed Text v1.5)");
                    return Ok(Arc::new(service) as Arc<dyn EmbeddingService>);
                }
                Err(e) => tracing::warn!("FastEmbed failed: {}, trying alternatives", e),
            }
            
            // 2. 尝试 OpenAI 或 SQL 回退
            match create_embedding_service(config).await {
                Ok(service) => Ok(service),
                Err(e) => {
                    tracing::warn!("OpenAI embedding failed ({}), trying SQL fallback", e);
                    #[cfg(feature = "vector")]
                    {
                        let service = SqlFallbackEmbeddingService::new().await?;
                        Ok(Arc::new(service) as Arc<dyn EmbeddingService>)
                    }
                    #[cfg(not(feature = "vector"))]
                    {
                        Err(CisError::configuration("No embedding service available and vector feature not enabled"))
                    }
                }
            }
        }
        EmbeddingInitOption::UseOpenAI => {
            if let Some(ref key) = init_config.openai_api_key {
                let mut config = config.cloned().unwrap_or_default();
                config.openai_api_key = Some(key.clone());
                config.provider = EmbeddingProvider::OpenAI;
                OpenAIEmbeddingService::with_config(&config)
                    .map(|s| Arc::new(s) as Arc<dyn EmbeddingService>)
            } else {
                create_embedding_service(config).await
            }
        }
        EmbeddingInitOption::UseClaudeCli => {
            #[cfg(feature = "vector")]
            {
                let service = ClaudeCliEmbeddingService::new().await;
                Ok(Arc::new(service) as Arc<dyn EmbeddingService>)
            }
            #[cfg(not(feature = "vector"))]
            {
                Err(CisError::configuration("vector feature not enabled"))
            }
        }
        EmbeddingInitOption::UseSqlFallback => {
            #[cfg(feature = "vector")]
            {
                let service = SqlFallbackEmbeddingService::new().await?;
                Ok(Arc::new(service) as Arc<dyn EmbeddingService>)
            }
            #[cfg(not(feature = "vector"))]
            {
                Err(CisError::configuration("vector feature not enabled"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        // 相同向量相似度为 1
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        // 正交向量相似度为 0
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);

        // 相反向量相似度为 -1
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_filter_by_similarity() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            ("a".to_string(), vec![1.0, 0.0, 0.0]), // sim = 1.0
            ("b".to_string(), vec![0.0, 1.0, 0.0]), // sim = 0.0
            ("c".to_string(), vec![0.9, 0.1, 0.0]), // sim ≈ 0.99
        ];

        let results = filter_by_similarity(&query, &candidates, 0.5, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a");
        assert_eq!(results[1].0, "c");
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, EmbeddingProvider::Auto);
        assert!(config.normalize);
    }
}
