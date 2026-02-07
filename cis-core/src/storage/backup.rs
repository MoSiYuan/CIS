//! 备份管理模块

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::paths::Paths;
use crate::error::{CisError, Result};

/// 备份管理器
pub struct BackupManager {
    max_backups: usize,
    backup_dir: PathBuf,
}

impl BackupManager {
    /// 创建默认备份管理器
    pub fn new() -> Self {
        Self {
            max_backups: 10,
            backup_dir: Paths::core_backup_dir(),
        }
    }

    /// 自定义最大备份数
    pub fn with_max_backups(mut self, max: usize) -> Self {
        self.max_backups = max;
        self
    }

    /// 创建核心数据库备份
    pub fn backup_core(&self) -> Result<PathBuf> {
        // 确保备份目录存在
        fs::create_dir_all(&self.backup_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.backup_dir.join(format!("core.db.{}", timestamp));

        // 打开源数据库
        let src = rusqlite::Connection::open(Paths::core_db())
            .map_err(|e| CisError::Storage(format!("Failed to open core db: {}", e)))?;

        // 创建目标数据库
        let mut dst = rusqlite::Connection::open(&backup_path)
            .map_err(|e| CisError::Storage(format!("Failed to create backup db: {}", e)))?;

        // 执行备份
        use rusqlite::backup::Backup;
        let backup = Backup::new(&src, &mut dst)
            .map_err(|e| CisError::Storage(format!("Failed to create backup: {}", e)))?;
        
        backup
            .step(-1)
            .map_err(|e| CisError::Storage(format!("Failed to backup db: {}", e)))?;

        // 清理旧备份
        self.cleanup_old_backups()?;

        Ok(backup_path)
    }

    /// 列出所有备份
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        if !self.backup_dir.exists() {
            return Ok(vec![]);
        }

        let mut backups = vec![];

        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension() == Some(std::ffi::OsStr::new("db")) 
                || path.to_string_lossy().contains("core.db.") {
                
                let metadata = entry.metadata()?;
                let size = metadata.len();
                let created = metadata.created()
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                backups.push(BackupInfo {
                    path: path.clone(),
                    size,
                    created: created.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                });
            }
        }

        // 按创建时间排序（最新的在前）
        backups.sort_by(|a, b| b.created.cmp(&a.created));

        Ok(backups)
    }

    /// 恢复核心数据库
    pub fn restore_core(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(CisError::Storage(format!(
                "Backup not found: {}",
                backup_path.display()
            )));
        }

        let core_db_path = Paths::core_db();

        // 备份当前数据库（以防万一）
        let recovery_path = core_db_path.with_extension("db.recovery");
        if core_db_path.exists() {
            fs::copy(&core_db_path, &recovery_path)?;
        }

        // 恢复备份
        fs::copy(backup_path, &core_db_path)?;

        Ok(())
    }

    /// 清理旧备份
    fn cleanup_old_backups(&self) -> Result<()> {
        let backups = self.list_backups()?;

        if backups.len() > self.max_backups {
            let to_remove = &backups[self.max_backups..];
            for backup in to_remove {
                let _ = fs::remove_file(&backup.path);
            }
        }

        Ok(())
    }

    /// 删除所有备份
    pub fn clear_all_backups(&self) -> Result<()> {
        if self.backup_dir.exists() {
            fs::remove_dir_all(&self.backup_dir)?;
            fs::create_dir_all(&self.backup_dir)?;
        }
        Ok(())
    }

    /// 获取备份目录大小
    pub fn backup_dir_size(&self) -> Result<u64> {
        if !self.backup_dir.exists() {
            return Ok(0);
        }

        let mut total_size = 0u64;
        for entry in fs::read_dir(&self.backup_dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
            }
        }

        Ok(total_size)
    }
}

impl Default for BackupManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 备份信息
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub size: u64,
    pub created: u64,
}

impl BackupInfo {
    /// 格式化文件大小
    pub fn format_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = self.size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_index])
    }

    /// 格式化创建时间
    pub fn format_created(&self) -> String {
        let datetime = chrono::DateTime::from_timestamp(self.created as i64, 0)
            .unwrap_or(chrono::DateTime::UNIX_EPOCH);
        datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_env() {
        let temp_dir = std::env::temp_dir().join("cis_test_backup");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
        super::super::paths::Paths::ensure_dirs().unwrap();
    }

    fn cleanup_test_env() {
        std::env::remove_var("CIS_DATA_DIR");
    }

    #[test]
    fn test_backup_info_formatting() {
        let info = BackupInfo {
            path: PathBuf::from("test"),
            size: 1536000,
            created: 1704067200, // 2024-01-01 00:00:00 UTC
        };

        assert_eq!(info.format_size(), "1.46 MB");
        assert_eq!(info.format_created(), "2024-01-01 00:00:00 UTC");
    }
}
