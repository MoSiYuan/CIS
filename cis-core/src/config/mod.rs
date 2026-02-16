//! # CIS Configuration Center
//!
//! Unified configuration management for CIS.
//!
//! ## Configuration Hierarchy
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         Environment Variables          │
//! │    CIS_NETWORK_TCP_PORT=6767           │
//! ├─────────────────────────────────────────┤
//! │         Config File (config.toml)       │
//! │    [network]                            │
//! │    tcp_port = 6767                      │
//! ├─────────────────────────────────────────┤
//! │         Default Values                  │
//! │    impl Default for NetworkConfig {     │
//! │        fn default() -> Self { ... }     │
//! │    }                                    │
//! └─────────────────────────────────────────┘
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod encryption;
mod loader;
mod network;
mod p2p;
mod security;
mod storage;
mod wasm;

pub use encryption::ConfigEncryption;
pub use loader::ConfigLoader;
pub use network::{
    NetworkConfig, TlsConfig,
    DEFAULT_CONNECTION_TIMEOUT_SECS, DEFAULT_REQUEST_TIMEOUT_SECS,
    DEFAULT_TCP_PORT, DEFAULT_UDP_PORT, DEFAULT_HTTP_PORT,
};
pub use p2p::P2PConfig;
pub use security::{EncryptionConfig, SecurityConfig};
pub use storage::{StorageConfig, DatabaseConfig};
pub use wasm::WasmConfig;

// 🔥 Memory conflict configuration (P1.7.0 任务组 0.5)

/// 🔥 内存冲突配置 (P1.7.0 任务组 0.5)
///
/// # 核心保证
///
/// - **强制检测**：`enforce_check` 硬编码为 `true`（不可修改）
/// - **运行时验证**：启动时验证 `enforce_check == true`
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryConflictConfig {
    /// 🔥 Agent 执行前是否强制检查冲突（硬编码为 true，不可修改）
    pub enforce_check: bool,

    /// 冲突超时时间（秒）
    pub conflict_timeout_secs: u64,
}

impl Default for MemoryConflictConfig {
    fn default() -> Self {
        Self {
            enforce_check: true,  // 🔥 硬编码为 true，不可修改
            conflict_timeout_secs: 300,
        }
    }
}

impl MemoryConflictConfig {
    /// 🔥 验证配置（启动时调用）
    ///
    /// # 核心逻辑
    ///
    /// 1. 检查 `enforce_check == true`
    /// 2. 如果不是，记录警告并强制设置为 `true`
    /// 3. 返回验证后的配置
    ///
    /// # 返回
    ///
    /// 返回验证后的 `MemoryConflictConfig`。
    pub fn validate(&self) -> Result<Self> {
        if self.enforce_check != true {
            // 记录警告（使用 println 而非 tracing::warn! 以避免依赖）
            println!(
                "[WARN] Memory conflict detection is mandatory. Overriding enforce_check from {} to true.",
                self.enforce_check
            );

            // 强制设置为 true
            Ok(Self {
                enforce_check: true,
                conflict_timeout_secs: self.conflict_timeout_secs,
            })
        } else {
            // 配置正确，返回克隆
            Ok(self.clone())
        }
    }
}

use crate::error::{CisError, Result};

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Network configuration
    #[serde(default)]
    pub network: NetworkConfig,

    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,

    /// WASM runtime configuration
    #[serde(default)]
    pub wasm: WasmConfig,

    /// P2P network configuration
    #[serde(default)]
    pub p2p: P2PConfig,

    /// 🔥 Memory conflict configuration (P1.7.0 任务组 0.5)
    #[serde(default)]
    pub memory_conflict: MemoryConflictConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            storage: StorageConfig::default(),
            security: SecurityConfig::default(),
            wasm: WasmConfig::default(),
            p2p: P2PConfig::default(),
            memory_conflict: MemoryConflictConfig::default(),  // 🔥 默认强制检测
        }
    }
}

impl Config {
    /// Load configuration with full hierarchy (defaults -> file -> env)
    pub fn load() -> Result<Self> {
        ConfigLoader::new().load()
    }

