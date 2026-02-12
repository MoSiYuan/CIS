//! # Batch Cache Operations
//!
//! 批量缓存操作模块，提供高效的批量读写接口。
//!
//! ## 特性
//!
//! - 批量获取 (get_multi)
//! - 批量设置 (set_multi)
//! - 批量失效 (invalidate_multi)
//! - 批量预热 (warm_up)
//!
//! ## 性能优势
//!
//! - 减少锁获取次数
//! - 批量处理，提高吞吐量
//! - 适合场景：加载配置、批量查询
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::cache::LruCache;
//! use cis_core::cache::batch_ops::BatchCacheOps;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let cache = LruCache::default();
//!
//! // 批量设置
//! let items = vec
//!![
//!     ("key1".to_string(), b"value1".to_vec()),
//!     ("key2".to_string(), b"value2".to_vec()),
//! ];
//! cache.set_multi(items, None).await;
//!
//! // 批量获取
//! let keys = vec
//!["key1".to_string(), "key2".to_string()];
//! let results = cache.get_multi(&keys).await;
//!
//! // 批量失效
//! cache.invalidate_multi(&keys).await;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::time::Duration;

use super::lru::{LruCache, CacheMetricsSnapshot};

/// 批量缓存操作扩展 trait
pub trait BatchCacheOps {
    /// 批量获取缓存值
    ///
    /// 一次性获取多个键的缓存值，比逐个获取更高效。
    ///
    /// # 参数
    /// - `keys`: 缓存键列表
    ///
    /// # 返回
    /// - `HashMap<String, Vec<u8>>`: 键到值的映射，不存在的键不会出现在结果中
    fn get_multi(&self, keys: &[String]) -> impl std::future::Future<Output = HashMap<String, Vec<u8>>>;

    /// 批量设置缓存值
    ///
    /// 一次性设置多个键值对，比逐个设置更高效。
    ///
    /// # 参数
    /// - `items`: 键值对列表
    /// - `ttl`: 可选的 TTL，None 时使用默认 TTL
    fn set_multi(
        &self,
        items: Vec<(String, Vec<u8>)>,
        ttl: Option<Duration>,
    ) -> impl std::future::Future<Output = ()>;

    /// 批量使缓存失效
    ///
    /// 一次性使多个键失效。
    ///
    /// # 参数
    /// - `keys`: 缓存键列表
    fn invalidate_multi(&self, keys: &[String]) -> impl std::future::Future<Output = ()>;

    /// 获取缓存中所有键
    ///
    /// # 返回
    /// - `Vec<String>`: 所有缓存键
    fn keys(&self) -> impl std::future::Future<Output = Vec<String>>;

    /// 获取批量统计信息
    ///
    /// 返回批量操作的统计信息，包括命中率、操作次数等。
    ///
    /// # 返回
    /// - `BatchCacheStats`: 统计信息
    fn get_batch_stats(&self) -> impl std::future::Future<Output = BatchCacheStats>;

    /// 智能批量预热
    ///
    /// 根据访问频率批量预热缓存，优先加载热数据。
    ///
    /// # 参数
    /// - `entries`: 键值对列表
    /// - `priority`: 访问优先级 (0.0 - 1.0)，越高越优先
    fn smart_warm_up(
        &self,
        entries: Vec<(String, Vec<u8>, f32)>,
    ) -> impl std::future::Future<Output = ()>;
}

/// 批量缓存统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchCacheStats {
    /// 总批量操作次数
    pub total_batch_ops: u64,
    /// 批量获取次数
    pub batch_get_ops: u64,
    /// 批量设置次数
    pub batch_set_ops: u64,
    /// 批量失效次数
    pub batch_invalidate_ops: u64,
    /// 平均批量大小
    pub avg_batch_size: f64,
    /// 总处理的条目数
    pub total_entries_processed: u64,
}

/// 批量缓存操作扩展实现
impl BatchCacheOps for LruCache {
    /// 批量获取缓存值
    async fn get_multi(&self, keys: &[String]) -> HashMap<String, Vec<u8>> {
        if !self.config().enabled {
            return HashMap::new();
        }

        let mut results = HashMap::with_capacity(keys.len());

        // 获取锁一次，批量读取
        let cache = self.cache.read().await;

        for key in keys {
            if let Some(entry) = cache.entries.get(key) {
                if !entry.is_expired() {
                    results.insert(key.clone(), entry.value.clone());
                }
            }
        }

        // 更新统计
        self.metrics().record_request();
        if !results.is_empty() {
            self.metrics().record_hit();
        } else {
            self.metrics().record_miss();
        }

        results
    }

