//! 🔒 文件描述符RAII守护 (P0安全修复)
//!
//! 确保文件描述符总是被正确释放，防止资源泄漏

use std::sync::atomic::{AtomicU32, Ordering};
use std::ops::Drop;

/// 🔒 文件描述符守卫（RAII模式）
pub struct FileDescriptorGuard<'a> {
    count: &'a AtomicU32,
    acquired: bool,
}

impl<'a> FileDescriptorGuard<'a> {
    /// 分配文件描述符
    pub fn acquire(count: &'a AtomicU32, max: u32) -> Option<Self> {
        let current = count.fetch_add(1, Ordering::SeqCst);
        
        if current >= max {
            // 超过限制，回退
            count.fetch_sub(1, Ordering::SeqCst);
            tracing::warn!(
                "File descriptor limit exceeded: {} (max: {})",
                current + 1,
                max
            );
            return None;
        }
        
        tracing::debug!("Allocated fd: {}/{}", current + 1, max);
        Some(Self {
            count,
            acquired: true,
        })
    }
    
    /// 检查是否已获取
    pub fn is_acquired(&self) -> bool {
        self.acquired
    }
}

impl<'a> Drop for FileDescriptorGuard<'a> {
    fn drop(&mut self) {
        if self.acquired {
            let current = self.count.fetch_sub(1, Ordering::SeqCst);
            tracing::debug!("Released fd: {}", current);
            self.acquired = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn test_fd_guard_acquisition() {
        let count = AtomicU32::new(0);
        let max = 2;
        
        {
            let _guard1 = FileDescriptorGuard::acquire(&count, max).unwrap();
            assert_eq!(count.load(Ordering::SeqCst), 1);
            
            let _guard2 = FileDescriptorGuard::acquire(&count, max).unwrap();
            assert_eq!(count.load(Ordering::SeqCst), 2);
            
            // 第3个应该失败
            let guard3 = FileDescriptorGuard::acquire(&count, max);
            assert!(guard3.is_none());
        }
        
        // guard1和guard2释放后
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
