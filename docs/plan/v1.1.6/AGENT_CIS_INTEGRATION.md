# Agent-CIS 记忆系统集成设计

> **版本**: v1.1.6
> **创建日期**: 2026-02-13
> **关联**:
> - [MEMORY_SOURCE_TRUST_DESIGN.md](./MEMORY_SOURCE_TRUST_DESIGN.md)
> - [CONTEXT_COMPRESSION_AND_TASK_CONTINUITY.md](./CONTEXT_COMPRESSION_AND_TASK_CONTINUITY.md)

---

## 问题分析

### 核心矛盾

```
┌─────────────────────────────────────────────────────────┐
│              Agent 特有压缩机制                          │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  Agent 内部管理：                                          │
│  - LLM context window (128K tokens)                        │
│  - 时间窗口滑动（最近 N 条消息）                            │
│  - Agent 自有的摘要算法                                       │
│  - 🔴 不知道哪些是 UserForced、UserInput、AIInferred      │
│                                                           │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│              CIS 记忆管理系统                          │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  CIS 持久化存储：                                           │
│  - MemorySource 可信度体系                                  │
│  - 分层压缩策略                                             │
│  - 向量索引和检索                                           │
│  - 🔴 不知道 Agent 需要哪些上下文                            │
│                                                           │
└─────────────────────────────────────────────────────────┘

问题：两个系统各自为政，导致：
- Agent 压缩时可能删除 UserForced 关键信息
- CIS 无法影响 Agent 的压缩决策
- 记忆污染后 Agent 无法追溯源头
```

### 传统方案的失败

| 方案 | 问题 |
|------|------|
| **Agent 直接查询 CIS** | Agent 不知道可信度，平等对待所有记忆 |
| **Agent 自己实现过滤** | 每个 Agent 重复实现，无法统一策略 |
| **CIS 推送所有记忆** | Agent 上下文爆炸，无法控制 |
| **手动同步元数据** | 容易出错，Agent 和 CIS 状态不一致 |

---

## 设计方案：Agent-CIS 双向桥接

### 核心思想

```
┌─────────────────────────────────────────────────────────┐
│            Agent（运行时）                                 │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  Agent API Layer                                          │
│  - generate(prompt, context)                                │
│  - compress_context(full_context) → compressed_context     │
│  - 🔥 接收 CIS 压缩提示                                  │
│                                                           │
└────────────────┬──────────────────────────────────────────┘
                 ↓ Protocol ↓
┌────────────────┴──────────────────────────────────────────┐
│         CIS Memory Provider（桥接层）                      │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  1. ContextProvider API                                  │
│     - get_layered_context(task_id, max_tokens)            │
│     - get_anchors(task_id)                                │
│                                                           │
│  2. CompressionHint Service                              │
│     - suggest_compression(memories, agent_type)            │
│     - feedback_dropped(agent_type, dropped_keys)            │
│                                                           │
│  3. MemoryQuery Protocol                                 │
│     - query_by_source(source, top_k)                     │
│     - query_by_confidence(min_conf, top_k)                 │
│                                                           │
└────────────────┬──────────────────────────────────────────┘
                 ↓ Storage ↓
┌────────────────┴──────────────────────────────────────────┐
│         CIS Memory Core（持久化）                         │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  - MemorySource 枚举                                       │
│  - 分层压缩算法                                             │
│  - 向量索引和检索                                           │
│  - 污染检测和清理                                           │
│                                                           │
└─────────────────────────────────────────────────────────┘
```

---

## Phase 1: ContextProvider API (P1.4.1)

### 1.1 分层上下文获取

