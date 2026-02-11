//! # Mock Embedding Service
//!
//! 嵌入服务的 Mock 实现，用于测试向量相关功能。

use async_trait::async_trait;
use crate::error::{CisError, Result};
use crate::traits::{EmbeddingServiceTrait, EmbeddingModelInfo};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock 嵌入服务
#[derive(Debug, Clone)]
pub struct MockEmbeddingService {
    dimension: usize,
    fixed_embedding: Option<Vec<f32>>,
    text_embeddings: Arc<Mutex<HashMap<String, Vec<f32>>>>,
    should_fail: Arc<Mutex<Option<String>>>,
    call_count: Arc<Mutex<usize>>,
    healthy: Arc<Mutex<bool>>,
    model_name: String,
}

impl MockEmbeddingService {
    /// 创建新的 Mock
    pub fn new() -> Self {
        Self {
            dimension: 768,
            fixed_embedding: None,
            text_embeddings: Arc::new(Mutex::new(HashMap::new())),
            should_fail: Arc::new(Mutex::new(None)),
            call_count: Arc::new(Mutex::new(0)),
            healthy: Arc::new(Mutex::new(true)),
            model_name: "mock-embedding-model".to_string(),
        }
    }

    /// 设置嵌入维度
    pub fn with_dimension(mut self, dimension: usize) -> Self {
        self.dimension = dimension;
        self
    }

    /// 设置固定嵌入向量（所有文本返回相同向量）
    pub fn with_fixed_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.dimension = embedding.len();
        self.fixed_embedding = Some(embedding);
        self
    }

    /// 设置模型名称
    pub fn with_model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }

    /// 预设文本的嵌入向量
    pub fn preset_embedding(&self, text: impl Into<String>, embedding: Vec<f32>) {
        let mut map = self.text_embeddings.lock().unwrap();
        map.insert(text.into(), embedding);
    }

    /// 设置下次调用失败
    pub fn will_fail(&self, message: impl Into<String>) {
        *self.should_fail.lock().unwrap() = Some(message.into());
    }

    /// 设置健康状态
    pub fn set_healthy(&self, healthy: bool) {
        *self.healthy.lock().unwrap() = healthy;
    }

    /// 获取调用次数
    pub fn call_count(&self) -> usize {
        *self.call_count.lock().unwrap()
    }

    /// 生成确定性嵌入（基于文本的哈希）
    fn generate_deterministic_embedding(&self, text: &str) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let hash = hasher.finish();

        // 使用哈希值生成伪随机但确定的向量
        let mut embedding = Vec::with_capacity(self.dimension);
        for i in 0..self.dimension {
            let value = (((hash.wrapping_add(i as u64) % 1000) as f32) / 1000.0) * 2.0 - 1.0;
            embedding.push(value);
        }

        // 归一化
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    /// 清空调用计数
    pub fn clear(&self) {
        *self.call_count.lock().unwrap() = 0;
        self.text_embeddings.lock().unwrap().clear();
    }
}

impl Default for MockEmbeddingService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingServiceTrait for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        *self.call_count.lock().unwrap() += 1;

        // 检查是否应该失败
        if let Some(msg) = self.should_fail.lock().unwrap().take() {
            return Err(CisError::vector(format!("Mock embedding failed: {}", msg)));
        }

        // 返回固定嵌入或预设嵌入
        if let Some(ref fixed) = self.fixed_embedding {
            return Ok(fixed.clone());
        }

        let embeddings = self.text_embeddings.lock().unwrap();
        if let Some(embedding) = embeddings.get(text) {
            return Ok(embedding.clone());
        }
        drop(embeddings);

        // 生成确定性嵌入
        Ok(self.generate_deterministic_embedding(text))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_info(&self) -> Result<EmbeddingModelInfo> {
        Ok(EmbeddingModelInfo {
            name: self.model_name.clone(),
            dimension: self.dimension,
            max_input_length: 8192,
            description: "Mock embedding model for testing".to_string(),
            provider: "mock".to_string(),
            supports_local: true,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        if let Some(msg) = self.should_fail.lock().unwrap().take() {
            return Err(CisError::vector(format!("Health check failed: {}", msg)));
        }
        Ok(*self.healthy.lock().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embed() {
        let mock = MockEmbeddingService::new();

        let embedding = mock.embed("Hello world").await.unwrap();
        assert_eq!(embedding.len(), 768);

        // 相同文本应该产生相同嵌入
        let embedding2 = mock.embed("Hello world").await.unwrap();
        assert_eq!(embedding, embedding2);

        // 不同文本应该产生不同嵌入
        let embedding3 = mock.embed("Different text").await.unwrap();
        assert_ne!(embedding, embedding3);
    }

    #[tokio::test]
    async fn test_mock_preset_embedding() {
        let mock = MockEmbeddingService::new();
        let preset = vec![0.1; 768];

        mock.preset_embedding("test", preset.clone());

        let embedding = mock.embed("test").await.unwrap();
        assert_eq!(embedding, preset);
    }

    #[tokio::test]
    async fn test_mock_fixed_embedding() {
        let fixed = vec![0.5; 768];
        let mock = MockEmbeddingService::new().with_fixed_embedding(fixed.clone());

        let embedding1 = mock.embed("text1").await.unwrap();
        let embedding2 = mock.embed("text2").await.unwrap();

        assert_eq!(embedding1, fixed);
        assert_eq!(embedding2, fixed);
    }

    #[tokio::test]
    async fn test_mock_batch_embed() {
        let mock = MockEmbeddingService::new();

        let texts = vec!["Hello", "World", "Test"];
        let embeddings = mock.embed_batch(&texts).await.unwrap();

        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].len(), 768);
    }

    #[tokio::test]
    async fn test_mock_error() {
        let mock = MockEmbeddingService::new();
        mock.will_fail("Simulated error");

        let result = mock.embed("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_call_count() {
        let mock = MockEmbeddingService::new();

        assert_eq!(mock.call_count(), 0);

        mock.embed("test1").await.unwrap();
        assert_eq!(mock.call_count(), 1);

        mock.embed("test2").await.unwrap();
        assert_eq!(mock.call_count(), 2);

        mock.clear();
        assert_eq!(mock.call_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_dimension() {
        let mock = MockEmbeddingService::new().with_dimension(384);
        assert_eq!(mock.dimension(), 384);

        let embedding = mock.embed("test").await.unwrap();
        assert_eq!(embedding.len(), 384);
    }

    #[tokio::test]
    async fn test_model_info() {
        let mock = MockEmbeddingService::new()
            .with_dimension(512)
            .with_model_name("test-model");

        let info = mock.model_info().unwrap();
        assert_eq!(info.name, "test-model");
        assert_eq!(info.dimension, 512);
        assert!(info.supports_local);
    }

    #[tokio::test]
    async fn test_health_check() {
        let mock = MockEmbeddingService::new();

        // 默认健康
        assert!(mock.health_check().await.unwrap());

        // 设置为不健康
        mock.set_healthy(false);
        assert!(!mock.health_check().await.unwrap());

        // 恢复为健康
        mock.set_healthy(true);
        assert!(mock.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_error() {
        let mock = MockEmbeddingService::new();
        mock.will_fail("Health check failed");

        let result = mock.health_check().await;
        assert!(result.is_err());
    }
}
