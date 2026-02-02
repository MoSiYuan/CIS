//! 数据库管理模块
//!
//! 核心数据库与 Skill 数据库严格分离，支持热插拔。

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use super::connection::MultiDbConnection;
use super::paths::Paths;
use super::wal::{checkpoint, set_wal_mode, WALConfig};
use crate::error::{CisError, Result};

/// 核心数据库
///
/// 存储 CIS 核心数据：任务、配置、节点信息等
pub struct CoreDb {
    conn: Connection,
}

impl CoreDb {
    /// 打开核心数据库
    pub fn open() -> Result<Self> {
        let db_path = Paths::node_db();
        
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| CisError::Storage(format!("Failed to open core db: {}", e)))?;

        let db = Self { conn };
        db.configure_wal()?;
        db.init_schema()?;
        
        Ok(db)
    }

    /// 配置 WAL 模式（随时关机安全）
    pub fn configure_wal(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 100000000;
             PRAGMA temp_store = memory;"
        ).map_err(|e| CisError::Storage(format!("Failed to configure WAL: {}", e)))?;
        Ok(())
    }

    /// 初始化核心数据库 Schema
    fn init_schema(&self) -> Result<()> {
        // 任务表
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                group_name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                priority INTEGER DEFAULT 2,
                dependencies TEXT, -- JSON array of task IDs
                result TEXT,
                error TEXT,
                workspace_dir TEXT,
                sandboxed BOOLEAN DEFAULT 1,
                allow_network BOOLEAN DEFAULT 0,
                created_at INTEGER NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                node_id TEXT,
                metadata TEXT -- JSON object
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create tasks table: {}", e)))?;

        // 核心配置表
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS core_config (
                key TEXT PRIMARY KEY,
                value BLOB,
                encrypted BOOLEAN DEFAULT 0,
                updated_at INTEGER
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create config table: {}", e)))?;

        // 记忆索引表（引用 Skill 数据，不存储实际 value）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS memory_index (
                key TEXT PRIMARY KEY,
                skill_name TEXT, -- NULL 表示核心
                storage_type TEXT NOT NULL CHECK(storage_type IN ('core', 'skill')),
                category TEXT,
                domain TEXT DEFAULT 'private',
                created_at INTEGER,
                updated_at INTEGER,
                accessed_at INTEGER,
                version INTEGER DEFAULT 1
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create memory_index table: {}", e)))?;

        // P2P 节点信息表（预留）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS peers (
                node_id TEXT PRIMARY KEY,
                public_key TEXT,
                last_seen INTEGER,
                endpoint TEXT,
                status TEXT DEFAULT 'offline',
                metadata TEXT
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create peers table: {}", e)))?;

        // 创建索引
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_skill ON memory_index(skill_name)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    /// 获取底层连接（用于复杂查询）
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 设置配置项
    pub fn set_config(&self, key: &str, value: &[u8], encrypted: bool) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO core_config (key, value, encrypted, updated_at) 
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(key) DO UPDATE SET 
             value = excluded.value, 
             encrypted = excluded.encrypted,
             updated_at = excluded.updated_at",
            rusqlite::params![key, value, encrypted, now],
        ).map_err(|e| CisError::Storage(format!("Failed to set config: {}", e)))?;
        Ok(())
    }

    /// 获取配置项
    pub fn get_config(&self, key: &str) -> Result<Option<(Vec<u8>, bool)>> {
        let mut stmt = self.conn.prepare(
            "SELECT value, encrypted FROM core_config WHERE key = ?1"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt.query_row([key], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, bool>(1)?))
        });

        match result {
            Ok(data) => Ok(Some(data)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::Storage(format!("Failed to get config: {}", e))),
        }
    }

    /// 注册记忆索引（引用 Skill 数据）
    pub fn register_memory_index(
        &self,
        key: &str,
        skill_name: Option<&str>,
        storage_type: &str,
        category: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO memory_index 
             (key, skill_name, storage_type, category, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
             skill_name = excluded.skill_name,
             storage_type = excluded.storage_type,
             category = excluded.category,
             updated_at = excluded.updated_at",
            rusqlite::params![
                key,
                skill_name.unwrap_or(""),
                storage_type,
                category.unwrap_or(""),
                now,
                now
            ],
        ).map_err(|e| CisError::Storage(format!("Failed to register memory index: {}", e)))?;
        Ok(())
    }

    /// 获取记忆索引
    pub fn get_memory_index(&self, key: &str) -> Result<Option<MemoryIndex>> {
        let mut stmt = self.conn.prepare(
            "SELECT key, skill_name, storage_type, category, domain, 
                    created_at, updated_at, accessed_at, version
             FROM memory_index WHERE key = ?1"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt.query_row([key], |row| {
            Ok(MemoryIndex {
                key: row.get(0)?,
                skill_name: row.get::<_, Option<String>>(1)?,
                storage_type: row.get(2)?,
                category: row.get(3)?,
                domain: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                accessed_at: row.get(7)?,
                version: row.get(8)?,
            })
        });

        match result {
            Ok(index) => Ok(Some(index)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::Storage(format!("Failed to get memory index: {}", e))),
        }
    }

    /// 执行备份
    pub fn backup(&self, path: &Path) -> Result<()> {
        use rusqlite::backup::Backup;
        
        let mut dst = Connection::open(path)
            .map_err(|e| CisError::Storage(format!("Failed to open backup db: {}", e)))?;
        
        let backup = Backup::new(&self.conn, &mut dst)
            .map_err(|e| CisError::Storage(format!("Failed to create backup: {}", e)))?;
        
        backup
            .step(-1)
            .map_err(|e| CisError::Storage(format!("Failed to backup db: {}", e)))?;
        
        Ok(())
    }

    /// 关闭连接
    /// 
    /// 在关闭前执行 checkpoint 以确保数据完整性
    pub fn close(self) -> Result<()> {
        info!("Closing core database...");
        
        // 执行 checkpoint 确保 WAL 数据写入主数据库
        if let Err(e) = checkpoint(&self.conn) {
            warn!("Checkpoint before close failed: {}", e);
        }
        
        self.conn.close()
            .map_err(|(_, e)| CisError::Storage(format!("Failed to close core db: {}", e)))
    }

    /// 执行手动 checkpoint
    /// 
    /// 使用 TRUNCATE 模式，将 WAL 文件内容完全写入数据库并清空 WAL 文件
    pub fn checkpoint(&self) -> Result<()> {
        checkpoint(&self.conn)
            .map_err(|e| CisError::Storage(format!("Checkpoint failed: {}", e)))
    }

    /// 获取底层连接（用于复杂查询）
    /// 
    /// 注意：直接操作连接时请注意 WAL 模式的行为
    pub fn into_inner(self) -> Connection {
        self.conn
    }
}

