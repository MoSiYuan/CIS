//! WAL 模式配置
//!
//! 提供 SQLite WAL (Write-Ahead Logging) 模式的配置和管理功能，
//! 确保 CIS 支持随时关机耐受。

use rusqlite::Connection;

use crate::error::{CisError, Result};

/// WAL 同步模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynchronousMode {
    /// 同步关闭（最快，最不安全）
    Off = 0,
    /// 标准模式（推荐，平衡性能和安全）
    Normal = 1,
    /// 完全同步（更安全，较慢）
    Full = 2,
    /// 额外同步（最安全的模式）
    Extra = 3,
}

impl SynchronousMode {
    /// 转换为 SQLite PRAGMA 值
    pub fn as_str(&self) -> &'static str {
        match self {
            SynchronousMode::Off => "OFF",
            SynchronousMode::Normal => "NORMAL",
            SynchronousMode::Full => "FULL",
            SynchronousMode::Extra => "EXTRA",
        }
    }
}

impl Default for SynchronousMode {
    fn default() -> Self {
        SynchronousMode::Normal
    }
}

/// WAL 模式配置
#[derive(Debug, Clone)]
pub struct WALConfig {
    /// 同步模式
    pub synchronous: SynchronousMode,
    /// WAL 自动检查点页数（默认 1000 页 ≈ 4MB）
    pub wal_autocheckpoint: i32,
    /// 日志文件大小限制（字节，默认 100MB）
    pub journal_size_limit: i64,
    /// 繁忙超时（毫秒，默认 5 秒）
    pub busy_timeout: i32,
}

impl Default for WALConfig {
    fn default() -> Self {
        Self {
            synchronous: SynchronousMode::Normal,
            wal_autocheckpoint: 1000,
            journal_size_limit: 100 * 1024 * 1024, // 100MB
            busy_timeout: 5000,                     // 5秒
        }
    }
}

impl WALConfig {
    /// 创建生产环境推荐配置
    /// 
    /// 使用 NORMAL 同步模式，平衡性能和数据安全
    pub fn production() -> Self {
        Self::default()
    }

    /// 创建高性能配置
    /// 
    /// 使用 OFF 同步模式，牺牲部分安全性换取更高性能
    /// 适用于可以容忍少量数据丢失的场景
    pub fn high_performance() -> Self {
        Self {
            synchronous: SynchronousMode::Off,
            ..Default::default()
        }
    }

    /// 创建高安全配置
    /// 
    /// 使用 FULL 同步模式，确保数据零丢失
    /// 适用于对数据完整性要求极高的场景
    pub fn high_safety() -> Self {
        Self {
            synchronous: SynchronousMode::Full,
            ..Default::default()
        }
    }
}

/// 为连接配置 WAL 模式
///
/// # Arguments
///
/// * `conn` - SQLite 连接
/// * `config` - WAL 配置
///
/// # Returns
///
/// 成功返回 Ok(())，失败返回错误
///
/// # Example
///
/// ```rust
/// use rusqlite::Connection;
/// use cis_core::storage::wal::{set_wal_mode, WALConfig};
///
/// let conn = Connection::open("test.db").unwrap();
/// let config = WALConfig::default();
/// set_wal_mode(&conn, &config).unwrap();
/// ```
pub fn set_wal_mode(conn: &Connection, config: &WALConfig) -> Result<()> {
    // 设置 WAL 模式
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|e| CisError::storage(format!("Failed to set journal_mode to WAL: {}", e)))?;

    if journal_mode.to_uppercase() != "WAL" {
        return Err(CisError::storage(format!(
            "Failed to enable WAL mode, got: {}",
            journal_mode
        )));
    }

    // 设置同步模式
    conn.execute_batch(&format!(
        "PRAGMA synchronous = {};
         PRAGMA wal_autocheckpoint = {};
         PRAGMA journal_size_limit = {};
         PRAGMA busy_timeout = {};",
        config.synchronous.as_str(),
        config.wal_autocheckpoint,
        config.journal_size_limit,
        config.busy_timeout
    ))
    .map_err(|e| CisError::storage(format!("Failed to configure WAL settings: {}", e)))?;

    Ok(())
}

/// 手动执行 checkpoint
///
/// 使用 TRUNCATE 模式，将 WAL 文件内容完全写入数据库并清空 WAL 文件
/// 适用于关机前或需要确保数据完全落盘的场景
///
/// # Arguments
///
/// * `conn` - SQLite 连接
///
/// # Returns
///
/// 成功返回 Ok(())，失败返回错误
///
/// # Example
///
/// ```rust
/// use rusqlite::Connection;
/// use cis_core::storage::wal::checkpoint;
///
/// let conn = Connection::open("test.db").unwrap();
/// checkpoint(&conn).unwrap();
/// ```
pub fn checkpoint(conn: &Connection) -> Result<()> {
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])
        .map_err(|e| CisError::storage(format!("Failed to execute checkpoint: {}", e)))?;
    Ok(())
}

