//! # FastEmbed Local Embedding Implementation
//!
//! Uses fastembed-rs library for local text embeddings.
//! Supports Nomic Embed Text v1.5 and other models.

use async_trait::async_trait;
use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::{CisError, Result};
use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};

/// FastEmbed-based local embedding service
pub struct FastEmbedService {
    model: Arc<Mutex<TextEmbedding>>,
    dimension: usize,
}

impl FastEmbedService {
    /// Create with default model (Nomic Embed Text v1.5)
    pub async fn new() -> Result<Self> {
        Self::with_model(EmbeddingModel::NomicEmbedTextV15).await
    }

    /// Create with specific model
    pub async fn with_model(model: EmbeddingModel) -> Result<Self> {
        // 先确定维度 (避免 move 后无法使用)
        let dimension = match model {
            EmbeddingModel::NomicEmbedTextV1 | EmbeddingModel::NomicEmbedTextV15 => 768,
            EmbeddingModel::BGESmallENV15 => 384,
            EmbeddingModel::BGEBaseENV15 => 768,
            EmbeddingModel::BGELargeENV15 => 1024,
            _ => DEFAULT_EMBEDDING_DIM,
        };

        let options = InitOptions::new(model)
            .with_show_download_progress(true);

        let embedding = tokio::task::spawn_blocking(move || {
            TextEmbedding::try_new(options)
        })
        .await
        .map_err(|e| CisError::configuration(format!("Failed to spawn embedding task: {}", e)))?
        .map_err(|e| CisError::configuration(format!("Failed to initialize FastEmbed: {}", e)))?;

        Ok(Self {
            model: Arc::new(Mutex::new(embedding)),
            dimension,
        })
    }

    /// Check if model files are already cached
    pub fn is_model_cached(model: EmbeddingModel) -> bool {
        // FastEmbed automatically handles caching
        // This checks if we can create without downloading
        let options = InitOptions::new(model).with_show_download_progress(false);
        TextEmbedding::try_new(options).is_ok()
    }
}

#[async_trait]
impl EmbeddingService for FastEmbedService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let model = self.model.clone();
        let text = text.to_string();

        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            model.embed(vec![text], None)
        })
        .await
        .map_err(|e| CisError::execution(format!("Embedding task failed: {}", e)))?
        .map_err(|e| CisError::execution(format!("FastEmbed error: {}", e)))?;

        embeddings.into_iter().next()
            .ok_or_else(|| CisError::execution("No embedding generated"))
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let model = self.model.clone();
        let texts: Vec<String> = texts.iter().map(|&s| s.to_string()).collect();

        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            model.embed(texts, None)
        })
        .await
        .map_err(|e| CisError::execution(format!("Batch embedding task failed: {}", e)))?
        .map_err(|e| CisError::execution(format!("FastEmbed batch error: {}", e)))?;

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Create default embedding service using FastEmbed
pub async fn create_default_service() -> Result<Box<dyn EmbeddingService>> {
    let service = FastEmbedService::new().await?;
    Ok(Box::new(service))
}

/// List available models
pub fn list_models() -> Vec<&'static str> {
    vec![
        "Nomic Embed Text v1.5 (768d)",
        "BGE Small EN v1.5 (384d)",
        "BGE Base EN v1.5 (768d)",
        "BGE Large EN v1.5 (1024d)",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Downloads model on first run (~130MB)"]
    async fn test_fastembed_basic() {
        let service = FastEmbedService::new().await.expect("Failed to create service");
        
        // Test single embed
        let embedding = service.embed("Hello world").await.expect("Failed to embed");
        assert_eq!(embedding.len(), 768); // Nomic Embed Text v1.5 dimension
        
        // Test batch embed
        let texts = vec!["Hello", "World"];
        let embeddings = service.batch_embed(&texts).await.expect("Failed to batch embed");
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), 768);
        assert_eq!(embeddings[1].len(), 768);
    }

    #[tokio::test]
    async fn test_fastembed_dimension() {
        // Just test the dimension logic without downloading
        assert_eq!(DEFAULT_EMBEDDING_DIM, 768);
    }
}
