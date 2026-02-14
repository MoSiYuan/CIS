//! # 向量搜索结果合并器
//!
//! 合并来自多个搜索源的结果，去重并排序。
//!
//! ## 功能
//!
//! - 合并 HNSW 和 SQLite 搜索结果
//! - 去重（保留最高分）
//! - 按相似度排序
//! - 支持多种合并策略
//!
//! ## 合并策略
//!
//! | 策略 | 说明 | 适用场景 |
//! |------|------|---------|
//! | `Union` | 并集，去重后排序 | 最大化召回率 |
//! | `Intersect` | 交集，仅公共结果 | 最大化精确率 |
//! | `Weighted` | 加权合并 | 混合多个算法 |
//! | `Rrf` | Reciprocal Rank Fusion | 综合排序 |
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::vector::merger::{ResultMerger, SearchResult, MergeStrategy};
//!
//! # fn example() -> anyhow::Result<()> {
//! let mut merger = ResultMerger::new();
//!
//! // 合并 HNSW 和 SQLite 结果
//! let hnsw_results = vec![
//!     SearchResult { id: "id1".into(), score: 0.95, ..Default::default() },
//!     SearchResult { id: "id2".into(), score: 0.85, ..Default::default() },
//! ];
//!
//! let sqlite_results = vec![
//!     SearchResult { id: "id1".into(), score: 0.90, ..Default::default() },
//!     SearchResult { id: "id3".into(), score: 0.80, ..Default::default() },
//! ];
//!
//! let merged = merger.merge(hnsw_results, sqlite_results, MergeStrategy::Union, 10)?;
//! assert_eq!(merged.len(), 3); // id1, id2, id3 (id1 去重)
//! assert_eq!(merged[0].id, "id1"); // 最高分 0.95
//!
//! # Ok(())
//! # }
//! ```

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::error::{CisError, Result};

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 结果 ID
    pub id: String,

    /// 相似度分数 (0.0-1.0)
    pub score: f32,

    /// 来源 (HNSW/SQLite/Hybrid)
    pub source: SearchSource,

    /// 原始排名（用于 RRF）
    pub original_rank: Option<usize>,

    /// 额外元数据
    #[serde(skip)]
    pub metadata: Option<serde_json::Value>,
}

impl SearchResult {
    /// 创建新的搜索结果
    pub fn new(id: String, score: f32, source: SearchSource) -> Self {
        Self {
            id,
            score,
            source,
            original_rank: None,
            metadata: None,
        }
    }

    /// 设置原始排名
    pub fn with_rank(mut self, rank: usize) -> Self {
        self.original_rank = Some(rank);
        self
    }

    /// 设置元数据
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            id: String::new(),
            score: 0.0,
            source: SearchSource::Unknown,
            original_rank: None,
            metadata: None,
        }
    }
}

/// 搜索来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchSource {
    /// HNSW 索引
    HNSW,

    /// SQLite 全文搜索
    SQLite,

    /// 混合结果
    Hybrid,

    /// 未知来源
    Unknown,
}

/// 合并策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// 并集：合并所有结果，去重，按分数排序
    Union,

    /// 交集：仅保留所有源都包含的结果
    Intersect,

    /// 加权合并：为不同源的分数应用权重
    Weighted {
        /// HNSW 权重
        hnsw_weight: f32,
        /// SQLite 权重
        sqlite_weight: f32,
    },

    /// Reciprocal Rank Fusion (RRF)
    Rrf {
        /// RRF 参数 K (默认 60)
        k: i32,
    },
}

/// 合并统计信息
#[derive(Debug, Clone)]
pub struct MergeStats {
    /// 第一个源的结果数
    pub source1_count: usize,

    /// 第二个源的结果数
    pub source2_count: usize,

    /// 合并后的结果数
    pub merged_count: usize,

    /// 去重数量
    pub deduped_count: usize,

    /// 合并策略
    pub strategy: MergeStrategy,
}

/// 结果合并器
///
/// 合并来自多个搜索源的结果。
pub struct ResultMerger {
    /// 是否保留来源信息
    preserve_source: bool,

    /// 是否记录统计信息
    track_stats: bool,

    /// 统计信息（最近一次合并）
    last_stats: Option<MergeStats>,
}

impl ResultMerger {
    /// 创建新的合并器
    pub fn new() -> Self {
        Self {
            preserve_source: true,
            track_stats: true,
            last_stats: None,
        }
    }

    /// 配置是否保留来源信息
    pub fn with_preserve_source(mut self, preserve: bool) -> Self {
        self.preserve_source = preserve;
        self
    }

    /// 配置是否记录统计信息
    pub fn with_track_stats(mut self, track: bool) -> Self {
        self.track_stats = track;
        self
    }