/// 自动 checkpoint（被动模式）
///
/// 使用 PASSIVE 模式，不会阻塞读写操作
/// 适用于定期维护，尽量减少对性能的影响
///
/// # Arguments
///
/// * `conn` - SQLite 连接
///
/// # Returns
///
/// 成功返回 Ok(())，失败返回错误
///
/// # Example
///
/// ```rust
/// use rusqlite::Connection;
/// use cis_core::storage::wal::checkpoint_passive;
///
/// let conn = Connection::open("test.db").unwrap();
/// checkpoint_passive(&conn).unwrap();
/// ```
pub fn checkpoint_passive(conn: &Connection) -> Result<()> {
    conn.execute("PRAGMA wal_checkpoint(PASSIVE)", [])
        .map_err(|e| CisError::storage(format!("Failed to execute passive checkpoint: {}", e)))?;
    Ok(())
}

/// 检查 WAL 文件状态
///
/// 返回 (日志页数, 已检查点页数, 总页数)
///
/// # Arguments
///
/// * `conn` - SQLite 连接
///
/// # Returns
///
/// 成功返回 WAL 状态元组，失败返回错误
pub fn wal_status(conn: &Connection) -> Result<(i32, i32, i32)> {
    let result = conn
        .query_row("PRAGMA wal_checkpoint", [], |row| {
            Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?, row.get::<_, i32>(2)?))
        })
        .map_err(|e| CisError::storage(format!("Failed to get WAL status: {}", e)))?;
    Ok(result)
}

/// 获取 WAL 文件大小（字节）
///
/// # Arguments
///
/// * `db_path` - 数据库文件路径
///
/// # Returns
///
/// 成功返回 WAL 文件大小，如果文件不存在返回 0
pub fn wal_file_size(db_path: &std::path::Path) -> Result<u64> {
    let wal_path = db_path.with_extension("db-wal");
    match std::fs::metadata(&wal_path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(CisError::Io(e)),
    }
}

/// 检查是否需要 checkpoint
///
/// 当 WAL 文件大小超过阈值时返回 true
///
/// # Arguments
///
/// * `conn` - SQLite 连接
/// * `db_path` - 数据库文件路径
/// * `threshold_bytes` - 阈值（字节）
///
/// # Returns
///
/// 需要 checkpoint 返回 true，否则返回 false
pub fn needs_checkpoint(
    conn: &Connection,
    db_path: &std::path::Path,
    threshold_bytes: u64,
) -> Result<bool> {
    let size = wal_file_size(db_path)?;
    if size >= threshold_bytes {
        return Ok(true);
    }

    // 同时检查页数
    let (logged, checkpointed, _) = wal_status(conn)?;
    Ok(logged > checkpointed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    fn get_test_db_path() -> std::path::PathBuf {
        temp_dir().join(format!("test_wal_{}.db", std::process::id()))
    }

    #[test]
    fn test_wal_config_default() {
        let config = WALConfig::default();
        assert_eq!(config.synchronous, SynchronousMode::Normal);
        assert_eq!(config.wal_autocheckpoint, 1000);
        assert_eq!(config.journal_size_limit, 100 * 1024 * 1024);
        assert_eq!(config.busy_timeout, 5000);
    }

    #[test]
    fn test_wal_config_presets() {
        let prod = WALConfig::production();
        assert_eq!(prod.synchronous, SynchronousMode::Normal);

        let perf = WALConfig::high_performance();
        assert_eq!(perf.synchronous, SynchronousMode::Off);

        let safe = WALConfig::high_safety();
        assert_eq!(safe.synchronous, SynchronousMode::Full);
    }

    #[test]
    fn test_set_wal_mode() {
        let db_path = get_test_db_path();
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        let conn = Connection::open(&db_path).unwrap();
        let config = WALConfig::default();

        assert!(set_wal_mode(&conn, &config).is_ok());

        // 验证 WAL 模式已启用
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_uppercase(), "WAL");

        // 清理
        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn test_checkpoint() {
        let db_path = get_test_db_path();
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        let conn = Connection::open(&db_path).unwrap();
        set_wal_mode(&conn, &WALConfig::default()).unwrap();

        // 创建表并写入数据
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, data TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO test (data) VALUES ('test')", [])
            .unwrap();

        // 执行 checkpoint
        assert!(checkpoint(&conn).is_ok());

        // 清理
        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn test_checkpoint_passive() {
        let db_path = get_test_db_path();
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));

        let conn = Connection::open(&db_path).unwrap();
        set_wal_mode(&conn, &WALConfig::default()).unwrap();

        // 创建表并写入数据
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, data TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO test (data) VALUES ('test')", [])
            .unwrap();

        // 执行被动 checkpoint
        assert!(checkpoint_passive(&conn).is_ok());

        // 清理
        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }
}
