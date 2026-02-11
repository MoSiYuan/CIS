//! # StorageService Trait
//!
//! 存储服务的抽象接口，定义数据持久化的基本操作。
//!
//! ## 设计原则
//!
//! - **一致性**: 所有操作都是原子性的
//! - **容错性**: 自动处理临时错误和重试
//! - **可观察性**: 提供统计和诊断接口
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::traits::{StorageService, StorageQuery};
//! use std::sync::Arc;
//!
//! # async fn example(storage: Arc<dyn StorageService>) -> anyhow::Result<()> {
//! // 存储数据
//! storage.put("user:123", b"John Doe").await?;
//!
//! // 读取数据
//! if let Some(data) = storage.get("user:123").await? {
//!     println!("User: {}", String::from_utf8_lossy(&data));
//! }
//!
//! // 查询数据
//! let results = storage.query(
//!     StorageQuery::new().with_prefix("user:")
//! ).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// 查询选项
#[derive(Debug, Clone)]
pub struct QueryOptions {
    /// 是否包含元数据
    pub include_metadata: bool,
    /// 排序字段
    pub sort_by: Option<String>,
    /// 是否降序
    pub descending: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            include_metadata: false,
            sort_by: None,
            descending: false,
        }
    }
}

impl QueryOptions {
    /// 创建默认查询选项
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置包含元数据
    pub fn with_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// 设置排序
    pub fn with_sort(mut self, field: impl Into<String>, descending: bool) -> Self {
        self.sort_by = Some(field.into());
        self.descending = descending;
        self
    }
}

/// 存储查询条件
#[derive(Debug, Clone)]
pub struct StorageQuery {
    /// 前缀匹配
    pub prefix: Option<String>,
    /// 键匹配模式
    pub key_pattern: Option<String>,
    /// 最大返回数量
    pub limit: Option<usize>,
    /// 偏移量
    pub offset: Option<usize>,
    /// 查询选项
    pub options: QueryOptions,
}

impl StorageQuery {
    /// 创建新的查询
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::traits::StorageQuery;
    ///
    /// let query = StorageQuery::new()
    ///     .with_prefix("user:")
    ///     .with_limit(10);
    /// ```
    pub fn new() -> Self {
        Self {
            prefix: None,
            key_pattern: None,
            limit: None,
            offset: None,
            options: QueryOptions::default(),
        }
    }

    /// 设置前缀
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// 设置键模式
    pub fn with_key_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.key_pattern = Some(pattern.into());
        self
    }

    /// 设置限制
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 设置偏移量
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// 设置查询选项
    pub fn with_options(mut self, options: QueryOptions) -> Self {
        self.options = options;
        self
    }
}

impl Default for StorageQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// 存储记录
#[derive(Debug, Clone)]
pub struct StorageRecord {
    /// 键
    pub key: String,
    /// 值
    pub value: Vec<u8>,
    /// 版本号（用于乐观锁）
    pub version: u64,
    /// 创建时间戳（Unix 秒）
    pub created_at: u64,
    /// 更新时间戳（Unix 秒）
    pub updated_at: u64,
    /// 过期时间（可选，Unix 秒）
    pub expires_at: Option<u64>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 存储统计信息
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// 总键数
    pub total_keys: u64,
    /// 总大小（字节）
    pub total_size: u64,
    /// 索引数量
    pub index_count: u32,
    /// 最后更新时间
    pub last_modified: Option<u64>,
    /// 压缩率（0-100）
    pub compression_ratio: Option<f64>,
}