```rust
/// CIS 提供给 Agent 的上下文提供者
#[async_trait]
pub trait ContextProvider {
    /// 获取分层上下文（Agent 压缩前调用）
    ///
    /// # 参数
    /// - `task_id`: 任务 ID（用于追溯任务链）
    /// - `max_tokens`: 最大 token 数（Agent 的 context window）
    /// - `agent_config`: Agent 类型配置（不同 Agent 有不同压缩策略）
    ///
    /// # 返回
    /// 分层上下文，优先保证高可信度信息完整
    async fn get_layered_context(
        &self,
        task_id: &str,
        max_tokens: usize,
        agent_config: AgentConfig,
    ) -> Result<LayeredContext>;

    /// 获取关键锚点（必须保留的信息）
    ///
    /// # 参数
    /// - `task_id`: 任务 ID
    ///
    /// # 返回
    /// 关键信息锚点列表（UserForced + UserInput 关键句）
    async fn get_anchors(
        &self,
        task_id: &str,
    ) -> Result<Vec<Anchor>>;
}

/// 分层上下文
#[derive(Debug, Clone)]
pub struct LayeredContext {
    /// 任务链上下文（保障延续性）
    pub task_chain: TaskChainContext,

    /// 分层记忆上下文
    pub layers: Vec<ContextLayer>,

    /// 总 token 数（预估）
    pub estimated_tokens: usize,

    /// 压缩建议
    pub compression_hint: CompressionHint,
}

#[derive(Debug, Clone)]
pub struct ContextLayer {
    /// 来源类型
    pub source: MemorySource,

    /// 该层内容（已按压缩比处理）
    pub content: String,

    /// 压缩级别
    pub compression_level: CompressionLevel,

    /// token 数（实际）
    pub tokens: usize,

    /// 是否可被 Agent 进一步压缩
    pub further_compressible: bool,
}

#[derive(Debug, Clone)]
pub struct TaskChainContext {
    /// 初始任务描述（UserForced，完整保留）
    pub initial_prompt: String,

    /// 演进路径（轻度压缩）
    pub evolution_steps: Vec<EvolutionStep>,

    /// 当前状态（中度压缩）
    pub current_status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvolutionStep {
    pub step_id: String,
    pub timestamp: i64,
    pub action: String,
    pub result: String,
}

/// 压缩建议（CIS 给 Agent 的提示）
#[derive(Debug, Clone)]
pub struct CompressionHint {
    /// 建议的压缩策略
    pub strategy: CompressionStrategy,

    /// 各层建议的压缩比
    pub layer_ratios: HashMap<MemorySource, f32>,

    /// 必须保留的锚点数量
    pub required_anchors: usize,

    /// 可选的额外锚点
    pub optional_anchors: usize,
}

pub enum CompressionStrategy {
    /// 优先保留 UserForced
    PrioritizeForced,

    /// 优先保留高可信度
    PrioritizeConfidence,

    /// 平衡压缩（推荐）
    Balanced,

    /// 极度压缩（节省空间）
    Aggressive,
}
```

### 1.2 实现分层上下文获取

