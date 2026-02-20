//! # Hybrid Search Operations
//!
//! v1.2.0: ZeroClaw 式混合搜索（向量语义 + FTS5 关键词）
//!
//! ## 混合搜索算法
//!
//! 1. 计算查询嵌入向量
//! 2. 并行执行向量搜索和关键词搜索
//! 3. 加权融合结果（默认向量 0.7 + 关键词 0.3）
//! 4. 返回排序后的完整记忆条目
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::memory::ops::HybridSearchOperations;
//!
//! let ops = HybridSearchOperations::new(state);
//! let results = ops.hybrid_search("Python 异步编程", 10, None, None).await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;
use crate::memory::ops::MemoryServiceState;
use crate::storage::memory_db::MemoryEntry;
use crate::types::{MemoryCategory, MemoryDomain};

/// 混合搜索结果
///
/// 包含向量搜索分数、关键词搜索分数和最终融合分数。
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    /// 记忆键
    pub key: String,
    /// 记忆值
    pub value: Vec<u8>,
    /// 记忆域
    pub domain: MemoryDomain,
    /// 记忆分类
    pub category: MemoryCategory,
    /// 向量搜索分数 (0.0 - 1.0)，如果未找到则为 None
    pub vector_score: Option<f32>,
    /// 关键词搜索分数 (0.0 - 1.0)，如果未找到则为 None
    pub keyword_score: Option<f32>,
    /// 最终融合分数 (0.0 - 1.0)
    pub final_score: f32,
}

/// 混合搜索配置
///
/// 配置向量搜索和关键词搜索的权重和候选数量。
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// 向量搜索权重（默认 0.7）
    pub vector_weight: f32,
    /// 关键词搜索权重（默认 0.3）
    pub keyword_weight: f32,
    /// 向量搜索候选数倍数（默认 2）
    pub vector_candidate_multiplier: usize,
    /// 关键词搜索候选数倍数（默认 2）
    pub keyword_candidate_multiplier: usize,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.7,
            keyword_weight: 0.3,
            vector_candidate_multiplier: 2,
            keyword_candidate_multiplier: 2,
        }
    }
}

/// 评分键（内部使用）
#[derive(Debug, Clone)]
struct ScoredKey {
    key: String,
    vector_score: Option<f32>,
    keyword_score: Option<f32>,
    final_score: f32,
}

/// 混合搜索操作
///
/// 实现向量语义搜索和 FTS5 关键词搜索的加权融合。
pub struct HybridSearchOperations {
    state: Arc<MemoryServiceState>,
    config: HybridSearchConfig,
}

