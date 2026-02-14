// cis-core/src/lock_timeout/monitor.rs
//
// 锁竞争监控器
//
// 收集和分析锁使用情况，生成性能报告

use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use crate::lock_timeout::{LockId, LockType, LockStats};

/// 锁竞争级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentionLevel {
    /// 无竞争：健康状态
    Healthy,
    /// 低竞争：需要关注
    Low,
    /// 中等竞争：需要优化
    Medium,
    /// 高竞争：需要立即处理
    High,
}

impl ContentionLevel {
    /// 从等待时间判断竞争级别
    pub fn from_wait_time(wait_time: Duration) -> Self {
        if wait_time < Duration::from_millis(10) {
            Self::Healthy
        } else if wait_time < Duration::from_millis(50) {
            Self::Low
        } else if wait_time < Duration::from_millis(100) {
            Self::Medium
        } else {
            Self::High
        }
    }
}

impl std::fmt::Display for ContentionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "✅ Healthy"),
            Self::Low => write!(f, "⚠️  Low contention"),
            Self::Medium => write!(f, "🟡 Medium contention"),
            Self::High => write!(f, "🔴 High contention"),
        }
    }
}

/// 锁性能报告
#[derive(Debug, Clone)]
pub struct LockReport {
    /// 报告生成时间
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// 所有锁的统计信息
    pub lock_stats: Vec<LockStatEntry>,
    /// 摘要信息
    pub summary: ReportSummary,
}

/// 单个锁的统计条目
#[derive(Debug, Clone)]
pub struct LockStatEntry {
    /// 锁标识
    pub id: LockId,
    /// 统计信息
    pub stats: LockStats,
    /// 竞争级别
    pub contention_level: ContentionLevel,
    /// 健康评分（0-100）
    pub health_score: u8,
}

/// 报告摘要
#[derive(Debug, Clone)]
pub struct ReportSummary {
    /// 锁总数
    pub total_locks: usize,
    /// 总获取次数
    pub total_acquisitions: u64,
    /// 总超时次数
    pub total_timeouts: u64,
    /// 超时率（百分比）
    pub timeout_rate: f64,
    /// 平均等待时间
    pub avg_wait_time: Duration,
    /// 最大等待时间
    pub max_wait_time: Duration,
    /// 需要关注的锁数量
    pub attention_needed: usize,
    /// 高竞争锁数量
    pub high_contention: usize,
}

/// 锁监控器
///
/// 用于收集和分析系统中所有锁的使用情况
pub struct LockMonitor {
    /// 所有锁的统计信息
    stats: StdRwLock<HashMap<LockId, Arc<LockStats>>>,
    /// 监控器创建时间
    created_at: chrono::DateTime<chrono::Utc>,
}

impl LockMonitor {
    /// 创建新的监控器
    pub fn new() -> Self {
        Self {
            stats: StdRwLock::new(HashMap::new()),
            created_at: chrono::Utc::now(),
        }
    }

    /// 注册一个锁进行监控
    pub fn register_lock(&self, id: LockId, stats: Arc<LockStats>) {
        let mut guard = self.stats.write().unwrap();
        guard.insert(id, stats);
        tracing::debug!("Registered lock for monitoring: {}", id);
    }

    /// 注销一个锁
    pub fn unregister_lock(&self, id: &LockId) {
        let mut guard = self.stats.write().unwrap();
        guard.remove(id);
        tracing::debug!("Unregistered lock from monitoring: {}", id);
    }

    /// 获取锁统计信息
    pub fn get_stats(&self, id: &LockId) -> Option<LockStats> {
        let guard = self.stats.read().unwrap();
        guard.get(id).map(|s| (**s).clone())
    }

    /// 获取所有锁的统计信息
    pub fn get_all_stats(&self) -> Vec<(LockId, LockStats)> {
        let guard = self.stats.read().unwrap();
        guard.iter()
            .map(|(id, stats)| (id.clone(), (**stats).clone()))
            .collect()
    }