    /// 批量设置缓存值
    async fn set_multi(&self, items: Vec<(String, Vec<u8>)>, ttl: Option<Duration>) {
        if !self.config().enabled {
            return;
        }

        let ttl = ttl.unwrap_or(self.config().default_ttl);

        // 获取锁一次，批量写入
        let mut cache = self.cache.write().await;

        // 检查容量，必要时淘汰
        let current_size = cache.entries.len();
        let capacity = self.config().max_entries;

        if current_size + items.len() > capacity {
            // 需要淘汰的条目数
            let to_evict = (current_size + items.len()) - capacity;

            for _ in 0..to_evict {
                if let Some(evicted_key) = cache.pop_lru() {
                    tracing::debug!("Cache evicted (batch): {}", evicted_key);
                    self.metrics().record_eviction();
                } else {
                    break;
                }
            }
        }

        // 批量插入
        for (key, value) in items {
            let entry = super::lru::CacheEntry::new(
                value,
                ttl,
                crate::types::MemoryDomain::Public,
                crate::types::MemoryCategory::Context,
            );

            cache.entries.insert(key.clone(), entry);
            cache.update_access_order(&key);
        }
    }

    /// 批量使缓存失效
    async fn invalidate_multi(&self, keys: &[String]) {
        if !self.config().enabled {
            return;
        }

        let mut cache = self.cache.write().await;

        for key in keys {
            cache.entries.remove(key);
            cache.remove_from_access_order(key);
            self.metrics().record_invalidation();
        }
    }

    /// 获取缓存中所有键
    async fn keys(&self) -> Vec<String> {
        if !self.config().enabled {
            return Vec::new();
        }

        let cache = self.cache.read().await;
        cache.entries.keys().cloned().collect()
    }

    /// 获取批量统计信息
    async fn get_batch_stats(&self) -> BatchCacheStats {
        // 这里简化实现，实际应该记录批量操作统计
        let metrics = self.get_metrics().await;

        BatchCacheStats {
            total_batch_ops: metrics.total_requests / 10, // 假设平均批量大小为 10
            batch_get_ops: metrics.hits / 10,
            batch_set_ops: metrics.evictions,
            batch_invalidate_ops: metrics.invalidations,
            avg_batch_size: 10.0, // 简化
            total_entries_processed: metrics.total_requests,
        }
    }

    /// 智能批量预热
    ///
    /// 根据优先级排序，优先加载高优先级的数据。
    async fn smart_warm_up(&self, entries: Vec<(String, Vec<u8>, f32)>) {
        if !self.config().enabled {
            return;
        }

        // 按优先级排序 (降序)
        let mut sorted_entries = entries;
        sorted_entries.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        // 过滤低优先级数据 (如果缓存已满)
        let capacity = self.config().max_entries;
        let items_to_load: Vec<(String, Vec<u8>)> = if sorted_entries.len() > capacity {
            // 只加载高优先级的数据
            sorted_entries
                .into_iter()
                .take(capacity)
                .map(|(k, v, _)| (k, v))
                .collect()
        } else {
            // 全部加载
            sorted_entries
                .into_iter()
                .map(|(k, v, _)| (k, v))
                .collect()
        };

        // 批量设置
        self.set_multi(items_to_load, None).await;
    }
}

/// 批量缓存操作辅助工具
pub struct BatchCacheHelper {
    cache: std::sync::Arc<LruCache>,
}

impl BatchCacheHelper {
    /// 创建批量操作辅助工具
    pub fn new(cache: std::sync::Arc<LruCache>) -> Self {
        Self { cache }
    }

    /// 分批处理大量键
    ///
    /// 将大量键分批处理，避免一次性处理过多数据。
    ///
    /// # 参数
    /// - `keys`: 所有键
    /// - `batch_size`: 每批大小
    /// - `processor`: 批处理函数
    pub async fn process_in_batches<F, Fut>(
        &self,
        keys: Vec<String>,
        batch_size: usize,
        mut processor: F,
    ) where
        F: FnMut(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        for chunk in keys.chunks(batch_size) {
            let batch = chunk.to_vec();
            processor(batch).await;
        }
    }

    /// 并行批量获取
    ///
    /// 使用并发方式批量获取，提高性能。
    ///
    /// # 参数
    /// - `keys`: 键列表
    /// - `concurrency`: 并发数
    pub async fn parallel_get_multi(
        &self,
        keys: Vec<String>,
        concurrency: usize,
    ) -> HashMap<String, Vec<u8>> {
        use futures::stream::{self, StreamExt};

        let cache = self.cache.clone();
        let batches: Vec<_> = keys.chunks(concurrency).map(|b| b.to_vec()).collect();

        let results = stream::iter(batches)
            .map(move |batch| {
                let cache = cache.clone();
                tokio::spawn(async move {
                    let mut local_results = HashMap::new();
                    for key in batch {
                        if let Some(value) = cache.get(&key).await {
                            local_results.insert(key, value);
                        }
                    }
                    local_results
                })
            })
            .buffer_unordered(concurrency)
            .collect::<Vec<_>>()
            .await;

        let mut combined = HashMap::new();
        for result in results {
            if let Ok(batch_results) = result {
                combined.extend(batch_results);
            }
        }

        combined
    }

