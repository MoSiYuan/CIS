//! # GET Operations
//!
//! Handles memory read operations, including single read, batch read, and domain filtering.

use std::sync::Arc;

use crate::error::Result;
use crate::memory::ops::MemoryServiceState;
use crate::types::{MemoryCategory, MemoryDomain};

use super::super::MemoryItem;
use bincode;

/// GET operation handler
///
/// Responsible for reading memory from database, handling decryption and domain filtering.
pub struct GetOperations {
    state: Arc<MemoryServiceState>,
}

impl GetOperations {
    /// 创建新的 GET 操作处理器
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self { state }
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
        let full_key = self.state.full_key(key);

        // 1. 尝试从缓存读取
        if let Some(cache) = &self.state.cache {
            if let Some(cached_value) = cache.get(key).await {
                tracing::debug!("Cache hit for key: {}", key);
                // 从缓存反序列化
                let item = self.deserialize_cached_item(key, cached_value)?;
                return Ok(Some(item));
            }
        }

        // 2. 缓存未命中，从数据库读取
        let db = self.state.memory_db.lock().await;

        match db.get(&full_key)? {
            Some(entry) => {
                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

                // 如果是加密的私域记忆，解密
                if item.encrypted {
                    if let Some(ref enc) = self.state.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }

                // 3. 更新缓存
                if let Some(cache) = &self.state.cache {
                    let serialized = self.serialize_cached_item(&item);
                    cache.put(key.to_string(), serialized, None).await;
                }

                // 更新向量索引（异步）
                self.spawn_index_update(key, &item.value, &item.category);

                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// 读取指定域的记忆
    ///
    /// 只读取指定域（私域或公域）的记忆。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `domain`: 域（私域或公域）
    ///
    /// # 返回
    /// - `Result<Option<MemoryItem>>`: 成功返回记忆项或 None
    pub async fn get_with_domain(
        &self,
        key: &str,
        domain: MemoryDomain,
    ) -> Result<Option<MemoryItem>> {
        let full_key = self.state.full_key(key);
        let db = self.state.memory_db.lock().await;

        match db.get(&full_key)? {
            Some(entry) => {
                // 检查域是否匹配
                if entry.domain != domain {
                    return Ok(None);
                }

                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

                // 解密私域记忆
                if item.encrypted {
                    if let Some(ref enc) = self.state.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }

                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// 批量读取记忆
    ///
    /// 一次读取多个记忆键，提高效率。
    ///
    /// # 参数
    /// - `keys`: 记忆键列表
    ///
    /// # 返回
    /// - `Result<Vec<Option<MemoryItem>>>`: 返回对应的记忆项列表，不存在的键返回 None
    pub async fn batch_get(&self, keys: &[String]) -> Result<Vec<Option<MemoryItem>>> {
        let mut results = Vec::with_capacity(keys.len());

        for key in keys {
            let item = self.get(key).await?;
            results.push(item);
        }

        Ok(results)
    }

    /// 读取私域记忆
    ///
    /// 只读取私域（加密）记忆。
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<Option<MemoryItem>>`: 成功返回记忆项或 None
    pub async fn get_private(&self, key: &str) -> Result<Option<MemoryItem>> {
        self.get_with_domain(key, MemoryDomain::Private).await
    }

    /// 读取公域记忆
    ///
    /// 只读取公域（明文）记忆。
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<Option<MemoryItem>>`: 成功返回记忆项或 None
    pub async fn get_public(&self, key: &str) -> Result<Option<MemoryItem>> {
        self.get_with_domain(key, MemoryDomain::Public).await
    }

    /// 后台更新向量索引
    fn spawn_index_update(&self, key: &str, value: &[u8], category: &MemoryCategory) {
        // 尝试获取当前运行时
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let storage = Arc::clone(&self.state.vector_storage);
            let key = self.state.full_key(key);
            let value = value.to_vec();
            let category_str = format!("{:?}", category);

            rt.spawn(async move {
                let text = String::from_utf8_lossy(&value);
                if let Err(e) =
                    storage.index_memory(&key, text.as_bytes(), Some(&category_str)).await
                {
                    tracing::warn!("Failed to index memory: {}", e);
                }
            });
        }
    }

    /// 序列化缓存项
    fn serialize_cached_item(&self, item: &MemoryItem) -> Vec<u8> {
        // 使用 bincode 序列化
        bincode::serialize(item).unwrap_or_else(|_| Vec::new())
    }

    /// 反序列化缓存项
    fn deserialize_cached_item(&self, key: &str, data: Vec<u8>) -> Result<MemoryItem> {
        bincode::deserialize(&data)
            .map_err(|e| crate::error::Error::Serialization(e.to_string()))
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
            // 简单的确定性模拟
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
    async fn test_get_basic() {
        let (state, _temp) = setup_test_state();
        let ops = GetOperations::new(state);

        // 需要先设置数据，这里简化测试
        // 实际测试应该通过 SetOperations 设置数据
    }

    #[tokio::test]
    async fn test_get_with_domain() {
        let (state, _temp) = setup_test_state();
        let ops = GetOperations::new(state);

        // 测试域过滤
        // 实际测试应该通过 SetOperations 设置不同域的数据
    }

    #[tokio::test]
    async fn test_batch_get() {
        let (state, _temp) = setup_test_state();
        let ops = GetOperations::new(state);

        let keys = vec
!["key1", "key2", "key3"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let results = ops.batch_get(&keys).await.unwrap();
        assert_eq!(results.len(), 3);
    }
}