    /// 生成性能报告
    pub fn generate_report(&self) -> LockReport {
        let all_stats = self.get_all_stats();

        let mut lock_entries = Vec::new();
        let mut total_acquisitions = 0u64;
        let mut total_timeouts = 0u64;
        let mut total_wait_time = Duration::ZERO;
        let mut max_wait_time = Duration::ZERO;
        let mut attention_needed = 0usize;
        let mut high_contention = 0usize;

        for (id, stats) in &all_stats {
            let contenton_level = if stats.total_acquisitions > 0 {
                ContentionLevel::from_wait_time(stats.total_wait_time / stats.total_acquisitions)
            } else {
                ContentionLevel::Healthy
            };

            let health_score = Self::calculate_health_score(&stats);

            if matches!(contenton_level, ContentionLevel::Medium | ContentionLevel::High) {
                attention_needed += 1;
            }

            if matches!(contenton_level, ContentionLevel::High) {
                high_contention += 1;
            }

            total_acquisitions += stats.total_acquisitions;
            total_timeouts += stats.timeout_count;
            total_wait_time += stats.total_wait_time;
            max_wait_time = max_wait_time.max(stats.max_wait_time);

            lock_entries.push(LockStatEntry {
                id: id.clone(),
                stats: stats.clone(),
                contention_level: contenton_level,
                health_score,
            });
        }

        // 按健康评分排序
        lock_entries.sort_by(|a, b| a.health_score.cmp(&b.health_score));

        let timeout_rate = if total_acquisitions > 0 {
            (total_timeouts as f64 / total_acquisitions as f64) * 100.0
        } else {
            0.0
        };

        let avg_wait_time = if total_acquisitions > 0 {
            total_wait_time / total_acquisitions as u32
        } else {
            Duration::ZERO
        };

        let summary = ReportSummary {
            total_locks: all_stats.len(),
            total_acquisitions,
            total_timeouts,
            timeout_rate,
            avg_wait_time,
            max_wait_time,
            attention_needed,
            high_contention,
        };

        LockReport {
            generated_at: chrono::Utc::now(),
            lock_stats: lock_entries,
            summary,
        }
    }

    /// 检测高竞争锁
    ///
    /// 返回平均等待时间超过阈值的锁 ID 列表
    pub fn detect_contention(&self, threshold: Duration) -> Vec<LockId> {
        let all_stats = self.get_all_stats();
        let mut contested = Vec::new();

        for (id, stats) in all_stats {
            if stats.total_acquisitions > 0 {
                let avg_wait = stats.total_wait_time / stats.total_acquisitions as u32;
                if avg_wait > threshold {
                    contested.push(id);
                }
            }
        }

        contested
    }

    /// 检测超时率异常的锁
    ///
    /// 返回超时率超过阈值的锁 ID 列表
    pub fn detect_high_timeout_rate(&self, threshold_percent: f64) -> Vec<LockId> {
        let all_stats = self.get_all_stats();
        let mut problematic = Vec::new();

        for (id, stats) in all_stats {
            if stats.total_acquisitions > 0 {
                let rate = (stats.timeout_count as f64 / stats.total_acquisitions as f64) * 100.0;
                if rate > threshold_percent {
                    problematic.push(id);
                }
            }
        }

        problematic
    }

