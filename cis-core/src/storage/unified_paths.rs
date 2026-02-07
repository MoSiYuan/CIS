//! # Unified Paths Module
//!
//! 整合 CIS 所有数据路径，遵循 XDG Base Directory 规范。
//!
//! ## 目录结构
//!
//! ```
//! ~/.local/share/cis/          # XDG_DATA_HOME
//! ├── bin/                     # 二进制文件（可选）
//! ├── config/                  # 配置文件
//! │   ├── config.toml
//! │   ├── embedding.toml
//! │   └── keys/
//! ├── data/                    # 运行时数据
//! │   ├── memory.db
//! │   ├── vector.idx
//! │   └── sessions/
//! ├── models/                  # AI 模型
//! │   └── nomic-embed-text-v1.5/
//! ├── logs/                    # 日志文件
//! │   └── cis-node.log
//! └── cache/                   # 缓存
//!     ├── downloads/
//!     └── tmp/
//!
//! ~/.config/cis/ -> ~/.local/share/cis/config/  # 符号链接（兼容）
//! ~/.cache/cis/  -> ~/.local/share/cis/cache/   # 符号链接（兼容）
//! ```
//!
//! ## 迁移策略
//!
//! 1. 检测旧目录 (~/.cis/)
//! 2. 自动迁移到新目录
//! 3. 创建符号链接保持兼容

use std::env;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::error::{CisError, Result};

/// 统一路径管理器
pub struct UnifiedPaths;

impl UnifiedPaths {
    // ==================== 基础目录 ====================

    /// 获取 CIS 基础目录 (~/.local/share/cis)
    pub fn base_dir() -> PathBuf {
        // 1. 优先使用 XDG_DATA_HOME
        if let Ok(data_home) = env::var("XDG_DATA_HOME") {
            return PathBuf::from(data_home).join("cis");
        }

        // 2. 回退到 ~/.local/share/cis
        dirs::data_dir()
            .expect("Failed to get data directory")
            .join("cis")
    }

    /// 获取配置目录
    pub fn config_dir() -> PathBuf {
        // 优先使用 XDG_CONFIG_HOME，但统一存储在 data_dir 下
        Self::base_dir().join("config")
    }

    /// 获取数据目录
    pub fn data_dir() -> PathBuf {
        Self::base_dir().join("data")
    }

    /// 获取缓存目录
    pub fn cache_dir() -> PathBuf {
        Self::base_dir().join("cache")
    }

    /// 获取日志目录
    pub fn logs_dir() -> PathBuf {
        Self::base_dir().join("logs")
    }

    /// 获取模型目录
    pub fn models_dir() -> PathBuf {
        Self::base_dir().join("models")
    }

    /// 获取二进制目录
    pub fn bin_dir() -> PathBuf {
        Self::base_dir().join("bin")
    }

    // ==================== 具体文件路径 ====================

    /// 主配置文件路径
    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Embedding 配置文件路径
    pub fn embedding_config_file() -> PathBuf {
        Self::config_dir().join("embedding.toml")
    }

    /// 节点密钥目录
    pub fn keys_dir() -> PathBuf {
        Self::config_dir().join("keys")
    }

    /// 节点密钥文件
    pub fn node_key_file() -> PathBuf {
        Self::keys_dir().join("node.key")
    }

    /// 记忆数据库路径
    pub fn memory_db_path() -> PathBuf {
        Self::data_dir().join("memory.db")
    }

    /// 向量存储路径
    pub fn vector_storage_path() -> PathBuf {
        Self::data_dir().join("vector.idx")
    }

    /// 会话数据目录
    pub fn sessions_dir() -> PathBuf {
        Self::data_dir().join("sessions")
    }

    /// DAG 运行数据库
    pub fn dag_runs_db_path() -> PathBuf {
        Self::data_dir().join("dag_runs.db")
    }

    /// 主日志文件
    pub fn main_log_file() -> PathBuf {
        Self::logs_dir().join("cis-node.log")
    }

    /// 模型下载缓存
    pub fn model_download_cache() -> PathBuf {
        Self::cache_dir().join("downloads")
    }

    /// 临时文件目录
    pub fn tmp_dir() -> PathBuf {
        Self::cache_dir().join("tmp")
    }

    // ==================== 旧路径兼容 ====================