    /// 合并两组搜索结果
    ///
    /// # 参数
    /// - `results1`: 第一组结果
    /// - `results2`: 第二组结果
    /// - `strategy`: 合并策略
    /// - `top_k`: 返回前 K 个结果
    ///
    /// # 返回
    /// - `Result<Vec<SearchResult>>`: 合并后的结果
    pub fn merge(
        &mut self,
        results1: Vec<SearchResult>,
        results2: Vec<SearchResult>,
        strategy: MergeStrategy,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        let (merged, stats) = match strategy {
            MergeStrategy::Union => {
                self.merge_union(results1, results2, top_k)?
            }

            MergeStrategy::Intersect => {
                self.merge_intersect(results1, results2, top_k)?
            }

            MergeStrategy::Weighted { hnsw_weight, sqlite_weight } => {
                self.merge_weighted(results1, results2, hnsw_weight, sqlite_weight, top_k)?
            }

            MergeStrategy::Rrf { k } => {
                self.merge_rrf(results1, results2, k, top_k)?
            }
        };

        if self.track_stats {
            self.last_stats = Some(stats);
        }

        Ok(merged)
    }

    /// 并集合并（默认策略）
    ///
    /// 合并所有结果，去重（保留最高分），按分数排序。
    fn merge_union(
        &self,
        mut results1: Vec<SearchResult>,
        mut results2: Vec<SearchResult>,
        top_k: usize,
    ) -> Result<(Vec<SearchResult>, MergeStats)> {
        let source1_count = results1.len();
        let source2_count = results2.len();

        // 合并结果
        results1.append(&mut results2);

        // 去重：保留每个 ID 的最高分
        let mut score_map: HashMap<String, SearchResult> = HashMap::new();

        for result in results1 {
            score_map
                .entry(result.id.clone())
                .and_modify(|existing| {
                    if result.score > existing.score {
                        *existing = result;
                    }
                })
                .or_insert(result);
        }

        let deduped_count = source1_count + source2_count - score_map.len();

        // 转换为 Vec 并排序
        let mut merged: Vec<_> = score_map.into_values().collect();
        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // 取 top_k
        merged.truncate(top_k);

        // 标记来源
        let merged = if self.preserve_source {
            merged
                .into_iter()
                .map(|mut r| {
                    r.source = SearchSource::Hybrid;
                    r
                })
                .collect()
        } else {
            merged
        };

        let stats = MergeStats {
            source1_count,
            source2_count,
            merged_count: merged.len(),
            deduped_count,
            strategy: MergeStrategy::Union,
        };

        Ok((merged, stats))
    }

    /// 交集合并
    ///
    /// 仅保留两个源都包含的结果。
    fn merge_intersect(
        &self,
        results1: Vec<SearchResult>,
        results2: Vec<SearchResult>,
        top_k: usize,
    ) -> Result<(Vec<SearchResult>, MergeStats)> {
        let source1_count = results1.len();
        let source2_count = results2.len();

        // 构建 ID 集合
        let ids1: HashSet<_> = results1.iter().map(|r| r.id.as_str()).collect();
        let ids2: HashSet<_> = results2.iter().map(|r| r.id.as_str()).collect();

        // 计算交集
        let intersect_ids: HashSet<_> = ids1.intersection(&ids2).cloned().collect();

        // 合并交集结果（取最高分）
        let mut merged = Vec::new();

        for id in intersect_ids {
            let r1 = results1.iter().find(|r| r.id == id).unwrap();
            let r2 = results2.iter().find(|r| r.id == id).unwrap();

            let score = r1.score.max(r2.score);
            merged.push(SearchResult::new(
                id.to_string(),
                score,
                SearchSource::Hybrid,
            ));
        }

        // 排序并取 top_k
        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        merged.truncate(top_k);

        let stats = MergeStats {
            source1_count,
            source2_count,
            merged_count: merged.len(),
            deduped_count: 0,
            strategy: MergeStrategy::Intersect,
        };

        Ok((merged, stats))
    }

