//! # 智能索引切换策略
//!
//! 根据索引大小动态选择最优的向量搜索策略。
//!
//! ## 功能
//!
//! - 自动根据索引大小选择 HNSW 或 SQLite
//! - 支持自适应阈值调整
//! - 性能监控和报告
//!
//! ## 切换策略
//!
//! | 索引大小 | 策略 | ef_search | 说明 |
//! |---------|------|-----------|------|
//! | < 1000 | SQLite Full Text | N/A | 快速扫描，无索引开销 |
//! | 1000-10000 | HNSW | 50 | 平衡性能和内存 |
//! | > 10000 | HNSW + Cache | 100 | 高性能，预加载热门结果 |
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::vector::switch::{IndexMonitor, SearchStrategy};
//!
//! # fn example() -> anyhow::Result<()> {
//! let mut monitor = IndexMonitor::new();
//!
//! // 决策搜索策略
//! let strategy = monitor.decide_strategy(5000);
//! println!("策略: {:?}", strategy);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{CisError, Result};

/// 小数据集阈值（使用 SQLite）
const DEFAULT_SMALL_THRESHOLD: usize = 1000;

/// 大数据集阈值（启用缓存和更大的 ef_search）
const DEFAULT_LARGE_THRESHOLD: usize = 10000;

/// HNSW ef_search（小数据集）
const DEFAULT_EF_SEARCH_SMALL: usize = 50;

/// HNSW ef_search（大数据集）
const DEFAULT_EF_SEARCH_LARGE: usize = 100;

/// 默认缓存大小（大数据集）
const DEFAULT_CACHE_SIZE: usize = 100;

/// 搜索策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStrategy {
    /// SQLite 全文搜索（适合小数据集）
    SQLiteFullText,

    /// HNSW 索引搜索（中等数据集）
    HNSW {
        /// HNSW ef_search 参数
        ef_search: usize,
    },

    /// HNSW + 缓存（大数据集）
    HNSWWithCache {
        /// HNSW ef_search 参数
        ef_search: usize,

        /// 预加载结果数量
        preload_top_k: usize,
    },
}

impl SearchStrategy {
    /// 获取 ef_search 参数
    pub fn ef_search(&self) -> Option<usize> {
        match self {
            Self::SQLiteFullText => None,
            Self::HNSW { ef_search } => Some(*ef_search),
            Self::HNSWWithCache { ef_search, .. } => Some(*ef_search),
        }
    }

    /// 是否使用缓存
    pub fn uses_cache(&self) -> bool {
        matches!(self, Self::HNSWWithCache { .. })
    }

    /// 是否使用 HNSW
    pub fn uses_hnsw(&self) -> bool {
        !matches!(self, Self::SQLiteFullText)
    }
}

/// 切换阈值配置
#[derive(Debug, Clone)]
pub struct SwitchThreshold {
    /// 小数据集阈值
    pub small_threshold: usize,

    /// 大数据集阈值
    pub large_threshold: usize,

    /// HNSW ef_search（小数据集）
    pub ef_search_small: usize,

    /// HNSW ef_search（大数据集）
    pub ef_search_large: usize,

    /// 预加载结果数量（大数据集）
    pub preload_top_k: usize,
}

impl Default for SwitchThreshold {
    fn default() -> Self {
        Self {
            small_threshold: DEFAULT_SMALL_THRESHOLD,
            large_threshold: DEFAULT_LARGE_THRESHOLD,
            ef_search_small: DEFAULT_EF_SEARCH_SMALL,
            ef_search_large: DEFAULT_EF_SEARCH_LARGE,
            preload_top_k: DEFAULT_CACHE_SIZE,
        }
    }
}

impl SwitchThreshold {
    /// 创建自定义阈值配置
    pub fn new(
        small_threshold: usize,
        large_threshold: usize,
        ef_search_small: usize,
        ef_search_large: usize,
        preload_top_k: usize,
    ) -> Self {
        Self {
            small_threshold,
            large_threshold,
            ef_search_small,
            ef_search_large,
            preload_top_k,
        }
    }

