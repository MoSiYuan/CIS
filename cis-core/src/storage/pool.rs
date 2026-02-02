//! 多库连接池
//!
//! 为需要多线程访问数据库的场景提供连接池管理。
//! 每个连接都是一个 MultiDbConnection，支持跨库查询。
//!
//! ## 使用场景
//!
//! - 多线程任务调度，每个线程需要独立的数据库连接
//! - Web 服务器处理并发请求
//! - 批量数据处理任务
//!
//! ## 注意
//!
//! 连接池中的每个连接都是独立的 MultiDbConnection，它们之间
//! 的数据库挂载状态不共享。如果需要共享挂载状态，需要在应用
//! 层进行管理。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use super::connection::MultiDbConnection;
use crate::error::{CisError, Result};

/// 连接池配置
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 初始连接数
    pub initial_connections: usize,
    /// 连接超时（秒）
    pub connection_timeout_secs: u64,
    /// 空闲连接超时（秒）
    pub idle_timeout_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            initial_connections: 2,
            connection_timeout_secs: 30,
            idle_timeout_secs: 600, // 10分钟
        }
    }
}

/// 连接池中的连接包装
///
/// 当连接被取出时，记录取出时间，用于超时检测。
struct PooledConnection {
    conn: MultiDbConnection,
    checked_out_at: Option<std::time::Instant>,
    created_at: std::time::Instant,
}

/// 多库连接池
///
/// 管理多个 MultiDbConnection 实例，支持并发访问。
///
/// # Example
/// ```
/// let pool = ConnectionPool::new(
///     PathBuf::from("core.db"),
///     PoolConfig::default()
/// )?;
///
/// // 获取连接
/// let conn = pool.get_connection()?;
/// // 使用连接...
/// // 连接自动归还到池中
/// ```
pub struct ConnectionPool {
    /// 主数据库路径
    primary_path: PathBuf,
    /// 连接池配置
    config: PoolConfig,
    /// 可用连接队列
    available: Mutex<VecDeque<PooledConnection>>,
    /// 当前连接数
    total_count: Mutex<usize>,
    /// 条件变量，用于等待可用连接
    condvar: Condvar,
    /// 是否已关闭
    closed: Mutex<bool>,
}

impl ConnectionPool {
    /// 创建新的连接池
    ///
    /// # Arguments
    /// * `primary_path` - 主数据库路径
    /// * `config` - 连接池配置
    ///
    /// # Returns
    /// * `Result<Arc<Self>>` - 线程安全的连接池引用
    pub fn new(primary_path: PathBuf, config: PoolConfig) -> Result<Arc<Self>> {
        let pool = Arc::new(Self {
            primary_path,
            config: config.clone(),
            available: Mutex::new(VecDeque::new()),
            total_count: Mutex::new(0),
            condvar: Condvar::new(),
            closed: Mutex::new(false),
        });

        // 创建初始连接
        for _ in 0..config.initial_connections {
            let conn = Self::create_connection(&pool.primary_path)?;
            pool.available
                .lock()
                .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?
                .push_back(PooledConnection {
                    conn,
                    checked_out_at: None,
                    created_at: std::time::Instant::now(),
                });
        }

        *pool.total_count.lock().map_err(|e| CisError::storage(format!("Lock failed: {}", e)))? =
            config.initial_connections;

        tracing::info!(
            "ConnectionPool created: max={}, initial={}",
            config.max_connections,
            config.initial_connections
        );

        Ok(pool)
    }

