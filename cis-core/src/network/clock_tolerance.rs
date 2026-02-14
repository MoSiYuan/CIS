//! 时钟偏差处理模块
//!
//! 处理分布式系统中的时钟偏差问题。

use std::time::{Duration, SystemTime};

/// 默认时钟容忍度（60 秒）
const DEFAULT_TOLERANCE_SECS: u64 = 60;

/// 时钟偏差管理器
///
/// 用于跟踪和调整不同节点之间的时钟偏差。
#[derive(Debug, Clone)]
pub struct ClockTolerance {
    /// 可接受的时钟偏差
    tolerance: Duration,
    /// 观察到的最大时钟偏差（用于调试）
    max_observed_skew: Duration,
}

impl Default for ClockTolerance {
    fn default() -> Self {
        Self {
            tolerance: Duration::from_secs(DEFAULT_TOLERANCE_SECS),
            max_observed_skew: Duration::ZERO,
        }
    }
}

impl ClockTolerance {
    /// 创建新的时钟容忍度管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用自定义容忍度创建
    pub fn with_tolerance(tolerance: Duration) -> Self {
        Self {
            tolerance,
            max_observed_skew: Duration::ZERO,
        }
    }

    /// 获取容忍度
    pub fn tolerance(&self) -> Duration {
        self.tolerance
    }

    /// 设置容忍度
    pub fn set_tolerance(&mut self, tolerance: Duration) {
        self.tolerance = tolerance;
    }

    /// 检查时间戳是否在可接受范围内
    ///
    /// # 参数
    /// - `timestamp`: 要检查的时间戳
    /// - `reference`: 参考时间（通常是当前时间）
    ///
    /// # 返回
    /// - `bool`: 如果在容忍度范围内返回 true
    pub fn is_acceptable(&self, timestamp: SystemTime, reference: SystemTime) -> bool {
        // 检查是否在未来（超出容忍度）
        if timestamp > reference + self.tolerance {
            return false;
        }

        // 检查是否在过去（超出容忍度）
        if let Ok(elapsed) = reference.duration_since(timestamp) {
            if elapsed > self.tolerance {
                return false;
            }
        }

        true
    }

    /// 计算时钟偏差
    ///
    /// 返回两个时间戳之间的差值（绝对值）。
    pub fn calculate_skew(&self, timestamp1: SystemTime, timestamp2: SystemTime) -> Duration {
        // 转换为 Duration 并计算差值
        let duration1 = timestamp1
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let duration2 = timestamp2
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();

        if duration1 > duration2 {
            duration1 - duration2
        } else {
            duration2 - duration1
        }
    }

    /// 更新观察到的最大偏差
    pub fn update_observed_skew(&mut self, skew: Duration) {
        if skew > self.max_observed_skew {
            self.max_observed_skew = skew;
        }
    }

    /// 获取观察到的最大偏差
    pub fn max_observed_skew(&self) -> Duration {
        self.max_observed_skew
    }

    /// 调整时间戳（考虑偏差）
    ///
    /// 如果时间戳在未来（超出容忍度），将其调整到最大可接受的未来时间。
    pub fn adjust_timestamp(&self, timestamp: SystemTime, reference: SystemTime) -> SystemTime {
        if timestamp > reference + self.tolerance {
            // 时间戳太远在未来，调整到最大可接受时间
            return reference + self.tolerance;
        }

        timestamp
    }

    /// 检查是否需要调整
    pub fn needs_adjustment(&self, timestamp: SystemTime, reference: SystemTime) -> bool {
        timestamp > reference + self.tolerance
            || reference
                .duration_since(timestamp)
                .map(|elapsed| elapsed > self.tolerance)
                .unwrap_or(false)
    }

    /// 验证时间范围
    ///
    /// 检查时间戳是否在 [start, end] 范围内（考虑容忍度）。
    pub fn is_within_range(
        &self,
        timestamp: SystemTime,
        start: SystemTime,
        end: SystemTime,
    ) -> bool {
        // 扩展范围以包含容忍度
        let adjusted_start = start.saturating_sub(self.tolerance);
        let adjusted_end = end + self.tolerance;

        timestamp >= adjusted_start && timestamp <= adjusted_end
    }
}

/// 时间同步状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncStatus {
    /// 已同步（时钟偏差在容忍度内）
    Synchronized,
    /// 未同步（时钟偏差超出容忍度）
    NotSynchronized,
    /// 未知（未观察到足够的时间戳）
    Unknown,
}

/// 时间同步跟踪器
///
/// 跟踪多个节点的时间戳以评估同步状态。
#[derive(Debug, Clone)]
pub struct TimeSyncTracker {
    tolerance: Duration,
    observed_skews: Vec<Duration>,
}

impl TimeSyncTracker {
    /// 创建新的跟踪器
    pub fn new(tolerance: Duration) -> Self {
        Self {
            tolerance,
            observed_skews: Vec::new(),
        }
    }

    /// 记录观察到的时钟偏差
    pub fn record_skew(&mut self, skew: Duration) {
        self.observed_skews.push(skew);

        // 只保留最近 100 个观察
        if self.observed_skews.len() > 100 {
            self.observed_skews.remove(0);
        }
    }

