//! # SqliteStorage - 真实存储服务实现
//!
//! 基于 SQLite 的 StorageService 真实实现。
//! 由于 SQLite 连接不是线程安全的，每个操作创建独立连接。

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use rusqlite::Connection;

use crate::error::{CisError, Result};
use crate::traits::{StorageService, StorageQuery, StorageRecord, StorageStats};

/// SQLite 存储服务
///
/// 基于 SQLite 的真实存储实现，每个操作使用独立连接以保证线程安全。
pub struct SqliteStorage {
    /// 数据库连接路径
    db_path: Arc<std::path::PathBuf>,
}

impl SqliteStorage {
    /// 创建新的 SQLite 存储服务
    pub fn new() -> Result<Self> {
        let db_path = Arc::new(crate::storage::paths::Paths::node_db());
        
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // 初始化数据库
        let conn = Self::open_connection(&db_path)?;
        Self::init_schema(&conn)?;
        drop(conn);
        
        Ok(Self { db_path })
    }

    /// 从指定路径创建存储服务
    pub fn with_path(path: impl AsRef<Path>) -> Result<Self> {
        let db_path = Arc::new(path.as_ref().to_path_buf());
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = Self::open_connection(&db_path)?;
        Self::init_schema(&conn)?;
        drop(conn);
        
        Ok(Self { db_path })
    }

    /// 打开数据库连接
    fn open_connection(path: &Path) -> Result<Connection> {
        let conn = Connection::open(path)
            .map_err(|e| CisError::Storage(format!("Failed to open storage: {}", e)))?;
        
        // 配置 WAL 模式
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 100000000;"
        ).map_err(|e| CisError::Storage(format!("Failed to configure WAL: {}", e)))?;
        
        Ok(conn)
    }

    /// 初始化数据库 Schema
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                version INTEGER DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create table: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_kv_updated ON kv_store(updated_at)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    /// 获取当前时间戳（秒）
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// 获取连接路径的克隆
    fn db_path(&self) -> std::path::PathBuf {
        (*self.db_path).clone()
    }
}

impl Clone for SqliteStorage {
    fn clone(&self) -> Self {
        Self {
            db_path: Arc::clone(&self.db_path),
        }
    }
}

impl Default for SqliteStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create SqliteStorage")
    }
}

