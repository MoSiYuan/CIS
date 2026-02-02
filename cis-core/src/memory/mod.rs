//! # 记忆服务模块
//!
//! 提供私域/公域记忆管理，支持加密和访问控制。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{CisError, Result};
use crate::storage::db::CoreDb;
use crate::types::{MemoryCategory, MemoryDomain};

pub mod encryption;

use encryption::MemoryEncryption;



/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
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

/// 记忆搜索选项
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub domain: Option<MemoryDomain>,
    pub category: Option<MemoryCategory>,
    pub key_prefix: Option<String>,
    pub time_range: Option<(i64, i64)>,
    pub limit: Option<usize>,
}

/// 记忆服务
pub struct MemoryService {
    core_db: Arc<Mutex<CoreDb>>,
    encryption: Option<MemoryEncryption>,
    namespace: Option<String>,
}

impl MemoryService {
    pub fn new(core_db: Arc<Mutex<CoreDb>>) -> Self {
        Self {
            core_db,
            encryption: None,
            namespace: None,
        }
    }

    pub fn with_encryption(mut self, encryption: MemoryEncryption) -> Self {
        self.encryption = Some(encryption);
        self
    }

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

    /// 存储记忆
    pub fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        match domain {
            MemoryDomain::Private => self.set_private(key, value, category),
            MemoryDomain::Public => self.set_public(key, value, category),
        }
    }

    /// 存储私域记忆（加密）
    fn set_private(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        let full_key = self.full_key(key);
        let now = chrono::Utc::now().timestamp();

        let (encrypted_value, encrypted_flag) = if let Some(ref enc) = self.encryption {
            (enc.encrypt(value)?, true)
        } else {
            (value.to_vec(), false)
        };

        let entry = MemoryEntry {
            key: full_key.clone(),
            value: encrypted_value,
            domain: MemoryDomain::Private,
            category,
            created_at: now,
            updated_at: now,
            accessed_at: None,
            version: 1,
            encrypted: encrypted_flag,
            metadata: HashMap::new(),
        };

        let data = bincode::serialize(&entry)
            .map_err(|e| CisError::storage(format!("Failed to serialize: {}", e)))?;

        let db = self.core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        db.set_config(&format!("memory/private/{}", full_key), &data, encrypted_flag)?;
        db.register_memory_index(&full_key, None, "core", Some(&format!("{:?}", category)))?;

        Ok(())
    }

    /// 存储公域记忆（明文）
    fn set_public(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        let full_key = self.full_key(key);
        let now = chrono::Utc::now().timestamp();

        let entry = MemoryEntry {
            key: full_key.clone(),
            value: value.to_vec(),
            domain: MemoryDomain::Public,
            category,
            created_at: now,
            updated_at: now,
            accessed_at: None,
            version: 1,
            encrypted: false,
            metadata: HashMap::new(),
        };

        let data = bincode::serialize(&entry)
            .map_err(|e| CisError::storage(format!("Failed to serialize: {}", e)))?;

        let db = self.core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        db.set_config(&format!("memory/public/{}", full_key), &data, false)?;
        db.register_memory_index(&full_key, None, "core", Some(&format!("{:?}", category)))?;

        Ok(())
    }

    /// 读取记忆
    pub fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let full_key = self.full_key(key);

        let db = self.core_db.lock()
            .map_err(|e| CisError::storage(format!("Lock failed: {}", e)))?;

        // 尝试私域
        if let Some((data, _)) = db.get_config(&format!("memory/private/{}", full_key))? {
            let mut entry: MemoryEntry = bincode::deserialize(&data)
                .map_err(|e| CisError::storage(format!("Failed to deserialize: {}", e)))?;

            if entry.encrypted {
                if let Some(ref enc) = self.encryption {
                    entry.value = enc.decrypt(&entry.value)?;
                } else {
                    return Err(CisError::storage("Encrypted memory but no key available"));
                }
            }
            return Ok(Some(entry));
        }

        // 尝试公域
        if let Some((data, _)) = db.get_config(&format!("memory/public/{}", full_key))? {
            let entry: MemoryEntry = bincode::deserialize(&data)
                .map_err(|e| CisError::storage(format!("Failed to deserialize: {}", e)))?;
            return Ok(Some(entry));
        }

        Ok(None)
    }

    /// 删除记忆
    pub fn delete(&self, key: &str) -> Result<bool> {
        let _ = key;
        // TODO: 实现删除
        Ok(false)
    }

    /// 搜索记忆
    pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<MemoryEntry>> {
        let _ = (query, options);
        Ok(vec![])
    }

    /// 列出记忆键
    pub fn list_keys(&self, prefix: &str, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        let _ = (prefix, domain);
        Ok(vec![])
    }

    /// 导出公域记忆（用于 P2P 同步）
    pub fn export_public(&self, since: i64) -> Result<Vec<MemoryEntry>> {
        let _ = since;
        Ok(vec![])
    }

    /// 导入公域记忆
    pub fn import_public(&self, entries: Vec<MemoryEntry>) -> Result<()> {
        for entry in entries {
            if entry.domain == MemoryDomain::Public {
                self.set_public(&entry.key, &entry.value, entry.category)?;
            }
        }
        Ok(())
    }
}

