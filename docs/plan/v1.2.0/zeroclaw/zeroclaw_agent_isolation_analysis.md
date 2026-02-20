# ZeroClaw Agent 隔离与记忆分组实现分析

> **分析日期**: 2026-02-20
> **分析对象**: zeroclaw-labs/zeroclaw (本地克隆版本)
> **分析重点**: Agent 拆分、记忆分组、降低幻觉
> **分析人**: GLM

---

## 1. 执行摘要

ZeroClaw 采用了**单 Agent + 记忆隔离**的架构模式，而非"多 Agent 实例"模式。核心思想是：
- **一个 Agent 实例**处理所有任务
- 通过 **session_id** 和 **MemoryCategory** 实现记忆分组
- 通过 **记忆相关性过滤**和**不可信记忆过滤**降低幻觉
- 通过 **查询分类**动态路由到不同模型

**关键发现**：
1. ✅ **不是多 Agent 实例**：ZeroClaw 没有为每个任务创建独立的 Agent 实例
2. ✅ **记忆隔离机制完善**：通过 `session_id` + `MemoryCategory` 实现分组
3. ✅ **防幻觉机制多重**：相关性过滤、不可信记忆过滤、上下文压缩
4. ⚠️ **适合单人场景**：当前设计不支持真正的多用户并发隔离

---

## 2. Agent 架构设计

### 2.1 Agent 结构体

**文件**: `zeroclaw/src/agent/agent.rs`

```rust
pub struct Agent {
    provider: Box<dyn Provider>,
    tools: Vec<Box<dyn Tool>>,
    tool_specs: Vec<ToolSpec>,
    memory: Arc<dyn Memory>,           // ← 关键：共享记忆实例
    observer: Arc<dyn Observer>,
    prompt_builder: SystemPromptBuilder,
    tool_dispatcher: Box<dyn ToolDispatcher>,
    memory_loader: Box<dyn MemoryLoader>, // ← 记忆加载器
    config: crate::config::AgentConfig,
    model_name: String,
    temperature: f64,
    workspace_dir: std::path::PathBuf,
    identity_config: crate::config::IdentityConfig,
    skills: Vec<crate::skills::Skill>,
    auto_save: bool,
    history: Vec<ConversationMessage>,  // ← 对话历史（Agent 级别）
    classification_config: crate::config::QueryClassificationConfig,
    available_hints: Vec<String>,
}
```

**关键观察**：
1. **单例模式**：Agent 是一个独立的服务实例，不是池化的对象
2. **共享记忆**：所有任务共享同一个 `Arc<dyn Memory>` 实例
3. **内置历史**：`history` 字段存储对话历史，不是持久的
4. **可配置的分类器**：`classification_config` 用于动态路由任务

### 2.2 Agent 构建模式

**Builder Pattern**:
```rust
pub struct AgentBuilder {
    provider: Option<Box<dyn Provider>>,
    tools: Option<Vec<Box<dyn Tool>>>,
    memory: Option<Arc<dyn Memory>>,
    observer: Option<Arc<dyn Observer>>,
    // ...
}

impl AgentBuilder {
    pub fn provider(mut self, provider: Box<dyn Provider>) -> Self { ... }
    pub fn tools(mut self, tools: Vec<Box<dyn Tool>>) -> Self { ... }
    pub fn memory(mut self, memory: Arc<dyn Memory>) -> Self { ... }
    // ...

    pub fn build(self) -> Result<Agent> {
        // 验证必需字段
        let tools = self.tools.ok_or_else(|| anyhow::anyhow!("tools are required"))?;
        let provider = self.provider.ok_or_else(|| anyhow::anyhow!("provider is required"))?;
        let memory = self.memory.ok_or_else(|| anyhow::anyhow!("memory is required"))?;
        // ...
    }
}
```

**从配置创建**:
```rust
impl Agent {
    pub fn from_config(config: &Config) -> Result<Self> {
        // 1. 创建 observer
        let observer: Arc<dyn Observer> = Arc::from(observability::create_observer(&config.observability));

        // 2. 创建 runtime
        let runtime: Arc<dyn runtime::RuntimeAdapter> = Arc::from(runtime::create_runtime(&config.runtime)?);

        // 3. 创建 security policy
        let security = Arc::new(SecurityPolicy::from_config(&config.autonomy, &config.workspace_dir));

        // 4. 创建 memory（带存储和路由）
        let memory: Arc<dyn Memory> = Arc::from(memory::create_memory_with_storage_and_routes(
            &config.memory,
            &config.embedding_routes,
            Some(&config.storage.provider.config),
            &config.workspace_dir,
            config.api_key.as_deref(),
        )?);

        // 5. 创建 tools（依赖 runtime, memory, security）
        let tools = tools::all_tools_with_runtime(
            Arc::new(config.clone()),
            &security,
            runtime,
            memory.clone(), // ← tools 也共享 memory
            // ...
        );

        // 6. 创建 provider
        let provider: Box<dyn Provider> = providers::create_routed_provider(
            provider_name,
            config.api_key.as_deref(),
            config.api_url.as_deref(),
            &config.reliability,
            &config.model_routes,
            &model_name,
        )?;

        // 7. 组装 Agent
        Agent::builder()
            .provider(provider)
            .tools(tools)
            .memory(memory)
            .observer(observer)
            // ...
            .build()
    }
}
```