    /// 验证配置有效性
    pub fn validate(&self) -> Result<()> {
        if self.small_threshold >= self.large_threshold {
            return Err(CisError::other(
                "small_threshold must be less than large_threshold"
            ));
        }

        if self.ef_search_small == 0 || self.ef_search_large == 0 {
            return Err(CisError::other("ef_search must be greater than 0"));
        }

        Ok(())
    }
}

/// 搜索性能指标
#[derive(Debug, Clone)]
pub struct SearchMetrics {
    /// 平均查询延迟（毫秒）
    pub avg_latency_ms: f64,

    /// QPS（每秒查询数）
    pub qps: f64,

    /// 缓存命中率（0.0-1.0）
    pub cache_hit_rate: f64,

    /// 当前索引大小
    pub index_size: usize,

    /// 采样时间
    pub sampled_at: Instant,
}

impl SearchMetrics {
    /// 创建新的性能指标
    pub fn new(
        avg_latency_ms: f64,
        qps: f64,
        cache_hit_rate: f64,
        index_size: usize,
    ) -> Self {
        Self {
            avg_latency_ms,
            qps,
            cache_hit_rate,
            index_size,
            sampled_at: Instant::now(),
        }
    }
}

/// 索引监控器
///
/// 监控索引大小并动态调整搜索策略。
///
/// ## 功能
///
/// - 跟踪当前索引大小
/// - 根据阈值自动选择策略
/// - 支持性能指标收集
/// - 自适应调整阈值
///
/// ## 示例
///
/// ```rust,no_run
/// use cis_core::vector::switch::IndexMonitor;
///
/// # fn example() -> anyhow::Result<()> {
/// let mut monitor = IndexMonitor::new();
///
/// // 更新索引大小
/// monitor.update_index_size(5000);
///
/// // 获取当前策略
/// let strategy = monitor.current_strategy();
/// println!("策略: {:?}", strategy);
///
/// // 记录搜索延迟
/// monitor.record_search_latency(std::time::Duration::from_millis(10));
///
/// // 获取性能指标
/// let metrics = monitor.get_metrics()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct IndexMonitor {
    /// 当前索引大小
    index_size: Arc<RwLock<usize>>,

    /// 切换阈值配置
    threshold: Arc<RwLock<SwitchThreshold>>,

    /// 搜索延迟记录（用于计算平均值）
    search_latencies: Arc<RwLock<Vec<Duration>>>,

    /// 缓存命中记录
    cache_hits: Arc<RwLock<usize>>,
    cache_misses: Arc<RwLock<usize>>,

    /// QPS 计算
    query_count: Arc<RwLock<usize>>,
    last_query_time: Arc<RwLock<Option<Instant>>>,
}

impl IndexMonitor {
    /// 创建新的索引监控器
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::switch::IndexMonitor;
    ///
    /// # fn example() {
    /// let monitor = IndexMonitor::new();
    /// # }
    /// ```
    pub fn new() -> Self {
        Self::with_threshold(SwitchThreshold::default())
    }

    /// 使用自定义阈值创建监控器
    ///
    /// # 参数
    /// - `threshold`: 切换阈值配置
    pub fn with_threshold(threshold: SwitchThreshold) -> Self {
        threshold.validate().expect("Invalid threshold configuration");

        Self {
            index_size: Arc::new(RwLock::new(0)),
            threshold: Arc::new(RwLock::new(threshold)),
            search_latencies: Arc::new(RwLock::new(Vec::with_capacity(1000))),
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
            query_count: Arc::new(RwLock::new(0)),
            last_query_time: Arc::new(RwLock::new(None)),
        }
    }

    /// 更新索引大小
    ///
    /// # 参数
    /// - `size`: 当前索引中的向量数量
    pub async fn update_index_size(&self, size: usize) {
        let mut index_size = self.index_size.write().await;
        *index_size = size;
    }

    /// 获取当前索引大小
    pub async fn get_index_size(&self) -> usize {
        *self.index_size.read().await
    }

    /// 决策搜索策略
    ///
    /// 根据当前索引大小和阈值配置，自动选择最优的搜索策略。
    ///
    /// # 返回
    /// - `SearchStrategy`: 推荐的搜索策略
    pub async fn decide_strategy(&self) -> SearchStrategy {
        let index_size = *self.index_size.read().await;
        self.decide_strategy_for_size(index_size)
    }

