//! # SET Operations
//!
//! 处理记忆存储操作，包括单个存储、批量存储和域分离。

use std::sync::Arc;

use crate::error::Result;
use crate::memory::ops::MemoryServiceState;
use crate::types::{MemoryCategory, MemoryDomain};

/// SET 操作处理器
///
/// 负责将记忆存储到数据库，处理加密和域分离。
pub struct SetOperations {
    state: Arc<MemoryServiceState>,
}

impl SetOperations {
    /// 创建新的 SET 操作处理器
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self { state }
    }

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
        let full_key = self.state.full_key(key);

        // 1. 写入数据库
        let result = match domain {
            MemoryDomain::Private => self.set_private(&full_key, value, category).await,
            MemoryDomain::Public => self.set_public(&full_key, value, category).await,
        };

        // 2. 使缓存失效 (无论数据库写入是否成功)
        if let Some(cache) = &self.state.cache {
            cache.invalidate(key).await;
        }

        result
    }

    /// 批量存储记忆
    ///
    /// 一次存储多个记忆，提高效率。
    ///
    /// # 参数
    /// - `items`: 记忆项列表 (key, value, domain, category)
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn batch_set(
        &self,
        items: Vec<(String, Vec<u8>, MemoryDomain, MemoryCategory)>,
    ) -> Result<()> {
        for (key, value, domain, category) in items {
            self.set(&key, &value, domain, category).await?;
        }
        Ok(())
    }

    /// 存储私域记忆
    ///
    /// 私域记忆会被加密存储，永不同步。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `category`: 分类
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn set_private(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 加密数据
        let encrypted = if let Some(ref enc) = self.state.encryption {
            enc.encrypt(value)?
        } else {
            // 无密钥时存储明文，但标记为私域
            value.to_vec()
        };

        // 2. 存储到 memory_db
        let db = self.state.memory_db.lock().await;
        db.set_private(key, &encrypted, category)?;

        // 3. 更新向量索引（使用原始值）
        self.spawn_index_update(key, value, &category);

        Ok(())
    }

    /// 存储公域记忆
    ///
    /// 公域记忆明文存储，可以 P2P 同步。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `category`: 分类
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn set_public(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 明文存储
        let db = self.state.memory_db.lock().await;
        db.set_public(key, value, category)?;

        // 2. 更新向量索引
        self.spawn_index_update(key, value, &category);

        // 3. 标记为待同步（P2P）- 已由 MemoryDb 自动处理

        Ok(())
    }

    /// 存储记忆并建立向量索引
    ///
    /// 存储记忆的同时，立即建立向量索引以便语义搜索。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值
    /// - `domain`: 私域或公域
    /// - `category`: 分类
    ///
    /// # 返回
    /// - `Result<()>`: 成功返回 Ok，失败返回错误
    pub async fn set_with_embedding(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        let full_key = self.state.full_key(key);
        let category_str = format!("{:?}", category);

        // 1. 存储到数据库
        match domain {
            MemoryDomain::Private => self.set_private(&full_key, value, category).await?,
            MemoryDomain::Public => self.set_public(&full_key, value, category).await?,
        }

        // 2. 同步建立向量索引（等待完成）
        let text = String::from_utf8_lossy(value);
        self.state
            .vector_storage
            .index_memory(&full_key, text.as_bytes(), Some(&category_str))
            .await?;

        Ok(())
    }

    /// 删除记忆
    ///
    /// # 参数
    /// - `key`: 记忆键
    ///
    /// # 返回
    /// - `Result<bool>`: 成功返回是否删除，失败返回错误
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let full_key = self.state.full_key(key);
        let db = self.state.memory_db.lock().await;

        let deleted = db.delete(&full_key)?;

        if deleted {
            // 从向量索引中移除
            let _ = self.state.vector_storage.delete_memory_index(&full_key);

            // 从缓存中移除
            if let Some(cache) = &self.state.cache {
                cache.invalidate(key).await;
            }
        }

        Ok(deleted)
    }

    /// 后台更新向量索引
    fn spawn_index_update(&self, key: &str, value: &[u8], category: &MemoryCategory) {
        // 尝试获取当前运行时
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
    async fn test_set_private() {
        let (state, _temp) = setup_test_state();
        let ops = SetOperations::new(state);

        ops.set_private("private_key", b"private_value", MemoryCategory::Context)
            .await
            .unwrap();

        // 验证存储成功
        // 实际测试应该通过 GetOperations 读取验证
    }

    #[tokio::test]
    async fn test_set_public() {
        let (state, _temp) = setup_test_state();
        let ops = SetOperations::new(state);

        ops.set_public("public_key", b"public_value", MemoryCategory::Result)
            .await
            .unwrap();

        // 验证存储成功
    }

    #[tokio::test]
    async fn test_set_with_encryption() {
        let temp_dir = tempfile::tempdir().unwrap();

        let db_path = temp_dir.path().join("memory.db");
        let memory_db = MemoryDb::open(&db_path).unwrap();

        let vector_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();

        let encryption = MemoryEncryption::from_node_key(b"test-key");

        let state = MemoryServiceState::new(
            Arc::new(tokio::sync::Mutex::new(memory_db)),
            Arc::new(vector_storage),
            Some(encryption),
            "test-node".to_string(),
            None,
        );

        let ops = SetOperations::new(Arc::new(state));

        ops.set_private("encrypted_key", b"secret_value", MemoryCategory::Context)
            .await
            .unwrap();

        // 验证加密存储成功
    }

    #[tokio::test]
    async fn test_batch_set() {
        let (state, _temp) = setup_test_state();
        let ops = SetOperations::new(state);

        let items = vec
![
            ("key1".to_string(), b"value1".to_vec(), MemoryDomain::Public, MemoryCategory::Context),
            ("key2".to_string(), b"value2".to_vec(), MemoryDomain::Private, MemoryCategory::Result),
        ];

        ops.batch_set(items).await.unwrap();

        // 验证批量存储成功
    }

    #[tokio::test]
    async fn test_delete() {
        let (state, _temp) = setup_test_state();
        let ops = SetOperations::new(state);

        ops.set_public("to_delete", b"value", MemoryCategory::Context)
            .await
            .unwrap();

        let deleted = ops.delete("to_delete").await.unwrap();
        assert!(deleted);

        let deleted_again = ops.delete("to_delete").await.unwrap();
        assert!(!deleted_again);
    }
}
