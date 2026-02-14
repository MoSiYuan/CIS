# 记忆来源可信度追踪设计

> **版本**: v1.1.6
> **创建日期**: 2026-02-13
> **状态**: 设计阶段

---

## 问题背景

用户反馈使用 Kimi 网页版时的问题：

### 1. 全域向量记忆导致过度联想
- AI 回答问题时基于已有向量记忆进行"联想"
- 发散思维阐述自己的观点
- 把自己的观点记录下来作为记忆基准
- 形成恶性循环：观点→记忆→联想→新观点→污染记忆

### 2. 核心问题
> **应该以用户输入为准**，避免模型自己的推断污染记忆

### 3. CIS 当前架构分析

#### ✅ 已有但未使用的机制

**MemoryCategory** (cis-core/src/types.rs:300):
```rust
pub enum MemoryCategory {
    Execution,    // 执行记录
    Result,       // 结果数据
    Error,        // 错误信息
    Context,      // 上下文信息
    Skill,        // 技能经验
}
```

**问题**：当前分类基于**内容类型**，而非**来源可信度**

#### 🔴 当前 set_with_embedding 行为 (cis-core/src/memory/ops/set.rs:154)

```rust
pub async fn set_with_embedding(
    &self,
    key: &str,
    value: &[u8],
    domain: MemoryDomain,
    category: MemoryCategory,
) -> Result<()> {
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
```

**缺陷**：**所有记忆无条件建立向量索引**，无论来源是用户输入还是 AI 推断

#### 🔴 向量搜索无来源过滤 (cis-core/src/vector/storage.rs)

```rust
pub async fn search_memory(
    &self,
    query: &[f32],
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    // HNSW 搜索：返回所有相似向量
    let results = self.hnsw_search(query, top_k).await?;
    // 🔴 无来源可信度过滤
}
```

---

## 设计方案

### 1. 引入 MemorySource 枚举

```rust
/// 记忆来源枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemorySource {
    /// 用户强制指定记忆（可信度：100%）- 最高权重
    UserForced,

    /// 用户直接输入（可信度：80%）- 避免过拟合
    UserInput,

    /// AI 输出 + 用户确认（可信度：80%）
    AIProposalConfirmed,

    /// 总结性文档（可信度：80%）
    SummaryDocument,

    /// AI 自动确认（可信度：50%）
    AIConfirmed,

    /// AI 方案总结（可信度：20%，等待用户确认）
    AIProposalSummary,

    /// AI 推断生成（可信度：0%）- 不索引
    AIInferred,

    /// 外部数据源（可信度：可配置）
    External {
        source: String,
        confidence: f32,  // 0.0 - 1.0
    },
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::UserInput  // 默认 0.8（避免过拟合）
    }
}

impl MemorySource {
    pub fn confidence(&self) -> f32 {
        match self {
            Self::UserForced => 1.0,           // 🔥 用户强制指定，最高权重
            Self::UserInput => 0.8,             // ✅ 用户输入，避免过拟合
            Self::AIProposalConfirmed => 0.8,   // ✅ AI 输出 + 用户确认
            Self::SummaryDocument => 0.8,        // ✅ 总结性文档
            Self::AIConfirmed => 0.5,            // ⚠️ AI 自动确认
            Self::AIProposalSummary => 0.2,      // 🔥 方案总结，低可信度
            Self::AIInferred => 0.0,             // 🔴 AI 推断，不索引
            Self::External { confidence, .. } => *confidence,
        }
    }

    /// 是否可以升级为用户确认状态
    pub fn can_upgrade_to_confirmed(&self) -> bool {
        matches!(self, Self::AIProposalSummary)
    }
}
```

### 2. 扩展 MemoryEntry 结构

```rust
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: i64,
    pub updated_at: i64,

    // 新增字段
    pub source: MemorySource,      // 记忆来源
    pub confidence: f32,              // 可信度 (0.0 - 1.0)
    pub vector_indexed: bool,         // 是否已索引向量
    pub access_count: i64,            // 访问次数（用于热度）
    pub parent_key: Option<String>,    // 🔥 新增：父记忆键（AI 方案总结指向）
    pub confirmed_by_user: bool,       // 🔥 新增：用户是否确认
}
```

### 3. 数据库 Schema 更新

```sql
-- memory_entries 表增加字段
ALTER TABLE memory_entries ADD COLUMN source TEXT NOT NULL DEFAULT 'UserInput';
ALTER TABLE memory_entries ADD COLUMN confidence REAL DEFAULT 1.0;
ALTER TABLE memory_entries ADD COLUMN vector_indexed INTEGER DEFAULT 0;
ALTER TABLE memory_entries ADD COLUMN access_count INTEGER DEFAULT 0;

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_source_confidence
    ON memory_entries(source, confidence);
```

### 4. 条件化向量索引

