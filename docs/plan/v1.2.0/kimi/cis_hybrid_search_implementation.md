# CIS 实现 ZeroClaw 式混合搜索完整方案

## 📋 概述

本方案指导如何在 CIS 中实现类似 ZeroClaw 的混合搜索（向量 + 关键词），结合 CIS 的 sqlite-vec 高性能向量索引和 ZeroClaw 的加权融合策略。

---

## 1. 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                  CIS Hybrid Search Module                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐   │
│  │              HybridSearchOperations                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │   │
│  │  │   Vector    │  │  Keyword    │  │   Hybrid    │ │   │
│  │  │   Search    │  │   Search    │  │    Merge    │ │   │
│  │  │(sqlite-vec) │  │  (FTS5)     │  │  (Weighted) │ │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘ │   │
│  └─────────┼────────────────┼────────────────┼────────┘   │
│            └────────────────┴────────────────┘             │
│                            │                                 │
│  ┌─────────────────────────▼────────────────────────────┐   │
│  │              MemoryServiceState (Arc)                 │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │   │
│  │  │  MemoryDb    │  │ VectorStorage│  │FTS5 Index │  │   │
│  │  │  (SQLite)    │  │(sqlite-vec)  │  │(memories_fts)│   │
│  │  └──────────────┘  └──────────────┘  └───────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 数据库 Schema 调整

### 2.1 添加 FTS5 虚拟表

```rust
// 文件: cis-core/src/storage/memory_db.rs

impl MemoryDb {
    /// 初始化 Schema（添加 FTS5 支持）
    fn init_schema(&self) -> Result<()> {
        // 原有表结构...

        // ============================================
        // 新增: FTS5 全文搜索虚拟表（ZeroClaw 式混合搜索）
        // ============================================
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                key,           -- 记忆键
                content,       -- 记忆内容（文本）
                content=memory_index,  -- 关联到主表
                content_rowid=rowid
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create FTS5 table: {}", e)))?;

        // FTS5 同步触发器
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS fts_memory_insert AFTER INSERT ON memory_index
             BEGIN
                 INSERT INTO memories_fts(rowid, key, content)
                 VALUES (new.rowid, new.key, COALESCE(
                     (SELECT content FROM private_entries WHERE key = new.key),
                     (SELECT content FROM public_entries WHERE key = new.key),
                     ''
                 ));
             END",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create FTS trigger: {}", e)))?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS fts_memory_delete AFTER DELETE ON memory_index
             BEGIN
                 INSERT INTO memories_fts(memories_fts, rowid, key, content)
                 VALUES ('delete', old.rowid, old.key, '');
             END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS fts_memory_update AFTER UPDATE ON memory_index
             BEGIN
                 INSERT INTO memories_fts(memories_fts, rowid, key, content)
                 VALUES ('delete', old.rowid, old.key, '');
                 INSERT INTO memories_fts(rowid, key, content)
                 VALUES (new.rowid, new.key, COALESCE(
                     (SELECT content FROM private_entries WHERE key = new.key),
                     (SELECT content FROM public_entries WHERE key = new.key),
                     ''
                 ));
             END",
            [],
        )?;

        Ok(())
    }

    /// FTS5 关键词搜索（BM25 评分）
    pub fn fts5_search(
        &self,
        query: &str,
        limit: usize,
        domain: Option<MemoryDomain>,
    ) -> Result<Vec<(String, f32)>> {
        // 转义 FTS5 特殊字符
        let fts_query: String = query
            .split_whitespace()
            .map(|w| format!("\"{}\"", w.replace('"', """")))
            .collect::<Vec<_>>()
            .join(" OR ");

        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        let sql = format!(
            "SELECT m.key, bm25(memories_fts) as score
             FROM memories_fts f
             JOIN memory_index m ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?1
               AND (?2 IS NULL OR m.domain = ?2)
             ORDER BY score
             LIMIT ?3"
        );

        let domain_str = domain.map(|d| format!("{:?}", d));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![fts_query, domain_str, limit as i64],
            |row| {
                let key: String = row.get(0)?;
                let score: f64 = row.get(1)?;
                // BM25 返回负分数（越小越好），取反后归一化
                Ok((key, (-score as f32).max(0.0).min(1.0)))
            },
        )?;

        rows.filter_map(|r| r.ok()).collect()
    }
}
```

