//! # 自适应阈值调整器
//!
//! 根据性能指标动态调整向量搜索的切换阈值和参数。
//!
//! ## 功能
//!
//! - 自动学习最优切换阈值
//! - 根据延迟和 QPS 调整参数
//! - 内存使用监控和优化
//! - 预测性调参
//!
//! ## 调整策略
//!
//! | 指标 | 触发条件 | 调整动作 |
//! |------|---------|---------|
//! | 延迟过高 | P99 > 100ms | 降低 ef_search |
//! | QPS 过低 | QPS < 500 | 增加预加载 |
//! | 缓存命中率低 | hit rate < 30% | 减少预加载 |
//! | 内存占用高 | memory > 80% | 减小批次大小 |
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::vector::adaptive_threshold::{AdaptiveThreshold, ThresholdAction};
//! use cis_core::vector::switch::SearchMetrics;
//! use std::time::Instant;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut adapter = AdaptiveThreshold::new();
//!
//! // 提供性能指标
//! let metrics = SearchMetrics::new(80.0, 400.0, 0.5, 5000);
//! let actions = adapter.adjust(&metrics)?;
//!
//! for action in actions {
//!     println!("建议操作: {:?}", action);
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::error::{CisError, Result};

/// 默认性能目标
const DEFAULT_TARGET_LATENCY_MS: f64 = 50.0;
const DEFAULT_TARGET_QPS: f64 = 1000.0;
const DEFAULT_MIN_CACHE_HIT_RATE: f64 = 0.3;
const DEFAULT_MAX_MEMORY_USAGE: f64 = 0.8;

/// 历史数据保留数量
const HISTORY_SIZE: usize = 100;

/// 调整幅度（百分比）
const ADJUSTMENT_STEP: f32 = 0.2; // 20%

/// 最小/最大 ef_search
const MIN_EF_SEARCH: usize = 10;
const MAX_EF_SEARCH: usize = 200;

/// 最小/最大预加载量
const MIN_PRELOAD: usize = 10;
const MAX_PRELOAD: usize = 500;

/// 阈值调整动作
#[derive(Debug, Clone, PartialEq)]
pub enum ThresholdAction {
    /// 无需调整
    None,

