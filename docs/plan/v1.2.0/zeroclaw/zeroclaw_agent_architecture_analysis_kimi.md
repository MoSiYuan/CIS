# ZeroClaw Agent 架构分析 - 任务拆分与记忆管理

> **分析日期**: 2026-02-20
> **分析对象**: ZeroClaw Agent 模块 (`src/agent/`)
> **分析重点**: Agent 拆分机制、记忆分组管理、幻觉降低策略
> **适用版本**: CIS v1.2.0 整合参考

---

## 执行摘要

ZeroClaw 采用**单 Agent 多工具**的架构模式，通过以下机制实现任务拆分和降低幻觉：

| 机制 | 实现方式 | 作用 |
|------|----------|------|
| **Delegate Tool** | 子 Agent 委派 | 不同任务使用不同模型/配置 |
| **Memory Loader** | 记忆分组检索 | 仅加载相关记忆，降低干扰 |
| **Query Classifier** | 查询分类路由 | 自动选择最适合的模型 |
| **Tool Dispatcher** | 工具调用抽象 | 支持多种工具调用格式 |
| **History Compaction** | 对话历史压缩 | 防止上下文过长导致幻觉 |

---

## 1. Agent 拆分机制

### 1.1 Delegate Tool - 子 Agent 委派

ZeroClaw 的 Agent 拆分通过 **`delegate` 工具**实现，而非多 Agent 实例常驻内存。

**核心设计**:
```rust
// src/tools/delegate.rs
pub struct DelegateTool {
    agents: Arc<HashMap<String, DelegateAgentConfig>>,
    security: Arc<SecurityPolicy>,
    fallback_credential: Option<String>,
    depth: u32,  // 委派深度，防止无限递归
}
```

**配置结构**:
```rust
pub struct DelegateAgentConfig {
    pub provider: String,           // 如 "ollama", "openrouter"
    pub model: String,              // 如 "llama3", "claude-sonnet"
    pub system_prompt: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f64>,
    pub max_depth: u32,             // 最大委派深度
}
```

**使用示例**:
```rust
// 主 Agent 检测到需要代码生成任务
let result = delegate_tool.execute(json!({
    "agent": "coder",           // 使用专门的 coder Agent
    "prompt": "实现一个快速排序算法",
    "context": "需要在 Rust 中实现，要求线程安全"
})).await?;
```

**委派深度控制**:
```rust
// 防止无限递归委派
if self.depth >= agent_config.max_depth {
    return Ok(ToolResult {
        success: false,
        error: Some(format!("Delegation depth limit reached"))
    });
}
```

### 1.2 不同任务启动不同 Agent 的策略

| 任务类型 | 推荐 Agent | 模型选择 | 理由 |
|----------|-----------|----------|------|
| 代码生成 | `coder` | Claude/GPT-4 | 强代码能力 |
| 快速总结 | `summarizer` | Llama3/GPT-3.5 | 成本低、速度快 |
| 深度推理 | `researcher` | Claude Opus | 强推理能力 |
| 翻译任务 | `translator` | 专用翻译模型 | 精确翻译 |

**配置示例**:
```toml
[agents.coder]
provider = "openrouter"
model = "anthropic/claude-sonnet-4-20250514"
system_prompt = "You are an expert programmer..."
temperature = 0.2
max_depth = 3
```

---

## 2. 记忆分组管理机制

### 2.1 Memory Loader - 记忆加载抽象

**核心 Trait**:
```rust
#[async_trait]
pub trait MemoryLoader: Send + Sync {
    async fn load_context(&self, memory: &dyn Memory, user_message: &str) 
        -> anyhow::Result<String>;
}
```

**默认实现** (`DefaultMemoryLoader`):
```rust
pub struct DefaultMemoryLoader {
    limit: usize,                   // 最大检索条数 (默认 5)
    min_relevance_score: f64,       // 最小相关度分数 (默认 0.4)
}

#[async_trait]
impl MemoryLoader for DefaultMemoryLoader {
    async fn load_context(&self, memory: &dyn Memory, user_message: &str) 
        -> anyhow::Result<String> {
        // 1. 使用向量搜索检索相关记忆
        let entries = memory.recall(user_message, self.limit, None).await?;
        
        // 2. 过滤低相关度记忆
        let mut context = String::from("[Memory context]\n");
        for entry in entries {
            if let Some(score) = entry.score {
                if score < self.min_relevance_score {
                    continue;  // 跳过不相关记忆
                }
            }
            // 3. 过滤助手自动保存的条目（防止幻觉传递）
            if memory::is_assistant_autosave_key(&entry.key) {
                continue;
            }
            writeln!(context, "- {}: {}", entry.key, entry.content)?;
        }
        
        Ok(context)
    }
}
```

### 2.2 记忆分组策略

**基于 MemoryCategory 的分类**:
```rust
pub enum MemoryCategory {
    Core,           // 长期事实、偏好、决策
    Daily,          // 每日会话日志
    Conversation,   // 对话上下文
    Custom(String), // 用户自定义分类
}
```

**降低幻觉的关键过滤**:
```rust
// 1. 过滤助手自动生成的记忆
if memory::is_assistant_autosave_key(&entry.key) {
    continue;  // 防止模型自己的幻觉被传递
}

// 2. 相关度阈值过滤
if score < min_relevance_score {
    continue;  // 只保留高相关度记忆
}
```

