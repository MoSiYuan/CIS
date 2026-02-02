//! # 批量处理器
//!
//! 提供异步批量向量索引功能，通过缓冲和批量处理提高性能。
//!
//! ## 功能
//!
//! - 自动批量化处理
//! - 背压控制
//! - 异步响应

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::{CisError, Result};
use super::storage::VectorStorage;

/// 批处理项
struct BatchItem {
    key: String,
    value: Vec<u8>,
    category: Option<String>,
    response_tx: oneshot::Sender<Result<String>>,
}

/// 批量处理器
///
/// 异步批量处理向量索引请求，通过缓冲提高吞吐量。
pub struct BatchProcessor {
    storage: Option<Arc<VectorStorage>>,
    batch_size: usize,
    tx: Option<mpsc::Sender<BatchItem>>,
    handle: Option<JoinHandle<()>>,
}

impl BatchProcessor {
    /// 创建新的批量处理器
    ///
    /// # Arguments
    /// * `storage` - 向量存储实例
    /// * `batch_size` - 每批处理的最大项数
    pub fn new(storage: Arc<VectorStorage>, batch_size: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<BatchItem>(1000);
        let storage_clone = Arc::clone(&storage);
        
        let handle = tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(batch_size);
            
            while let Some(item) = rx.recv().await {
                buffer.push(item);
                
                if buffer.len() >= batch_size {
                    Self::flush_batch(&storage_clone, &mut buffer).await;
                }
            }
            
            // 刷新剩余项
            if !buffer.is_empty() {
                Self::flush_batch(&storage_clone, &mut buffer).await;
            }
        });
        
        Self {
            storage: Some(storage),
            batch_size,
            tx: Some(tx),
            handle: Some(handle),
        }
    }
    
    /// 刷新批量缓冲区
    async fn flush_batch(storage: &Arc<VectorStorage>, batch: &mut Vec<BatchItem>) {
        // 提取所有需要的数据和发送者
        let items_and_senders: Vec<_> = batch.drain(..)
            .map(|item| ((item.key, item.value, item.category), item.response_tx))
            .collect();
        
        let items: Vec<_> = items_and_senders.iter()
            .map(|(item, _)| (item.0.clone(), item.1.clone(), item.2.clone()))
            .collect();
        
        match storage.batch_index_memory(items).await {
            Ok(ids) => {
                // 发送成功响应给每个等待者
                for (id, (_, sender)) in ids.into_iter().zip(items_and_senders.into_iter()) {
                    let _ = sender.send(Ok(id));
                }
            }
            Err(e) => {
                eprintln!("Batch processing error: {}", e);
                // 发送错误给所有等待者
                for (_, sender) in items_and_senders {
                    let _ = sender.send(Err(CisError::storage(format!("Batch processing error: {}", e))));
                }
            }
        }
    }
    
    /// 提交单个索引请求
    ///
    /// # Arguments
    /// * `key` - 记忆键
    /// * `value` - 记忆值
    /// * `category` - 可选类别
    ///
    /// # Returns
    /// 返回生成的记忆ID
    pub async fn submit(&self, key: String, value: Vec<u8>, category: Option<String>) -> Result<String> {
        let (response_tx, response_rx) = oneshot::channel();
        
        let tx = self.tx.as_ref().ok_or_else(|| CisError::Other("Batch processor closed".into()))?;
        tx.send(BatchItem {
            key,
            value,
            category,
            response_tx,
        }).await.map_err(|_| CisError::Other("Batch processor closed".into()))?;
        
        response_rx.await.map_err(|_| CisError::Other("Response cancelled".into()))?
    }
    
    /// 提交多个索引请求
    ///
    /// # Arguments
    /// * `items` - 要索引的项列表 (key, value, category)
    ///
    /// # Returns
    /// 返回所有生成的记忆ID
    pub async fn submit_batch(&self, items: Vec<(String, Vec<u8>, Option<String>)>) -> Result<Vec<String>> {
        let mut ids = Vec::with_capacity(items.len());
        
        for (key, value, category) in items {
            let id = self.submit(key, value, category).await?;
            ids.push(id);
        }
        
        Ok(ids)
    }
    
    /// 优雅关闭处理器
    ///
    /// 等待所有待处理项完成
    pub async fn shutdown(&mut self) -> Result<()> {
        // 关闭发送端以触发接收端完成
        self.tx.take();
        
        // 等待处理任务完成
        if let Some(handle) = self.handle.take() {
            handle.await.map_err(|e| CisError::Other(format!("Batch processor task failed: {}", e)))?;
        }
        
        // 释放存储引用
        self.storage.take();
        
        Ok(())
    }
    
    /// 获取批处理大小
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

impl Drop for BatchProcessor {
    fn drop(&mut self) {
        // 确保任务被清理
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use crate::error::Result;
    use async_trait::async_trait;

    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            // 简单的模拟向量
            let vec = vec![0.1f32; DEFAULT_EMBEDDING_DIM];
            Ok(vec)
        }

        async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            let mut results = Vec::with_capacity(texts.len());
            for _ in texts {
                results.push(vec![0.1f32; DEFAULT_EMBEDDING_DIM]);
            }
            Ok(results)
        }
    }

    #[tokio::test]
    async fn test_batch_processor() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = Arc::new(VectorStorage::open_with_service(&db_path, embedding).unwrap());
        
        let mut processor = BatchProcessor::new(storage, 2);
        
        // 提交几个项目
        let id1 = processor.submit("key1".to_string(), b"value1".to_vec(), Some("test".to_string())).await.unwrap();
        let id2 = processor.submit("key2".to_string(), b"value2".to_vec(), Some("test".to_string())).await.unwrap();
        let id3 = processor.submit("key3".to_string(), b"value3".to_vec(), Some("test".to_string())).await.unwrap();
        
        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
        assert!(!id3.is_empty());
        
        // 优雅关闭
        processor.shutdown().await.unwrap();
    }
}