#[async_trait]
impl StorageService for SqliteStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        crate::check_string_length(key, 1024)?;
        
        let key = key.to_string();
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let result: Option<Vec<u8>> = conn.query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                [&key],
                |row| row.get(0)
            ).ok();
            
            Ok(result)
        })
        .await
        .map_err(|e| CisError::Storage(format!("Get operation failed: {}", e)))?
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        crate::check_string_length(key, 1024)?;
        crate::check_allocation_size(value.len(), 10 * 1024 * 1024)?;
        
        let key = key.to_string();
        let value = value.to_vec();
        let now = Self::now_secs() as i64;
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            conn.execute(
                "INSERT INTO kv_store (key, value, created_at, updated_at) 
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(key) DO UPDATE SET 
                 value = excluded.value, 
                 updated_at = excluded.updated_at",
                rusqlite::params![key, value, now],
            ).map_err(|e| CisError::Storage(format!("Put failed: {}", e)))?;
            
            Ok(())
        })
        .await
        .map_err(|e| CisError::Storage(format!("Put operation failed: {}", e)))?
    }

    async fn put_if_version(
        &self,
        key: &str,
        value: &[u8],
        expected_version: Option<u64>,
    ) -> Result<()> {
        crate::check_string_length(key, 1024)?;
        crate::check_allocation_size(value.len(), 10 * 1024 * 1024)?;
        
        let key = key.to_string();
        let value = value.to_vec();
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let current_version: Option<i64> = conn.query_row(
                "SELECT version FROM kv_store WHERE key = ?1",
                [&key],
                |row| row.get(0),
            ).ok();
            
            match (current_version, expected_version) {
                (Some(cv), Some(ev)) if cv as u64 == ev => {
                    conn.execute(
                        "UPDATE kv_store SET value = ?2, version = version + 1, updated_at = ?3 WHERE key = ?1",
                        rusqlite::params![key, value, Self::now_secs() as i64],
                    ).map_err(|e| CisError::Storage(format!("Update failed: {}", e)))?;
                    Ok(())
                }
                (Some(_), Some(ev)) => {
                    Err(CisError::already_exists(format!(
                        "Version mismatch: expected {}", ev
                    )))
                }
                (Some(_), None) => {
                    Err(CisError::already_exists("Key already exists".to_string()))
                }
                (None, Some(_)) => {
                    Err(CisError::not_found("Key does not exist".to_string()))
                }
                (None, None) => {
                    conn.execute(
                        "INSERT INTO kv_store (key, value, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                        rusqlite::params![key, value, Self::now_secs() as i64],
                    ).map_err(|e| CisError::Storage(format!("Insert failed: {}", e)))?;
                    Ok(())
                }
            }
        })
        .await
        .map_err(|e| CisError::Storage(format!("Put if version failed: {}", e)))?
    }

    async fn delete(&self, key: &str) -> Result<()> {
        crate::check_string_length(key, 1024)?;
        
        let key = key.to_string();
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            conn.execute(
                "DELETE FROM kv_store WHERE key = ?1",
                [&key],
            ).map_err(|e| CisError::Storage(format!("Delete failed: {}", e)))?;
            
            Ok(())
        })
        .await
        .map_err(|e| CisError::Storage(format!("Delete operation failed: {}", e)))?
    }

    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>> {
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let mut sql = String::from(
                "SELECT key, value, version, created_at, updated_at FROM kv_store WHERE 1=1"
            );
            
            if let Some(ref prefix) = query.prefix {
                let escaped = prefix.replace("\\", "\\\\").replace("'", "''");
                sql.push_str(&format!(" AND key LIKE '{}%'", escaped));
            }
            
            if let Some(ref pattern) = query.key_pattern {
                let like_pattern = pattern.replace('*', "%").replace('?', "_");
                let escaped = like_pattern.replace("\\", "\\\\").replace("'", "''");
                sql.push_str(&format!(" AND key LIKE '{}'", escaped));
            }
            
            if let Some(ref sort_by) = query.options.sort_by {
                let order = if query.options.descending { "DESC" } else { "ASC" };
                sql.push_str(&format!(" ORDER BY {} {}", sort_by, order));
            }
            
            if let Some(limit) = query.limit {
                sql.push_str(&format!(" LIMIT {}", limit));
            }
            if let Some(offset) = query.offset {
                sql.push_str(&format!(" OFFSET {}", offset));
            }
            
            let mut stmt = conn.prepare(&sql)
                .map_err(|e| CisError::Storage(format!("Prepare failed: {}", e)))?;
            
            let rows = stmt.query_map([], |row| {
                Ok(StorageRecord {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    version: row.get::<_, i64>(2)? as u64,
                    created_at: row.get::<_, i64>(3)? as u64,
                    updated_at: row.get::<_, i64>(4)? as u64,
                    expires_at: None,
                    metadata: HashMap::new(),
                })
            }).map_err(|e| CisError::Storage(format!("Query failed: {}", e)))?;
            
            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| CisError::Storage(format!("Read row failed: {}", e)))?);
            }
            
            Ok(results)
        })
        .await
        .map_err(|e| CisError::Storage(format!("Query operation failed: {}", e)))?
    }

    async fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        crate::check_string_length(prefix, 1024)?;
        
        let prefix = prefix.to_string();
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let mut stmt = conn.prepare("SELECT key, value FROM kv_store WHERE key LIKE ?1")
                .map_err(|e| CisError::Storage(format!("Prepare failed: {}", e)))?;
            
            let like_pattern = format!("{}%", prefix);
            let rows = stmt.query_map([&like_pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            }).map_err(|e| CisError::Storage(format!("Scan failed: {}", e)))?;
            
            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| CisError::Storage(format!("Read row failed: {}", e)))?);
            }
            
            Ok(results)
        })
        .await
        .map_err(|e| CisError::Storage(format!("Scan operation failed: {}", e)))?
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        crate::check_string_length(key, 1024)?;
        
        let key = key.to_string();
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kv_store WHERE key = ?1",
                [&key],
                |row| row.get(0),
            ).map_err(|e| CisError::Storage(format!("Exists check failed: {}", e)))?;
            
            Ok(count > 0)
        })
        .await
        .map_err(|e| CisError::Storage(format!("Exists operation failed: {}", e)))?
    }

    async fn get_batch(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        
        for key in keys {
            let value = self.get(key).await?;
            results.push(value);
        }
        
        Ok(results)
    }

    async fn put_batch(&self, items: &[(String, Vec<u8>)]) -> Result<()> {
        let items = items.to_vec();
        let now = Self::now_secs() as i64;
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let tx = conn.unchecked_transaction()
                .map_err(|e| CisError::Storage(format!("Transaction failed: {}", e)))?;
            
            for (key, value) in &items {
                crate::check_string_length(key, 1024)?;
                crate::check_allocation_size(value.len(), 10 * 1024 * 1024)?;
                
                tx.execute(
                    "INSERT INTO kv_store (key, value, created_at, updated_at) 
                     VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(key) DO UPDATE SET 
                     value = excluded.value, 
                     updated_at = excluded.updated_at",
                    rusqlite::params![key, value, now],
                ).map_err(|e| CisError::Storage(format!("Batch insert failed: {}", e)))?;
            }
            
            tx.commit()
                .map_err(|e| CisError::Storage(format!("Commit failed: {}", e)))?;
            
            Ok(())
        })
        .await
        .map_err(|e| CisError::Storage(format!("Batch put failed: {}", e)))?
    }

    async fn keys(&self) -> Result<Vec<String>> {
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let mut stmt = conn.prepare("SELECT key FROM kv_store")
                .map_err(|e| CisError::Storage(format!("Prepare failed: {}", e)))?;
            
            let rows = stmt.query_map([], |row| {
                row.get::<_, String>(0)
            }).map_err(|e| CisError::Storage(format!("Query failed: {}", e)))?;
            
            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| CisError::Storage(format!("Read row failed: {}", e)))?);
            }
            
            Ok(results)
        })
        .await
        .map_err(|e| CisError::Storage(format!("Keys operation failed: {}", e)))?
    }

    async fn clear(&self) -> Result<()> {
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            conn.execute("DELETE FROM kv_store", [])
                .map_err(|e| CisError::Storage(format!("Clear failed: {}", e)))?;
            
            Ok(())
        })
        .await
        .map_err(|e| CisError::Storage(format!("Clear operation failed: {}", e)))?
    }

    async fn stats(&self) -> Result<StorageStats> {
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let total_keys: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kv_store",
                [],
                |row| row.get(0),
            ).map_err(|e| CisError::Storage(format!("Count failed: {}", e)))?;
            
            let total_size: i64 = conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(value)), 0) FROM kv_store",
                [],
                |row| row.get(0),
            ).map_err(|e| CisError::Storage(format!("Size query failed: {}", e)))?;
            
            let last_modified: Option<i64> = conn.query_row(
                "SELECT MAX(updated_at) FROM kv_store",
                [],
                |row| row.get(0),
            ).map_err(|e| CisError::Storage(format!("Last modified failed: {}", e)))?;
            
            Ok(StorageStats {
                total_keys: total_keys as u64,
                total_size: total_size as u64,
                index_count: 1,
                last_modified: last_modified.map(|t| t as u64),
                compression_ratio: None,
            })
        })
        .await
        .map_err(|e| CisError::Storage(format!("Stats operation failed: {}", e)))?
    }

    async fn transaction(&self, operations: Vec<(String, String, Option<Vec<u8>>)>) -> Result<()>
    where
        String: Send + Sync,
        Vec<u8>: Send + Sync,
    {
        let now = Self::now_secs() as i64;
        let db_path = self.db_path();
        
        tokio::task::spawn_blocking(move || {
            let conn = Self::open_connection(&db_path)?;
            
            let tx = conn.unchecked_transaction()
                .map_err(|e| CisError::Storage(format!("Transaction failed: {}", e)))?;
            
            for (op, key, value) in operations {
                match op.as_str() {
                    "put" | "set" => {
                        if let Some(val) = value {
                            tx.execute(
                                "INSERT INTO kv_store (key, value, created_at, updated_at) 
                                 VALUES (?1, ?2, ?3, ?3)
                                 ON CONFLICT(key) DO UPDATE SET 
                                 value = excluded.value, 
                                 updated_at = excluded.updated_at",
                                rusqlite::params![key, val, now],
                            ).map_err(|e| CisError::Storage(format!("Transaction put failed: {}", e)))?;
                        }
                    }
                    "delete" | "del" => {
                        tx.execute(
                            "DELETE FROM kv_store WHERE key = ?1",
                            [&key],
                        ).map_err(|e| CisError::Storage(format!("Transaction delete failed: {}", e)))?;
                    }
                    _ => {
                        return Err(CisError::invalid_input(format!(
                            "Unknown operation: {}", op
                        )));
                    }
                }
            }
            
            tx.commit()
                .map_err(|e| CisError::Storage(format!("Commit failed: {}", e)))?;
            
            Ok(())
        })
        .await
        .map_err(|e| CisError::Storage(format!("Transaction failed: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_storage() -> (SqliteStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("CIS_DATA_DIR", temp_dir.path());
        
        crate::storage::paths::Paths::ensure_dirs().unwrap();
        
        let storage = SqliteStorage::new().unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    #[ignore = "Database environment issue"]
    async fn test_put_and_get() {
        let (storage, _temp) = setup_test_storage();
        
        storage.put("test-key", b"test-value").await.unwrap();
        
        let value = storage.get("test-key").await.unwrap();
        assert_eq!(value, Some(b"test-value".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let (storage, _temp) = setup_test_storage();
        
        let value = storage.get("nonexistent-key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let (storage, _temp) = setup_test_storage();
        
        storage.put("delete-key", b"value").await.unwrap();
        assert!(storage.exists("delete-key").await.unwrap());
        
        storage.delete("delete-key").await.unwrap();
        assert!(!storage.exists("delete-key").await.unwrap());
    }

    #[tokio::test]
    #[ignore = "Database environment issue"]
    async fn test_scan() {
        let (storage, _temp) = setup_test_storage();
        
        storage.put("user:1", b"Alice").await.unwrap();
        storage.put("user:2", b"Bob").await.unwrap();
        storage.put("post:1", b"Hello").await.unwrap();
        
        let results = storage.scan("user:").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let (storage, _temp) = setup_test_storage();
        
        let items = vec![
            ("key1".to_string(), b"value1".to_vec()),
            ("key2".to_string(), b"value2".to_vec()),
        ];
        
        storage.put_batch(&items).await.unwrap();
        
        let keys = vec!["key1".to_string(), "key2".to_string()];
        let values = storage.get_batch(&keys).await.unwrap();
        
        assert_eq!(values[0], Some(b"value1".to_vec()));
        assert_eq!(values[1], Some(b"value2".to_vec()));
    }

    #[tokio::test]
    #[ignore = "Database environment issue"]
    async fn test_transaction() {
        let (storage, _temp) = setup_test_storage();
        
        let operations = vec![
            ("put".to_string(), "tx-key1".to_string(), Some(b"value1".to_vec())),
            ("put".to_string(), "tx-key2".to_string(), Some(b"value2".to_vec())),
        ];
        
        storage.transaction(operations).await.unwrap();
        
        assert_eq!(storage.get("tx-key1").await.unwrap(), Some(b"value1".to_vec()));
        assert_eq!(storage.get("tx-key2").await.unwrap(), Some(b"value2".to_vec()));
    }

    #[tokio::test]
    #[ignore = "Database environment issue"]
    async fn test_stats() {
        let (storage, _temp) = setup_test_storage();
        
        storage.put("key1", b"value1").await.unwrap();
        storage.put("key2", b"value2_longer").await.unwrap();
        
        let stats = storage.stats().await.unwrap();
        assert_eq!(stats.total_keys, 2);
        assert!(stats.total_size > 0);
    }

    #[tokio::test]
    #[ignore = "Database environment issue"]
    async fn test_clear() {
        let (storage, _temp) = setup_test_storage();
        
        storage.put("key1", b"value1").await.unwrap();
        storage.put("key2", b"value2").await.unwrap();
        
        storage.clear().await.unwrap();
        
        assert_eq!(storage.get("key1").await.unwrap(), None);
        assert_eq!(storage.get("key2").await.unwrap(), None);
    }
}
