// cis-core/src/agent/leak_detector.rs
//
// Agent 泄漏检测器
//
// 定期检查 Agent 守卫的存活时间，检测潜在的泄漏

use std::collections::HashMap;
use std::panic;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

use tracing::{warn, info};
use tokio::sync::oneshot;

use crate::agent::guard::{GuardId, GuardStats};

/// Agent 泄漏检测器
///
/// 定期扫描活跃的 Agent 守卫，报告超过阈值的守卫
pub struct AgentLeakDetector {
    /// 活跃的守卫
    active_guards: Arc<StdRwLock<HashMap<GuardId, GuardInfo>>>,

    /// 泄漏阈值（默认：5 分钟）
    leak_threshold: Duration,

    /// 检测间隔（默认：60 秒）
    check_interval: Duration,

    /// 运行状态
    running: Arc<std::sync::atomic::AtomicBool>,

    /// 停止信号发送器
    shutdown_tx: Option<oneshot::Sender<()>>,
}

/// 守卫信息
#[derive(Debug, Clone)]
struct GuardInfo {
    /// 守卫 ID
    id: GuardId,
    /// 创建时间
    created_at: Instant,
    /// 创建位置
    location: &'static panic::Location<'static>,
    /// Agent ID 或名称
    agent_name: String,
}

/// 泄漏报告
#[derive(Debug, Clone)]
pub struct LeakReport {
    /// 报告时间
    pub reported_at: Instant,
    /// 检测到的泄漏
    pub leaks: Vec<LeakedAgent>,
    /// 摘要统计
    pub summary: LeakSummary,
}

/// 泄漏的 Agent
#[derive(Debug, Clone)]
pub struct LeakedAgent {
    /// 守卫 ID
    pub guard_id: GuardId,
    /// Agent 名称
    pub agent_name: String,
    /// 存活时间
    pub lifetime: Duration,
    /// 创建位置
    pub location: &'static panic::Location<'static>,
    /// 泄漏级别
    pub severity: LeakSeverity,
}

/// 泄漏级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeakSeverity {
    /// 低：轻微超过阈值
    Low,
    /// 中：明显超过阈值
    Medium,
    /// 高：严重超过阈值（2 倍阈值以上）
    High,
}

/// 泄漏摘要
#[derive(Debug, Clone)]
pub struct LeakSummary {
    /// 当前活跃守卫数
    pub active_guards: usize,
    /// 检测到的泄漏数量
    pub leak_count: usize,
    /// 最长存活时间
    pub max_lifetime: Duration,
    /// 平均存活时间
    pub avg_lifetime: Duration,
}

impl AgentLeakDetector {
    /// 创建新的泄漏检测器
    pub fn new() -> Self {
        Self::with_config(Duration::from_secs(300), Duration::from_secs(60))
    }

    /// 使用自定义配置创建
    ///
    /// # 参数
    ///
    /// - `leak_threshold`: 泄漏阈值（存活时间超过此值视为泄漏）
    /// - `check_interval`: 检测间隔
    pub fn with_config(leak_threshold: Duration, check_interval: Duration) -> Self {
        Self {
            active_guards: Arc::new(StdRwLock::new(HashMap::new())),
            leak_threshold,
            check_interval,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            shutdown_tx: None,
        }
    }

    /// 注册守卫
    pub fn register_guard(
        &self,
        id: GuardId,
        location: &'static panic::Location<'static>,
        agent_name: String,
    ) {
        self.active_guards.write().unwrap().insert(
            id.clone(),
            GuardInfo {
                id,
                created_at: Instant::now(),
                location,
                agent_name,
            },
        );

        info!(
            "Registered guard {:?} for agent '{}' at {}:{}",
            id,
            agent_name,
            location.file(),
            location.line()
        );
    }

    /// 注销守卫
    pub fn unregister_guard(&self, id: &GuardId) {
        if let Some(info) = self.active_guards.write().unwrap().remove(id) {
            let lifetime = info.created_at.elapsed();
            info!(
                "Unregistered guard {:?} for agent '{}' after {:?}",
                info.id,
                info.agent_name,
                lifetime
            );
        }
    }