    /// 加权合并
    ///
    /// 为不同源的分数应用权重后合并。
    fn merge_weighted(
        &self,
        results1: Vec<SearchResult>,
        results2: Vec<SearchResult>,
        weight1: f32,
        weight2: f32,
        top_k: usize,
    ) -> Result<(Vec<SearchResult>, MergeStats)> {
        let source1_count = results1.len();
        let source2_count = results2.len();

        // 归一化权重
        let total_weight = weight1 + weight2;
        let w1 = weight1 / total_weight;
        let w2 = weight2 / total_weight;

        // 构建分数映射
        let mut score_map: HashMap<String, (f32, SearchSource)> = HashMap::new();

        // 处理第一组结果
        for result in results1 {
            score_map
                .entry(result.id.clone())
                .and_modify(|(score, _)| {
                    *score = (*score + result.score * w1).max(0.0).min(1.0);
                })
                .or_insert((result.score * w1, result.source));
        }

        // 处理第二组结果
        for result in results2 {
            score_map
                .entry(result.id.clone())
                .and_modify(|(score, _)| {
                    *score = (*score + result.score * w2).max(0.0).min(1.0);
                })
                .or_insert((result.score * w2, result.source));
        }

        // 转换并排序
        let mut merged: Vec<_> = score_map
            .into_iter()
            .map(|(id, tuple)| {
                let (score, source) = tuple;
                SearchResult::new(id, score, source)
            })
            .collect();

        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        merged.truncate(top_k);

        let stats = MergeStats {
            source1_count,
            source2_count,
            merged_count: merged.len(),
            deduped_count: 0,
            strategy: MergeStrategy::Weighted {
                hnsw_weight: weight1,
                sqlite_weight: weight2,
            },
        };

        Ok((merged, stats))
    }

    /// Reciprocal Rank Fusion (RRF)
    ///
    /// 根据排名而非分数合并结果，适用于分数不可比的场景。
    ///
    /// 公式: score = Σ 1/(k + rank_i)
    fn merge_rrf(
        &self,
        results1: Vec<SearchResult>,
        results2: Vec<SearchResult>,
        k: i32,
        top_k: usize,
    ) -> Result<(Vec<SearchResult>, MergeStats)> {
        let source1_count = results1.len();
        let source2_count = results2.len();

        // 确保 results 已按原始分数排序
        let mut sorted1 = results1;
        let mut sorted2 = results2;

        sorted1.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        sorted2.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // 为每个结果添加排名
        let ranked1: Vec<_> = sorted1
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), 1.0 / (k as f32 + i as f32 + 1.0)))
            .collect();

        let ranked2: Vec<_> = sorted2
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), 1.0 / (k as f32 + i as f32 + 1.0)))
            .collect();

        // 合并 RRF 分数
        let mut rrf_map: HashMap<String, f32> = HashMap::new();

        for (id, score) in ranked1.into_iter().chain(ranked2.into_iter()) {
            rrf_map.entry(id).and_modify(|s| *s += score).or_insert(score);
        }

        // 转换并排序
        let mut merged: Vec<_> = rrf_map
            .into_iter()
            .map(|(id, score)| SearchResult::new(id, score, SearchSource::Hybrid))
            .collect();

        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        merged.truncate(top_k);

        let stats = MergeStats {
            source1_count,
            source2_count,
            merged_count: merged.len(),
            deduped_count: 0,
            strategy: MergeStrategy::Rrf { k },
        };

        Ok((merged, stats))
    }

    /// 获取最近一次合并的统计信息
    pub fn last_stats(&self) -> Option<&MergeStats> {
        self.last_stats.as_ref()
    }

    /// 批量合并多个结果集
    ///
    /// # 参数
    /// - `results`: 多个结果集
    /// - `strategy`: 合并策略
    /// - `top_k`: 返回前 K 个结果
    ///
    /// # 返回
    /// - `Result<Vec<SearchResult>>`: 合并后的结果
    pub fn merge_multiple(
        &mut self,
        mut results: Vec<Vec<SearchResult>>,
        strategy: MergeStrategy,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        if results.is_empty() {
            return Ok(Vec::new());
        }

        if results.len() == 1 {
            return Ok(results.pop().unwrap());
        }

        // 迭代合并
        let mut merged = results.pop().unwrap();

        while let Some(next) = results.pop() {
            merged = self.merge(merged, next, strategy, top_k)?;
        }

        Ok(merged)
    }

    /// 验证结果有效性
    ///
    /// 检查结果是否符合基本约束。
    pub fn validate_results(results: &[SearchResult]) -> Result<()> {
        for (i, result) in results.iter().enumerate() {
            // 检查分数范围
            if result.score < 0.0 || result.score > 1.0 {
                return Err(CisError::other(format!(
                    "Invalid score at index {}: {} (expected 0.0-1.0)",
                    i, result.score
                )));
            }

            // 检查 ID 非空
            if result.id.is_empty() {
                return Err(CisError::other(format!(
                    "Empty ID at index {}",
                    i
                )));
            }
        }

        Ok(())
    }
}