    /// 降低 ef_search（减少延迟）
    DecreaseEfSearch {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 增加 ef_search（提高精度）
    IncreaseEfSearch {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 减少预加载（节省内存）
    DecreasePreload {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 增加预加载（提高 QPS）
    IncreasePreload {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 调整小数据集阈值
    AdjustSmallThreshold {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 调整大数据集阈值
    AdjustLargeThreshold {
        current: usize,
        suggested: usize,
        reason: String,
    },

    /// 切换搜索策略
    SwitchStrategy {
        from: String,
        to: String,
        reason: String,
    },
}

impl ThresholdAction {
    /// 创建无操作
    pub fn none() -> Self {
        Self::None
    }

    /// 检查是否需要调整
    pub fn is_action_required(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// 格式化动作描述
    pub fn describe(&self) -> String {
        match self {
            Self::None => "无需调整".to_string(),

            Self::DecreaseEfSearch { suggested, reason, .. } => {
                format!("降低 ef_search 至 {} ({})", suggested, reason)
            }

            Self::IncreaseEfSearch { suggested, reason, .. } => {
                format!("增加 ef_search 至 {} ({})", suggested, reason)
            }

            Self::DecreasePreload { suggested, reason, .. } => {
                format!("减少预加载至 {} ({})", suggested, reason)
            }

            Self::IncreasePreload { suggested, reason, .. } => {
                format!("增加预加载至 {} ({})", suggested, reason)
            }

            Self::AdjustSmallThreshold { suggested, reason, .. } => {
                format!("调整小数据集阈值至 {} ({})", suggested, reason)
            }

            Self::AdjustLargeThreshold { suggested, reason, .. } => {
                format!("调整大数据集阈值至 {} ({})", suggested, reason)
            }

            Self::SwitchStrategy { to, reason, .. } => {
                format!("切换策略至 {} ({})", to, reason)
            }
        }
    }
}

/// 性能配置目标
#[derive(Debug, Clone)]
pub struct PerformanceTarget {
    /// 目标延迟（毫秒）
    pub target_latency_ms: f64,

    /// 目标 QPS
    pub target_qps: f64,

    /// 最低缓存命中率
    pub min_cache_hit_rate: f64,

    /// 最大内存使用率
    pub max_memory_usage: f64,
}

impl Default for PerformanceTarget {
    fn default() -> Self {
        Self {
            target_latency_ms: DEFAULT_TARGET_LATENCY_MS,
            target_qps: DEFAULT_TARGET_QPS,
            min_cache_hit_rate: DEFAULT_MIN_CACHE_HIT_RATE,
            max_memory_usage: DEFAULT_MAX_MEMORY_USAGE,
        }
    }
}

impl PerformanceTarget {
    /// 创建自定义目标
    pub fn new(
        target_latency_ms: f64,
        target_qps: f64,
        min_cache_hit_rate: f64,
        max_memory_usage: f64,
    ) -> Self {
        Self {
            target_latency_ms,
            target_qps,
            min_cache_hit_rate,
            max_memory_usage,
        }
    }

    /// 验证目标合理性
    pub fn validate(&self) -> Result<()> {
        if self.target_latency_ms <= 0.0 {
            return Err(CisError::other("Target latency must be positive"));
        }

        if self.target_qps <= 0.0 {
            return Err(CisError::other("Target QPS must be positive"));
        }

        if self.min_cache_hit_rate < 0.0 || self.min_cache_hit_rate > 1.0 {
            return Err(CisError::other("Cache hit rate must be in [0, 1]"));
        }

        if self.max_memory_usage <= 0.0 || self.max_memory_usage > 1.0 {
            return Err(CisError::other("Memory usage must be in (0, 1]"));
        }

        Ok(())
    }
}

/// 自适应阈值调整器
///
/// 根据历史性能数据动态调整搜索参数。
///
/// ## 功能
///
/// - 收集性能指标历史
/// - 分析趋势和异常
/// - 生成调整建议
/// - 预测最优参数
pub struct AdaptiveThreshold {
    /// 性能目标
    target: PerformanceTarget,

    /// 历史指标（用于趋势分析）
    history: VecDeque<HistoricalMetrics>,

    /// 当前参数
    current_ef_search: usize,
    current_preload: usize,
    current_small_threshold: usize,
    current_large_threshold: usize,

    /// 调整次数限制（避免频繁调整）
    adjustment_count: usize,
    last_adjustment: Instant,
    min_adjustment_interval: Duration,
}

/// 历史指标记录
#[derive(Debug, Clone)]
struct HistoricalMetrics {
    timestamp: Instant,
    latency_ms: f64,
    qps: f64,
    cache_hit_rate: f64,
    index_size: usize,
}

impl AdaptiveThreshold {
    /// 创建新的调整器
    pub fn new() -> Self {
        Self::with_target(PerformanceTarget::default())
    }

    /// 使用自定义目标创建
    pub fn with_target(target: PerformanceTarget) -> Self {
        target.validate().expect("Invalid performance target");

        Self {
            target,
            history: VecDeque::with_capacity(HISTORY_SIZE),
            current_ef_search: 50,
            current_preload: 100,
            current_small_threshold: 1000,
            current_large_threshold: 10000,
            adjustment_count: 0,
            last_adjustment: Instant::now(),
            min_adjustment_interval: Duration::from_secs(60), // 至少 60 秒调整一次
        }
    }

    /// 配置最小调整间隔
    pub fn with_min_adjustment_interval(mut self, interval: Duration) -> Self {
        self.min_adjustment_interval = interval;
        self
    }

    /// 设置当前参数
    pub fn set_current_params(
        &mut self,
        ef_search: usize,
        preload: usize,
        small_threshold: usize,
        large_threshold: usize,
    ) {
        self.current_ef_search = ef_search;
        self.current_preload = preload;
        self.current_small_threshold = small_threshold;
        self.current_large_threshold = large_threshold;
    }

    /// 调整阈值（主要接口）
    ///
    /// # 参数
    /// - `metrics`: 当前性能指标
    ///
    /// # 返回
    /// - `Result<Vec<ThresholdAction>>`: 调整建议列表
    pub fn adjust(&mut self, metrics: &SearchMetrics) -> Result<Vec<ThresholdAction>> {
        // 记录历史
        self.record_history(metrics);

        // 检查是否需要调整
        if !self.should_adjust() {
            return Ok(vec![ThresholdAction::none()]);
        }

        let mut actions = Vec::new();

        // 1. 延迟检查
        if metrics.avg_latency_ms > self.target.target_latency_ms * 1.5 {
            // 延迟过高，降低 ef_search
            let suggested = self.decrease_ef_search();
            actions.push(ThresholdAction::DecreaseEfSearch {
                current: self.current_ef_search,
                suggested,
                reason: format!("延迟 {}ms 超过目标 {}ms 的 150%",
                    metrics.avg_latency_ms, self.target.target_latency_ms),
            });
            self.current_ef_search = suggested;
        }

        // 2. QPS 检查
        if metrics.qps < self.target.target_qps * 0.5 && metrics.index_size > 10000 {
            // QPS 过低，增加预加载
            let suggested = self.increase_preload();
            actions.push(ThresholdAction::IncreasePreload {
                current: self.current_preload,
                suggested,
                reason: format!("QPS {} 低于目标 {} 的 50%",
                    metrics.qps, self.target.target_qps),
            });
            self.current_preload = suggested;
        }

        // 3. 缓存命中率检查
        if metrics.cache_hit_rate < self.target.min_cache_hit_rate {
            // 缓存命中率低，减少预加载
            let suggested = self.decrease_preload();
            actions.push(ThresholdAction::DecreasePreload {
                current: self.current_preload,
                suggested,
                reason: format!("缓存命中率 {:.1}% 低于目标 {:.1}%",
                    metrics.cache_hit_rate * 100.0,
                    self.target.min_cache_hit_rate * 100.0),
            });
            self.current_preload = suggested;
        }

        // 4. 趋势分析（基于历史数据）
        if let Some(trend) = self.analyze_trend()? {
            actions.extend(trend);
        }

        // 更新调整时间
        self.last_adjustment = Instant::now();
        self.adjustment_count += 1;

        Ok(actions)
    }

    /// 降低 ef_search
    fn decrease_ef_search(&self) -> usize {
        ((self.current_ef_search as f32 * (1.0 - ADJUSTMENT_STEP)) as usize)
            .max(MIN_EF_SEARCH)
    }

    /// 增加 ef_search
    fn increase_ef_search(&self) -> usize {
        ((self.current_ef_search as f32 * (1.0 + ADJUSTMENT_STEP)) as usize)
            .min(MAX_EF_SEARCH)
    }

    /// 增加预加载
    fn increase_preload(&self) -> usize {
        ((self.current_preload as f32 * (1.0 + ADJUSTMENT_STEP)) as usize)
            .min(MAX_PRELOAD)
    }

    /// 减少预加载
    fn decrease_preload(&self) -> usize {
        ((self.current_preload as f32 * (1.0 - ADJUSTMENT_STEP)) as usize)
            .max(MIN_PRELOAD)
    }

    /// 检查是否应该调整
    fn should_adjust(&self) -> bool {
        // 检查最小间隔
        if self.last_adjustment.elapsed() < self.min_adjustment_interval {
            return false;
        }

        // 检查是否有足够的历史数据
        if self.history.len() < 10 {
            return false;
        }

        true
    }

    /// 记录历史指标
    fn record_history(&mut self, metrics: &SearchMetrics) {
        let record = HistoricalMetrics {
            timestamp: metrics.sampled_at,
            latency_ms: metrics.avg_latency_ms,
            qps: metrics.qps,
            cache_hit_rate: metrics.cache_hit_rate,
            index_size: metrics.index_size,
        };

        self.history.push_back(record);

        // 保留最近 N 条记录
        if self.history.len() > HISTORY_SIZE {
            self.history.pop_front();
        }
    }

    /// 分析趋势并生成调整建议
    fn analyze_trend(&self) -> Result<Option<Vec<ThresholdAction>>> {
        if self.history.len() < 10 {
            return Ok(None);
        }

        let mut actions = Vec::new();

        // 计算最近 10 次的平均延迟
        let recent_latencies: Vec<_> = self.history
            .iter()
            .rev()
            .take(10)
            .map(|m| m.latency_ms)
            .collect();

        let avg_recent: f64 = recent_latencies.iter().sum::<f64>() / recent_latencies.len() as f64;

        // 如果延迟持续上升，建议更大的调整
        let is_rising = recent_latencies.windows(2)
            .all(|w| w[1] >= w[0]);

        if is_rising && avg_recent > self.target.target_latency_ms {
            // 延迟持续上升，建议切换策略
            actions.push(ThresholdAction::SwitchStrategy {
                from: format!("HNSW (ef={})", self.current_ef_search),
                to: "SQLite Full Text".to_string(),
                reason: format!("延迟持续上升至 {:.1}ms", avg_recent),
            });
        }

        // 分析缓存命中率趋势
        let recent_cache: Vec<_> = self.history
            .iter()
            .rev()
            .take(10)
            .map(|m| m.cache_hit_rate)
            .collect();

        let avg_cache = recent_cache.iter().sum::<f64>() / recent_cache.len() as f64;

        // 如果缓存命中率持续下降且低于目标
        let is_cache_declining = recent_cache.windows(2)
            .all(|w| w[1] <= w[0]);

        if is_cache_declining && avg_cache < self.target.min_cache_hit_rate {
            let suggested = self.decrease_preload();
            actions.push(ThresholdAction::DecreasePreload {
                current: self.current_preload,
                suggested,
                reason: "缓存命中率持续下降".to_string(),
            });
        }

        if actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(actions))
        }
    }

    /// 获取当前参数
    pub fn current_params(&self) -> (usize, usize, usize, usize) {
        (
            self.current_ef_search,
            self.current_preload,
            self.current_small_threshold,
            self.current_large_threshold,
        )
    }

    /// 获取调整次数
    pub fn adjustment_count(&self) -> usize {
        self.adjustment_count
    }

    /// 重置历史记录
    pub fn reset_history(&mut self) {
        self.history.clear();
        self.adjustment_count = 0;
        self.last_adjustment = Instant::now();
    }

    /// 获取历史记录数量
    pub fn history_size(&self) -> usize {
        self.history.len()
    }

    /// 获取平均延迟（最近 N 次）
    pub fn avg_latency_last_n(&self, n: usize) -> Option<f64> {
        let n = n.min(self.history.len());
        if n == 0 {
            return None;
        }

        let sum: f64 = self.history.iter().rev().take(n)
            .map(|m| m.latency_ms)
            .sum();

        Some(sum / n as f64)
    }

    /// 获取平均 QPS（最近 N 次）
    pub fn avg_qps_last_n(&self, n: usize) -> Option<f64> {
        let n = n.min(self.history.len());
        if n == 0 {
            return None;
        }

        let sum: f64 = self.history.iter().rev().take(n)
            .map(|m| m.qps)
            .sum();

        Some(sum / n as f64)
    }
}

impl Default for AdaptiveThreshold {
    fn default() -> Self {
        Self::new()
    }
}

// 为了方便使用，这里重新导出 SearchMetrics
pub use crate::vector::switch::SearchMetrics;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::switch::SearchMetrics;

    #[test]
    fn test_performance_target_validation() {
        // 有效目标
        let target = PerformanceTarget::new(50.0, 1000.0, 0.3, 0.8);
        assert!(target.validate().is_ok());

        // 无效：延迟为负
        let target = PerformanceTarget::new(-10.0, 1000.0, 0.3, 0.8);
        assert!(target.validate().is_err());

        // 无效：缓存命中率 > 1
        let target = PerformanceTarget::new(50.0, 1000.0, 1.5, 0.8);
        assert!(target.validate().is_err());
    }

    #[test]
    fn test_ef_search_adjustment() {
        let adapter = AdaptiveThreshold::new();
        adapter.set_current_params(100, 100, 1000, 10000);

        // 降低 20%
        let decreased = adapter.decrease_ef_search();
        assert_eq!(decreased, 80);

        // 增加 20%
        let increased = adapter.increase_ef_search();
        assert_eq!(increased, 120);
    }

    #[test]
    fn test_preload_adjustment() {
        let adapter = AdaptiveThreshold::new();
        adapter.set_current_params(50, 200, 1000, 10000);

        // 增加 20%
        let increased = adapter.increase_preload();
        assert_eq!(increased, 240);

        // 降低 20%
        let decreased = adapter.decrease_preload();
        assert_eq!(decreased, 160);
    }

    #[test]
    fn test_adjust_high_latency() {
        let mut adapter = AdaptiveThreshold::new();
        adapter.set_current_params(100, 100, 1000, 10000);

        // 延迟过高（80ms > 50ms * 1.5）
        let metrics = SearchMetrics::new(80.0, 800.0, 0.5, 5000);
        let actions = adapter.adjust(&metrics).unwrap();

        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(a, ThresholdAction::DecreaseEfSearch { .. })));
    }

    #[test]
    fn test_adjust_low_qps() {
        let mut adapter = AdaptiveThreshold::new();
        adapter.set_current_params(50, 100, 1000, 10000);

        // QPS 过低（400 < 1000 * 0.5）且索引大
        let metrics = SearchMetrics::new(40.0, 400.0, 0.5, 15000);
        let actions = adapter.adjust(&metrics).unwrap();

        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(a, ThresholdAction::IncreasePreload { .. })));
    }

    #[test]
    fn test_adjust_low_cache_hit_rate() {
        let mut adapter = AdaptiveThreshold::new();
        adapter.set_current_params(50, 200, 1000, 10000);

        // 缓存命中率低（0.2 < 0.3）
        let metrics = SearchMetrics::new(30.0, 1200.0, 0.2, 5000);
        let actions = adapter.adjust(&metrics).unwrap();

        assert!(!actions.is_empty());
        assert!(actions.iter().any(|a| matches!(a, ThresholdAction::DecreasePreload { .. })));
    }

    #[test]
    fn test_no_adjustment_needed() {
        let mut adapter = AdaptiveThreshold::new();
        adapter.set_current_params(50, 100, 1000, 10000);

        // 性能良好
        let metrics = SearchMetrics::new(30.0, 1200.0, 0.5, 5000);
        let actions = adapter.adjust(&metrics).unwrap();

        // 应该只有一个 None
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ThresholdAction::None);
    }

    #[test]
    fn test_adjustment_interval() {
        let mut adapter = AdaptiveThreshold::new()
            .with_min_adjustment_interval(Duration::from_secs(10));

        adapter.set_current_params(100, 100, 1000, 10000);

        // 第一次调整
        let metrics = SearchMetrics::new(80.0, 800.0, 0.5, 5000);
        let actions1 = adapter.adjust(&metrics).unwrap();
        assert!(!actions1.is_empty());

        // 立即再次尝试（应该被阻止）
        let actions2 = adapter.adjust(&metrics).unwrap();
        assert_eq!(actions2.len(), 1);
        assert_eq!(actions2[0], ThresholdAction::None);
    }

    #[test]
    fn test_action_describe() {
        let action = ThresholdAction::DecreaseEfSearch {
            current: 100,
            suggested: 80,
            reason: "延迟过高".to_string(),
        };

        let description = action.describe();
        assert!(description.contains("降低 ef_search"));
        assert!(description.contains("80"));
        assert!(description.contains("延迟过高"));
    }

    #[test]
    fn test_history_tracking() {
        let mut adapter = AdaptiveThreshold::new();

        // 记录 20 次指标
        for i in 0..20 {
            let metrics = SearchMetrics::new(30.0, 1000.0, 0.5, 5000);
            adapter.adjust(&metrics).unwrap();
        }

        assert_eq!(adapter.history_size(), 20);

        // 测试平均值计算
        let avg_latency = adapter.avg_latency_last_n(10);
        assert!(avg_latency.is_some());
        assert_eq!(avg_latency.unwrap(), 30.0);
    }
}
