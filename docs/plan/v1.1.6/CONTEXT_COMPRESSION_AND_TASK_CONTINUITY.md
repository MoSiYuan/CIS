# 上下文压缩与任务延续性设计

> **版本**: v1.1.6
> **创建日期**: 2026-02-13
> **关联**: [MEMORY_SOURCE_TRUST_DESIGN.md](./MEMORY_SOURCE_TRUST_DESIGN.md)

---

## 问题分析

### 核心矛盾

```
┌─────────────────────────────────────────────────────────────┐
│                    上下文压缩困境                              │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  完整保真（保留所有细节）                                  │
│       ↓                                                        │
│   - 上下文占用高（浪费空间）                                  │
│   - 超过 LLM 上下文窗口                                      │
│   - 成本高（token 计费）                                      │
│                                                               │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    极度压缩困境                              │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  高度压缩（丢弃细节）                                         │
│       ↓                                                        │
│   - 数据失真（语义漂移）                                      │
│   - 任务延续性下降（Agent 理解偏差）                         │
│   - 重复相同问题（用户抱怨）                                   │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### 传统压缩方案的问题

| 方案 | 优势 | 劣势 | 失真影响 |
|------|------|------|----------|
| **时间窗口滑动**（最近 N 条消息） | 简单 | 丢失早期关键信息 | 🔴 高（任务目标丢失） |
| **关键信息提取**（JSON 结构） | 结构化 | 丢失上下文细节 | 🔴 中（语义漂移） |
| **摘要生成**（AI 总结） | 压缩率高 | 摘要主观性 | 🔴 高（AI 观点污染） |
| **向量检索 TopK** | 语义相关 | 召回率不稳定 | 🔴 中（漏关键信息） |

---

## 设计方案：分层次可信度压缩

### 核心思想

结合 **MemorySource 可信度体系**，实现**分层渐进压缩**：

```
原始上下文（100K tokens）
    ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: UserForced (1.0)           │ ← 完整保留（0% 压缩）
│ - 用户强制标记："这个决策很重要"                    │
│ - 项目架构约定（Rust + SQLite）                        │
│ 压缩后：100% 保留                                        │
└─────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: UserInput (0.8)              │ ← 轻度压缩（10-20%）
│ - 用户偏好："我喜欢深色主题"                        │
│ - 用户确认的方案                                     │
│ 压缩后：保留关键句，移除冗余                          │
└─────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 3: AIProposalConfirmed (0.8)    │ ← 中度压缩（30-50%）
│ - AI 输出 + 用户确认                                  │
│ - 总结性文档                                           │
│ 压缩后：提取要点，结构化存储                         │
└─────────────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 4: AIConfirmed (0.5)             │ ← 高度压缩（60-80%）
│ - AI 自动确认                                         │
│ 压缩后：极简表示或向量化                           │
└─────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 5: AIInferred (0.0)              │ ← 极度压缩或丢弃
│ - 单纯 AI 输出                                         │
│ 压缩后：只保留向量嵌入，不占用上下文                 │
└─────────────────────────────────────────────────────────────┘

