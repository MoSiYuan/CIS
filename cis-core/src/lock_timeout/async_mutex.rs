// cis-core/src/lock_timeout/async_mutex.rs
//
// 带超时功能的互斥锁包装器
// 基于 tokio::sync::Mutex，添加超时和监控功能

use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex as TokioMutex;
use tokio::time::timeout;

use crate::error::{CisError, Result};
use super::{LockTimeoutError, LockId};

/// 锁统计信息（原子计数器）
#[derive(Debug, Clone, Default)]
struct LockStatsAtomic {
    /// 锁获取总次数
    total_acquisitions: std::sync::atomic::AtomicU64,

    /// 锁超时次数
    timeout_count: std::sync::atomic::AtomicU64,

    /// 总等待时间（纳秒）
    total_wait_time: std::sync::atomic::AtomicU64,

    /// 最大等待时间（纳秒）
    max_wait_time: std::sync::atomic::AtomicU64,

    /// 总持有时间（纳秒）
    total_hold_time: std::sync::atomic::AtomicU64,

    /// 最大持有时间（纳秒）
    max_hold_time: std::sync::atomic::AtomicU64,

    /// 当前等待者数量
    current_waiters: std::sync::atomic::AtomicI32,

    /// 最后等待时间（纳秒）
    last_wait_time: std::sync::atomic::AtomicU64,
}

/// 带超时的互斥锁
pub struct AsyncMutex<T> {
    inner: Arc<TokioMutex<T>>,
    default_timeout: Duration,
    id: Option<LockId>,
    stats: Arc<LockStatsAtomic>,
}

