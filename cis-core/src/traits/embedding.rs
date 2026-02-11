//! # EmbeddingService Trait
//!
//! 文本嵌入服务的抽象接口，定义向量化的基本操作。
//!
//! ## 设计原则
//!
//! - **批处理优化**: 支持批量嵌入以提高吞吐量
//! - **本地优先**: 支持本地模型，保护数据隐私
//! - **相似度计算**: 内置常用相似度算法
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::traits::EmbeddingServiceTrait;
//! use std::sync::Arc;
//!
//! # async fn example(service: Arc<dyn EmbeddingServiceTrait>) -> anyhow::Result<()> {
//! // 单个文本嵌入
//! let embedding = service.embed("Hello world").await?;
//! println!("Dimension: {}", embedding.len());
//!
//! // 批量嵌入
//! let texts = vec!["Text 1", "Text 2", "Text 3"];
//! let embeddings = service.embed_batch(&texts).await?;
//!
//! // 计算相似度
//! let similarity = service.cosine_similarity(&embeddings[0], &embeddings[1])?;
//! println!("Similarity: {}", similarity);
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use crate::error::{CisError, Result};
use std::sync::Arc;

/// 嵌入模型信息
#[derive(Debug, Clone)]
pub struct EmbeddingModelInfo {
    /// 模型名称
    pub name: String,
    /// 维度
    pub dimension: usize,
    /// 最大输入长度
    pub max_input_length: usize,
    /// 描述
    pub description: String,
    /// 模型提供商
    pub provider: String,
    /// 是否支持本地运行
    pub supports_local: bool,
}