impl HybridSearchOperations {
    /// 创建新的混合搜索操作
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self {
            state,
            config: HybridSearchConfig::default(),
        }
    }

    /// 使用自定义配置
    pub fn with_config(mut self, config: HybridSearchConfig) -> Self {
        self.config = config;
        self
    }

    /// 混合搜索（向量 + 关键词）
    ///
    /// 执行混合搜索，结合向量语义相似度和关键词 BM25 分数。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `limit`: 返回结果数量限制
    /// - `domain`: 可选的域过滤
    /// - `category`: 可选的分类过滤
    ///
    /// # 返回
    /// - `Result<Vec<HybridSearchResult>>`: 搜索结果列表，按最终分数降序排列
    pub async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
    ) -> Result<Vec<HybridSearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // 步骤 1: 计算查询嵌入向量
        let query_embedding = self.compute_query_embedding(query).await?;

        // 步骤 2: 并行执行向量搜索和关键词搜索
        let vector_limit = limit * self.config.vector_candidate_multiplier;
        let keyword_limit = limit * self.config.keyword_candidate_multiplier;

        let (vector_results, keyword_results) = tokio::join!(
            self.vector_search(&query_embedding, vector_limit, domain, category),
            self.keyword_search(query, keyword_limit, domain)
        );

        let vector_results = vector_results?;
        let keyword_results = keyword_results?;

        // 步骤 3: 加权融合
        let merged = self.hybrid_merge(&vector_results, &keyword_results, limit);

        // 步骤 4: 获取完整记忆内容
        self.fetch_full_entries(merged).await
    }

    /// 计算查询嵌入向量
    async fn compute_query_embedding(&self, query: &str) -> Result<Vec<f32>> {
        self.state.vector_storage.generate_embedding(query).await
    }

    /// 向量搜索（使用 sqlite-vec HNSW 索引）
    async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
    ) -> Result<Vec<(String, f32)>> {
        // 使用 VectorStorage 的语义搜索
        let results = self.state
            .vector_storage
            .search_memory_by_embedding(query_embedding, limit * 2, Some(0.0))
            .await?;

        // 如果没有过滤条件，直接返回
        if domain.is_none() && category.is_none() {
            return Ok(results.into_iter().map(|r| (r.key, r.similarity)).collect());
        }

        // 过滤结果（应用域和分类过滤）
        let mut filtered = Vec::new();
        let memory_db = self.state.memory_db.lock().await;

        for r in results {
            // 从数据库获取条目进行过滤
            if let Ok(Some(entry)) = memory_db.get(&r.key) {
                if let Some(d) = domain {
                    if entry.domain != d {
                        continue;
                    }
                }
                if let Some(c) = category {
                    if entry.category != c {
                        continue;
                    }
                }
                filtered.push((r.key, r.similarity));
            }
        }

        Ok(filtered)
    }

    /// 关键词搜索（使用 FTS5 BM25）
    async fn keyword_search(
        &self,
        query: &str,
        limit: usize,
        domain: Option<MemoryDomain>,
    ) -> Result<Vec<(String, f32)>> {
        let memory_db = self.state.memory_db.lock().await;
        memory_db.fts5_search(query, limit, domain)
    }

    /// 加权融合（ZeroClaw 算法）
    fn hybrid_merge(
        &self,
        vector_results: &[(String, f32)],
        keyword_results: &[(String, f32)],
        limit: usize,
    ) -> Vec<ScoredKey> {
        let mut map: HashMap<String, ScoredKey> = HashMap::new();

        // 添加向量搜索结果
        for (key, score) in vector_results {
            map.entry(key.clone())
                .and_modify(|e| e.vector_score = Some(*score))
                .or_insert_with(|| ScoredKey {
                    key: key.clone(),
                    vector_score: Some(*score),
                    keyword_score: None,
                    final_score: 0.0,
                });
        }

        // 添加关键词搜索结果并归一化
        let max_kw = keyword_results
            .iter()
            .map(|(_, s)| *s)
            .fold(0.0_f32, f32::max)
            .max(f32::EPSILON);

        for (key, score) in keyword_results {
            let normalized = score / max_kw;
            map.entry(key.clone())
                .and_modify(|e| e.keyword_score = Some(normalized))
                .or_insert_with(|| ScoredKey {
                    key: key.clone(),
                    vector_score: None,
                    keyword_score: Some(normalized),
                    final_score: 0.0,
                });
        }

        // 计算最终分数
        let mut results: Vec<ScoredKey> = map
            .into_values()
            .map(|mut sk| {
                let vs = sk.vector_score.unwrap_or(0.0);
                let ks = sk.keyword_score.unwrap_or(0.0);
                sk.final_score = self.config.vector_weight * vs
                    + self.config.keyword_weight * ks;
                sk
            })
            .collect();

        // 排序并截断
        results.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        results
    }

    /// 获取完整记忆条目
    async fn fetch_full_entries(
        &self,
        scored_keys: Vec<ScoredKey>,
    ) -> Result<Vec<HybridSearchResult>> {
        let mut results = Vec::with_capacity(scored_keys.len());

        for scored in scored_keys {
            if let Some(entry) = self.get_memory_entry(&scored.key).await? {
                // 解密私域记忆
                let value = if matches!(entry.domain, MemoryDomain::Private) {
                    if let Some(ref enc) = self.state.encryption {
                        enc.decrypt(&entry.value).unwrap_or_else(|_| entry.value)
                    } else {
                        entry.value
                    }
                } else {
                    entry.value
                };

                results.push(HybridSearchResult {
                    key: scored.key,
                    value,
                    domain: entry.domain,
                    category: entry.category,
                    vector_score: scored.vector_score,
                    keyword_score: scored.keyword_score,
                    final_score: scored.final_score,
                });
            }
        }

        Ok(results)
    }

    /// 获取单个记忆条目
    async fn get_memory_entry(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let memory_db = self.state.memory_db.lock().await;
        memory_db.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_config_default() {
        let config = HybridSearchConfig::default();
        assert_eq!(config.vector_weight, 0.7);
        assert_eq!(config.keyword_weight, 0.3);
        assert_eq!(config.vector_candidate_multiplier, 2);
        assert_eq!(config.keyword_candidate_multiplier, 2);
    }

    #[test]
    fn test_hybrid_merge() {
        let config = HybridSearchConfig::default();
        let ops = HybridSearchOperations {
            state: std::sync::Arc::new(MemoryServiceState::new(
                std::sync::Arc::new(tokio::sync::Mutex::new(
                    tempfile::tempdir().unwrap().path().join("test.db")
                )),
                std::sync::Arc::new(crate::vector::VectorStorage::open_default().unwrap()),
                None,
                "test".to_string(),
                None,
            )),
            config,
        };

        let vector = vec![("a".to_string(), 0.9), ("b".to_string(), 0.7)];
        let keyword = vec![("b".to_string(), 0.8), ("c".to_string(), 0.6)];

        let merged = ops.hybrid_merge(&vector, &keyword, 10);

        // b 应该排在最前面（同时有向量和关键词分数）
        assert_eq!(merged[0].key, "b");
        assert!(merged[0].vector_score.is_some());
        assert!(merged[0].keyword_score.is_some());
        assert!(merged[0].final_score > 0.0);
    }
}
