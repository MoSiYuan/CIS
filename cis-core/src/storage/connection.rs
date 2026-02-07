//! 多库连接管理
//!
//! 支持同时管理多个 SQLite 数据库（core, memory, federation, skills），
//! 并支持运行时 ATTACH 挂载，实现跨库查询能力。

use rusqlite::{Connection, Row as RusqliteRow, ToSql};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::error::{CisError, Result};

/// 多库连接管理器
///
/// 管理主连接和通过 ATTACH 挂载的其他数据库。
/// 支持跨库查询，使用 `alias.table` 语法访问已挂载的数据库表。
///
/// # Example
/// ```
/// let mut conn = MultiDbConnection::open(Path::new("core.db"))?;
/// conn.attach(Path::new("memory.db"), "memory")?;
/// conn.attach(Path::new("skills/im.db"), "skill_im")?;
/// 
/// // 执行跨库查询
/// let rows = conn.query_cross_db(
///     "SELECT * FROM memory.entries JOIN skill_im.messages ON ..."
/// )?;
/// ```
pub struct MultiDbConnection {
    /// 主连接（core.db）
    primary: Connection,
    /// 已挂载的数据库别名 -> 路径
    attached: HashMap<String, String>,
    /// 主数据库路径（用于日志和调试）
    primary_path: String,
}

impl MultiDbConnection {
    /// 创建新的多库连接（打开 core.db）
    ///
    /// # Arguments
    /// * `primary_path` - 主数据库文件路径
    ///
    /// # Returns
    /// * `Result<Self>` - 成功返回 MultiDbConnection 实例
    pub fn open(primary_path: &Path) -> Result<Self> {
        // 确保目录存在
        if let Some(parent) = primary_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(primary_path)
            .map_err(|e| CisError::storage(format!("Failed to open primary db: {}", e)))?;

        let multi_conn = Self {
            primary: conn,
            attached: HashMap::new(),
            primary_path: primary_path.to_string_lossy().to_string(),
        };

        // 配置 WAL 模式
        multi_conn.configure_wal()?;

        tracing::info!(
            "MultiDbConnection opened: primary={}",
            multi_conn.primary_path
        );

        Ok(multi_conn)
    }

