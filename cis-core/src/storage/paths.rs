//! 跨平台目录路径管理
//!
//! 路径解析策略（优先级从高到低）：
//! 1. 环境变量 `CIS_DATA_DIR` - 手动覆盖
//! 2. Release 模式: 使用可执行文件所在目录
//! 3. Git 项目: 使用 Git 根目录下的 `.cis/`
//! 4. 系统默认目录
//!
//! 各平台默认：
//! - macOS: `~/Library/Application Support/CIS`
//! - Linux: `~/.local/share/cis` 或 `$XDG_DATA_HOME/cis`
//! - Windows: `%LOCALAPPDATA%\CIS`

use std::path::PathBuf;

/// 目录路径管理器
pub struct Paths;

/// 运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Release 模式 - 便携版，使用当前目录
    Release,
    /// 开发模式 - 使用 Git 根目录或系统目录
    Development,
}

impl Paths {
    /// 检测当前运行模式
    pub fn run_mode() -> RunMode {
        // 如果可执行文件在 target/release 中，认为是 Release 模式
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_str = exe_path.to_string_lossy();
            if exe_str.contains("target/release/") || exe_str.contains("target\\release\\") {
                return RunMode::Release;
            }
        }
        
        // 检查环境变量 CIS_PORTABLE=1 强制使用便携模式
        if std::env::var("CIS_PORTABLE").unwrap_or_default() == "1" {
            return RunMode::Release;
        }
        
        RunMode::Development
    }

    /// 获取 Git 项目根目录
    pub fn git_root() -> Option<PathBuf> {
        // 从当前工作目录开始向上查找
        let mut current = std::env::current_dir().ok()?;
        
        loop {
            let git_dir = current.join(".git");
            if git_dir.exists() {
                return Some(current);
            }
            
            // 尝试父目录
            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => break,
            }
        }
        
        None
    }

    // ==================== 基础目录 ====================

    /// 获取 CIS 数据根目录
    ///
    /// 解析优先级：
    /// 1. 环境变量 `CIS_DATA_DIR`
    /// 2. Release 模式: 可执行文件所在目录
    /// 3. Git 项目: `.cis/` 目录
    /// 4. 系统默认目录
    pub fn data_dir() -> PathBuf {
        // 1. 环境变量覆盖
        if let Ok(dir) = std::env::var("CIS_DATA_DIR") {
            return PathBuf::from(dir);
        }

        // 2. Release 模式 - 使用可执行文件所在目录
        if Self::run_mode() == RunMode::Release {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    return exe_dir.join(".cis");
                }
            }
        }

        // 3. Git 项目模式
        if let Some(git_root) = Self::git_root() {
            let cis_dir = git_root.join(".cis");
            // 如果 .cis 目录已存在，优先使用它
            if cis_dir.exists() {
                return cis_dir;
            }
        }

        // 4. 系统默认目录
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

    /// 获取配置目录
    pub fn config_dir() -> PathBuf {
        // Release 模式下，配置文件也在 .cis 目录
        if Self::run_mode() == RunMode::Release {
            return Self::data_dir();
        }
        
        // 开发模式：Git 项目使用 .cis/，否则使用系统配置目录
        if let Some(git_root) = Self::git_root() {
            return git_root.join(".cis");
        }
        
        Self::data_dir()
    }

    #[cfg(target_os = "macos")]
    fn macos_data_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Failed to get home directory")
            .join(".cis")
    }

    #[cfg(target_os = "linux")]
    fn linux_data_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Failed to get home directory")
            .join(".cis")
    }

    #[cfg(target_os = "windows")]
    fn windows_data_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Failed to get home directory")
            .join(".cis")
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

    /// 主数据库路径
    pub fn node_db() -> PathBuf {
        Self::data_dir().join("node.db")
    }

    /// 邦联数据库路径
    pub fn federation_db() -> PathBuf {
        Self::data_dir().join("federation.db")
    }

    /// Matrix 事件数据库路径（协议核心，包含房间、事件、同步状态）
    pub fn matrix_events_db() -> PathBuf {
        Self::data_dir().join("matrix-events.db")
    }

    /// Matrix 社交数据库路径（人类用户数据，包含用户、设备、令牌、资料）
    /// 分离设计允许独立备份用户数据，并支持 Skill 化的注册逻辑
    pub fn matrix_social_db() -> PathBuf {
        Self::data_dir().join("matrix-social.db")
    }

    /// 记忆数据库路径
    pub fn memory_db() -> PathBuf {
        Self::data_dir().join("memory.db")
    }

    /// 向量数据库路径
    pub fn vector_db() -> PathBuf {
        Self::data_dir().join("vector.db")
    }

    /// 模型目录路径
    pub fn models_dir() -> PathBuf {
        Self::data_dir().join("models")
    }

    /// WAL 文件目录
    pub fn wal_dir() -> PathBuf {
        Self::data_dir().join("wal")
    }

    /// 核心备份目录
    pub fn core_backup_dir() -> PathBuf {
        Self::core_dir().join("backup")
    }

    /// 主配置文件路径
    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// 节点密钥路径
    pub fn node_key_file() -> PathBuf {
        Self::data_dir().join("node.key")
    }

    /// 获取当前项目目录
    pub fn current_project_dir() -> Option<PathBuf> {
        let current = std::env::current_dir().ok()?;
        
        // 如果在 Git 项目中，返回 Git 根目录
        if let Some(git_root) = Self::git_root() {
            if current.starts_with(&git_root) {
                return Some(current);
            }
        }
        
        // 否则返回当前目录
        Some(current)
    }

    /// 获取当前项目的 CIS 目录
    pub fn current_project_cis_dir() -> Option<PathBuf> {
        Self::current_project_dir().map(|d| d.join(".cis"))
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

    /// 特定 Skill 的数据库路径
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

    /// 显示路径信息（用于诊断）
    pub fn print_info() {
        println!("📁 CIS 路径信息:");
        println!("{}", "-".repeat(50));
        println!("  运行模式: {}", match Self::run_mode() {
            RunMode::Release => "Release (便携模式)",
            RunMode::Development => "Development (开发模式)",
        });
        
        if let Some(git_root) = Self::git_root() {
            println!("  Git 根目录: {}", git_root.display());
        } else {
            println!("  Git 根目录: 未检测到");
        }
        
        println!("  数据目录:   {}", Self::data_dir().display());
        println!("  配置目录:   {}", Self::config_dir().display());
        println!("  配置文件:   {}", Self::config_file().display());
        println!("{}", "-".repeat(50));
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
        assert!(core_db.to_string_lossy().contains("node"));
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
