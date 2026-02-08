//! # Decision Configuration
//!
//! 四级决策机制的配置管理
//! 配置文件路径: ~/.config/cis/decision.toml

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 默认 Recommended 级别超时时间（秒）
pub const DEFAULT_TIMEOUT_RECOMMENDED: u16 = 30;
/// 默认 Confirmed 级别超时时间（秒）
pub const DEFAULT_TIMEOUT_CONFIRMED: u16 = 300; // 5分钟
/// 默认 Arbitrated 级别超时时间（秒）
pub const DEFAULT_TIMEOUT_ARBITRATED: u16 = 3600; // 1小时

/// 决策配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConfig {
    /// Recommended 级别超时时间（秒）
    #[serde(default = "default_timeout_recommended")]
    pub timeout_recommended: u16,
    /// Confirmed 级别超时时间（秒）
    #[serde(default = "default_timeout_confirmed")]
    pub timeout_confirmed: u16,
    /// Arbitrated 级别超时时间（秒）
    #[serde(default = "default_timeout_arbitrated")]
    pub timeout_arbitrated: u16,
    /// 是否显示倒计时
    #[serde(default = "default_show_countdown")]
    pub show_countdown: bool,
    /// 是否启用交互式确认
    #[serde(default = "default_interactive")]
    pub interactive: bool,
    /// 仲裁投票阈值（同意比例，0.0-1.0）
    #[serde(default = "default_arbitration_threshold")]
    pub arbitration_threshold: f32,
}

impl DecisionConfig {
    /// 创建默认配置
    pub fn default_config() -> Self {
        Self {
            timeout_recommended: DEFAULT_TIMEOUT_RECOMMENDED,
            timeout_confirmed: DEFAULT_TIMEOUT_CONFIRMED,
            timeout_arbitrated: DEFAULT_TIMEOUT_ARBITRATED,
            show_countdown: true,
            interactive: true,
            arbitration_threshold: 0.5, // 简单多数
        }
    }

    /// 加载配置
    ///
    /// 按以下顺序查找配置：
    /// 1. ~/.config/cis/decision.toml
    /// 2. 环境变量 CIS_DECISION_*
    /// 3. 使用默认值
    pub fn load() -> Self {
        // 首先尝试从文件加载
        if let Some(config_path) = Self::config_path() {
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(config) = toml::from_str::<DecisionConfig>(&content) {
                        tracing::info!("Loaded decision config from {:?}", config_path);
                        return config;
                    }
                }
            }
        }

        // 从环境变量加载
        let mut config = Self::default_config();
        
        if let Ok(val) = std::env::var("CIS_DECISION_TIMEOUT_RECOMMENDED") {
            if let Ok(timeout) = val.parse() {
                config.timeout_recommended = timeout;
            }
        }
        
        if let Ok(val) = std::env::var("CIS_DECISION_TIMEOUT_CONFIRMED") {
            if let Ok(timeout) = val.parse() {
                config.timeout_confirmed = timeout;
            }
        }
        
        if let Ok(val) = std::env::var("CIS_DECISION_TIMEOUT_ARBITRATED") {
            if let Ok(timeout) = val.parse() {
                config.timeout_arbitrated = timeout;
            }
        }
        
        if let Ok(val) = std::env::var("CIS_DECISION_SHOW_COUNTDOWN") {
            config.show_countdown = val == "1" || val.to_lowercase() == "true";
        }
        
        if let Ok(val) = std::env::var("CIS_DECISION_INTERACTIVE") {
            config.interactive = val == "1" || val.to_lowercase() == "true";
        }
        
        if let Ok(val) = std::env::var("CIS_DECISION_ARBITRATION_THRESHOLD") {
            if let Ok(threshold) = val.parse::<f32>() {
                config.arbitration_threshold = threshold.clamp(0.0, 1.0);
            }
        }

        config
    }

    /// 保存配置到文件
    pub fn save(&self) -> crate::error::Result<()> {
        let config_path = Self::config_path()
            .ok_or_else(|| crate::error::CisError::configuration("Could not determine config path"))?;
        
        // 确保目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::CisError::configuration(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(&config_path, content)
            .map_err(|e| crate::error::CisError::configuration(format!("Failed to write config: {}", e)))?;
        
        tracing::info!("Saved decision config to {:?}", config_path);
        Ok(())
    }

    /// 获取配置文件路径
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("cis/decision.toml"))
    }

    /// 确保配置目录存在
    pub fn ensure_config_dir() -> crate::error::Result<PathBuf> {
        let config_path = Self::config_path()
            .ok_or_else(|| crate::error::CisError::configuration("Could not determine config path"))?;
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(config_path)
    }
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

fn default_timeout_recommended() -> u16 {
    DEFAULT_TIMEOUT_RECOMMENDED
}

fn default_timeout_confirmed() -> u16 {
    DEFAULT_TIMEOUT_CONFIRMED
}

fn default_timeout_arbitrated() -> u16 {
    DEFAULT_TIMEOUT_ARBITRATED
}

fn default_show_countdown() -> bool {
    true
}

fn default_interactive() -> bool {
    true
}

fn default_arbitration_threshold() -> f32 {
    0.5
}

/// 生成默认配置文件内容
pub fn generate_default_config() -> String {
    r#"# CIS Four-Tier Decision Configuration
# 四级决策机制配置文件

# Recommended 级别超时时间（秒）
# 倒计时结束后自动执行默认动作
timeout_recommended = 30

# Confirmed 级别超时时间（秒）
# 超时后自动取消任务
timeout_confirmed = 300

# Arbitrated 级别超时时间（秒）
# 超时后自动拒绝任务
timeout_arbitrated = 3600

# 是否显示倒计时进度
show_countdown = true

# 是否启用交互式确认
interactive = true

# 仲裁投票阈值（同意比例，0.0-1.0）
# 默认简单多数（0.5）
arbitration_threshold = 0.5
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DecisionConfig::default();
        assert_eq!(config.timeout_recommended, DEFAULT_TIMEOUT_RECOMMENDED);
        assert_eq!(config.timeout_confirmed, DEFAULT_TIMEOUT_CONFIRMED);
        assert_eq!(config.timeout_arbitrated, DEFAULT_TIMEOUT_ARBITRATED);
        assert!(config.show_countdown);
        assert!(config.interactive);
        assert_eq!(config.arbitration_threshold, 0.5);
    }

    #[test]
    fn test_config_serialization() {
        let config = DecisionConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("timeout_recommended"));
        
        let parsed: DecisionConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.timeout_recommended, config.timeout_recommended);
    }
}
