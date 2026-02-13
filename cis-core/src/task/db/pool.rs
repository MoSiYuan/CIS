//! # 任务数据库连接池
//!
//! 提供高效的 SQLite 连接池管理，支持并发访问。

use rusqlite::{Connection, Result as SqliteResult, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// 数据库连接池
#[derive(Clone)]
pub struct DatabasePool {
    db_path: Arc<PathBuf>,
    max_connections: usize,
    semaphore: Arc<Semaphore>,
}

impl DatabasePool {
    /// 创建新的数据库连接池
    pub fn new(db_path: PathBuf, max_connections: usize) -> SqliteResult<Self> {
        // 确保父目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::Path(format!("{:?}", e)))?;
        }

        Ok(Self {
            db_path: Arc::new(db_path),
            max_connections,
            semaphore: Arc::new(Semaphore::new(max_connections)),
        })
    }

    /// 获取连接（异步信号量控制）
    pub async fn acquire(&self) -> SqliteResult<Connection> {
        let _permit = self.semaphore.acquire().await;

        Connection::open_with_flags(
            &self.db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| rusqlite::Error::SqliteSingleThreadedMode)
    }

    /// 执行事务
    pub async fn transaction<F, R>(&self, f: F) -> SqliteResult<R>
    where
        F: FnOnce(Connection) -> SqliteResult<R> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.acquire().await?;

        let result = tokio::task::spawn_blocking(move || {
            conn.execute("BEGIN IMMEDIATE TRANSACTION", [])?;
            let result = f(conn);
            if result.is_ok() {
                conn.execute("COMMIT", [])?;
            } else {
                conn.execute("ROLLBACK", [])?;
            }
            result
        })
        .await
        .map_err(|e| rusqlite::Error::SqliteFailure(e.to_string()))??;

        result
    }

    /// 获取数据库路径
    pub fn path(&self) -> &Path {
        self.db_path.as_path()
    }

    /// 获取最大连接数
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_pool_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = DatabasePool::new(temp_file.path().to_path_buf(), 5).unwrap();

        assert_eq!(pool.max_connections(), 5);
        assert_eq!(pool.path(), temp_file.path());
    }

    #[tokio::test]
    async fn test_acquire_connection() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = DatabasePool::new(temp_file.path().to_path_buf(), 2).unwrap();

        let conn1 = pool.acquire().await.unwrap();
        let conn2 = pool.acquire().await.unwrap();

        // 执行简单查询测试连接
        conn1.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
        conn2.execute("INSERT INTO test (id) VALUES (1)", []).unwrap();

        drop(conn1);
        drop(conn2);
    }

    #[tokio::test]
    async fn test_transaction() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = DatabasePool::new(temp_file.path().to_path_buf(), 2).unwrap();

        // 初始化表
        pool.acquire()
            .await
            .unwrap()
            .execute("CREATE TABLE test (id INTEGER)", [])
            .unwrap();

        // 成功的事务
        let result: SqliteResult<i64> = pool
            .transaction(|conn| {
                conn.execute("INSERT INTO test (id) VALUES (1)", [])?;
                conn.execute("INSERT INTO test (id) VALUES (2)", [])?;
                Ok(2)
            })
            .await
            .unwrap();

        assert_eq!(result, 2);

        // 失败的事务（回滚）
        let result: SqliteResult<i64> = pool
            .transaction(|conn| {
                conn.execute("INSERT INTO test (id) VALUES (3)", [])?;
                conn.execute("INSERT INTO test (id) VALUES ('invalid')", [])?; // 类型错误
                Ok(1)
            })
            .await;

        assert!(result.is_err());

        // 验证回滚成功
        let conn = pool.acquire().await.unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 2); // 只有第一个事务的数据
    }
}