    /// 旧配置目录 (~/.cis)
    pub fn legacy_config_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Failed to get home directory")
            .join(".cis")
    }

    /// 检查是否需要迁移
    pub fn needs_migration() -> bool {
        let legacy = Self::legacy_config_dir();
        let new_base = Self::base_dir();

        // 旧目录存在且新目录不存在
        legacy.exists() && !new_base.exists()
    }

    /// 执行迁移
    pub fn migrate() -> Result<MigrationReport> {
        let legacy = Self::legacy_config_dir();
        let new_base = Self::base_dir();

        if !legacy.exists() {
            return Ok(MigrationReport {
                migrated: false,
                message: "No legacy directory found".to_string(),
            });
        }

        if new_base.exists() {
            return Ok(MigrationReport {
                migrated: false,
                message: "New directory already exists".to_string(),
            });
        }

        info!("Starting migration from legacy directory");
        info!("From: {}", legacy.display());
        info!("To: {}", new_base.display());

        // 创建新目录结构
        Self::create_directory_structure()?;

        // 迁移配置文件
        let legacy_config = legacy.join("config.toml");
        if legacy_config.exists() {
            std::fs::copy(&legacy_config, Self::config_file())?;
            info!("Migrated config.toml");
        }

        // 迁移 embedding 配置
        let legacy_embedding = legacy.join("embedding.toml");
        if legacy_embedding.exists() {
            std::fs::copy(&legacy_embedding, Self::embedding_config_file())?;
            info!("Migrated embedding.toml");
        }

        // 迁移数据文件
        let legacy_data = legacy.join("data");
        if legacy_data.exists() {
            Self::migrate_directory(&legacy_data, &Self::data_dir())?;
            info!("Migrated data directory");
        }

        // 迁移模型
        let legacy_models = legacy.join("models");
        if legacy_models.exists() {
            Self::migrate_directory(&legacy_models, &Self::models_dir())?;
            info!("Migrated models directory");
        }

        // 创建符号链接保持兼容（可选）
        #[cfg(unix)]
        {
            Self::create_compat_symlinks(&legacy, &new_base)?;
        }

        info!("Migration completed successfully");

        Ok(MigrationReport {
            migrated: true,
            message: format!("Migrated from {} to {}", legacy.display(), new_base.display()),
        })
    }

    /// 创建目录结构
    fn create_directory_structure() -> Result<()> {
        let dirs = [
            Self::base_dir(),
            Self::config_dir(),
            Self::data_dir(),
            Self::cache_dir(),
            Self::logs_dir(),
            Self::models_dir(),
            Self::bin_dir(),
            Self::keys_dir(),
            Self::sessions_dir(),
            Self::model_download_cache(),
            Self::tmp_dir(),
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir)
                .map_err(|e| CisError::io(format!("Failed to create directory {}: {}", dir.display(), e)))?;
        }

        Ok(())
    }

    /// 迁移目录内容
    fn migrate_directory(src: &Path, dst: &Path) -> Result<()> {
        if !src.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(dst)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                Self::migrate_directory(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    /// 创建兼容符号链接（Unix only）
    #[cfg(unix)]
    fn create_compat_symlinks(legacy: &Path, new_base: &Path) -> Result<()> {
        use std::os::unix::fs::symlink;

        // 备份旧目录
        let backup = legacy.with_extension("backup");
        std::fs::rename(legacy, &backup)?;

        // 创建符号链接
        symlink(new_base, legacy)
            .map_err(|e| CisError::io(format!("Failed to create symlink: {}", e)))?;

        info!("Created compatibility symlink: {} -> {}", legacy.display(), new_base.display());

        Ok(())
    }

    /// 初始化所有目录（首次运行）
    pub fn init() -> Result<()> {
        // 检查是否需要迁移
        if Self::needs_migration() {
            warn!("Legacy directory detected, migration needed");
            Self::migrate()?;
        }

        // 创建目录结构
        Self::create_directory_structure()?;

        // 设置权限（Unix）
        #[cfg(unix)]
        Self::set_unix_permissions()?;

        info!("Unified paths initialized");

        Ok(())
    }

    /// 设置 Unix 权限
    #[cfg(unix)]
    fn set_unix_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        // 配置文件仅用户可读写
        let config_dir = Self::config_dir();
        if config_dir.exists() {
            let mut perms = std::fs::metadata(&config_dir)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&config_dir, perms)?;
        }

        // 密钥目录严格权限
        let keys_dir = Self::keys_dir();
        if keys_dir.exists() {
            let mut perms = std::fs::metadata(&keys_dir)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&keys_dir, perms)?;
        }

        Ok(())
    }

    /// 获取卸载/清理命令
    pub fn uninstall_commands() -> Vec<(&'static str, &'static str)> {
        vec![
            ("rm -rf ~/.local/share/cis", "Remove all CIS data"),
            ("rm -rf ~/.config/cis", "Remove CIS config (if separate)"),
            ("rm -rf ~/.cache/cis", "Remove CIS cache (if separate)"),
            ("rm ~/.cis.backup", "Remove backup (if exists)"),
        ]
    }
}

/// 迁移报告
pub struct MigrationReport {
    pub migrated: bool,
    pub message: String,
}

/// 清理工具
pub struct Cleanup;

impl Cleanup {
    /// 清理缓存
    pub fn clean_cache() -> Result<()> {
        let cache_dir = UnifiedPaths::cache_dir();
        if cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)?;
            std::fs::create_dir_all(&cache_dir)?;
        }
        info!("Cache cleaned");
        Ok(())
    }

    /// 清理日志
    pub fn clean_logs(days: u32) -> Result<()> {
        let logs_dir = UnifiedPaths::logs_dir();
        if !logs_dir.exists() {
            return Ok(());
        }

        let cutoff = std::time::SystemTime::now()
            - std::time::Duration::from_secs(days as u64 * 24 * 60 * 60);

        for entry in std::fs::read_dir(logs_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            
            if let Ok(modified) = metadata.modified() {
                if modified < cutoff {
                    std::fs::remove_file(entry.path())?;
                    info!("Removed old log: {}", entry.path().display());
                }
            }
        }

        Ok(())
    }

    /// 完全卸载（危险操作）
    pub fn purge_all() -> Result<()> {
        let base = UnifiedPaths::base_dir();
        
        if base.exists() {
            std::fs::remove_dir_all(&base)?;
            info!("Removed all CIS data: {}", base.display());
        }

        // 也清理旧目录
        let legacy = UnifiedPaths::legacy_config_dir();
        if legacy.exists() {
            std::fs::remove_dir_all(&legacy)?;
            info!("Removed legacy directory: {}", legacy.display());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir() {
        let base = UnifiedPaths::base_dir();
        assert!(base.to_string_lossy().contains("cis"));
    }

    #[test]
    fn test_directory_structure() {
        // 这个测试应该在临时目录中运行
        // 这里只是验证路径生成正确
        let paths = [
            UnifiedPaths::config_dir(),
            UnifiedPaths::data_dir(),
            UnifiedPaths::cache_dir(),
            UnifiedPaths::logs_dir(),
            UnifiedPaths::models_dir(),
        ];

        for path in &paths {
            assert!(path.to_string_lossy().contains("cis"));
        }
    }
}