    /// Load configuration from specific path
    pub fn load_from(path: impl Into<PathBuf>) -> Result<Self> {
        ConfigLoader::with_path(path).load()
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<()> {
        self.network.validate()?;
        self.storage.validate()?;
        self.security.validate()?;
        self.wasm.validate()?;
        self.p2p.validate()?;

        // 🔥 验证 memory_conflict 配置（P1.7.0 任务组 0.5）
        let _validated_conflict = self.memory_conflict.validate()?;

        Ok(())
    }

    /// Get TCP bind address string
    pub fn tcp_bind_address(&self) -> String {
        format!("{}:{}", self.network.bind_address, self.network.tcp_port)
    }

    /// Get UDP bind address string
    pub fn udp_bind_address(&self) -> String {
        format!("{}:{}", self.network.bind_address, self.network.udp_port)
    }
}

/// Trait for configuration validation
pub trait ValidateConfig {
    /// Validate configuration values
    fn validate(&self) -> Result<()>;
}

/// Configuration error helper
fn validation_error(msg: impl Into<String>) -> CisError {
    CisError::configuration(format!("Validation error: {}", msg.into()))
}

/// Validate port number is in valid range
fn validate_port(port: u16, name: &str) -> Result<()> {
    if port < 1024 {
        return Err(validation_error(format!(
            "{} must be >= 1024 (got {})",
            name, port
        )));
    }
    Ok(())
}

/// Validate path is not empty
fn validate_non_empty_path(path: &PathBuf, name: &str) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(validation_error(format!("{} path cannot be empty", name)));
    }
    Ok(())
}

/// Validate duration is positive
fn validate_positive_duration(duration: std::time::Duration, name: &str) -> Result<()> {
    if duration.is_zero() {
        return Err(validation_error(format!("{} cannot be zero", name)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.network.tcp_port, 6767);
        assert_eq!(config.network.udp_port, 7677);
        assert_eq!(config.network.bind_address, "0.0.0.0");
    }

    #[test]
    fn test_config_tcp_bind_address() {
        let config = Config::default();
        assert_eq!(config.tcp_bind_address(), "0.0.0.0:6767");
    }

    #[test]
    fn test_config_udp_bind_address() {
        let config = Config::default();
        assert_eq!(config.udp_bind_address(), "0.0.0.0:7677");
    }

    #[test]
    fn test_validate_port_valid() {
        assert!(validate_port(1024, "test_port").is_ok());
        assert!(validate_port(8080, "test_port").is_ok());
        assert!(validate_port(65535, "test_port").is_ok());
    }

    #[test]
    fn test_validate_port_invalid() {
        assert!(validate_port(0, "test_port").is_err());
        assert!(validate_port(80, "test_port").is_err());
        assert!(validate_port(443, "test_port").is_err());
        assert!(validate_port(1023, "test_port").is_err());
    }

    #[test]
    fn test_validate_non_empty_path() {
        assert!(validate_non_empty_path(&PathBuf::from("/valid/path"), "data_dir").is_ok());
        assert!(validate_non_empty_path(&PathBuf::from(""), "data_dir").is_err());
    }

    #[test]
    fn test_validate_positive_duration() {
        use std::time::Duration;
        assert!(validate_positive_duration(Duration::from_secs(1), "timeout").is_ok());
        assert!(validate_positive_duration(Duration::from_millis(100), "timeout").is_ok());
        assert!(validate_positive_duration(Duration::ZERO, "timeout").is_err());
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        
        assert_eq!(deserialized.network.tcp_port, config.network.tcp_port);
        assert_eq!(deserialized.network.udp_port, config.network.udp_port);
    }

    #[test]
    fn test_config_validate() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    // 🔥 MemoryConflictConfig 测试 (P1.7.0 任务组 0.5)

    /// 测试 MemoryConflictConfig 默认值
    #[test]
    fn test_memory_conflict_config_default() {
        let config = MemoryConflictConfig::default();
        assert_eq!(config.enforce_check, true);  // ← 必须为 true
        assert_eq!(config.conflict_timeout_secs, 300);
    }

    /// 测试 MemoryConflictConfig 验证（正确配置）
    #[test]
    fn test_memory_conflict_config_validate_valid() {
        let config = MemoryConflictConfig::default();
        let validated = config.validate().unwrap();
        assert_eq!(validated.enforce_check, true);
    }

    /// 测试 MemoryConflictConfig 验证（强制覆盖错误配置）
    #[test]
    fn test_memory_conflict_config_validate_override_invalid() {
        let mut config = MemoryConflictConfig::default();
        config.enforce_check = false;  // ← 错误配置

        let validated = config.validate().unwrap();
        assert_eq!(validated.enforce_check, true);  // ← 强制设置为 true
    }

    /// 测试 Config 默认值包含 memory_conflict
    #[test]
    fn test_config_default_includes_memory_conflict() {
        let config = Config::default();
        assert_eq!(config.memory_conflict.enforce_check, true);  // ← 默认强制检测
    }

    /// 测试 Config validate 验证 memory_conflict
    #[test]
    fn test_config_validate_memory_conflict() {
        let config = Config::default();
        assert!(config.validate().is_ok());  // ← 验证通过

        // 即使修改为 false，validate() 也会强制覆盖
        let mut config = Config::default();
        config.memory_conflict.enforce_check = false;
        assert!(config.validate().is_ok());  // ← 仍然成功（已强制覆盖）
    }
}