/// 存储服务抽象接口
///
/// 定义数据持久化的基本操作，包括键值存储、查询和批量操作。
///
/// ## 实现要求
///
/// - 所有方法必须是线程安全的 (Send + Sync)
/// - 所有异步方法必须返回 Result 类型
/// - 实现应该提供事务支持（如果底层存储支持）
///
/// ## 使用示例
///
/// ```rust,no_run
/// use cis_core::traits::{StorageService, StorageQuery};
/// use std::sync::Arc;
///
/// # async fn example(storage: Arc<dyn StorageService>) -> anyhow::Result<()> {
/// // 基本 CRUD
/// storage.put("key", b"value").await?;
/// let value = storage.get("key").await?;
/// storage.delete("key").await?;
///
/// // 批量操作
/// let items = vec![
///     ("key1".to_string(), b"value1".to_vec()),
///     ("key2".to_string(), b"value2".to_vec()),
/// ];
/// storage.put_batch(&items).await?;
///
/// // 查询
/// let results = storage.query(
///     StorageQuery::new().with_prefix("key")
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait StorageService: Send + Sync {
    /// 获取值
    ///
    /// # Arguments
    /// * `key` - 键
    ///
    /// # Returns
    /// * `Ok(Some(Vec<u8>))` - 键存在，返回值
    /// * `Ok(None)` - 键不存在
    /// * `Err(CisError::Storage(_))` - 存储错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::StorageService;
    ///
    /// # async fn example(storage: &dyn StorageService) -> anyhow::Result<()> {
    /// match storage.get("user:123").await? {
    ///     Some(data) => println!("Found: {}", String::from_utf8_lossy(&data)),
    ///     None => println!("Not found"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// 存储值
    ///
    /// # Arguments
    /// * `key` - 键
    /// * `value` - 值
    ///
    /// # Returns
    /// * `Ok(())` - 存储成功
    /// * `Err(CisError::Storage(_))` - 存储错误
    /// * `Err(CisError::InvalidInput(_))` - 键或值无效
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::StorageService;
    ///
    /// # async fn example(storage: &dyn StorageService) -> anyhow::Result<()> {
    /// storage.put("config:theme", b"dark").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;

    /// 带版本控制的存储（乐观锁）
    ///
    /// # Arguments
    /// * `key` - 键
    /// * `value` - 值
    /// * `expected_version` - 期望的当前版本（用于 CAS）
    ///
    /// # Returns
    /// * `Ok(())` - 存储成功
    /// * `Err(CisError::AlreadyExists(_))` - 版本冲突
    async fn put_if_version(
        &self,
        key: &str,
        value: &[u8],
        expected_version: Option<u64>,
    ) -> Result<()>;

    /// 删除键
    ///
    /// # Arguments
    /// * `key` - 键
    ///
    /// # Returns
    /// * `Ok(())` - 删除成功（即使键不存在）
    /// * `Err(CisError::Storage(_))` - 存储错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::StorageService;
    ///
    /// # async fn example(storage: &dyn StorageService) -> anyhow::Result<()> {
    /// storage.delete("temp:data").await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn delete(&self, key: &str) -> Result<()>;

    /// 查询存储
    ///
    /// # Arguments
    /// * `query` - 查询条件
    ///
    /// # Returns
    /// * `Ok(Vec<StorageRecord>)` - 匹配的记录列表
    /// * `Err(CisError::Storage(_))` - 查询错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::{StorageService, StorageQuery};
    ///
    /// # async fn example(storage: &dyn StorageService) -> anyhow::Result<()> {
    /// let results = storage.query(
    ///     StorageQuery::new()
    ///         .with_prefix("user:")
    ///         .with_limit(10)
    /// ).await?;
    ///
    /// for record in results {
    ///     println!("{}: {:?}", record.key, record.value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>>;

    /// 扫描键前缀
    ///
    /// # Arguments
    /// * `prefix` - 键前缀
    ///
    /// # Returns
    /// * `Ok(Vec<(String, Vec<u8>)>)` - 匹配的键值对
    /// * `Err(CisError::Storage(_))` - 扫描错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::StorageService;
    ///
    /// # async fn example(storage: &dyn StorageService) -> anyhow::Result<()> {
    /// let items = storage.scan("config:").await?;
    /// for (key, value) in items {
    ///     println!("{}: {:?}", key, value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>>;

    /// 检查键是否存在
    ///
    /// # Arguments
    /// * `key` - 键
    ///
    /// # Returns
    /// * `Ok(true)` - 键存在
    /// * `Ok(false)` - 键不存在
    /// * `Err(CisError::Storage(_))` - 检查错误
    async fn exists(&self, key: &str) -> Result<bool>;

    /// 批量获取
    ///
    /// # Arguments
    /// * `keys` - 键列表
    ///
    /// # Returns
    /// * `Ok(Vec<Option<Vec<u8>>>)` - 与输入一一对应的结果
    /// * `Err(CisError::Storage(_))` - 批量获取错误
    async fn get_batch(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>>;

    /// 批量存储
    ///
    /// # Arguments
    /// * `items` - 键值对列表
    ///
    /// # Returns
    /// * `Ok(())` - 全部存储成功
    /// * `Err(CisError::Storage(_))` - 部分或全部存储失败
    async fn put_batch(&self, items: &[(String, Vec<u8>)]) -> Result<()>;

    /// 获取所有键
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - 所有键的列表
    /// * `Err(CisError::Storage(_))` - 获取错误
    async fn keys(&self) -> Result<Vec<String>>;

    /// 清空存储
    ///
    /// # Returns
    /// * `Ok(())` - 清空成功
    /// * `Err(CisError::Storage(_))` - 清空错误
    ///
    /// # Warning
    /// 此操作会删除所有数据，请谨慎使用
    async fn clear(&self) -> Result<()>;

    /// 获取存储统计信息
    ///
    /// # Returns
    /// * `Ok(StorageStats)` - 统计信息
    /// * `Err(CisError::Storage(_))` - 获取失败
    async fn stats(&self) -> Result<StorageStats>;

    /// 原子性事务执行
    ///
    /// 在一个事务中执行多个操作，要么全部成功，要么全部回滚。
    ///
    /// # Arguments
    /// * `operations` - 操作列表（元组：操作类型, 键, 值）
    ///
    /// # Returns
    /// * `Ok(())` - 事务成功
    /// * `Err(CisError::Storage(_))` - 事务失败并已回滚
    async fn transaction(&self, operations: Vec<(String, String, Option<Vec<u8>>)>) -> Result<()>
    where
        String: Send + Sync, // 操作类型
        Vec<u8>: Send + Sync; // 值
}

/// StorageService 的 Arc 包装类型
pub type StorageServiceRef = Arc<dyn StorageService>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_query_builder() {
        let query = StorageQuery::new()
            .with_prefix("user:")
            .with_key_pattern("*admin*")
            .with_limit(10)
            .with_offset(5);

        assert_eq!(query.prefix, Some("user:".to_string()));
        assert_eq!(query.key_pattern, Some("*admin*".to_string()));
        assert_eq!(query.limit, Some(10));
        assert_eq!(query.offset, Some(5));
    }

    #[test]
    fn test_query_options_builder() {
        let opts = QueryOptions::new()
            .with_metadata(true)
            .with_sort("created_at", true);

        assert!(opts.include_metadata);
        assert_eq!(opts.sort_by, Some("created_at".to_string()));
        assert!(opts.descending);
    }

    #[test]
    fn test_storage_record_creation() {
        let record = StorageRecord {
            key: "test".to_string(),
            value: b"data".to_vec(),
            version: 1,
            created_at: 1000,
            updated_at: 2000,
            expires_at: Some(3000),
            metadata: HashMap::new(),
        };

        assert_eq!(record.key, "test");
        assert_eq!(record.version, 1);
        assert_eq!(record.expires_at, Some(3000));
    }

    #[test]
    fn test_storage_stats_default() {
        let stats = StorageStats::default();
        assert_eq!(stats.total_keys, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.index_count, 0);
    }
}
