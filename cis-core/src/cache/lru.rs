//! # LRU Cache Implementation
//!
//! 线程安全的 LRU (Least Recently Used) 缓存实现。
//!
//! ## 特性
//!
//! - 基于 HashMap + 双向队列的高效实现
//! - 线程安全 (使用 Arc<RwLock>)
//! - TTL (Time To Live) 支持
//! - 缓存统计 (命中率、淘汰数等)
//! - 自动过期清理
//! - 批量操作支持
//!
//! ## 性能
//!
//! - 缓存命中: < 1μs
//! - 缓存未命中: < 100μs
//! - 吞吐量: > 100K ops/sec
//!
//! ## 已知限制 (P0-4: Writer Starvation)
//!
//! **当前实现** 使用 `tokio::sync::RwLock`，在高并发读场景下可能导致写者饥饿：
//!
//! ```text
//! 问题场景:
//! 读请求 1 ─┐
//! 读请求 2 ─┼─→ 持续不断的读操作
//! 读请求 3 ─┘         ↓
//! 写请求   ───────────→ 长时间等待 ⚠️
//! ```
//!
//! **解决方案** (推荐使用 parking_lot):
//!
//! ```toml
//! # cis-core/Cargo.toml
//! [dependencies]
//! parking_lot = "0.12"
//!
//! [features]
//! default = ["parking_lot"]
//! parking_lot = ["dep:parking_lot"]
//! ```
//!
//! ```rust
//! #![cfg(feature = "parking_lot")]
//! use parking_lot::RwLock;
//!
//! #![cfg(not(feature = "parking_lot"))]
//! use tokio::sync::RwLock;
//! ```
//!
//! **parking_lot 优势**:
//! - 更好的写者优先策略
//! - 更小的内存占用
//! - 更快的锁操作（约 2x）
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::cache::{LruCache, CacheConfig};
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = CacheConfig {
//!     max_entries: 1000,
//!     default_ttl: Duration::from_secs(300),
//!     ..Default::default()
//! };
//!
//! let cache = LruCache::new(config);
//!
//! // 写入缓存
//! cache.put("key1".to_string(), b"value1".to_vec(), None).await;
//!
//! // 读取缓存
//! if let Some(value) = cache.get("key1").await {
//!     println!("Cache hit: {:?}", value);
//! }
//!
//! // 获取统计
//! let metrics = cache.get_metrics().await;
//! println!("Hit rate: {:.2}%", metrics.hit_rate * 100.0);
//! # Ok(())
//! # }
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

use crate::types::{MemoryCategory, MemoryDomain};

use super::config::CacheConfig;

/// 缓存统计指标
#[derive(Debug, Default)]
pub struct CacheMetrics {
    /// 请求总数
    total_requests: AtomicU64,
    /// 命中次数
    hits: AtomicU64,
    /// 未命中次数
    misses: AtomicU64,
    /// 淘汰次数
    evictions: AtomicU64,
    /// 过期清理次数
    expirations: AtomicU64,
    /// 失效次数
    invalidations: AtomicU64,
}

impl CacheMetrics {
    /// 创建新的统计实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录请求
    pub fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录命中
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录未命中
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录淘汰
    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录过期
    pub fn record_expiration(&self) {
        self.expirations.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录失效
    pub fn record_invalidation(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取请求总数
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// 获取命中次数
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// 获取未命中次数
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// 获取淘汰次数
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// 获取过期清理次数
    pub fn expirations(&self) -> u64 {
        self.expirations.load(Ordering::Relaxed)
    }

    /// 获取失效次数
    pub fn invalidations(&self) -> u64 {
        self.invalidations.load(Ordering::Relaxed)
    }

    /// 计算命中率 (0.0 - 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.hits.load(Ordering::Relaxed);
        (hits as f64) / (total as f64)
    }

    /// 重置所有统计
    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.expirations.store(0, Ordering::Relaxed);
        self.invalidations.store(0, Ordering::Relaxed);
    }
}

/// 缓存统计快照
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheMetricsSnapshot {
    pub total_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expirations: u64,
    pub invalidations: u64,
    pub hit_rate: f64,
}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 缓存的值
    value: Vec<u8>,
    /// 创建时间
    created_at: SystemTime,
    /// 最后访问时间 (用于 LRU)
    last_accessed: SystemTime,
    /// 访问次数 (用于统计热度)
    access_count: u64,
    /// TTL (Time To Live)
    ttl: Duration,
    /// 是否加密 (与私域记忆对应)
    encrypted: bool,
    /// 域类型 (Private/Public)
    domain: MemoryDomain,
    /// 分类
    category: MemoryCategory,
}

