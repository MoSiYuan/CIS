//! 随时关机安全机制
//!
//! 提供 SIGTERM/SIGINT 信号处理、启动恢复和定期 checkpoint 功能，
//! 确保 CIS 支持随时关机耐受。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rusqlite::Connection;
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{info, warn};

use crate::error::Result;
use crate::storage::wal::{checkpoint, checkpoint_passive, set_wal_mode, WALConfig};

/// 关机安全管理者
///
/// 管理多个数据库连接的关机安全机制，包括：
/// - 优雅关机信号处理
/// - 启动时 WAL 恢复
/// - 定期自动 checkpoint
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use tokio::sync::Mutex;
/// use rusqlite::Connection;
/// use cis_core::storage::safety::ShutdownSafety;
///
/// # async fn example() {
/// let safety = ShutdownSafety::new();
///
/// // 注册连接
/// // safety.register(conn).await;
///
/// // 注册优雅关机
/// safety.register_graceful_shutdown().await;
/// # }
/// ```
#[derive(Debug)]
pub struct ShutdownSafety {
    connections: Arc<Mutex<Vec<Arc<Mutex<Connection>>>>>,
}

impl Default for ShutdownSafety {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownSafety {
    /// 创建新的关机安全管理者
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 注册数据库连接
    ///
    /// 注册后的连接将在关机时自动执行 checkpoint
    ///
    /// # Arguments
    ///
    /// * `conn` - 数据库连接（Arc<Mutex<Connection>>）
    pub async fn register(&self, conn: Arc<Mutex<Connection>>) {
        let mut connections = self.connections.lock().await;
        connections.push(conn);
    }

    /// 注销数据库连接
    ///
    /// # Arguments
    ///
    /// * `conn` - 要注销的连接
    pub async fn unregister(&self, conn: &Arc<Mutex<Connection>>) {
        let mut connections = self.connections.lock().await;
        connections.retain(|c| !Arc::ptr_eq(c, conn));
    }

    /// 注册 SIGTERM/SIGINT handler 进行优雅关机
    ///
    /// 当收到 Ctrl+C (SIGINT) 或终止信号时，自动执行所有注册连接的 checkpoint
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cis_core::storage::safety::ShutdownSafety;
    ///
    /// # async fn example() {
    /// let safety = ShutdownSafety::new();
    /// safety.register_graceful_shutdown().await;
    /// # }
    /// ```
    pub async fn register_graceful_shutdown(&self) {
        let connections = self.connections.clone();

        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Received shutdown signal, performing checkpoint...");

                    let connections = connections.lock().await;
                    for (i, conn) in connections.iter().enumerate() {
                        if let Ok(conn) = conn.try_lock() {
                            if let Err(e) = checkpoint(&conn) {
                                warn!("Failed to checkpoint connection {}: {}", i, e);
                            } else {
                                info!("Checkpoint completed for connection {}", i);
                            }
                        } else {
                            warn!("Could not acquire lock for connection {}", i);
                        }
                    }

                    info!("Graceful shutdown completed");
                }
                Err(e) => {
                    warn!("Failed to listen for shutdown signal: {}", e);
                }
            }
        });
    }

    /// 对所有注册连接执行 checkpoint
    pub async fn checkpoint_all(&self) -> Result<()> {
        let connections = self.connections.lock().await;
        for (i, conn) in connections.iter().enumerate() {
            let conn = conn.lock().await;
            checkpoint(&conn).map_err(|e| {
                crate::error::CisError::storage(format!(
                    "Failed to checkpoint connection {}: {}",
                    i, e
                ))
            })?;
        }
        Ok(())
    }

    /// 对所有注册连接执行被动 checkpoint
    pub async fn checkpoint_all_passive(&self) -> Result<()> {
        let connections = self.connections.lock().await;
        for (i, conn) in connections.iter().enumerate() {
            let conn = conn.lock().await;
            checkpoint_passive(&conn).map_err(|e| {
                crate::error::CisError::storage(format!(
                    "Failed to passive checkpoint connection {}: {}",
                    i, e
                ))
            })?;
        }
        Ok(())
    }
}

