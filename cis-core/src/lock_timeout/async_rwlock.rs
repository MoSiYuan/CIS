// cis-core/src/lock_timeout/async_rwlock.rs
//
// 带超时功能的读写锁包装器
// 基于 tokio::sync::RwLock，添加超时和监控功能

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock as TokioRwLock;
use tokio::time::timeout;

use crate::error::{CisError, Result};

/// 锁类型标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockType {
    ReadLock,
    WriteLock,
}

/// 锁的唯一标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockId {
    /// 文件路径
    pub file: String,
    /// 变量名
    pub variable: String,
    /// 代码行号（可选）
    pub line: Option<u32>,
}

impl LockId {
    pub fn new(file: &str, variable: &str) -> Self {
        Self {
            file: file.to_string(),
            variable: variable.to_string(),
            line: None,
        }
    }

    pub fn with_line(file: &str, variable: &str, line: u32) -> Self {
        Self {
            file: file.to_string(),
            variable: variable.to_string(),
            line: Some(line),
        }
    }
}

impl std::fmt::Display for LockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            write!(f, "{}:{} ({})", self.file, line, self.variable)
        } else {
            write!(f, "{} ({})", self.file, self.variable)
        }
    }
}

/// 锁超时错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTimeoutError {
    Timeout { lock_type: LockType, timeout: Duration },
    Cancelled,
}

impl std::fmt::Display for LockTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { lock_type, timeout } => {
                write!(f, "{:?} acquisition timeout after {:?}", lock_type, timeout)
            }
            Self::Cancelled => {
                write!(f, "Lock acquisition cancelled")
            }
        }
    }
}

impl std::error::Error for LockTimeoutError {}

impl From<LockTimeoutError> for CisError {
    fn from(err: LockTimeoutError) -> Self {
        CisError::LockTimeout(err.to_string())
    }
}

/// 锁统计信息
#[derive(Debug, Clone, Default)]
pub struct LockStats {
    /// 锁获取总次数
    pub total_acquisitions: u64,

    /// 锁超时次数
    pub timeout_count: u64,

    /// 总等待时间
    total_wait_time: Duration,

    /// 最大等待时间
    max_wait_time: Duration,

    /// 总持有时间
    total_hold_time: Duration,

    /// 最大持有时间
    max_hold_time: Duration,

    /// 当前等待的协程数
    pub current_waiters: u32,

    /// 最后一次等待时间
    last_wait_time: Option<Duration>,
}

impl LockStats {
    /// 记录锁获取开始
    pub fn start_wait(&self) -> WaitToken {
        self.current_waiters.fetch_add(
            1,
            std::sync::atomic::Ordering::Relaxed
        );
        WaitToken {
            start: Instant::now(),
            waiters: &self.current_waiters,
        }
    }

    /// 记录锁获取成功
    pub fn record_acquisition(&self, wait_token: WaitToken) {
        let wait_time = wait_token.start.elapsed();

        // 更新统计（使用原子操作避免竞争）
        self.total_acquisitions.fetch_add(
            1,
            std::sync::atomic::Ordering::Relaxed
        );

        // 更新等待时间
        Self::update_duration(&self.total_wait_time, &self.max_wait_time, wait_time);

        self.last_wait_time.store(
            wait_time.as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed
        );
    }

    /// 记录锁超时
    pub fn record_timeout(&self, wait_token: WaitToken) {
        let wait_time = wait_token.start.elapsed();

        self.timeout_count.fetch_add(
            1,
            std::sync::atomic::Ordering::Relaxed
        );

        Self::update_duration(&self.total_wait_time, &self.max_wait_time, wait_time);
    }

    /// 记录锁释放
    pub fn record_release(&self, hold_time: Duration) {
        Self::update_duration(&self.total_hold_time, &self.max_hold_time, hold_time);
    }

    /// 获取平均等待时间
    pub fn avg_wait_time(&self) -> Duration {
        let count = self.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed);
        if count == 0 {
            return Duration::ZERO;
        }