impl<T> AsyncMutex<T> {
    /// 创建新的带超时的互斥锁
    pub fn new(value: T, default_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(TokioMutex::new(value)),
            default_timeout,
            id: None,
            stats: Arc::new(LockStatsAtomic::default()),
        }
    }

    /// 创建带有监控标识的锁
    pub fn with_id(value: T, default_timeout: Duration, id: LockId) -> Self {
        Self {
            inner: Arc::new(TokioMutex::new(value)),
            default_timeout,
            id: Some(id),
            stats: Arc::new(LockStatsAtomic::default()),
        }
    }

    /// 使用默认超时获取锁
    pub async fn lock_with_timeout(&self) -> Result<AsyncMutexGuard<T>, LockTimeoutError> {
        self.lock_with_custom_timeout(self.default_timeout).await
    }

    /// 使用自定义超时获取锁
    pub async fn lock_with_custom_timeout(
        &self,
        timeout_duration: Duration,
    ) -> Result<AsyncMutexGuard<T>, LockTimeoutError> {
        let start = Instant::now();
        self.stats.current_waiters.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let result = timeout(timeout_duration, self.inner.lock()).await;

        self.stats.current_waiters.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        match result {
            Ok(guard) => {
                let wait_time = start.elapsed();
                self.stats.total_acquisitions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.update_max_wait_time(wait_time);

                tracing::trace!(
                    "Mutex lock acquired for {:?} after {:?}",
                    self.id,
                    wait_time
                );

                Ok(AsyncMutexGuard {
                    inner: guard,
                    acquired_at: Instant::now(),
                    stats: self.stats.clone(),
                })
            }
            Err(_) => {
                let wait_time = start.elapsed();
                self.stats.timeout_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                tracing::warn!(
                    "Mutex lock timeout for {:?} after {:?} (timeout: {:?})",
                    self.id,
                    wait_time,
                    timeout_duration
                );

                Err(LockTimeoutError::Timeout {
                    lock_type: super::LockType::WriteLock,
                    timeout: timeout_duration,
                })
            }
        }
    }

    /// 获取锁统计信息
    pub fn stats(&self) -> super::LockStats {
        super::LockStats {
            total_acquisitions: self.stats.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed),
            timeout_count: self.stats.timeout_count.load(std::sync::atomic::Ordering::Relaxed),
            total_wait_time: Duration::from_nanos(
                self.stats.total_wait_time.load(std::sync::atomic::Ordering::Relaxed)
            ),
            max_wait_time: Duration::from_nanos(
                self.stats.max_wait_time.load(std::sync::atomic::Ordering::Relaxed)
            ),
            total_hold_time: Duration::from_nanos(
                self.stats.total_hold_time.load(std::sync::atomic::Ordering::Relaxed)
            ),
            max_hold_time: Duration::from_nanos(
                self.stats.max_hold_time.load(std::sync::atomic::Ordering::Relaxed)
            ),
            current_waiters: self.stats.current_waiters.load(std::sync::atomic::Ordering::Relaxed),
            last_wait_time: {
                let nanos = self.stats.last_wait_time.load(std::sync::atomic::Ordering::Relaxed);
                if nanos > 0 {
                    Some(Duration::from_nanos(nanos))
                } else {
                    None
                }
            },
        }
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.total_acquisitions.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.timeout_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.total_wait_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.max_wait_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.total_hold_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.max_hold_time.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.current_waiters.store(0, std::sync::atomic::Ordering::Relaxed);
        self.stats.last_wait_time.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// 更新最大等待时间
    fn update_max_wait_time(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;
        loop {
            let current_max = self.stats.max_wait_time.load(std::sync::atomic::Ordering::Relaxed);
            match nanos.cmp(&current_max) {
                Ordering::Less | Ordering::Equal => break,
                Ordering::Greater => {
                    match self.stats.max_wait_time.compare_exchange_weak(
                        current_max,
                        nanos,
                        std::sync::atomic::Ordering::Relaxed,
                        std::sync::atomic::Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        }

        // 更新总等待时间
        self.stats.total_wait_time.fetch_add(nanos, std::sync::atomic::Ordering::Relaxed);
    }
}

impl<T> Clone for AsyncMutex<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            default_timeout: self.default_timeout,
            id: self.id.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// 互斥锁守卫
pub struct AsyncMutexGuard<'a, T> {
    inner: TokioMutex::Guard<'a, T>,
    acquired_at: Instant,
    stats: Arc<LockStatsAtomic>,
}

impl<T> Deref for AsyncMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T> DerefMut for AsyncMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<T> Drop for AsyncMutexGuard<'_, T> {
    fn drop(&mut self) {
        let hold_time = self.acquired_at.elapsed();

        // 更新持有时间统计
        let total_nanos = hold_time.as_nanos() as u64;
        self.stats.total_hold_time.fetch_add(total_nanos, std::sync::atomic::Ordering::Relaxed);

        // 更新最大持有时间
        loop {
            let current_max = self.stats.max_hold_time.load(std::sync::atomic::Ordering::Relaxed);
            match total_nanos.cmp(&current_max) {
                Ordering::Less | Ordering::Equal => break,
                Ordering::Greater => {
                    match self.stats.max_hold_time.compare_exchange_weak(
                        current_max,
                        total_nanos,
                        std::sync::atomic::Ordering::Relaxed,
                        std::sync::atomic::Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        }

        tracing::trace!("Mutex lock released, held for {:?}", hold_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_mutex_lock() {
        let mutex = AsyncMutex::new(42, Duration::from_secs(5));

        {
            let mut guard = mutex.lock_with_timeout().await.unwrap();
            assert_eq!(*guard, 42);
            *guard = 100;
        }

        let value = mutex.lock_with_timeout().await.unwrap();
        assert_eq!(*value, 100);
    }

    #[tokio::test]
    async fn test_mutex_timeout() {
        let mutex = AsyncMutex::new(42, Duration::from_millis(100));

        // 获取锁
        let guard = mutex.lock_with_timeout().await.unwrap();

        // 在另一个协程中尝试获取锁，应该超时
        let mutex_clone = mutex.clone();
        let handle = tokio::spawn(async move {
            let result = mutex_clone
                .lock_with_custom_timeout(Duration::from_millis(50))
                .await;
            result
        });

        let result = handle.await.unwrap();
        assert!(result.is_err());

        drop(guard);

        // 统计应该显示超时
        let stats = mutex.stats();
        assert_eq!(stats.timeout_count, 1);
    }

    #[tokio::test]
    async fn test_mutex_exclusive_access() {
        let mutex = AsyncMutex::new(vec![1, 2, 3], Duration::from_secs(5));

        let mutex1 = mutex.clone();
        let mutex2 = mutex.clone();

        let h1 = tokio::spawn(async move {
            let mut guard = mutex1.lock_with_timeout().await.unwrap();
            guard.push(4);
            sleep(Duration::from_millis(100)).await;
            guard.push(5);
        });

        let h2 = tokio::spawn(async move {
            sleep(Duration::from_millis(50)).await;
            let mut guard = mutex2.lock_with_timeout().await.unwrap();
            guard.push(6);
        });

        h1.await.unwrap();
        h2.await.unwrap();

        let result = mutex.lock_with_timeout().await.unwrap();
        assert_eq!(*result, vec![1, 2, 3, 4, 5, 6]);
    }

    #[tokio::test]
    async fn test_mutex_stats() {
        let mutex = AsyncMutex::new(42, Duration::from_secs(5));

        // 多次获取锁
        for _ in 0..5 {
            let _guard = mutex.lock_with_timeout().await.unwrap();
            drop(_guard);
        }

        let stats = mutex.stats();
        assert_eq!(stats.total_acquisitions, 5);
        assert_eq!(stats.timeout_count, 0);
        assert!(stats.avg_hold_time() > Duration::ZERO);
    }
}
