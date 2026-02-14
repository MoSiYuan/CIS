//! # Memory Operations Module
//!
//! 将 MemoryService 的操作拆分为独立模块，每个模块负责一类操作。
//!
//! ## 模块结构
//!
//! - `get` - 记忆读取操作
//! - `set` - 记忆存储操作
//! - `search` - 搜索和查询操作
//! - `sync` - P2P 同步操作

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::cache::LruCache;
use crate::memory::{MemoryEncryption, MemoryEncryptionV2};
use crate::storage::memory_db::MemoryDb;
use crate::vector::VectorStorage;

/// 加密器枚举，支持 v1 和 v2
#[derive(Clone)]
pub enum EncryptionWrapper {
    V1(MemoryEncryption),
    V2(MemoryEncryptionV2),
}

impl EncryptionWrapper {
    /// 加密数据
    pub fn encrypt(&self, plaintext: &[u8]) -> crate::error::Result<Vec<u8>> {
        match self {
            Self::V1(enc) => enc.encrypt(plaintext),
            Self::V2(enc) => enc.encrypt(plaintext),
        }
    }

    /// 解密数据
    pub fn decrypt(&self, ciphertext: &[u8]) -> crate::error::Result<Vec<u8>> {
        match self {
            Self::V1(enc) => enc.decrypt(ciphertext),
            Self::V2(enc) => enc.decrypt(ciphertext),
        }
    }

    /// 重新加密（密钥轮换）
    pub fn re_encrypt(&self, ciphertext: &[u8], new_wrapper: &EncryptionWrapper) -> crate::error::Result<Vec<u8>> {
        let plaintext = self.decrypt(ciphertext)?;
        new_wrapper.encrypt(&plaintext)
    }
}

/// 共享的服务状态
///
/// 所有操作模块共享此状态，确保数据一致性。
pub struct MemoryServiceState {
    /// 记忆数据库（私域/公域分离存储）
    pub memory_db: Arc<Mutex<MemoryDb>>,
    /// 向量存储（用于语义搜索）
    pub vector_storage: Arc<VectorStorage>,
    /// 加密器（用于私域记忆）- 支持 v1 和 v2
    pub encryption: Option<EncryptionWrapper>,
    /// 节点ID
    pub node_id: String,
    /// 命名空间隔离
    pub namespace: Option<String>,
    /// 缓存层（可选）
    pub cache: Option<Arc<LruCache>>,
}

impl MemoryServiceState {
    /// 创建新的服务状态
    pub fn new(
        memory_db: Arc<Mutex<MemoryDb>>,
        vector_storage: Arc<VectorStorage>,
        encryption: Option<EncryptionWrapper>,
        node_id: String,
        namespace: Option<String>,
    ) -> Self {
        Self {
            memory_db,
            vector_storage,
            encryption,
            node_id,
            namespace,
            cache: None,
        }
    }

    /// 设置缓存
    pub fn with_cache(mut self, cache: Arc<LruCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// 生成完整的键（包含命名空间前缀）
    pub fn full_key(&self, key: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}", ns, key),
            None => key.to_string(),
        }
    }

    /// 获取节点ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// 获取命名空间
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// 检查是否启用加密
    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }

    /// 迁移到 v2 加密
    pub fn migrate_to_v2(&self, v2_encryption: MemoryEncryptionV2) -> Option<EncryptionWrapper> {
        self.encryption.as_ref()?;
        Some(EncryptionWrapper::V2(v2_encryption))
    }

    /// 从 v1 加密创建
    pub fn with_v1_encryption(encryption: MemoryEncryption) -> Option<EncryptionWrapper> {
        Some(EncryptionWrapper::V1(encryption))
    }

    /// 从 v2 加密创建
    pub fn with_v2_encryption(encryption: MemoryEncryptionV2) -> Option<EncryptionWrapper> {
        Some(EncryptionWrapper::V2(encryption))
    }
}

pub mod get;
pub mod set;
pub mod search;
pub mod sync;

pub use get::GetOperations;
pub use set::SetOperations;
pub use search::SearchOperations;
pub use sync::SyncOperations;
