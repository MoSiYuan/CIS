# TASK 7.5: 记忆分组与幻觉降低

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 13+

---

## 任务概述

实现三级记忆隔离（Agent/Task/Device 级）和四层幻觉降低机制。

## 工作内容

### 1. 实现三级记忆隔离

**文件**: `cis-memory/src/namespace.rs`

```rust
//! 记忆命名空间管理 - 三级隔离

use std::fmt;

/// 三级记忆隔离
/// 
/// Level 1: Agent 级隔离 (receptionist/, coder/, doc/)
/// Level 2: Task 级隔离 (task_001/, task_002/)  
/// Level 3: Device 级隔离 (device_local/, device_remote_A/)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryNamespace {
    /// Agent 命名空间
    pub agent: String,
    /// Task ID (可选)
    pub task_id: Option<String>,
    /// Device ID
    pub device_id: String,
}

impl MemoryNamespace {
    /// 创建新的命名空间
    pub fn new(agent: impl Into<String>, device_id: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            task_id: None,
            device_id: device_id.into(),
        }
    }
    
    /// 设置 Task ID
    pub fn with_task(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }
    
    /// 生成完整的键前缀
    /// 格式: {agent}/{task_id}/{device_id}/{key}
    pub fn key(&self, key: impl AsRef<str>) -> String {
        match &self.task_id {
            Some(task) => format!("{}/{}/{}/{}", 
                self.agent, task, self.device_id, key.as_ref()),
            None => format!("{}/{}/{}", 
                self.agent, self.device_id, key.as_ref()),
        }
    }
    
    /// 获取父级命名空间（用于搜索）
    pub fn parent_prefix(&self) -> String {
        match &self.task_id {
            Some(task) => format!("{}/{}/{}/", 
                self.agent, task, self.device_id),
            None => format!("{}/{}/", self.agent, self.device_id),
        }
    }
}

impl fmt::Display for MemoryNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.parent_prefix())
    }
}

/// 预定义 Agent 命名空间
pub mod agents {
    pub const RECEPTIONIST: &str = "receptionist";
    pub const CODER: &str = "coder";
    pub const DOC: &str = "doc";
    pub const DEBUGGER: &str = "debugger";
}

/// 预定义 Device 命名空间
pub mod devices {
    pub const LOCAL: &str = "local";
    pub const REMOTE_PREFIX: &str = "remote_";
}
```

### 2. 实现 AntiHallucinationLoader

**文件**: `cis-memory/src/loader/anti_hallucination.rs`

```rust
//! 四层幻觉降低机制

use crate::{Memory, MemoryEntry, SearchResult};

/// 四层幻觉过滤配置
#[derive(Debug, Clone)]
pub struct AntiHallucinationConfig {
    /// Layer 1: 最小相关性分数 (0.0 - 1.0)
    pub min_relevance: f64,
    /// Layer 2: 是否过滤不可信记忆
    pub exclude_untrusted: bool,
    /// Layer 3: 是否要求来源验证
    pub require_source: bool,
    /// Layer 4: 最大返回条目数
    pub max_entries: usize,
}

impl Default for AntiHallucinationConfig {
    fn default() -> Self {
        Self {
            min_relevance: 0.7,
            exclude_untrusted: true,
            require_source: false,
            max_entries: 5,
        }
    }
}

/// 不可信记忆键前缀列表
const UNTRUSTED_PREFIXES: &[&str] = &[
    "ai_summary_",
    "assistant_resp_",
    "generated_",
    "draft_",
];

/// 幻觉降低加载器
pub struct AntiHallucinationLoader<M: Memory> {
    memory: M,
    config: AntiHallucinationConfig,
}

impl<M: Memory> AntiHallucinationLoader<M> {
    pub fn new(memory: M, config: AntiHallucinationConfig) -> Self {
        Self { memory, config }
    }
    
    /// 安全加载上下文（四层过滤）
    pub async fn load_context_safe(
        &self,
        query: &str,
        namespace: &MemoryNamespace,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // 1. Hybrid search (max_entries * 2 预留过滤空间)
        let search_limit = self.config.max_entries * 2;
        let entries = self.memory
            .hybrid_search(query, search_limit, Some(namespace))
            .await?;
        
        // 2. Layer 1: 相关性过滤
        let relevant: Vec<_> = entries
            .into_iter()
            .filter(|e| e.final_score >= self.config.min_relevance)
            .collect();
        
        // 3. Layer 2: 不可信记忆过滤
        let trusted = if self.config.exclude_untrusted {
            relevant
                .into_iter()
                .filter(|e| !is_untrusted_memory(&e.key))
                .collect()
        } else {
            relevant
        };
        
        // 4. Layer 3: 来源验证
        let verified = if self.config.require_source {
            trusted
                .into_iter()
                .filter(|e| e.source.is_some())
                .collect()
        } else {
            trusted
        };
        
        // 5. Layer 4: 数量限制
        let limited: Vec<_> = verified
            .into_iter()
            .take(self.config.max_entries)
            .collect();
        
        Ok(limited)
    }
    
    /// 带置信度分数的搜索
    pub async fn search_with_confidence(
        &self,
        query: &str,
        namespace: &MemoryNamespace,
    ) -> anyhow::Result<Vec<(MemoryEntry, f64)>> {
        let entries = self.load_context_safe(query, namespace).await?;
        
        let results: Vec<_> = entries
            .into_iter()
            .map(|e| {
                let confidence = calculate_confidence(&e);
                (e, confidence)
            })
            .collect();
        
        Ok(results)
    }
}

/// 检查是否为不可信记忆
fn is_untrusted_memory(key: &str) -> bool {
    UNTRUSTED_PREFIXES.iter().any(|prefix| key.starts_with(prefix))
}

/// 计算记忆条目的置信度分数
fn calculate_confidence(entry: &MemoryEntry) -> f64 {
    let mut score = entry.final_score;
    
    // 有来源的记忆更可信
    if entry.source.is_some() {
        score *= 1.2;
    }
    
    // 较新的记忆更可信（衰减因子）
    let age_days = (Utc::now() - entry.timestamp).num_days();
    let recency_factor = 1.0 - (age_days as f64 * 0.01).min(0.3);
    score *= recency_factor;
    
    // 有人工验证的记忆更可信
    if entry.verified {
        score *= 1.3;
    }
    
    score.min(1.0)
}
```