impl Default for ResultMerger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_result(id: &str, score: f32) -> SearchResult {
        SearchResult::new(id.to_string(), score, SearchSource::HNSW)
    }

    #[test]
    fn test_merge_union() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.95),
            create_result("id2", 0.85),
            create_result("id3", 0.75),
        ];

        let results2 = vec![
            create_result("id1", 0.90),
            create_result("id4", 0.80),
        ];

        let merged = merger
            .merge(results1, results2, MergeStrategy::Union, 10)
            .unwrap();

        assert_eq!(merged.len(), 4); // id1, id2, id3, id4 (id1 去重)
        assert_eq!(merged[0].id, "id1");
        assert_eq!(merged[0].score, 0.95); // 保留最高分
    }

    #[test]
    fn test_merge_intersect() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.95),
            create_result("id2", 0.85),
            create_result("id3", 0.75),
        ];

        let results2 = vec![
            create_result("id1", 0.90),
            create_result("id4", 0.80),
        ];

        let merged = merger
            .merge(results1, results2, MergeStrategy::Intersect, 10)
            .unwrap();

        assert_eq!(merged.len(), 1); // 只有 id1 在两个结果集中
        assert_eq!(merged[0].id, "id1");
        assert_eq!(merged[0].score, 0.95); // 保留最高分
    }

    #[test]
    fn test_merge_weighted() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.80),
            create_result("id2", 0.60),
        ];

        let results2 = vec![
            create_result("id1", 0.40),
            create_result("id3", 0.90),
        ];

        let merged = merger
            .merge(
                results1,
                results2,
                MergeStrategy::Weighted {
                    hnsw_weight: 0.7,
                    sqlite_weight: 0.3,
                },
                10,
            )
            .unwrap();

        // id1: 0.8 * 0.7 + 0.4 * 0.3 = 0.56 + 0.12 = 0.68
        assert_eq!(merged.len(), 3);

        let id1_result = merged.iter().find(|r| r.id == "id1").unwrap();
        assert!((id1_result.score - 0.68).abs() < 0.01);
    }

    #[test]
    fn test_merge_rrf() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.95), // rank 0
            create_result("id2", 0.85), // rank 1
        ];

        let results2 = vec![
            create_result("id3", 0.90), // rank 0
            create_result("id1", 0.80), // rank 1
        ];

        let merged = merger
            .merge(results1, results2, MergeStrategy::Rrf { k: 60 }, 10)
            .unwrap();

        // id1: 1/(60+0+1) + 1/(60+1+1) ≈ 0.0164 + 0.0161 = 0.0325
        let id1_result = merged.iter().find(|r| r.id == "id1").unwrap();
        assert!((id1_result.score - 0.0325).abs() < 0.001);
    }

    #[test]
    fn test_top_k() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.95),
            create_result("id2", 0.85),
            create_result("id3", 0.75),
        ];

        let results2 = vec![
            create_result("id4", 0.90),
            create_result("id5", 0.80),
        ];

        let merged = merger
            .merge(results1, results2, MergeStrategy::Union, 2)
            .unwrap();

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, "id1");
        assert_eq!(merged[1].id, "id4");
    }

    #[test]
    fn test_merge_stats() {
        let mut merger = ResultMerger::new();

        let results1 = vec![
            create_result("id1", 0.95),
            create_result("id2", 0.85),
        ];

        let results2 = vec![
            create_result("id1", 0.90),
            create_result("id3", 0.80),
        ];

        merger
            .merge(results1, results2, MergeStrategy::Union, 10)
            .unwrap();

        let stats = merger.last_stats().unwrap();
        assert_eq!(stats.source1_count, 2);
        assert_eq!(stats.source2_count, 2);
        assert_eq!(stats.merged_count, 3);
        assert_eq!(stats.deduped_count, 1);
    }

    #[test]
    fn test_validate_results() {
        // 有效结果
        let results = vec![
            create_result("id1", 0.95),
            create_result("id2", 0.85),
        ];

        assert!(ResultMerger::validate_results(&results).is_ok());

        // 无效分数
        let invalid = vec![create_result("id1", 1.5)];
        assert!(ResultMerger::validate_results(&invalid).is_err());

        // 空 ID
        let invalid = vec![SearchResult {
            id: String::new(),
            score: 0.5,
            source: SearchSource::Unknown,
            original_rank: None,
            metadata: None,
        }];
        assert!(ResultMerger::validate_results(&invalid).is_err());
    }

    #[test]
    fn test_merge_multiple() {
        let mut merger = ResultMerger::new();

        let results1 = vec![create_result("id1", 0.95)];
        let results2 = vec![create_result("id2", 0.85)];
        let results3 = vec![create_result("id3", 0.75)];

        let merged = merger
            .merge_multiple(vec![results1, results2, results3], MergeStrategy::Union, 10)
            .unwrap();

        assert_eq!(merged.len(), 3);
    }
}
