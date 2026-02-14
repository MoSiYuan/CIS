//! # 任务数据库模块
//!
//! 提供数据库连接池、Schema 初始化和迁移功能。

pub mod pool;
pub mod schema;

pub use pool::DatabasePool;
pub use schema::{initialize_schema, DatabaseStats, vacuum_database};

use std::path::PathBuf;
use std::sync::Arc;

/// 默认数据库路径
pub fn default_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cis")
        .join("data")
        .join("tasks.db")
}

/// 创建并初始化数据库连接池
pub async fn create_database_pool(
    db_path: Option<PathBuf>,
    max_connections: usize,
) -> Arc<DatabasePool> {
    let path = db_path.unwrap_or_else(default_db_path);
    let pool = DatabasePool::new(path, max_connections)
        .expect("Failed to create database pool");

    // 初始化 Schema
    let conn = pool.acquire().await
        .expect("Failed to acquire connection for schema initialization");

    initialize_schema(&conn).expect("Failed to initialize database schema");
    drop(conn);

    Arc::new(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_database_pool() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let pool = create_database_pool(Some(db_path), 5).await;

        // 验证池配置
        assert_eq!(pool.max_connections(), 5);
        assert!(pool.path().exists());

        // 验证数据库已初始化
        let conn = pool.acquire().await.unwrap();
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_count >= 8);
    }
}