---

## 3. 混合搜索核心实现

### 3.1 混合搜索结果结构

```rust
// 文件: cis-core/src/memory/ops/hybrid_search.rs

use std::collections::HashMap;

/// 混合搜索结果
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub vector_score: Option<f32>,
    pub keyword_score: Option<f32>,
    pub final_score: f32,
}

/// 搜索配置
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// 向量搜索权重（默认 0.7）
    pub vector_weight: f32,
    /// 关键词搜索权重（默认 0.3）
    pub keyword_weight: f32,
    /// 向量搜索候选数倍数
    pub vector_candidate_multiplier: usize,
    /// 关键词搜索候选数倍数
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
```

### 3.2 混合搜索操作实现

```rust
// 文件: cis-core/src/memory/ops/hybrid_search.rs

use crate::memory::ops::MemoryServiceState;
use crate::types::{MemoryDomain, MemoryCategory};
use std::sync::Arc;

/// 混合搜索操作
pub struct HybridSearchOperations {
    state: Arc<MemoryServiceState>,
    config: HybridSearchConfig,
}

impl HybridSearchOperations {
    pub fn new(state: Arc<MemoryServiceState>) -> Self {
        Self {
            state,
            config: HybridSearchConfig::default(),
        }
    }

    pub fn with_config(mut self, config: HybridSearchConfig) -> Self {
        self.config = config;
        self
    }

    /// 混合搜索（向量 + 关键词）
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

        // ============================================
        // 步骤 1: 计算查询嵌入向量
        // ============================================
        let query_embedding = self.compute_query_embedding(query).await?;

        // ============================================
        // 步骤 2: 并行执行向量搜索和关键词搜索
        // ============================================
        let vector_limit = limit * self.config.vector_candidate_multiplier;
        let keyword_limit = limit * self.config.keyword_candidate_multiplier;

        let (vector_results, keyword_results) = tokio::join!(
            self.vector_search(&query_embedding, vector_limit, domain, category),
            self.keyword_search(query, keyword_limit, domain)
        );

        let vector_results = vector_results?;
        let keyword_results = keyword_results?;

        // ============================================
        // 步骤 3: 加权融合
        // ============================================
        let merged = self.hybrid_merge(
            &vector_results,
            &keyword_results,
            limit,
        );

        // ============================================
        // 步骤 4: 获取完整记忆内容
        // ============================================
        let results = self.fetch_full_entries(merged).await?;

        Ok(results)
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
        let results = self.state.vector_storage
            .search_raw(query_embedding, limit, domain, category)
            .await?;

        Ok(results.into_iter()
            .map(|r| (r.key, r.similarity))
            .collect())
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

        // 归一化向量分数（已经是 0-1）
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

        // 归一化关键词分数（BM25 可能是任意正数）
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
                results.push(HybridSearchResult {
                    key: scored.key,
                    value: entry.value,
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

/// 评分键（内部使用）
#[derive(Debug, Clone)]
struct ScoredKey {
    key: String,
    vector_score: Option<f32>,
    keyword_score: Option<f32>,
    final_score: f32,
}
```

---

## 4. VectorStorage 扩展

### 4.1 添加原始搜索方法