/// 启动时检查并恢复 WAL
///
/// 检查数据库文件目录中的 `-wal` 文件是否存在，如果存在则尝试恢复。
/// 这应该在应用启动时、打开数据库连接后调用。
///
/// # Arguments
///
/// * `db_path` - 数据库文件路径
///
/// # Returns
///
/// - `Ok(true)` - 检测到 WAL 文件并成功恢复
/// - `Ok(false)` - 没有检测到 WAL 文件，无需恢复
/// - `Err(e)` - 恢复过程中发生错误
///
/// # Example
///
/// ```rust,no_run
/// use std::path::Path;
/// use cis_core::storage::safety::recover_on_startup;
///
/// # fn example() {
/// let db_path = Path::new("/path/to/db.sqlite");
/// match recover_on_startup(db_path) {
///     Ok(true) => println!("WAL recovered successfully"),
///     Ok(false) => println!("No WAL file found, clean start"),
///     Err(e) => eprintln!("Recovery failed: {}", e),
/// }
/// # }
/// ```
pub fn recover_on_startup(db_path: &Path) -> Result<bool> {
    let wal_path = db_path.with_extension("db-wal");
    let shm_path = db_path.with_extension("db-shm");

    // 检查 WAL 文件是否存在
    if !wal_path.exists() {
        info!("No WAL file found at {:?}, clean start", wal_path);
        return Ok(false);
    }

    let wal_size = std::fs::metadata(&wal_path)
        .map(|m| m.len())
        .unwrap_or(0);

    info!(
        "Found WAL file at {:?} ({} bytes), performing recovery",
        wal_path, wal_size
    );

    // 打开数据库连接并执行 checkpoint
    let conn = Connection::open(db_path)
        .map_err(|e| crate::error::CisError::storage(format!("Failed to open db for recovery: {}", e)))?;

    // 确保 WAL 模式已启用
    set_wal_mode(&conn, &WALConfig::default())
        .map_err(|e| crate::error::CisError::storage(format!("Failed to set WAL mode: {}", e)))?;

    // 执行 TRUNCATE checkpoint 确保完全恢复
    checkpoint(&conn)
        .map_err(|e| crate::error::CisError::storage(format!("Failed to checkpoint during recovery: {}", e)))?;

    // 验证 WAL 文件是否已被处理
    if wal_path.exists() {
        let new_size = std::fs::metadata(&wal_path)
            .map(|m| m.len())
            .unwrap_or(0);
        if new_size == 0 {
            info!("WAL file emptied after checkpoint");
        } else {
            warn!("WAL file still has {} bytes after checkpoint", new_size);
        }
    } else {
        info!("WAL file removed after checkpoint");
    }

    // 清理 SHM 文件（如果存在）
    if shm_path.exists() {
        if let Err(e) = std::fs::remove_file(&shm_path) {
            warn!("Failed to remove SHM file: {}", e);
        } else {
            info!("Removed SHM file");
        }
    }

    info!("WAL recovery completed successfully");
    Ok(true)
}

/// 定期自动 checkpoint 任务
///
/// 启动一个后台任务，定期执行 PASSIVE checkpoint。
/// 适用于需要定期维护 WAL 文件大小的场景。
///
/// # Arguments
///
/// * `conn` - 数据库连接（Arc<Mutex<Connection>>）
/// * `interval_duration` - 检查点执行间隔（默认 5 分钟）
///
/// # Returns
///
/// 返回一个 JoinHandle，可以用于取消任务
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use std::time::Duration;
/// use tokio::sync::Mutex;
/// use rusqlite::Connection;
/// use cis_core::storage::safety::start_periodic_checkpoint;
///
/// # async fn example() {
/// let conn = Arc::new(Mutex::new(Connection::open("test.db").unwrap()));
/// let handle = start_periodic_checkpoint(conn, Duration::from_secs(300));
///
/// // 当需要停止时
/// handle.abort();
/// # }
/// ```
pub fn start_periodic_checkpoint(
    conn: Arc<Mutex<Connection>>,
    interval_duration: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(interval_duration);

        loop {
            ticker.tick().await;

            if let Ok(conn) = conn.try_lock() {
                match checkpoint_passive(&conn) {
                    Ok(()) => info!("Periodic checkpoint completed"),
                    Err(e) => warn!("Periodic checkpoint failed: {}", e),
                }
            } else {
                warn!("Could not acquire lock for periodic checkpoint");
            }
        }
    })
}