    /// 配置 WAL 模式（随时关机安全）
    fn configure_wal(&self) -> Result<()> {
        self.primary
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA wal_autocheckpoint = 1000;
                 PRAGMA journal_size_limit = 100000000;
                 PRAGMA temp_store = memory;",
            )
            .map_err(|e| CisError::storage(format!("Failed to configure WAL: {}", e)))?;
        Ok(())
    }

    /// ATTACH 一个数据库
    ///
    /// 将指定路径的数据库挂载到当前连接，使用给定的别名访问。
    /// 别名可用于 SQL 查询中的 `alias.table` 语法。
    ///
    /// # Arguments
    /// * `db_path` - 要挂载的数据库文件路径
    /// * `alias` - 数据库别名（用于 SQL 中引用）
    ///
    /// # Example
    /// ```
    /// conn.attach(Path::new("memory.db"), "memory")?;
    /// conn.attach(Path::new("skills/im.db"), "skill_im")?;
    /// ```
    pub fn attach(&mut self, db_path: &Path, alias: &str) -> Result<()> {
        // 验证别名合法性（SQLite 标识符规则）
        if !Self::is_valid_alias(alias) {
            return Err(CisError::invalid_input(format!(
                "Invalid database alias: {}",
                alias
            )));
        }

        // 检查别名是否已存在
        if self.attached.contains_key(alias) {
            return Err(CisError::already_exists(format!(
                "Database alias '{}' already attached",
                alias
            )));
        }

        // 转换路径为绝对路径
        let abs_path = db_path
            .canonicalize()
            .unwrap_or_else(|_| db_path.to_path_buf());
        let path_str = abs_path.to_string_lossy();

        // 执行 ATTACH DATABASE
        let sql = format!("ATTACH DATABASE '{}' AS {}", path_str, alias);
        self.primary
            .execute(&sql, [])
            .map_err(|e| CisError::storage(format!("Failed to attach database '{}': {}", alias, e)))?;

        // 为挂载的数据库配置 WAL 模式
        let wal_sql = format!("PRAGMA {}.journal_mode = WAL", alias);
        let _ = self.primary.execute(&wal_sql, []);

        // 记录挂载信息
        self.attached.insert(alias.to_string(), path_str.to_string());

        tracing::info!("Database attached: alias={}, path={}", alias, path_str);

        Ok(())
    }

    /// DETACH 一个数据库
    ///
    /// 卸载指定别名的数据库。
    ///
    /// # Arguments
    /// * `alias` - 要卸载的数据库别名
    pub fn detach(&mut self, alias: &str) -> Result<()> {
        // 检查别名是否存在
        if !self.attached.contains_key(alias) {
            return Err(CisError::not_found(format!(
                "Database alias '{}' not attached",
                alias
            )));
        }

        // 执行 DETACH DATABASE
        let sql = format!("DETACH DATABASE {}", alias);
        self.primary
            .execute(&sql, [])
            .map_err(|e| CisError::storage(format!("Failed to detach database '{}': {}", alias, e)))?;

        // 移除记录
        let path = self.attached.remove(alias);

        tracing::info!(
            "Database detached: alias={}, path={:?}",
            alias,
            path
        );

        Ok(())
    }

    /// 获取主连接的可变引用
    ///
    /// 可用于执行 SQL 操作，包括跨库查询。
    pub fn primary(&self) -> &Connection {
        &self.primary
    }

    /// 获取主连接的可变引用
    pub fn primary_mut(&mut self) -> &mut Connection {
        &mut self.primary
    }

    /// 执行跨库查询
    ///
    /// 执行 SQL 查询，可以使用 `alias.table` 语法访问已挂载的数据库。
    /// 返回查询结果的所有行。
    ///
    /// # Arguments
    /// * `sql` - SQL 查询语句
    ///
    /// # Returns
    /// * `Result<Vec<CrossDbRow>>` - 查询结果行列表
    ///
    /// # Example
    /// ```
    /// let rows = conn.query_cross_db(
    ///     "SELECT * FROM memory.entries JOIN skill_im.messages ON ..."
    /// )?;
    /// for row in rows {
    ///     println!("{:?}", row.get::<String>("name"));
    /// }
    /// ```
    pub fn query_cross_db(&self, sql: &str) -> Result<Vec<CrossDbRow>> {
        let mut stmt = self
            .primary
            .prepare(sql)
            .map_err(CisError::Database)?;

        let rows = stmt
            .query_map([], |row| Ok(CrossDbRow::from_rusqlite(row)))
            .map_err(CisError::Database)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(CisError::Database)?);
        }

        Ok(result)
    }

    /// 执行参数化跨库查询
    ///
    /// 与 `query_cross_db` 类似，但支持参数绑定。
    pub fn query_cross_db_with_params(
        &self,
        sql: &str,
        params: &[&dyn ToSql],
    ) -> Result<Vec<CrossDbRow>> {
        let mut stmt = self
            .primary
            .prepare(sql)
            .map_err(CisError::Database)?;

        let rows = stmt
            .query_map(params, |row| Ok(CrossDbRow::from_rusqlite(row)))
            .map_err(CisError::Database)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(CisError::Database)?);
        }

        Ok(result)
    }

    /// 执行跨库 SQL 语句（INSERT, UPDATE, DELETE 等）
    ///
    /// # Arguments
    /// * `sql` - SQL 语句
    ///
    /// # Returns
    /// * `Result<usize>` - 受影响的行数
    pub fn execute_cross_db(&self, sql: &str) -> Result<usize> {
        self.primary
            .execute(sql, [])
            .map_err(CisError::Database)
    }

    /// 执行参数化跨库 SQL 语句
    pub fn execute_cross_db_with_params(
        &self,
        sql: &str,
        params: &[&dyn ToSql],
    ) -> Result<usize> {
        self.primary
            .execute(sql, params)
            .map_err(CisError::Database)
    }

    /// 列出已挂载的数据库
    ///
    /// # Returns
    /// * `Vec<String>` - 已挂载数据库的别名列表
    pub fn list_attached(&self) -> Vec<String> {
        self.attached.keys().cloned().collect()
    }

    /// 检查数据库是否已挂载
    pub fn is_attached(&self, alias: &str) -> bool {
        self.attached.contains_key(alias)
    }

    /// 获取已挂载数据库的路径
    pub fn get_attached_path(&self, alias: &str) -> Option<&str> {
        self.attached.get(alias).map(|s| s.as_str())
    }

    /// 获取已挂载数据库的数量
    pub fn attached_count(&self) -> usize {
        self.attached.len()
    }

    /// 优雅关闭所有连接
    ///
    /// 执行流程：
    /// 1. 对所有挂载的库执行 checkpoint
    /// 2. DETACH 所有数据库
    /// 3. 关闭主连接
    pub fn close(self) -> Result<()> {
        tracing::info!("Closing MultiDbConnection: primary={}", self.primary_path);

        // 1. 对所有挂载的数据库执行 checkpoint
        for (alias, path) in &self.attached {
            let checkpoint_sql = format!("PRAGMA {}.wal_checkpoint(TRUNCATE)", alias);
            if let Err(e) = self.primary.execute(&checkpoint_sql, []) {
                tracing::warn!("Failed to checkpoint '{}': {}", alias, e);
            } else {
                tracing::debug!("Checkpointed: alias={}, path={}", alias, path);
            }
        }

        // 2. DETACH 所有数据库（逆序以避免依赖问题）
        let aliases: Vec<String> = self.attached.keys().cloned().collect();
        for alias in aliases.iter().rev() {
            let sql = format!("DETACH DATABASE {}", alias);
            if let Err(e) = self.primary.execute(&sql, []) {
                tracing::warn!("Failed to detach '{}': {}", alias, e);
            } else {
                tracing::debug!("Detached: {}", alias);
            }
        }

        // 3. 对主数据库执行 checkpoint
        if let Err(e) = self.primary.execute("PRAGMA wal_checkpoint(TRUNCATE)", []) {
            tracing::warn!("Failed to checkpoint primary: {}", e);
        }

        // 4. 关闭主连接
        self.primary
            .close()
            .map_err(|(_, e)| CisError::storage(format!("Failed to close primary connection: {}", e)))?;

        tracing::info!("MultiDbConnection closed successfully");

        Ok(())
    }

    /// 开始事务
    pub fn transaction(&mut self) -> Result<rusqlite::Transaction<'_>> {
        self.primary
            .transaction()
            .map_err(CisError::Database)
    }

    /// 验证别名是否合法
    ///
    /// SQLite 标识符规则：
    /// - 必须以字母或下划线开头
    /// - 只能包含字母、数字、下划线
    /// - 不能是保留关键字
    fn is_valid_alias(alias: &str) -> bool {
        if alias.is_empty() {
            return false;
        }

        // 检查第一个字符
        let first = alias.chars().next().unwrap();
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }

        // 检查其余字符
        if !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }

        // 检查保留关键字（简化检查，实际使用时可根据需要扩展）
        let reserved = ["main", "temp", "sqlite"];
        if reserved.contains(&alias.to_lowercase().as_str()) {
            return false;
        }

        true
    }

    /// 获取主数据库路径
    pub fn primary_path(&self) -> &str {
        &self.primary_path
    }
}