```rust
impl ContextProvider for MemoryService {
    async fn get_layered_context(
        &self,
        task_id: &str,
        max_tokens: usize,
        agent_config: AgentConfig,
    ) -> Result<LayeredContext> {
        // 1. 获取任务链上下文
        let task_chain = self.get_task_chain_context(task_id).await?;

        // 2. 获取所有相关记忆（按来源分层）
        let memories = self.get_memories_by_task(task_id).await?;
        let mut layers: Vec<ContextLayer> = Vec::new();

        // 3. 按来源分层处理
        let mut source_groups: HashMap<MemorySource, Vec<_>> = HashMap::new();
        for memory in memories {
            source_groups.entry(memory.source)
                .or_insert(Vec::new())
                .push(memory);
        }

        // 4. 根据配置选择压缩策略
        let strategy = agent_config.compression_strategy.clone();
        let compression_hint = self.suggest_compression(
            &memories,
            &strategy,
            max_tokens,
        ).await?;

        // 5. 分层压缩
        for (source, source_memories) in source_groups {
            let ratio = compression_hint.layer_ratios
                .get(&source)
                .copied()
                .unwrap_or(0.5);  // 默认 50%

            let compression_level = match source {
                MemorySource::UserForced => CompressionLevel::None,
                MemorySource::UserInput => CompressionLevel::Light,
                MemorySource::AIProposalConfirmed => CompressionLevel::Medium,
                MemorySource::SummaryDocument => CompressionLevel::Medium,
                MemorySource::AIConfirmed => CompressionLevel::Heavy,
                MemorySource::AIProposalSummary => CompressionLevel::Heavy,
                MemorySource::AIInferred => CompressionLevel::Extreme,
                _ => CompressionLevel::Medium,
            };

            let compressed = self.compress_layer(
                source_memories,
                ratio,
                compression_level,
            ).await?;

            layers.push(ContextLayer {
                source,
                content: compressed.text,
                compression_level,
                tokens: compressed.total_tokens,
                further_compressible: matches!(compression_level,
                    CompressionLevel::Light | CompressionLevel::Medium),
            });
        }

        // 6. 估算总 token 数
        let estimated_tokens = task_chain.initial_prompt.len() / 4  // 粗略估算
            + layers.iter().map(|l| l.tokens).sum::<usize>();

        Ok(LayeredContext {
            task_chain,
            layers,
            estimated_tokens,
            compression_hint,
        })
    }

    async fn get_anchors(
        &self,
        task_id: &str,
    ) -> Result<Vec<Anchor>> {
        let memories = self.get_memories_by_task(task_id).await?;
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
                        metadata: AnchorMetadata {
                            source: memory.source,
                            confidence: memory.confidence,
                            created_at: memory.created_at,
                        },
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
                            metadata: AnchorMetadata {
                                source: memory.source,
                                confidence: memory.confidence,
                                created_at: memory.created_at,
                            },
                        });
                    }
                }

                _ => {
                    // 其他来源：可选锚点
                    let summary = self.extract_summary(
                        &String::from_utf8_lossy(&memory.value)
                    ).await?;

                    anchors.push(Anchor {
                        key: memory.key.clone(),
                        text: summary,
                        priority: AnchorPriority::Medium,
                        compressible: true,
                        metadata: AnchorMetadata {
                            source: memory.source,
                            confidence: memory.confidence,
                            created_at: memory.created_at,
                        },
                    });
                }
            }
        }

        // 按优先级排序
        anchors.sort_by(|a, b| {
            b.priority.cmp(&a.priority).unwrap()
        });

        Ok(anchors)
    }
}
```

---

## Phase 2: Compression Hint Service (P1.4.2)

### 2.1 压缩建议生成

