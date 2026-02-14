//! # SEARCH Operations
//!
//! 处理记忆搜索操作，包括关键词搜索、语义搜索和列表查询。

use std::sync::Arc;

use crate::error::Result;
use crate::memory::ops::MemoryServiceState;
use crate::types::{MemoryCategory, MemoryDomain};

use super::super::{MemoryItem, MemorySearchResult, SearchOptions};

/// SEARCH 操作处理器
///
/// 负责搜索记忆，包括向量语义搜索和过滤。
pub struct SearchOperations {
    state: Arc<MemoryServiceState>,
}

impl SearchOperations {
    /// 创建新的 SEARCH 操作处理器
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self { state }
    }

    /// 搜索记忆
    ///
    /// 使用向量相似度搜索相关记忆，同时搜索私域和公域记忆，
    /// 根据选项进行过滤。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `options`: 搜索选项（域、分类、限制等）
    ///
    /// # 返回
    /// - `Result<Vec<MemoryItem>>`: 搜索结果列表
    pub async fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<MemoryItem>> {
        let limit = options.limit;
        let threshold = options.threshold;

        // 1. 向量搜索获取候选键
        let results = self
            .state
            .vector_storage
            .search_memory(query, limit * 2, Some(threshold))
            .await?;

        let mut items = Vec::new();
        let db = self.state.memory_db.lock().await;

        for result in results {
            // 2. 从数据库获取完整条目
            if let Some(entry) = db.get(&result.key)? {
                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

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
                    if let Some(ref enc) = self.state.encryption {
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

    /// 语义搜索记忆（返回包含相似度的详细结果）
    ///
    /// 执行语义搜索并返回包含相似度分数的详细结果。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `limit`: 返回结果数量限制
    /// - `threshold`: 相似度阈值 (0.0 - 1.0)
    ///
    /// # 返回
    /// - `Result<Vec<MemorySearchResult>>`: 搜索结果列表，按相似度降序排列
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<MemorySearchResult>> {
        // 1. 向量搜索
        let results = self
            .state
            .vector_storage
            .search_memory(query, limit * 2, Some(threshold))
            .await?;

        let mut search_results = Vec::new();
        let db = self.state.memory_db.lock().await;

        for result in results {
            // 2. 从数据库获取完整条目
            if let Some(entry) = db.get(&result.key)? {
                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

                // 3. 解密私域记忆
                let value = if item.encrypted {
                    if let Some(ref enc) = self.state.encryption {
                        enc.decrypt(&item.value)?
                    } else {
                        item.value
                    }
                } else {
                    item.value
                };

                search_results.push(MemorySearchResult {
                    key: item.key.clone(),
                    value,
                    domain: item.domain,
                    category: item.category,
                    similarity: result.similarity,
                    owner: item.owner,
                });
            }
        }

        // 按相似度降序排序并限制数量
        search_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        search_results.truncate(limit);

        Ok(search_results)
    }

    /// 列出所有记忆键
    ///
    /// # 参数
    /// - `domain`: 可选的域过滤
    ///
    /// # 返回
    /// - `Result<Vec<String>>`: 记忆键列表
    pub async fn list_keys(&self, domain: Option<MemoryDomain>) -> Result<Vec<String>> {
        let db = self.state.memory_db.lock().await;

        let prefix = match &self.state.namespace {
            Some(ns) => format!("{}/", ns),
            None => String::new(),
        };

        db.list_keys(&prefix, domain)
    }

    /// 使用过滤器列出记忆
    ///
    /// 根据多个条件过滤记忆并返回完整条目。
    ///
    /// # 参数
    /// - `domain`: 域过滤
    /// - `category`: 分类过滤
    /// - `limit`: 返回数量限制
    ///
    /// # 返回
    /// - `Result<Vec<MemoryItem>>`: 过滤后的记忆列表
    pub async fn list_with_filter(
        &self,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
        limit: Option<usize>,
    ) -> Result<Vec<MemoryItem>> {
        let keys = self.list_keys(domain).await?;

        let mut items = Vec::new();
        let db = self.state.memory_db.lock().await;

        for key in keys {
            if let Some(limit) = limit {
                if items.len() >= limit {
                    break;
                }
            }

            if let Some(entry) = db.get(&key)? {
                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

                // 应用分类过滤
                if let Some(cat) = category {
                    if item.category != cat {
                        continue;
                    }
                }

                // 解密私域记忆
                if item.encrypted {
                    if let Some(ref enc) = self.state.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }

                items.push(item);
            }
        }

        Ok(items)
    }

    /// 统计记忆数量
    ///
    /// # 参数
    /// - `domain`: 可选的域过滤
    ///
    /// # 返回
    /// - `Result<usize>`: 记忆数量
    pub async fn count(&self, domain: Option<MemoryDomain>) -> Result<usize> {
        let keys = self.list_keys(domain).await?;
        Ok(keys.len())
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
    async fn test_list_keys() {
        let (state, _temp) = setup_test_state();
        let ops = SearchOperations::new(state);

        let keys = ops.list_keys(None).await.unwrap();
        // 初始应该为空
        assert_eq!(keys.len(), 0);
    }

    #[tokio::test]
    async fn test_count() {
        let (state, _temp) = setup_test_state();
        let ops = SearchOperations::new(state);

        let count = ops.count(None).await.unwrap();
        assert_eq!(count, 0);

        let public_count = ops.count(Some(MemoryDomain::Public)).await.unwrap();
        assert_eq!(public_count, 0);
    }

    #[tokio::test]
    async fn test_search_with_options() {
        let (state, _temp) = setup_test_state();
        let ops = SearchOperations::new(state);

        let options = SearchOptions::new()
            .with_domain(MemoryDomain::Public)
            .with_category(MemoryCategory::Context)
            .with_limit(10)
            .with_threshold(0.7);

        let results = ops.search("test query", options).await.unwrap();
        // 初始应该为空
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let (state, _temp) = setup_test_state();
        let ops = SearchOperations::new(state);

        let results = ops.semantic_search("查询内容", 5, 0.7).await.unwrap();
        // 初始应该为空
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_list_with_filter() {
        let (state, _temp) = setup_test_state();
        let ops = SearchOperations::new(state);

        let items = ops
            .list_with_filter(Some(MemoryDomain::Public), Some(MemoryCategory::Context), Some(10))
            .await
            .unwrap();

        // 初始应该为空
        assert_eq!(items.len(), 0);
    }
}