    /// 检测泄漏
    pub fn detect_leaks(&self) -> Vec<LeakedAgent> {
        let guards = self.active_guards.read().unwrap();
        let now = Instant::now();

        guards
            .values()
            .filter(|info| now.duration_since(info.created_at) > self.leak_threshold)
            .map(|info| {
                let lifetime = now.duration_since(info.created_at);
                let severity = if lifetime > self.leak_threshold * 2 {
                    LeakSeverity::High
                } else if lifetime > self.leak_threshold * 1.5 {
                    LeakSeverity::Medium
                } else {
                    LeakSeverity::Low
                };

                LeakedAgent {
                    guard_id: info.id.clone(),
                    agent_name: info.agent_name.clone(),
                    lifetime,
                    location: info.location,
                    severity,
                }
            })
            .collect()
    }

    /// 生成泄漏报告
    pub fn generate_report(&self) -> LeakReport {
        let guards = self.active_guards.read().unwrap();
        let now = Instant::now();

        let active_count = guards.len();
        let leaks: Vec<LeakedAgent> = guards
            .values()
            .filter(|info| now.duration_since(info.created_at) > self.leak_threshold)
            .map(|info| {
                let lifetime = now.duration_since(info.created_at);
                let severity = if lifetime > self.leak_threshold * 2 {
                    LeakSeverity::High
                } else if lifetime > self.leak_threshold * 1.5 {
                    LeakSeverity::Medium
                } else {
                    LeakSeverity::Low
                };

                LeakedAgent {
                    guard_id: info.id.clone(),
                    agent_name: info.agent_name.clone(),
                    lifetime,
                    location: info.location,
                    severity,
                }
            })
            .collect();

        let leak_count = leaks.len();
        let max_lifetime = leaks
            .iter()
            .map(|l| l.lifetime)
            .max()
            .unwrap_or(Duration::ZERO);
        let avg_lifetime = if leak_count > 0 {
            let total: Duration = leaks.iter().map(|l| l.lifetime).sum();
            total / leak_count as u32
        } else {
            Duration::ZERO
        };

        LeakReport {
            reported_at: now,
            leaks,
            summary: LeakSummary {
                active_guards: active_count,
                leak_count,
                max_lifetime,
                avg_lifetime,
            },
        }
    }

    /// 启动定期检测
    pub async fn start(&mut self) -> Result<(), crate::error::CisError> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(crate::error::CisError::AlreadyRunning);
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        let guards = self.active_guards.clone();
        let interval = self.check_interval;
        let threshold = self.leak_threshold;
        let running_flag = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        // 执行泄漏检测
                        let report = Self::check_leaks_internal(&guards, threshold);