总压缩后：~30-40K tokens（节省 60-70%）
任务延续性：✅ 高（关键信息完整）
```

---

## Phase 1: 分层压缩算法 (P1.3.1)

### 1.1 定义压缩配置

```rust
/// 分层压缩配置
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// UserForced: 压缩率（0.0 = 不压缩）
    pub user_forced_ratio: f32,  // 默认 0.0

    /// UserInput: 压缩率
    pub user_input_ratio: f32,  // 默认 0.15（15%）

    /// AIProposalConfirmed: 压缩率
    pub ai_proposal_confirmed_ratio: f32,  // 默认 0.4（40%）

    /// AIConfirmed: 压缩率
    pub ai_confirmed_ratio: f32,  // 默认 0.7（70%）

    /// AIInferred: 压缩率
    pub ai_inferred_ratio: f32,  // 默认 0.95（95%，几乎全压）

    /// 最大上下文长度（tokens）
    pub max_context_tokens: usize,  // 默认 40K
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            user_forced_ratio: 0.0,
            user_input_ratio: 0.15,
            ai_proposal_confirmed_ratio: 0.4,
            ai_confirmed_ratio: 0.7,
            ai_inferred_ratio: 0.95,
            max_context_tokens: 40_000,
        }
    }
}
```

### 1.2 分层压缩实现

```rust
impl ContextService {
    /// 分层压缩上下文
    ///
    /// # 参数
    /// - `memories`: 所有记忆（按可信度分层）
    /// - `config`: 压缩配置
    ///
    /// # 返回
    /// (压缩后上下文, 压缩报告)
    pub async fn compress_context_layered(
        &self,
        memories: Vec<MemoryEntry>,
        config: CompressionConfig,
    ) -> Result<(String, CompressionReport)> {
        let mut layers: HashMap<MemorySource, Vec<_>> = HashMap::new();
        let mut report = CompressionReport {
            original_tokens: 0,
            compressed_tokens: 0,
            layer_stats: HashMap::new(),
        };

        // 1. 按来源分层
        for memory in memories {
            let token_count = self.count_tokens(&memory.value);
            report.original_tokens += token_count;

            layers.entry(memory.source)
                .or_insert_with(Vec::new)
                .push((memory, token_count));
        }

        // 2. 分层压缩
        let mut compressed_context = String::new();

        // Layer 1: UserForced（完整保留）
        if let Some(items) = layers.remove(&MemorySource::UserForced) {
            for (memory, tokens) in items {
                compressed_context.push_str(&format!(
                    "[USER_FORCED] {}\n",
                    String::from_utf8_lossy(&memory.value)
                ));
                report.compressed_tokens += tokens;
                report.layer_stats.insert("UserForced", LayerStat {
                    original: tokens,
                    compressed: tokens,
                    ratio: 0.0,
                });
            }
        }

        // Layer 2: UserInput（轻度压缩）
        if let Some(items) = layers.remove(&MemorySource::UserInput) {
            let compressed = self.compress_layer(
                items,
                config.user_input_ratio,
                CompressionLevel::Light,
            ).await?;

            report.compressed_tokens += compressed.total_tokens;
            report.layer_stats.insert("UserInput", compressed.stat);
            compressed_context.push_str(&compressed.text);
        }

        // Layer 3: AIProposalConfirmed（中度压缩）
        if let Some(items) = layers.remove(&MemorySource::AIProposalConfirmed) {
            let compressed = self.compress_layer(
                items,
                config.ai_proposal_confirmed_ratio,
                CompressionLevel::Medium,
            ).await?;

            report.compressed_tokens += compressed.total_tokens;
            report.layer_stats.insert("AIProposalConfirmed", compressed.stat);
            compressed_context.push_str(&compressed.text);
        }

        // Layer 4: AIConfirmed（高度压缩）
        if let Some(items) = layers.remove(&MemorySource::AIConfirmed) {
            let compressed = self.compress_layer(
                items,
                config.ai_confirmed_ratio,
                CompressionLevel::Heavy,
            ).await?;

            report.compressed_tokens += compressed.total_tokens;
            report.layer_stats.insert("AIConfirmed", compressed.stat);
            compressed_context.push_str(&compressed.text);
        }

        // Layer 5: AIInferred（极度压缩）
        if let Some(items) = layers.remove(&MemorySource::AIInferred) {
            let compressed = self.compress_layer(
                items,
                config.ai_inferred_ratio,
                CompressionLevel::Extreme,
            ).await?;

            report.compressed_tokens += compressed.total_tokens;
            report.layer_stats.insert("AIInferred", compressed.stat);
            compressed_context.push_str(&compressed.text);
        }

        Ok((compressed_context, report))
    }