**关键设计决策**：
1. **依赖注入**：所有组件通过依赖注入组装
2. **共享 Arc**：`memory`, `observer` 使用 `Arc` 共享
3. **工具共享记忆**：`memory_store`, `memory_recall` 等工具也共享同一个 memory 实例

---

## 3. 记忆分组与隔离机制

### 3.1 Memory Trait 定义

**文件**: `zeroclaw/src/memory/traits.rs`

```rust
#[async_trait]
pub trait Memory: Send + Sync {
    /// Backend name
    fn name(&self) -> &str;

    /// Store a memory entry, optionally scoped to a session
    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>, // ← 关键：session_id 用于隔离
    ) -> anyhow::Result<()>;

    /// Recall memories matching a query (keyword search), optionally scoped to a session
    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>, // ← 关键：可按 session 过滤
    ) -> anyhow::Result<Vec<MemoryEntry>>;

    /// Get a specific memory by key
    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>>;

    /// List all memory keys, optionally filtered by category and/or session
    async fn list(
        &self,
        category: Option<&MemoryCategory>,
        session_id: Option<&str>, // ← 关键：可按 session 列出
    ) -> anyhow::Result<Vec<MemoryEntry>>;

    /// Remove a memory by key
    async fn forget(&self, key: &str) -> anyhow::Result<bool>;

    /// Count total memories
    async fn count(&self) -> anyhow::Result<usize>;

    /// Health check
    async fn health_check(&self) -> bool;
}
```

### 3.2 MemoryEntry 结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub content: String,
    pub category: MemoryCategory,  // ← 分类：Core, Daily, Conversation, Custom
    pub timestamp: String,
    pub session_id: Option<String>, // ← 会话隔离
    pub score: Option<f64>,         // ← 相关性分数
}
```

### 3.3 MemoryCategory 分类

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    /// Long-term facts, preferences, decisions
    Core,
    /// Daily session logs
    Daily,
    /// Conversation context
    Conversation,
    /// User-defined custom category
    Custom(String),
}
```

**分类用途**：
- `Core`：长期事实、用户偏好、决策（跨会话持久）
- `Daily`：日常日志（按日期归档）
- `Conversation`：对话上下文（短期记忆，易过期）
- `Custom(name)`：用户自定义分类（如 "project_a", "team_x"）

### 3.4 session_id 隔离机制

**工作原理**：
1. **存储时**：`memory.store(key, content, category, Some(session_id))`
2. **检索时**：`memory.recall(query, limit, Some(session_id))` - 只返回该 session 的记忆
3. **列出时**：`memory.list(Some(category), Some(session_id))` - 按会话过滤

**实际应用**：
```rust
// 用户 A 的会话
memory.store("user_a_pref", "prefers dark mode", MemoryCategory::Core, Some("session_a")).await?;

// 用户 B 的会话
memory.store("user_b_pref", "prefers light mode", MemoryCategory::Core, Some("session_b")).await?;

// 检索时互不干扰
let a_prefs = memory.recall("theme", 10, Some("session_a")).await?; // 只返回 session_a 的记忆
let b_prefs = memory.recall("theme", 10, Some("session_b")).await?; // 只返回 session_b 的记忆
```

**关键特点**：
- ✅ **逻辑隔离**：通过 `session_id` 实现逻辑隔离
- ✅ **物理共享**：同一个 memory 实例，同一个数据库
- ✅ **可选隔离**：`session_id: Option<&str>` 允许全局记忆（`None`）
- ⚠️ **应用层管理**：`session_id` 由应用层管理，ZeroClaw 不自动生成

---

## 4. 记忆加载与上下文构建

### 4.1 MemoryLoader Trait

**文件**: `zeroclaw/src/agent/memory_loader.rs`

```rust
#[async_trait]
pub trait MemoryLoader: Send + Sync {
    async fn load_context(&self, memory: &dyn Memory, user_message: &str)
        -> anyhow::Result<String>;
}
```

### 4.2 DefaultMemoryLoader 实现