/// 记忆索引记录
#[derive(Debug, Clone)]
pub struct MemoryIndex {
    pub key: String,
    pub skill_name: Option<String>,
    pub storage_type: String,
    pub category: String,
    pub domain: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub accessed_at: Option<i64>,
    pub version: i32,
}

/// Skill 数据库
///
/// 每个 Skill 拥有独立的数据库，支持热插拔
pub struct SkillDb {
    name: String,
    conn: Connection,
    path: std::path::PathBuf,
}

impl SkillDb {
    /// 打开 Skill 数据库
    /// 
    /// 自动配置 WAL 模式以确保随时关机安全
    pub fn open(skill_name: &str) -> Result<Self> {
        let db_path = Paths::skill_db(skill_name);
        
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| CisError::Storage(format!(
                "Failed to open skill db for {}: {}", skill_name, e
            )))?;

        // 配置 WAL 模式
        let config = WALConfig::default();
        set_wal_mode(&conn, &config)
            .map_err(|e| CisError::Storage(format!(
                "Failed to configure WAL for skill {}: {}", skill_name, e
            )))?;

        info!("Skill database opened with WAL mode: {:?}", db_path);

        Ok(Self {
            name: skill_name.to_string(),
            conn,
            path: db_path,
        })
    }

    /// 获取 Skill 名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取数据库路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取底层连接
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 执行初始化 SQL
    pub fn init_schema(&self, sql: &str) -> Result<()> {
        self.conn.execute_batch(sql)
            .map_err(|e| CisError::Storage(format!(
                "Failed to init schema for skill {}: {}", self.name, e
            )))?;
        Ok(())
    }

    /// 关闭连接（支持热插拔）
    /// 
    /// 在关闭前执行 checkpoint 以确保数据完整性
    pub fn close(self) -> Result<()> {
        info!("Closing skill database: {}", self.name);
        
        // 执行 checkpoint 确保 WAL 数据写入主数据库
        if let Err(e) = checkpoint(&self.conn) {
            warn!("Checkpoint before close failed for skill {}: {}", self.name, e);
        }
        
        self.conn.close()
            .map_err(|(_, e)| CisError::Storage(format!(
                "Failed to close skill db {}: {}", self.name, e
            )))
    }

    /// 执行手动 checkpoint
    /// 
    /// 使用 TRUNCATE 模式，将 WAL 文件内容完全写入数据库并清空 WAL 文件
    pub fn checkpoint(&self) -> Result<()> {
        checkpoint(&self.conn)
            .map_err(|e| CisError::Storage(format!(
                "Checkpoint failed for skill {}: {}", self.name, e
            )))
    }

    /// 获取底层连接（用于复杂查询）
    /// 
    /// 注意：直接操作连接时请注意 WAL 模式的行为
    pub fn into_inner(self) -> Connection {
        self.conn
    }
}

