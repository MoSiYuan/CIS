//! Matrix Server 生命周期管理
//!
//! 提供 Matrix Server 的真实启动、停止和状态管理

use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

use crate::system::pid_manager::PidManager;

/// Matrix Server 配置
#[derive(Debug, Clone)]
pub struct MatrixConfig {
    /// 监听端口
    pub port: u16,
    /// 绑定地址
    pub bind_addr: String,
    /// 数据目录
    pub data_dir: std::path::PathBuf,
}

impl Default for MatrixConfig {
    fn default() -> Self {
        Self {
            port: 7676,
            bind_addr: "0.0.0.0".to_string(),
            data_dir: dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("cis")
                .join("matrix"),
        }
    }
}

/// 服务器状态
#[derive(Debug, Clone)]
pub struct ServerStatus {
    /// 是否运行中
    pub running: bool,
    /// 进程 ID
    pub pid: Option<u32>,
    /// 监听端口
    pub port: u16,
    /// 运行时长（秒）
    pub uptime_secs: Option<u64>,
}

/// Matrix Server 管理器
pub struct MatrixServerManager {
    pid_manager: PidManager,
    config: MatrixConfig,
}

impl MatrixServerManager {
    /// 创建新的管理器
    pub fn new(config: MatrixConfig) -> Self {
        let pid_manager = PidManager::new("matrix-server");
        Self {
            pid_manager,
            config,
        }
    }

    /// 使用默认配置创建
    pub fn default_with_port(port: u16) -> Self {
        let config = MatrixConfig {
            port,
            ..Default::default()
        };
        Self::new(config)
    }

    /// 启动 Matrix Server
    ///
    /// 阻塞直到服务器启动成功或失败
    pub async fn start(&self) -> Result<ServerHandle> {
        // 检查是否已在运行
        if let Some(pid) = self.pid_manager.read()? {
            if self.pid_manager.is_running() {
                info!("Matrix server already running (PID: {})", pid);
                return Ok(ServerHandle {
                    pid,
                    port: self.config.port,
                });
            } else {
                // 清理僵尸 PID 文件
                self.pid_manager.cleanup()?;
            }
        }

        info!(
            "Starting Matrix server on {}:{}",
            self.config.bind_addr, self.config.port
        );

        // 确保数据目录存在
        tokio::fs::create_dir_all(&self.config.data_dir).await?;

        // 启动服务器进程
        let mut child = Command::new("cis-node")
            .arg("matrix")
            .arg("start")
            .arg("--port")
            .arg(self.config.port.to_string())
            .arg("--daemon")
            .current_dir(&self.config.data_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn Matrix server process")?;

        let pid = child.id().ok_or_else(|| anyhow!("Failed to get child PID"))?;

        // 等待启动确认
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 检查进程是否还在运行
        if !Self::is_process_running(pid) {
            let exit_status = child.try_wait()?;
            return Err(anyhow!(
                "Matrix server process exited early: {:?}",
                exit_status
            ));
        }

        // 写入 PID 文件
        self.pid_manager.write()?;

        info!("Matrix server started successfully (PID: {})", pid);

        Ok(ServerHandle {
            pid,
            port: self.config.port,
        })
    }

    /// 停止服务器
    pub async fn stop(&self) -> Result<()> {
        match self.pid_manager.read()? {
            Some(pid) => {
                info!("Stopping Matrix server (PID: {})...", pid);

                // 发送 SIGTERM
                self.pid_manager
                    .signal(crate::system::pid_manager::ProcessSignal::Term)?;

                // 等待进程退出
                let timeout = Duration::from_secs(10);
                let start = std::time::Instant::now();

                while start.elapsed() < timeout {
                    if !self.pid_manager.is_running() {
                        info!("Matrix server stopped gracefully");
                        self.pid_manager.cleanup()?;
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }

                // 超时，强制停止
                warn!("Timeout, force killing Matrix server");
                self.pid_manager
                    .signal(crate::system::pid_manager::ProcessSignal::Kill)?;
                self.pid_manager.cleanup()?;

                Ok(())
            }
            None => {
                warn!("Matrix server not running");
                Ok(())
            }
        }
    }

    /// 获取状态
    pub fn status(&self) -> ServerStatus {
        match self.pid_manager.read() {
            Ok(Some(pid)) => {
                let running = self.pid_manager.is_running();
                ServerStatus {
                    running,
                    pid: Some(pid),
                    port: self.config.port,
                    uptime_secs: None, // TODO: 从 PID 文件读取启动时间
                }
            }
            _ => ServerStatus {
                running: false,
                pid: None,
                port: self.config.port,
                uptime_secs: None,
            },
        }
    }

    /// 重启服务器
    pub async fn restart(&self) -> Result<ServerHandle> {
        self.stop().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.start().await
    }

    /// 检查进程是否运行
    fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use libc::kill;
            unsafe { kill(pid as i32, 0) == 0 }
        }
        #[cfg(windows)]
        {
            // Windows 实现
            false
        }
    }
}

/// 服务器句柄
pub struct ServerHandle {
    /// 进程 ID
    pub pid: u32,
    /// 监听端口
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_config_default() {
        let config = MatrixConfig::default();
        assert_eq!(config.port, 7676);
    }

    #[test]
    fn test_server_status() {
        let status = ServerStatus {
            running: true,
            pid: Some(1234),
            port: 7676,
            uptime_secs: Some(100),
        };
        assert!(status.running);
        assert_eq!(status.pid, Some(1234));
    }
}