```rust
pub struct DefaultMemoryLoader {
    limit: usize,              // 默认 5 条记忆
    min_relevance_score: f64,  // 默认 0.4 相关性阈值
}

#[async_trait]
impl MemoryLoader for DefaultMemoryLoader {
    async fn load_context(
        &self,
        memory: &dyn Memory,
        user_message: &str,
    ) -> anyhow::Result<String> {
        // 1. 语义搜索相关记忆
        let entries = memory.recall(user_message, self.limit, None).await?;

        if entries.is_empty() {
            return Ok(String::new());
        }

        // 2. 格式化上下文
        let mut context = String::from("[Memory context]\n");
        for entry in entries {
            // 3. 过滤掉不可信的 AI 自动保存记忆
            if memory::is_assistant_autosave_key(&entry.key) {
                continue;
            }

            // 4. 相关性过滤
            if let Some(score) = entry.score {
                if score < self.min_relevance_score {
                    continue;
                }
            }

            // 5. 添加到上下文
            let _ = writeln!(context, "- {}: {}", entry.key, entry.content);
        }

        // 6. 如果所有记忆都被过滤，返回空
        if context == "[Memory context]\n" {
            return Ok(String::new());
        }

        context.push('\n');
        Ok(context)
    }
}
```

### 4.3 不可信记忆过滤

**文件**: `zeroclaw/src/memory/mod.rs`

```rust
/// Legacy auto-save key used for model-authored assistant summaries.
/// These entries are treated as untrusted context and should not be re-injected.
pub fn is_assistant_autosave_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    normalized == "assistant_resp" || normalized.starts_with("assistant_resp_")
}
```

**关键设计**：
1. ✅ **标记不可信记忆**：AI 自动生成的总结标记为 "assistant_resp"
2. ✅ **防止循环幻觉**：避免 AI 生成的内容再次作为上下文
3. ✅ **白名单机制**：只过滤特定 key，其他记忆正常加载

### 4.4 上下文构建流程

**文件**: `zeroclaw/src/agent/loop_.rs`

```rust
/// Build context preamble by searching memory for relevant entries.
/// Entries with a hybrid score below `min_relevance_score` are dropped to
/// prevent unrelated memories from bleeding into the conversation.
async fn build_context(mem: &dyn Memory, user_msg: &str, min_relevance_score: f64) -> String {
    let mut context = String::new();

    // Pull relevant memories for this message
    if let Ok(entries) = mem.recall(user_msg, 5, None).await {
        let relevant: Vec<_> = entries
            .iter()
            .filter(|e| match e.score {
                Some(score) => score >= min_relevance_score, // ← 相关性过滤
                None => true,
            })
            .collect();

        if !relevant.is_empty() {
            context.push_str("[Memory context]\n");
            for entry in &relevant {
                // 过滤掉不可信的 AI 自动保存记忆
                if memory::is_assistant_autosave_key(&entry.key) {
                    continue;
                }
                let _ = writeln!(context, "- {}: {}", entry.key, entry.content);
            }
            if context != "[Memory context]\n" {
                context.push('\n');
            } else {
                context.clear();
            }
        }
    }

    context
}
```

**上下文注入时机**：
```rust
// 在 Agent 主循环中
let context = memory_loader.load_context(&*self.memory, &user_msg).await?;

// 构建完整的用户消息
let full_prompt = format!(
    "{}\n\n[User message]\n{}",
    context,  // ← 记忆上下文在前
    user_msg
);

// 发送给 LLM
let response = self.provider.chat_with_system(
    Some(&system_prompt),
    &full_prompt,
    &self.model_name,
    self.temperature,
).await?;
```

---

## 5. 降低幻觉的机制

### 5.1 多重过滤机制

ZeroClaw 使用 **4 层过滤** 降低幻觉：

#### Layer 1: 语义搜索 + 相关性评分
```rust
// 记忆检索时自动计算相关性分数
let entries = memory.recall(query, limit, None).await?;
// 每个 entry 包含 score: Option<f64>
```

**过滤**:
```rust
let relevant: Vec<_> = entries
    .iter()
    .filter(|e| match e.score {
        Some(score) => score >= 0.4, // ← 低于 0.4 相关性的记忆被丢弃
        None => true,
    })
    .collect();
```

#### Layer 2: 不可信记忆过滤
```rust
// 过滤掉 AI 自动生成的不可信记忆
if memory::is_assistant_autosave_key(&entry.key) {
    continue;
}
```

**防止**:
- AI 生成的总结被当作事实
- 循环幻觉（AI 的输出再次作为输入）

#### Layer 3: 数量限制
```rust
const DEFAULT_LIMIT: usize = 5; // ← 最多加载 5 条相关记忆
```

**防止**:
- 上下文过长导致注意力分散
- 不相关记忆混入

