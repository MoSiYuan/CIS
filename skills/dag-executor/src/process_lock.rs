//! # 进程文件锁
//!
//! 实现跨平台的进程间互斥锁，用于确保同一 Worker 只有一个实例在运行。
//!
//! ## 使用场景
//! - 防止重复启动相同 worker
//! - 检测孤儿进程
//! - 实现进程存活检查

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// 进程锁
pub struct ProcessLock {
    /// 锁文件路径
    lock_path: PathBuf,
    /// 锁文件句柄（保持打开状态）
    #[allow(dead_code)]
    lock_file: File,
}

impl ProcessLock {
    /// 尝试获取锁
    ///
    /// 如果锁已被其他进程持有，返回 None
    pub fn try_acquire(worker_id: &str) -> Option<Self> {
        let lock_path = Self::lock_path(worker_id);
        
        // 检查是否已有锁文件
        if lock_path.exists() {
            // 检查持有锁的进程是否仍然存活
            if let Some(pid) = Self::read_lock_pid(&lock_path) {
                if Self::is_process_alive(pid) {
                    warn!("Worker {} is already running (pid: {})", worker_id, pid);
                    return None;
                } else {
                    // 进程已死亡，但锁文件残留（孤儿锁）
                    info!("Removing stale lock for worker {} (dead pid: {})", worker_id, pid);
                    let _ = std::fs::remove_file(&lock_path);
                }
            }
        }

        // 创建锁文件
        match File::create(&lock_path) {
            Ok(mut file) => {
                // 写入当前进程 PID
                let pid = std::process::id();
                if let Err(e) = writeln!(file, "{}", pid) {
                    error!("Failed to write lock file: {}", e);
                    return None;
                }

                // 尝试获取文件锁（Unix）
                #[cfg(unix)]
                {
                    use std::os::unix::io::AsRawFd;
                    let fd = file.as_raw_fd();
                    let result = unsafe {
                        libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB)
                    };
                    if result != 0 {
                        warn!("Failed to acquire file lock for worker {}", worker_id);
                        let _ = std::fs::remove_file(&lock_path);
                        return None;
                    }
                }

                debug!("Acquired lock for worker {} (pid: {})", worker_id, pid);
                
                Some(Self {
                    lock_path,
                    lock_file: file,
                })
            }
            Err(e) => {
                error!("Failed to create lock file: {}", e);
                None
            }
        }
    }

    /// 释放锁
    pub fn release(self) {
        let worker_id = self.lock_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        
        // 删除锁文件
        if let Err(e) = std::fs::remove_file(&self.lock_path) {
            warn!("Failed to remove lock file for {}: {}", worker_id, e);
        } else {
            info!("Released lock for worker {}", worker_id);
        }
    }

    /// 检查是否已加锁（用于检测是否已有实例在运行）
    pub fn is_locked(worker_id: &str) -> bool {
        let lock_path = Self::lock_path(worker_id);
        
        if !lock_path.exists() {
            return false;
        }

        // 检查持有锁的进程是否存活
        if let Some(pid) = Self::read_lock_pid(&lock_path) {
            Self::is_process_alive(pid)
        } else {
            false
        }
    }

    /// 获取锁文件路径
    fn lock_path(worker_id: &str) -> PathBuf {
        let lock_dir = std::env::temp_dir().join("cis").join("worker_locks");
        // 确保目录存在
        let _ = std::fs::create_dir_all(&lock_dir);
        lock_dir.join(format!("{}.lock", worker_id.replace(':', "_")))
    }

    /// 读取锁文件中的 PID
    fn read_lock_pid(path: &PathBuf) -> Option<u32> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| content.trim().parse().ok())
    }

    /// 检查进程是否存活
    #[cfg(unix)]
    fn is_process_alive(pid: u32) -> bool {
        unsafe {
            libc::kill(pid as libc::pid_t, 0) == 0
        }
    }

    #[cfg(windows)]
    fn is_process_alive(pid: u32) -> bool {
        use windows_sys::Win32::System::Threading::OpenProcess;
        use windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
        use windows_sys::Win32::Foundation::CloseHandle;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle == 0 {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn is_process_alive(_pid: u32) -> bool {
        // 非 Unix/Windows 平台，默认认为进程存活
        true
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        // 尝试删除锁文件
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

/// 孤儿进程检测器
pub struct OrphanDetector {
    /// Worker ID
    worker_id: String,
    /// 父进程 PID
    parent_pid: u32,
    /// 检查间隔（秒）
    check_interval_secs: u64,
}

impl OrphanDetector {
    pub fn new(worker_id: String, check_interval_secs: u64) -> Self {
        Self {
            worker_id,
            parent_pid: std::process::id(),
            check_interval_secs,
        }
    }

    /// 启动孤儿检测循环
    /// 
    /// 如果检测到父进程死亡，调用回调函数
    pub async fn start<F>(self, on_orphan: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_secs(self.check_interval_secs)
        );

        loop {
            interval.tick().await;

            if !Self::is_process_alive(self.parent_pid) {
                warn!(
                    "Worker {} detected parent process (pid: {}) died, becoming orphan",
                    self.worker_id, self.parent_pid
                );
                on_orphan();
                break;
            }
        }
    }

    #[cfg(unix)]
    fn is_process_alive(pid: u32) -> bool {
        unsafe {
            libc::kill(pid as libc::pid_t, 0) == 0
        }
    }

    #[cfg(windows)]
    fn is_process_alive(pid: u32) -> bool {
        use windows_sys::Win32::System::Threading::OpenProcess;
        use windows_sys::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
        use windows_sys::Win32::Foundation::CloseHandle;

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle == 0 {
                return false;
            }
            CloseHandle(handle);
            true
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn is_process_alive(_pid: u32) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_path() {
        let path = ProcessLock::lock_path("worker-test");
        assert!(path.to_string_lossy().contains("worker-test.lock"));
        
        // 测试冒号被替换
        let path2 = ProcessLock::lock_path("worker:global");
        assert!(path2.to_string_lossy().contains("worker_global.lock"));
    }

    #[test]
    fn test_acquire_and_release() {
        let worker_id = "test-worker-1";
        
        // 确保没有遗留的锁
        let lock_path = ProcessLock::lock_path(worker_id);
        let _ = std::fs::remove_file(&lock_path);

        // 获取锁
        let lock = ProcessLock::try_acquire(worker_id);
        assert!(lock.is_some(), "Should acquire lock");

        // 再次获取应该失败
        let lock2 = ProcessLock::try_acquire(worker_id);
        assert!(lock2.is_none(), "Should not acquire same lock twice");

        // 释放锁
        drop(lock);

        // 等待文件系统同步
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 应该可以再次获取
        let lock3 = ProcessLock::try_acquire(worker_id);
        assert!(lock3.is_some(), "Should acquire lock after release");
        
        // 清理
        drop(lock3);
    }

    #[test]
    fn test_is_locked() {
        let worker_id = "test-worker-2";
        let lock_path = ProcessLock::lock_path(worker_id);
        let _ = std::fs::remove_file(&lock_path);

        assert!(!ProcessLock::is_locked(worker_id), "Should not be locked initially");

        let lock = ProcessLock::try_acquire(worker_id);
        assert!(lock.is_some());
        assert!(ProcessLock::is_locked(worker_id), "Should be locked after acquire");

        drop(lock);
    }
}