    /// 获取连接
    ///
    /// 如果池中有可用连接，立即返回；
    /// 如果连接数未达到上限，创建新连接；
    /// 否则等待直到有连接可用或超时。
    ///
    /// # Returns
    /// * `Result<PoolConnectionGuard>` - 连接守卫，自动归还连接
    pub fn get_connection(self: &Arc<Self>) -> Result<PoolConnectionGuard> {
        let timeout = std::time::Duration::from_secs(self.config.connection_timeout_secs);
        let deadline = std::time::Instant::now() + timeout;

        loop {
            // 检查是否已关闭
            if *self.closed.lock().map_err(|e| CisError::storage(format!("Lock failed: {}", e)))? {
                return Err(CisError::storage("Connection pool is closed".to_string()));
            }

            // 尝试获取可用连接
            {
                let mut available = self
                    .available
                    .lock()
                    .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

                if let Some(mut pooled) = available.pop_front() {
                    pooled.checked_out_at = Some(std::time::Instant::now());
                    return Ok(PoolConnectionGuard {
                        pool: Arc::clone(self),
                        conn: Some(pooled.conn),
                    });
                }
            }

            // 尝试创建新连接
            {
                let mut total = self
                    .total_count
                    .lock()
                    .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

                if *total < self.config.max_connections {
                    *total += 1;
                    drop(total); // 释放锁

                    let conn = Self::create_connection(&self.primary_path)?;
                    return Ok(PoolConnectionGuard {
                        pool: Arc::clone(self),
                        conn: Some(conn),
                    });
                }
            }

            // 等待可用连接或超时
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(CisError::storage(
                    "Connection pool timeout: no available connection".to_string(),
                ));
            }

            let remaining = deadline - now;
            let (lock, cvar_result) = self
                .condvar
                .wait_timeout(
                    self.available.lock().map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?,
                    remaining,
                )
                .map_err(|e| CisError::storage(format!("Wait failed: {}", e)))?;

            if cvar_result.timed_out() {
                return Err(CisError::storage(
                    "Connection pool timeout: no available connection".to_string(),
                ));
            }
            drop(lock);
        }
    }

    /// 尝试获取连接（非阻塞）
    ///
    /// 如果无法立即获取连接，返回 None。
    pub fn try_get_connection(self: &Arc<Self>) -> Result<Option<PoolConnectionGuard>> {
        // 检查是否已关闭
        if *self.closed.lock().map_err(|e| CisError::storage(format!("Lock failed: {}", e)))? {
            return Err(CisError::storage("Connection pool is closed".to_string()));
        }

        // 尝试获取可用连接
        {
            let mut available = self
                .available
                .lock()
                .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

            if let Some(mut pooled) = available.pop_front() {
                pooled.checked_out_at = Some(std::time::Instant::now());
                return Ok(Some(PoolConnectionGuard {
                    pool: Arc::clone(self),
                    conn: Some(pooled.conn),
                }));
            }
        }

        // 尝试创建新连接
        {
            let mut total = self
                .total_count
                .lock()
                .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

            if *total < self.config.max_connections {
                *total += 1;
                drop(total);

                let conn = Self::create_connection(&self.primary_path)?;
                return Ok(Some(PoolConnectionGuard {
                    pool: Arc::clone(self),
                    conn: Some(conn),
                }));
            }
        }

        Ok(None)
    }

    /// 归还连接到池中
    fn return_connection(&self, conn: MultiDbConnection) {
        // 检查是否已关闭
        if let Ok(closed) = self.closed.lock() {
            if *closed {
                // 直接关闭连接
                let _ = conn.close();
                return;
            }
        }

        // 归还到池中
        if let Ok(mut available) = self.available.lock() {
            available.push_back(PooledConnection {
                conn,
                checked_out_at: None,
                created_at: std::time::Instant::now(),
            });
        }

        // 通知等待的线程
        self.condvar.notify_one();
    }

    /// 创建新连接
    fn create_connection(primary_path: &PathBuf) -> Result<MultiDbConnection> {
        MultiDbConnection::open(primary_path)
    }

    /// 获取当前连接数
    pub fn total_connections(&self) -> Result<usize> {
        self.total_count
            .lock()
            .map(|c| *c)
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))
    }

    /// 获取可用连接数
    pub fn available_connections(&self) -> Result<usize> {
        self.available
            .lock()
            .map(|a| a.len())
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))
    }

    /// 关闭连接池
    ///
    /// 关闭所有连接，等待正在使用的连接归还。
    pub fn close(&self) -> Result<()> {
        let mut closed = self
            .closed
            .lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        *closed = true;
        drop(closed);

        // 关闭所有可用连接
        let mut available = self
            .available
            .lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        while let Some(pooled) = available.pop_front() {
            let _ = pooled.conn.close();
        }

        // 通知所有等待线程
        self.condvar.notify_all();

        tracing::info!("ConnectionPool closed");

        Ok(())
    }
}