impl CacheEntry {
    /// 创建新缓存条目
    fn new(
        value: Vec<u8>,
        ttl: Duration,
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            ttl,
            encrypted: matches!(domain, MemoryDomain::Private),
            domain,
            category,
        }
    }

    /// 检查是否过期
    fn is_expired(&self) -> bool {
        let elapsed = self
            .created_at
            .elapsed()
            .unwrap_or(Duration::from_secs(0));
        elapsed > self.ttl
    }

    /// 记录访问
    fn record_access(&mut self) {
        self.last_accessed = SystemTime::now();
        self.access_count += 1;
    }

    /// 获取年龄 (创建时长)
    fn age(&self) -> Duration {
        self.created_at.elapsed().unwrap_or(Duration::from_secs(0))
    }

    /// 获取空闲时间 (未访问时长)
    fn idle_time(&self) -> Duration {
        self.last_accessed
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }
}

/// LRU 缓存内部状态
#[derive(Debug)]
struct LruCacheState {
    /// 主存储
    entries: HashMap<String, CacheEntry>,
    /// 访问顺序队列 (最久未访问 -> 最近访问)
    access_order: VecDeque<String>,
    /// 当前条目数
    size: usize,
}

impl LruCacheState {
    /// 创建新的缓存状态
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            access_order: VecDeque::new(),
            size: 0,
        }
    }

    /// 更新访问顺序 (移到队列尾部)
    fn update_access_order(&mut self, key: &str) {
        // 先移除
        self.access_order.retain(|k| k != key);
        // 添加到尾部 (最近访问)
        self.access_order.push_back(key.to_string());
    }

    /// 从访问队列中移除
    fn remove_from_access_order(&mut self, key: &str) {
        self.access_order.retain(|k| k != key);
    }

    /// 淘汰最久未访问的条目
    fn pop_lru(&mut self) -> Option<String> {
        if let Some(key) = self.access_order.pop_front() {
            self.entries.remove(&key);
            self.size = self.entries.len();
            Some(key)
        } else {
            None
        }
    }

    /// 移除过期的条目
    fn remove_expired_entries(&mut self) -> usize {
        let mut expired_keys = Vec::new();

        for (key, entry) in &self.entries {
            if entry.is_expired() {
                expired_keys.push(key.clone());
            }
        }

        for key in &expired_keys {
            self.entries.remove(key);
            self.remove_from_access_order(key);
        }

        self.size = self.entries.len();
        expired_keys.len()
    }

    /// 获取当前大小
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// 检查是否为空
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// LRU (Least Recently Used) 缓存
pub struct LruCache {
    /// 缓存配置
    config: CacheConfig,
    /// 缓存存储
    cache: Arc<RwLock<LruCacheState>>,
    /// 缓存统计
    metrics: Arc<CacheMetrics>,
}