    /// 压缩单层记忆
    async fn compress_layer(
        &self,
        items: Vec<(MemoryEntry, usize)>,  // (entry, token_count)
        target_ratio: f32,
        level: CompressionLevel,
    ) -> Result<CompressedLayer> {
        let original_tokens: usize = items.iter().map(|(_, t)| t).sum();
        let target_tokens = (original_tokens as f32 * (1.0 - target_ratio)) as usize;

        let mut compressed = String::new();
        let mut compressed_tokens = 0;

        match level {
            CompressionLevel::None => {
                // 完整保留
                for (memory, _) in items {
                    compressed.push_str(&String::from_utf8_lossy(&memory.value));
                    compressed.push_str("\n");
                }
                compressed_tokens = original_tokens;
            }

            CompressionLevel::Light => {
                // 轻度压缩：移除冗余，保留关键句
                for (memory, _) in items {
                    let text = String::from_utf8_lossy(&memory.value);
                    let sentences = self.extract_key_sentences(&text, 0.8).await?;
                    compressed.push_str(&sentences.join(" "));
                    compressed.push_str("\n");
                }
                compressed_tokens = self.count_tokens(&compressed);
            }

            CompressionLevel::Medium => {
                // 中度压缩：提取要点，结构化
                for (memory, _) in items {
                    let text = String::from_utf8_lossy(&memory.value);
                    let summary = self.extract_summary(&text).await?;
                    compressed.push_str(&format!("- {}\n", summary));
                }
                compressed_tokens = self.count_tokens(&compressed);
            }

            CompressionLevel::Heavy => {
                // 高度压缩：极简表示
                for (memory, _) in items {
                    let text = String::from_utf8_lossy(&memory.value);
                    let keywords = self.extract_keywords(&text, 3).await?;  // 前 3 个关键词
                    compressed.push_str(&format!("[{}]\n", keywords.join(", ")));
                }
                compressed_tokens = self.count_tokens(&compressed);
            }

            CompressionLevel::Extreme => {
                // 极度压缩：向量化（只保留向量嵌入）
                // 🔥 不占用上下文，只保留向量索引
                for (memory, _) in items {
                    // 向量化：将文本转为向量 ID
                    let vec_id = self.vector_storage.get_vector_id(&memory.key).await?;
                    compressed.push_str(&format!("<VEC:{}> ", vec_id));
                }
                compressed_tokens = items.len();  // 每个 <VEC:id> 算作 1 token
            }
        }

        Ok(CompressedLayer {
            text: compressed,
            total_tokens: compressed_tokens,
            stat: LayerStat {
                original: original_tokens,
                compressed: compressed_tokens,
                ratio: if original_tokens > 0 {
                    (original_tokens - compressed_tokens) as f32 / original_tokens as f32
                } else {
                    0.0
                },
            },
        })
    }
}

#[derive(Debug)]
pub struct CompressedLayer {
    pub text: String,
    pub total_tokens: usize,
    pub stat: LayerStat,
}

#[derive(Debug)]
pub struct LayerStat {
    pub original: usize,
    pub compressed: usize,
    pub ratio: f32,  // 压缩率
}

pub enum CompressionLevel {
    None,       // 完整保留（0% 压缩）
    Light,       // 轻度压缩（10-20%）
    Medium,      // 中度压缩（30-50%）
    Heavy,       // 高度压缩（60-80%）
    Extreme,     // 极度压缩（90-95%）
}
```

---

## Phase 2: 语义去重与聚类 (P1.3.2)

### 问题

即使分层压缩，仍可能出现：
- **重复信息**：用户多次提到相同偏好
- **语义相近**：不同记忆表达相似含义
- **上下文浪费**：重复内容占用空间

### 解决方案：语义聚类去重

```rust
impl ContextService {
    /// 语义聚类去重
    ///
    /// # 算法
    /// 1. 将所有记忆转为向量嵌入
    /// 2. DBSCAN 聚类（相似度阈值 0.85）
    /// 3. 每个聚类选择最高可信度的代表
    /// 4. 移除聚类内的其他成员
    pub async fn semantic_dedup_compress(
        &self,
        memories: Vec<MemoryEntry>,
        similarity_threshold: f32,  // 默认 0.85
    ) -> Result<(Vec<MemoryEntry>, DedupReport)> {
        let mut report = DedupReport {
            original_count: memories.len(),
            deduped_count: 0,
            clusters_found: 0,
            tokens_saved: 0,
        };

        // 1. 获取所有向量嵌入
        let mut embeddings = Vec::new();
        for memory in &memories {
            let vec = self.vector_storage.get_embedding(&memory.key).await?;
            embeddings.push((memory.clone(), vec));
        }

        // 2. DBSCAN 聚类
        let clusters = self.dbscan_cluster(
            embeddings,
            similarity_threshold,
            min_points: 2,  // 至少 2 个点才算聚类
        ).await?;

        report.clusters_found = clusters.len();

        // 3. 每个聚类选择代表（最高可信度）
        let mut deduped_memories = Vec::new();
        let mut to_remove = HashSet::new();

        for cluster in clusters {
            if cluster.len() <= 1 {
                // 单点聚类，保留
                deduped_memories.push(cluster[0].0.clone());
                continue;
            }

            // 找到最高可信度的记忆
            let best = cluster.iter()
                .max_by_key(|(mem, _)| mem.source.confidence())
                .unwrap();

            deduped_memories.push(best.0.clone());

            // 标记其他为待删除
            for (mem, _) in cluster {
                if mem.key != best.0.key {
                    to_remove.insert(mem.key.clone());
                    report.tokens_saved += self.count_tokens(&mem.value);
                }
            }
        }

        report.deduped_count = memories.len() - deduped_memories.len();

        tracing::info!(
            "Semantic dedup: {} -> {} memories, saved {} tokens",
            memories.len(),
            deduped_memories.len(),
            report.tokens_saved
        );

        Ok((deduped_memories, report))
    }

