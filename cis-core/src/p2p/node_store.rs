//! 持久化节点和记录存储
//!
//! 为 libp2p KadDHT 提供持久化存储层，支持：
//! - DHT 记录持久化
//! - 节点信息存储
//! - 快速恢复和重启
//!
//! ## 存储结构
//!
//! ```
//! ~/.cis/data/dht/
//! ├── records/           # DHT 记录 (RocksDB)
//! ├── nodes/            # 节点信息 (RocksDB)
//! └── metadata/         # 元数据 (JSON)
//!     └── info.json
//! ```

use crate::error::{CisError, Result};
use crate::p2p::NodeInfo;

use libp2p::{
    kad::{
        record::{Key, Record},
        store::{RecordStore, Error as StoreError},
    },
    PeerId,
};

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};

// 持久化存储特性标志
#[cfg(feature = "persistent-storage")]
use rocksdb::{DB, Options as RocksOptions, Iterator as RocksIterator};

/// 持久化记录存储
///
/// 实现 libp2p kad RecordStore trait，使用 RocksDB 进行持久化。
#[cfg(feature = "persistent-storage")]
pub struct PersistentRecordStore {
    db: Arc<DB>,
    local_peer_id: PeerId,
}

#[cfg(feature = "persistent-storage")]
impl PersistentRecordStore {
    /// 创建新的持久化 Store
    pub fn new(local_peer_id: PeerId, db_path: PathBuf) -> Result<Self> {
        // 确保目录存在
        std::fs::create_dir_all(&db_path)
            .map_err(|e| CisError::storage(format!("Failed to create directory: {}", e)))?;

        // 配置 RocksDB
        let mut opts = RocksOptions::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        opts.set_max_open_files(1000);
        opts.set_use_fsync(false);

        // 打开数据库
        let db = DB::open(&opts, &db_path)
            .map_err(|e| CisError::storage(format!("Failed to open RocksDB: {}", e)))?;

        tracing::info!("PersistentRecordStore opened at: {:?}", db_path);

        Ok(Self {
            db: Arc::new(db),
            local_peer_id,
        })
    }

