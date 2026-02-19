//! Independent Memory Database
//!
//! Separated from core.db, supports private/public memory separated storage with independent WAL files.
//! Private memory is encrypted, public memory supports federation sync.

use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::error::{CisError, Result};
use crate::types::{MemoryCategory, MemoryDomain};

/// Memory entry
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Independent memory database
///
/// Stores private and public memory, separated from core database
pub struct MemoryDb {
    conn: Connection,
    path: PathBuf,
}

impl MemoryDb {
    /// Open memory database (create if not exists)
    pub fn open(path: &Path) -> Result<Self> {
        // Create directory
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::storage(format!("Failed to create directory: {}", e)))?;
        }

        // Open connection
        let conn = Connection::open(path)
            .map_err(|e| CisError::storage(format!("Failed to open memory db: {}", e)))?;

        let db = Self {
            conn,
            path: path.to_path_buf(),
        };

        // Configure WAL mode
        db.configure_wal()?;

        // Initialize schema
        db.init_schema()?;

        Ok(db)
    }

    /// Open memory database with default path
    pub fn open_default() -> Result<Self> {
        use super::paths::Paths;
        Self::open(&Paths::memory_db())
    }

    /// Configure WAL mode (safe for sudden shutdown)
    fn configure_wal(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 100000000;
             PRAGMA temp_store = memory;",
        ).map_err(|e| CisError::storage(format!("Failed to configure WAL: {}", e)))?;
        Ok(())
    }

    /// Initialize Schema
    fn init_schema(&self) -> Result<()> {
        // Private memory table (encrypted)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS private_entries (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                category TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                encrypted INTEGER DEFAULT 1
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create private_entries table: {}", e)))?;

        // Public memory table (federatable sync)
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS public_entries (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                category TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                federate INTEGER DEFAULT 1,
                sync_status TEXT DEFAULT 'pending'
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create public_entries table: {}", e)))?;

        // Memory index table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_index (
                key TEXT PRIMARY KEY,
                domain TEXT, -- 'private' or 'public'
                category TEXT,
                skill_name TEXT,
                created_at INTEGER
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create memory_index table: {}", e)))?;

        // Create indexes
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_private_category ON private_entries(category)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_public_sync ON public_entries(sync_status)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_domain ON memory_index(domain)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_skill ON memory_index(skill_name)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    /// 获取数据库路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取底层连接（用于复杂查询）
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 存储私域记忆
    pub fn set_private(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let category_str = format!("{:?}", category);

        self.conn.execute(
            "INSERT INTO private_entries (key, value, category, created_at, updated_at, encrypted)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             category = excluded.category,
             updated_at = excluded.updated_at",
            rusqlite::params![key, value, category_str, now, now],
        ).map_err(|e| CisError::storage(format!("Failed to set private memory: {}", e)))?;

        // 更新索引
        self.update_index(key, MemoryDomain::Private, category, None)?;

        Ok(())
    }

    /// 存储公域记忆
    pub fn set_public(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let category_str = format!("{:?}", category);

        self.conn.execute(
            "INSERT INTO public_entries (key, value, category, created_at, updated_at, federate, sync_status)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 'pending')
             ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             category = excluded.category,
             updated_at = excluded.updated_at,
             sync_status = 'pending'",
            rusqlite::params![key, value, category_str, now, now],
        ).map_err(|e| CisError::storage(format!("Failed to set public memory: {}", e)))?;

        // 更新索引
        self.update_index(key, MemoryDomain::Public, category, None)?;

        Ok(())
    }

    /// 存储记忆（指定域）
    pub fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        match domain {
            MemoryDomain::Private => self.set_private(key, value, category),
            MemoryDomain::Public => self.set_public(key, value, category),
        }
    }

    /// 读取记忆（自动判断私域/公域）
    pub fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        // 先尝试私域
        if let Some(entry) = self.get_private(key)? {
            return Ok(Some(entry));
        }

        // 再尝试公域
        if let Some(entry) = self.get_public(key)? {
            return Ok(Some(entry));
        }

        Ok(None)
    }

    /// 读取私域记忆
    fn get_private(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, category, created_at, updated_at FROM private_entries WHERE key = ?1"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt.query_row([key], |row| {
            Ok(MemoryEntry {
                key: row.get(0)?,
                value: row.get(1)?,
                domain: MemoryDomain::Private,
                category: parse_category(&row.get::<_, String>(2)?),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get private memory: {}", e))),
        }
    }

    /// 读取公域记忆
    fn get_public(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, value, category, created_at, updated_at FROM public_entries WHERE key = ?1"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt.query_row([key], |row| {
            Ok(MemoryEntry {
                key: row.get(0)?,
                value: row.get(1)?,
                domain: MemoryDomain::Public,
                category: parse_category(&row.get::<_, String>(2)?),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        });

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get public memory: {}", e))),
        }
    }

    /// 删除记忆
    pub fn delete(&self, key: &str) -> Result<bool> {
        let domain = self.get_domain(key)?;

        let deleted = match domain {
            Some(MemoryDomain::Private) => {
                self.conn.execute(
                    "DELETE FROM private_entries WHERE key = ?1",
                    [key],
                ).map_err(|e| CisError::storage(format!("Failed to delete private memory: {}", e)))?
            }
            Some(MemoryDomain::Public) => {
                self.conn.execute(
                    "DELETE FROM public_entries WHERE key = ?1",
                    [key],
                ).map_err(|e| CisError::storage(format!("Failed to delete public memory: {}", e)))?
            }
            None => return Ok(false),
        };

        // 删除索引
        if deleted > 0 {
            self.conn.execute(
                "DELETE FROM memory_index WHERE key = ?1",
                [key],
            ).map_err(|e| CisError::storage(format!("Failed to delete index: {}", e)))?;
        }

        Ok(deleted > 0)
    }

    /// 获取记忆的域
    fn get_domain(&self, key: &str) -> Result<Option<MemoryDomain>> {
        // 检查私域
        let exists_private: bool = self.conn.query_row(
            "SELECT 1 FROM private_entries WHERE key = ?1 LIMIT 1",
            [key],
            |_| Ok(true),
        ).unwrap_or(false);

        if exists_private {
            return Ok(Some(MemoryDomain::Private));
        }

        // 检查公域
        let exists_public: bool = self.conn.query_row(
            "SELECT 1 FROM public_entries WHERE key = ?1 LIMIT 1",
            [key],
            |_| Ok(true),
        ).unwrap_or(false);

        if exists_public {
            return Ok(Some(MemoryDomain::Public));
        }

        Ok(None)
    }

    /// 列出记忆键
    pub fn list_keys(&self, prefix: &str, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        match domain {
            Some(MemoryDomain::Private) | None => {
                let like = format!("{}%", prefix);
                let mut stmt = self.conn.prepare(
                    "SELECT key FROM private_entries WHERE key LIKE ?1"
                ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

                let rows = stmt.query_map([&like], |row| {
                    row.get::<_, String>(0)
                }).map_err(|e| CisError::storage(format!("Failed to query keys: {}", e)))?;

                for row in rows {
                    keys.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
                }
            }
            _ => {}
        }

        match domain {
            Some(MemoryDomain::Public) | None => {
                let like = format!("{}%", prefix);
                let mut stmt = self.conn.prepare(
                    "SELECT key FROM public_entries WHERE key LIKE ?1"
                ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

                let rows = stmt.query_map([&like], |row| {
                    row.get::<_, String>(0)
                }).map_err(|e| CisError::storage(format!("Failed to query keys: {}", e)))?;

                for row in rows {
                    keys.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
                }
            }
            _ => {}
        }

        Ok(keys)
    }

    /// 获取待同步的公域记忆（用于 P2P 同步）
    pub fn get_pending_sync(&self, limit: usize) -> Result<Vec<MemoryEntry>> {
        let mut entries = Vec::new();

        let mut stmt = self.conn.prepare(
            "SELECT key, value, category, created_at, updated_at 
             FROM public_entries 
             WHERE sync_status = 'pending' AND federate = 1
             LIMIT ?1"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map([limit as i64], |row| {
            Ok(MemoryEntry {
                key: row.get(0)?,
                value: row.get(1)?,
                domain: MemoryDomain::Public,
                category: parse_category(&row.get::<_, String>(2)?),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        }).map_err(|e| CisError::storage(format!("Failed to query pending entries: {}", e)))?;

        for row in rows {
            entries.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
        }

        Ok(entries)
    }

    /// 标记已同步
    pub fn mark_synced(&self, key: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE public_entries SET sync_status = 'synced' WHERE key = ?1",
            [key],
        ).map_err(|e| CisError::storage(format!("Failed to mark synced: {}", e)))?;
        Ok(())
    }

    /// 更新记忆索引
    fn update_index(
        &self,
        key: &str,
        domain: MemoryDomain,
        category: MemoryCategory,
        skill_name: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let domain_str = format!("{:?}", domain);
        let category_str = format!("{:?}", category);

        self.conn.execute(
            "INSERT INTO memory_index (key, domain, category, skill_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key) DO UPDATE SET
             domain = excluded.domain,
             category = excluded.category,
             skill_name = excluded.skill_name",
            rusqlite::params![key, domain_str, category_str, skill_name.unwrap_or(""), now],
        ).map_err(|e| CisError::storage(format!("Failed to update index: {}", e)))?;

        Ok(())
    }

    /// 执行 checkpoint
    pub fn checkpoint(&self) -> Result<()> {
        self.conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])
            .map_err(|e| CisError::storage(format!("Failed to checkpoint: {}", e)))?;
        Ok(())
    }

    /// 关闭连接（执行 checkpoint）
    pub fn close(self) -> Result<()> {
        // 执行 checkpoint
        let _ = self.checkpoint();

        // 关闭连接
        self.conn.close()
            .map_err(|(_, e)| CisError::storage(format!("Failed to close memory db: {}", e)))
    }
}