    /// 记录报告到日志
    pub fn log_report(&self) {
        let report = self.generate_report();
        info!("=== Lock Performance Report ===");
        info!("Generated at: {}", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC"));
        info!("");
        info!("Summary:");
        info!("  Total locks: {}", report.summary.total_locks);
        info!("  Total acquisitions: {}", report.summary.total_acquisitions);
        info!("  Total timeouts: {} ({:.2}%)",
            report.summary.total_timeouts,
            report.summary.timeout_rate
        );
        info!("  Avg wait time: {:?}", report.summary.avg_wait_time);
        info!("  Max wait time: {:?}", report.summary.max_wait_time);
        info!("  Locks needing attention: {}", report.summary.attention_needed);
        info!("  High contention locks: {}", report.summary.high_contention);
        info!("");

        // 显示前 10 个问题锁
        let problem_locks: Vec<_> = report.lock_stats.iter()
            .filter(|entry| matches!(
                entry.contention_level,
                ContentionLevel::Medium | ContentionLevel::High
            ) || entry.stats.timeout_count > 0)
            .take(10)
            .collect();

        if !problem_locks.is_empty() {
            info!("Problem Locks (Top 10):");
            for entry in problem_locks {
                info!("Lock: {}", entry.id);
                info!("  Total acquisitions: {}", entry.stats.total_acquisitions);
                info!("  Timeouts: {} ({:.2}%)",
                    entry.stats.timeout_count,
                    (entry.stats.timeout_count as f64
                        / entry.stats.total_acquisitions.max(1) as f64) * 100.0
                );
                info!("  Avg wait time: {:?}", entry.stats.avg_wait_time());
                info!("  Max wait time: {:?}", entry.stats.max_wait_time);
                info!("  Avg hold time: {:?}", entry.stats.avg_hold_time());
                info!("  Max hold time: {:?}", entry.stats.max_hold_time);
                info!("  Current waiters: {}", entry.stats.current_waiters);
                info!("  Status: {}", entry.contention_level);
                info!("  Health score: {}/100", entry.health_score);
                info!("");
            }
        }

        if report.summary.high_contention > 0 {
            warn!(
                "Detected {} locks with high contention - consider optimization",
                report.summary.high_contention
            );
        }
    }

    /// 计算健康评分（0-100）
    ///
    /// 考虑因素：
    /// - 超时率（权重：40%）
    /// - 平均等待时间（权重：30%）
    /// - 平均持有时间（权重：20%）
    /// - 当前等待者数量（权重：10%）
    fn calculate_health_score(stats: &LockStats) -> u8 {
        if stats.total_acquisitions == 0 {
            return 100;
        }

        let mut score = 100u8;

        // 超时率惩罚（40%）
        let timeout_rate = (stats.timeout_count as f32 / stats.total_acquisitions as f32) * 100.0;
        score -= (timeout_rate * 0.4) as u8;

        // 等待时间惩罚（30%）
        let avg_wait_ms = stats.avg_wait_time().as_millis() as f32;
        if avg_wait_ms > 10.0 {
            let wait_penalty = ((avg_wait_ms - 10.0) / 1000.0 * 30.0).min(30.0);
            score -= wait_penalty as u8;
        }

        // 持有时间惩罚（20%）
        let avg_hold_ms = stats.avg_hold_time().as_millis() as f32;
        if avg_hold_ms > 100.0 {
            let hold_penalty = ((avg_hold_ms - 100.0) / 10000.0 * 20.0).min(20.0);
            score -= hold_penalty as u8;
        }

        // 等待者惩罚（10%）
        let waiter_penalty = (stats.current_waiters as f32 * 2.0).min(10.0);
        score -= waiter_penalty as u8;

        score.max(0).min(100)
    }

    /// 清空所有统计信息
    pub fn clear_all(&self) {
        let mut guard = self.stats.write().unwrap();
        guard.clear();
        info!("Cleared all lock monitoring statistics");
    }
}

impl Default for LockMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// 创建测试用的统计信息
    fn create_test_stats(
        acquisitions: u64,
        timeouts: u64,
        total_wait: Duration,
        max_wait: Duration,
        total_hold: Duration,
        max_hold: Duration,
    ) -> LockStats {
        LockStats {
            total_acquisitions: acquisitions,
            timeout_count: timeouts,
            total_wait_time: total_wait,
            max_wait_time: max_wait,
            total_hold_time: total_hold,
            max_hold_time: max_hold,
            current_waiters: 0,
            last_wait_time: None,
        }
    }

    #[test]
    fn test_contention_level_healthy() {
        let level = ContentionLevel::from_wait_time(Duration::from_millis(5));
        assert_eq!(level, ContentionLevel::Healthy);
    }

    #[test]
    fn test_contention_level_low() {
        let level = ContentionLevel::from_wait_time(Duration::from_millis(25));
        assert_eq!(level, ContentionLevel::Low);
    }

