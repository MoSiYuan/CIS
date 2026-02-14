// cis-core/src/lock_timeout/mod.rs
//
// 锁超时机制模块
//
// 提供带超时功能的锁包装器，防止死锁和长期阻塞

pub mod async_rwlock;
pub mod async_mutex;
pub mod monitor;

pub use async_rwlock::{
    AsyncRwLock,
    AsyncRwLockReadGuard,
    AsyncRwLockWriteGuard,
    LockType,
    LockTimeoutError,
    LockId,
    LockStats,
};

pub use async_mutex::{AsyncMutex, AsyncMutexGuard};

pub use monitor::{LockMonitor, LockReport, ContentionLevel};

use std::time::Duration;

/// 默认锁超时时间（用于一般场景）
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// 短期操作锁超时时间（用于快速操作）
pub const SHORT_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

/// 长期操作锁超时时间（用于耗时操作）
pub const LONG_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// 锁竞争阈值（用于告警）
pub const CONTENTION_THRESHOLD_MS: u64 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_constants() {
        assert!(DEFAULT_LOCK_TIMEOUT >= Duration::from_secs(1));
        assert!(SHORT_LOCK_TIMEOUT < DEFAULT_LOCK_TIMEOUT);
        assert!(LONG_LOCK_TIMEOUT > DEFAULT_LOCK_TIMEOUT);
    }
}
