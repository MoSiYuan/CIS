//! # MemoryService (Refactored)
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
//! ## 架构
//!
//! 重构后的 MemoryService 采用模块化设计：
//! - GET/SET/SEARCH/SYNC 操作委托给专门的 ops 模块
//! - 主服务只负责配置、协调和 API 暴露
//! - 共享状态通过 Arc<MemoryServiceState> 共享
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
use crate::memory::{MemoryEncryption, MemoryEncryptionV2};
use crate::memory::ops::{EncryptionWrapper, GetOperations, MemoryServiceState, SearchOperations, SetOperations, SyncOperations};
use crate::storage::memory_db::MemoryDb;
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

impl From<crate::storage::memory_db::MemoryEntry> for MemoryItem {
    fn from(entry: crate::storage::memory_db::MemoryEntry) -> Self {
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

/// 记忆服务 - 私域/公域记忆分离管理
///
/// 重构后的版本，使用 ops 模块分离职责。
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
    /// 共享服务状态
    state: Arc<MemoryServiceState>,
    /// GET 操作处理器
    get_ops: GetOperations,
    /// SET 操作处理器
    set_ops: SetOperations,
    /// SEARCH 操作处理器
    search_ops: SearchOperations,
    /// SYNC 操作处理器
    sync_ops: SyncOperations,
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
        let state = Arc::new(MemoryServiceState::new(
            memory_db,
            vector_storage,
            None,
            node_id.into(),
            None,
        ));

        let get_ops = GetOperations::new(Arc::clone(&state));
        let set_ops = SetOperations::new(Arc::clone(&state));
        let search_ops = SearchOperations::new(Arc::clone(&state));
        let sync_ops = SyncOperations::new(Arc::clone(&state));

        Ok(Self {
            state,
            get_ops,
            set_ops,
            search_ops,
            sync_ops,
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

        let state = Arc::new(MemoryServiceState::new(
            Arc::new(tokio::sync::Mutex::new(memory_db)),
            Arc::new(vector_storage),
            None,
            node_id.into(),
            None,
        ));

        let get_ops = GetOperations::new(Arc::clone(&state));
        let set_ops = SetOperations::new(Arc::clone(&state));
        let search_ops = SearchOperations::new(Arc::clone(&state));
        let sync_ops = SyncOperations::new(Arc::clone(&state));

        Ok(Self {
            state,
            get_ops,
            set_ops,
            search_ops,
            sync_ops,
        })
    }

    /// 设置加密密钥 (v1)
    pub fn with_encryption(mut self, encryption: MemoryEncryption) -> Self {
        // 创建新的 state 并替换所有 ops
        let new_state = Arc::new(MemoryServiceState::new(
            Arc::clone(&self.state.memory_db),
            Arc::clone(&self.state.vector_storage),
            Some(EncryptionWrapper::V1(encryption)),
            self.state.node_id.clone(),
            self.state.namespace.clone(),
        ));

        self.get_ops = GetOperations::new(Arc::clone(&new_state));
        self.set_ops = SetOperations::new(Arc::clone(&new_state));
        self.search_ops = SearchOperations::new(Arc::clone(&new_state));
        self.sync_ops = SyncOperations::new(Arc::clone(&new_state));
        self.state = new_state;
        self
    }

    /// 设置 v2 加密密钥
    pub fn with_v2_encryption(mut self, encryption: MemoryEncryptionV2) -> Self {
        // 创建新的 state 并替换所有 ops
        let new_state = Arc::new(MemoryServiceState::new(
            Arc::clone(&self.state.memory_db),
            Arc::clone(&self.state.vector_storage),
            Some(EncryptionWrapper::V2(encryption)),
            self.state.node_id.clone(),
            self.state.namespace.clone(),
        ));

        self.get_ops = GetOperations::new(Arc::clone(&new_state));
        self.set_ops = SetOperations::new(Arc::clone(&new_state));
        self.search_ops = SearchOperations::new(Arc::clone(&new_state));
        self.sync_ops = SyncOperations::new(Arc::clone(&new_state));
        self.state = new_state;
        self
    }

    /// 设置命名空间
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        // 创建新的 state 并替换所有 ops
        let new_state = Arc::new(MemoryServiceState::new(
            Arc::clone(&self.state.memory_db),
            Arc::clone(&self.state.vector_storage),
            self.state.encryption.clone(),
            self.state.node_id.clone(),
            Some(namespace.into()),
        ));

        self.get_ops = GetOperations::new(Arc::clone(&new_state));
        self.set_ops = SetOperations::new(Arc::clone(&new_state));
        self.search_ops = SearchOperations::new(Arc::clone(&new_state));
        self.sync_ops = SyncOperations::new(Arc::clone(&new_state));
        self.state = new_state;

        self
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
    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        self.set_ops.set(key, value, domain, category).await
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
    pub async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        self.get_ops.get(key).await
    }

    /// 删除记忆
    pub async fn delete(&self, key: &str) -> Result<bool> {
        self.set_ops.delete(key).await
    }

    /// 搜索记忆
    pub async fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<MemoryItem>> {
        self.search_ops.search(query, options).await
    }

    /// 存储记忆并建立向量索引
    pub async fn set_with_embedding(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        self.set_ops.set_with_embedding(key, value, domain, category).await
    }

    /// 语义搜索记忆
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<MemorySearchResult>> {
        self.search_ops.semantic_search(query, limit, threshold).await
    }

    /// 列出所有记忆键
    pub async fn list_keys(&self, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        self.search_ops.list_keys(domain).await
    }

    // ==================== 私域记忆操作 ====================

    /// 存储私域记忆（内部方法）
    async fn set_private(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        self.set_ops.set_private(key, value, category).await
    }

    /// 读取私域记忆（内部方法）
    async fn get_private(&self, key: &str) -> Result<Option<MemoryItem>> {
        self.get_ops.get_private(key).await
    }

    /// 删除私域记忆（内部方法）
    async fn delete_private(&self, key: &str) -> Result<bool> {
        self.set_ops.delete(key).await
    }

    // ==================== 公域记忆操作 ====================

    /// 存储公域记忆（内部方法）
    async fn set_public(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        self.set_ops.set_public(key, value, category).await
    }

    /// 读取公域记忆（内部方法）
    async fn get_public(&self, key: &str) -> Result<Option<MemoryItem>> {
        self.get_ops.get_public(key).await
    }

    /// 删除公域记忆（内部方法）
    async fn delete_public(&self, key: &str) -> Result<bool> {
        self.set_ops.delete(key).await
    }

    // ==================== 向量索引 ====================

    /// 手动索引记忆（用于批量操作）
    pub async fn index_memory(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<String> {
        let full_key = self.state.full_key(key);
        let category_str = format!("{:?}", category);
        self.state
            .vector_storage
            .index_memory(&full_key, value, Some(&category_str))
            .await
    }

    // ==================== P2P 同步 ====================

    /// 获取待同步的公域记忆
    pub async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>> {
        self.sync_ops.get_pending_sync(limit).await
    }

    /// 标记已同步
    pub async fn mark_synced(&self, key: &str) -> Result<()> {
        self.sync_ops.mark_synced(key).await
    }

    /// 导出公域记忆
    pub async fn export_public(&self, since: i64) -> Result<Vec<MemoryItem>> {
        self.sync_ops.export_public(since).await
    }

    /// 导入公域记忆
    pub async fn import_public(&self, items: Vec<MemoryItem>) -> Result<()> {
        self.sync_ops.import_public(items).await
    }

    /// 同步完成回调
    pub async fn on_sync_complete(&self, key: &str, peer_id: &str) -> Result<()> {
        self.sync_ops.on_sync_complete(key, peer_id).await
    }

    // ==================== 工具方法 ====================

    /// 获取节点ID
    pub fn node_id(&self) -> &str {
        self.state.node_id()
    }

    /// 获取命名空间
    pub fn namespace(&self) -> Option<&str> {
        self.state.namespace()
    }

    /// 检查是否有加密
    pub fn is_encrypted(&self) -> bool {
        self.state.is_encrypted()
    }

    /// 关闭服务
    pub async fn close(self) -> Result<()> {
        // 尝试获取锁并关闭数据库
        if let Ok(db) = Arc::try_unwrap(self.state.memory_db.clone()) {
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
                    handle.block_on(async { self.delete(key).await.map(|_| ()) })
                })
            }
            Err(_) => {
                // 没有运行时，创建新的运行时
                let rt = tokio::runtime::Runtime::new()
                    .map_err(|e| crate::error::CisError::Memory(format!("Failed to create runtime: {}", e)))?;
                rt.block_on(async { self.delete(key).await.map(|_| ()) })
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
                        let options = SearchOptions::new().with_limit(limit).with_threshold(0.5);

                        let results = self.search(query, options).await?;

                        let items = results
                            .into_iter()
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
                let rt = tokio::runtime::Runtime::new().map_err(|e| {
                    crate::error::CisError::Memory(format!("Failed to create runtime: {}", e))
                })?;
                rt.block_on(async {
                    let options = SearchOptions::new().with_limit(limit).with_threshold(0.5);

                    let results = self.search(query, options).await?;

                    let items = results
                        .into_iter()
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
            let hash = text
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
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
            state: Arc::new(MemoryServiceState::new(
                Arc::new(tokio::sync::Mutex::new(memory_db)),
                Arc::new(vector_storage),
                None,
                "test-node".to_string(),
                None,
            )),
            get_ops: unsafe { std::mem::zeroed() }, // Will be replaced in actual usage
            set_ops: unsafe { std::mem::zeroed() },
            search_ops: unsafe { std::mem::zeroed() },
            sync_ops: unsafe { std::mem::zeroed() },
        };
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_refactored_service_basic() {
        let service = MemoryService::open_default("test-node").unwrap();

        // 存储私域记忆
        service
            .set("private_key", b"private_value", MemoryDomain::Private, MemoryCategory::Context)
            .await
            .unwrap();

        // 存储公域记忆
        service
            .set("public_key", b"public_value", MemoryDomain::Public, MemoryCategory::Result)
            .await
            .unwrap();

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
    async fn test_refactored_service_delete() {
        let service = MemoryService::open_default("test-node").unwrap();

        service
            .set("to_delete", b"value", MemoryDomain::Private, MemoryCategory::Context)
            .await
            .unwrap();
        assert!(service.get("to_delete").await.unwrap().is_some());

        let deleted = service.delete("to_delete").await.unwrap();
        assert!(deleted);
        assert!(service.get("to_delete").await.unwrap().is_none());

        service.close().await.unwrap();
    }
}