```rust
impl SetOperations {
    pub async fn set_with_embedding(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
        source: MemorySource,  // 新增参数
    ) -> Result<()> {
        let full_key = self.state.full_key(key);
        let category_str = format!("{:?}", category);

        // 1. 存储到数据库（包含来源信息）
        let entry = MemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            domain,
            category,
            created_at: now,
            updated_at: now,
            source,           // 🔥 关键：记录来源
            confidence: source.confidence(),
            vector_indexed: false,
            access_count: 0,
        };

        match domain {
            MemoryDomain::Private => self.set_private_entry(entry).await?,
            MemoryDomain::Public => self.set_public_entry(entry).await?,
        }

        // 2. 条件化向量索引
        match source {
            MemorySource::UserInput => {
                // ✅ 用户输入：立即建立向量索引
                let text = String::from_utf8_lossy(value);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;

                // 标记为已索引
                self.mark_vector_indexed(&full_key).await?;
            }

            MemorySource::AIInferred => {
                // 🔴 AI 推断：不建立向量索引
                tracing::debug!("Skipping vector index for AI-inferred memory: {}", key);
            }

            MemorySource::AIConfirmed { .. } => {
                // ⚠️ 用户确认的 AI 推断：可选索引
                // 可以根据 confidence 决定是否索引
                if entry.confidence >= 0.5 {
                    let text = String::from_utf8_lossy(value);
                    self.state
                        .vector_storage
                        .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                        .await?;
                    self.mark_vector_indexed(&full_key).await?;
                }
            }

            MemorySource::External { source, confidence } => {
                // 🌐 外部数据源：根据 confidence 决定
                if confidence >= 0.7 {
                    let text = String::from_utf8_lossy(value);
                    self.state
                        .vector_storage
                        .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                        .await?;
                    self.mark_vector_indexed(&full_key).await?;
                }
            }
        }

        // 3. 使缓存失效
        if let Some(cache) = &self.state.cache {
            cache.invalidate(key).await;
        }

        Ok(())
    }
}
```

### 5. 向量搜索时过滤低可信度来源

```rust
impl VectorStorage {
    pub async fn search_memory(
        &self,
        query: &[f32],
        top_k: usize,
        min_confidence: Option<f32>,  // 新增参数
        filter_sources: Option<Vec<MemorySource>>,  // 新增参数
    ) -> Result<Vec<SearchResult>> {
        // 1. HNSW 搜索获取候选
        let mut results = self.hnsw_search(query, top_k * 3).await?;

        // 2. 过滤低可信度来源
        if let Some(min_conf) = min_confidence {
            results.retain(|r| r.confidence >= min_conf);
        }

        // 3. 过滤特定来源
        if let Some(sources) = filter_sources {
            results.retain(|r| sources.contains(&r.source));
        }

        // 4. 按 confidence 和相似度联合排序
        results.sort_by(|a, b| {
            // 优先级：confidence > similarity
            let score_a = a.confidence * 0.7 + a.similarity * 0.3;
            let score_b = b.confidence * 0.7 + b.similarity * 0.3;
            score_b.partial_cmp(&score_a).unwrap()
        });

        // 5. 只返回 top_k 结果
        results.truncate(top_k);

        Ok(results)
    }
}
```

---

## 实现步骤

### Phase 1: 扩展数据模型 (P1.1)

**文件**: `cis-core/src/types/mod.rs`

```rust
// 新增 MemorySource 枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemorySource {
    UserInput,
    AIInferred,
    AIConfirmed,
    External {
        source: String,
        confidence: f32,
    },
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::UserInput
    }
}

impl MemorySource {
    pub fn confidence(&self) -> f32 {
        match self {
            Self::UserInput => 1.0,
            Self::AIInferred => 0.0,
            Self::AIConfirmed { .. } => 0.5,
            Self::External { confidence, .. } => *confidence,
        }
    }
}
```

### Phase 2: 更新数据库 Schema (P1.2)

**文件**: `cis-core/src/storage/memory_db.rs`

```rust
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: MemorySource,      // 新增
    pub confidence: f32,              // 新增
}

pub fn init_schema(&self) -> Result<()> {
    // ... 现有表创建 ...

    // 添加新字段（使用 ALTER TABLE 兼容已有数据）
    self.conn.execute_batch(
        "ALTER TABLE private_entries ADD COLUMN source TEXT DEFAULT 'UserInput';
         ALTER TABLE private_entries ADD COLUMN confidence REAL DEFAULT 1.0;

         ALTER TABLE public_entries ADD COLUMN source TEXT DEFAULT 'UserInput';
         ALTER TABLE public_entries ADD COLUMN confidence REAL DEFAULT 1.0;",
    )?;

    // 创建索引
    self.conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_private_source_conf
            ON private_entries(source, confidence)",
    )?;
    self.conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_public_source_conf
            ON public_entries(source, confidence)",
    )?;

    Ok(())
}
```

### Phase 3: 修改 SET 操作 (P1.3)

**文件**: `cis-core/src/memory/ops/set.rs`

