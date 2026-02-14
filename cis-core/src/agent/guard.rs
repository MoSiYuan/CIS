// cis-core/src/agent/guard.rs
//
// AgentGuard - RAII 风格的 Agent 生命周期管理
//
// 确保 Agent 及其资源在离开作用域时被正确清理

use std::collections::HashMap;
use std::panic;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

use tracing::{debug, info, warn};

use crate::error::{CisError, Result};

/// Agent 守卫，确保资源自动清理
///
/// # 示例
///
/// ```rust,no_run
/// use cis_core::agent::guard::AgentGuard;
///
/// # async fn example() -> anyhow::Result<()> {
/// let agent = create_agent()?;
/// let mut guard = AgentGuard::new(agent)
///     .on_drop(|agent| {
///         println!("Cleaning up agent {}", agent.id());
///     });
///
/// // 使用 agent
/// guard.agent().execute_task().await?;
///
/// // 离开作用域时自动清理
/// # Ok(())
/// # }
/// ```
pub struct AgentGuard<T> {
    /// Agent 实例
    agent: Option<T>,
    /// 清理处理器列表
    cleanup_handlers: Vec<Box<dyn FnOnce(T) + Send>>,
    /// 是否在 panic 时清理
    cleanup_on_panic: bool,
    /// 守卫创建时间
    created_at: Instant,
    /// 守卫标识
    id: GuardId,
    /// 守卫创建位置
    location: &'static panic::Location<'static>,
    /// 是否已清理
    cleaned: Arc<atomic::AtomicBool>,
}

/// 守卫唯一标识
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuardId(pub String);

impl GuardId {
    /// 创建新的守卫 ID
    pub fn new(name: &str) -> Self {
        Self(format!("guard-{}", name))
    }

    /// 生成唯一 ID
    pub fn unique() -> Self {
        Self(format!("guard-{}", uuid::Uuid::new_v4()))
    }
}

impl<T> AgentGuard<T> {
    /// 创建新的守卫
    ///
    /// # 参数
    ///
    /// - `agent`: Agent 实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::agent::guard::AgentGuard;
    ///
    /// let agent = create_agent()?;
    /// let guard = AgentGuard::new(agent);
    /// ```
    #[track_caller]
    pub fn new(agent: T) -> Self {
        Self::with_id(agent, GuardId::unique())
    }

    /// 创建带有自定义 ID 的守卫
    ///
    /// # 参数
    ///
    /// - `agent`: Agent 实例
    /// - `id`: 守卫 ID
    #[track_caller]
    pub fn with_id(agent: T, id: GuardId) -> Self {
        // 注册到泄漏检测器
        if let Some(detector) = LeakDetector::global() {
            detector.register_guard(id.clone(), panic::Location::caller());
        }

        Self {
            agent: Some(agent),
            cleanup_handlers: Vec::new(),
            cleanup_on_panic: true,
            created_at: Instant::now(),
            id,
            location: panic::Location::caller(),
            cleaned: Arc::new(atomic::AtomicBool::new(false)),
        }
    }

    /// 添加同步清理回调
    ///
    /// # 参数
    ///
    /// - `f`: 清理函数，接收 Agent 的所有权
    ///
    /// # 示例
    ///
    /// ```rust
    /// # use cis_core::agent::guard::AgentGuard;
    /// # let agent = ();
    /// let guard = AgentGuard::new(agent)
    ///     .on_drop(|agent| {
    ///         println!("Cleaning up");
    ///     });
    /// ```
    pub fn on_drop<F>(mut self, f: F) -> Self
    where
        F: FnOnce(T) + Send + 'static,
    {
        self.cleanup_handlers.push(Box::new(f));
        self
    }

