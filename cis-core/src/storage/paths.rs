//! 跨平台目录路径管理
//!
//! 遵循各平台规范：
//! - macOS: `~/Library/Application Support/CIS`
//! - Linux: `~/.local/share/cis` 或 `$XDG_DATA_HOME/cis`
//! - Windows: `%LOCALAPPDATA%\CIS`

use std::path::PathBuf;

/// 目录路径管理器
pub struct Paths;

impl Paths {
    // ==================== 基础目录 ====================

    /// 获取 CIS 数据根目录
    ///
    /// 环境变量 `CIS_DATA_DIR` 可覆盖默认路径
    pub fn data_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("CIS_DATA_DIR") {
            return PathBuf::from(dir);
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_data_dir()
        }

        #[cfg(target_os = "linux")]
        {
            Self::linux_data_dir()
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_data_dir()
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_data_dir() -> PathBuf {
        dirs::data_dir()
            .expect("Failed to get macOS data directory")
            .join("CIS")
    }

    #[cfg(target_os = "linux")]
    fn linux_data_dir() -> PathBuf {
        // 优先使用 XDG_DATA_HOME
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .map(|p| p.join("cis"))
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .expect("Failed to get home directory")
                    .join(".local/share/cis")
            })
    }

    #[cfg(target_os = "windows")]
    fn windows_data_dir() -> PathBuf {
        dirs::data_local_dir()
            .expect("Failed to get Windows local data directory")
            .join("CIS")
    }

    // ==================== 核心目录 ====================

    /// 核心数据目录
    pub fn core_dir() -> PathBuf {
        Self::data_dir().join("core")
    }

    /// 核心数据库路径 (旧版，保留兼容性)
    pub fn core_db() -> PathBuf {
        Self::node_db()
    }

    /// 主数据库路径 (~/.cis/node.db)
    pub fn node_db() -> PathBuf {
        Self::data_dir().join("node.db")
    }

    /// 邦联数据库路径 (~/.cis/federation.db)
    pub fn federation_db() -> PathBuf {
        Self::data_dir().join("federation.db")
    }

    /// 记忆数据库路径 (~/.cis/memory.db)
    pub fn memory_db() -> PathBuf {
        Self::data_dir().join("memory.db")
    }

    /// 向量数据库路径 (~/.cis/vector.db)
    pub fn vector_db() -> PathBuf {
        Self::data_dir().join("vector.db")
    }

    /// WAL 文件目录 (~/.cis/wal/)
    pub fn wal_dir() -> PathBuf {
        Self::data_dir().join("wal")
    }

    /// 核心备份目录
    pub fn core_backup_dir() -> PathBuf {
        Self::core_dir().join("backup")
    }

    /// 主配置文件路径
    pub fn config_file() -> PathBuf {
        Self::data_dir().join("config.toml")
    }

    /// 节点密钥路径
    pub fn node_key_file() -> PathBuf {
        Self::data_dir().join("node.key")
    }

    // ==================== Skill 目录 ====================

    /// Skill 根目录
    pub fn skills_dir() -> PathBuf {
        Self::data_dir().join("skills")
    }

    /// Skill 注册表路径
    pub fn skill_registry() -> PathBuf {
        Self::skills_dir().join("registry.json")
    }

    /// 已安装 Skill 代码目录
    pub fn skills_installed_dir() -> PathBuf {
        Self::skills_dir().join("installed")
    }

    /// Native Skill 安装目录
    pub fn skills_native_dir() -> PathBuf {
        Self::skills_installed_dir().join("native")
    }

    /// WASM Skill 安装目录
    pub fn skills_wasm_dir() -> PathBuf {
        Self::skills_installed_dir().join("wasm")
    }

    /// Skill 数据目录（数据库等）
    pub fn skills_data_dir() -> PathBuf {
        Self::skills_dir().join("data")
    }

    /// 特定 Skill 的数据目录
    pub fn skill_data_dir(skill_name: &str) -> PathBuf {
        Self::skills_data_dir().join(skill_name)
    }

    /// 特定 Skill 的数据库路径 (~/.cis/skills/{skill_name}.db)
    pub fn skill_db(skill_name: &str) -> PathBuf {
        Self::skills_dir().join(format!("{}.db", skill_name))
    }

    // ==================== 日志目录 ====================

    /// 日志根目录
    pub fn logs_dir() -> PathBuf {
        Self::data_dir().join("logs")
    }

    /// Skill 日志目录
    pub fn skill_logs_dir() -> PathBuf {
        Self::logs_dir().join("skills")
    }

    /// 特定 Skill 的日志路径
    pub fn skill_log_file(skill_name: &str) -> PathBuf {
        Self::skill_logs_dir().join(format!("{}.log", skill_name))
    }

    // ==================== 缓存目录 ====================

    /// 缓存根目录
    pub fn cache_dir() -> PathBuf {
        Self::data_dir().join("cache")
    }

    /// AI 响应缓存目录
    pub fn cache_ai_dir() -> PathBuf {
        Self::cache_dir().join("ai")
    }

    /// HTTP 缓存目录
    pub fn cache_http_dir() -> PathBuf {
        Self::cache_dir().join("http")
    }

    /// 临时目录
    pub fn cache_tmp_dir() -> PathBuf {
        Self::cache_dir().join("tmp")
    }

    // ==================== 运行时目录 ====================

    /// 运行时数据目录
    pub fn runtime_dir() -> PathBuf {
        Self::data_dir().join("runtime")
    }

    /// PID 文件路径
    pub fn pid_file() -> PathBuf {
        Self::runtime_dir().join("pid")
    }

    /// Socket 目录
    pub fn sockets_dir() -> PathBuf {
        Self::runtime_dir().join("sockets")
    }

    /// 锁文件目录
    pub fn locks_dir() -> PathBuf {
        Self::runtime_dir().join("locks")
    }

    // ==================== 初始化 ====================

    /// 初始化所有必要的目录
    pub fn ensure_dirs() -> std::io::Result<()> {
        // 核心目录
        std::fs::create_dir_all(Self::core_dir())?;
        std::fs::create_dir_all(Self::core_backup_dir())?;

        // Skill 目录
        std::fs::create_dir_all(Self::skills_native_dir())?;
        std::fs::create_dir_all(Self::skills_wasm_dir())?;
        std::fs::create_dir_all(Self::skills_data_dir())?;

        // 日志目录
        std::fs::create_dir_all(Self::logs_dir())?;
        std::fs::create_dir_all(Self::skill_logs_dir())?;

        // 缓存目录
        std::fs::create_dir_all(Self::cache_ai_dir())?;
        std::fs::create_dir_all(Self::cache_http_dir())?;
        std::fs::create_dir_all(Self::cache_tmp_dir())?;

        // 运行时目录
        std::fs::create_dir_all(Self::runtime_dir())?;
        std::fs::create_dir_all(Self::sockets_dir())?;
        std::fs::create_dir_all(Self::locks_dir())?;

        Ok(())
    }

    /// 清理运行时目录（重启时调用）
    pub fn cleanup_runtime() -> std::io::Result<()> {
        let runtime = Self::runtime_dir();
        if runtime.exists() {
            std::fs::remove_dir_all(&runtime)?;
            std::fs::create_dir_all(&runtime)?;
            std::fs::create_dir_all(Self::sockets_dir())?;
            std::fs::create_dir_all(Self::locks_dir())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_structure() {
        // 测试路径构建
        let data_dir = Paths::data_dir();
        assert!(!data_dir.as_os_str().is_empty());

        let core_db = Paths::core_db();
        assert!(core_db.to_string_lossy().contains("core"));
        assert!(core_db.to_string_lossy().ends_with(".db"));

        let skill_db = Paths::skill_db("test-skill");
        assert!(skill_db.to_string_lossy().contains("test-skill"));
    }

    #[test]
    fn test_ensure_dirs() {
        // 设置临时测试目录
        let temp_dir = std::env::temp_dir().join("cis_test_paths");
        std::env::set_var("CIS_DATA_DIR", &temp_dir);

        // 清理并创建
        let _ = std::fs::remove_dir_all(&temp_dir);
        Paths::ensure_dirs().unwrap();

        // 验证目录存在
        assert!(Paths::core_dir().exists());
        assert!(Paths::skills_dir().exists());
        assert!(Paths::logs_dir().exists());
        assert!(Paths::cache_dir().exists());
        assert!(Paths::runtime_dir().exists());

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::remove_var("CIS_DATA_DIR");
    }
}