```rust
// 文件: cis-core/src/vector/storage.rs

impl VectorStorage {
    /// 原始向量搜索（返回键和相似度）
    pub async fn search_raw(
        &self,
        query_embedding: &[f32],
        limit: usize,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
    ) -> Result<Vec<RawSearchResult>> {
        let embedding_bytes = embedding_to_bytes(query_embedding);

        let conn = self.conn.lock().unwrap();

        // 构建查询
        let mut sql = String::from(
            "SELECT key, vec_distance_cosine(embedding, ?1) as distance
             FROM memory_vectors
             WHERE embedding IS NOT NULL"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(embedding_bytes)
        ];

        // 添加过滤条件
        if let Some(d) = domain {
            sql.push_str(" AND domain = ?");
            params.push(Box::new(format!("{:?}", d)));
        }

        if let Some(c) = category {
            sql.push_str(" AND category = ?");
            params.push(Box::new(format!("{:?}", c)));
        }

        sql.push_str(" ORDER BY distance LIMIT ?");
        params.push(Box::new(limit as i64));

        // 执行查询
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|p| p.as_ref())
            .collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(RawSearchResult {
                key: row.get(0)?,
                similarity: 1.0 - row.get::<_, f32>(1)?,  // 距离转相似度
            })
        })?;

        rows.filter_map(|r| r.ok()).collect()
    }
}

/// 原始搜索结果
#[derive(Debug, Clone)]
pub struct RawSearchResult {
    pub key: String,
    pub similarity: f32,
}
```

---

## 5. MemoryService 集成

### 5.1 扩展 MemoryService

```rust
// 文件: cis-core/src/memory/service.rs

pub struct MemoryService {
    state: Arc<MemoryServiceState>,
    get_ops: GetOperations,
    set_ops: SetOperations,
    search_ops: SearchOperations,
    sync_ops: SyncOperations,
    // 新增: 混合搜索操作
    hybrid_ops: HybridSearchOperations,
}

impl MemoryService {
    pub fn new(
        memory_db: Arc<Mutex<MemoryDb>>,
        vector_storage: Arc<VectorStorage>,
        node_id: impl Into<String>,
    ) -> Result<Self> {
        let state = Arc::new(MemoryServiceState::new(
            memory_db,
            vector_storage,
            None,
            node_id.into(),
            None,
        ));

        let hybrid_ops = HybridSearchOperations::new(Arc::clone(&state));

        Ok(Self {
            state,
            get_ops: GetOperations::new(Arc::clone(&state)),
            set_ops: SetOperations::new(Arc::clone(&state)),
            search_ops: SearchOperations::new(Arc::clone(&state)),
            sync_ops: SyncOperations::new(Arc::clone(&state)),
            hybrid_ops,
        })
    }

    /// 混合搜索（向量 + 关键词）
    pub async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
    ) -> Result<Vec<HybridSearchResult>> {
        let mut results = self.hybrid_ops
            .hybrid_search(query, limit, domain, category)
            .await?;

        // 应用阈值过滤
        results.retain(|r| r.final_score >= threshold);

        Ok(results)
    }

    /// 配置混合搜索权重
    pub fn with_hybrid_config(mut self, config: HybridSearchConfig) -> Self {
        self.hybrid_ops = HybridSearchOperations::new(Arc::clone(&self.state))
            .with_config(config);
        self
    }
}
```

---

## 6. 配置示例

### 6.1 默认配置（ZeroClaw 风格）

```rust
use cis_core::memory::{MemoryService, HybridSearchConfig};

// 创建服务（使用默认混合搜索配置）
let service = MemoryService::open_default("node-1")?;

// 执行混合搜索
let results = service.hybrid_search(
    "Python 异步编程",
    10,           // limit
    0.6,          // threshold
    None,         // domain (不限)
    None,         // category (不限)
).await?;

for result in results {
    println!(
        "{}: final={:.2}, vector={:.2}, keyword={:.2}",
        result.key,
        result.final_score,
        result.vector_score.unwrap_or(0.0),
        result.keyword_score.unwrap_or(0.0)
    );
}
```

### 6.2 自定义权重配置

```rust
// 创建自定义配置（更侧重关键词）
let config = HybridSearchConfig {
    vector_weight: 0.5,
    keyword_weight: 0.5,
    vector_candidate_multiplier: 3,
    keyword_candidate_multiplier: 3,
};

let service = MemoryService::open_default("node-1")?
    .with_hybrid_config(config);
```