```rust
pub async fn set_with_embedding(
    &self,
    key: &str,
    value: &[u8],
    domain: MemoryDomain,
    category: MemoryCategory,
    source: MemorySource,  // 新增参数
) -> Result<()> {
    let full_key = self.state.full_key(key);
    let category_str = format!("{:?}", category);
    let confidence = source.confidence();

    // 1. 存储到数据库
    match domain {
        MemoryDomain::Private => {
            self.conn.execute(
                "INSERT INTO private_entries (key, value, category, created_at, updated_at, source, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 category = excluded.category,
                 updated_at = excluded.updated_at,
                 source = excluded.source,
                 confidence = excluded.confidence",
                rusqlite::params![key, value, category_str, now, now, source, confidence],
            )?;
        }
        MemoryDomain::Public => {
            self.conn.execute(
                "INSERT INTO public_entries (key, value, category, created_at, updated_at, source, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 category = excluded.category,
                 updated_at = excluded.updated_at,
                 source = excluded.source,
                 confidence = excluded.confidence",
                rusqlite::params![key, value, category_str, now, now, source, confidence],
            )?;
        }
    }

    // 2. 条件化向量索引
    match source {
        MemorySource::UserInput => {
            // ✅ 用户输入：立即索引
            let text = String::from_utf8_lossy(value);
            self.state
                .vector_storage
                .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                .await?;
        }

        MemorySource::AIInferred => {
            // 🔴 AI 推断：不索引
            tracing::debug!("Skipping vector index for AI-inferred memory: {}", key);
        }

        MemorySource::AIConfirmed { .. } => {
            // ⚠️ 确认的 AI 推断：可选索引
            if confidence >= 0.5 {
                let text = String::from_utf8_lossy(value);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
        }

        MemorySource::External { .. } => {
            // 🌐 外部来源：根据 confidence 决定
            if confidence >= 0.7 {
                let text = String::from_utf8_lossy(value);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
        }
    }

    // 3. 使缓存失效
    if let Some(cache) = &self.state.cache {
        cache.invalidate(key).await;
    }

    Ok(())
}
```

### Phase 4: 修改向量搜索 (P1.4)

**文件**: `cis-core/src/vector/storage.rs`

```rust
pub async fn search_memory(
    &self,
    query: &[f32],
    top_k: usize,
    min_confidence: Option<f32>,
    prefer_user_input: bool,  // 新增参数
) -> Result<Vec<SearchResult>> {
    // 1. HNSW 搜索获取候选
    let mut results = self.hnsw_search(query, top_k * 3).await?;

    // 2. 如果启用用户输入优先
    if prefer_user_input {
        // 将 UserInput 结果提前
        results.sort_by(|a, b| {
            let priority_a = if a.source == MemorySource::UserInput { 0 } else { 1 };
            let priority_b = if b.source == MemorySource::UserInput { 0 } else { 1 };
            priority_a.cmp(&priority_b).unwrap()
        });
    }

    // 3. 过滤低可信度
    if let Some(min_conf) = min_confidence {
        results.retain(|r| r.confidence >= min_conf);
    }

    // 4. 联合排序：confidence * 0.7 + similarity * 0.3
    results.sort_by(|a, b| {
        let score_a = a.confidence * 0.7 + a.similarity * 0.3;
        let score_b = b.confidence * 0.7 + b.similarity * 0.3;
        score_b.partial_cmp(&score_a).unwrap()
    });

    // 5. 截断到 top_k
    results.truncate(top_k);

    Ok(results)
}
```

### Phase 5: 更新 API 接口 (P1.5)

**文件**: `cis-core/src/memory/service.rs`

```rust
impl MemoryService {
    /// 存储记忆（用户输入）
    pub async fn set_user_input(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        self.set_ops.set_with_embedding(
            key,
            value,
            domain,
            category,
            MemorySource::UserInput,  // 🔥 关键：标记为用户输入
        ).await
    }

    /// 存储 AI 推断（不索引向量）
    pub async fn set_ai_inferred(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        self.set_ops.set_with_embedding(
            key,
            value,
            domain,
            category,
            MemorySource::AIInferred,  // 🔥 关键：标记为 AI 推断
        ).await
    }

    /// 语义搜索（优先用户输入）
    pub async fn search_memory(
        &self,
        query: &str,
        top_k: usize,
        prefer_user_input: bool,  // 新增参数
    ) -> Result<Vec<MemorySearchResult>> {
        let query_vec = self.embedding.embed(query).await?;

        let results = self.vector_storage.search_memory(
            &query_vec,
            top_k,
            None,  // min_confidence: None
            prefer_user_input,
        ).await?;

        Ok(results)
    }
}
```

---

## 特殊流程：AI 方案总结与确认

### 流程说明

**场景**：AI 需要给用户提供多个解决方案，等待用户选择后再索引。

