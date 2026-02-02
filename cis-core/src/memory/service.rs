//! # 记忆服务
//!
//! 提供私域(Private)和公域(Public)记忆分离存储。
//!
//! ## 特性
//! - 私域记忆: 本地加密存储，永不同步
//! - 公域记忆: 明文存储，可P2P同步
//! - 向量索引: 统一语义检索
//! - 访问控制: 基于权限的读写

use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{CisError, Result};
use crate::memory::{MemoryEncryption, MemoryEntryExt};
use crate::storage::memory_db::{MemoryDb, MemoryEntry};
use crate::types::{MemoryCategory, MemoryDomain};
use crate::vector::VectorStorage;

/// 记忆服务 - 私域/公域记忆分离管理
///
/// 提供统一的记忆管理接口，支持：
/// - 私域记忆：本地加密，永不同步
/// - 公域记忆：可联邦同步
/// - 向量索引：语义检索
pub struct MemoryService {
    /// 记忆数据库（私域/公域分离存储）
    memory_db: Arc<Mutex<MemoryDb>>,
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
    pub fn new(
        memory_db: Arc<Mutex<MemoryDb>>,
        vector_storage: Arc<VectorStorage>,
        node_id: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            memory_db,
            vector_storage,
            encryption: None,
            node_id: node_id.into(),
            namespace: None,
        })
    }

    /// 从默认路径创建记忆服务
    pub fn open_default(node_id: impl Into<String>) -> Result<Self> {
        let memory_db = MemoryDb::open_default()?;
        let vector_storage = VectorStorage::open_default()?;
        
        Ok(Self {
            memory_db: Arc::new(Mutex::new(memory_db)),
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
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `domain`: 私域或公域
    /// - `category`: 分类
    pub fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.full_key(key);
        
        match domain {
            MemoryDomain::Private => self.set_private(&full_key, value, category),
            MemoryDomain::Public => self.set_public(&full_key, value, category),
        }
    }

    /// 读取记忆
    pub fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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
    pub fn delete(&self, key: &str) -> Result<bool> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        let deleted = db.delete(&full_key)?;

        if deleted {
            // 从向量索引中移除
            let _ = self.vector_storage.delete_memory_index(&full_key);
        }

        Ok(deleted)
    }

    /// 语义搜索记忆
    ///
    /// 同时搜索私域和公域记忆，根据选项过滤
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
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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

    /// 列出所有记忆键
    pub fn list_keys(&self, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        
        let prefix = match &self.namespace {
            Some(ns) => format!("{}/", ns),
            None => String::new(),
        };
        
        db.list_keys(&prefix, domain)
    }

    // ==================== 私域记忆操作 ====================

    fn set_private(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 加密数据
        let encrypted = if let Some(ref enc) = self.encryption {
            enc.encrypt(value)?
        } else {
            // 无密钥时存储明文，但标记为私域
            value.to_vec()
        };

        // 2. 存储到 memory_db
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        db.set_private(key, &encrypted, category)?;

        // 3. 更新向量索引（使用原始值）
        self.spawn_index_update(key, value, &category);

        Ok(())
    }

    fn get_private(&self, key: &str) -> Result<Option<MemoryItem>> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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

    fn delete_private(&self, key: &str) -> Result<bool> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        // 检查是否是私域记忆
        if let Some(entry) = db.get(key)? {
            if entry.domain != MemoryDomain::Private {
                return Ok(false);
            }
        }

        db.delete(key)
    }

    // ==================== 公域记忆操作 ====================

    fn set_public(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 明文存储
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        db.set_public(key, value, category)?;

        // 2. 更新向量索引
        self.spawn_index_update(key, value, &category);

        // 3. 标记为待同步（P2P）- 已由 MemoryDb 自动处理

        Ok(())
    }

    fn get_public(&self, key: &str) -> Result<Option<MemoryItem>> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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

    fn delete_public(&self, key: &str) -> Result<bool> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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
    pub fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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
    pub fn mark_synced(&self, key: &str) -> Result<()> {
        let full_key = self.full_key(key);
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;
        db.mark_synced(&full_key)
    }

    /// 导出公域记忆（用于 P2P 同步）
    pub fn export_public(&self, since: i64) -> Result<Vec<MemoryItem>> {
        let db = self.memory_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

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
    pub fn import_public(&self, items: Vec<MemoryItem>) -> Result<()> {
        for item in items {
            if item.domain == MemoryDomain::Public {
                self.set_public(&item.key, &item.value, item.category)?;
            }
        }
        Ok(())
    }

    /// 同步完成回调
    pub fn on_sync_complete(&self, key: &str, peer_id: &str) -> Result<()> {
        tracing::info!("Memory {} synced to peer {}", key, peer_id);
        
        // 标记为已同步
        self.mark_synced(key)
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
    pub fn close(self) -> Result<()> {
        // 尝试获取锁并关闭数据库
        if let Ok(db) = Arc::try_unwrap(self.memory_db) {
            if let Ok(db) = db.into_inner() {
                db.close()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_service() -> MemoryService {
        let temp_dir = env::temp_dir().join("cis_test_memory_service_v2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        // 使用 mock vector storage（需要特殊处理）
        let vector_path = temp_dir.join("vector.db");
        let vector_storage = VectorStorage::open(&vector_path, None).unwrap();
        
        MemoryService {
            memory_db: Arc::new(Mutex::new(memory_db)),
            vector_storage: Arc::new(vector_storage),
            encryption: None,
            node_id: "test-node".to_string(),
            namespace: None,
        }
    }

    fn cleanup_test_service() {
        let temp_dir = env::temp_dir().join("cis_test_memory_service_v2");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_memory_service_basic() {
        let service = setup_test_service();

        // 存储私域记忆
        service.set_private("private_key", b"private_value", MemoryCategory::Context).unwrap();

        // 存储公域记忆
        service.set_public("public_key", b"public_value", MemoryCategory::Result).unwrap();

        // 读取
        let item = service.get("private_key").unwrap().unwrap();
        assert_eq!(item.key, "private_key");
        assert_eq!(item.value, b"private_value");
        assert!(matches!(item.domain, MemoryDomain::Private));
        assert_eq!(item.owner, "test-node");

        let item = service.get("public_key").unwrap().unwrap();
        assert_eq!(item.key, "public_key");
        assert_eq!(item.value, b"public_value");
        assert!(matches!(item.domain, MemoryDomain::Public));

        service.close().unwrap();
        cleanup_test_service();
    }

    #[test]
    fn test_memory_service_delete() {
        let service = setup_test_service();

        service.set_private("to_delete", b"value", MemoryCategory::Context).unwrap();
        assert!(service.get("to_delete").unwrap().is_some());

        let deleted = service.delete("to_delete").unwrap();
        assert!(deleted);
        assert!(service.get("to_delete").unwrap().is_none());

        service.close().unwrap();
        cleanup_test_service();
    }

    #[test]
    fn test_memory_service_with_encryption() {
        let temp_dir = env::temp_dir().join("cis_test_memory_enc_v2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        let vector_path = temp_dir.join("vector.db");
        let vector_storage = VectorStorage::open(&vector_path, None).unwrap();
        
        let encryption = MemoryEncryption::from_node_key(b"test-key");
        
        let service = MemoryService {
            memory_db: Arc::new(Mutex::new(memory_db)),
            vector_storage: Arc::new(vector_storage),
            encryption: Some(encryption),
            node_id: "test-node".to_string(),
            namespace: None,
        };

        // 存储私域记忆（会被加密）
        service.set_private("encrypted_key", b"secret_value", MemoryCategory::Context).unwrap();

        // 读取（自动解密）
        let item = service.get("encrypted_key").unwrap().unwrap();
        assert_eq!(item.value, b"secret_value");
        assert!(item.encrypted);

        service.close().unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_memory_service_namespace() {
        let temp_dir = env::temp_dir().join("cis_test_memory_ns_v2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();
        
        let vector_path = temp_dir.join("vector.db");
        let vector_storage = VectorStorage::open(&vector_path, None).unwrap();
        
        let memory_db_arc = Arc::new(Mutex::new(memory_db));
        let vector_storage_arc = Arc::new(vector_storage);
        
        let service1 = MemoryService {
            memory_db: Arc::clone(&memory_db_arc),
            vector_storage: Arc::clone(&vector_storage_arc),
            encryption: None,
            node_id: "node1".to_string(),
            namespace: Some("ns1".to_string()),
        };
        
        // 需要先存储，再读取
        service1.set_private("key", b"value1", MemoryCategory::Context).unwrap();

        let item = service1.get("key").unwrap().unwrap();
        assert_eq!(item.value, b"value1");

        // Note: 实际上两个服务共享同一个底层数据库，
        // 所以 namespace 隔离只是在 key 上加前缀
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