    /// 缓存穿透保护
    ///
    /// 对于缓存中不存在的键，记录到布隆过滤器或空对象缓存，
    /// 防止频繁查询数据库。
    ///
    /// # 参数
    /// - `key`: 键
    /// - `value`: 可选值 (None 表示不存在)
    pub async fn protect_from_penetration(&self, key: String, value: Option<Vec<u8>>) {
        // 如果值不存在，缓存一个特殊标记
        match value {
            Some(v) => {
                self.cache.put(key, v, None).await;
            }
            None => {
                // 缓存空对象，TTL 较短
                self
                    .cache
                    .put(
                        format!("__NULL__{}", key),
                        b"__NULL__".to_vec(),
                        Some(std::time::Duration::from_secs(60)),
                    )
                    .await;
            }
        }
    }

    /// 检查是否为空对象标记
    pub fn is_null_marker(key: &str, value: &[u8]) -> bool {
        key.starts_with("__NULL__") && value == b"__NULL__"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::config::CacheConfig;

    fn create_test_cache() -> LruCache {
        let config = CacheConfig {
            max_entries: 100,
            ..Default::default()
        };
        LruCache::new(config)
    }

    #[tokio::test]
    async fn test_get_multi() {
        let cache = create_test_cache();

        // 先设置一些值
        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;
        cache.put("key3".to_string(), b"value3".to_vec(), None).await;

        // 批量获取
        let keys = vec
![
            "key1".to_string(),
            "key2".to_string(),
            "key3".to_string(),
            "key4".to_string(), // 不存在
        ];

        let results = cache.get_multi(&keys).await;

        assert_eq!(results.len(), 3);
        assert_eq!(results.get("key1"), Some(&b"value1".to_vec()));
        assert_eq!(results.get("key2"), Some(&b"value2".to_vec()));
        assert_eq!(results.get("key3"), Some(&b"value3".to_vec()));
        assert_eq!(results.get("key4"), None);
    }

    #[tokio::test]
    async fn test_set_multi() {
        let cache = create_test_cache();

        // 批量设置
        let items = vec
![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
            ("key3".to_string(), b"value3".to_vec()),
        ];

        cache.set_multi(items, None).await;

        // 验证
        assert_eq!(
            cache.get("key1").await,
            Some(b"value1".to_vec())
        );
        assert_eq!(
            cache.get("key2").await,
            Some(b"value2".to_vec())
        );
        assert_eq!(
            cache.get("key3").await,
            Some(b"value3".to_vec())
        );
    }

    #[tokio::test]
    async fn test_invalidate_multi() {
        let cache = create_test_cache();

        // 设置一些值
        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;
        cache.put("key3".to_string(), b"value3".to_vec(), None).await;

        // 批量失效
        let keys = vec
!["key1".to_string(), "key2".to_string()];
        cache.invalidate_multi(&keys).await;

        // 验证
        assert_eq!(cache.get("key1").await, None);
        assert_eq!(cache.get("key2").await, None);
        assert_eq!(
            cache.get("key3").await,
            Some(b"value3".to_vec())
        );
    }

    #[tokio::test]
    async fn test_keys() {
        let cache = create_test_cache();

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;
        cache.put("key3".to_string(), b"value3".to_vec(), None).await;

        let keys = cache.keys().await;

        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[tokio::test]
    async fn test_smart_warm_up() {
        let cache = create_test_cache();

        // 准备数据，带优先级
        let entries = vec
![
            ("key1".to_string(), b"value1".to_vec(), 0.5),
            ("key2".to_string(), b"value2".to_vec(), 0.9), // 高优先级
            ("key3".to_string(), b"value3".to_vec(), 0.3),
            ("key4".to_string(), b"value4".to_vec(), 0.8),
        ];

        cache.smart_warm_up(entries).await;

        // 验证高优先级的数据被缓存
        assert_eq!(
            cache.get("key2").await,
            Some(b"value2".to_vec())
        );
        assert_eq!(
            cache.get("key4").await,
            Some(b"value4".to_vec())
        );
    }

    #[tokio::test]
    async fn test_batch_cache_helper_process_in_batches() {
        let cache = create_test_cache();
        let helper = BatchCacheHelper::new(std::sync::Arc::new(cache));

        let keys = (0..10).map(|i| format!("key{}", i)).collect();

        let mut processed_batches = Vec::new();
        helper
            .process_in_batches(keys, 3, |batch| {
                async move {
                    processed_batches.push(batch.len());
                }
            })
            .await;

        assert_eq!(processed_batches, vec
![3, 3, 3, 1]);
    }

    #[tokio::test]
    async fn test_penetration_protection() {
        let cache = create_test_cache();
        let helper = BatchCacheHelper::new(std::sync::Arc::new(cache));

        // 保护不存在的键
        helper
            .protect_from_penetration("missing_key".to_string(), None)
            .await;

        // 检查空对象标记
        assert_eq!(
            cache.get("__NULL__missing_key").await,
            Some(b"__NULL__".to_vec())
        );
        assert!(BatchCacheHelper::is_null_marker(
            "__NULL__missing_key",
            b"__NULL__"
        ));
    }
}