    /// 添加异步清理回调
    ///
    /// # 参数
    ///
    /// - `f`: 异步清理函数
    ///
    /// # 示例
    ///
    /// ```rust
    /// # use cis_core::agent::guard::AgentGuard;
    /// # let agent = ();
    /// let guard = AgentGuard::new(agent)
    ///     .on_drop_async(|agent| async move {
    ///         tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    ///         println!("Async cleanup");
    ///     });
    /// ```
    pub fn on_drop_async<F, Fut>(mut self, f: F) -> Self
    where
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.cleanup_handlers.push(Box::new(|agent| {
            tokio::spawn(async move {
                f(agent).await;
            });
        }));
        self
    }

    /// 设置是否在 panic 时清理（默认：true）
    pub fn cleanup_on_panic(mut self, cleanup: bool) -> Self {
        self.cleanup_on_panic = cleanup;
        self
    }

    /// 获取 Agent 引用
    pub fn agent(&self) -> &T {
        self.agent
            .as_ref()
            .expect("Agent already cleaned")
    }

    /// 获取 Agent 可变引用
    pub fn agent_mut(&mut self) -> &mut T {
        self.agent
            .as_mut()
            .expect("Agent already cleaned")
    }

    /// 手动触发清理（提前释放）
    ///
    /// # 注意
    ///
    /// 手动清理后，`agent()` 和 `agent_mut()` 将 panic
    pub async fn cleanup(mut self) -> Result<(), AgentCleanupError> {
        if self.agent.is_none() {
            return Ok(());
        }

        // 手动清理时先注销监控
        if let Some(detector) = LeakDetector::global() {
            detector.unregister_guard(&self.id);
        }

        // 执行清理
        self.perform_cleanup().await?;

        Ok(())
    }

    /// 检查是否已清理
    pub fn is_cleaned(&self) -> bool {
        self.agent.is_none()
    }

    /// 获取守卫 ID
    pub fn id(&self) -> &GuardId {
        &self.id
    }

    /// 获取守卫存活时间
    pub fn lifetime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// 内部清理逻辑
    async fn perform_cleanup(&mut self) -> Result<(), AgentCleanupError> {
        if let Some(agent) = self.agent.take() {
            let lifetime = self.created_at.elapsed();

            debug!(
                "AgentGuard {:?} cleaning up after {:?} (created at {}:{})",
                self.id,
                lifetime,
                self.location.file(),
                self.location.line()
            );

            // 执行所有清理回调（反向）
            let mut errors = Vec::new();
            for handler in self.cleanup_handlers.drain(..).rev() {
                // 捕获清理过程中的 panic
                if let Err(e) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    handler(agent);
                })) {
                    warn!("Cleanup handler panicked: {:?}", e);
                    errors.push(AgentCleanupError::HandlerPanic);
                }
            }

            // 标记已清理
            self.cleaned.store(true, atomic::Ordering::SeqCst);

            // 记录统计
            if let Some(stats) = GuardStats::global() {
                stats.record_lifetime(lifetime);
                stats.record_cleanup();
            }

            if !errors.is_empty() {
                warn!(
                    "AgentGuard {:?} cleanup completed with {} errors",
                    self.id,
                    errors.len()
                );
                return Err(AgentCleanupError::PartialFailure);
            }

            debug!("AgentGuard {:?} cleaned up successfully", self.id);
        }

        Ok(())
    }
}

impl<T> Drop for AgentGuard<T> {
    fn drop(&mut self) {
        // 检查是否需要清理
        if let Some(agent) = self.agent.take() {
            let lifetime = self.created_at.elapsed();
            let is_panic = std::thread::panicking();

            // 检查是否应该在 panic 时清理
            if is_panic && !self.cleanup_on_panic {
                warn!(
                    "AgentGuard {:?} skipping cleanup during panic",
                    self.id
                );
                return;
            }

            // 从泄漏检测器注销
            if let Some(detector) = LeakDetector::global() {
                detector.unregister_guard(&self.id);
            }

            info!(
                "AgentGuard {:?} cleaning up after {:?} (created at {}:{}, panic: {})",
                self.id,
                lifetime,
                self.location.file(),
                self.location.line(),
                is_panic
            );

            // 执行所有清理回调（反向）
            let mut error_count = 0;
            for handler in self.cleanup_handlers.drain(..).rev() {
                // 捕获清理过程中的 panic
                if let Err(e) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    handler(agent);
                })) {
                    warn!("Cleanup handler panicked: {:?}", e);
                    error_count += 1;
                }
            }

            // 标记已清理
            self.cleaned.store(true, atomic::Ordering::SeqCst);

            // 记录统计
            if let Some(stats) = GuardStats::global() {
                stats.record_lifetime(lifetime);
                if is_panic {
                    stats.record_panic_cleanup();
                } else {
                    stats.record_normal_cleanup();
                }
            }

            if error_count > 0 {
                warn!(
                    "AgentGuard {:?} cleanup completed with {} errors",
                    self.id,
                    error_count
                );
            } else {
                debug!("AgentGuard {:?} cleaned up successfully", self.id);
            }
        }
    }
}

