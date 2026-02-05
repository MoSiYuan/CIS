//! # Embedding Service
//!
//! 提供文本向量化的统一接口，支持本地模型 (MiniLM-L6-v2) 和云端 API (OpenAI) 降级。
//!
//! ## 特性
//!
//! - 本地嵌入: 使用 ONNX Runtime 运行 MiniLM-L6-v2 (768维)
//! - 云端降级: OpenAI text-embedding-3-small 备用
//! - 延迟初始化: 首次调用时加载模型（约2秒）
//! - 批量处理: 支持批量向量化提升性能

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{CisError, Result};

/// 默认嵌入维度 (MiniLM-L6-v2)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingProvider {
    /// 本地 ONNX Runtime
    Local,
    /// OpenAI API
    OpenAI,
    /// 自动选择（优先本地）
    Auto,
}

impl Default for EmbeddingProvider {
    fn default() -> Self {
        Self::Auto
    }
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

/// 本地嵌入服务 (MiniLM-L6-v2 via ONNX Runtime)
///
/// 使用 ONNX Runtime 本地运行 all-MiniLM-L6-v2 模型，输出 768 维向量。
/// 首次加载模型可能需要 1-3 秒，后续调用是毫秒级。
#[cfg(feature = "vector")]
pub struct LocalEmbeddingService {
    tokenizer: tokenizers::Tokenizer,
    model_path: std::path::PathBuf,
    normalize: bool,
    dimension: usize,
}

#[cfg(not(feature = "vector"))]
pub struct LocalEmbeddingService {
    dimension: usize,
    _phantom: std::marker::PhantomData<()>,
}

impl LocalEmbeddingService {
    /// 使用默认模型路径创建服务
    pub fn new() -> Result<Self> {
        Self::with_config(&EmbeddingConfig::default())
    }

    /// 使用配置创建服务
    #[cfg(feature = "vector")]
    pub fn with_config(config: &EmbeddingConfig) -> Result<Self> {
        // 确定模型路径
        let model_dir = config.model_path.as_ref()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                // 尝试默认路径
                let default_paths = [
                    "models/all-MiniLM-L6-v2",
                    "models/all-MiniLM-L6-v2.onnx",
                    "/usr/share/cis/models/all-MiniLM-L6-v2",
                ];
                default_paths.iter()
                    .map(|p| std::path::PathBuf::from(p))
                    .find(|p| p.exists())
            })
            .ok_or_else(|| CisError::configuration(
                "Model path not specified and default model not found. \
                 Please specify model_path in config or place model at models/all-MiniLM-L6-v2/"
            ))?;

        // 加载 tokenizer
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = if tokenizer_path.exists() {
            tokenizers::Tokenizer::from_file(&tokenizer_path)
                .map_err(|e| CisError::configuration(format!("Failed to load tokenizer: {}", e)))?
        } else {
            // 尝试从 Hugging Face 格式加载
            let vocab_path = model_dir.join("vocab.txt");
            if vocab_path.exists() {
                let tokenizer = tokenizers::Tokenizer::new(
                    tokenizers::models::wordpiece::WordPiece::builder()
                        .files(vocab_path.to_string_lossy().to_string())
                        .build()
                        .map_err(|e| CisError::configuration(format!("Failed to load WordPiece: {}", e)))?
                );
                tokenizer
            } else {
                return Err(CisError::configuration(
                    format!("Tokenizer not found at {:?}", tokenizer_path)
                ));
            }
        };

        // 检查 ONNX 模型文件
        let model_path = if model_dir.extension().map(|e| e == "onnx").unwrap_or(false) {
            model_dir.clone()
        } else {
            model_dir.join("model.onnx")
        };

        if !model_path.exists() {
            return Err(CisError::configuration(
                format!("ONNX model not found at {:?}", model_path)
            ));
        }

        // TODO: 初始化 ONNX Runtime 会话
        // 由于 ort 2.0 API 变化较大，需要更仔细地适配
        // 目前返回一个占位服务，提示用户使用 OpenAI 降级
        tracing::warn!("ONNX Runtime embedding not yet fully implemented. Found model at {:?}", model_path);
        tracing::info!("Please use OpenAI embedding service as fallback: set OPENAI_API_KEY environment variable");

        Ok(Self {
            tokenizer,
            model_path,
            normalize: config.normalize,
            dimension: DEFAULT_EMBEDDING_DIM,
        })
    }

    #[cfg(not(feature = "vector"))]
    pub fn with_config(_config: &EmbeddingConfig) -> Result<Self> {
        Err(CisError::configuration(
            "Vector feature not enabled. Enable 'vector' feature or use OpenAI embedding service.",
        ))
    }

    /// 向量归一化 (L2)
    #[cfg(feature = "vector")]
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

    #[cfg(feature = "vector")]
    fn encode_internal(&self, _text: &str) -> Result<Vec<f32>> {
        // ONNX Runtime 推理尚未完全实现
        // 返回错误提示用户使用 OpenAI 降级
        Err(CisError::configuration(
            "Local ONNX embedding not yet fully implemented. \
             Please use OpenAI embedding service by setting OPENAI_API_KEY environment variable. \
             Model found at: ".to_string() + &self.model_path.to_string_lossy()
        ))
    }

    #[cfg(not(feature = "vector"))]
    fn encode_internal(&self, _text: &str) -> Result<Vec<f32>> {
        Err(CisError::configuration("Vector feature not enabled"))
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.encode_internal(text)
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.encode_internal(text)?);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
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
/// 优先级：
/// 1. 如果配置指定 Local，尝试本地模型
/// 2. 如果配置指定 OpenAI，使用 OpenAI API
/// 3. 如果配置指定 Auto，先尝试本地，失败则降级到 OpenAI
pub fn create_embedding_service(config: Option<&EmbeddingConfig>) -> Result<Arc<dyn EmbeddingService>> {
    let config = config.cloned().unwrap_or_default();

    match config.provider {
        EmbeddingProvider::Local => {
            let service = LocalEmbeddingService::with_config(&config)?;
            Ok(Arc::new(service))
        }
        EmbeddingProvider::OpenAI => {
            let service = OpenAIEmbeddingService::with_config(&config)?;
            Ok(Arc::new(service))
        }
        EmbeddingProvider::Auto => {
            // 先尝试本地模型
            match LocalEmbeddingService::with_config(&config) {
                Ok(service) => Ok(Arc::new(service)),
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