#### Layer 4: 上下文压缩（针对长对话）
```rust
async fn compact_history_if_needed(
    history: &mut Vec<ConversationMessage>,
    provider: &dyn Provider,
    model: &str,
) -> anyhow::Result<bool> {
    // 当历史超过阈值时，压缩旧消息
    if history.len() > COMPACTION_THRESHOLD {
        // 使用 LLM 生成总结
        let summary = provider.chat_with_system(
            Some(summarizer_system),
            &transcript,
            model,
            0.2, // ← 低 temperature 保证总结准确
        ).await?;

        // 用总结替换旧消息
        apply_compaction_summary(history, start, compact_end, &summary);
    }
}
```

### 5.2 查询分类与动态路由

**文件**: `zeroclaw/src/agent/classifier.rs`

```rust
pub fn classify(config: &QueryClassificationConfig, message: &str) -> Option<String> {
    if !config.enabled || config.rules.is_empty() {
        return None;
    }

    let lower = message.to_lowercase();
    let len = message.len();

    // 按优先级排序规则
    let mut rules: Vec<_> = config.rules.iter().collect();
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    for rule in rules {
        // 长度约束
        if let Some(min) = rule.min_length {
            if len < min { continue; }
        }
        if let Some(max) = rule.max_length {
            if len > max { continue; }
        }

        // 关键词匹配（大小写不敏感）
        let keyword_hit = rule.keywords
            .iter()
            .any(|kw| lower.contains(&kw.to_lowercase()));

        // 模式匹配（大小写敏感）
        let pattern_hit = rule.patterns
            .iter()
            .any(|pat| message.contains(pat.as_str()));

        if keyword_hit || pattern_hit {
            return Some(rule.hint.clone()); // ← 返回 model hint
        }
    }

    None
}
```

**配置示例**:
```toml
[query_classification]
enabled = true

[[query_classification.rules]]
hint = "fast"
keywords = ["hello", "hi", "quick"]
max_length = 50
priority = 1

[[query_classification.rules]]
hint = "code"
keywords = ["code", "function", "debug"]
patterns = ["fn ", "impl ", "pub fn"]
min_length = 20
priority = 10

[[query_classification.rules]]
hint = "reasoning"
keywords = ["explain", "analyze", "why"]
min_length = 50
priority = 5
```

**动态路由**:
```rust
// 在 Agent 主循环中
let hint = classify(&self.classification_config, &user_msg);
let model_name = hint
    .and_then(|h| self.resolve_model_hint(&h))
    .unwrap_or_else(|| self.default_model.clone());

// 使用选定的模型
let response = self.provider.chat(&request, &model_name).await?;
```

**路由配置**（model_routes）:
```toml
[[model_routes]]
hint = "fast"
provider = "openrouter"
model = "anthropic/claude-haiku-3.5"

[[model_routes]]
hint = "code"
provider = "openrouter"
model = "anthropic/claude-sonnet-4-6"

[[model_routes]]
hint = "reasoning"
provider = "openrouter"
model = "openai/o1-preview"
```

**优势**：
- ✅ **简单任务用快速模型**：降低成本和延迟
- ✅ **复杂任务用强大模型**：提升准确性和推理能力
- ✅ **自动分类**：用户无需手动选择

### 5.3 幻觉检测的缺失

**⚠️ 重要发现**：ZeroClaw **没有实现显式的幻觉检测机制**

**依赖的策略**：
1. ✅ **相关性过滤**：避免不相关记忆混入
2. ✅ **不可信记忆过滤**：避免 AI 输出循环
3. ✅ **上下文压缩**：避免上下文过长
4. ⚠️ **事后验证缺失**：没有验证 LLM 输出是否与记忆一致
5. ⚠️ **事实核查缺失**：没有引用追踪或来源验证

**潜在改进方向**：
- 添加引用追踪（标记每个事实的来源记忆）
- 添加事后验证（检查 LLM 输出是否与上下文矛盾）
- 添加不确定性标记（让 LLM 标记不确定的内容）

---

## 6. 与 CIS 的对比分析

### 6.1 架构对比

| 维度 | ZeroClaw | CIS (v1.2.0 plan) |
|-----|----------|------------------|
| **Agent 模式** | 单 Agent 实例 | Agent Pool + 多 Runtime |
| **记忆隔离** | `session_id` + `MemoryCategory` | `MemoryDomain` (Private/Public) + `MemoryCategory` |
| **上下文管理** | Agent 内置 `history: Vec<Message>` | 记忆系统持久化，无内置历史 |
| **记忆加载** | `MemoryLoader` trait（语义搜索） | `MemoryVectorIndex` trait（hybrid search） |
| **防幻觉机制** | 相关性过滤 + 不可信记忆过滤 | 待实现 |
| **动态路由** | 查询分类 + model_routes | DAG 四级决策 |

### 6.2 CIS 可以借鉴的设计

#### ✅ 应该借鉴