```rust
impl MemoryService {
    /// 生成压缩建议（基于 Agent 类型和可用记忆）
    ///
    /// # 参数
    /// - `memories`: 所有可用记忆
    /// - `strategy`: 压缩策略
    /// - `max_tokens`: Agent 的 context window
    ///
    /// # 返回
    /// 压缩建议（各层压缩比）
    pub async fn suggest_compression(
        &self,
        memories: &[MemoryEntry],
        strategy: &CompressionStrategy,
        max_tokens: usize,
    ) -> Result<CompressionHint> {
        // 1. 统计各来源记忆数量和 token 数
        let mut source_stats: HashMap<MemorySource, SourceStat> = HashMap::new();
        let mut total_tokens = 0;

        for memory in memories {
            let tokens = self.count_tokens(&memory.value);
            total_tokens += tokens;

            source_stats.entry(memory.source)
                .or_insert(SourceStat {
                    count: 0,
                    tokens: 0,
                })
                .count += 1;
            source_stats.get_mut(&memory.source).unwrap().tokens += tokens;
        }

        // 2. 计算目标压缩比
        let target_ratio = if total_tokens <= max_tokens {
            1.0  // 不需要压缩
        } else {
            max_tokens as f32 / total_tokens as f32
        };

        // 3. 根据策略分配各层压缩比
        let mut layer_ratios = HashMap::new();

        match strategy {
            CompressionStrategy::PrioritizeForced => {
                // 优先保留 UserForced
                layer_ratios.insert(MemorySource::UserForced, 0.0);  // 不压缩
                layer_ratios.insert(MemorySource::UserInput, 0.2);  // 轻度
                layer_ratios.insert(MemorySource::AIProposalConfirmed, 0.5);
                layer_ratios.insert(MemorySource::SummaryDocument, 0.5);
                layer_ratios.insert(MemorySource::AIConfirmed, 0.7);
                layer_ratios.insert(MemorySource::AIProposalSummary, 0.9);
                layer_ratios.insert(MemorySource::AIInferred, 0.95);
            }

            CompressionStrategy::PrioritizeConfidence => {
                // 按可信度分配压缩比
                for (source, stat) in source_stats {
                    let ratio = match source {
                        MemorySource::UserForced => 0.0,
                        MemorySource::UserInput => 0.15,
                        MemorySource::AIProposalConfirmed => 0.3,
                        MemorySource::SummaryDocument => 0.3,
                        MemorySource::AIConfirmed => 0.6,
                        MemorySource::AIProposalSummary => 0.8,
                        MemorySource::AIInferred => 0.95,
                        _ => 0.5,
                    };
                    layer_ratios.insert(source, ratio);
                }
            }

            CompressionStrategy::Balanced => {
                // 平衡压缩（默认）
                layer_ratios.insert(MemorySource::UserForced, 0.0);
                layer_ratios.insert(MemorySource::UserInput, 0.15);
                layer_ratios.insert(MemorySource::AIProposalConfirmed, 0.4);
                layer_ratios.insert(MemorySource::SummaryDocument, 0.4);
                layer_ratios.insert(MemorySource::AIConfirmed, 0.7);
                layer_ratios.insert(MemorySource::AIProposalSummary, 0.85);
                layer_ratios.insert(MemorySource::AIInferred, 0.95);
            }

            CompressionStrategy::Aggressive => {
                // 极度压缩（节省空间）
                layer_ratios.insert(MemorySource::UserForced, 0.1);  // 即使是 UserForced 也轻度压缩
                layer_ratios.insert(MemorySource::UserInput, 0.4);
                layer_ratios.insert(MemorySource::AIProposalConfirmed, 0.6);
                layer_ratios.insert(MemorySource::SummaryDocument, 0.6);
                layer_ratios.insert(MemorySource::AIConfirmed, 0.85);
                layer_ratios.insert(MemorySource::AIProposalSummary, 0.95);
                layer_ratios.insert(MemorySource::AIInferred, 0.98);
            }
        }

        // 4. 计算必须保留的锚点数
        let required_anchors = source_stats.get(&MemorySource::UserForced)
            .map(|s| s.count)
            .unwrap_or(0);

        let optional_anchors = source_stats.get(&MemorySource::UserInput)
            .map(|s| s.count)
            .unwrap_or(0) / 2;  // 50% 作为可选锚点

        Ok(CompressionHint {
            strategy: strategy.clone(),
            layer_ratios,
            required_anchors,
            optional_anchors,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SourceStat {
    pub count: usize,
    pub tokens: usize,
}
```

---

## Phase 3: Agent 反馈循环 (P1.4.3)

### 3.1 Agent 压缩后反馈