---

## 7. 性能优化建议

### 7.1 索引优化

```sql
-- 确保 FTS5 索引已优化
INSERT INTO memories_fts(memories_fts) VALUES('optimize');

-- 定期重建 FTS5 索引（每周）
INSERT INTO memories_fts(memories_fts) VALUES('rebuild');
```

### 7.2 缓存策略

```rust
/// 嵌入缓存（避免重复计算查询嵌入）
pub struct EmbeddingCache {
    cache: DashMap<String, (Vec<f32>, Instant)>,
    ttl: Duration,
}

impl EmbeddingCache {
    pub async fn get_or_compute(&self, query: &str) -> Result<Vec<f32>> {
        if let Some((embedding, ts)) = self.cache.get(query) {
            if ts.elapsed() < self.ttl {
                return Ok(embedding.clone());
            }
        }

        let embedding = self.compute_embedding(query).await?;
        self.cache.insert(query.to_string(), (embedding.clone(), Instant::now()));

        Ok(embedding)
    }
}
```

### 7.3 并行化

```rust
// 向量搜索和关键词搜索并行执行
let (vector_results, keyword_results) = tokio::join!(
    self.vector_search(...),
    self.keyword_search(...)
);
```

---

## 8. 测试用例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hybrid_search() {
        let service = create_test_service().await;

        // 索引测试数据
        service.set("rust/async", b"Rust async/await programming", 
                   MemoryDomain::Public, MemoryCategory::Technical).await.unwrap();
        service.set("python/async", b"Python asyncio programming",
                   MemoryDomain::Public, MemoryCategory::Technical).await.unwrap();

        // 混合搜索
        let results = service.hybrid_search("async programming", 5, 0.5, None, None)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].final_score >= 0.5);
    }

    #[tokio::test]
    async fn test_hybrid_merge() {
        let ops = HybridSearchOperations::new(create_test_state());

        let vector = vec![("a".to_string(), 0.9), ("b".to_string(), 0.7)];
        let keyword = vec![("b".to_string(), 0.8), ("c".to_string(), 0.6)];

        let merged = ops.hybrid_merge(&vector, &keyword, 10);

        // b 应该排在最前面（同时有向量和关键词分数）
        assert_eq!(merged[0].key, "b");
        assert!(merged[0].vector_score.is_some());
        assert!(merged[0].keyword_score.is_some());
    }
}
```

---

## 9. 迁移指南

### 9.1 从纯向量搜索迁移

```rust
// 旧代码（纯向量搜索）
let results = service.semantic_search("query", 10, 0.7).await?;

// 新代码（混合搜索）
let results = service.hybrid_search("query", 10, 0.7, None, None).await?;
```

### 9.2 向后兼容

```rust
impl MemoryService {
    /// 保持 semantic_search 作为 hybrid_search 的别名（默认高向量权重）
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<MemorySearchResult>> {
        // 使用高向量权重执行混合搜索
        let config = HybridSearchConfig {
            vector_weight: 0.9,
            keyword_weight: 0.1,
            ..Default::default()
        };

        self.hybrid_ops
            .with_config(config)
            .hybrid_search(query, limit, None, None)
            .await
            .map(|r| r.into_iter().map(|h| h.into()).collect())
    }
}
```

---

## 10. 总结

通过本方案，CIS 可以获得：

1. ✅ **混合搜索能力**：向量语义 + 关键词精确匹配
2. ✅ **加权融合**：可配置的 vector/keyword 权重
3. ✅ **高性能**：保留 sqlite-vec HNSW O(log N) 向量检索
4. ✅ **向后兼容**：原有 semantic_search API 保持不变
5. ✅ **灵活配置**：支持自定义搜索策略

**核心改进点**：
- 添加 FTS5 虚拟表和触发器
- 实现加权融合算法（hybrid_merge）
- 并行化向量+关键词搜索
- 提供友好的配置 API