1. **session_id 隔离机制**
   ```rust
   // CIS 当前没有 session_id
   // 建议添加：
   pub struct MemoryEntry {
       // ...
       pub session_id: Option<String>, // ← 新增
   }

   // Memory trait 增加 session 参数
   async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory, session_id: Option<&str>) -> anyhow::Result<()>;
   async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, session_id: Option<&str>) -> anyhow::Result<Vec<HybridSearchResult>>;
   ```

2. **MemoryLoader trait 模式**
   ```rust
   // cis-traits/src/memory_loader.rs
   #[async_trait]
   pub trait MemoryLoader: Send + Sync {
       async fn load_context(&self, memory: &dyn Memory, user_message: &str, session_id: Option<&str>) -> anyhow::Result<String>;
   }

   // cis-memory/src/loader.rs
   pub struct CisMemoryLoader {
       limit: usize,
       min_relevance_score: f64,
       include_domains: Vec<MemoryDomain>,
       exclude_categories: Vec<MemoryCategory>,
   }
   ```

3. **不可信记忆过滤**
   ```rust
   // cis-memory/src/hygiene.rs
   pub fn is_untrusted_memory_key(key: &str) -> bool {
       let normalized = key.trim().to_ascii_lowercase();
       // AI 生成的总结
       normalized.starts_with("ai_summary_") ||
       normalized.starts_with("assistant_resp_") ||
       // 未验证的外部数据
       normalized.starts_with("external_") && !normalized.contains("_verified_")
   }

   // 在 MemoryLoader 中使用
   if is_untrusted_memory_key(&entry.key) {
       continue; // 跳过不可信记忆
   }
   ```

4. **查询分类与动态路由**
   ```rust
   // cis-scheduler/src/classifier.rs
   pub fn classify_task(user_message: &str, config: &TaskClassificationConfig) -> Option<TaskType> {
       // 简单任务 → Mechanical level
       // 代码任务 → Recommended level
       // 复杂推理 → Confirmed level
   }

   // 配合 DAG 四级决策
   match classify_task(&user_msg, &config) {
       Some(TaskType::Simple) => TaskLevel::Mechanical { retry: 3 },
       Some(TaskType::Code) => TaskLevel::Recommended { default_action: true, timeout_secs: 300 },
       Some(TaskType::Complex) => TaskLevel::Confirmed,
       None => TaskLevel::Arbitrated { stakeholders: vec!["human".into()] },
   }
   ```

#### ⚠️ 不完全适合

1. **单 Agent 模式**
   - ZeroClaw: 单个 Agent 实例处理所有任务
   - CIS: Agent Pool + 多 Runtime（Claude, OpenCode, Kimi）
   - **结论**: CIS 的多 Agent 模式更灵活，不应改变

2. **Agent 内置历史**
   - ZeroClaw: `history: Vec<ConversationMessage>` 在 Agent 内部
   - CIS: 历史持久化在记忆系统中
   - **结论**: CIS 的持久化方案更好，不适合改为内存历史

3. **上下文压缩**
   - ZeroClaw: 当历史过长时，用 LLM 总结
   - CIS: 记忆系统自动归档（54周归档策略）
   - **结论**: CIS 的归档方案更可靠，不需要 LLM 总结

### 6.3 CIS 可以改进的方案

#### 改进 1: 添加 session_id 支持

**当前 CIS**:
```rust
// cis-types/src/memory.rs
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,    // Private | Public
    pub category: MemoryCategory, // Context | Skill | Result | Error
    pub timestamp: DateTime<Utc>,
    pub ttl: Option<u64>,
    // ❌ 没有 session_id
}
```

**建议改进**:
```rust
// cis-types/src/memory.rs
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub timestamp: DateTime<Utc>,
    pub ttl: Option<u64>,
    pub session_id: Option<String>, // ← 新增
    pub tags: Vec<String>,           // ← 新增（用于更灵活的分组）
}
```

**使用场景**:
```rust
// 用户 A 的项目记忆
memory.set(
    "project_a/config",
    b"database_url=postgres://...",
    MemoryDomain::Public,
    MemoryCategory::Context,
    Some("user_a_session"), // ← session 隔离
).await?;

// 用户 B 的项目记忆
memory.set(
    "project_b/config",
    b"database_url=mysql://...",
    MemoryDomain::Public,
    MemoryCategory::Context,
    Some("user_b_session"), // ← session 隔离
).await?;

// 检索时互不干扰
let a_config = memory.get("project_a/config", Some("user_a_session")).await?;
let b_config = memory.get("project_b/config", Some("user_b_session")).await?;
```

#### 改进 2: 添加 MemoryLoader trait

