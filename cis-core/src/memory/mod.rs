//! # Memory Service Module
//!
//! Provides private/public memory management with encryption and access control.
//! Uses independent MemoryDb storage, separated from the core database.
//!
//! ## Phase 3 Migration Note
//!
//! This module is kept for backward compatibility. The memory functionality has been migrated
//! to cis-common/cis-memory crate. New code should use:
//!
//! ```rust
//! use cis_memory::*;  // Recommended
//! ```
//!
//! This module re-exports from cis_memory for backward compatibility.

pub use cis_memory::*;

/// 记忆搜索项
#[derive(Debug, Clone)]
pub struct MemorySearchItem {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
}

/// 记忆服务 Trait（用于 WASM Host API）
pub trait MemoryServiceTrait: Send + Sync {
    /// 获取记忆值
    fn get(&self, key: &str) -> Option<Vec<u8>>;
    /// 设置记忆值
    fn set(&self, key: &str, value: &[u8]) -> crate::error::Result<()>;
    /// 删除记忆
    fn delete(&self, key: &str) -> crate::error::Result<()>;
    /// 搜索记忆
    fn search(&self, query: &str, limit: usize) -> crate::error::Result<Vec<MemorySearchItem>>;
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage::memory_db::MemoryEntry;
use crate::types::{MemoryCategory, MemoryDomain};

pub mod encryption;
pub mod encryption_v2;
pub mod service;
pub mod ops;
pub mod crypto;
pub mod weekly_archived;
pub mod guard;  // Conflict detection guard module (Phase 0: P1.7.0)
pub mod scope;   // Memory scope (v1.1.7: stable hash binding)

// Re-export all public types
pub use self::encryption::MemoryEncryption;
pub use self::encryption_v2::{EncryptionKeyV2, MemoryEncryptionV2};
pub use self::service::{MemoryItem, MemoryService, MemorySearchResult, SearchOptions, SyncMarker};
pub use self::weekly_archived::{WeeklyArchivedMemory, MemoryItem as WeeklyMemoryItem, WeeklyMemoryStats};
pub use self::guard::{ConflictChecked, SafeMemoryContext};  // Conflict detection types
pub use self::scope::MemoryScope;  // Memory scope

/// 扩展的记忆条目（包含更多元数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntryExt {
    pub key: String,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: i64,
    pub updated_at: i64,
    pub accessed_at: Option<i64>,
    pub version: u32,
    pub encrypted: bool,
    pub metadata: HashMap<String, String>,
}

impl From<MemoryEntry> for MemoryEntryExt {
    fn from(entry: MemoryEntry) -> Self {
        Self {
            key: entry.key,
            value: entry.value,
            domain: entry.domain,
            category: entry.category,
            created_at: entry.created_at,
            updated_at: entry.updated_at,
            accessed_at: None,
            version: 1,
            encrypted: matches!(entry.domain, MemoryDomain::Private),
            metadata: HashMap::new(),
        }
    }
}

impl From<MemoryItem> for MemoryEntryExt {
    fn from(item: MemoryItem) -> Self {
        Self {
            key: item.key,
            value: item.value,
            domain: item.domain,
            category: item.category,
            created_at: item.created_at.timestamp(),
            updated_at: item.updated_at.timestamp(),
            accessed_at: None,
            version: item.version as u32,
            encrypted: item.encrypted,
            metadata: HashMap::new(),
        }
    }
}

// SearchOptions is now directly exported from the service module
// Add backward-compatible conversions here if needed

/// 私域/公域记忆过滤条件
#[derive(Debug, Clone)]
pub struct MemoryFilter {
    pub domain: Option<MemoryDomain>,
    pub category: Option<MemoryCategory>,
    pub key_pattern: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_env() {
        let temp_dir = env::temp_dir().join("cis_test_memory_mod");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
    }

    fn cleanup_test_env() {
        std::env::remove_var("CIS_DATA_DIR");
    }

    #[test]
    fn test_memory_entry_ext_from_entry() {
        let entry = MemoryEntry {
            key: "test".to_string(),
            value: b"value".to_vec(),
            domain: MemoryDomain::Private,
            category: MemoryCategory::Context,
            created_at: 1234567890,
            updated_at: 1234567890,
        };

        let ext: MemoryEntryExt = entry.into();
        assert_eq!(ext.key, "test");
        assert_eq!(ext.value, b"value");
        assert!(ext.encrypted);
        assert!(matches!(ext.domain, MemoryDomain::Private));
    }

    #[test]
    fn test_memory_entry_ext_from_item() {
        use chrono::Utc;
        
        let item = MemoryItem {
            key: "test".to_string(),
            value: b"value".to_vec(),
            domain: MemoryDomain::Public,
            category: MemoryCategory::Result,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 2,
            encrypted: false,
            owner: "node1".to_string(),
        };

        let ext: MemoryEntryExt = item.into();
        assert_eq!(ext.key, "test");
        assert_eq!(ext.value, b"value");
        assert!(!ext.encrypted);
        assert_eq!(ext.version, 2);
    }
}