        let total_nanos = self.total_wait_time.load(std::sync::atomic::Ordering::Relaxed);
        Duration::from_nanos(total_nanos / count)
    }

    /// 获取平均持有时间
    pub fn avg_hold_time(&self) -> Duration {
        let count = self.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed);
        if count == 0 {
            return Duration::ZERO;
        }

        let total_nanos = self.total_hold_time.load(std::sync::atomic::Ordering::Relaxed);
        Duration::from_nanos(total_nanos / count)
    }

    /// 更新持续时间统计（辅助方法）
    fn update_duration(
        total: &std::sync::atomic::AtomicU64,
        max: &std::sync::atomic::AtomicU64,
        duration: Duration,
    ) {
        let nanos = duration.as_nanos() as u64;

        // 更新总时间
        total.fetch_add(nanos, std::sync::atomic::Ordering::Relaxed);

        // 更新最大时间
        loop {
            let current_max = max.load(std::sync::atomic::Ordering::Relaxed);
            if nanos <= current_max {
                break;
            }
            match max.compare_exchange_weak(
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

    /// 打印统计信息
    pub fn print(&self) {
        println!(
            "Lock Statistics:\n\
             - Total acquisitions: {}\n\
             - Timeouts: {} ({:.2}%)\n\
             - Avg wait time: {:?}\n\
             - Max wait time: {:?}\n\
             - Avg hold time: {:?}\n\
             - Max hold time: {:?}\n\
             - Current waiters: {}",
            self.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed),
            self.timeout_count.load(std::sync::atomic::Ordering::Relaxed),
            (self.timeout_count.load(std::sync::atomic::Ordering::Relaxed) as f64
                / self.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed).max(1) as f64)
                * 100.0,
            self.avg_wait_time(),
            Duration::from_nanos(self.max_wait_time.load(std::sync::atomic::Ordering::Relaxed)),
            self.avg_hold_time(),
            Duration::from_nanos(self.max_hold_time.load(std::sync::atomic::Ordering::Relaxed)),
            self.current_waiters.load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}

/// 等待令牌（用于统计）
pub struct WaitToken<'a> {
    start: Instant,
    waiters: &'a std::sync::atomic::AtomicU32,
}

impl Drop for WaitToken<'_> {
    fn drop(&mut self) {
        self.waiters.fetch_sub(
            1,
            std::sync::atomic::Ordering::Relaxed
        );
    }
}

/// 带超时的读写锁
pub struct AsyncRwLock<T> {
    inner: Arc<TokioRwLock<T>>,
    default_timeout: Duration,
    id: Option<LockId>,
    stats: Arc<LockStatsAtomic>,
}

/// 原子版本的锁统计（用于并发更新）
struct LockStatsAtomic {
    total_acquisitions: std::sync::atomic::AtomicU64,
    timeout_count: std::sync::atomic::AtomicU64,
    total_wait_time: std::sync::atomic::AtomicU64,
    max_wait_time: std::sync::atomic::AtomicU64,
    total_hold_time: std::sync::atomic::AtomicU64,
    max_hold_time: std::sync::atomic::AtomicU64,
    current_waiters: std::sync::atomic::AtomicU32,
    last_wait_time: std::sync::atomic::AtomicU64,
}

impl Default for LockStatsAtomic {
    fn default() -> Self {
        Self {
            total_acquisitions: std::sync::atomic::AtomicU64::new(0),
            timeout_count: std::sync::atomic::AtomicU64::new(0),
            total_wait_time: std::sync::atomic::AtomicU64::new(0),
            max_wait_time: std::sync::atomic::AtomicU64::new(0),
            total_hold_time: std::sync::atomic::AtomicU64::new(0),
            max_hold_time: std::sync::atomic::AtomicU64::new(0),
            current_waiters: std::sync::atomic::AtomicU32::new(0),
            last_wait_time: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl<T> AsyncRwLock<T> {
    /// 创建新的带超时的读写锁
    pub fn new(value: T, default_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(TokioRwLock::new(value)),
            default_timeout,
            id: None,
            stats: Arc::new(LockStatsAtomic::default()),
        }
    }

    /// 创建带有监控标识的锁
    pub fn with_id(value: T, default_timeout: Duration, id: LockId) -> Self {
        Self {
            inner: Arc::new(TokioRwLock::new(value)),
            default_timeout,
            id: Some(id),
            stats: Arc::new(LockStatsAtomic::default()),
        }
    }

    /// 使用默认超时获取读锁
    pub async fn read_with_timeout(&self) -> Result<AsyncRwLockReadGuard<T>, LockTimeoutError> {
        self.read_with_custom_timeout(self.default_timeout).await
    }

    /// 使用自定义超时获取读锁
    pub async fn read_with_custom_timeout(
        &self,
        timeout_duration: Duration,
    ) -> Result<AsyncRwLockReadGuard<T>, LockTimeoutError> {
        let start = Instant::now();
        self.stats.current_waiters.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let result = timeout(timeout_duration, self.inner.read()).await;

        self.stats.current_waiters.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        match result {
            Ok(guard) => {
                let wait_time = start.elapsed();
                self.stats.total_acquisitions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.update_max_wait_time(wait_time);

                tracing::trace!(
                    "Read lock acquired for {:?} after {:?}",
                    self.id,
                    wait_time
                );

                Ok(AsyncRwLockReadGuard {
                    inner: guard,
                    acquired_at: Instant::now(),
                    stats: self.stats.clone(),
                })
            }
            Err(_) => {
                let wait_time = start.elapsed();
                self.stats.timeout_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                tracing::warn!(
                    "Read lock timeout for {:?} after {:?} (timeout: {:?})",
                    self.id,
                    wait_time,
                    timeout_duration
                );

                Err(LockTimeoutError::Timeout {
                    lock_type: LockType::ReadLock,
                    timeout: timeout_duration,
                })
            }
        }
    }

    /// 使用默认超时获取写锁
    pub async fn write_with_timeout(&self) -> Result<AsyncRwLockWriteGuard<T>, LockTimeoutError> {
        self.write_with_custom_timeout(self.default_timeout).await
    }

    /// 使用自定义超时获取写锁
    pub async fn write_with_custom_timeout(
        &self,
        timeout_duration: Duration,
    ) -> Result<AsyncRwLockWriteGuard<T>, LockTimeoutError> {
        let start = Instant::now();
        self.stats.current_waiters.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let result = timeout(timeout_duration, self.inner.write()).await;

        self.stats.current_waiters.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        match result {
            Ok(guard) => {
                let wait_time = start.elapsed();
                self.stats.total_acquisitions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.update_max_wait_time(wait_time);

                tracing::trace!(
                    "Write lock acquired for {:?} after {:?}",
                    self.id,
                    wait_time
                );

                Ok(AsyncRwLockWriteGuard {
                    inner: guard,
                    acquired_at: Instant::now(),
                    stats: self.stats.clone(),
                })
            }
            Err(_) => {
                let wait_time = start.elapsed();
                self.stats.timeout_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                tracing::warn!(
                    "Write lock timeout for {:?} after {:?} (timeout: {:?})",
                    self.id,
                    wait_time,
                    timeout_duration
                );

                Err(LockTimeoutError::Timeout {
                    lock_type: LockType::WriteLock,
                    timeout: timeout_duration,
                })
            }
        }
    }

    /// 获取锁统计信息
    pub fn stats(&self) -> LockStats {
        LockStats {
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

impl<T> Clone for AsyncRwLock<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            default_timeout: self.default_timeout,
            id: self.id.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// 读锁守卫
pub struct AsyncRwLockReadGuard<'a, T> {
    inner: TokioRwLock::ReadGuard<'a, T>,
    acquired_at: Instant,
    stats: Arc<LockStatsAtomic>,
}

impl<T> Deref for AsyncRwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T> Drop for AsyncRwLockReadGuard<'_, T> {
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

        tracing::trace!("Read lock released, held for {:?}", hold_time);
    }
}

/// 写锁守卫
pub struct AsyncRwLockWriteGuard<'a, T> {
    inner: TokioRwLock::WriteGuard<'a, T>,
    acquired_at: Instant,
    stats: Arc<LockStatsAtomic>,
}

impl<T> Deref for AsyncRwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T> DerefMut for AsyncRwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<T> Drop for AsyncRwLockWriteGuard<'_, T> {
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