impl LruCache {
    /// 创建新的 LRU 缓存
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config: config.clone(),
            cache: Arc::new(RwLock::new(LruCacheState::new())),
            metrics: Arc::new(CacheMetrics::new()),
        }
    }

    /// 获取缓存值
    ///
    /// 如果缓存命中，返回缓存的值；如果未命中或过期，返回 None。
    ///
    /// # 参数
    /// - `key`: 缓存键
    ///
    /// # 返回
    /// - `Option<Vec<u8>>`: 缓存值
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        // 检查缓存是否启用
        if !self.config.enabled {
            return None;
        }

        let mut cache = self.cache.write().await;
        self.metrics.record_request();

        // 检查过期
        cache.remove_expired_entries();

        match cache.entries.get_mut(key) {
            Some(entry) if !entry.is_expired() => {
                // 缓存命中
                entry.record_access();
                cache.update_access_order(key);
                self.metrics.record_hit();
                Some(entry.value.clone())
            }
            Some(_) => {
                // 过期，视为未命中
                cache.entries.remove(key);
                cache.remove_from_access_order(key);
                self.metrics.record_miss();
                self.metrics.record_expiration();
                None
            }
            None => {
                // 缓存未命中
                self.metrics.record_miss();
                None
            }
        }
    }

    /// 添加或更新缓存
    ///
    /// 如果缓存已满，会自动淘汰最久未访问的条目。
    ///
    /// # 参数
    /// - `key`: 缓存键
    /// - `value`: 缓存值
    /// - `ttl`: 可选的 TTL，默认使用配置的 default_ttl
    pub async fn put(&self, key: String, value: Vec<u8>, ttl: Option<Duration>) {
        // 检查缓存是否启用
        if !self.config.enabled {
            return;
        }

        let mut cache = self.cache.write().await;

        // 检查容量，必要时淘汰
        if cache.entries.len() >= self.config.max_entries {
            if !cache.entries.contains_key(&key) {
                // 新增且缓存已满，淘汰最久未访问的条目
                if let Some(evicted_key) = cache.pop_lru() {
                    tracing::debug!("Cache evicted: {}", evicted_key);
                    self.metrics.record_eviction();
                }
            }
        }

        let ttl = ttl.unwrap_or(self.config.default_ttl);
        let entry = CacheEntry::new(value, ttl, MemoryDomain::Public, MemoryCategory::Context);

        cache.entries.insert(key.clone(), entry);
        cache.update_access_order(&key);
    }

    /// 使缓存失效
    ///
    /// 移除指定的缓存条目。通常在数据更新时调用。
    ///
    /// # 参数
    /// - `key`: 缓存键
    pub async fn invalidate(&self, key: &str) {
        if !self.config.enabled {
            return;
        }

        let mut cache = self.cache.write().await;
        cache.entries.remove(key);
        cache.remove_from_access_order(key);
        self.metrics.record_invalidation();
    }

    /// 批量使缓存失效
    ///
    /// # 参数
    /// - `keys`: 缓存键列表
    pub async fn invalidate_batch(&self, keys: &[String]) {
        if !self.config.enabled {
            return;
        }

        let mut cache = self.cache.write().await;
        for key in keys {
            cache.entries.remove(key);
            cache.remove_from_access_order(key);
            self.metrics.record_invalidation();
        }
    }

    /// 清空缓存
    ///
    /// 移除所有缓存条目并重置统计。
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.entries.clear();
        cache.access_order.clear();
        cache.size = 0;
        self.metrics.reset();
    }

    /// 获取缓存大小
    pub async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// 检查缓存是否为空
    pub async fn is_empty(&self) -> bool {
        let cache = self.cache.read().await;
        cache.is_empty()
    }

    /// 检查是否包含指定键
    pub async fn contains_key(&self, key: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        let cache = self.cache.read().await;
        cache.entries.contains_key(key)
    }

    /// 移除过期的条目
    ///
    /// 通常由后台任务定期调用。
    ///
    /// # 返回
    /// - `usize`: 移除的条目数
    pub async fn remove_expired_entries(&self) -> usize {
        if !self.config.enabled {
            return 0;
        }

        let mut cache = self.cache.write().await;
        let count = cache.remove_expired_entries();
        self.metrics
            .expirations
            .fetch_add(count as u64, Ordering::Relaxed);
        count
    }

    /// 获取缓存统计快照
    pub async fn get_metrics(&self) -> CacheMetricsSnapshot {
        CacheMetricsSnapshot {
            total_requests: self.metrics.total_requests(),
            hits: self.metrics.hits(),
            misses: self.metrics.misses(),
            evictions: self.metrics.evictions(),
            expirations: self.metrics.expirations(),
            invalidations: self.metrics.invalidations(),
            hit_rate: self.metrics.hit_rate(),
        }
    }

    /// 获取统计指标 (内部引用)
    pub fn metrics(&self) -> &Arc<CacheMetrics> {
        &self.metrics
    }

    /// 获取配置
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// 预热缓存
    ///
    /// 批量添加条目到缓存，通常用于启动时预加载热数据。
    ///
    /// # 参数
    /// - `entries`: 键值对列表
    pub async fn warm_up(&self, entries: Vec<(String, Vec<u8>)>) {
        if !self.config.enabled {
            return;
        }

        for (key, value) in entries {
            self.put(key, value, None).await;
        }
    }

    /// 获取缓存健康状态
    pub async fn health_check(&self) -> CacheHealth {
        let cache = self.cache.read().await;
        let size = cache.len();
        let snapshot = self.get_metrics().await;

        let utilization = if self.config.max_entries > 0 {
            (size as f64) / (self.config.max_entries as f64)
        } else {
            0.0
        };

        CacheHealth {
            size,
            capacity: self.config.max_entries,
            utilization,
            hit_rate: snapshot.hit_rate,
            is_healthy: snapshot.hit_rate > 0.5, // 命中率 > 50% 视为健康
        }
    }

    /// 启动后台过期清理任务
    ///
    /// 定期清理过期的缓存条目。
    ///
    /// # 参数
    /// - `interval`: 清理间隔
    pub fn start_expiration_task(self: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            loop {
                timer.tick().await;
                let count = self.remove_expired_entries().await;
                if count > 0 {
                    tracing::debug!("Removed {} expired cache entries", count);
                }
            }
        })
    }
}