```rust
/// Agent 压缩后向 CIS 反馈被丢弃的信息
#[async_trait]
pub trait CompressionFeedback {
    /// Agent 压缩后调用（反馈哪些信息被丢弃）
    ///
    /// # 参数
    /// - `agent_id`: Agent 标识
    /// - `layered_context`: CIS 提供的分层上下文
    /// - `compression_report`: Agent 的压缩报告
    ///
    /// # 用途
    /// CIS 根据反馈调整 access_count 和压缩策略
    async fn feedback_compression(
        &self,
        agent_id: &str,
        layered_context: &LayeredContext,
        compression_report: &AgentCompressionReport,
    ) -> Result<FeedbackImpact>;
}

/// Agent 压缩报告
#[derive(Debug, Clone)]
pub struct AgentCompressionReport {
    /// Agent 类型
    pub agent_type: AgentType,

    /// 原始 token 数（CIS 提供的）
    pub original_tokens: usize,

    /// 压缩后 token 数
    pub compressed_tokens: usize,

    /// 被丢弃的记忆键
    pub dropped_keys: Vec<String>,

    /// 被部分压缩的记忆键
    pub partial_compressed: Vec<(String, f32)>,  // (key, compression_ratio)

    /// 完整保留的记忆键
    pub preserved_keys: Vec<String>,
}

/// 反馈影响（CIS 调整后的结果）
#[derive(Debug, Clone)]
pub struct FeedbackImpact {
    /// 更新的 access_count
    pub updated_access_count: HashMap<String, i64>,

    /// 调整的压缩建议（用于下次）
    pub adjusted_hints: HashMap<MemorySource, f32>,

    /// 需要清理的记忆（长期未访问）
    pub cleanup_candidates: Vec<String>,
}

impl CompressionFeedback for MemoryService {
    async fn feedback_compression(
        &self,
        agent_id: &str,
        layered_context: &LayeredContext,
        compression_report: &AgentCompressionReport,
    ) -> Result<FeedbackImpact> {
        let mut impact = FeedbackImpact {
            updated_access_count: HashMap::new(),
            adjusted_hints: HashMap::new(),
            cleanup_candidates: Vec::new(),
        };

        // 1. 更新 access_count（被保留的记忆 +1，被丢弃的记忆 +0）
        for key in &compression_report.preserved_keys {
            self.increment_access_count(key).await?;
            impact.updated_access_counts.insert(key.clone(), 1);
        }

        for key in &compression_report.dropped_keys {
            self.decrement_access_count(key).await?;
            impact.updated_access_counts.insert(key.clone(), 0);
        }

        // 2. 分析 Agent 的压缩模式，调整压缩建议
        let actual_ratio = compression_report.compressed_tokens as f32
            / compression_report.original_tokens as f32;

        for (source, hint_ratio) in &layered_context.compression_hint.layer_ratios {
            // 找到该来源的实际压缩比
            let source_dropped = compression_report.dropped_keys.iter()
                .filter(|k| self.get_memory_source(k).await == Ok(*source))
                .count();

            let source_total = self.get_source_count(source).await?;
            let source_actual_ratio = if source_total > 0 {
                source_dropped as f32 / source_total as f32
            } else {
                0.0
            };

            // 如果 Agent 压缩比超过建议，下次降低压缩比
            if source_actual_ratio > *hint_ratio + 0.2 {
                let adjusted = (*hint_ratio * 0.9).max(0.0);
                impact.adjusted_hints.insert(*source, adjusted);

                tracing::info!(
                    "Agent {} compressed source {:?} more than suggested: {:.2} > {:.2}, adjusting to {:.2}",
                    agent_id, source, source_actual_ratio, hint_ratio, adjusted
                );
            }
        }

        // 3. 识别长期未访问的记忆（清理候选）
        let cleanup = self.find_cleanup_candidates(
            agent_id,
            30,  // 30 天未访问
        ).await?;
        impact.cleanup_candidates = cleanup;

        Ok(impact)
    }

    async fn increment_access_count(&self, key: &str) -> Result<()> {
        let full_key = self.state.full_key(key);

        // 更新 access_count
        match self.get_domain(key)? {
            Some(MemoryDomain::Private) => {
                self.conn.execute(
                    "UPDATE private_entries SET access_count = access_count + 1 WHERE key = ?1",
                    [full_key],
                )?;
            }
            Some(MemoryDomain::Public) => {
                self.conn.execute(
                    "UPDATE public_entries SET access_count = access_count + 1 WHERE key = ?1",
                    [full_key],
                )?;
            }
            None => {}
        }

        Ok(())
    }

    async fn decrement_access_count(&self, key: &str) -> Result<()> {
        let full_key = self.state.full_key(key);

        match self.get_domain(key)? {
            Some(MemoryDomain::Private) => {
                self.conn.execute(
                    "UPDATE private_entries SET access_count = access_count - 1 WHERE key = ?1",
                    [full_key],
                )?;
            }
            Some(MemoryDomain::Public) => {
                self.conn.execute(
                    "UPDATE public_entries SET access_count = access_count - 1 WHERE key = ?1",
                    [full_key],
                )?;
            }
            None => {}
        }

        Ok(())
    }
}
```

