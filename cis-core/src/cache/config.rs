//! # Cache Configuration
//!
//! 缓存配置管理模块。
//!
//! ## 配置选项
//!
//! - `enabled`: 是否启用缓存
//! - `max_entries`: 最大缓存条目数
//! - `default_ttl`: 默认 TTL (Time To Live)
//! - `key_prefix`: 缓存键前缀 (用于命名空间隔离)
//! - `enable_metrics`: 是否启用统计信息收集
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::cache::CacheConfig;
//! use std::time::Duration;
//!
//! // 使用默认配置
//! let config = CacheConfig::default();
//!
//! // 自定义配置
//! let config = CacheConfig {
//!     enabled: true,
//!     max_entries: 2000,
//!     default_ttl: Duration::from_secs(600),
//!     key_prefix: Some("my-app".to_string()),
//!     enable_metrics: true,
//! };
//! ```

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 是否启用缓存
    ///
    /// 全局开关，可快速禁用缓存用于调试或测试。
    pub enabled: bool,

    /// 最大缓存条目数
    ///
    /// 超过此数量时，会触发 LRU 淘汰。
    /// 建议值：500 - 5000，根据应用的热数据量调整。
    pub max_entries: usize,

    /// 默认 TTL (Time To Live)
    ///
    /// 缓存条目的默认过期时间。超过此时间的条目会被自动清理。
    /// 建议值：60s - 600s，根据数据更新频率调整。
    pub default_ttl: Duration,

    /// 缓存键前缀
    ///
    /// 用于命名空间隔离，防止不同应用或实例的缓存冲突。
    /// 示例：`"my-app"`, `"node-1"`, `"production"`
    pub key_prefix: Option<String>,

    /// 是否启用统计信息收集
    ///
    /// 生产环境建议开启，测试环境可关闭以减少开销。
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,
            default_ttl: Duration::from_secs(300), // 5 分钟
            key_prefix: None,
            enable_metrics: true,
        }
    }
}

impl CacheConfig {
    /// 创建新的缓存配置
    ///
    /// # 参数
    /// - `max_entries`: 最大缓存条目数
    /// - `default_ttl`: 默认 TTL
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            enabled: true,
            max_entries,
            default_ttl,
            key_prefix: None,
            enable_metrics: true,
        }
    }

    /// 设置是否启用缓存
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置最大缓存条目数
    pub fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// 设置默认 TTL
    pub fn with_default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// 设置缓存键前缀
    pub fn with_key_prefix(mut self, prefix: String) -> Self {
        self.key_prefix = Some(prefix);
        self
    }

    /// 设置是否启用统计信息收集
    pub fn with_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    /// 验证配置是否有效
    ///
    /// # 返回
    /// - `Result<(), String>`: 配置无效时返回错误描述
    pub fn validate(&self) -> Result<(), String> {
        if self.max_entries == 0 {
            return Err("max_entries must be greater than 0".to_string());
        }

        if self.max_entries > 100_000 {
            return Err("max_entries is too large (max: 100000)".to_string());
        }

        if self.default_ttl.as_secs() == 0 {
            return Err("default_ttl must be greater than 0".to_string());
        }

        if self.default_ttl.as_secs() > 86400 {
            return Err("default_ttl is too large (max: 86400s = 1 day)".to_string());
        }

        Ok(())
    }

    /// 预估内存占用 (字节)
    ///
    /// 这是一个粗略估计，假设每个缓存条目平均 1KB。
    ///
    /// # 返回
    /// - `usize`: 预估内存占用 (字节)
    pub fn estimated_memory_usage(&self) -> usize {
        const AVG_ENTRY_SIZE: usize = 1024; // 1KB per entry
        self.max_entries * AVG_ENTRY_SIZE
    }

    /// 创建用于开发环境的配置
    ///
    /// 特点：较小的缓存，较短的 TTL，关闭统计。
    pub fn development() -> Self {
        Self {
            enabled: true,
            max_entries: 100,
            default_ttl: Duration::from_secs(60),
            key_prefix: Some("dev".to_string()),
            enable_metrics: false,
        }
    }

    /// 创建用于测试环境的配置
    ///
    /// 特点：禁用缓存，确保测试正确性。
    pub fn testing() -> Self {
        Self {
            enabled: false,
            max_entries: 10,
            default_ttl: Duration::from_secs(1),
            key_prefix: Some("test".to_string()),
            enable_metrics: false,
        }
    }

    /// 创建用于生产环境的配置
    ///
    /// 特点：较大的缓存，适中的 TTL，启用统计。
    pub fn production() -> Self {
        Self {
            enabled: true,
            max_entries: 5000,
            default_ttl: Duration::from_secs(600),
            key_prefix: None,
            enable_metrics: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CacheConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.default_ttl, Duration::from_secs(300));
        assert!(config.key_prefix.is_none());
        assert!(config.enable_metrics);
    }

    #[test]
    fn test_builder_pattern() {
        let config = CacheConfig::default()
            .with_enabled(false)
            .with_max_entries(2000)
            .with_default_ttl(Duration::from_secs(600))
            .with_key_prefix("my-app".to_string())
            .with_metrics(false);

        assert!(!config.enabled);
        assert_eq!(config.max_entries, 2000);
        assert_eq!(config.default_ttl, Duration::from_secs(600));
        assert_eq!(config.key_prefix, Some("my-app".to_string()));
        assert!(!config.enable_metrics);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = CacheConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_max_entries() {
        let mut config = CacheConfig::default();
        config.max_entries = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_too_large_max_entries() {
        let mut config = CacheConfig::default();
        config.max_entries = 200_000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_ttl() {
        let mut config = CacheConfig::default();
        config.default_ttl = Duration::from_secs(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_too_large_ttl() {
        let mut config = CacheConfig::default();
        config.default_ttl = Duration::from_secs(100_000);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_estimated_memory_usage() {
        let config = CacheConfig::default();
        assert_eq!(config.estimated_memory_usage(), 1000 * 1024);
    }

    #[test]
    fn test_development_config() {
        let config = CacheConfig::development();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 100);
        assert_eq!(config.default_ttl, Duration::from_secs(60));
        assert_eq!(config.key_prefix, Some("dev".to_string()));
        assert!(!config.enable_metrics);
    }

    #[test]
    fn test_testing_config() {
        let config = CacheConfig::testing();
        assert!(!config.enabled);
        assert_eq!(config.max_entries, 10);
        assert_eq!(config.default_ttl, Duration::from_secs(1));
        assert_eq!(config.key_prefix, Some("test".to_string()));
        assert!(!config.enable_metrics);
    }

    #[test]
    fn test_production_config() {
        let config = CacheConfig::production();
        assert!(config.enabled);
        assert_eq!(config.max_entries, 5000);
        assert_eq!(config.default_ttl, Duration::from_secs(600));
        assert!(config.key_prefix.is_none());
        assert!(config.enable_metrics);
    }
}
