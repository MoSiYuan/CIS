//! # Memory Operations Module
//!
//! Splits MemoryService operations into independent modules, each responsible for one type of operation.
//!
//! ## Module Structure
//!
//! - `get` - Memory read operations
//! - `set` - Memory write operations
//! - `search` - Search and query operations
//! - `sync` - P2P synchronization operations

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::cache::LruCache;
use crate::memory::{MemoryEncryption, MemoryEncryptionV2};
use crate::storage::memory_db::MemoryDb;
use crate::vector::VectorStorage;

/// Encryption wrapper enum, supporting v1 and v2
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

/// Shared service state
///
/// All operation modules share this state to ensure data consistency.
pub struct MemoryServiceState {
    /// Memory database (private/public separated storage)
    pub memory_db: Arc<Mutex<MemoryDb>>,
    /// Vector storage (for semantic search)
    pub vector_storage: Arc<VectorStorage>,
    /// Encryptor (for private memory) - supports v1 and v2
    pub encryption: Option<EncryptionWrapper>,
    /// Node ID
    pub node_id: String,
    /// Namespace isolation
    pub namespace: Option<String>,
    /// Cache layer (optional)
    pub cache: Option<Arc<LruCache>>,
}

impl MemoryServiceState {
    /// Create a new service state
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

    /// Set cache
    pub fn with_cache(mut self, cache: Arc<LruCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Generate full key (with namespace prefix)
    pub fn full_key(&self, key: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}", ns, key),
            None => key.to_string(),
        }
    }

    /// Get node ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Get namespace
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// Check if encryption is enabled
    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }

    /// Migrate to v2 encryption
    pub fn migrate_to_v2(&self, v2_encryption: MemoryEncryptionV2) -> Option<EncryptionWrapper> {
        self.encryption.as_ref()?;
        Some(EncryptionWrapper::V2(v2_encryption))
    }

    /// Create with v1 encryption
    pub fn with_v1_encryption(encryption: MemoryEncryption) -> Option<EncryptionWrapper> {
        Some(EncryptionWrapper::V1(encryption))
    }

    /// Create with v2 encryption
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