    /// DBSCAN 聚类（基于余弦相似度）
    async fn dbscan_cluster(
        &self,
        embeddings: Vec<(MemoryEntry, Vec<f32>)>,
        similarity_threshold: f32,
        min_points: usize,
    ) -> Result<Vec<Vec<(MemoryEntry, Vec<f32>)>> {
        // 简化的 DBSCAN 实现
        let mut clusters = Vec::new();
        let mut visited = HashSet::new();

        for (i, (mem_i, vec_i)) in embeddings.iter().enumerate() {
            if visited.contains(&mem_i.key) {
                continue;
            }

            // 找邻域点
            let mut neighbors = Vec::new();
            for (mem_j, vec_j) in &embeddings {
                if i == embeddings.iter().position(|(m, _)| m.key == mem_j.key).unwrap() {
                    continue;
                }

                let similarity = cosine_similarity(vec_i, vec_j);
                if similarity >= similarity_threshold {
                    neighbors.push((mem_j.clone(), vec_j.clone()));
                }
            }

            if neighbors.len() < min_points {
                // 噪声点
                visited.insert(mem_i.key.clone());
                continue;
            }

            // 创建新聚类
            let mut cluster = vec![(mem_i.clone(), vec_i.clone())];
            visited.insert(mem_i.key.clone());

            // 扩展聚类（递归）
            let mut idx = 0;
            while idx < neighbors.len() {
                let (mem_n, vec_n) = &neighbors[idx];
                if !visited.contains(&mem_n.key) {
                    visited.insert(mem_n.key.clone());
                    cluster.push((mem_n.clone(), vec_n.clone()));

                    // 找邻域的邻域
                    for (mem_m, vec_m) in &embeddings {
                        if visited.contains(&mem_m.key) {
                            continue;
                        }

                        let sim = cosine_similarity(vec_n, vec_m);
                        if sim >= similarity_threshold {
                            neighbors.push((mem_m.clone(), vec_m.clone()));
                        }
                    }
                }
                idx += 1;
            }

            clusters.push(cluster);
        }

        Ok(clusters)
    }
}