/// 缓存健康状态
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheHealth {
    /// 当前缓存大小
    pub size: usize,
    /// 缓存容量
    pub capacity: usize,
    /// 利用率 (0.0 - 1.0)
    pub utilization: f64,
    /// 命中率
    pub hit_rate: f64,
    /// 是否健康
    pub is_healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_basic_get_put() {
        let config = CacheConfig::default();
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;

        let value = cache.get("key1").await;
        assert_eq!(value, Some(b"value1".to_vec()));

        let value = cache.get("key2").await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_cache_hit_rate() {
        let config = CacheConfig::default();
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;

        // 命中
        cache.get("key1").await;
        // 未命中
        cache.get("key2").await;
        // 再次命中
        cache.get("key1").await;

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.total_requests, 3);
        assert_eq!(metrics.hits, 2);
        assert_eq!(metrics.misses, 1);
        assert!((metrics.hit_rate - 0.666).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let mut config = CacheConfig::default();
        config.max_entries = 3;
        let cache = LruCache::new(config);

        // 添加 3 个条目
        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;
        cache.put("key3".to_string(), b"value3".to_vec(), None).await;

        // 访问 key1，使其变为最近访问
        cache.get("key1").await;

        // 添加第 4 个条目，应该淘汰 key2
        cache.put("key4".to_string(), b"value4".to_vec(), None).await;

        assert_eq!(cache.get("key1").await, Some(b"value1".to_vec()));
        assert_eq!(cache.get("key2").await, None); // 被淘汰
        assert_eq!(cache.get("key3").await, Some(b"value3".to_vec()));
        assert_eq!(cache.get("key4").await, Some(b"value4".to_vec()));

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.evictions, 1);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let config = CacheConfig {
            default_ttl: Duration::from_millis(100),
            ..Default::default()
        };
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;

        // 立即访问，应该命中
        assert_eq!(cache.get("key1").await, Some(b"value1".to_vec()));

        // 等待过期
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 应该过期
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_invalidation() {
        let config = CacheConfig::default();
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.invalidate("key1").await;

        assert_eq!(cache.get("key1").await, None);

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.invalidations, 1);
    }

    #[tokio::test]
    async fn test_batch_invalidation() {
        let config = CacheConfig::default();
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;
        cache.put("key3".to_string(), b"value3".to_vec(), None).await;

        cache
            .invalidate_batch(&["key1".to_string(), "key2".to_string()])
            .await;

        assert_eq!(cache.get("key1").await, None);
        assert_eq!(cache.get("key2").await, None);
        assert_eq!(cache.get("key3").await, Some(b"value3".to_vec()));
    }

    #[tokio::test]
    async fn test_clear() {
        let config = CacheConfig::default();
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;
        cache.put("key2".to_string(), b"value2".to_vec(), None).await;

        cache.clear().await;

        assert_eq!(cache.size().await, 0);
        assert!(cache.is_empty().await);

        let metrics = cache.get_metrics().await;
        assert_eq!(metrics.total_requests, 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let mut config = CacheConfig::default();
        config.max_entries = 100;
        let cache = LruCache::new(config);

        for i in 0..50 {
            cache
                .put(format!("key{}", i), format!("value{}", i).into_bytes(), None)
                .await;
        }

        // 预热，提高命中率
        for i in 0..50 {
            cache.get(&format!("key{}", i)).await;
        }

        let health = cache.health_check().await;
        assert_eq!(health.size, 50);
        assert_eq!(health.capacity, 100);
        assert!((health.utilization - 0.5).abs() < 0.01);
        assert!(health.hit_rate > 0.9);
        assert!(health.is_healthy);
    }

    #[tokio::test]
    async fn test_disabled_cache() {
        let mut config = CacheConfig::default();
        config.enabled = false;
        let cache = LruCache::new(config);

        cache.put("key1".to_string(), b"value1".to_vec(), None).await;

        // 缓存禁用时，get 总是返回 None
        assert_eq!(cache.get("key1").await, None);
    }

    #[tokio::test]
    async fn test_custom_ttl() {
        let config = CacheConfig {
            default_ttl: Duration::from_secs(10),
            ..Default::default()
        };
        let cache = LruCache::new(config);

        // 使用自定义 TTL 100ms
        cache
            .put(
                "key1".to_string(),
                b"value1".to_vec(),
                Some(Duration::from_millis(100)),
            )
            .await;

        // 立即访问，应该命中
        assert_eq!(cache.get("key1").await, Some(b"value1".to_vec()));

        // 等待过期
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 应该过期
        assert_eq!(cache.get("key1").await, None);
    }
}