/// 嵌入服务抽象接口
///
/// 定义文本向量化的基本操作，支持单条和批量处理。
///
/// ## 实现要求
///
/// - 所有方法必须是线程安全的 (Send + Sync)
/// - 异步方法返回 Result 类型
/// - 同步方法返回 Result 类型
/// - 实现应该缓存模型以提高性能
///
/// ## 使用示例
///
/// ```rust,no_run
/// use cis_core::traits::EmbeddingServiceTrait;
///
/// # async fn example(service: &dyn EmbeddingServiceTrait) -> anyhow::Result<()> {
/// // 获取模型维度
/// let dim = service.dimension();
/// println!("Embedding dimension: {}", dim);
///
/// // 嵌入单个文本
/// let vec1 = service.embed("Machine learning").await?;
/// let vec2 = service.embed("Deep learning").await?;
///
/// // 计算相似度
/// let sim = service.cosine_similarity(&vec1, &vec2)?;
/// println!("Similarity: {}", sim);
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait EmbeddingServiceTrait: Send + Sync {
    /// 嵌入单个文本
    ///
    /// # Arguments
    /// * `text` - 输入文本
    ///
    /// # Returns
    /// * `Ok(Vec<f32>)` - 嵌入向量
    /// * `Err(CisError::Vector(_))` - 嵌入失败
    /// * `Err(CisError::InvalidInput(_))` - 输入文本过长或无效
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::EmbeddingServiceTrait;
    ///
    /// # async fn example(service: &dyn EmbeddingServiceTrait) -> anyhow::Result<()> {
    /// let embedding = service.embed("Rust programming language").await?;
    /// println!("Vector length: {}", embedding.len());
    /// # Ok(())
    /// # }
    /// ```
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量嵌入
    ///
    /// # Arguments
    /// * `texts` - 输入文本列表
    ///
    /// # Returns
    /// * `Ok(Vec<Vec<f32>>)` - 嵌入向量列表（与输入一一对应）
    /// * `Err(CisError::Vector(_))` - 嵌入失败
    /// * `Err(CisError::InvalidInput(_))` - 输入无效
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::EmbeddingServiceTrait;
    ///
    /// # async fn example(service: &dyn EmbeddingServiceTrait) -> anyhow::Result<()> {
    /// let texts = vec![
    ///     "Machine learning",
    ///     "Deep learning",
    ///     "Neural networks",
    /// ];
    ///
    /// let embeddings = service.embed_batch(&texts).await?;
    /// 
    /// // 计算所有配对相似度
    /// for i in 0..embeddings.len() {
    ///     for j in (i+1)..embeddings.len() {
    ///         let sim = service.cosine_similarity(&embeddings[i], &embeddings[j])?;
    ///         println!("Similarity {}-{}: {}", i, j, sim);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// 获取嵌入维度
    ///
    /// # Returns
    /// 向量维度（如 768、1536 等）
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::EmbeddingServiceTrait;
    ///
    /// fn print_dimension(service: &dyn EmbeddingServiceTrait) {
    ///     println!("Embedding dimension: {}", service.dimension());
    /// }
    /// ```
    fn dimension(&self) -> usize;

    /// 获取模型信息
    ///
    /// # Returns
    /// * `Ok(EmbeddingModelInfo)` - 模型信息
    /// * `Err(CisError::Vector(_))` - 获取失败
    fn model_info(&self) -> Result<EmbeddingModelInfo>;

    /// 计算两个向量的余弦相似度
    ///
    /// # Arguments
    /// * `a` - 向量 A
    /// * `b` - 向量 B
    ///
    /// # Returns
    /// * `Ok(f32)` - 余弦相似度 (-1.0 到 1.0)
    /// * `Err(CisError::InvalidInput(_))` - 向量维度不匹配
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::EmbeddingServiceTrait;
    ///
    /// # async fn example(service: &dyn EmbeddingServiceTrait) -> anyhow::Result<()> {
    /// let vec1 = service.embed("Python programming").await?;
    /// let vec2 = service.embed("Rust programming").await?;
    /// let vec3 = service.embed("Apple pie recipe").await?;
    ///
    /// let sim1 = service.cosine_similarity(&vec1, &vec2)?;
    /// let sim2 = service.cosine_similarity(&vec1, &vec3)?;
    ///
    /// println!("Programming similarity: {}", sim1); // 较高
    /// println!("Unrelated similarity: {}", sim2);    // 较低
    /// # Ok(())
    /// # }
    /// ```
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(CisError::invalid_input(format!(
                "Vectors must have the same dimension: {} vs {}",
                a.len(),
                b.len()
            )));
        }

        if a.is_empty() {
            return Err(CisError::invalid_input(
                "Vectors cannot be empty".to_string()
            ));
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(0.0);
        }

        Ok(dot_product / (norm_a * norm_b))
    }

    /// 计算两个向量的欧氏距离
    ///
    /// # Arguments
    /// * `a` - 向量 A
    /// * `b` - 向量 B
    ///
    /// # Returns
    /// * `Ok(f32)` - 欧氏距离（越小越相似）
    /// * `Err(CisError::InvalidInput(_))` - 向量维度不匹配
    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(CisError::invalid_input(format!(
                "Vectors must have the same dimension: {} vs {}",
                a.len(),
                b.len()
            )));
        }

        let distance: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt();

        Ok(distance)
    }

    /// 计算两个向量的点积
    ///
    /// # Arguments
    /// * `a` - 向量 A
    /// * `b` - 向量 B
    ///
    /// # Returns
    /// * `Ok(f32)` - 点积结果
    /// * `Err(CisError::InvalidInput(_))` - 向量维度不匹配
    fn dot_product(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(CisError::invalid_input(format!(
                "Vectors must have the same dimension: {} vs {}",
                a.len(),
                b.len()
            )));
        }

        Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
    }

    /// 检查服务是否健康
    ///
    /// # Returns
    /// * `Ok(true)` - 服务正常
    /// * `Ok(false)` - 服务异常（如模型未加载）
    /// * `Err(CisError::Vector(_))` - 检查失败
    async fn health_check(&self) -> Result<bool>;
}

/// EmbeddingServiceTrait 的 Arc 包装类型
pub type EmbeddingServiceRef = Arc<dyn EmbeddingServiceTrait>;

/// 支持的嵌入模型列表
pub static SUPPORTED_MODELS: &[EmbeddingModelInfo] = &[
    EmbeddingModelInfo {
        name: String::new(),
        dimension: 768,
        max_input_length: 8192,
        description: String::new(),
        provider: String::new(),
        supports_local: true,
    },
];

/// 获取默认模型信息
///
/// # Returns
/// 默认模型信息的引用
pub fn default_model_info() -> &'static EmbeddingModelInfo {
    &SUPPORTED_MODELS[0]
}