    /// 为指定大小决策策略
    ///
    /// # 参数
    /// - `size`: 索引大小
    pub fn decide_strategy_for_size(&self, size: usize) -> SearchStrategy {
        // 读取阈值（使用 blocking_get 因为这是同步方法）
        let threshold = self.threshold.try_read().ok();

        if let Some(threshold) = threshold {
            if size < threshold.small_threshold {
                SearchStrategy::SQLiteFullText
            } else if size < threshold.large_threshold {
                SearchStrategy::HNSW {
                    ef_search: threshold.ef_search_small,
                }
            } else {
                SearchStrategy::HNSWWithCache {
                    ef_search: threshold.ef_search_large,
                    preload_top_k: threshold.preload_top_k,
                }
            }
        } else {
            // 如果无法读取配置（并发冲突），使用默认策略
            if size < DEFAULT_SMALL_THRESHOLD {
                SearchStrategy::SQLiteFullText
            } else if size < DEFAULT_LARGE_THRESHOLD {
                SearchStrategy::HNSW {
                    ef_search: DEFAULT_EF_SEARCH_SMALL,
                }
            } else {
                SearchStrategy::HNSWWithCache {
                    ef_search: DEFAULT_EF_SEARCH_LARGE,
                    preload_top_k: DEFAULT_CACHE_SIZE,
                }
            }
        }
    }

    /// 获取当前策略（基于当前索引大小）
    pub async fn current_strategy(&self) -> SearchStrategy {
        self.decide_strategy().await
    }

    /// 记录搜索延迟
    ///
    /// # 参数
    /// - `latency`: 查询延迟
    pub async fn record_search_latency(&self, latency: Duration) {
        let mut latencies = self.search_latencies.write().await;

        // 保留最近 1000 次查询
        if latencies.len() >= 1000 {
            latencies.remove(0);
        }

        latencies.push(latency);

        // 更新查询计数
        *self.query_count.write().await += 1;
        *self.last_query_time.write().await = Some(Instant::now());
    }

    /// 记录缓存命中
    pub async fn record_cache_hit(&self) {
        *self.cache_hits.write().await += 1;
    }

    /// 记录缓存未命中
    pub async fn record_cache_miss(&self) {
        *self.cache_misses.write().await += 1;
    }

    /// 获取性能指标
    ///
    /// # 返回
    /// - `Result<SearchMetrics>`: 性能指标
    pub async fn get_metrics(&self) -> Result<SearchMetrics> {
        let index_size = *self.index_size.read().await;

        // 计算平均延迟
        let latencies = self.search_latencies.read().await;
        let avg_latency_ms = if latencies.is_empty() {
            0.0
        } else {
            let total: f64 = latencies.iter().map(|d| d.as_millis() as f64).sum();
            total / latencies.len() as f64
        };

        // 计算 QPS（基于最近 1 分钟）
        let qps = {
            let query_count = *self.query_count.read().await;
            let last_time = *self.last_query_time.read().await;

            if let Some(last) = last_time {
                let elapsed = last.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    query_count as f64 / elapsed
                } else {
                    0.0
                }
            } else {
                0.0
            }
        };

        // 计算缓存命中率
        let cache_hit_rate = {
            let hits = *self.cache_hits.read().await;
            let misses = *self.cache_misses.read().await;
            let total = hits + misses;

            if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            }
        };

        Ok(SearchMetrics::new(avg_latency_ms, qps, cache_hit_rate, index_size))
    }

    /// 获取当前阈值配置
    pub async fn get_threshold(&self) -> SwitchThreshold {
        self.threshold.read().await.clone()
    }

    /// 更新阈值配置
    ///
    /// # 参数
    /// - `threshold`: 新的阈值配置
    pub async fn set_threshold(&self, threshold: SwitchThreshold) -> Result<()> {
        threshold.validate()?;
        *self.threshold.write().await = threshold;
        Ok(())
    }

    /// 重置性能指标
    pub async fn reset_metrics(&self) {
        self.search_latencies.write().await.clear();
        *self.cache_hits.write().await = 0;
        *self.cache_misses.write().await = 0;
        *self.query_count.write().await = 0;
        *self.last_query_time.write().await = None;
    }

    /// 获取平均延迟（仅最近 N 次查询）
    ///
    /// # 参数
    /// - `n`: 查询次数（0 表示全部）
    pub async fn avg_latency_last_n(&self, n: usize) -> Duration {
        let latencies = self.search_latencies.read().await;

        let subset = if n == 0 || n >= latencies.len() {
            &latencies[..]
        } else {
            &latencies[latencies.len() - n..]
        };

        if subset.is_empty() {
            return Duration::ZERO;
        }

        let total_ms: f64 = subset.iter().map(|d| d.as_millis() as f64).sum();
        Duration::from_millis((total_ms / subset.len() as f64) as u64)
    }
}