    /// 评估同步状态
    pub fn sync_status(&self) -> SyncStatus {
        if self.observed_skews.is_empty() {
            return SyncStatus::Unknown;
        }

        // 检查所有最近的偏差是否在容忍度内
        let all_in_tolerance = self
            .observed_skews
            .iter()
            .all(|skew| *skew <= self.tolerance);

        if all_in_tolerance {
            SyncStatus::Synchronized
        } else {
            SyncStatus::NotSynchronized
        }
    }

    /// 计算平均时钟偏差
    pub fn average_skew(&self) -> Option<Duration> {
        if self.observed_skews.is_empty() {
            return None;
        }

        let total: Duration = self.observed_skews.iter().sum();
        Some(total / self.observed_skews.len() as u32)
    }

    /// 计算最大时钟偏差
    pub fn max_skew(&self) -> Option<Duration> {
        self.observed_skews.iter().max().copied()
    }

    /// 重置跟踪器
    pub fn reset(&mut self) {
        self.observed_skews.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_acceptable_within_tolerance() {
        let tolerance = ClockTolerance::with_tolerance(Duration::from_secs(60));
        let now = SystemTime::now();

        // 在容忍度内
        assert!(tolerance.is_acceptable(now + Duration::from_secs(30), now));
        assert!(tolerance.is_acceptable(now - Duration::from_secs(30), now));
    }

    #[test]
    fn test_is_acceptable_outside_tolerance() {
        let tolerance = ClockTolerance::with_tolerance(Duration::from_secs(60));
        let now = SystemTime::now();

        // 超出容忍度
        assert!(!tolerance.is_acceptable(now + Duration::from_secs(120), now));
        assert!(!tolerance.is_acceptable(now - Duration::from_secs(120), now));
    }

    #[test]
    fn test_calculate_skew() {
        let tolerance = ClockTolerance::new();
        let now = SystemTime::now();
        let future = now + Duration::from_secs(10);
        let past = now - Duration::from_secs(5);

        assert_eq!(tolerance.calculate_skew(now, future), Duration::from_secs(10));
        assert_eq!(tolerance.calculate_skew(now, past), Duration::from_secs(5));
    }

    #[test]
    fn test_adjust_timestamp() {
        let tolerance = ClockTolerance::with_tolerance(Duration::from_secs(60));
        let now = SystemTime::now();
        let far_future = now + Duration::from_secs(120);

        // 调整超出容忍度的时间戳
        let adjusted = tolerance.adjust_timestamp(far_future, now);
        assert_eq!(adjusted, now + Duration::from_secs(60));

        // 不调整在容忍度内的时间戳
        let acceptable = now + Duration::from_secs(30);
        let adjusted = tolerance.adjust_timestamp(acceptable, now);
        assert_eq!(adjusted, acceptable);
    }

    #[test]
    fn test_is_within_range() {
        let tolerance = ClockTolerance::with_tolerance(Duration::from_secs(60));
        let start = SystemTime::now();
        let end = start + Duration::from_secs(300);

        // 在范围内（包括容忍度）
        assert!(tolerance.is_within_range(start, start, end));
        assert!(tolerance.is_within_range(end, start, end));
        assert!(tolerance.is_within_range(start + Duration::from_secs(150), start, end));

        // 超出范围
        assert!(!tolerance.is_within_range(start - Duration::from_secs(120), start, end));
        assert!(!tolerance.is_within_range(end + Duration::from_secs(120), start, end));
    }

    #[test]
    fn test_time_sync_tracker() {
        let mut tracker = TimeSyncTracker::new(Duration::from_secs(60));

        // 初始状态未知
        assert_eq!(tracker.sync_status(), SyncStatus::Unknown);

        // 记录在容忍度内的偏差
        tracker.record_skew(Duration::from_secs(30));
        assert_eq!(tracker.sync_status(), SyncStatus::Synchronized);

        // 记录超出容忍度的偏差
        tracker.record_skew(Duration::from_secs(120));
        assert_eq!(tracker.sync_status(), SyncStatus::NotSynchronized);

        // 计算平均偏差
        assert_eq!(tracker.average_skew(), Some(Duration::from_secs(75)));

        // 计算最大偏差
        assert_eq!(tracker.max_skew(), Some(Duration::from_secs(120)));

        // 重置
        tracker.reset();
        assert_eq!(tracker.sync_status(), SyncStatus::Unknown);
    }

    #[test]
    fn test_update_observed_skew() {
        let mut tolerance = ClockTolerance::new();

        tolerance.update_observed_skew(Duration::from_secs(10));
        assert_eq!(tolerance.max_observed_skew(), Duration::from_secs(10));

        tolerance.update_observed_skew(Duration::from_secs(20));
        assert_eq!(tolerance.max_observed_skew(), Duration::from_secs(20));

        // 更小的偏差不会更新最大值
        tolerance.update_observed_skew(Duration::from_secs(15));
        assert_eq!(tolerance.max_observed_skew(), Duration::from_secs(20));
    }

    #[test]
    fn test_needs_adjustment() {
        let tolerance = ClockTolerance::with_tolerance(Duration::from_secs(60));
        let now = SystemTime::now();

        // 需要调整
        assert!(tolerance.needs_adjustment(now + Duration::from_secs(120), now));
        assert!(tolerance.needs_adjustment(now - Duration::from_secs(120), now));

        // 不需要调整
        assert!(!tolerance.needs_adjustment(now + Duration::from_secs(30), now));
        assert!(!tolerance.needs_adjustment(now - Duration::from_secs(30), now));
        assert!(!tolerance.needs_adjustment(now, now));
    }
}