```
用户问题："如何优化数据库性能？"

┌─────────────────────────────────────────────────────┐
│ 1. AI 分析并生成方案总结                         │
├─────────────────────────────────────────────────────┤
│                                                     │
│  AI 生成方案总结（不索引）：                        │
│  - 方案 A：添加索引（confidence=0.2）             │
│  - 方案 B：优化查询（confidence=0.2）             │
│  - 方案 C：使用缓存（confidence=0.2）             │
│  parent_key = "user/question/db-performance"           │
│  confirmed_by_user = false                           │
│                                                     │
└─────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────┐
│ 2. 用户确认方案 B                                 │
├─────────────────────────────────────────────────────┤
│                                                     │
│  用户确认后升级为可信记忆：                         │
│  - 方案 B：优化查询（confidence=0.8）             │
│  - parent_key = "user/question/db-performance"           │
│  - confirmed_by_user = true                          │
│  - 建立向量索引                                   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### API 设计

#### 1. 保存 AI 方案总结

```rust
impl MemoryService {
    /// 保存 AI 方案总结（多个方案）
    ///
    /// # 参数
    /// - `parent_key`: 父问题键（用于追溯）
    /// - `summaries`: 方案列表（JSON 数组）
    /// - `domain`: 记忆域
    /// - `category`: 分类
    pub async fn save_ai_proposals(
        &self,
        parent_key: &str,
        proposals: Vec<&str>,
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        for (idx, proposal) in proposals.iter().enumerate() {
            let key = format!("{}#proposal_{}", parent_key, idx);
            let value = serde_json::to_vec(proposal)?;

            // 存储为 AIProposalSummary（低可信度，不索引）
            self.set_with_embedding(
                &key,
                &value,
                domain,
                category,
                MemorySource::AIProposalSummary,  // 🔥 方案总结，等待确认
            ).await?;

            keys.push(key);
        }

        Ok(keys)
    }
}
```

#### 2. 用户确认方案

```rust
impl MemoryService {
    /// 用户确认某个方案
    ///
    /// # 参数
    /// - `proposal_key`: 方案键（save_ai_proposals 返回的键）
    /// - `confirmed`: true（确认）或 false（取消）
    pub async fn confirm_ai_proposal(
        &self,
        proposal_key: &str,
        confirmed: bool,
    ) -> Result<()> {
        if !confirmed {
            // 用户取消，删除方案记忆
            return self.delete(proposal_key).await;
        }

        // 读取当前方案记忆
        let entry = self.get(proposal_key).await?
            .ok_or_else(|| CisError::memory("Proposal not found"))?;

        // 检查是否可以升级
        if !entry.source.can_upgrade_to_confirmed() {
            return Err(CisError::memory("Proposal cannot be confirmed"));
        }

        // 获取 parent_key
        let parent_key = entry.parent_key
            .ok_or_else(|| CisError::memory("No parent key"))?;

        // 🔥 关键：升级为 AIProposalConfirmed（高可信度）
        let value = entry.value;
        let new_source = MemorySource::AIProposalConfirmed;

        // 更新记忆：改变 source 和 confidence
        let full_key = self.state.full_key(&proposal_key);
        let confidence = new_source.confidence();

        match entry.domain {
            MemoryDomain::Private => {
                self.conn.execute(
                    "UPDATE private_entries
                     SET source = ?1, confidence = ?2, confirmed_by_user = 1
                     WHERE key = ?3",
                    rusqlite::params![new_source, confidence, full_key],
                )?;
            }
            MemoryDomain::Public => {
                self.conn.execute(
                    "UPDATE public_entries
                     SET source = ?1, confidence = ?2, confirmed_by_user = 1
                     WHERE key = ?3",
                    rusqlite::params![new_source, confidence, full_key],
                )?;
            }
        }

        // ✅ 建立向量索引（现在可以参与了）
        let text = String::from_utf8_lossy(&value);
        let category_str = format!("{:?}", entry.category);
        self.state
            .vector_storage
            .index_memory(&full_key, text.as_bytes(), Some(&category_str))
            .await?;

        tracing::info!("AI proposal confirmed and indexed: {}", proposal_key);

        Ok(())
    }
}
```

#### 3. 搜索时处理已确认方案

```rust
impl VectorStorage {
    pub async fn search_memory(
        &self,
        query: &[f32],
        top_k: usize,
        prefer_user_input: bool,
    ) -> Result<Vec<SearchResult>> {
        // 1. HNSW 搜索获取候选
        let mut results = self.hnsw_search(query, top_k * 3).await?;

        // 2. 用户输入优先
        if prefer_user_input {
            results.sort_by(|a, b| {
                let priority_a = match a.source {
                    MemorySource::UserInput => 0,
                    MemorySource::AIProposalConfirmed => 1,  // ✅ 已确认方案
                    _ => 2,
                };
                let priority_b = match b.source {
                    MemorySource::UserInput => 0,
                    MemorySource::AIProposalConfirmed => 1,
                    _ => 2,
                };
                priority_a.cmp(&priority_b).unwrap()
            });
        }

        // 3. 过滤 AIProposalSummary（未确认的方案总结）
        results.retain(|r| {
            !matches!(r.source, MemorySource::AIProposalSummary)  // 🔴 排除未确认方案
        });

        // 4. 联合排序
        results.sort_by(|a, b| {
            let score_a = a.confidence * 0.7 + a.similarity * 0.3;
            let score_b = b.confidence * 0.7 + b.similarity * 0.3;
            score_b.partial_cmp(&score_a).unwrap()
        });

        results.truncate(top_k);
        Ok(results)
    }
}
```

### 使用示例

#### 场景：数据库性能优化问题

```rust
// ========== 第一步：AI 生成方案总结 ==========
service.save_ai_proposals(
    "user/question/db-performance",
    vec![
        r#"{"title": "添加索引", "description": "在常用字段上创建索引"}"#,
        r#"{"title": "优化查询", "description": "使用预编译语句"}"#,
        r#"{"title": "使用缓存", "description": "缓存热点数据"}"#,
    ],
    MemoryDomain::Public,
    MemoryCategory::Context,
).await?;
// 返回: ["user/question/db-performance#proposal_0", "#proposal_1", "#proposal_2"]