---

## 3. 幻觉降低策略

### 3.1 对话历史压缩 (History Compaction)

**ZeroClaw 解决方案**:
```rust
async fn auto_compact_history(
    history: &mut Vec<ChatMessage>,
    provider: &dyn Provider,
    max_history: usize,
) -> Result<bool> {
    // 1. 检查是否需要压缩
    let non_system_count = history.len().saturating_sub(1);
    if non_system_count <= max_history {
        return Ok(false);
    }

    // 2. 保留最近的消息
    let keep_recent = COMPACTION_KEEP_RECENT_MESSAGES.min(non_system_count);
    let compact_count = non_system_count.saturating_sub(keep_recent);
    
    // 3. 使用模型生成摘要
    let summarizer_system = "You are a conversation compaction engine. \
        Summarize older chat history into concise context. \
        Preserve: user preferences, commitments, decisions. \
        Omit: filler, repeated chit-chat.";
    
    let summary = provider
        .chat_with_system(Some(summarizer_system), &transcript, model, 0.2)
        .await?;
    
    // 4. 用摘要替换原始消息
    apply_compaction_summary(history, start, compact_end, &summary);
    
    Ok(true)
}
```

### 3.2 工具结果凭证脱敏

```rust
static SENSITIVE_KEY_PATTERNS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r"(?i)token",
        r"(?i)api[_-]?key",
        r"(?i)password",
        r"(?i)secret",
    ]).unwrap()
});

fn scrub_credentials(input: &str) -> String {
    SENSITIVE_KV_REGEX.replace_all(input, |caps: &regex::Captures| {
        let key = &caps[1];
        let val = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let prefix = if val.len() > 4 { &val[..4] } else { "" };
        format!("{}: {}*[REDACTED]", key, prefix)
    }).to_string()
}
```

### 3.3 查询分类降低不匹配幻觉

**Query Classifier 实现**:
```rust
pub fn classify(config: &QueryClassificationConfig, message: &str) -> Option<String> {
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

        // 关键词匹配（不区分大小写）
        let keyword_hit = rule.keywords.iter()
            .any(|kw| lower.contains(&kw.to_lowercase()));
        
        // 模式匹配（区分大小写）
        let pattern_hit = rule.patterns.iter()
            .any(|pat| message.contains(pat.as_str()));

        if keyword_hit || pattern_hit {
            return Some(rule.hint.clone());
        }
    }

    None
}
```

---

## 4. CIS v1.2.0 整合建议

### 4.1 引入 Delegate Tool 模式

```rust
// cis-skills/src/delegate.rs
pub struct DelegateSkill {
    agents: HashMap<String, AgentConfig>,
    max_depth: u32,
}

#[async_trait]
impl Skill for DelegateSkill {
    async fn execute(&self, ctx: &Context, params: Value) -> Result<Value> {
        let agent_name = params["agent"].as_str().unwrap();
        let sub_agent = self.agents.get(agent_name)
            .ok_or_else(|| anyhow!("Unknown agent: {}", agent_name))?;
        
        // 检查委派深度
        if ctx.depth >= self.max_depth {
            return Err(anyhow!("Max delegation depth reached"));
        }
        
        // 创建子 Agent 上下文
        let sub_ctx = ctx.with_depth(ctx.depth + 1);
        
        // 执行子 Agent
        sub_agent.execute(sub_ctx, params["prompt"].clone()).await
    }
}
```

### 4.2 记忆分组加载机制

```rust
// cis-memory/src/loader.rs
pub struct GroupedMemoryLoader {
    groups: Vec<MemoryGroup>,
    relevance_threshold: f32,
}

impl MemoryLoader for GroupedMemoryLoader {
    async fn load(&self, query: &str) -> Result<MemoryContext> {
        let mut context = MemoryContext::new();
        
        for group in &self.groups {
            let entries = self.memory
                .search_in_group(query, group, self.limit)
                .await?;
            
            let filtered: Vec<_> = entries
                .into_iter()
                .filter(|e| e.score >= self.relevance_threshold)
                .collect();
            
            context.add_group(group.name(), filtered);
        }
        
        Ok(context)
    }
}
```

---

## 5. 关键设计对比

| 特性 | ZeroClaw 设计 | CIS v1.2.0 建议 |
|------|---------------|-----------------|
| Agent 拆分 | Delegate Tool | Delegate Skill |
| 记忆分组 | MemoryCategory + relevance | MemoryDomain + Vector Search |
| 历史压缩 | 模型生成摘要 | 模型摘要 + 结构化压缩 |
| 查询分类 | 规则引擎 | 规则引擎 + LLM 分类器 |
| 幻觉过滤 | 过滤 autosave + 相关度 | 过滤低置信度 + 来源追踪 |

---

## 6. 参考代码位置

| 文件 | 功能 |
|------|------|
| `src/tools/delegate.rs` | 子 Agent 委派工具 |
| `src/agent/memory_loader.rs` | 记忆加载抽象 |
| `src/agent/classifier.rs` | 查询分类器 |
| `src/agent/loop_.rs` | Agent 主循环 + 历史压缩 |

---

**分析完成时间**: 2026-02-20
**分析师**: Kimi
**整合目标**: CIS v1.2.0