### 3. 扩展 MemoryEntry

**文件**: `cis-memory/src/entry.rs`

```rust
/// 记忆条目扩展
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub timestamp: DateTime<Utc>,
    
    // 新增字段
    /// 命名空间
    pub namespace: MemoryNamespace,
    /// 来源（用于来源验证）
    pub source: Option<SourceInfo>,
    /// 是否人工验证
    pub verified: bool,
    /// 相关性分数（搜索时使用）
    #[serde(skip)]
    pub final_score: f64,
    /// 标签
    pub tags: Vec<String>,
    /// 嵌入向量（可选）
    pub embedding: Option<Vec<f32>>,
}

/// 来源信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    /// 来源类型
    pub source_type: SourceType,
    /// 来源 ID
    pub source_id: String,
    /// 来源 URL 或路径
    pub source_url: Option<String>,
    /// 创建者
    pub creator: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    UserInput,
    File,
    WebPage,
    AgentResponse,
    ExternalAPI,
}
```

### 4. 集成到 Agent

**文件**: `cis-core/src/agent/context.rs`

```rust
//! Agent 上下文构建

use cis_memory::loader::{AntiHallucinationLoader, AntiHallucinationConfig};

pub struct AgentContextBuilder<M: Memory> {
    memory: M,
    namespace: MemoryNamespace,
    config: AntiHallucinationConfig,
}

impl<M: Memory> AgentContextBuilder<M> {
    pub fn new(agent_type: &str, memory: M) -> Self {
        let namespace = MemoryNamespace::new(agent_type, "local");
        
        Self {
            memory,
            namespace,
            config: AntiHallucinationConfig::default(),
        }
    }
    
    pub fn with_task(mut self, task_id: &str) -> Self {
        self.namespace = self.namespace.with_task(task_id);
        self
    }
    
    pub fn with_config(mut self, config: AntiHallucinationConfig) -> Self {
        self.config = config;
        self
    }
    
    pub async fn build(self, query: &str) -> anyhow::Result<AgentContext> {
        let loader = AntiHallucinationLoader::new(self.memory, self.config);
        
        let memories = loader
            .load_context_safe(query, &self.namespace)
            .await?;
        
        Ok(AgentContext {
            memories,
            namespace: self.namespace,
        })
    }
}

pub struct AgentContext {
    pub memories: Vec<MemoryEntry>,
    pub namespace: MemoryNamespace,
}

impl AgentContext {
    /// 格式化为 LLM 上下文
    pub fn to_prompt(&self) -> String {
        let mut sections = vec![];
        
        for (i, entry) in self.memories.iter().enumerate() {
            let section = format!(
                "[{}] {} (relevance: {:.2})\n{}",
                i + 1,
                entry.key,
                entry.final_score,
                String::from_utf8_lossy(&entry.value)
            );
            sections.push(section);
        }
        
        sections.join("\n\n")
    }
}
```

### 5. 配置选项

**文件**: `cis-core/src/agent/config.rs`

```rust
/// Agent 记忆配置
#[derive(Debug, Clone, Deserialize)]
pub struct AgentMemoryConfig {
    /// 命名空间
    pub namespace: String,
    /// 是否启用幻觉过滤
    pub enable_hallucination_filter: bool,
    /// 最小相关性
    pub min_relevance: f64,
    /// 最大上下文条目
    pub max_context_entries: usize,
    /// 是否过滤不可信记忆
    pub exclude_untrusted: bool,
    /// 是否要求来源
    pub require_source: bool,
}

impl Default for AgentMemoryConfig {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            enable_hallucination_filter: true,
            min_relevance: 0.7,
            max_context_entries: 5,
            exclude_untrusted: true,
            require_source: false,
        }
    }
}
```

## 验收标准

- [ ] 三级记忆隔离实现
- [ ] 四层幻觉过滤工作正常
- [ ] AntiHallucinationLoader 测试通过
- [ ] 来源追踪机制可用
- [ ] 置信度计算准确
- [ ] 集成到 Agent 上下文构建
- [ ] 性能可接受（过滤开销 < 10%）

## 依赖

- Task 2.2 (cis-memory)
- Task 7.1 (Agent trait)

## 阻塞

- Task 7.6 (集成测试)

---