/// 跨库查询结果行
///
/// 简化的行数据封装，支持按索引和列名访问。
#[derive(Debug, Clone)]
pub struct CrossDbRow {
    /// 列名 -> (索引, 值)
    columns: HashMap<String, (usize, SqlValue)>,
    /// 按索引存储的值
    values: Vec<SqlValue>,
}

impl CrossDbRow {
    /// 从 rusqlite::Row 创建
    fn from_rusqlite(row: &RusqliteRow) -> Self {
        let column_count = row.as_ref().column_count();
        let mut columns = HashMap::new();
        let mut values = Vec::with_capacity(column_count);

        for i in 0..column_count {
            let name = row.as_ref().column_name(i).unwrap_or("").to_string();
            let value = SqlValue::from_rusqlite(row, i);
            columns.insert(name, (i, value.clone()));
            values.push(value);
        }

        Self { columns, values }
    }

    /// 通过列名获取值
    pub fn get<T: FromSqlValue>(&self, column: &str) -> Option<T> {
        self.columns.get(column).and_then(|(_, v)| v.convert())
    }

    /// 通过索引获取值
    pub fn get_by_index<T: FromSqlValue>(&self, index: usize) -> Option<T> {
        self.values.get(index).and_then(|v| v.convert())
    }

    /// 获取列名列表
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.keys().map(|s| s.as_str()).collect()
    }

    /// 获取列数
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// 是否为空行
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// SQL 值类型
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    /// 从 rusqlite::Row 读取值
    fn from_rusqlite(row: &RusqliteRow, index: usize) -> Self {
        // 尝试按不同类型读取
        if let Ok(v) = row.get::<_, Option<i64>>(index) {
            if let Some(v) = v {
                return Self::Integer(v);
            } else {
                return Self::Null;
            }
        }

        if let Ok(Some(v)) = row.get::<_, Option<f64>>(index) {
            return Self::Real(v);
        }

        if let Ok(Some(v)) = row.get::<_, Option<String>>(index) {
            return Self::Text(v);
        }

        if let Ok(Some(v)) = row.get::<_, Option<Vec<u8>>>(index) {
            return Self::Blob(v);
        }

        Self::Null
    }

    /// 转换为指定类型
    fn convert<T: FromSqlValue>(&self) -> Option<T> {
        T::from_sql_value(self)
    }
}

