//! # CIS 初始化模块
//!
//! 提供 CIS 环境初始化和项目设置的完整向导。
//!
//! ## 功能
//! - 环境检查（AI Agent、Git、目录权限）
//! - 全局配置生成
//! - 项目初始化
//! - 验证测试

pub mod wizard;

pub use wizard::{
    quick_init, init_non_interactive,
    InitWizard, WizardResult, EnvironmentCheck, AgentCheck,
};