        tracing::trace!("Write lock released, held for {:?}", hold_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_basic_read_lock() {
        let lock = AsyncRwLock::new(42, Duration::from_secs(5));

        {
            let guard = lock.read_with_timeout().await.unwrap();
            assert_eq!(*guard, 42);
        }

        // 统计应该记录一次获取
        let stats = lock.stats();
        assert_eq!(stats.total_acquisitions, 1);
        assert_eq!(stats.timeout_count, 0);
    }

    #[tokio::test]
    async fn test_basic_write_lock() {
        let lock = AsyncRwLock::new(42, Duration::from_secs(5));

        {
            let mut guard = lock.write_with_timeout().await.unwrap();
            *guard = 100;
        }

        let value = lock.read_with_timeout().await.unwrap();
        assert_eq!(*value, 100);
    }

    #[tokio::test]
    async fn test_lock_timeout() {
        let lock = AsyncRwLock::new(42, Duration::from_millis(100));

        // 获取写锁
        let write_guard = lock.write_with_timeout().await.unwrap();

        // 在另一个协程中尝试获取写锁，应该超时
        let lock_clone = lock.clone();
        let handle = tokio::spawn(async move {
            let result = lock_clone
                .write_with_custom_timeout(Duration::from_millis(50))
                .await;
            result
        });

        let result = handle.await.unwrap();
        assert!(result.is_err());

        drop(write_guard);

        // 统计应该显示超时
        let stats = lock.stats();
        assert_eq!(stats.timeout_count, 1);
    }

    #[tokio::test]
    async fn test_multiple_readers() {
        let lock = AsyncRwLock::new(42, Duration::from_secs(5));

        let lock1 = lock.clone();
        let lock2 = lock.clone();
        let lock3 = lock.clone();

        let h1 = tokio::spawn(async move { lock1.read_with_timeout().await });
        let h2 = tokio::spawn(async move { lock2.read_with_timeout().await });
        let h3 = tokio::spawn(async move { lock3.read_with_timeout().await });

        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();
        let r3 = h3.await.unwrap();

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());

        let stats = lock.stats();
        assert_eq!(stats.total_acquisitions, 3);
    }

    #[tokio::test]
    async fn test_writer_blocks_readers() {
        let lock = AsyncRwLock::new(42, Duration::from_secs(5));

        // 获取写锁
        let write_guard = lock.write_with_timeout().await.unwrap();

        let lock_clone = lock.clone();
        let handle = tokio::spawn(async move {
            // 尝试获取读锁，应该被阻塞
            let result = lock_clone
                .read_with_custom_timeout(Duration::from_millis(100))
                .await;
            result
        });

        // 等待超时
        let result = handle.await.unwrap();
        assert!(result.is_err());

        drop(write_guard);
    }
}
