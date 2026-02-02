//! # Storage Module
//!
//! 跨平台存储管理，支持数据隔离和热插拔。
//!
//! ## 模块结构
//!
//! - `paths`: 跨平台目录路径管理
//! - `db`: 数据库连接管理（核心 + Skill 隔离）
//! - `backup`: 自动备份管理

pub mod backup;
pub mod db;
pub mod paths;

pub use backup::BackupManager;
pub use db::{CoreDb, SkillDb, DbManager};
pub use paths::Paths;