    /// 获取记录过期时间
    fn get_expiration(&self, key: &Key) -> Option<SystemTime> {
        let meta_key = format!("meta:{}", hex::encode(key.as_ref()));
        match self.db.get(meta_key.as_bytes()) {
            Ok(Some(data)) => {
                if let Ok(ts) = serde_json::from_slice::<ExpirationMeta>(&data) {
                    ts.expires.map(|d| {
                        SystemTime::UNIX_EPOCH + Duration::from_secs(d)
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 设置记录过期时间
    fn set_expiration(&self, key: &Key, expires: SystemTime) {
        let meta_key = format!("meta:{}", hex::encode(key.as_ref()));
        let timestamp = expires
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let meta = ExpirationMeta {
            expires: Some(timestamp),
            created_at: Some(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
        };

        if let Ok(data) = serde_json::to_vec(&meta) {
            let _ = self.db.put(meta_key.as_bytes(), &data);
        }
    }

    /// 清理过期记录
    pub fn cleanup_expired(&self) -> Result<usize> {
        let now = SystemTime::now();
        let mut cleaned = 0;

        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        for item in iter {
            if let Ok((key, _)) = item {
                // 跳过元数据记录
                if key.starts_with(b"meta:") {
                    continue;
                }

                let key_bytes = Key::new(&hex::encode(&key));
                if let Some(expiration) = self.get_expiration(&key_bytes) {
                    if let Ok(dur) = now.duration_since(expiration) {
                        if dur.as_secs() > 0 {
                            let _ = self.db.delete(&key);
                            cleaned += 1;
                        }
                    }
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned up {} expired records", cleaned);
        }

        Ok(cleaned)
    }
}

#[cfg(feature = "persistent-storage")]
impl RecordStore for PersistentRecordStore {
    type RecordsIter<'a> = std::vec::IntoIter<Record>;

    fn get(&self, k: &Key) -> std::result::Result<Option<Record>, StoreError> {
        // 检查是否过期
        if let Some(expiration) = self.get_expiration(k) {
            if let Ok(dur) = SystemTime::now().duration_since(expiration) {
                if dur.as_secs() > 0 {
                    return Ok(None);
                }
            }
        }

        // 获取记录
        let key_bytes = hex::encode(k.as_ref());
        match self.db.get(key_bytes.as_bytes()) {
            Ok(Some(value)) => {
                bincode::deserialize(&value)
                    .map(Some)
                    .map_err(|_| StoreError::Unavailable)
            }
            Ok(None) => Ok(None),
            Err(_) => Err(StoreError::Unavailable),
        }
    }

    fn put(&self, record: Record) -> std::result::Result<(), StoreError> {
        let key_bytes = hex::encode(record.key.as_ref());
        let value = bincode::serialize(&record)
            .map_err(|_| StoreError::Unavailable)?;

        self.db.put(key_bytes.as_bytes(), &value)
            .map_err(|_| StoreError::Unavailable)?;

        // 设置过期时间
        if let Some(expires) = record.expires {
            self.set_expiration(&record.key, expires);
        }

        Ok(())
    }

    fn remove(&self, k: &Key) -> std::result::Result<(), StoreError> {
        let key_bytes = hex::encode(k.as_ref());

        self.db.delete(key_bytes.as_bytes())
            .map_err(|_| StoreError::Unavailable)?;

        // 删除元数据
        let meta_key = format!("meta:{}", key_bytes);
        let _ = self.db.delete(meta_key.as_bytes());

        Ok(())
    }
}

/// 过期元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExpirationMeta {
    expires: Option<u64>,
    created_at: Option<u64>,
}

/// 节点信息存储
///
/// 持久化存储已知的节点信息，支持快速恢复。
#[cfg(feature = "persistent-storage")]
pub struct NodeInfoStore {
    db: Arc<DB>,
}

#[cfg(feature = "persistent-storage")]
impl NodeInfoStore {
    /// 创建新的节点存储
    pub fn new(db_path: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&db_path)
            .map_err(|e| CisError::storage(format!("Failed to create directory: {}", e)))?;

        let mut opts = RocksOptions::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, &db_path)
            .map_err(|e| CisError::storage(format!("Failed to open RocksDB: {}", e)))?;

        tracing::info!("NodeInfoStore opened at: {:?}", db_path);

        Ok(Self {
            db: Arc::new(db),
        })
    }

    /// 保存节点信息
    pub fn save_node(&self, node: &NodeInfo) -> Result<()> {
        let key = format!("node:{}", node.summary.id);
        let value = bincode::serialize(node)
            .map_err(|e| CisError::storage(format!("Serialize error: {}", e)))?;

        self.db.put(key.as_bytes(), &value)
            .map_err(|e| CisError::storage(format!("DB error: {}", e)))?;

        tracing::debug!("Saved node: {}", node.summary.id);
        Ok(())
    }

    /// 获取节点信息
    pub fn get_node(&self, node_id: &str) -> Result<Option<NodeInfo>> {
        let key = format!("node:{}", node_id);
        match self.db.get(key.as_bytes()) {
            Ok(Some(value)) => {
                let node = bincode::deserialize(&value)
                    .map_err(|e| CisError::storage(format!("Deserialize error: {}", e)))?;
                Ok(Some(node))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CisError::storage(format!("DB error: {}", e))),
        }
    }

    /// 删除节点
    pub fn delete_node(&self, node_id: &str) -> Result<()> {
        let key = format!("node:{}", node_id);
        self.db.delete(key.as_bytes())
            .map_err(|e| CisError::storage(format!("DB error: {}", e)))?;
        Ok(())
    }

    /// 列出所有节点
    pub fn list_all_nodes(&self) -> Result<Vec<NodeInfo>> {
        let prefix = b"node:";
        let iter = self.db.prefix_iterator(prefix);
        let mut nodes = Vec::new();

        for item in iter {
            let (_, value) = item
                .map_err(|e| CisError::storage(format!("DB iterator error: {}", e)))?;
            let node = bincode::deserialize(&value)
                .map_err(|e| CisError::storage(format!("Deserialize error: {}", e)))?;
            nodes.push(node);
        }

        tracing::debug!("Listed {} nodes", nodes.len());
        Ok(nodes)
    }

    /// 列出在线节点
    pub fn list_online_nodes(&self) -> Result<Vec<NodeInfo>> {
        let all_nodes = self.list_all_nodes()?;
        let now = Utc::now();

        let online_nodes: Vec<_> = all_nodes
            .into_iter()
            .filter(|n| {
                if let Ok(elapsed) = now.signed_duration_since(n.summary.last_seen).num_seconds() {
                    elapsed < 600 // 10 分钟内活跃
                } else {
                    false
                }
            })
            .collect();

        Ok(online_nodes)
    }

    /// 获取节点数量
    pub fn count(&self) -> Result<usize> {
        let iter = self.db.prefix_iterator(b"node:");
        let count = iter.count();
        Ok(count)
    }

    /// 清理不活跃的节点
    pub fn cleanup_inactive(&self, inactive_duration_secs: i64) -> Result<usize> {
        let nodes = self.list_all_nodes()?;
        let now = Utc::now();
        let mut removed = 0;

        for node in nodes {
            if let Ok(elapsed) = now.signed_duration_since(node.summary.last_seen).num_seconds() {
                if elapsed > inactive_duration_secs {
                    self.delete_node(&node.summary.id)?;
                    removed += 1;
                }
            }
        }

        if removed > 0 {
            tracing::info!("Cleaned up {} inactive nodes", removed);
        }

        Ok(removed)
    }
}

/// 内存记录存储（非持久化，用于测试）
///
/// 当不启用 persistent-storage 特性时使用。
#[cfg(not(feature = "persistent-storage"))]
pub struct PersistentRecordStore {
    records: Arc<tokio::sync::RwLock<std::collections::HashMap<Vec<u8>, Record>>>,
    local_peer_id: PeerId,
}

#[cfg(not(feature = "persistent-storage"))]
impl PersistentRecordStore {
    pub fn new(local_peer_id: PeerId, _db_path: PathBuf) -> Result<Self> {
        tracing::warn!("PersistentRecordStore running in-memory mode (rocksdb feature not enabled)");
        Ok(Self {
            records: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            local_peer_id,
        })
    }
}

#[cfg(not(feature = "persistent-storage"))]
impl RecordStore for PersistentRecordStore {
    type RecordsIter<'a> = std::vec::IntoIter<Record>;

    fn get(&self, k: &Key) -> std::result::Result<Option<Record>, StoreError> {
        let records = self.records.blocking_read();
        Ok(records.get(k.as_ref()).cloned())
    }

    fn put(&self, record: Record) -> std::result::Result<(), StoreError> {
        let mut records = self.records.blocking_write();
        records.insert(record.key.as_ref().to_vec(), record);
        Ok(())
    }

    fn remove(&self, k: &Key) -> std::result::Result<(), StoreError> {
        let mut records = self.records.blocking_write();
        records.remove(k.as_ref());
        Ok(())
    }
}

/// 存储统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub total_records: usize,
    pub total_nodes: usize,
    pub online_nodes: usize,
    pub storage_size_bytes: u64,
}

/// 存储管理器
///
/// 管理记录和节点的持久化存储。
pub struct StorageManager {
    records: PersistentRecordStore,
    nodes: NodeInfoStore,
    local_peer_id: PeerId,
}

impl StorageManager {
    /// 创建新的存储管理器
    pub fn new(local_peer_id: PeerId, base_path: PathBuf) -> Result<Self> {
        let records_db_path = base_path.join("records");
        let nodes_db_path = base_path.join("nodes");

        let records = PersistentRecordStore::new(local_peer_id, records_db_path)?;
        let nodes = NodeInfoStore::new(nodes_db_path)?;

        Ok(Self {
            records,
            nodes,
            local_peer_id,
        })
    }

    /// 获取记录存储
    pub fn records(&self) -> &PersistentRecordStore {
        &self.records
    }

    /// 获取节点存储
    pub fn nodes(&self) -> &NodeInfoStore {
        &self.nodes
    }

    /// 获取本地 PeerId
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// 清理过期数据
    pub fn cleanup(&self) -> Result<CleanupStats> {
        let expired_records = self.records.cleanup_expired()?;
        let inactive_nodes = self.nodes.cleanup_inactive(3600)?; // 1 小时

        Ok(CleanupStats {
            expired_records,
            inactive_nodes,
        })
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> Result<StoreStats> {
        // TODO: 实现存储大小计算
        Ok(StoreStats {
            total_records: 0, // 需要实现
            total_nodes: self.nodes.count()?,
            online_nodes: self.nodes.list_online_nodes()?.len(),
            storage_size_bytes: 0,
        })
    }
}

/// 清理统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupStats {
    pub expired_records: usize,
    pub inactive_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_store_stats() {
        let stats = StoreStats {
            total_records: 100,
            total_nodes: 50,
            online_nodes: 30,
            storage_size_bytes: 1024000,
        };

        assert_eq!(stats.total_records, 100);
        assert_eq!(stats.total_nodes, 50);
    }

    #[cfg(feature = "persistent-storage")]
    #[test]
    fn test_persistent_record_store() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let local_peer_id = PeerId::random();
        let store = PersistentRecordStore::new(local_peer_id, db_path).unwrap();

        // 测试基本操作
        let key = Key::new(&hex::encode(b"test_key"));
        let record = Record {
            key: key.clone(),
            value: b"test_value".to_vec(),
            publisher: Some(local_peer_id),
            expires: None,
        };

        // Put
        store.put(record.clone()).unwrap();

        // Get
        let retrieved = store.get(&key).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, b"test_value");

        // Remove
        store.remove(&key).unwrap();
        let removed = store.get(&key).unwrap();
        assert!(removed.is_none());
    }

    #[cfg(feature = "persistent-storage")]
    #[test]
    fn test_node_info_store() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_nodes");

        use crate::service::node_service::{NodeSummary, NodeStatus};

        let store = NodeInfoStore::new(db_path).unwrap();

        let node = NodeInfo {
            summary: NodeSummary {
                id: "test_node".to_string(),
                did: "did:cis:test_node".to_string(),
                name: "Test Node".to_string(),
                status: NodeStatus::Online,
                endpoint: "127.0.0.1:7677".to_string(),
                version: "1.0.0".to_string(),
                last_seen: Utc::now(),
                capabilities: vec![],
            },
            public_key: "test_key".to_string(),
            metadata: std::collections::HashMap::new(),
            trust_score: 1.0,
            is_blacklisted: false,
            created_at: Utc::now(),
        };

        // Save
        store.save_node(&node).unwrap();

        // Get
        let retrieved = store.get_node("test_node").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().summary.id, "test_node");

        // List
        let all_nodes = store.list_all_nodes().unwrap();
        assert_eq!(all_nodes.len(), 1);

        // Delete
        store.delete_node("test_node").unwrap();
        let deleted = store.get_node("test_node").unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn test_cleanup_stats() {
        let stats = CleanupStats {
            expired_records: 10,
            inactive_nodes: 5,
        };

        assert_eq!(stats.expired_records, 10);
        assert_eq!(stats.inactive_nodes, 5);
    }
}
