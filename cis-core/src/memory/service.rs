//! # MemoryService
//!
//! 提供私域(Private)和公域(Public)记忆分离存储。
//!
//! ## 特性
//!
//! - 私域记忆: 本地加密存储，永不同步
//! - 公域记忆: 明文存储，可P2P同步
//! - 向量索引: 统一语义检索
//! - 访问控制: 基于权限的读写
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::memory::MemoryService;
//! use cis_core::types::{MemoryDomain, MemoryCategory};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let service = MemoryService::open_default("node-1")?;
//!
//! // 存储私域记忆
//! service.set("private-key", b"secret", MemoryDomain::Private, MemoryCategory::Context).await?;
//!
//! // 存储公域记忆
//! service.set("public-key", b"shared", MemoryDomain::Public, MemoryCategory::Result).await?;
//!
//! // 读取记忆
//! if let Some(item) = service.get("private-key").await? {
//!     println!("Found: {:?}", item.value);
//! }
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::memory::{MemoryEncryption, MemoryEntryExt};
use crate::storage::memory_db::{MemoryDb, MemoryEntry};
use crate::types::{MemoryCategory, MemoryDomain};
use crate::vector::VectorStorage;

/// 记忆搜索结果
#[derive(Debug, Clone)]
pub struct MemorySearchResult {
    /// 记忆键
    pub key: String,
    /// 记忆值
    pub value: Vec<u8>,
    /// 域（私域/公域）
    pub domain: MemoryDomain,
    /// 分类
    pub category: MemoryCategory,
    /// 相似度分数 (0.0 - 1.0)
    pub similarity: f32,
    /// 所有者节点ID
    pub owner: String,
}

/// 记忆服务 - 私域/公域记忆分离管理
///
/// 提供统一的记忆管理接口，支持：
/// - 私域记忆：本地加密，永不同步
/// - 公域记忆：可联邦同步
/// - 向量索引：语义检索
///
/// ## 线程安全
///
/// `MemoryService` 是线程安全的，可以在多个线程间共享。
///
/// ## 示例
///
/// ```rust,no_run
/// use cis_core::memory::{MemoryService, SearchOptions};
/// use cis_core::types::{MemoryDomain, MemoryCategory};
///
/// # async fn example() -> anyhow::Result<()> {
/// let service = MemoryService::open_default("node-1")?;
///
/// // 存储并建立向量索引
/// service.set_with_embedding("key", b"value", MemoryDomain::Public, MemoryCategory::Context).await?;
///
/// // 语义搜索
/// let results = service.semantic_search("query", 10, 0.7).await?;
/// # Ok(())
/// # }
/// ```
pub struct MemoryService {
    /// 记忆数据库（私域/公域分离存储）
    memory_db: Arc<tokio::sync::Mutex<MemoryDb>>,
    /// 向量存储（用于语义搜索）
    vector_storage: Arc<VectorStorage>,
    /// 加密器（用于私域记忆）
    encryption: Option<MemoryEncryption>,
    /// 节点ID
    node_id: String,
    /// 命名空间隔离
    namespace: Option<String>,
}

/// 记忆条目（完整版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub key: String,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u64,
    pub encrypted: bool,
    pub owner: String, // 节点ID
}

impl From<MemoryEntry> for MemoryItem {
    fn from(entry: MemoryEntry) -> Self {
        Self {
            key: entry.key,
            value: entry.value,
            domain: entry.domain,
            category: entry.category,
            created_at: DateTime::from_timestamp(entry.created_at, 0).unwrap_or_else(Utc::now),
            updated_at: DateTime::from_timestamp(entry.updated_at, 0).unwrap_or_else(Utc::now),
            version: 1,
            encrypted: matches!(entry.domain, MemoryDomain::Private),
            owner: String::new(),
        }
    }
}

impl From<MemoryEntryExt> for MemoryItem {
    fn from(entry: MemoryEntryExt) -> Self {
        Self {
            key: entry.key,
            value: entry.value,
            domain: entry.domain,
            category: entry.category,
            created_at: DateTime::from_timestamp(entry.created_at, 0).unwrap_or_else(Utc::now),
            updated_at: DateTime::from_timestamp(entry.updated_at, 0).unwrap_or_else(Utc::now),
            version: entry.version as u64,
            encrypted: entry.encrypted,
            owner: String::new(),
        }
    }
}

/// 记忆搜索选项（service 模块专用）
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub domain: Option<MemoryDomain>,
    pub category: Option<MemoryCategory>,
    pub limit: usize,
    pub threshold: f32,
}