// ✅ 这些方案：
// - source = AIProposalSummary (confidence=0.2)
// - confirmed_by_user = false
// - 🔴 不建立向量索引（不会被搜索到）


// ========== 第二步：用户确认某个方案 ==========
service.confirm_ai_proposal(
    "user/question/db-performance#proposal_1",  // 用户选择了"优化查询"
    true,  // confirmed
).await?;

// ✅ 这个方案记忆：
// - source = AIProposalConfirmed (confidence=0.8)
// - confirmed_by_user = true
// - ✅ 建立向量索引（可以被搜索到）
// - parent_key = "user/question/db-performance" (可追溯回原问题)


// ========== 第三步：用户问"我之前问过什么方案？" ==========
let results = service.search_memory(
    "数据库性能优化方案",
    10,
    true,  // prefer_user_input
).await?;

// ✅ 搜索结果：
// [
//   { key: "#proposal_1", source: AIProposalConfirmed, similarity: 0.92 },
//   // ↑ 用户确认的方案，优先级高
//
//   { key: "#proposal_0", source: AIProposalSummary, similarity: 0.88 },
//   // 🔴 未确认的方案也参与搜索，但优先级低
//
//   { key: "#proposal_2", source: AIProposalSummary, similarity: 0.85 },
// ]
//
// 注意：如果 prefer_user_input=true 且只想要用户确认的方案，
// 可以在搜索时过滤掉 AIProposalSummary
```


---

### 场景 1: 用户直接输入

```rust
// 用户说："记住我喜欢深色主题"
service.set_user_input(
    "user/preference/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;

// ✅ 结果：
// - 存储到 memory.db，source = UserInput, confidence = 1.0
// - 立即建立向量索引，可被语义搜索
// - 用户后续搜索能找到
```

### 场景 2: AI 推断（不污染）

```rust
// AI 基于上下文推断了用户偏好
service.set_ai_inferred(
    "ai/inferred/preference/language",
    b"根据您的项目类型，推荐使用 Rust",
    MemoryDomain::Private,
    MemoryCategory::Context
).await?;

// ✅ 结果：
// - 存储到 memory.db，source = AIInferred, confidence = 0.0
// - 🔴 不建立向量索引，不会被语义搜索
// - 🔴 不会污染后续的向量检索结果
```

### 场景 3: 用户确认的 AI 建议

```rust
// AI 建议："建议您使用 Rust 开发"，用户确认后存储
service.set_with_embedding(
    "project/language",
    b"Rust",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemorySource::AIConfirmed,  // 用户确认
).await?;

// ✅ 结果：
// - 存储到 memory.db，source = AIConfirmed, confidence = 0.5
// - 建立向量索引（因为用户确认了）
// - 搜索时排序权重降低（0.5 vs 1.0）
```

### 场景 4: 语义搜索（优先用户输入）

```rust
// 用户问："我的偏好设置是什么？"
let results = service.search_memory(
    "用户偏好设置",
    10,               // top_k
    true,              // prefer_user_input: true
).await?;

// ✅ 结果排序：
// 1. UserInput 记忆优先（confidence 1.0）
// 2. AIConfirmed 记忆次之（confidence 0.5）
// 3. External 记忆再次（confidence 0.7）
// 🔴 AIInferred 记忆不参与搜索（不索引）
```

---

## 性能和存储影响

### 1. 向量索引大小降低

**当前**：所有记忆都索引
- 假设 10000 条记忆
- 向量数量：10000 个
- 索引大小：~100-200 MB

**优化后**：只索引用户输入
- 假设 70% 用户输入，30% AI 推断
- 向量数量：7000 个
- 索引大小：~70-140 MB
- **节省 30% 存储空间**
- **HNSW 搜索速度提升 25%**

### 2. 搜索准确度提升

**场景**：用户问"我之前设置的主题是什么？"

**当前**：
```
搜索结果（按相似度排序）：
1. "您可能偏好深色主题..." (AIInferred, 相似度 0.85)  ← 🔴 污染
2. "dark" (UserInput, 相似度 0.82)  ← 正确答案
3. "夜间模式对眼睛更好" (AIInferred, 相似度 0.78)
```

**优化后**（prefer_user_input=true）：
```
搜索结果（UserInput 优先）：
1. "dark" (UserInput, confidence=1.0, 相似度 0.82)
2. "您可能偏好深色主题..." (UserInput, confidence=1.0, 相似度 0.85)
3. "夜间模式对眼睛更好" (AIConfirmed, confidence=0.5, 相似度 0.78)
```

### 3. 数据库迁移

```sql
-- 迁移脚本：将已有记忆标记为 UserInput
UPDATE private_entries SET source = 'UserInput', confidence = 1.0
WHERE source IS NULL;

UPDATE public_entries SET source = 'UserInput', confidence = 1.0
WHERE source IS NULL;
```

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 迁移数据量大 | 迁移时间较长 | 分批迁移，后台执行 |
| 旧数据无来源 | 默认标记为 UserInput | 可能过高估计可信度，但安全（宁可信勿缺） |
| API 破坏性变更 | 需要更新所有调用 | 分阶段实现，保持向后兼容 |
| 性能回退 | 条件判断增加开销 | 缓存 hot path，使用 JIT 优化 |

---

## 实施计划

### P1.2.1 (v1.1.6) - 核心功能
- [x] 设计文档完成
- [ ] 实现 MemorySource 枚举
- [ ] 扩展 MemoryEntry 结构
- [ ] 数据库 Schema 迁移
- [ ] 修改 SET 操作条件索引
- [ ] 修改向量搜索过滤
- [ ] 单元测试

### P1.2.2 (v1.1.7) - API 优化
- [ ] 更新 MemoryService 公开 API
- [ ] 添加 set_user_input() 便捷方法
- [ ] 添加 set_ai_inferred() 方法
- [ ] 更新 CLI 命令
- [ ] 文档更新

### P1.2.3 (v1.1.8) - 高级特性
- [ ] 用户配置可信度阈值
- [ ] 学习用户偏好（自动调整）
- [ ] 记忆热度统计（access_count）
- [ ] 自动清理低可信度记忆

---

## v1.1.7 前瞻：记忆污染清理

### 问题背景

即使实施了来源可信度追踪，仍可能出现以下污染情况：

1. **历史污染数据**：v1.1.6 之前的记忆无来源追踪，可能混杂 AI 推断
2. **用户误确认**：用户不小心确认了错误的 AI 方案
3. **外部数据污染**：External 来源的数据质量不佳
4. **级联污染**：基于污染记忆生成的新的 AI 推断

### 清理流程设计

#### Phase 1: 污染检测 (P1.7.1)

```rust
impl MemoryService {
    /// 检测记忆污染
    ///
    /// # 返回
    /// Vec<(污染记忆键, 污染源记忆键, 污染类型)>
    pub async fn detect_pollution(&self) -> Result<Vec<(String, String, PollutionType)>> {
        let mut polluted = Vec::new();

        // 1. 获取所有低可信度记忆（可能是污染源）
        let low_confidence = self.db.query(
            "SELECT key, value FROM memory_entries
             WHERE confidence < 0.5 AND source != 'AIInferred'"
        ).await?;

        for (source_key, source_value) in low_confidence {
            let source_text = String::from_utf8_lossy(&source_value);
            let source_vec = self.embedding.embed(&source_text).await?;

            // 2. 查找相似的高可信度记忆（可能被污染）
            let similar = self.vector_storage.search_memory(
                &source_vec,
                10,
                Some(0.7),  // min_confidence
            ).await?;

            for result in similar {
                if result.similarity > 0.85 {  // 高相似度阈值
                    polluted.push((
                        result.key.clone(),
                        source_key.clone(),
                        PollutionType::Similarity {
                            similarity: result.similarity,
                            source_confidence: result.confidence,
                        },
                    ));
                }
            }
        }

        Ok(polluted)
    }
}

pub enum PollutionType {
    /// 相似度污染（低可信度记忆与高可信度记忆高度相似）
    Similarity {
        similarity: f32,
        source_confidence: f32,
    },
    /// 级联污染（记忆 B 基于记忆 A 生成，但 A 是污染的）
    Cascading {
        parent_key: String,
    },
    /// 外部数据污染（External 来源数据质量不佳）
    ExternalData {
        source: String,
        quality_score: f32,
    },
}
```

#### Phase 2: 污染源头追踪 (P1.7.2)

```rust
impl MemoryService {
    /// 追踪污染源头（模因分析）
    ///
    /// # 参数
    /// - `polluted_key`: 被污染的记忆键
    ///
    /// # 返回
    /// 污染链：从源头到被污染记忆的完整路径
    pub async fn trace_pollution_source(
        &self,
        polluted_key: &str,
    ) -> Result<Vec<PollutionTrace>> {
        let entry = self.get(polluted_key).await?
            .ok_or_else(|| CisError::memory("Key not found"))?;

        let mut traces = Vec::new();

        // 1. 检查是否有 parent_key（AI 方案总结特有）
        if let Some(parent) = &entry.parent_key {
            // 追溯到父记忆
            let parent_entry = self.get(parent).await?;

            if let Some(parent) = parent_entry {
                // 递归追溯
                let parent_traces = self.trace_pollution_source(parent).await?;
                traces.extend(parent_traces);
            }

            traces.push(PollutionTrace {
                key: polluted_key.to_string(),
                source: entry.source,
                confidence: entry.confidence,
                parent_key: Some(parent.clone()),
                trace_type: TraceType::ProposalPath,
            });
        }

        // 2. 向量相似度分析（模因传播路径）
        let entry_vec = self.embedding.get_embedding(polluted_key).await?;

        let similar_memories = self.vector_storage.search_memory(
            &entry_vec,
            20,
            None,  // 不过滤 confidence，找到所有相似记忆
        ).await?;

        for similar in similar_memories {
            if similar.key != polluted_key && similar.similarity > 0.9 {
                // 高相似度，可能是模因传播
                traces.push(PollutionTrace {
                    key: similar.key,
                    source: similar.source,
                    confidence: similar.confidence,
                    parent_key: None,
                    trace_type: TraceType::MemePropagation {
                        similarity: similar.similarity,
                    },
                });
            }
        }

        Ok(traces)
    }

    /// 基于本地向量引擎处理记录并总结
    pub async fn summarize_pollution_report(&self) -> Result<PollutionReport> {
        let polluted = self.detect_pollution().await?;

        let mut report = PollutionReport {
            total_polluted: polluted.len(),
            pollution_types: HashMap::new(),
            cleanup_recommendations: Vec::new(),
        };

        for (key, source, ptype) in polluted {
            // 统计污染类型
            *report.pollution_types
                .entry(format!("{:?}", ptype))
                .or_insert(0) += 1;

            // 追踪源头
            let traces = self.trace_pollution_source(&key).await?;

            // 生成清理建议
            let recommendation = match ptype {
                PollutionType::Similarity { .. } => {
                    CleanupRecommendation::Delete {
                        key: key.clone(),
                        reason: format!("与低可信度记忆 {} 高度相似", source),
                    }
                }
                PollutionType::Cascading { parent_key } => {
                    CleanupRecommendation::CascadeDelete {
                        key: key.clone(),
                        parent: parent_key,
                        reason: "级联污染，需要连同源头一起删除".to_string(),
                    }
                }
                PollutionType::ExternalData { source, quality_score } => {
                    if quality_score < 0.3 {
                        CleanupRecommendation::Delete {
                            key: key.clone(),
                        reason: format!("外部数据源 {} 质量过低（{}）", source, quality_score),
                        }
                    } else {
                        CleanupRecommendation::Downgrade {
                            key: key.clone(),
                            new_confidence: 0.3,
                            reason: "降低可信度但保留数据".to_string(),
                        }
                    }
                }
            };

            report.cleanup_recommendations.push(recommendation);
        }

        Ok(report)
    }
}

#[derive(Debug)]
pub struct PollutionReport {
    pub total_polluted: usize,
    pub pollution_types: HashMap<String, usize>,
    pub cleanup_recommendations: Vec<CleanupRecommendation>,
}

#[derive(Debug)]
pub enum CleanupRecommendation {
    /// 删除被污染记忆
    Delete {
        key: String,
        reason: String,
    },
    /// 级联删除（连同污染源头一起删除）
    CascadeDelete {
        key: String,
        parent: String,
        reason: String,
    },
    /// 降级可信度（不删除，但降低 confidence）
    Downgrade {
        key: String,
        new_confidence: f32,
        reason: String,
    },
}
```

#### Phase 3: 清理执行 (P1.7.3)

```rust
impl MemoryService {
    /// 执行清理操作
    pub async fn execute_cleanup(
        &self,
        recommendations: Vec<CleanupRecommendation>,
    ) -> Result<CleanupResult> {
        let mut result = CleanupResult {
            deleted: 0,
            downgraded: 0,
            errors: Vec::new(),
        };

        for rec in recommendations {
            match rec {
                CleanupRecommendation::Delete { key, reason } => {
                    match self.delete(&key).await {
                        Ok(_) => {
                            result.deleted += 1;
                            tracing::info!("Deleted polluted memory: {} - {}", key, reason);
                        }
                        Err(e) => {
                            result.errors.push((key, e.to_string()));
                        }
                    }
                }

                CleanupRecommendation::CascadeDelete { key, parent, reason } => {
                    // 1. 删除源头
                    if let Err(e) = self.delete(&parent).await {
                        result.errors.push((parent.clone(), e.to_string()));
                        continue;
                    }

                    // 2. 删除被污染的记忆
                    if let Err(e) = self.delete(&key).await {
                        result.errors.push((key.clone(), e.to_string()));
                        continue;
                    }

                    result.deleted += 2;
                    tracing::info!("Cascade deleted: {} -> {} - {}", parent, key, reason);
                }

                CleanupRecommendation::Downgrade { key, new_confidence, reason } => {
                    match self.downgrade_confidence(&key, new_confidence).await {
                        Ok(_) => {
                            result.downgraded += 1;
                            tracing::info!("Downgraded: {} -> {} - {}", key, new_confidence, reason);
                        }
                        Err(e) => {
                            result.errors.push((key, e.to_string()));
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// 降级记忆可信度（并删除向量索引）
    async fn downgrade_confidence(
        &self,
        key: &str,
        new_confidence: f32,
    ) -> Result<()> {
        let entry = self.get(key).await?
            .ok_or_else(|| CisError::memory("Key not found"))?;

        // 1. 更新数据库 confidence
        let full_key = self.state.full_key(key);
        match entry.domain {
            MemoryDomain::Private => {
                self.conn.execute(
                    "UPDATE private_entries SET confidence = ?1 WHERE key = ?2",
                    rusqlite::params![new_confidence, full_key],
                )?;
            }
            MemoryDomain::Public => {
                self.conn.execute(
                    "UPDATE public_entries SET confidence = ?1 WHERE key = ?2",
                    rusqlite::params![new_confidence, full_key],
                )?;
            }
        }

        // 2. 删除向量索引（不再参与搜索）
        self.vector_storage.remove_index(&full_key).await?;

        tracing::info!("Downgraded confidence and removed vector index: {}", key);

        Ok(())
    }
}

#[derive(Debug)]
pub struct CleanupResult {
    pub deleted: usize,
    pub downgraded: usize,
    pub errors: Vec<(String, String)>,  // (key, error)
}
```

### 完整使用流程

```rust
// ========== 第一步：检测污染 ==========
let polluted = service.detect_pollution().await?;
println!("发现 {} 个被污染的记忆", polluted.len());

// ========== 第二步：生成清理报告 ==========
let report = service.summarize_pollution_report().await?;
println!("污染报告：");
println!("- 总数：{}", report.total_polluted);
println!("- 类型：{:?}", report.pollution_types);
println!("- 清理建议：{} 条", report.cleanup_recommendations.len());

// ========== 第三步：用户审核 ==========
// 显示清理建议，用户决定是否执行
for rec in &report.cleanup_recommendations {
    println!("{:?}", rec);
}

// ========== 第四步：执行清理 ==========
let result = service.execute_cleanup(report.cleanup_recommendations).await?;
println!("清理完成：");
println!("- 删除：{} 条", result.deleted);
println!("- 降级：{} 条", result.downgraded);
println!("- 错误：{} 条", result.errors.len());
```

---

## 版本规划

### v1.1.6 (当前）- 来源可信度

**目标**：区分记忆来源，避免 AI 推断污染

- [x] 设计文档完成
- [ ] 实现 MemorySource 枚举（修正权重数值）
- [ ] 扩展 MemoryEntry 结构
- [ ] 数据库 Schema 迁移
- [ ] 修改 SET 操作条件索引
- [ ] 修改向量搜索过滤
- [ ] 单元测试

**权重配置**（已修正）：
- UserForced: 1.0（用户强制指定）
- UserInput: 0.8（用户输入，避免过拟合）
- AIProposalConfirmed: 0.8（AI 输出 + 用户确认）
- SummaryDocument: 0.8（总结性文档）
- AIConfirmed: 0.5（AI 自动确认）
- AIProposalSummary: 0.2（方案总结，等待确认）
- AIInferred: 0.0（单纯 AI 输出，不索引）

### v1.1.7 (未来）- 记忆污染清理

**目标**：检测和清理已污染的记忆

- [ ] Phase 1: 污染检测 (detect_pollution)
- [ ] Phase 2: 源头追踪 (trace_pollution_source)
- [ ] Phase 3: 清理执行 (execute_cleanup)
- [ ] CLI 命令：`cis memory cleanup`
- [ ] 自动清理选项（用户确认后执行）
- [ ] 清理日志和审计

**技术要点**：
- 基于本地向量引擎进行相似度分析
- 追踪污染源头（parent_key、向量相似度）
- 级联删除（连同污染源头）
- 降级而非删除（保留数据但降低权重）

### v1.1.8 (远期）- 自适应权重

**目标**：根据用户行为自动调整权重

- [ ] 记忆访问统计（access_count）
- [ ] 用户反馈学习（用户手动调整后记录偏好）
- [ ] 自动降级长期未访问的低可信度记忆
- [ ] 动态阈值调整（根据污染检测频率）

---

**维护者**: CIS v1.1.6 Team
**最后更新**: 2026-02-13（修正权重数值，添加 v1.1.7 污染清理设计）