/// 从 SqlValue 转换 trait
pub trait FromSqlValue: Sized {
    fn from_sql_value(value: &SqlValue) -> Option<Self>;
}

impl FromSqlValue for i64 {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Integer(v) => Some(*v),
            SqlValue::Real(v) => Some(*v as i64),
            SqlValue::Text(v) => v.parse().ok(),
            _ => None,
        }
    }
}

impl FromSqlValue for i32 {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Integer(v) => Some(*v as i32),
            SqlValue::Real(v) => Some(*v as i32),
            SqlValue::Text(v) => v.parse().ok(),
            _ => None,
        }
    }
}

impl FromSqlValue for f64 {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Real(v) => Some(*v),
            SqlValue::Integer(v) => Some(*v as f64),
            SqlValue::Text(v) => v.parse().ok(),
            _ => None,
        }
    }
}

impl FromSqlValue for f32 {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Real(v) => Some(*v as f32),
            SqlValue::Integer(v) => Some(*v as f32),
            SqlValue::Text(v) => v.parse().ok(),
            _ => None,
        }
    }
}

impl FromSqlValue for String {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Text(v) => Some(v.clone()),
            SqlValue::Integer(v) => Some(v.to_string()),
            SqlValue::Real(v) => Some(v.to_string()),
            SqlValue::Blob(v) => String::from_utf8(v.clone()).ok(),
            SqlValue::Null => None,
        }
    }
}

impl FromSqlValue for Vec<u8> {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Blob(v) => Some(v.clone()),
            SqlValue::Text(v) => Some(v.as_bytes().to_vec()),
            _ => None,
        }
    }
}

impl FromSqlValue for bool {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Integer(v) => Some(*v != 0),
            SqlValue::Real(v) => Some(*v != 0.0),
            SqlValue::Text(v) => {
                let lower = v.to_lowercase();
                Some(lower == "true" || lower == "1" || lower == "yes")
            }
            _ => None,
        }
    }
}

impl<T: FromSqlValue> FromSqlValue for Option<T> {
    fn from_sql_value(value: &SqlValue) -> Option<Self> {
        match value {
            SqlValue::Null => Some(None),
            _ => T::from_sql_value(value).map(Some),
        }
    }
}