/// 计算余弦相似度（独立函数）
///
/// 不依赖于具体服务实例，可直接使用。
///
/// # Arguments
/// * `a` - 向量 A
/// * `b` - 向量 B
///
/// # Returns
/// * `Ok(f32)` - 余弦相似度
/// * `Err(CisError::InvalidInput(_))` - 维度不匹配或空向量
///
/// # Examples
///
/// ```rust
/// use cis_core::traits::cosine_similarity;
///
/// let a = vec![1.0, 0.0, 0.0];
/// let b = vec![0.0, 1.0, 0.0];
///
/// let sim = cosine_similarity(&a, &b).unwrap();
/// assert!((sim - 0.0).abs() < 0.001); // 正交向量，相似度为 0
/// ```
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        return Err(CisError::invalid_input(format!(
            "Vectors must have the same dimension: {} vs {}",
            a.len(),
            b.len()
        )));
    }

    if a.is_empty() {
        return Err(CisError::invalid_input(
            "Vectors cannot be empty".to_string()
        ));
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return Ok(0.0);
    }

    Ok((dot_product / (norm_a * norm_b)).clamp(-1.0, 1.0))
}

/// 过滤相似度结果
///
/// 返回高于阈值的项目及其相似度分数。
///
/// # Arguments
/// * `query` - 查询向量
/// * `candidates` - 候选向量列表
/// * `threshold` - 相似度阈值 (0.0 - 1.0)
///
/// # Returns
/// * `Ok(Vec<(usize, f32)>)` - (索引, 相似度) 列表，按相似度降序排列
///
/// # Examples
///
/// ```rust
/// use cis_core::traits::filter_by_similarity;
///
/// # async fn example() -> anyhow::Result<()> {
/// let query = vec![1.0, 0.0, 0.0];
/// let candidates = vec![
///     vec![0.9, 0.1, 0.0],
///     vec![0.0, 1.0, 0.0],
///     vec![0.8, 0.2, 0.0],
/// ];
///
/// let results = filter_by_similarity(&query, &candidates, 0.7)?;
/// // results 将包含索引 0 和 2（相似度 > 0.7）
/// # Ok(())
/// # }
/// ```
pub fn filter_by_similarity(
    query: &[f32],
    candidates: &[Vec<f32>],
    threshold: f32,
) -> Result<Vec<(usize, f32)>> {
    let threshold = threshold.clamp(0.0, 1.0);
    let mut results: Vec<(usize, f32)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| {
            match cosine_similarity(query, candidate) {
                Ok(sim) if sim >= threshold => Some((idx, sim)),
                _ => None,
            }
        })
        .collect();

    // 按相似度降序排列
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cosine_similarity() {
        // 相同向量，相似度为 1
        let a = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &a).unwrap();
        assert!((sim - 1.0).abs() < 0.001);

        // 正交向量，相似度为 0
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(sim.abs() < 0.001);

        // 相反向量，相似度为 -1
        let c = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &c).unwrap();
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_dimension_mismatch() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &b).is_err());
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert!(cosine_similarity(&a, &b).is_err());
    }

    #[test]
    fn test_euclidean_distance() {
        struct MockService;
        
        #[async_trait::async_trait]
        impl EmbeddingServiceTrait for MockService {
            async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
                Ok(vec![])
            }
            async fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
                Ok(vec![])
            }
            fn dimension(&self) -> usize {
                3
            }
            fn model_info(&self) -> Result<EmbeddingModelInfo> {
                Ok(default_model_info().clone())
            }
            async fn health_check(&self) -> Result<bool> {
                Ok(true)
            }
        }

        let service = MockService;
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let dist = service.euclidean_distance(&a, &b).unwrap();
        assert!((dist - 5.0).abs() < 0.001); // 3-4-5 三角形
    }

    #[test]
    fn test_filter_by_similarity() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![0.9, 0.1, 0.0],  // 高相似度
            vec![0.0, 1.0, 0.0],  // 低相似度（正交）
            vec![0.8, 0.2, 0.0],  // 高相似度
            vec![0.1, 0.9, 0.0],  // 低相似度
        ];

        let results = filter_by_similarity(&query, &candidates, 0.7).unwrap();
        
        // 应该返回 2 个结果
        assert_eq!(results.len(), 2);
        
        // 第一个应该是最相似的（索引 0）
        assert_eq!(results[0].0, 0);
        assert!(results[0].1 > 0.9);

        // 第二个应该是索引 2
        assert_eq!(results[1].0, 2);
    }

    #[test]
    fn test_embedding_model_info() {
        let info = EmbeddingModelInfo {
            name: "test-model".to_string(),
            dimension: 768,
            max_input_length: 512,
            description: "Test model".to_string(),
            provider: "test".to_string(),
            supports_local: true,
        };

        assert_eq!(info.name, "test-model");
        assert_eq!(info.dimension, 768);
        assert!(info.supports_local);
    }
}
