//! 统一的 Embedding 服务
//!
//! 使用 fastembed 提供真实的文本嵌入。
//!
//! ## 废弃警告
//!
//! `EmbeddingService::global()` 全局单例方法已废弃。
//! 请使用 `ServiceContainer` 进行依赖注入。

use anyhow::{anyhow, Result};
use fastembed::{InitOptions, TextEmbedding, EmbeddingModel};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::traits::EmbeddingServiceTrait;
use async_trait::async_trait;

/// Embedding 服务
pub struct EmbeddingService {
    model: Arc<Mutex<TextEmbedding>>,
    dimension: usize,
}

impl EmbeddingService {
    /// 创建新的 Embedding 服务
    /// 
    /// 首次调用时会自动下载模型 (~130MB)
    pub async fn new() -> Result<Self> {
        let model = tokio::task::spawn_blocking(|| {
            TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::NomicEmbedTextV15)
                    .with_show_download_progress(true)
            )
        })
        .await
        .map_err(|e| anyhow!("Failed to create embedding model: {}", e))?
        .map_err(|e| anyhow!("Failed to initialize embedding: {}", e))?;
        
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            dimension: 768, // Nomic Embed Text v1.5
        })
    }
    
    /// 嵌入单个文本
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let model = self.model.clone();
        let text = text.to_string();
        
        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            model.embed(vec![&text], None)
        })
        .await
        .map_err(|e| anyhow!("Embedding task failed: {}", e))?
        .map_err(|e| anyhow!("Embedding failed: {}", e))?;
        
        Ok(embeddings[0].clone())
    }
    
    /// 批量嵌入
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let model = self.model.clone();
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        
        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            model.embed(text_refs, None)
        })
        .await
        .map_err(|e| anyhow!("Embedding task failed: {}", e))?
        .map_err(|e| anyhow!("Embedding failed: {}", e))?;
        
        Ok(embeddings)
    }
    
    /// 获取嵌入维度
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

/// 全局 Embedding 服务实例 (DEPRECATED)
///
/// [WARNING] 警告: 此全局单例已废弃，将在 v1.2.0 中移除。
/// 请使用 `ServiceContainer` 进行依赖注入。
#[deprecated(
    since = "1.1.4",
    note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
)]
static EMBEDDING_SERVICE: tokio::sync::OnceCell<EmbeddingService> = tokio::sync::OnceCell::const_new();

impl EmbeddingService {
    /// 获取全局实例 (DEPRECATED)
    ///
    /// [WARNING] 警告: 此方法已废弃，将在 v1.2.0 中移除。
    /// 请使用 `ServiceContainer` 进行依赖注入。
    #[deprecated(
        since = "1.1.4",
        note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
    )]
    #[allow(deprecated)]
    pub async fn global() -> Result<&'static Self> {
        EMBEDDING_SERVICE.get_or_try_init(|| async {
            Self::new().await
        }).await
    }
}

#[async_trait]
impl EmbeddingServiceTrait for EmbeddingService {
    async fn embed(&self, text: &str) -> crate::error::Result<Vec<f32>> {
        let model = self.model.clone();
        let text = text.to_string();
        
        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            model.embed(vec![&text], None)
        })
        .await
        .map_err(|e| crate::error::CisError::ai(format!("Embedding task failed: {}", e)))?
        .map_err(|e| crate::error::CisError::ai(format!("Embedding failed: {}", e)))?;
        
        Ok(embeddings[0].clone())
    }
    
    async fn embed_batch(&self, texts: &[&str]) -> crate::error::Result<Vec<Vec<f32>>> {
        let model = self.model.clone();
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        
        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            model.embed(text_refs, None)
        })
        .await
        .map_err(|e| crate::error::CisError::ai(format!("Embedding task failed: {}", e)))?
        .map_err(|e| crate::error::CisError::ai(format!("Embedding failed: {}", e)))?;
        
        Ok(embeddings)
    }
    
    fn dimension(&self) -> usize {
        self.dimension
    }
    
    fn model_info(&self) -> crate::error::Result<crate::traits::EmbeddingModelInfo> {
        Ok(crate::traits::EmbeddingModelInfo {
            name: "nomic-embed-text-v1.5".to_string(),
            dimension: self.dimension,
            max_input_length: 8192,
            description: "Nomic Embed Text v1.5 (local model)".to_string(),
            provider: "local".to_string(),
            supports_local: true,
        })
    }
    
    async fn health_check(&self) -> crate::error::Result<bool> {
        // 尝试简单的嵌入来验证服务健康
        match self.embed("test").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore = "Requires ONNX model download (~130MB)"]
    async fn test_embedding() {
        let service = EmbeddingService::new().await.unwrap();
        
        let embedding = service.embed("Hello world").await.unwrap();
        assert_eq!(embedding.len(), 768);
        
        // 相同文本应该产生相同嵌入
        let embedding2 = service.embed("Hello world").await.unwrap();
        assert_eq!(embedding, embedding2);
    }
    
    #[tokio::test]
    #[ignore = "Requires ONNX model download (~130MB)"]
    async fn test_batch_embedding() {
        let service = EmbeddingService::new().await.unwrap();
        
        let texts = vec!["Hello", "World", "Test"];
        let embeddings = service.embed_batch(&texts).await.unwrap();
        
        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].len(), 768);
    }
}