**新增 trait**:
```rust
// cis-traits/src/memory_loader.rs
#[async_trait]
pub trait MemoryLoader: Send + Sync {
    /// Load relevant memory context for a given user message
    async fn load_context(
        &self,
        memory: &dyn Memory,
        user_message: &str,
        session_id: Option<&str>,
    ) -> anyhow::Result<MemoryContext>;
}

pub struct MemoryContext {
    pub entries: Vec<MemoryEntry>,
    pub total_count: usize,
    pub max_score: f64,
    pub min_score: f64,
}
```

**默认实现**:
```rust
// cis-memory/src/loader.rs
pub struct DefaultMemoryLoader {
    limit: usize,
    min_relevance_score: f64,
    include_untrusted: bool,
}

#[async_trait]
impl MemoryLoader for DefaultMemoryLoader {
    async fn load_context(
        &self,
        memory: &dyn Memory,
        user_message: &str,
        session_id: Option<&str>,
    ) -> anyhow::Result<MemoryContext> {
        // 1. Hybrid search
        let results = memory.hybrid_search(
            user_message,
            self.limit,
            None, // domain
            None, // category
            session_id,
        ).await?;

        // 2. Filter by relevance
        let relevant: Vec<_> = results
            .into_iter()
            .filter(|r| r.final_score >= self.min_relevance_score)
            .filter(|r| {
                if self.include_untrusted {
                    true
                } else {
                    !is_untrusted_memory_key(&r.key)
                }
            })
            .collect();

        Ok(MemoryContext {
            total_count: relevant.len(),
            max_score: relevant.first().map(|r| r.final_score).unwrap_or(0.0),
            min_score: relevant.last().map(|r| r.final_score).unwrap_or(0.0),
            entries: relevant.into_iter().map(|r| r.entry).collect(),
        })
    }
}
```

**在 Agent 中使用**:
```rust
// cis-core/src/agent/pool.rs
use cis_traits::MemoryLoader;

pub struct AgentPool {
    loaders: HashMap<String, Box<dyn MemoryLoader>>,
}

impl AgentPool {
    pub async fn execute_task(&self, task: TaskRequest) -> anyhow::TaskResponse> {
        // 1. 获取或创建 agent
        let agent = self.acquire(&task.runtime_type).await?;

        // 2. Load context（使用 session 隔离）
        let context = self
            .loaders
            .get("default")
            .unwrap()
            .load_context(
                &*agent.memory,
                &task.prompt,
                task.session_id.as_deref(),
            )
            .await?;

        // 3. 构建完整 prompt
        let full_prompt = format!(
            "{}\n\n[Task]\n{}",
            format_context(&context),
            task.prompt
        );

        // 4. 执行任务
        agent.execute(&full_prompt).await
    }
}
```

#### 改进 3: 添加不可信记忆过滤

** hygiene 模块**:
```rust
// cis-memory/src/hygiene.rs

/// 判断是否为不可信的记忆 key
pub fn is_untrusted_memory_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();

    // AI 生成的总结（可能包含幻觉）
    if normalized.starts_with("ai_summary_") ||
       normalized.starts_with("assistant_resp_") ||
       normalized.starts_with("llm_generated_") {
        return true;
    }

    // 未验证的外部数据
    if normalized.starts_with("external_") && !normalized.contains("_verified_") {
        return true;
    }

    // 用户标记的不可信内容
    if normalized.starts_with("untrusted_") {
        return true;
    }

    false
}

/// 标记记忆为不可信
pub fn mark_as_untrusted(key: &str) -> String {
    format!("untrusted_{}", key)
}

/// 标记外部数据为已验证
pub fn mark_external_verified(key: &str) -> String {
    format!("external_verified_{}", key)
}
```

**在 MemoryLoader 中使用**:
```rust
// 过滤不可信记忆
let relevant: Vec<_> = results
    .into_iter()
    .filter(|r| {
        // 可信性检查
        if config.exclude_untrusted && is_untrusted_memory_key(&r.key) {
            return false;
        }

        // 相关性检查
        r.final_score >= config.min_relevance_score
    })
    .collect();
```

---

## 7. 最佳实践建议

### 7.1 为不同任务创建不同 Agent

**ZeroClaw 的做法**（不推荐）：
- 单 Agent 实例处理所有任务
- 通过 session_id 隔离记忆
- 通过查询分类路由到不同模型