/// 连接池守卫
///
/// 当守卫被 drop 时，自动归还连接到池中。
pub struct PoolConnectionGuard {
    pool: Arc<ConnectionPool>,
    conn: Option<MultiDbConnection>,
}

impl PoolConnectionGuard {
    /// 获取连接的引用
    pub fn get(&self) -> &MultiDbConnection {
        self.conn.as_ref().unwrap()
    }

    /// 获取连接的可变引用
    pub fn get_mut(&mut self) -> &mut MultiDbConnection {
        self.conn.as_mut().unwrap()
    }

    /// 手动归还连接（如果不希望等待 Drop）
    pub fn release(mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn);
        }
    }
}

impl Drop for PoolConnectionGuard {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn);
        }
    }
}

impl std::ops::Deref for PoolConnectionGuard {
    type Target = MultiDbConnection;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl std::ops::DerefMut for PoolConnectionGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn setup_test_db(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
            .unwrap();
    }

    #[test]
    fn test_pool_basic() {
        let temp_dir = std::env::temp_dir().join("cis_test_pool");
        let _ = std::fs::create_dir_all(&temp_dir);

        let db_path = temp_dir.join("test.db");
        setup_test_db(&db_path);

        let config = PoolConfig {
            max_connections: 3,
            initial_connections: 1,
            ..Default::default()
        };

        let pool = ConnectionPool::new(db_path.clone(), config).unwrap();
        
        // 检查初始状态
        assert_eq!(pool.total_connections().unwrap(), 1);
        assert_eq!(pool.available_connections().unwrap(), 1);

        // 获取连接
        {
            let conn = pool.get_connection().unwrap();
            assert_eq!(pool.available_connections().unwrap(), 0);
            
            // 连接应该可用
            let _ = conn.get();
        } // 连接在这里归还

        // 连接应该已归还
        assert_eq!(pool.available_connections().unwrap(), 1);

        // 关闭池
        pool.close().unwrap();

        // 清理
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pool_concurrent() {
        let temp_dir = std::env::temp_dir().join("cis_test_pool_concurrent");
        let _ = std::fs::create_dir_all(&temp_dir);

        let db_path = temp_dir.join("test.db");
        setup_test_db(&db_path);

        let config = PoolConfig {
            max_connections: 2,
            initial_connections: 1,
            connection_timeout_secs: 1,
            ..Default::default()
        };

        let pool = ConnectionPool::new(db_path.clone(), config).unwrap();
        let pool = Arc::new(pool);

        // 使用所有连接
        let conn1 = pool.get_connection().unwrap();
        let conn2 = pool.get_connection().unwrap();

        assert_eq!(pool.total_connections().unwrap(), 2);
        assert_eq!(pool.available_connections().unwrap(), 0);

        // 第三个连接应该超时
        let pool2 = Arc::clone(&pool);
        let result = pool2.get_connection();
        assert!(result.is_err()); // 应该超时

        // 释放连接
        drop(conn1);
        drop(conn2);

        // 关闭池
        pool.close().unwrap();

        // 清理
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_try_get_connection() {
        let temp_dir = std::env::temp_dir().join("cis_test_pool_try");
        let _ = std::fs::create_dir_all(&temp_dir);

        let db_path = temp_dir.join("test.db");
        setup_test_db(&db_path);

        let config = PoolConfig {
            max_connections: 1,
            initial_connections: 1,
            ..Default::default()
        };

        let pool = ConnectionPool::new(db_path.clone(), config).unwrap();

        // 获取唯一连接
        let conn = pool.try_get_connection().unwrap();
        assert!(conn.is_some());

        // 再次尝试应该返回 None
        let conn2 = pool.try_get_connection().unwrap();
        assert!(conn2.is_none());

        // 关闭池
        pool.close().unwrap();

        // 清理
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
