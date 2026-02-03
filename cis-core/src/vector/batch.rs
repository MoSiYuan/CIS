//! # BatchProcessor
//!
//! 提供异步批量向量索引功能，通过缓冲和批量处理提高性能。
//!
//! ## 功能
//!
//! - 自动批量化处理
//! - 背压控制
//! - 异步响应
//! - 并行处理支持
//! - 性能基准测试
//!
//! ## 性能目标
//!
//! - 1000 条数据 < 5s
//! - 平均每条 < 5ms
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::vector::batch::BatchProcessor;
//! use cis_core::vector::VectorStorage;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = Arc::new(VectorStorage::open_default()?);
//! let processor = BatchProcessor::new(storage, 10);
//!
//! // 提交索引请求
//! let id = processor.submit(
//!     "key".to_string(),
//!     b"value".to_vec(),
//!     Some("category".to_string())
//! ).await?;
//!
//! // 优雅关闭
//! processor.shutdown().await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use std::time::Instant;

use crate::error::{CisError, Result};
use super::storage::VectorStorage;

/// 批量处理统计
#[derive(Debug, Clone)]
pub struct BatchStats {
    /// 处理的总项数
    pub total_processed: usize,
    /// 成功数量
    pub success_count: usize,
    /// 失败数量
    pub failed_count: usize,
    /// 总耗时（毫秒）
    pub total_duration_ms: u64,
    /// 平均每项耗时（毫秒）
    pub avg_duration_per_item_ms: f64,
}

impl BatchStats {
    /// 创建新的统计信息
    pub fn new() -> Self {
        Self {
            total_processed: 0,
            success_count: 0,
            failed_count: 0,
            total_duration_ms: 0,
            avg_duration_per_item_ms: 0.0,
        }
    }
    
    /// 更新统计信息
    pub fn update(&mut self, items: usize, success: usize, failed: usize, duration_ms: u64) {
        self.total_processed += items;
        self.success_count += success;
        self.failed_count += failed;
        self.total_duration_ms += duration_ms;
        if self.total_processed > 0 {
            self.avg_duration_per_item_ms = self.total_duration_ms as f64 / self.total_processed as f64;
        }
    }
}

impl Default for BatchStats {
    fn default() -> Self {
        Self::new()
    }
}

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
///
/// ## 工作原理
///
/// 1. 接收索引请求并放入缓冲区
/// 2. 当缓冲区达到 batch_size 时触发批量处理
/// 3. 使用批量嵌入 API 提高效率
/// 4. 返回结果给等待的请求
///
/// ## 线程安全
///
/// `BatchProcessor` 可以安全地在多个任务间共享。内部使用 channel 进行通信。
///
/// ## 示例
///
/// ```rust,no_run
/// use cis_core::vector::batch::BatchProcessor;
/// use cis_core::vector::VectorStorage;
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let storage = Arc::new(VectorStorage::open_default()?);
/// let processor = BatchProcessor::new(storage, 10);
///
/// // 批量提交
/// let items = vec![
///     ("key1".to_string(), b"value1".to_vec(), Some("cat1".to_string())),
///     ("key2".to_string(), b"value2".to_vec(), Some("cat2".to_string())),
/// ];
/// let ids = processor.submit_batch(items).await?;
///
/// // 查看统计
/// let stats = processor.stats();
/// println!("处理: {}, 成功: {}", stats.total_processed, stats.success_count);
///
/// // 关闭
/// processor.shutdown().await?;
/// # Ok(())
/// # }
/// ```
pub struct BatchProcessor {
    storage: Option<Arc<VectorStorage>>,
    batch_size: usize,
    tx: Option<mpsc::Sender<BatchItem>>,
    handle: Option<JoinHandle<()>>,
    stats: std::sync::Arc<std::sync::Mutex<BatchStats>>,
}

impl BatchProcessor {
    /// 创建新的批量处理器
    ///
    /// # 参数
    /// - `storage`: 向量存储实例
    /// - `batch_size`: 每批处理的最大项数（建议 10-100）
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::batch::BatchProcessor;
    /// use cis_core::vector::VectorStorage;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let storage = Arc::new(VectorStorage::open_default()?);
    /// let processor = BatchProcessor::new(storage, 50);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(storage: Arc<VectorStorage>, batch_size: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<BatchItem>(1000);
        let storage_clone = Arc::clone(&storage);
        let stats = std::sync::Arc::new(std::sync::Mutex::new(BatchStats::new()));
        let stats_clone = Arc::clone(&stats);
        
        let handle = tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(batch_size);
            
            while let Some(item) = rx.recv().await {
                buffer.push(item);
                
                if buffer.len() >= batch_size {
                    let start = Instant::now();
                    let count = buffer.len();
                    let (success, failed) = Self::flush_batch(&storage_clone, &mut buffer).await;
                    let duration = start.elapsed().as_millis() as u64;
                    
                    if let Ok(mut s) = stats_clone.lock() {
                        s.update(count, success, failed, duration);
                    }
                }
            }
            