**CIS 的推荐做法**：
```rust
// 1. 为不同类型的任务创建不同的 Agent
use cis_core::agent::{Agent, AgentConfig, RuntimeType};

// Code Review Agent
let code_review_agent = Agent::builder()
    .runtime(RuntimeType::Claude)
    .model("claude-3-sonnet-4-20250514")
    .system_prompt("You are a code reviewer. Focus on bug detection and best practices.")
    .memoryNamespace("code_review") // ← 独立记忆命名空间
    .build()
    .await?;

// Documentation Agent
let doc_agent = Agent::builder()
    .runtime(RuntimeType::OpenCode)
    .model("glm-4.7-free")
    .system_prompt("You are a technical writer. Focus on clarity and completeness.")
    .memoryNamespace("documentation") // ← 独立记忆命名空间
    .build()
    .await?;

// Debugging Agent
let debug_agent = Agent::builder()
    .runtime(RuntimeType::Kimi)
    .model("kimi-latest")
    .system_prompt("You are a debugging assistant. Focus on root cause analysis.")
    .memoryNamespace("debugging") // ← 独立记忆命名空间
    .build()
    .await?;

// 2. 使用 Agent Pool 管理
let pool = AgentPool::new();
pool.register("code_review", code_review_agent).await;
pool.register("documentation", doc_agent).await;
pool.register("debugging", debug_agent).await;

// 3. 根据任务类型路由到不同的 Agent
match classify_task(&user_message) {
    TaskType::CodeReview => pool.get("code_review").execute(user_message).await,
    TaskType::Documentation => pool.get("documentation").execute(user_message).await,
    TaskType::Debugging => pool.get("debugging").execute(user_message).await,
}
```

**优势**：
- ✅ **真正的隔离**：每个 Agent 有独立的系统提示、模型、记忆命名空间
- ✅ **并行执行**：多个 Agent 可以并行处理不同任务
- ✅ **故障隔离**：一个 Agent 崩溃不影响其他 Agent
- ✅ **灵活配置**：每个 Agent 可以使用不同的 runtime 和模型

### 7.2 读取 Agent 记忆分组后执行任务

**推荐流程**：
```rust
use cis_core::{memory::MemoryService, agent::Agent};

pub async fn execute_task_with_grouped_memory(
    agent: &Agent,
    task: &Task,
    memory: &MemoryService,
) -> anyhow::Result<TaskResult> {
    // 1. 确定记忆分组（session_id）
    let session_id = task.session_id.as_deref()
        .or_else(|| task.metadata.get("session").map(|s| s.as_str()))
        .unwrap_or("default");

    // 2. 加载该 session 的相关记忆
    let memory_context = memory.hybrid_search(
        &task.prompt,
        10, // limit
        Some(MemoryDomain::Public), // domain filter
        Some(MemoryCategory::Context), // category filter
        Some(session_id), // ← session filter
    ).await?;

    // 3. 加载全局共享记忆（跨 session）
    let global_context = memory.hybrid_search(
        &task.prompt,
        5, // 更小的 limit
        Some(MemoryDomain::Public),
        Some(MemoryCategory::Core), // ← Core 记忆是全局的
        None, // ← 无 session 限制
    ).await?;

    // 4. 合并上下文（session 优先）
    let full_context = format!(
        "[Session-specific memory]\n{}\n\n[Global memory]\n{}",
        format_memory_entries(&memory_context),
        format_memory_entries(&global_context)
    );

    // 5. 构建完整 prompt
    let full_prompt = format!(
        "{}\n\n[Task]\n{}\n\n[Instructions]\nUse the memory context above to avoid repeating past mistakes.",
        full_context,
        task.prompt
    );

    // 6. 执行任务
    let result = agent.execute(&full_prompt).await?;

    // 7. 保存结果到记忆（session 隔离）
    memory.set(
        &format!("task_result_{}", task.id),
        serde_json::to_vec(&result)?.as_slice(),
        MemoryDomain::Public,
        MemoryCategory::Result,
        Some(session_id), // ← 保存到 session
    ).await?;

    Ok(result)
}
```

### 7.3 确保降低幻觉

**推荐机制**：

#### 1. 严格的相关性过滤
```rust
// 只使用高相关性记忆（分数 > 0.7）
const HIGH_RELEVANCE_THRESHOLD: f64 = 0.7;

let relevant_entries: Vec<_> = memory.hybrid_search(...)
    .await?
    .into_iter()
    .filter(|r| r.final_score > HIGH_RELEVANCE_THRESHOLD)
    .collect();

// 如果没有高相关性记忆，不注入上下文
if relevant_entries.is_empty() {
    warn!("No highly relevant memories found for task {}. Proceeding without memory context.", task.id);
    return agent.execute(&task.prompt).await;
}
```

#### 2. 引用追踪
```rust
// 在 prompt 中引用来源
let context_with_references: Vec<String> = relevant_entries
    .iter()
    .map(|entry| {
        format!(
            "- [{}] {} (relevance: {:.2})",
            entry.key, // ← 引用 key
            String::from_utf8_lossy(&entry.value),
            entry.final_score
        )
    })
    .collect();

let full_prompt = format!(
    "[Memory context - use only these facts]\n{}\n\n[Task]\n{}\n\n[Important]\nOnly use facts from the memory context above. If the answer is not in the context, say 'I don't have enough information'.",
    context_with_references.join("\n"),
    task.prompt
);
```

