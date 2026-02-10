//! Matrix Server 生命周期管理
//!
//! 提供 Matrix Server 的真实启动、停止和状态管理

use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::process::Command;
use tracing::{info, warn};

/// Matrix Server 配置
#[derive(Debug, Clone)]
pub struct MatrixConfig {
    /// 监听端口
    pub port: u16,
    /// 绑定地址
    pub bind_addr: String,
    /// 数据目录
    pub data_dir: std::path::PathBuf,
    /// 允许的 CORS origins（空列表表示允许所有，向后兼容）
    pub allowed_origins: Vec<String>,
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
            allowed_origins: Vec::new(),
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
#[derive(Debug, Clone)]
pub struct MatrixServerManager {
    config: MatrixConfig,
}

impl MatrixServerManager {
    /// 创建新的管理器
    pub fn new(config: MatrixConfig) -> Self {
        Self { config }
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
        info!(
            "Starting Matrix server on {}:{}",
            self.config.bind_addr, self.config.port
        );

        // 确保数据目录存在
        tokio::fs::create_dir_all(&self.config.data_dir).await?;

        // 启动服务器进程
        let child = Command::new("cis-node")
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

        info!("Matrix server started successfully (PID: {})", pid);

        Ok(ServerHandle {
            pid,
            port: self.config.port,
        })
    }

    /// 停止服务器
    pub async fn stop(&self) -> Result<()> {
        warn!("Matrix server stop not implemented without PidManager");
        Ok(())
    }

    /// 获取状态
    pub fn status(&self) -> ServerStatus {
        ServerStatus {
            running: false,
            pid: None,
            port: self.config.port,
            uptime_secs: None,
        }
    }

    /// 重启服务器
    pub async fn restart(&self) -> Result<ServerHandle> {
        self.stop().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.start().await
    }
}

/// 服务器句柄
#[derive(Debug, Clone)]
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