            // 刷新剩余项
            if !buffer.is_empty() {
                let start = Instant::now();
                let count = buffer.len();
                let (success, failed) = Self::flush_batch(&storage_clone, &mut buffer).await;
                let duration = start.elapsed().as_millis() as u64;
                
                if let Ok(mut s) = stats_clone.lock() {
                    s.update(count, success, failed, duration);
                }
            }
        });
        
        Self {
            storage: Some(storage),
            batch_size,
            tx: Some(tx),
            handle: Some(handle),
            stats,
        }
    }
    
    /// 刷新批量缓冲区
    /// 
    /// 返回 (成功数, 失败数)
    async fn flush_batch(storage: &Arc<VectorStorage>, batch: &mut Vec<BatchItem>) -> (usize, usize) {
        // 提取所有需要的数据和发送者
        let items_and_senders: Vec<_> = batch.drain(..)
            .map(|item| ((item.key, item.value, item.category), item.response_tx))
            .collect();
        
        let items: Vec<_> = items_and_senders.iter()
            .map(|(item, _)| (item.0.clone(), item.1.clone(), item.2.clone()))
            .collect();
        
        let total = items_and_senders.len();
        
        match storage.batch_index_memory(items).await {
            Ok(ids) => {
                let success_count = ids.len();
                // 发送成功响应给每个等待者
                for (id, (_, sender)) in ids.into_iter().zip(items_and_senders.into_iter()) {
                    let _ = sender.send(Ok(id));
                }
                (success_count, total - success_count)
            }
            Err(e) => {
                tracing::error!("Batch processing error: {}", e);
                // 发送错误给所有等待者
                for (_, sender) in items_and_senders {
                    let _ = sender.send(Err(CisError::storage(format!("Batch processing error: {}", e))));
                }
                (0, total)
            }
        }
    }
    
    /// 提交单个索引请求
    ///
    /// 将索引请求放入队列，等待批量处理。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `category`: 可选类别
    ///
    /// # 返回
    /// - `Result<String>`: 生成的记忆 ID
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::batch::BatchProcessor;
    ///
    /// # async fn example(processor: &BatchProcessor) -> anyhow::Result<()> {
    /// let id = processor.submit(
    ///     "user/pref".to_string(),
    ///     b"dark mode".to_vec(),
    ///     Some("preferences".to_string())
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
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
    /// 等待所有待处理项完成，然后关闭处理器。
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::batch::BatchProcessor;
    ///
    /// # async fn example(mut processor: BatchProcessor) -> anyhow::Result<()> {
    /// // 提交一些请求...
    ///
    /// // 优雅关闭，等待所有请求完成
    /// processor.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
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
    
    /// 获取处理统计信息
    pub fn stats(&self) -> BatchStats {
        self.stats.lock().unwrap().clone()
    }
    
    /// 运行性能基准测试
    ///
    /// 测试批量处理性能并返回统计信息。
    ///
    /// # 参数
    /// - `storage`: 向量存储实例
    /// - `item_count`: 要测试的项数
    /// - `batch_size`: 每批处理大小
    ///
    /// # 返回
    /// - `Result<BatchStats>`: 性能统计信息
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::batch::BatchProcessor;
    /// use cis_core::vector::VectorStorage;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = Arc::new(VectorStorage::open_default()?);
    /// let stats = BatchProcessor::benchmark(storage, 1000, 50).await?;
    ///
    /// println!("总处理: {}", stats.total_processed);
    /// println!("成功: {}", stats.success_count);
    /// println!("平均耗时: {:.2} ms/项", stats.avg_duration_per_item_ms);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn benchmark(
        storage: Arc<VectorStorage>,
        item_count: usize,
        batch_size: usize,
    ) -> Result<BatchStats> {
        let mut processor = Self::new(storage, batch_size);
        let start = Instant::now();
        
        // 生成测试数据
        let mut items = Vec::with_capacity(item_count);
        for i in 0..item_count {
            let key = format!("bench_key_{}", i);
            let value = format!("Benchmark test value number {} with some content to simulate real data", i);
            items.push((key, value.into_bytes(), Some("benchmark".to_string())));
        }
        
        // 提交所有项目
        for (key, value, category) in items {
            processor.submit(key, value, category).await?;
        }
        
        // 关闭处理器并等待完成
        processor.shutdown().await?;
        
        let total_duration = start.elapsed();
        let stats = processor.stats();
        
        tracing::info!(
            "Batch benchmark completed: {} items in {}ms (avg: {:.2}ms/item, throughput: {:.2} items/sec)",
            item_count,
            total_duration.as_millis(),
            total_duration.as_millis() as f64 / item_count as f64,
            item_count as f64 / total_duration.as_secs_f64()
        );
        
        Ok(stats)
    }
    
    /// 执行搜索性能基准测试
    /// 
    /// # Arguments
    /// * `storage` - 向量存储实例
    /// * `query_count` - 查询次数
    /// * `limit` - 每次查询返回的最大结果数
    /// 
    /// # Returns
    /// 返回平均查询延迟（毫秒）
    pub async fn benchmark_search(
        storage: Arc<VectorStorage>,
        query_count: usize,
        limit: usize,
    ) -> Result<f64> {
        let queries: Vec<_> = (0..query_count)
            .map(|i| format!("search query {} benchmark test", i))
            .collect();
        
        let start = Instant::now();
        
        for query in &queries {
            let _ = storage.search_memory(query, limit, None).await?;
        }
        
        let total_duration = start.elapsed();
        let avg_latency = total_duration.as_millis() as f64 / query_count as f64;
        
        tracing::info!(
            "Search benchmark completed: {} queries in {}ms (avg latency: {:.2}ms)",
            query_count,
            total_duration.as_millis(),
            avg_latency
        );
        
        Ok(avg_latency)
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