    #[test]
    fn test_contention_level_medium() {
        let level = ContentionLevel::from_wait_time(Duration::from_millis(75));
        assert_eq!(level, ContentionLevel::Medium);
    }

    #[test]
    fn test_contention_level_high() {
        let level = ContentionLevel::from_wait_time(Duration::from_millis(150));
        assert_eq!(level, ContentionLevel::High);
    }

    #[test]
    fn test_health_score_perfect() {
        let stats = create_test_stats(
            1000,
            0,
            Duration::from_millis(100),
            Duration::from_millis(10),
            Duration::from_millis(50),
            Duration::from_millis(5),
        );

        let score = LockMonitor::calculate_health_score(&stats);
        assert!(score >= 95, "Expected high score, got {}", score);
    }

    #[test]
    fn test_health_score_with_timeouts() {
        let stats = create_test_stats(
            1000,
            10, // 1% timeout rate
            Duration::from_millis(100),
            Duration::from_millis(10),
            Duration::from_millis(50),
            Duration::from_millis(5),
        );

        let score = LockMonitor::calculate_health_score(&stats);
        assert!(score < 100, "Score should be penalized for timeouts");
        assert!(score > 90, "Score shouldn't be too low for 1% timeout");
    }

    #[test]
    fn test_health_score_poor() {
        let stats = create_test_stats(
            1000,
            100, // 10% timeout rate
            Duration::from_secs(10),
            Duration::from_secs(1),
            Duration::from_secs(5),
            Duration::from_millis(500),
        );

        let score = LockMonitor::calculate_health_score(&stats);
        assert!(score < 50, "Expected low score for poor performance");
    }

    #[test]
    fn test_monitor_register_and_retrieve() {
        let monitor = LockMonitor::new();
        let id = LockId::new("test.rs", "test_lock");

        let stats = Arc::new(create_test_stats(
            100,
            0,
            Duration::from_millis(10),
            Duration::from_millis(5),
            Duration::from_millis(20),
            Duration::from_millis(10),
        ));

        monitor.register_lock(id.clone(), stats.clone());

        let retrieved = monitor.get_stats(&id);
        assert!(retrieved.is_some());

        let retrieved_stats = retrieved.unwrap();
        assert_eq!(retrieved_stats.total_acquisitions, 100);
    }

    #[test]
    fn test_detect_contention() {
        let monitor = LockMonitor::new();

        // 注册一个高竞争锁
        let id1 = LockId::new("test.rs", "contended_lock");
        let stats1 = Arc::new(create_test_stats(
            1000,
            0,
            Duration::from_millis(200), // avg 200ms
            Duration::from_millis(500),
            Duration::from_millis(50),
            Duration::from_millis(100),
        ));
        monitor.register_lock(id1, stats1);

        // 注册一个正常锁
        let id2 = LockId::new("test.rs", "normal_lock");
        let stats2 = Arc::new(create_test_stats(
            1000,
            0,
            Duration::from_millis(10), // avg 10ms
            Duration::from_millis(20),
            Duration::from_millis(5),
            Duration::from_millis(10),
        ));
        monitor.register_lock(id2, stats2);

        // 检测竞争（阈值 100ms）
        let contested = monitor.detect_contention(Duration::from_millis(100));
        assert_eq!(contested.len(), 1);
        assert!(contested.contains(&id1));
        assert!(!contested.contains(&id2));
    }

    #[test]
    fn test_generate_report() {
        let monitor = LockMonitor::new();

        let id = LockId::new("test.rs", "test_lock");
        let stats = Arc::new(create_test_stats(
            1000,
            10,
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(50),
            Duration::from_millis(100),
        ));
        monitor.register_lock(id, stats);

        let report = monitor.generate_report();

        assert_eq!(report.lock_stats.len(), 1);
        assert_eq!(report.summary.total_locks, 1);
        assert_eq!(report.summary.total_acquisitions, 1000);
        assert_eq!(report.summary.total_timeouts, 10);
    }
}