/// Agent 清理错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCleanupError {
    /// 部分清理失败
    PartialFailure,
    /// 清理处理器 panic
    HandlerPanic,
    /// 清理超时
    Timeout,
}

impl std::fmt::Display for AgentCleanupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PartialFailure => write!(f, "Partial cleanup failure"),
            Self::HandlerPanic => write!(f, "Cleanup handler panicked"),
            Self::Timeout => write!(f, "Cleanup timeout"),
        }
    }
}

impl std::error::Error for AgentCleanupError {}

impl From<AgentCleanupError> for CisError {
    fn from(err: AgentCleanupError) -> Self {
        CisError::AgentCleanup(err.to_string())
    }
}

/// 泄漏检测器
pub struct LeakDetector {
    /// 活跃的守卫
    active_guards: Arc<StdRwLock<HashMap<GuardId, GuardInfo>>>,
    /// 泄漏阈值（秒）
    leak_threshold: Duration,
}

/// 守卫信息
#[derive(Debug, Clone)]
struct GuardInfo {
    /// 守卫 ID
    id: GuardId,
    /// 创建时间
    created_at: Instant,
    /// 守卫创建位置
    location: &'static panic::Location<'static>,
}

impl LeakDetector {
    /// 获取全局泄漏检测器
    pub fn global() -> Option<&'static Self> {
        // 使用 once_cell::sync::Lazy 或类似机制
        // 这里简化为 None
        None
    }

    /// 创建新的泄漏检测器
    pub fn new(leak_threshold: Duration) -> Self {
        Self {
            active_guards: Arc::new(StdRwLock::new(HashMap::new())),
            leak_threshold,
        }
    }

    /// 注册守卫
    pub fn register_guard(
        &self,
        id: GuardId,
        location: &'static panic::Location<'static>,
    ) {
        self.active_guards.write().unwrap().insert(
            id.clone(),
            GuardInfo {
                id,
                created_at: Instant::now(),
                location,
            },
        );
        debug!("Registered guard {:?}", id);
    }

    /// 注销守卫
    pub fn unregister_guard(&self, id: &GuardId) {
        self.active_guards.write().unwrap().remove(id);
        debug!("Unregistered guard {:?}", id);
    }

    /// 检测泄漏
    pub fn detect_leaks(&self) -> Vec<LeakedGuard> {
        let guards = self.active_guards.read().unwrap();
        let now = Instant::now();

        guards
            .values()
            .filter(|info| now.duration_since(info.created_at) > self.leak_threshold)
            .map(|info| LeakedGuard {
                id: info.id.clone(),
                lifetime: now.duration_since(info.created_at),
                location: info.location,
            })
            .collect()
    }

    /// 获取当前活跃守卫数
    pub fn active_count(&self) -> usize {
        self.active_guards.read().unwrap().len()
    }
}

/// 泄漏的守卫
#[derive(Debug, Clone)]
pub struct LeakedGuard {
    /// 守卫 ID
    pub id: GuardId,
    /// 存活时间
    pub lifetime: Duration,
    /// 创建位置
    pub location: &'static panic::Location<'static>,
}

/// 守卫统计
pub struct GuardStats {
    /// 创建的守卫总数
    total_created: atomic::AtomicU64,
    /// 正常清理的守卫数
    cleaned_normally: atomic::AtomicU64,
    /// 因 panic 清理的守卫数
    cleaned_on_panic: atomic::AtomicU64,
    /// 总存活时间
    total_lifetime: atomic::AtomicU64,
    /// 最大存活时间
    max_lifetime: atomic::AtomicU64,
}