/// 创建带有关机安全的数据库连接
///
/// 打开数据库连接，配置 WAL 模式，并检查是否需要恢复
///
/// # Arguments
///
/// * `db_path` - 数据库文件路径
/// * `config` - WAL 配置（可选，默认使用 WALConfig::default()）
///
/// # Returns
///
/// 成功返回连接，失败返回错误
///
/// # Example
///
/// ```rust,no_run
/// use cis_core::storage::safety::open_with_safety;
///
/// # fn example() {
/// let conn = open_with_safety("test.db", None).unwrap();
/// # }
/// ```
pub fn open_with_safety<P: AsRef<Path>>(
    db_path: P,
    config: Option<WALConfig>,
) -> Result<Connection> {
    let path = db_path.as_ref();

    // 启动时恢复检查
    recover_on_startup(path)?;

    // 打开连接
    let conn = Connection::open(path)
        .map_err(|e| crate::error::CisError::storage(format!("Failed to open database: {}", e)))?;

    // 配置 WAL 模式
    let config = config.unwrap_or_default();
    set_wal_mode(&conn, &config)
        .map_err(|e| crate::error::CisError::storage(format!("Failed to configure WAL: {}", e)))?;

    info!("Database opened with WAL safety: {:?}", path);
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn get_test_db_path() -> std::path::PathBuf {
        temp_dir().join(format!("test_safety_{}.db", std::process::id()))
    }

    fn cleanup_db(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn test_recover_on_startup_clean() {
        let db_path = get_test_db_path();
        cleanup_db(&db_path);

        // 创建一个新的数据库
        let conn = Connection::open(&db_path).unwrap();
        set_wal_mode(&conn, &WALConfig::default()).unwrap();
        drop(conn);

        // 没有 WAL 文件时的恢复
        let result = recover_on_startup(&db_path).unwrap();
        assert!(!result);

        cleanup_db(&db_path);
    }

    #[test]
    fn test_recover_on_startup_with_wal() {
        let db_path = get_test_db_path();
        cleanup_db(&db_path);

        // 创建 WAL 文件
        {
            let conn = Connection::open(&db_path).unwrap();
            set_wal_mode(&conn, &WALConfig::default()).unwrap();
            conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
                .unwrap();
            conn.execute("INSERT INTO test VALUES (1)", []).unwrap();
            // 注意：这里不执行 checkpoint，所以会有 WAL 数据
            drop(conn);
        }

        // 验证 WAL 文件存在
        let wal_path = db_path.with_extension("db-wal");
        assert!(wal_path.exists());

        // 执行恢复
        let result = recover_on_startup(&db_path).unwrap();
        assert!(result);

        cleanup_db(&db_path);
    }

    #[test]
    fn test_open_with_safety() {
        let db_path = get_test_db_path();
        cleanup_db(&db_path);

        let conn = open_with_safety(&db_path, None).unwrap();

        // 验证 WAL 模式已启用
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_uppercase(), "WAL");

        drop(conn);
        cleanup_db(&db_path);
    }

    #[tokio::test]
    async fn test_shutdown_safety_register() {
        let safety = ShutdownSafety::new();
        let db_path = get_test_db_path();
        cleanup_db(&db_path);

        let conn = Arc::new(Mutex::new(Connection::open(&db_path).unwrap()));
        safety.register(conn.clone()).await;

        // 验证已注册
        let connections = safety.connections.lock().await;
        assert_eq!(connections.len(), 1);
        drop(connections);

        // 注销
        safety.unregister(&conn).await;
        let connections = safety.connections.lock().await;
        assert!(connections.is_empty());

        cleanup_db(&db_path);
    }

    #[tokio::test]
    async fn test_shutdown_safety_checkpoint_all() {
        let safety = ShutdownSafety::new();
        let db_path = get_test_db_path();
        cleanup_db(&db_path);

        let conn = Arc::new(Mutex::new(Connection::open(&db_path).unwrap()));
        {
            let c = conn.lock().await;
            set_wal_mode(&*c, &WALConfig::default()).unwrap();
        }

        safety.register(conn).await;

        // 创建表和数据
        {
            let conn = safety.connections.lock().await;
            let c = conn[0].lock().await;
            c.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
                .unwrap();
            c.execute("INSERT INTO test VALUES (1)", []).unwrap();
        }

        // 执行 checkpoint
        let result = safety.checkpoint_all().await;
        assert!(result.is_ok());

        cleanup_db(&db_path);
    }
}