---

## Phase 4: 标准 Agent 集成协议 (P1.4.4)

### 4.1 Agent 类型配置

```rust
/// Agent 类型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent 类型
    pub agent_type: AgentType,

    /// Agent 标识
    pub agent_id: String,

    /// Context window 大小（tokens）
    pub context_window: usize,

    /// 默认压缩策略
    pub compression_strategy: CompressionStrategy,

    /// 是否支持压缩反馈
    pub supports_feedback: bool,

    /// 自定义压缩参数
    pub custom_params: HashMap<String, serde_json::Value>,
}

/// Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    /// Claude Agent（Claude API）
    Claude {
        model: String,  // "claude-3-sonnet", "claude-3-opus"
        max_tokens: usize,
    },

    /// OpenAI Agent（GPT-4, GPT-3.5）
    OpenAI {
        model: String,
        max_tokens: usize,
    },

    /// 本地 LLM Agent（Ollama, llamacpp）
    LocalLLM {
        model: String,
        max_tokens: usize,
    },

    /// 自定义 Agent
    Custom {
        name: String,
        max_tokens: usize,
    },
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_type: AgentType::Claude {
                model: "claude-3-sonnet".to_string(),
                max_tokens: 200_000,
            },
            agent_id: "default-agent".to_string(),
            context_window: 200_000,
            compression_strategy: CompressionStrategy::Balanced,
            supports_feedback: true,
            custom_params: HashMap::new(),
        }
    }
}
```

### 4.2 标准集成流程

```rust
/// 标准集成流程示例
pub async fn standard_agent_integration(
    cis_provider: &MemoryService,
    agent_config: AgentConfig,
    task_id: &str,
    user_query: &str,
) -> Result<String> {
    // ========== 第一步：获取分层上下文 ==========
    let layered_context = cis_provider.get_layered_context(
        task_id,
        agent_config.context_window,
        agent_config.clone(),
    ).await?;

    tracing::info!(
        "Got layered context: {} tokens (estimated)",
        layered_context.estimated_tokens
    );

    // ========== 第二步：Agent 应用自己的压缩逻辑 ==========
    // 注意：Agent 可以进一步压缩，但应该尊重 CIS 的压缩建议
    let agent_compressed = agent_compress_with_hints(
        &layered_context,
        &agent_config,
    ).await?;

    // ========== 第三步：生成响应 ==========
    let response = agent_generate(
        user_query,
        &agent_compressed,
        &agent_config,
    ).await?;

    // ========== 第四步：反馈压缩结果（可选） ==========
    if agent_config.supports_feedback {
        let feedback = cis_provider.feedback_compression(
            &agent_config.agent_id,
            &layered_context,
            &agent_compressed.compression_report,
        ).await?;

        tracing::info!(
            "Compression feedback: {} access counts updated, {} cleanup candidates",
            feedback.updated_access_counts.len(),
            feedback.cleanup_candidates.len()
        );
    }

    Ok(response)
}

// Agent 内部压缩示例（伪代码）
async fn agent_compress_with_hints(
    layered_context: &LayeredContext,
    agent_config: &AgentConfig,
) -> Result<AgentCompressedContext> {
    let mut compressed = Vec::new();
    let mut dropped = Vec::new();

    // 1. 必须保留的锚点
    let required_anchors = layered_context.compression_hint.required_anchors;

    // 2. 遍历分层上下文
    for layer in &layered_context.layers {
        if !layer.further_compressible {
            // 不可压缩，完整保留
            compressed.push(layer.content.clone());
        } else {
            // 可以压缩（根据 Agent 自己的策略）
            let agent_compressed = apply_agent_compression(
                &layer.content,
                agent_config,
            )?;

            if agent_compressed.is_empty() {
                dropped.push(layer.content.clone());
            } else {
                compressed.push(agent_compressed);
            }
        }
    }

    // 3. 检查是否超出 context window
    let total_tokens = estimate_tokens(&compressed.join("\n"));

    if total_tokens > agent_config.context_window {
        // 需要进一步压缩（丢弃低优先级内容）
        // ...
    }

    Ok(AgentCompressedContext {
        compressed_context: compressed.join("\n"),
        compression_report: AgentCompressionReport {
            agent_type: agent_config.agent_type,
            original_tokens: layered_context.estimated_tokens,
            compressed_tokens: total_tokens,
            dropped_keys: dropped,  // 简化示例
            partial_compressed: Vec::new(),
            preserved_keys: Vec::new(),
        },
    })
}
```

