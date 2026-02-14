//! # Cache Module
//!
//! CIS 记忆服务的缓存层实现。
//!
//! ## 模块结构
//!
//! - `config`: 缓存配置
//! - `lru`: LRU 缓存核心实现
//! - `batch_ops`: 批量操作
//! - `tests`: 缓存测试
//!
//! ## 特性
//!
//! - LRU 淘汰策略
//! - TTL 支持
//! - 线程安全
//! - 缓存统计
//! - 批量操作
//!
//! ## 性能目标
//!
//! - 缓存命中率 > 70%
//! - 缓存命中延迟 < 1ms
//! - 吞吐量 > 100K ops/sec
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::cache::{LruCache, CacheConfig};
//! use std::time::Duration;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 创建缓存
//! let config = CacheConfig {
//!     max_entries: 1000,
//!     default_ttl: Duration::from_secs(300),
//!     ..Default::default()
//! };
//! let cache = LruCache::new(config);
//!
//! // 使用缓存
//! cache.put("key".to_string(), b"value".to_vec(), None).await;
//! if let Some(value) = cache.get("key").await {
//!     println!("Cache hit: {:?}", value);
//! }
//!
//! // 获取统计
//! let metrics = cache.get_metrics().await;
//! println!("Hit rate: {:.2}%", metrics.hit_rate * 100.0);
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod lru;
pub mod batch_ops;

#[cfg(test)]
mod integration_tests;

pub use config::CacheConfig;
pub use lru::{LruCache, CacheMetrics, CacheMetricsSnapshot, CacheHealth};
pub use batch_ops::{BatchCacheOps, BatchCacheStats, BatchCacheHelper};