#[derive(Debug)]
pub struct DedupReport {
    pub original_count: usize,
    pub deduped_count: usize,
    pub clusters_found: usize,
    pub tokens_saved: usize,
}
```

---

## Phase 3: 自适应压缩比调整 (P1.3.3)

### 问题

固定压缩比可能导致：
- **压缩不足**：上下文仍然超长
- **过度压缩**：任务延续性下降

### 解决方案：动态调整压缩比

```rust
impl ContextService {
    /// 自适应压缩（根据上下文长度动态调整）
    pub async fn adaptive_compress(
        &self,
        memories: Vec<MemoryEntry>,
        base_config: CompressionConfig,
        max_tokens: usize,
    ) -> Result<(String, CompressionReport)> {
        let mut config = base_config.clone();
        let mut iteration = 0;
        let max_iterations = 5;

        loop {
            iteration += 1;
            if iteration > max_iterations {
                return Err(CisError::context(
                    format!("Failed to compress to {} tokens after {} iterations", max_tokens, max_iterations)
                ));
            }

            // 1. 尝试压缩
            let (compressed, report) = self.compress_context_layered(
                memories.clone(),
                config.clone(),
            ).await?;

            // 2. 检查长度
            let compressed_tokens = self.count_tokens(&compressed);

            if compressed_tokens <= max_tokens {
                // ✅ 达到目标
                tracing::info!(
                    "Adaptive compress converged in {} iterations: {} -> {} tokens ({}% saved)",
                    iteration,
                    report.original_tokens,
                    compressed_tokens,
                    (report.original_tokens - compressed_tokens) * 100 / report.original_tokens
                );
                return Ok((compressed, report));
            }

            // 3. 超出目标，增加压缩比
            let over_ratio = (compressed_tokens - max_tokens) as f32 / compressed_tokens as f32;
            tracing::debug!(
                "Iteration {}: {} tokens (target {}), over by {:.1}%, increasing compression",
                iteration,
                compressed_tokens,
                max_tokens,
                over_ratio * 100.0
            );

            // 按比例增加各层压缩比
            config.user_input_ratio = (config.user_input_ratio + over_ratio * 0.5).min(0.8);
            config.ai_proposal_confirmed_ratio = (config.ai_proposal_confirmed_ratio + over_ratio * 0.6).min(0.9);
            config.ai_confirmed_ratio = (config.ai_confirmed_ratio + over_ratio * 0.7).min(0.95);
            config.ai_inferred_ratio = (config.ai_inferred_ratio + over_ratio * 0.8).min(0.98);

            // UserForced 永不压缩（保持 0.0）
        }
    }
}
```

---

## Phase 4: 任务延续性保障 (P1.3.4)

### 关键问题

压缩后如何确保 Agent 仍能准确理解任务目标？

### 解决方案 1: 任务链追踪

```rust
/// 任务链（追踪任务演进）
#[derive(Debug, Clone)]
pub struct TaskChain {
    pub task_id: String,
    pub created_at: i64,
    pub initial_prompt: String,           // 初始任务描述（UserForced, 完整保留）
    pub evolution_steps: Vec<TaskStep>,    // 任务演进历史
}

#[derive(Debug, Clone)]
pub struct TaskStep {
    pub step_id: String,
    pub timestamp: i64,
    pub action: String,                   // 操作描述
    pub result: String,                   // 结果
    pub next_tasks: Vec<String>,         // 衍生任务
}

impl ContextService {
    /// 构建任务链上下文（确保延续性）
    pub async fn build_task_chain_context(
        &self,
        task_chain: &TaskChain,
    ) -> Result<String> {
        let mut context = String::new();

        // 1. 初始任务（完整保留）
        context.push_str(&format!(
            "[TASK_INIT] {}\n",
            task_chain.initial_prompt
        ));

        // 2. 演进路径（轻度压缩）
        context.push_str("[EVOLUTION_PATH]\n");
        for (idx, step) in task_chain.evolution_steps.iter().enumerate() {
            context.push_str(&format!(
                "{}. {} -> {}\n",
                idx + 1,
                step.action,
                step.result
            ));
        }

        // 3. 当前状态（中度压缩）
        if let Some(latest) = task_chain.evolution_steps.last() {
            context.push_str(&format!(
                "[CURRENT_STATUS] Last action: {}, Result: {}\n",
                latest.action,
                latest.result
            ));
        }

        Ok(context)
    }
}
```

### 解决方案 2: 关键信息锚点

```rust
impl ContextService {
    /// 提取关键信息锚点（必须保留）
    pub async fn extract_anchors(
        &self,
        memories: Vec<MemoryEntry>,
    ) -> Result<Vec<Anchor>> {
        let mut anchors = Vec::new();

        for memory in memories {
            match memory.source {
                MemorySource::UserForced => {
                    // 🔥 强制锚点：完整保留
                    anchors.push(Anchor {
                        key: memory.key.clone(),
                        text: String::from_utf8_lossy(&memory.value),
                        priority: AnchorPriority::Critical,
                        compressible: false,
                    });
                }

                MemorySource::UserInput => {
                    // 提取关键句（轻度压缩）
                    let sentences = self.extract_key_sentences(
                        &String::from_utf8_lossy(&memory.value),
                        0.9,  // 高阈值
                    ).await?;

                    for sentence in sentences {
                        anchors.push(Anchor {
                            key: format!("{}#anchor", memory.key),
                            text: sentence,
                            priority: AnchorPriority::High,
                            compressible: true,
                        });
                    }
                }

                _ => {
                    // 其他来源：可选压缩
                    let summary = self.extract_summary(
                        &String::from_utf8_lossy(&memory.value)
                    ).await?;

                    anchors.push(Anchor {
                        key: memory.key.clone(),
                        text: summary,
                        priority: AnchorPriority::Medium,
                        compressible: true,
                    });
                }
            }
        }

        Ok(anchors)
    }
}

