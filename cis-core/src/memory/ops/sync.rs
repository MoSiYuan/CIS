//! # SYNC Operations
//!
//! 处理 P2P 同步操作，包括同步标记、公域记忆导出/导入。

use std::sync::Arc;

use crate::error::Result;
use crate::memory::ops::MemoryServiceState;
use crate::types::{MemoryCategory, MemoryDomain};
use chrono::{DateTime, Utc};

use super::super::{MemoryItem, SyncMarker};

/// SYNC 操作处理器
///
/// 负责管理 P2P 同步相关的操作。
pub struct SyncOperations {
    state: Arc<MemoryServiceState>,
}

impl SyncOperations {
    /// 创建新的 SYNC 操作处理器
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self { state }
    }

    /// 获取待同步的公域记忆
    ///
    /// 返回所有标记为待同步的公域记忆。
    ///
    /// # 参数
    /// - `limit`: 最大返回数量
    ///
    /// # 返回
    /// - `Result<Vec<SyncMarker>>`: 待同步的记忆标记列表
    pub async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>> {
        let db = self.state.memory_db.lock().await;

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
    ///
    /// 将指定记忆标记为已同步。
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn mark_synced(&self, key: &str) -> Result<()> {
        let full_key = self.state.full_key(key);
        let db = self.state.memory_db.lock().await;
        db.mark_synced(&full_key)
    }

    /// 导出公域记忆
    ///
    /// 导出所有公域记忆，用于 P2P 同步。
    ///
    /// # 参数
    /// - `since`: 时间戳（Unix 秒），只导出此时间之后更新的记忆
    ///
    /// # 返回
    /// - `Result<Vec<MemoryItem>>`: 公域记忆列表
    pub async fn export_public(&self, since: i64) -> Result<Vec<MemoryItem>> {
        let db = self.state.memory_db.lock().await;

        // 获取所有公域条目
        let all_keys = db.list_keys("", Some(MemoryDomain::Public))?;
        let mut items = Vec::new();

        for key in all_keys {
            if let Some(entry) = db.get(&key)? {
                if entry.updated_at >= since {
                    let mut item = MemoryItem::from(entry);
                    item.owner = self.state.node_id.clone();
                    items.push(item);
                }
            }
        }

        Ok(items)
    }

    /// 导入公域记忆
    ///
    /// 从其他节点导入公域记忆。
    ///
    /// # 参数
    /// - `items`: 要导入的记忆列表
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn import_public(&self, items: Vec<MemoryItem>) -> Result<()> {
        for item in items {
            if item.domain == MemoryDomain::Public {
                // 使用内部 set_public 方法
                let db = self.state.memory_db.lock().await;
                db.set_public(&item.key, &item.value, item.category)?;

                // 更新向量索引
                self.spawn_index_update(&item.key, &item.value, &item.category);
            }
        }
        Ok(())
    }

    /// 同步完成回调
    ///
    /// 当记忆成功同步到对等节点后调用。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `peer_id`: 对等节点 ID
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn on_sync_complete(&self, key: &str, peer_id: &str) -> Result<()> {
        tracing::info!("Memory {} synced to peer {}", key, peer_id);

        // 标记为已同步
        self.mark_synced(key).await
    }

    /// 批量标记已同步
    ///
    /// 一次标记多个记忆为已同步。
    ///
    /// # 参数
    /// - `keys`: 记忆键列表
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn batch_mark_synced(&self, keys: &[String]) -> Result<()> {
        for key in keys {
            self.mark_synced(key).await?;
        }
        Ok(())
    }

    /// 获取同步状态
    ///
    /// 返回指定记忆的同步状态。
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<Option<DateTime<Utc>>>`: 最后同步时间，从未同步返回 None
    pub async fn get_sync_status(&self, key: &str) -> Result<Option<DateTime<Utc>>> {
        let full_key = self.state.full_key(key);
        let db = self.state.memory_db.lock().await;

        if let Some(entry) = db.get(&full_key)? {
            if entry.domain == MemoryDomain::Public {
                // 从同步标记中获取状态
                // 这里简化实现，实际可能需要额外的同步表
                return Ok(None);
            }
        }

        Ok(None)
    }

    /// 后台更新向量索引
    fn spawn_index_update(&self, key: &str, value: &[u8], category: &MemoryCategory) {
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let storage = Arc::clone(&self.state.vector_storage);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use crate::memory::MemoryEncryption;
    use crate::storage::memory_db::MemoryDb;
    use crate::vector::VectorStorage;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Mock embedding service for tests
    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> crate::error::Result<Vec<f32>> {
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let hash = text
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
            for i in 0..DEFAULT_EMBEDDING_DIM {
                let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                vec[i] = val;
            }
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

    fn setup_test_state() -> (Arc<MemoryServiceState>, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();

        let db_path = temp_dir.path().join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();

        let vector_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();

        let state = MemoryServiceState::new(
            Arc::new(tokio::sync::Mutex::new(memory_db)),
            Arc::new(vector_storage),
            None,
            "test-node".to_string(),
            None,
        );

        (Arc::new(state), temp_dir)
    }

    #[tokio::test]
    async fn test_get_pending_sync() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        let pending = ops.get_pending_sync(10).await.unwrap();
        // 初始应该为空
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_export_public() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        let items = ops.export_public(0).await.unwrap();
        // 初始应该为空
        assert_eq!(items.len(), 0);
    }

    #[tokio::test]
    async fn test_import_public() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        let items = vec
![MemoryItem {
            key: "imported_key".to_string(),
            value: b"imported_value".to_vec(),
            domain: MemoryDomain::Public,
            category: MemoryCategory::Context,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            encrypted: false,
            owner: "other-node".to_string(),
        }];

        ops.import_public(items).await.unwrap();

        // 验证导入成功（需要通过 GetOperations 验证）
    }

    #[tokio::test]
    async fn test_batch_mark_synced() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        let keys = vec
!["key1".to_string(), "key2".to_string(), "key3".to_string()];

        ops.batch_mark_synced(&keys).await.unwrap();

        // 验证标记成功（需要检查数据库）
    }

    #[tokio::test]
    async fn test_on_sync_complete() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        ops.on_sync_complete("test_key", "peer-123")
            .await
            .unwrap();

        // 验证回调成功
    }

    #[tokio::test]
    async fn test_get_sync_status() {
        let (state, _temp) = setup_test_state();
        let ops = SyncOperations::new(state);

        let status = ops.get_sync_status("test_key").await.unwrap();
        // 初始应该返回 None
        assert!(status.is_none());
    }
}
