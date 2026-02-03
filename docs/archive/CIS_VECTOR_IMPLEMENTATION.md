# CIS Vector Intelligence 实施文档

**版本**: 1.0-FINAL  
**状态**:  ready for implementation  
**工期**: 6周 (24工作日)  

---

## 一、核心目标

构建 CIS Vector Intelligence (CVI) 三层向量架构：

| 层级 | 功能 | 关键表 |
|------|------|--------|
| Memory | 语义记忆搜索 | `memory_embeddings` |
| Session | 对话持久化+跨项目恢复 | `message_embeddings`, `summary_embeddings` |
| Skill ⭐ | 自然语言调用+链式编排 | `skill_intent_vec`, `skill_capability_vec` |

---

## 二、数据库Schema

```sql
-- 记忆向量
CREATE VIRTUAL TABLE memory_embeddings USING vec0(
    memory_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    key TEXT,
    category TEXT
);

-- 对话消息向量
CREATE VIRTUAL TABLE message_embeddings USING vec0(
    message_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    conversation_id TEXT,
    content TEXT
);

-- 对话摘要向量
CREATE VIRTUAL TABLE summary_embeddings USING vec0(
    conversation_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    summary TEXT,
    project_path TEXT
);

-- Skill意图向量 ⭐核心
CREATE VIRTUAL TABLE skill_intent_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    skill_name TEXT,
    description TEXT
);

-- Skill能力向量 ⭐核心
CREATE VIRTUAL TABLE skill_capability_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    capabilities TEXT
);

-- Skill语义元数据
CREATE TABLE skill_semantics (
    skill_id TEXT PRIMARY KEY,
    skill_name TEXT,
    description TEXT,
    example_intents TEXT, -- JSON
    parameter_schema TEXT, -- JSON
    io_signature TEXT, -- JSON
    scope TEXT -- 'local' | 'global'
);

-- Skill兼容性矩阵
CREATE TABLE skill_compatibility (
    source_skill_id TEXT,
    target_skill_id TEXT,
    compatibility_score REAL,
    project_id TEXT, -- NULL=global, 有值=项目专属
    PRIMARY KEY (source_skill_id, target_skill_id, project_id)
);
```

---

## 三、核心组件

### 3.1 VectorStorage
```rust
pub struct VectorStorage {
    conn: Connection,
    embedding: Arc<dyn EmbeddingService>,
}

impl VectorStorage {
    pub async fn index_memory(&self, key: &str, value: &[u8]) -> Result<()>;
    pub async fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>>;
    
    pub async fn index_message(&self, msg: &ConversationMessage) -> Result<()>;
    pub async fn search_conversations(&self, query: &str, project: Option<&str>) -> Result<Vec<Conversation>>;
    
    pub async fn register_skill(&self, semantics: &SkillSemantics) -> Result<()>; ⭐
    pub async fn search_skills(&self, query: &str, project: Option<&str>) -> Result<Vec<SkillMatch>>; ⭐
}
```

### 3.2 ConversationContext
```rust
pub struct ConversationContext {
    db: Arc<ConversationDb>,
    storage: Arc<VectorStorage>,
    current_conv: Arc<RwLock<Option<Conversation>>>,
}

impl ConversationContext {
    pub async fn start(&self, session: String, project: Option<String>) -> Result<()>;
    pub async fn add_message(&self, role: Role, content: &str) -> Result<()>;
    pub async fn find_similar(&self, query: &str, limit: usize) -> Result<Vec<Conversation>>; // 跨项目
    pub async fn save_with_summary(&self) -> Result<()>;
}
```

### 3.3 SkillVectorRouter ⭐核心创新
```rust
pub struct SkillVectorRouter {
    local_registry: Arc<ProjectSkillRegistry>, // 项目内
    global_registry: Option<Arc<GlobalSkillRegistry>>, // 全局
}

impl SkillVectorRouter {
    // 自然语言调用Skill - 核心方法
    pub async fn route(&self, input: &str) -> Result<SkillRoutingResult> {
        let parsed = self.parser.parse(input).await?;
        
        // 1. 先在本地搜索
        let local = self.local_registry.search(&parsed.intent, 5).await?;
        if !local.is_empty() && local[0].similarity > 0.7 {
            return self.build_local_result(local[0].clone(), &parsed).await;
        }
        
        // 2. 本地没有，询问是否搜索全局
        Err(CisError::skill_not_found_local(input))
    }
    
    // 显式全局搜索（需确认）
    pub async fn route_global(&self, input: &str) -> Result<SkillRoutingResult>;
    
    // 发现Skill链
    pub async fn discover_chain(&self, task: &str) -> Result<SkillChain>;
}
```

### 3.4 CisAdminSkill
```rust
pub struct CisAdminSkill {
    project_registry: Arc<ProjectSkillRegistry>,
}

impl Skill for CisAdminSkill {
    fn name(&self) -> &str { "cis-local" }
    
    async fn handle(&self, ctx: &dyn Context, event: Event) -> Result<()> {
        match event {
            // 只搜索本地Skills（无幻觉）
            Event::Custom { name: "find", data } => {
                let matches = self.project_registry
                    .search(data.get("query"), 5).await?;
                ctx.respond(json!({"scope": "local", "results": matches}));
            }
            
            // 显式搜索全局（带警告）
            Event::Custom { name: "find_global", data } => {
                let matches = self.search_global(data.get("query")).await?;
                ctx.respond(json!({
                    "scope": "global",
                    "warning": "来自其他项目",
                    "requires_confirmation": true,
                    "results": matches
                }));
            }
            
            _ => {}
        }
        Ok(())
    }
}
```

---

## 四、项目隔离机制

```rust
pub struct ProjectSkillRegistry {
    project_id: String,
    storage: Arc<VectorStorage>, // 指向项目专属DB
}

impl ProjectSkillRegistry {
    pub fn for_project(path: &Path) -> Result<Self> {
        let db_path = path.join(".cis/skill_vectors.db");
        Ok(Self {
            project_id: hash(path),
            storage: VectorStorage::open(&db_path)?,
        })
    }
    
    // 严格本地搜索
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillMatch>> {
        self.storage.search_skills(query, Some(&self.project_id), limit).await
    }
}
```

**无幻觉保证**：
- 默认只搜索 `.cis/skill_vectors.db`（项目专属）
- 跨项目搜索需显式调用 `find_global`
- 返回结果带 `scope` 标记和 `warning`

---

## 五、CLI命令

```bash
# 自然语言调用Skill（本地优先）
cis skill do "分析今天的销售数据"

# 显式搜索其他项目
cis skill search-global "Markdown编辑器"

# 推荐Skill链
cis skill chain "分析数据并生成PDF报告"

# 语义搜索记忆
cis memory search "数据库优化方案"

# 查找相似历史会话
cis session find "上次的导航设置"
```

---

## 六、成功指标

| 指标 | 目标 | 测量 |
|------|------|------|
| Skill匹配准确率 | >80% | 50个自然语言指令 |
| Skill链发现准确率 | >75% | 20个多步骤场景 |
| 跨项目搜索召回率 | >90% | 10个跨项目场景 |
| 向量搜索延迟(10k) | <50ms | 基准测试 |

---

## 七、依赖

```toml
[dependencies]
sqlite-vec = "0.1"
ort = "1.16" # ONNX Runtime for embeddings
# 或
rust-bert = "0.23" # 纯Rust
```

---

**文档版本**: 1.0  
**最后更新**: 2026-02-02