/// 数据库管理器
///
/// 管理核心数据库和所有 Skill 数据库的生命周期。
/// 使用 MultiDbConnection 支持跨库查询和 ATTACH/DETACH 机制。
pub struct DbManager {
    core: Arc<Mutex<CoreDb>>,
    skills: Arc<Mutex<HashMap<String, Arc<Mutex<SkillDb>>>>>,
    /// 多库连接（用于跨库查询）
    multi_conn: Arc<Mutex<Option<MultiDbConnection>>>,
    /// 已挂载的 Skill 别名映射: skill_name -> alias
    attached_aliases: Arc<Mutex<HashMap<String, String>>>,
}

impl DbManager {
    /// 创建新的数据库管理器
    pub fn new() -> Result<Self> {
        let core = CoreDb::open()?;
        
        Ok(Self {
            core: Arc::new(Mutex::new(core)),
            skills: Arc::new(Mutex::new(HashMap::new())),
            multi_conn: Arc::new(Mutex::new(None)),
            attached_aliases: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// 初始化多库连接
    ///
    /// 创建 MultiDbConnection 并挂载核心数据库。
    /// 应在应用启动后调用此方法。
    pub fn init_multi_connection(&self) -> Result<()> {
        let mut multi_conn = self.multi_conn.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        if multi_conn.is_some() {
            return Ok(());
        }

        let db_path = Paths::node_db();
        let conn = MultiDbConnection::open(&db_path)?;
        
        *multi_conn = Some(conn);
        
        tracing::info!("MultiDbConnection initialized");
        Ok(())
    }

    /// 获取多库连接
    ///
    /// 如果未初始化，会自动初始化
    pub fn multi_connection(&self) -> Result<Arc<Mutex<Option<MultiDbConnection>>>> {
        // 检查是否需要初始化
        {
            let multi_conn = self.multi_conn.lock()
                .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
            if multi_conn.is_some() {
                return Ok(self.multi_conn.clone());
            }
        }
        
        // 初始化
        self.init_multi_connection()?;
        Ok(self.multi_conn.clone())
    }

    /// ATTACH Skill 数据库
    ///
    /// 使用别名挂载 Skill 数据库到多库连接。
    /// 别名格式：`skill_{skill_name}`（将 - 替换为 _）
    pub fn attach_skill_db(&self, skill_name: &str) -> Result<String> {
        let mut multi_conn_guard = self.multi_conn.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        let conn = multi_conn_guard.as_mut()
            .ok_or_else(|| CisError::Storage("MultiDbConnection not initialized".to_string()))?;

        // 生成别名（SQLite 标识符规则）
        let alias = format!("skill_{}", skill_name.replace("-", "_").replace(".", "_"));

        // 检查是否已挂载
        if conn.is_attached(&alias) {
            return Ok(alias);
        }

        // 获取 Skill 数据库路径
        let db_path = Paths::skill_db(skill_name);

        // ATTACH
        conn.attach(&db_path, &alias)?;

        // 记录别名映射
        let mut aliases = self.attached_aliases.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        aliases.insert(skill_name.to_string(), alias.clone());

        tracing::info!("Skill database attached: {} as {}", skill_name, alias);

        Ok(alias)
    }

    /// DETACH Skill 数据库
    ///
    /// 卸载已挂载的 Skill 数据库
    pub fn detach_skill_db(&self, skill_name: &str) -> Result<()> {
        let mut multi_conn_guard = self.multi_conn.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        let conn = multi_conn_guard.as_mut()
            .ok_or_else(|| CisError::Storage("MultiDbConnection not initialized".to_string()))?;

        // 获取别名
        let alias = {
            let mut aliases = self.attached_aliases.lock()
                .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
            aliases.remove(skill_name)
        };

        if let Some(alias) = alias {
            if conn.is_attached(&alias) {
                conn.detach(&alias)?;
                tracing::info!("Skill database detached: {} ({})", skill_name, alias);
            }
        }

        Ok(())
    }

    /// 检查 Skill 数据库是否已挂载
    pub fn is_skill_attached(&self, skill_name: &str) -> Result<bool> {
        let aliases = self.attached_aliases.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        Ok(aliases.contains_key(skill_name))
    }

    /// 获取 Skill 的别名
    pub fn get_skill_alias(&self, skill_name: &str) -> Result<Option<String>> {
        let aliases = self.attached_aliases.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        Ok(aliases.get(skill_name).cloned())
    }

    /// 列出已挂载的 Skill
    pub fn list_attached_skills(&self) -> Result<Vec<(String, String)>> {
        let aliases = self.attached_aliases.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        Ok(aliases.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    /// 获取多库连接并 ATTACH 所有已加载的 Skills
    ///
    /// 用于在初始化后批量挂载所有已加载的 Skill 数据库
    pub fn attach_all_skills(&self) -> Result<Vec<String>> {
        let skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        let mut attached = Vec::new();
        for skill_name in skills.keys() {
            match self.attach_skill_db(skill_name) {
                Ok(alias) => attached.push(alias),
                Err(e) => {
                    tracing::warn!("Failed to attach skill {}: {}", skill_name, e);
                }
            }
        }
        
        Ok(attached)
    }

    /// 获取核心数据库引用
    pub fn core(&self) -> Arc<Mutex<CoreDb>> {
        self.core.clone()
    }

    /// 加载 Skill 数据库（热插拔入口）
    pub fn load_skill_db(&self, skill_name: &str) -> Result<Arc<Mutex<SkillDb>>> {
        let mut skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;

        if let Some(db) = skills.get(skill_name) {
            return Ok(db.clone());
        }

        let db = SkillDb::open(skill_name)?;
        let db_arc = Arc::new(Mutex::new(db));
        skills.insert(skill_name.to_string(), db_arc.clone());

        Ok(db_arc)
    }

    /// 卸载 Skill 数据库（热插拔出口）
    pub fn unload_skill_db(&self, skill_name: &str) -> Result<()> {
        let mut skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;

        if let Some(db) = skills.remove(skill_name) {
            // 尝试获取锁并关闭
            if let Ok(db) = Arc::try_unwrap(db) {
                if let Ok(db) = db.into_inner() {
                    db.close()?;
                }
            }
        }

        Ok(())
    }

    /// 检查 Skill 数据库是否已加载
    pub fn is_skill_loaded(&self, skill_name: &str) -> Result<bool> {
        let skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        Ok(skills.contains_key(skill_name))
    }

    /// 获取已加载的 Skill 列表
    pub fn loaded_skills(&self) -> Result<Vec<String>> {
        let skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        Ok(skills.keys().cloned().collect())
    }

    /// 关闭所有连接（应用退出时）
    /// 
    /// 执行 checkpoint 后关闭所有数据库连接，确保数据完整性
    pub fn shutdown(self) -> Result<()> {
        tracing::info!("Shutting down database manager...");
        
        // 关闭多库连接（会 DETACH 所有挂载的数据库）
        let mut multi_conn = self.multi_conn.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        if let Some(conn) = multi_conn.take() {
            if let Err(e) = conn.close() {
                tracing::warn!("Failed to close multi-connection: {}", e);
            }
        }
        drop(multi_conn);

        // 关闭所有 Skill 数据库
        let mut skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        for (name, db) in skills.drain() {
            if let Ok(db) = Arc::try_unwrap(db) {
                if let Ok(db) = db.into_inner() {
                    if let Err(e) = db.close() {
                        tracing::warn!("Failed to close skill db {}: {}", name, e);
                    }
                }
            }
        }

        // 关闭核心数据库
        if let Ok(core) = Arc::try_unwrap(self.core) {
            if let Ok(core) = core.into_inner() {
                core.close()?;
            }
        }

        tracing::info!("Database manager shutdown completed");
        Ok(())
    }

    /// 对所有数据库执行 checkpoint
    /// 
    /// 用于优雅关机或定期维护
    pub fn checkpoint_all(&self) -> Result<()> {
        info!("Executing checkpoint for all databases...");
        
        // Checkpoint 核心数据库
        {
            let core = self.core.lock()
                .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
            core.checkpoint()?;
        }

        // Checkpoint Skill 数据库
        let skills = self.skills.lock()
            .map_err(|e| CisError::Storage(format!("Lock failed: {}", e)))?;
        
        for (name, db) in skills.iter() {
            if let Ok(db) = db.lock() {
                if let Err(e) = db.checkpoint() {
                    warn!("Failed to checkpoint skill db {}: {}", name, e);
                }
            }
        }

        info!("Checkpoint all completed");
        Ok(())
    }
}

impl Default for DbManager {
    fn default() -> Self {
        Self::new().expect("Failed to create DbManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_env() {
        let temp_dir = std::env::temp_dir().join("cis_test_db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
        Paths::ensure_dirs().unwrap();
    }

    fn cleanup_test_env() {
        std::env::remove_var("CIS_DATA_DIR");
    }

    #[test]
    fn test_core_db() {
        setup_test_env();

        let db = CoreDb::open().unwrap();
        
        // 测试配置
        db.set_config("test_key", b"test_value", false).unwrap();
        let result = db.get_config("test_key").unwrap();
        assert!(result.is_some());
        let (value, encrypted) = result.unwrap();
        assert_eq!(value, b"test_value");
        assert!(!encrypted);

        cleanup_test_env();
    }

    #[test]
    fn test_skill_db_hotplug() {
        setup_test_env();

        let manager = DbManager::new().unwrap();

        // 加载 Skill 数据库
        let skill_db = manager.load_skill_db("test-skill").unwrap();
        assert!(manager.is_skill_loaded("test-skill").unwrap());

        // 使用数据库
        {
            let db = skill_db.lock().unwrap();
            db.init_schema("CREATE TABLE IF NOT EXISTS test (id INTEGER PRIMARY KEY)").unwrap();
        }

        // 卸载 Skill 数据库（热插拔）
        manager.unload_skill_db("test-skill").unwrap();
        assert!(!manager.is_skill_loaded("test-skill").unwrap());

        cleanup_test_env();
    }
}