impl Default for IndexMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_selection() {
        let monitor = IndexMonitor::new();

        // 小数据集
        let strategy = monitor.decide_strategy_for_size(500);
        assert_eq!(strategy, SearchStrategy::SQLiteFullText);
        assert!(!strategy.uses_hnsw());

        // 中等数据集
        let strategy = monitor.decide_strategy_for_size(5000);
        assert_eq!(strategy, SearchStrategy::HNSW { ef_search: 50 });
        assert!(strategy.uses_hnsw());
        assert!(!strategy.uses_cache());

        // 大数据集
        let strategy = monitor.decide_strategy_for_size(15000);
        assert_eq!(
            strategy,
            SearchStrategy::HNSWWithCache {
                ef_search: 100,
                preload_top_k: 100
            }
        );
        assert!(strategy.uses_hnsw());
        assert!(strategy.uses_cache());
    }

    #[test]
    fn test_threshold_validation() {
        // 有效配置
        let threshold = SwitchThreshold::new(1000, 10000, 50, 100, 100);
        assert!(threshold.validate().is_ok());

        // 无效配置：small >= large
        let threshold = SwitchThreshold::new(10000, 1000, 50, 100, 100);
        assert!(threshold.validate().is_err());

        // 无效配置：ef_search = 0
        let threshold = SwitchThreshold::new(1000, 10000, 0, 100, 100);
        assert!(threshold.validate().is_err());
    }

    #[tokio::test]
    async fn test_index_monitoring() {
        let monitor = IndexMonitor::new();

        // 更新索引大小
        monitor.update_index_size(5000).await;
        assert_eq!(monitor.get_index_size().await, 5000);

        // 获取策略
        let strategy = monitor.current_strategy().await;
        assert_eq!(strategy, SearchStrategy::HNSW { ef_search: 50 });

        // 记录延迟
        monitor
            .record_search_latency(Duration::from_millis(10))
            .await;
        monitor
            .record_search_latency(Duration::from_millis(20))
            .await;

        // 计算平均延迟
        let avg = monitor.avg_latency_last_n(0).await;
        assert_eq!(avg.as_millis(), 15);

        // 获取指标
        let metrics = monitor.get_metrics().await.unwrap();
        assert_eq!(metrics.index_size, 5000);
        assert_eq!(metrics.avg_latency_ms, 15.0);
    }

    #[tokio::test]
    async fn test_cache_metrics() {
        let monitor = IndexMonitor::new();

        // 记录缓存命中/未命中
        monitor.record_cache_hit().await;
        monitor.record_cache_hit().await;
        monitor.record_cache_miss().await;

        // 获取指标
        let metrics = monitor.get_metrics().await.unwrap();
        assert!((metrics.cache_hit_rate - 0.666).abs() < 0.01); // 2/3
    }

    #[tokio::test]
    async fn test_custom_threshold() {
        let custom = SwitchThreshold::new(500, 5000, 30, 80, 50);
        let monitor = IndexMonitor::with_threshold(custom.clone());

        // 验证自定义阈值生效
        assert_eq!(
            monitor.decide_strategy_for_size(100),
            SearchStrategy::SQLiteFullText
        );
        assert_eq!(
            monitor.decide_strategy_for_size(3000),
            SearchStrategy::HNSW { ef_search: 30 }
        );
        assert_eq!(
            monitor.decide_strategy_for_size(10000),
            SearchStrategy::HNSWWithCache {
                ef_search: 80,
                preload_top_k: 50
            }
        );
    }
}