/// 解析 category 字符串
fn parse_category(s: &str) -> MemoryCategory {
    match s {
        "Execution" => MemoryCategory::Execution,
        "Result" => MemoryCategory::Result,
        "Error" => MemoryCategory::Error,
        "Context" => MemoryCategory::Context,
        "Skill" => MemoryCategory::Skill,
        _ => MemoryCategory::Context,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_db() -> (MemoryDb, std::path::PathBuf) {
        // 使用唯一的临时目录避免测试间干扰
        let temp_dir = env::temp_dir().join(format!("cis_test_memory_db_{}", std::process::id()))
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("memory.db");
        (MemoryDb::open(&db_path).unwrap(), temp_dir)
    }

    fn cleanup_test_db(temp_dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_memory_db_basic() {
        let (db, temp_dir) = setup_test_db();

        // 存储私域记忆
        db.set_private("private_key", b"private_value", MemoryCategory::Context).unwrap();

        // 存储公域记忆
        db.set_public("public_key", b"public_value", MemoryCategory::Result).unwrap();

        // 读取私域
        let entry = db.get("private_key").unwrap().unwrap();
        assert_eq!(entry.key, "private_key");
        assert_eq!(entry.value, b"private_value");
        assert!(matches!(entry.domain, MemoryDomain::Private));
        assert!(matches!(entry.category, MemoryCategory::Context));

        // 读取公域
        let entry = db.get("public_key").unwrap().unwrap();
        assert_eq!(entry.key, "public_key");
        assert_eq!(entry.value, b"public_value");
        assert!(matches!(entry.domain, MemoryDomain::Public));
        assert!(matches!(entry.category, MemoryCategory::Result));

        db.close().unwrap();
        cleanup_test_db(&temp_dir);
    }

    #[test]
    fn test_memory_db_list_keys() {
        let (db, temp_dir) = setup_test_db();

        db.set_private("prefix/a", b"1", MemoryCategory::Context).unwrap();
        db.set_private("prefix/b", b"2", MemoryCategory::Context).unwrap();
        db.set_public("prefix/c", b"3", MemoryCategory::Result).unwrap();
        db.set_public("other/d", b"4", MemoryCategory::Result).unwrap();

        // 列出所有带 prefix 的键
        let keys = db.list_keys("prefix/", None).unwrap();
        assert_eq!(keys.len(), 3);

        // 只列私域
        let keys = db.list_keys("prefix/", Some(MemoryDomain::Private)).unwrap();
        assert_eq!(keys.len(), 2);

        // 只列公域
        let keys = db.list_keys("prefix/", Some(MemoryDomain::Public)).unwrap();
        assert_eq!(keys.len(), 1);

        db.close().unwrap();
        cleanup_test_db(&temp_dir);
    }

    #[test]
    fn test_memory_db_delete() {
        let (db, temp_dir) = setup_test_db();

        db.set_private("to_delete", b"value", MemoryCategory::Context).unwrap();
        assert!(db.get("to_delete").unwrap().is_some());

        let deleted = db.delete("to_delete").unwrap();
        assert!(deleted);
        assert!(db.get("to_delete").unwrap().is_none());

        // 删除不存在的
        let deleted = db.delete("nonexistent").unwrap();
        assert!(!deleted);

        db.close().unwrap();
        cleanup_test_db(&temp_dir);
    }

    #[test]
    fn test_memory_db_sync() {
        let (db, temp_dir) = setup_test_db();

        db.set_public("sync1", b"value1", MemoryCategory::Result).unwrap();
        db.set_public("sync2", b"value2", MemoryCategory::Result).unwrap();
        db.set_private("private1", b"value3", MemoryCategory::Context).unwrap();

        // 获取待同步条目
        let pending = db.get_pending_sync(10).unwrap();
        assert_eq!(pending.len(), 2);

        // 标记已同步
        db.mark_synced("sync1").unwrap();

        // 再次获取待同步
        let pending = db.get_pending_sync(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].key, "sync2");

        db.close().unwrap();
        cleanup_test_db(&temp_dir);
    }
}