#### 3. 事后验证
```rust
// 执行后验证输出是否与记忆一致
let result = agent.execute(&full_prompt).await?;

// 提取 LLM 输出中引用的 key
let cited_keys = extract_cited_keys(&result.output);

// 验证这些 key 是否真的在上下文中
for key in &cited_keys {
    let exists = relevant_entries.iter().any(|e| &e.key == key);
    if !exists {
        warn!("Agent cited non-existent memory key: {}. Possible hallucination.", key);
        // 标记结果为不确定
        return Ok(TaskResult {
            status: TaskStatus::Uncertain,
            warning: Some(format!("Cited non-existent memory: {}", key)),
            ..result
        });
    }
}
```

#### 4. 不确定性标记
```rust
// 在系统提示中要求 LLM 标记不确定内容
let system_prompt = r#"
You are a helpful assistant with access to memory.

When answering:
1. ONLY use facts from the provided memory context.
2. If you're uncertain about a fact, mark it with [UNCERTAIN].
3. If the answer is not in the context, say "I don't have enough information in my memory to answer this."

Example:
"Based on my memory, the database URL is postgres://localhost:5432/mydb [UNCERTAIN - this may have changed since the last update]."
"#;

let full_prompt = format!("{}\n\n{}", system_prompt, user_prompt);
```

#### 5. 循环检测
```rust
// 防止 LLM 输出再次作为上下文
pub fn is_llm_generated_key(key: &str) -> bool {
    key.starts_with("llm_output_") ||
    key.starts_with("agent_response_") ||
    key.starts_with("ai_generated_")
}

// 在 MemoryLoader 中过滤
let entries: Vec<_> = all_entries
    .into_iter()
    .filter(|e| !is_llm_generated_key(&e.key))
    .collect();
```

---

## 8. 总结与建议

### 8.1 ZeroClaw 的核心设计

| 特性 | 实现方式 | 评价 |
|-----|---------|------|
| **Agent 隔离** | 单 Agent + session_id | ⚠️ 适合单人场景，多用户需要额外抽象 |
| **记忆分组** | MemoryCategory + session_id | ✅ 简单有效，但缺乏标签系统 |
| **降低幻觉** | 相关性过滤 + 不可信记忆过滤 | ✅ 基础机制完善，但缺少事后验证 |
| **动态路由** | 查询分类 + model_routes | ✅ 实用，但规则管理较复杂 |
| **上下文管理** | Agent 内置 history + 压缩 | ⚠️ 内存历史不适合持久化场景 |

### 8.2 CIS 应该借鉴的

| 特性 | 借鉴方案 | 优先级 |
|-----|---------|--------|
| **session_id 隔离** | 添加 `session_id: Option<String>` 到 `MemoryEntry` | P0 |
| **MemoryLoader trait** | 定义 `load_context()` 方法，支持相关性过滤 | P0 |
| **不可信记忆过滤** | 实现 `is_untrusted_memory_key()` 函数 | P1 |
| **查询分类** | 配合 DAG 四级决策，实现智能路由 | P1 |
| **引用追踪** | 在 prompt 中标记来源，事后验证 | P2 |

### 8.3 CIS 不应该借鉴的

| 特性 | 不借鉴原因 |
|-----|----------|
| **单 Agent 模式** | CIS 的 Agent Pool 更灵活，支持并行和多 Runtime |
| **内置历史** | CIS 的记忆持久化更好，不需要内存历史 |
| **LLM 压缩** | CIS 的 54 周归档策略更可靠，不需要 LLM 总结 |

### 8.4 最终建议

**Phase 1 (P0)**: 添加 session_id 和 MemoryLoader
- 在 `cis-types` 中添加 `session_id` 字段
- 在 `cis-traits` 中定义 `MemoryLoader` trait
- 在 `cis-memory` 中实现 `DefaultMemoryLoader`

**Phase 2 (P1)**: 实现不可信记忆过滤
- 在 `cis-memory` 中添加 `hygiene` 模块
- 实现 `is_untrusted_memory_key()` 函数
- 在 `MemoryLoader` 中应用过滤

**Phase 3 (P2)**: 引用追踪和事后验证
- 在 prompt 中添加来源标记
- 实现事后验证逻辑
- 添加不确定性标记机制

**Phase 4 (P3)**: 高级特性
- 实现标签系统（`tags: Vec<String>`）
- 支持复杂查询（按标签、domain、category、session 组合过滤）
- 添加记忆版本控制和冲突解决

---

**分析完成时间**: 2026-02-20
**相关文档**:
- [ZeroClaw AGENTS.md](https://github.com/zeroclaw-labs/zeroclaw/blob/main/AGENTS.md)
- [CIS v1.2.0 Final Plan](../task/CIS_V1.2.0_FINAL_PLAN.md)
- [CIS 记忆系统设计](../../design/memory-system-design.md)