impl SearchOptions {
    /// 创建默认搜索选项
    pub fn new() -> Self {
        Self {
            domain: None,
            category: None,
            limit: 10,
            threshold: 0.6,
        }
    }
    
    /// 设置搜索域
    pub fn with_domain(mut self, domain: MemoryDomain) -> Self {
        self.domain = Some(domain);
        self
    }
    
    /// 设置分类过滤
    pub fn with_category(mut self, category: MemoryCategory) -> Self {
        self.category = Some(category);
        self
    }
    
    /// 设置返回数量限制
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
    
    /// 设置相似度阈值
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }
}

/// 同步标记
#[derive(Debug, Clone)]
pub struct SyncMarker {
    pub key: String,
    pub domain: MemoryDomain,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub sync_peers: Vec<String>,
}

impl MemoryService {
    /// 创建新的记忆服务
    ///
    /// # 参数
    /// - `memory_db`: 记忆数据库
    /// - `vector_storage`: 向量存储（用于语义搜索）
    /// - `node_id`: 节点标识符
    ///
    /// # 返回
    /// - `Result<Self>`: 成功返回 MemoryService，失败返回错误
    pub fn new(
        memory_db: Arc<Mutex<MemoryDb>>,
        vector_storage: Arc<VectorStorage>,
        node_id: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            memory_db: memory_db as Arc<tokio::sync::Mutex<MemoryDb>>,
            vector_storage,
            encryption: None,
            node_id: node_id.into(),
            namespace: None,
        })
    }

    /// 从默认路径创建记忆服务
    ///
    /// 使用默认路径打开记忆数据库和向量存储。
    ///
    /// # 参数
    /// - `node_id`: 节点标识符
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::MemoryService;
    ///
    /// let service = MemoryService::open_default("node-1").unwrap();
    /// ```
    pub fn open_default(node_id: impl Into<String>) -> Result<Self> {
        let memory_db = MemoryDb::open_default()?;
        let vector_storage = VectorStorage::open_default()?;
        
        Ok(Self {
            memory_db: Arc::new(tokio::sync::Mutex::new(memory_db)),
            vector_storage: Arc::new(vector_storage),
            encryption: None,
            node_id: node_id.into(),
            namespace: None,
        })
    }

    /// 设置加密密钥
    pub fn with_encryption(mut self, encryption: MemoryEncryption) -> Self {
        self.encryption = Some(encryption);
        self
    }

    /// 设置命名空间
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    fn full_key(&self, key: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}", ns, key),
            None => key.to_string(),
        }
    }

    // ==================== 核心操作 ====================

    /// 存储记忆
    ///
    /// 将记忆存储到数据库，并根据域进行加密或标记。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `domain`: 私域或公域
    /// - `category`: 分类
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::MemoryService;
    /// use cis_core::types::{MemoryDomain, MemoryCategory};
    ///
    /// # async fn example(service: &MemoryService) -> anyhow::Result<()> {
    /// // 存储私域记忆（加密）
    /// service.set("api-key", b"secret123", MemoryDomain::Private, MemoryCategory::Context).await?;
    ///
    /// // 存储公域记忆（明文，可同步）
    /// service.set("config", b"{\"theme\":\"dark\"}", MemoryDomain::Public, MemoryCategory::Context).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.full_key(key);
        
        match domain {
            MemoryDomain::Private => self.set_private(&full_key, value, category).await,
            MemoryDomain::Public => self.set_public(&full_key, value, category).await,
        }
    }

    /// 读取记忆
    ///
    /// 根据键读取记忆值。如果是私域加密记忆，会自动解密。
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<Option<MemoryItem>>`: 成功返回记忆项或 None，失败返回错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::MemoryService;
    ///
    /// # async fn example(service: &MemoryService) -> anyhow::Result<()> {
    /// if let Some(item) = service.get("my-key").await? {
    ///     println!("Value: {:?}", item.value);
    ///     println!("Domain: {:?}", item.domain);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock().await;

        match db.get(&full_key)? {
            Some(entry) => {
                let mut item = MemoryItem::from(entry);
                item.owner = self.node_id.clone();
                
                // 如果是加密的私域记忆，解密
                if item.encrypted {
                    if let Some(ref enc) = self.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }
                
                // 更新向量索引（异步）
                self.spawn_index_update(key, &item.value, &item.category);
                
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// 删除记忆
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock().await;

        let deleted = db.delete(&full_key)?;

        if deleted {
            // 从向量索引中移除
            let _ = self.vector_storage.delete_memory_index(&full_key);
        }

        Ok(deleted)
    }

    /// 语义搜索记忆
    ///
    /// 使用向量相似度搜索相关记忆，同时搜索私域和公域记忆，
    /// 根据选项进行过滤。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `options`: 搜索选项（域、分类、限制等）
    ///
    /// # 返回
    /// - `Result<Vec<MemoryItem>>`: 搜索结果列表
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::{MemoryService, SearchOptions};
    /// use cis_core::types::{MemoryDomain, MemoryCategory};
    ///
    /// # async fn example(service: &MemoryService) -> anyhow::Result<()> {
    /// let options = SearchOptions::new()
    ///     .with_domain(MemoryDomain::Public)
    ///     .with_category(MemoryCategory::Context)
    ///     .with_limit(10)
    ///     .with_threshold(0.7);
    ///
    /// let results = service.search("用户偏好", options).await?;
    /// for item in results {
    ///     println!("{}: {:?}", item.key, item.value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search(
        &self,
        query: &str,
        options: SearchOptions,
    ) -> Result<Vec<MemoryItem>> {
        let limit = options.limit;
        let threshold = options.threshold;

        // 1. 向量搜索获取候选键
        let results = self.vector_storage.search_memory(
            query,
            limit * 2, // 获取更多候选以便过滤
            Some(threshold),
        ).await?;

        let mut items = Vec::new();
        let db = self.memory_db.lock().await;

        for result in results {
            // 2. 从数据库获取完整条目
            if let Some(entry) = db.get(&result.key)? {
                let mut item = MemoryItem::from(entry);
                item.owner = self.node_id.clone();

                // 3. 应用过滤条件
                if let Some(domain) = options.domain {
                    if item.domain != domain {
                        continue;
                    }
                }

                if let Some(category) = options.category {
                    if item.category != category {
                        continue;
                    }
                }

                // 4. 解密私域记忆
                if item.encrypted {
                    if let Some(ref enc) = self.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }

                items.push(item);
            }
        }

        // 限制返回数量
        items.truncate(limit);

        Ok(items)
    }

    /// 存储记忆并建立向量索引
    ///
    /// 存储记忆的同时，立即建立向量索引以便语义搜索。
    /// 这是一个同步操作，会等待索引完成。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `domain`: 私域或公域
    /// - `category`: 分类
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::MemoryService;
    /// use cis_core::types::{MemoryDomain, MemoryCategory};
    ///
    /// # async fn example(service: &MemoryService) -> anyhow::Result<()> {
    /// service.set_with_embedding(
    ///     "user-prefs",
    ///     b"{\"theme\":\"dark\",\"lang\":\"zh\"}",
    ///     MemoryDomain::Public,
    ///     MemoryCategory::Context
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_with_embedding(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.full_key(key);
        let category_str = format!("{:?}", category);

        // 1. 存储到数据库
        match domain {
            MemoryDomain::Private => self.set_private(&full_key, value, category).await?,
            MemoryDomain::Public => self.set_public(&full_key, value, category).await?,
        }

        // 2. 同步建立向量索引（等待完成）
        let text = String::from_utf8_lossy(value);
        self.vector_storage
            .index_memory(&full_key, text.as_bytes(), Some(&category_str))
            .await?;

        Ok(())
    }

    /// 语义搜索记忆（返回包含相似度的详细结果）
    ///
    /// 执行语义搜索并返回包含相似度分数的详细结果。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `limit`: 返回结果数量限制
    /// - `threshold`: 相似度阈值 (0.0 - 1.0)
    ///
    /// # 返回
    /// - `Result<Vec<MemorySearchResult>>`: 搜索结果列表，按相似度降序排列
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::memory::MemoryService;
    ///
    /// # async fn example(service: &MemoryService) -> anyhow::Result<()> {
    /// let results = service.semantic_search("暗黑模式配置", 5, 0.7).await?;
    /// for result in results {
    ///     println!("{}: {:.2}% similar", result.key, result.similarity * 100.0);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<MemorySearchResult>> {
        // 1. 向量搜索
        let results = self.vector_storage.search_memory(query, limit * 2, Some(threshold)).await?;

        let mut search_results = Vec::new();
        let db = self.memory_db.lock().await;

        for result in results {
            // 2. 从数据库获取完整条目
            if let Some(entry) = db.get(&result.key)? {
                let mut item = MemoryItem::from(entry);
                item.owner = self.node_id.clone();

                // 3. 解密私域记忆
                let value = if item.encrypted {
                    if let Some(ref enc) = self.encryption {
                        enc.decrypt(&item.value)?
                    } else {
                        item.value
                    }
                } else {
                    item.value
                };

                search_results.push(MemorySearchResult {
                    key: item.key.clone(),
                    value,
                    domain: item.domain,
                    category: item.category,
                    similarity: result.similarity,
                    owner: item.owner,
                });
            }
        }

        // 按相似度降序排序并限制数量
        search_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        search_results.truncate(limit);

        Ok(search_results)
    }

    /// 列出所有记忆键
    pub async fn list_keys(&self, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        let db = self.memory_db.lock().await;
        
        let prefix = match &self.namespace {
            Some(ns) => format!("{}/", ns),
            None => String::new(),
        };
        
        db.list_keys(&prefix, domain)
    }

    // ==================== 私域记忆操作 ====================

    async fn set_private(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.full_key(key);
        
        // 1. 加密数据
        let encrypted = if let Some(ref enc) = self.encryption {
            enc.encrypt(value)?
        } else {
            // 无密钥时存储明文，但标记为私域
            value.to_vec()
        };

        // 2. 存储到 memory_db
        let db = self.memory_db.lock().await;
        db.set_private(&full_key, &encrypted, category)?;

        // 3. 更新向量索引（使用原始值）
        self.spawn_index_update(&full_key, value, &category);

        Ok(())
    }

    #[allow(dead_code)]
    async fn get_private(&self, key: &str) -> Result<Option<MemoryItem>> {
        let db = self.memory_db.lock().await;

        if let Some(entry) = db.get(key)? {
            if entry.domain != MemoryDomain::Private {
                return Ok(None);
            }

            let mut item = MemoryItem::from(entry);
            item.owner = self.node_id.clone();

            // 解密数据
            if item.encrypted {
                if let Some(ref enc) = self.encryption {
                    item.value = enc.decrypt(&item.value)?;
                }
            }

            return Ok(Some(item));
        }

        Ok(None)
    }

    #[allow(dead_code)]
    async fn delete_private(&self, key: &str) -> Result<bool> {
        let db = self.memory_db.lock().await;

        // 检查是否是私域记忆
        if let Some(entry) = db.get(key)? {
            if entry.domain != MemoryDomain::Private {
                return Ok(false);
            }
        }

        db.delete(key)
    }

    // ==================== 公域记忆操作 ====================

    async fn set_public(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.full_key(key);
        
        // 1. 明文存储
        let db = self.memory_db.lock().await;
        db.set_public(&full_key, value, category)?;

        // 2. 更新向量索引
        self.spawn_index_update(&full_key, value, &category);

        // 3. 标记为待同步（P2P）- 已由 MemoryDb 自动处理

        Ok(())
    }

    #[allow(dead_code)]
    async fn get_public(&self, key: &str) -> Result<Option<MemoryItem>> {
        let db = self.memory_db.lock().await;

        if let Some(entry) = db.get(key)? {
            if entry.domain != MemoryDomain::Public {
                return Ok(None);
            }

            let mut item = MemoryItem::from(entry);
            item.owner = self.node_id.clone();
            return Ok(Some(item));
        }

        Ok(None)
    }

    #[allow(dead_code)]
    async fn delete_public(&self, key: &str) -> Result<bool> {
        let db = self.memory_db.lock().await;

        // 检查是否是公域记忆
        if let Some(entry) = db.get(key)? {
            if entry.domain != MemoryDomain::Public {
                return Ok(false);
            }
        }

        db.delete(key)
    }

    // ==================== 向量索引 ====================

    /// 后台更新向量索引
    fn spawn_index_update(&self, key: &str, value: &[u8], category: &MemoryCategory) {
        // 尝试获取当前运行时
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let storage = Arc::clone(&self.vector_storage);
            let key = key.to_string();
            let value = value.to_vec();
            let category_str = format!("{:?}", category);
            
            rt.spawn(async move {
                let text = String::from_utf8_lossy(&value);
                if let Err(e) = storage.index_memory(&key, text.as_bytes(), Some(&category_str)).await {
                    tracing::warn!("Failed to index memory: {}", e);
                }
            });
        }
    }

    /// 手动索引记忆（用于批量操作）
    pub async fn index_memory(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<String> {
        let category_str = format!("{:?}", category);
        self.vector_storage.index_memory(key, value, Some(&category_str)).await
    }

    // ==================== P2P 同步 ====================

    /// 获取待同步的公域记忆
    pub async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>> {
        let db = self.memory_db.lock().await;

        let entries = db.get_pending_sync(limit)?;
        
        let markers = entries
            .into_iter()
            .map(|entry| SyncMarker {
                key: entry.key,
                domain: MemoryDomain::Public,
                last_sync_at: None,
                sync_peers: Vec::new(),
            })
            .collect();

        Ok(markers)
    }

    /// 标记已同步
    pub async fn mark_synced(&self, key: &str) -> Result<()> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock().await;
        db.mark_synced(&full_key)
    }

    /// 导出公域记忆（用于 P2P 同步）
    pub async fn export_public(&self, since: i64) -> Result<Vec<MemoryItem>> {
        let db = self.memory_db.lock().await;

        // 获取所有公域条目
        let all_keys = db.list_keys("", Some(MemoryDomain::Public))?;
        let mut items = Vec::new();

        for key in all_keys {
            if let Some(entry) = db.get(&key)? {
                if entry.updated_at >= since {
                    let mut item = MemoryItem::from(entry);
                    item.owner = self.node_id.clone();
                    items.push(item);
                }
            }
        }

        Ok(items)
    }

    /// 导入公域记忆
    pub async fn import_public(&self, items: Vec<MemoryItem>) -> Result<()> {
        for item in items {
            if item.domain == MemoryDomain::Public {
                self.set_public(&item.key, &item.value, item.category).await?;
            }
        }
        Ok(())
    }

    /// 同步完成回调
    pub async fn on_sync_complete(&self, key: &str, peer_id: &str) -> Result<()> {
        tracing::info!("Memory {} synced to peer {}", key, peer_id);
        
        // 标记为已同步
        self.mark_synced(key).await
    }

    // ==================== 工具方法 ====================

    /// 获取节点ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// 获取命名空间
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// 检查是否有加密
    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }

    /// 关闭服务
    pub async fn close(self) -> Result<()> {
        // 尝试获取锁并关闭数据库
        if let Ok(db) = Arc::try_unwrap(self.memory_db) {
            let db = db.into_inner();
            db.close()?;
        }
        Ok(())
    }
}

// ==================== MemoryServiceTrait Implementation ====================

use crate::memory::{MemoryServiceTrait, MemorySearchItem};

impl MemoryServiceTrait for MemoryService {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        // 尝试获取当前运行时句柄
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // 在已有运行时中，使用 block_in_place 避免嵌套运行时错误
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        match self.get(key).await {
                            Ok(Some(item)) => Some(item.value),
                            _ => None,
                        }
                    })
                })
            }
            Err(_) => {
                // 没有运行时，创建新的运行时
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt.block_on(async {
                        match self.get(key).await {
                            Ok(Some(item)) => Some(item.value),
                            _ => None,
                        }
                    }),
                    Err(_) => None,
                }
            }
        }
    }

    fn set(&self, key: &str, value: &[u8]) -> crate::error::Result<()> {
        // 尝试获取当前运行时句柄
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // 在已有运行时中，使用 block_in_place
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        self.set(key, value, MemoryDomain::Public, MemoryCategory::Context).await
                    })
                })
            }
            Err(_) => {
                // 没有运行时，创建新的运行时
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| crate::error::CisError::Memory(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(async {
                    self.set(key, value, MemoryDomain::Public, MemoryCategory::Context).await
                })
            }
        }
    }

    fn delete(&self, key: &str) -> crate::error::Result<()> {
        // 尝试获取当前运行时句柄
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // 在已有运行时中，使用 block_in_place
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        self.delete(key).await.map(|_| ())
                    })
                })
            }
            Err(_) => {
                // 没有运行时，创建新的运行时
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| crate::error::CisError::Memory(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(async {
                    self.delete(key).await.map(|_| ())
                })
            }
        }
    }

    fn search(&self, query: &str, limit: usize) -> crate::error::Result<Vec<MemorySearchItem>> {
        // 尝试获取当前运行时句柄
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // 在已有运行时中，使用 block_in_place
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        let options = SearchOptions::new()
                            .with_limit(limit)
                            .with_threshold(0.5);
                        
                        let results = self.search(query, options).await?;
                        
                        let items = results.into_iter()
                            .map(|item| MemorySearchItem {
                                key: item.key,
                                value: item.value,
                                domain: item.domain,
                                category: item.category,
                            })
                            .collect();
                        
                        Ok(items)
                    })
                })
            }
            Err(_) => {
                // 没有运行时，创建新的运行时
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| crate::error::CisError::Memory(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(async {
                    let options = SearchOptions::new()
                        .with_limit(limit)
                        .with_threshold(0.5);
                    
                    let results = self.search(query, options).await?;
                    
                    let items = results.into_iter()
                        .map(|item| MemorySearchItem {
                            key: item.key,
                            value: item.value,
                            domain: item.domain,
                            category: item.category,
                        })
                        .collect();
                    
                    Ok(items)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use async_trait::async_trait;

    /// Mock embedding service for tests
    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> crate::error::Result<Vec<f32>> {
            // 简单的确定性模拟：根据文本哈希生成向量
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let hash = text.bytes().fold(0u64, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as u64)
            });
            for i in 0..DEFAULT_EMBEDDING_DIM {
                let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                vec[i] = val;
            }
            // 归一化
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            Ok(vec)
        }

        async fn batch_embed(&self, texts: &[&str]) -> crate::error::Result<Vec<Vec<f32>>> {
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text).await?);
            }
            Ok(results)
        }
    }

    fn setup_test_service() -> (MemoryService, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        
        let db_path = temp_dir.path().join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        // 使用 mock vector storage
        let vector_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();
        
        let service = MemoryService {
            memory_db: Arc::new(tokio::sync::Mutex::new(memory_db)),
            vector_storage: Arc::new(vector_storage),
            encryption: None,
            node_id: "test-node".to_string(),
            namespace: None,
        };
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_memory_service_basic() {
        let (service, _temp) = setup_test_service();

        // 存储私域记忆
        service.set_private("private_key", b"private_value", MemoryCategory::Context).await.unwrap();

        // 存储公域记忆
        service.set_public("public_key", b"public_value", MemoryCategory::Result).await.unwrap();

        // 读取
        let item = service.get("private_key").await.unwrap().unwrap();
        assert_eq!(item.key, "private_key");
        assert_eq!(item.value, b"private_value");
        assert!(matches!(item.domain, MemoryDomain::Private));
        assert_eq!(item.owner, "test-node");

        let item = service.get("public_key").await.unwrap().unwrap();
        assert_eq!(item.key, "public_key");
        assert_eq!(item.value, b"public_value");
        assert!(matches!(item.domain, MemoryDomain::Public));

        service.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_memory_service_delete() {
        let (service, _temp) = setup_test_service();

        service.set_private("to_delete", b"value", MemoryCategory::Context).await.unwrap();
        assert!(service.get("to_delete").await.unwrap().is_some());

        let deleted = service.delete("to_delete").await.unwrap();
        assert!(deleted);
        assert!(service.get("to_delete").await.unwrap().is_none());

        service.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_memory_service_with_encryption() {
        let temp_dir = tempfile::tempdir().unwrap();
        
        let db_path = temp_dir.path().join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        let vector_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();
        
        let encryption = MemoryEncryption::from_node_key(b"test-key");
        
        let service = MemoryService {
            memory_db: Arc::new(tokio::sync::Mutex::new(memory_db)),
            vector_storage: Arc::new(vector_storage),
            encryption: Some(encryption),
            node_id: "test-node".to_string(),
            namespace: None,
        };

        // 存储私域记忆（会被加密）
        service.set_private("encrypted_key", b"secret_value", MemoryCategory::Context).await.unwrap();

        // 读取（自动解密）
        let item = service.get("encrypted_key").await.unwrap().unwrap();
        assert_eq!(item.value, b"secret_value");
        assert!(item.encrypted);

        service.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_memory_service_namespace() {
        let temp_dir = tempfile::tempdir().unwrap();
        
        let db_path = temp_dir.path().join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        let vector_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();
        
        let memory_db_arc = Arc::new(tokio::sync::Mutex::new(memory_db));
        let vector_storage_arc = Arc::new(vector_storage);
        
        let service1 = MemoryService {
            memory_db: Arc::clone(&memory_db_arc),
            vector_storage: Arc::clone(&vector_storage_arc),
            encryption: None,
            node_id: "node1".to_string(),
            namespace: Some("ns1".to_string()),
        };
        
        // 需要先存储，再读取
        service1.set_private("key", b"value1", MemoryCategory::Context).await.unwrap();

        let item = service1.get("key").await.unwrap().unwrap();
        assert_eq!(item.value, b"value1");

        // Note: 实际上两个服务共享同一个底层数据库，
        // 所以 namespace 隔离只是在 key 上加前缀
    }
}