/// 线程安全的共享多库连接
pub type SharedMultiDbConnection = Arc<Mutex<MultiDbConnection>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn setup_test_db(path: &Path, init_sql: &str) {
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(init_sql).unwrap();
    }

    fn cleanup_test_files(primary: &Path, attached: &[&Path]) {
        let _ = std::fs::remove_file(primary);
        for path in attached {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn test_multidb_attach_detach() {
        let temp_dir = std::env::temp_dir().join("cis_test_connection");
        let _ = std::fs::create_dir_all(&temp_dir);

        let primary_path = temp_dir.join("primary.db");
        let memory_path = temp_dir.join("memory.db");

        // 设置测试数据库
        setup_test_db(&primary_path, "CREATE TABLE main_table (id INTEGER PRIMARY KEY);");
        setup_test_db(
            &memory_path,
            "CREATE TABLE memory_entries (id INTEGER PRIMARY KEY, content TEXT);",
        );

        // 测试连接和挂载
        {
            let mut conn = MultiDbConnection::open(&primary_path).unwrap();
            assert!(!conn.is_attached("memory"));

            // ATTACH
            conn.attach(&memory_path, "memory").unwrap();
            assert!(conn.is_attached("memory"));
            assert_eq!(conn.attached_count(), 1);

            // 重复 ATTACH 应该失败
            assert!(conn.attach(&memory_path, "memory").is_err());

            // 列出已挂载
            let attached = conn.list_attached();
            assert_eq!(attached.len(), 1);
            assert!(attached.contains(&"memory".to_string()));

            // 获取路径
            assert!(conn.get_attached_path("memory").is_some());
            assert!(conn.get_attached_path("nonexistent").is_none());

            // DETACH
            conn.detach("memory").unwrap();
            assert!(!conn.is_attached("memory"));
            assert_eq!(conn.attached_count(), 0);

            // DETACH 不存在的应该失败
            assert!(conn.detach("nonexistent").is_err());

            // 关闭连接
            conn.close().unwrap();
        }

        cleanup_test_files(&primary_path, &[&memory_path]);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cross_db_query() {
        let temp_dir = std::env::temp_dir().join("cis_test_crossdb");
        let _ = std::fs::create_dir_all(&temp_dir);

        let primary_path = temp_dir.join("core.db");
        let skill_path = temp_dir.join("skill.db");

        // 设置测试数据库
        setup_test_db(
            &primary_path,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
        );
        setup_test_db(
            &skill_path,
            "CREATE TABLE messages (id INTEGER PRIMARY KEY, user_id INTEGER, content TEXT);",
        );

        {
            let mut conn = MultiDbConnection::open(&primary_path).unwrap();
            conn.attach(&skill_path, "skill").unwrap();

            // 插入测试数据
            conn.execute_cross_db("INSERT INTO users (name) VALUES ('Alice');")
                .unwrap();
            conn.execute_cross_db("INSERT INTO skill.messages (user_id, content) VALUES (1, 'Hello');")
                .unwrap();

            // 跨库查询
            let rows = conn
                .query_cross_db("SELECT * FROM users JOIN skill.messages ON users.id = skill.messages.user_id")
                .unwrap();

            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].get::<String>("name"), Some("Alice".to_string()));
            assert_eq!(rows[0].get::<String>("content"), Some("Hello".to_string()));

            conn.close().unwrap();
        }

        cleanup_test_files(&primary_path, &[&skill_path]);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_invalid_alias() {
        let temp_dir = std::env::temp_dir().join("cis_test_alias");
        let _ = std::fs::create_dir_all(&temp_dir);

        let primary_path = temp_dir.join("primary.db");
        setup_test_db(&primary_path, "");

        {
            let mut conn = MultiDbConnection::open(&primary_path).unwrap();

            // 无效别名测试
            let test_path = temp_dir.join("test.db");
            setup_test_db(&test_path, "");

            assert!(conn.attach(&test_path, "123invalid").is_err()); // 数字开头
            assert!(conn.attach(&test_path, "").is_err()); // 空字符串
            assert!(conn.attach(&test_path, "main").is_err()); // 保留关键字
            assert!(conn.attach(&test_path, "invalid-alias").is_err()); // 包含连字符
            assert!(conn.attach(&test_path, "valid_alias").is_ok()); // 有效

            conn.close().unwrap();
        }

        cleanup_test_files(&primary_path, &[]);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