impl GuardStats {
    /// 获取全局统计
    pub fn global() -> Option<&'static Self> {
        // 使用 once_cell::sync::Lazy
        None
    }

    /// 记录存活时间
    fn record_lifetime(&self, lifetime: Duration) {
        let nanos = lifetime.as_nanos() as u64;
        self.total_lifetime.fetch_add(nanos, atomic::Ordering::Relaxed);

        // 更新最大值
        loop {
            let current = self.max_lifetime.load(atomic::Ordering::Relaxed);
            if nanos <= current {
                break;
            }
            match self.max_lifetime.compare_exchange_weak(
                current,
                nanos,
                atomic::Ordering::Relaxed,
                atomic::Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    /// 记录正常清理
    fn record_normal_cleanup(&self) {
        self.cleaned_normally.fetch_add(1, atomic::Ordering::Relaxed);
    }

    /// 记录因 panic 清理
    fn record_panic_cleanup(&self) {
        self.cleaned_on_panic.fetch_add(1, atomic::Ordering::Relaxed);
    }

    /// 记录清理
    fn record_cleanup(&self) {
        self.total_created.fetch_add(1, atomic::Ordering::Relaxed);
    }

    /// 获取统计摘要
    pub fn summary(&self) -> GuardStatsSummary {
        let total_cleaned = self.cleaned_normally.load(atomic::Ordering::Relaxed)
            + self.cleaned_on_panic.load(atomic::Ordering::Relaxed);
        let total_lifetime_nanos = self.total_lifetime.load(atomic::Ordering::Relaxed);
        let avg_lifetime = if total_cleaned > 0 {
            Duration::from_nanos(total_lifetime_nanos / total_cleaned)
        } else {
            Duration::ZERO
        };

        GuardStatsSummary {
            total_created: self.total_created.load(atomic::Ordering::Relaxed),
            cleaned_normally: self.cleaned_normally.load(atomic::Ordering::Relaxed),
            cleaned_on_panic: self.cleaned_on_panic.load(atomic::Ordering::Relaxed),
            avg_lifetime,
            max_lifetime: Duration::from_nanos(
                self.max_lifetime.load(atomic::Ordering::Relaxed)
            ),
        }
    }
}

/// 统计摘要
#[derive(Debug, Clone)]
pub struct GuardStatsSummary {
    /// 创建的守卫总数
    pub total_created: u64,
    /// 正常清理的守卫数
    pub cleaned_normally: u64,
    /// 因 panic 清理的守卫数
    pub cleaned_on_panic: u64,
    /// 平均存活时间
    pub avg_lifetime: Duration,
    /// 最大存活时间
    pub max_lifetime: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_guard_basic_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    cleaned_clone.store(true, Ordering::SeqCst);
                });
        }

        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[test]
    fn test_guard_multiple_handlers() {
        let counter = Arc::new(atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();

        {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
                .on_drop(move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
                .on_drop(move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                });
        }

        // 反向执行（3 -> 2 -> 1）
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_guard_panic_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        let result = std::panic::catch_unwind(|| {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    cleaned_clone.store(true, Ordering::SeqCst);
                });

            panic!("Intentional panic");
        });

        assert!(result.is_err());
        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[test]
    fn test_guard_no_cleanup_on_panic() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        let result = std::panic::catch_unwind(|| {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    cleaned_clone.store(true, Ordering::SeqCst);
                })
                .cleanup_on_panic(false);

            panic!("Intentional panic");
        });

        assert!(result.is_err());
        assert!(!cleaned.load(Ordering::SeqCst));
    }

    #[test]
    fn test_guard_manual_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        let mut guard = AgentGuard::new(())
            .on_drop(move |_| {
                cleaned_clone.store(true, Ordering::SeqCst);
            });

        // 手动清理
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            guard.cleanup().await.unwrap();
        });

        assert!(cleaned.load(Ordering::SeqCst));
        assert!(guard.is_cleaned());
    }

    #[test]
    fn test_leak_detector() {
        let detector = LeakDetector::new(Duration::from_millis(100));
        let id = GuardId::new("test-guard");

        detector.register_guard(id.clone());

        assert_eq!(detector.active_count(), 1);

        detector.unregister_guard(&id);

        assert_eq!(detector.active_count(), 0);
    }

    #[tokio::test]
    async fn test_leak_detection() {
        let detector = LeakDetector::new(Duration::from_millis(100));
        let id = GuardId::new("test-guard");

        detector.register_guard(id.clone());

        // 立即检查，不应泄漏
        assert!(detector.detect_leaks().is_empty());

        // 等待超过阈值
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 应该检测到泄漏
        let leaked = detector.detect_leaks();
        assert_eq!(leaked.len(), 1);
        assert_eq!(leaked[0].id, id);

        detector.unregister_guard(&id);
    }
}