                        if !report.leaks.is_empty() {
                            Self::log_leak_report(&report);
                        }
                    }
                    _ = &mut rx => {
                        info!("Leak detector shutting down");
                        running_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                }
            }
        });

        info!(
            "Leak detector started (threshold: {:?}, interval: {:?})",
            self.leak_threshold,
            self.check_interval
        );

        Ok(())
    }

    /// 停止检测器
    pub async fn stop(&mut self) {
        if !self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // 等待停止
        tokio::time::sleep(Duration::from_millis(100)).await;

        info!("Leak detector stopped");
    }

    /// 内部检测逻辑
    fn check_leaks_internal(
        guards: &Arc<StdRwLock<HashMap<GuardId, GuardInfo>>>,
        threshold: Duration,
    ) -> LeakReport {
        let guard_map = guards.read().unwrap();
        let now = Instant::now();

        let active_count = guard_map.len();
        let leaks: Vec<LeakedAgent> = guard_map
            .values()
            .filter(|info| now.duration_since(info.created_at) > threshold)
            .map(|info| {
                let lifetime = now.duration_since(info.created_at);
                let severity = if lifetime > threshold * 2 {
                    LeakSeverity::High
                } else if lifetime > threshold * 1.5 {
                    LeakSeverity::Medium
                } else {
                    LeakSeverity::Low
                };

                LeakedAgent {
                    guard_id: info.id.clone(),
                    agent_name: info.agent_name.clone(),
                    lifetime,
                    location: info.location,
                    severity,
                }
            })
            .collect();

        let leak_count = leaks.len();
        let max_lifetime = leaks
            .iter()
            .map(|l| l.lifetime)
            .max()
            .unwrap_or(Duration::ZERO);
        let avg_lifetime = if leak_count > 0 {
            let total: Duration = leaks.iter().map(|l| l.lifetime).sum();
            total / leak_count as u32
        } else {
            Duration::ZERO
        };

        LeakReport {
            reported_at: now,
            leaks,
            summary: LeakSummary {
                active_guards: active_count,
                leak_count,
                max_lifetime,
                avg_lifetime,
            },
        }
    }

    /// 记录泄漏报告
    fn log_leak_report(report: &LeakReport) {
        warn!("=== Agent Leak Report ===");
        warn!("Active guards: {}", report.summary.active_guards);
        warn!("Leaks detected: {}", report.summary.leak_count);
        warn!("Max lifetime: {:?}", report.summary.max_lifetime);
        warn!("Avg lifetime: {:?}", report.summary.avg_lifetime);
        warn!("");

        if report.leaks.is_empty() {
            return;
        }

        // 按严重程度排序
        let mut sorted_leaks = report.leaks.clone();
        sorted_leaks.sort_by(|a, b| {
            // 首先按严重程度排序
            match (b.severity, a.severity) {
                (LeakSeverity::High, LeakSeverity::Medium) => std::cmp::Ordering::Greater,
                (LeakSeverity::High, LeakSeverity::Low) => std::cmp::Ordering::Greater,
                (LeakSeverity::Medium, LeakSeverity::Low) => std::cmp::Ordering::Greater,
                (LeakSeverity::High, LeakSeverity::High) |
                (LeakSeverity::Medium, LeakSeverity::Medium) |
                (LeakSeverity::Low, LeakSeverity::Low) => std::cmp::Ordering::Equal,
                _ => std::cmp::Ordering::Less,
            }
        });

        for leak in sorted_leaks {
            let severity_icon = match leak.severity {
                LeakSeverity::Low => "⚠️",
                LeakSeverity::Medium => "🟡",
                LeakSeverity::High => "🔴",
            };

            warn!(
                "{} Guard {:?} for agent '{}' - lifetime: {:?}, location: {}:{}",
                severity_icon,
                leak.guard_id,
                leak.agent_name,
                leak.lifetime,
                leak.location.file(),
                leak.location.line()
            );
        }
    }

    /// 获取当前活跃守卫数
    pub fn active_count(&self) -> usize {
        self.active_guards.read().unwrap().len()
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for AgentLeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leak_detector_registration() {
        let detector = AgentLeakDetector::new();
        let id = GuardId::new("test-guard");

        detector.register_guard(
            id.clone(),
            panic::Location::caller(),
            "test-agent".to_string(),
        );

        assert_eq!(detector.active_count(), 1);

        detector.unregister_guard(&id);

        assert_eq!(detector.active_count(), 0);
    }

    #[tokio::test]
    async fn test_leak_detection() {
        let detector = AgentLeakDetector::with_config(
            Duration::from_millis(100),
            Duration::from_millis(50),
        );

        let id = GuardId::new("test-guard");
        detector.register_guard(
            id.clone(),
            panic::Location::caller(),
            "test-agent".to_string(),
        );

        // 立即检查，不应泄漏
        let leaks = detector.detect_leaks();
        assert_eq!(leaks.len(), 0);

        // 等待超过阈值
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 应该检测到泄漏
        let leaks = detector.detect_leaks();
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].guard_id, id);

        detector.unregister_guard(&id);
    }

    #[tokio::test]
    async fn test_leak_detector_start_stop() {
        let mut detector = AgentLeakDetector::with_config(
            Duration::from_millis(100),
            Duration::from_millis(50),
        );

        // 启动检测器
        detector.start().await.unwrap();
        assert!(detector.is_running());

        // 停止检测器
        detector.stop().await;
        assert!(!detector.is_running());
    }

    #[test]
    fn test_severity_levels() {
        let threshold = Duration::from_secs(10);

        // 低严重度
        assert_eq!(
            LeakSeverity::Low,
            if Duration::from_secs(12) > threshold * 2 {
                LeakSeverity::High
            } else if Duration::from_secs(12) > threshold * 1.5 {
                LeakSeverity::Medium
            } else {
                LeakSeverity::Low
            }
        );

        // 中严重度
        assert_eq!(
            LeakSeverity::Medium,
            if Duration::from_secs(17) > threshold * 2 {
                LeakSeverity::High
            } else if Duration::from_secs(17) > threshold * 1.5 {
                LeakSeverity::Medium
            } else {
                LeakSeverity::Low
            }
        );

        // 高严重度
        assert_eq!(
            LeakSeverity::High,
            if Duration::from_secs(25) > threshold * 2 {
                LeakSeverity::High
            } else if Duration::from_secs(25) > threshold * 1.5 {
                LeakSeverity::Medium
            } else {
                LeakSeverity::Low
            }
        );
    }
}
