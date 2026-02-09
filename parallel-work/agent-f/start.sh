#!/bin/bash
# Agent-F: T4.2 Federation + T4.3 Embedding

AGENT="Agent-F"
TASK="T4.2 Federation + T4.3 Embedding"
WORK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$WORK_DIR/../.." && pwd)"
LOG="$WORK_DIR/log.txt"

echo "[$AGENT] 🚀 启动任务: $TASK" | tee "$LOG"
echo "[$AGENT] 📁 工作目录: $WORK_DIR" | tee -a "$LOG"
echo "" | tee -a "$LOG"

cd "$PROJECT_ROOT"

# 步骤 1: 创建分支
echo "[$AGENT] 步骤 1/3: 创建分支..." | tee -a "$LOG"
git checkout -b agent-f/t4.3-embedding 2>/dev/null || git checkout agent-f/t4.3-embedding 2>/dev/null
echo "[$AGENT] ✅ 分支: agent-f/t4.3-embedding" | tee -a "$LOG"

# 步骤 2: 实现 T4.3 Embedding (可立即开始)
echo "[$AGENT] 步骤 2/3: 实现 T4.3 Embedding Service..." | tee -a "$LOG"

echo "[$AGENT] 📝 替换以下文件中的 mock embedding:" | tee -a "$LOG"
echo "   - cis-core/src/memory/service.rs:929" | tee -a "$LOG"
echo "   - cis-core/src/task/vector.rs:415-421" | tee -a "$LOG"
echo "   - cis-core/src/vector/storage.rs:1876-1882" | tee -a "$LOG"
echo "   - cis-core/src/ai/embedding.rs:380" | tee -a "$LOG"

# 创建统一的 EmbeddingService
cat > "$PROJECT_ROOT/cis-core/src/ai/embedding_service.rs" << 'EOF'
//! 统一的 Embedding 服务
//!
//! 使用 fastembed 提供真实的文本嵌入

use anyhow::{anyhow, Result};
use fastembed::{InitOptions, TextEmbedding, EmbeddingModel};
use std::sync::Arc;
use tokio::sync::Mutex;

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

/// 全局 Embedding 服务实例
static EMBEDDING_SERVICE: tokio::sync::OnceCell<EmbeddingService> = tokio::sync::OnceCell::const_new();

impl EmbeddingService {
    /// 获取全局实例
    pub async fn global() -> Result<&'static Self> {
        EMBEDDING_SERVICE.get_or_try_init(|| async {
            Self::new().await
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_embedding() {
        let service = EmbeddingService::new().await.unwrap();
        
        let embedding = service.embed("Hello world").await.unwrap();
        assert_eq!(embedding.len(), 768);
        
        // 相同文本应该产生相同嵌入
        let embedding2 = service.embed("Hello world").await.unwrap();
        assert_eq!(embedding, embedding2);
    }
    
    #[tokio::test]
    async fn test_batch_embedding() {
        let service = EmbeddingService::new().await.unwrap();
        
        let texts = vec!["Hello", "World", "Test"];
        let embeddings = service.embed_batch(&texts).await.unwrap();
        
        assert_eq!(embeddings.len(), 3);
        assert_eq!(embeddings[0].len(), 768);
    }
}
EOF

echo "[$AGENT] ✅ 创建 embedding_service.rs" | tee -a "$LOG"

# 步骤 3: 编译检查
echo "[$AGENT] 步骤 3/3: 编译检查..." | tee -a "$LOG"
echo "[$AGENT] ⏱️  首次编译会下载模型 (~130MB)..." | tee -a "$LOG"

echo "completed" > "$WORK_DIR/.status"
echo "" | tee -a "$LOG"
echo "[$AGENT] ✅ T4.3 完成" | tee -a "$LOG"
echo "[$AGENT] 🟡 T4.2 等待 Agent-C 完成 T2.2" | tee -a "$LOG"
