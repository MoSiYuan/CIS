//! Agent 安全模块
//!
//! 提供 Agent 命令执行的安全控制功能，包括：
//! - 命令白名单验证
//! - 命令分类（安全/危险/禁止）
//! - 基于 YAML 的配置

pub mod command_whitelist;

pub use command_whitelist::{
    CommandClass,
    CommandWhitelist,
    ValidationResult,
    WhitelistConfig,
    AllowedPattern,
    DeniedPattern,
};