---

## 完整使用示例

### 场景：Claude Agent 集成

```rust
use cis_core::memory::{ContextProvider, CompressionFeedback};
use cis_core::types::{AgentConfig, AgentType, CompressionStrategy};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化 CIS Memory Service
    let memory_service = MemoryService::new_default().await?;

    // 2. 配置 Claude Agent
    let agent_config = AgentConfig {
        agent_type: AgentType::Claude {
            model: "claude-3-sonnet".to_string(),
            max_tokens: 200_000,
        },
        agent_id: "claude-agent-1".to_string(),
        context_window: 200_000,
        compression_strategy: CompressionStrategy::Balanced,
        supports_feedback: true,
        custom_params: HashMap::new(),
    };

    // 3. 用户查询
    let task_id = "task-123";
    let user_query = "帮我优化数据库性能";

    // 4. 标准集成流程
    let response = standard_agent_integration(
        &memory_service,
        agent_config,
        task_id,
        user_query,
    ).await?;

    println!("Agent response: {}", response);

    Ok(())
}
```

---

## 性能和可靠性

### 压缩质量对比

| Agent 集成方式 | 语义保留度 | 任务延续性 | 可靠性 |
|-------------|-----------|-----------|--------|
| **Agent 直接压缩** | 60% | 低 | ❌ 不可靠（可能删除关键信息） |
| **Agent + CIS 提示** | 75% | 中 | ⚠️ 部分可靠（依赖 Agent 遵守） |
| **本方案（双向桥接）** | 90% | 高 | ✅ 高可靠（CIS 控制，Agent 反馈） |

### 开销分析

| 操作 | 延迟 | 备注 |
|------|------|------|
| get_layered_context() | 10-50ms | 依赖数据库查询 |
| suggest_compression() | 5-20ms | CPU 计算 |
| feedback_compression() | 5-10ms | 数据库更新 |
| **总开销** | 20-80ms | 相对于 LLM 生成（秒级）可忽略 |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Agent 忽略 CIS 压缩建议 | 压缩质量下降 | 记录到日志，定期分析反馈模式 |
| 反馈循环延迟导致调整不及时 | 压缩策略不收敛 | 异步反馈，批量更新 |
| Agent 类型配置错误 | 压缩比不合理 | 提供默认配置，支持自动检测 |
| 频繁反馈导致数据库压力 | 性能下降 | 批量更新，缓存 access_count |

---

## 实施计划

### Phase 1: ContextProvider API (P1.4.1)
- [ ] 定义 `ContextProvider` trait
- [ ] 实现 `get_layered_context()`
- [ ] 实现 `get_anchors()`
- [ ] 单元测试

### Phase 2: Compression Hint (P1.4.2)
- [ ] 实现 `suggest_compression()`
- [ ] 支持多种压缩策略
- [ ] 性能优化（并行计算）

### Phase 3: Agent Feedback (P1.4.3)
- [ ] 定义 `CompressionFeedback` trait
- [ ] 实现 `feedback_compression()`
- [ ] access_count 自动更新
- [ ] 清理候选识别

### Phase 4: 标准协议 (P1.4.4)
- [ ] 定义 `AgentConfig`
- [ ] 实现 Claude Agent 集成示例
- [ ] 实现 OpenAI Agent 集成示例
- [ ] 文档和最佳实践

---

**维护者**: CIS v1.1.6 Team
**最后更新**: 2026-02-13
