//! PID 文件管理器
//!
//! 跨平台进程管理

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// 进程信号
#[derive(Debug, Clone, Copy)]
pub enum ProcessSignal {
    Term,  // SIGTERM
    Kill,  // SIGKILL
    Hup,   // SIGHUP
}

/// 进程状态
#[derive(Debug, Clone)]
pub struct ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub start_time: Option<std::time::SystemTime>,
}

/// PID 管理器
pub struct PidManager {
    name: String,
    pid_file: PathBuf,
}

impl PidManager {
    /// 创建 PID 管理器
    pub fn new(name: &str) -> Self {
        let pid_file = Self::pid_file_path(name);
        Self {
            name: name.to_string(),
            pid_file,
        }
    }
    
    /// 获取 PID 文件路径
    fn pid_file_path(name: &str) -> PathBuf {
        // XDG 运行时目录
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(runtime_dir).join(format!("cis-{}.pid", name));
        }
        
        // 回退到用户目录
        let home = dirs::home_dir().expect("Home directory not found");
        
        #[cfg(target_os = "macos")]
        {
            home.join("Library/Run").join(format!("cis-{}.pid", name))
        }
        
        #[cfg(target_os = "linux")]
        {
            home.join(".local/run").join(format!("cis-{}.pid", name))
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            home.join(format!("cis-{}.pid", name))
        }
    }
    
    /// 写入 PID
    pub fn write(&self) -> Result<()> {
        let pid = std::process::id();
        let content = format!("{}\n{}\n", pid, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
        
        // 确保目录存在
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(&self.pid_file, content)
            .with_context(|| format!("Failed to write PID file: {:?}", self.pid_file))?;
        
        info!("PID {} written to {:?}", pid, self.pid_file);
        Ok(())
    }
    
    /// 读取 PID
    pub fn read(&self) -> Result<Option<u32>> {
        if !self.pid_file.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(&self.pid_file)
            .with_context(|| format!("Failed to read PID file: {:?}", self.pid_file))?;
        
        let pid: u32 = content.lines()
            .next()
            .ok_or_else(|| anyhow!("Empty PID file"))?
            .parse()
            .map_err(|e| anyhow!("Invalid PID: {}", e))?;
        
        Ok(Some(pid))
    }
    
    /// 检查进程是否运行
    pub fn is_running(&self) -> bool {
        match self.read() {
            Ok(Some(pid)) => Self::check_process(pid),
            _ => false,
        }
    }
    
    /// 获取状态
    pub fn status(&self) -> ProcessStatus {
        match self.read() {
            Ok(Some(pid)) => ProcessStatus {
                running: Self::check_process(pid),
                pid: Some(pid),
                start_time: None, // 可以从文件读取
            },
            _ => ProcessStatus {
                running: false,
                pid: None,
                start_time: None,
            },
        }
    }
    
    /// 发送信号
    pub fn signal(&self, signal: ProcessSignal) -> Result<bool> {
        let pid = match self.read()? {
            Some(pid) => pid,
            None => return Ok(false),
        };
        
        #[cfg(unix)]
        {
            use libc::{kill, SIGTERM, SIGKILL, SIGHUP};
            
            let sig = match signal {
                ProcessSignal::Term => SIGTERM,
                ProcessSignal::Kill => SIGKILL,
                ProcessSignal::Hup => SIGHUP,
            };
            
            let result = unsafe { kill(pid as i32, sig) };
            
            if result == 0 {
                info!("Signal {:?} sent to process {}", signal, pid);
                Ok(true)
            } else {
                Err(anyhow!("Failed to send signal to process {}: {}", pid, std::io::Error::last_os_error()))
            }
        }
        
        #[cfg(windows)]
        {
            // Windows 实现
            anyhow::bail!("Windows not yet supported")
        }
    }
    
    /// 停止进程
    pub fn stop(&self, timeout: Duration) -> Result<bool> {
        if !self.is_running() {
            return Ok(false);
        }
        
        // 发送 SIGTERM
        self.signal(ProcessSignal::Term)?;
        
        // 等待进程退出
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !self.is_running() {
                info!("Process stopped gracefully");
                return Ok(true);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        
        // 超时，发送 SIGKILL
        warn!("Timeout, force killing process");
        self.signal(ProcessSignal::Kill)?;
        
        Ok(true)
    }
    
    /// 清理 PID 文件
    pub fn cleanup(&self) -> Result<()> {
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file)
                .with_context(|| format!("Failed to remove PID file: {:?}", self.pid_file))?;
            info!("PID file removed: {:?}", self.pid_file);
        }
        Ok(())
    }
    
    /// 检查进程是否存在
    fn check_process(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use libc::{kill, ESRCH};
            let result = unsafe { kill(pid as i32, 0) };
            result == 0
        }
        
        #[cfg(windows)]
        {
            // Windows 实现
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pid_manager_creation() {
        let manager = PidManager::new("test");
        assert!(!manager.pid_file.as_os_str().is_empty());
    }
    
    #[test]
    fn test_write_and_read() {
        let manager = PidManager::new("test-write");
        
        // 写入
        manager.write().unwrap();
        
        // 读取
        let pid = manager.read().unwrap();
        assert_eq!(pid, Some(std::process::id()));
        
        // 清理
        manager.cleanup().unwrap();
    }
}