#[derive(Debug)]
pub struct Anchor {
    pub key: String,
    pub text: String,
    pub priority: AnchorPriority,
    pub compressible: bool,  // 是否可压缩
}

#[derive(Debug)]
pub enum AnchorPriority {
    Critical,  // 不可压缩（UserForced）
    High,      // 轻度压缩
    Medium,    // 中度压缩
    Low,       // 高度压缩
}
```

---

## 完整使用流程

### 场景：长对话任务延续

```rust
// ========== 第一步：获取所有相关记忆 ==========
let memories = service.get_memories_by_task("task-123").await?;

// ========== 第二步：语义去重 ==========
let (deduped, dedup_report) = service.semantic_dedup_compress(
    memories,
    0.85,  // 相似度阈值
).await?;

println!("去重：{} -> {} 记忆，节省 {} tokens",
    dedup_report.original_count,
    dedup_report.deduped_count,
    dedup_report.tokens_saved
);

// ========== 第三步：自适应压缩 ==========
let (compressed, compress_report) = service.adaptive_compress(
    deduped,
    CompressionConfig::default(),
    40_000,  // 目标 40K tokens
).await?;

println!("压缩：{} -> {} tokens ({}% 节省)",
    compress_report.original_tokens,
    compress_report.compressed_tokens,
    (compress_report.original_tokens - compress_report.compressed_tokens) * 100 / compress_report.original_tokens
);

// ========== 第四步：构建任务链上下文 ==========
let task_chain = service.get_task_chain("task-123").await?;
let task_context = service.build_task_chain_context(&task_chain).await?;

// ========== 第五步：组合最终上下文 ==========
let final_context = format!(
    "{}\n{}\n{}",
    task_context,      // 任务链（保障延续性）
    "[COMPRESSED_CONTEXT]",  // 分隔符
    compressed         // 压缩后的记忆（节省空间）
);

// ========== 第六步：发送给 Agent ==========
let response = agent.generate(&final_context).await?;
```

---

## 性能预测

### 压缩效果

| 场景 | 原始 tokens | 压缩后 tokens | 节省率 | 任务延续性 |
|------|-------------|---------------|--------|------------|
| **短任务**（50 条记忆） | 50K | 18K | 64% | ✅ 高 |
| **中等任务**（200 条记忆） | 150K | 42K | 72% | ✅ 高 |
| **长任务**（500 条记忆） | 400K | 65K | 84% | ⚠️ 中 |

### 去缩后质量对比

| 方案 | 语义保留度 | 任务延续性 | 空间节省 |
|------|-----------|-----------|---------|
| **时间窗口滑动** | 🔴 60% | 🔴 低 | ✅ 80% |
| **AI 摘要生成** | 🔴 70% | 🔴 中 | ✅ 75% |
| **本方案（分层压缩）** | ✅ 85% | ✅ 高 | ✅ 70% |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 压缩比过高导致失真 | 任务延续性下降 | 自适应调整，逐步增加压缩比 |
| 语义去重误删关键信息 | 遗漏重要上下文 | DBSCAN min_points 参数，保守阈值 |
| 极度压缩后向量检索失效 | Agent 理解偏差 | 保留 <VEC:id> 标记，支持向量检索回溯 |
| 压缩算法性能开销 | 延迟增加 | 批量处理，并行聚类 |

---

## 实施计划

### Phase 1: 分层压缩 (P1.3.1)
- [ ] 实现 `CompressionConfig`
- [ ] 实现 `compress_context_layered()`
- [ ] 实现各级压缩算法（Light/Medium/Heavy/Extreme）
- [ ] 单元测试

### Phase 2: 语义去重 (P1.3.2)
- [ ] 实现 DBSCAN 聚类
- [ ] 实现 `semantic_dedup_compress()`
- [ ] 性能优化（并行相似度计算）
- [ ] 基准测试

### Phase 3: 自适应调整 (P1.3.3)
- [ ] 实现 `adaptive_compress()`
- [ ] 参数收敛算法优化
- [ ] 压缩质量评估

### Phase 4: 任务延续性 (P1.3.4)
- [ ] 实现 `TaskChain` 追踪
- [ ] 实现关键信息锚点提取
- [ ] 集成到 Agent 生成流程

---

**维护者**: CIS v1.1.6 Team
**最后更新**: 2026-02-13
