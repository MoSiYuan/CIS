# CIS Vector Intelligence - 并行开发任务

**总工期**: 6周  
**并行度**: 4条主线  
**任务数**: 18个  

---

## 并行主线规划

```
Week 1   Week 2   Week 3   Week 4   Week 5   Week 6
├────────┴────────┴────────┴────────┴────────┴───────┤
│ 主线A: 基础设施 (Embedding + VectorStorage)          │
├────────┴────────┴────────┴─────────────────────────┤
│ 主线B: Session层 (ConversationDb + Context)          │
│         ├─Week1-2: DB实现                            │
│         └─Week3-4: 上下文恢复                        │
├────────────────────────┴────────┴────────┴───────────┤
│ 主线C: Skill自动化 ⭐核心 (Router + Chain)            │
│                         ├─Week3: IntentParser       │
│                         ├─Week4: VectorRegistry     │
│                         └─Week5: Router + Chain     │
├────────────────────────────────────────┴────────┴────┤
│ 主线D: 集成与测试 (CLI + RAG + 测试)                  │
│                                         Week5-6     │
└──────────────────────────────────────────────────────┘
```

---

## 主线A: 基础设施 (2人)

### A1: Embedding Service
**ID**: T-A1  
**工时**: 3天  
**负责人**: ____________  
**依赖**: 无  

```rust
// cis-core/src/ai/embedding.rs
pub trait EmbeddingService: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

pub struct LocalEmbeddingService; // MiniLM-L6-v2
pub struct OpenAIEmbeddingService; // 降级
```

**验收**: `embed("test").await?.len() == 768`

---

### A2: VectorStorage Core
**ID**: T-A2  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-A1  

```rust
// cis-core/src/vector/storage.rs
pub struct VectorStorage {
    conn: Connection,
    embedding: Arc<dyn EmbeddingService>,
}

// 实现: index_memory, search_memory, index_message, search_message
```

**验收**: `storage.search_memory("暗黑模式", 5).await?.len() > 0`

---

### A3: HNSW索引优化
**ID**: T-A3  
**工时**: 2天  
**负责人**: ____________  
**依赖**: T-A2  

- 添加HNSW索引
- 批量向量化
- 异步索引

**验收**: 10k向量搜索<50ms

---

## 主线B: Session层 (2人)

### B1: ConversationDb
**ID**: T-B1  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-A2  

```rust
// cis-core/src/storage/conversation_db.rs
pub struct ConversationDb;

// 表: conversations, conversation_messages
// 方法: save_conv, get_conv, save_message, list_messages
```

---

### B2: Message向量索引
**ID**: T-B2  
**工时**: 2天  
**负责人**: ____________  
**依赖**: T-B1, T-A2  

- 消息自动向量化
- 摘要向量索引

---

### B3: ConversationContext
**ID**: T-B3  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-B2  

```rust
// cis-core/src/conversation/context.rs
pub struct ConversationContext;

// 方法: start, add_message, find_similar(project?), save_with_summary
```

**验收**: 跨项目相似对话搜索可用

---

### B4: 跨项目恢复
**ID**: T-B4  
**工时**: 2天  
**负责人**: ____________  
**依赖**: T-B3  

- 项目切换时上下文恢复
- 摘要合并

---

## 主线C: Skill自动化 ⭐核心 (3人)

### C1: IntentParser
**ID**: T-C1  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-A1  

```rust
// cis-core/src/intent/parser.rs
pub struct IntentParser;

pub struct ParsedIntent {
    pub raw: String,
    pub normalized: String,
    pub embedding: Vec<f32>,
    pub entities: HashMap<String, EntityValue>,
    pub action: ActionType,
}

// NER: 时间、文件路径、数字
// 动作: Analyze/Generate/Commit/Query/Send
```

---

### C2: ProjectSkillRegistry
**ID**: T-C2  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-C1, T-A2  

```rust
// cis-core/src/skill/project_registry.rs
pub struct ProjectSkillRegistry {
    project_id: String,
    storage: Arc<VectorStorage>, // 项目专属DB
}

// 方法: register(skill), search(query, scope: Local|Global)
```

**关键**: 项目隔离，无幻觉

---

### C3: SkillVectorRouter
**ID**: T-C3  
**工时**: 4天  
**负责人**: ____________  
**依赖**: T-C2  

```rust
// cis-core/src/skill/router.rs
pub struct SkillVectorRouter;

// 核心方法
pub async fn route(&self, input: &str) -> Result<SkillRoutingResult>;
// 1. parse intent
// 2. search local registry
// 3. if found && sim>0.7 -> return
// 4. else -> Err(skill_not_found_local)

pub async fn route_global(&self, input: &str) -> Result<SkillRoutingResult>;
// 显式全局搜索，返回带warning
```

---

### C4: Skill Chain编排
**ID**: T-C4  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-C3  

```rust
// cis-core/src/skill/chain.rs
pub struct SkillChain {
    pub steps: Vec<ChainStep>,
}

// 方法: discover_chain(task) -> 基于IO兼容性
// 方法: execute_chain(chain, params)
```

---

### C5: CisAdminSkill
**ID**: T-C5  
**工时**: 2天  
**负责人**: ____________  
**依赖**: T-C3  

```rust
impl Skill for CisAdminSkill {
    fn name(&self) -> &str { "cis-local" }
    
    async fn handle(&self, ctx: &Context, event: Event) -> Result<()> {
        match event {
            Event::Custom { name: "find", data } => {
                // 只搜索本地
                let matches = self.local_registry.search(query).await?;
                ctx.respond(json!({"scope": "local", results}));
            }
            Event::Custom { name: "find_global", data } => {
                // 全局搜索，带warning
                let matches = self.search_global(query).await?;
                ctx.respond(json!({
                    "scope": "global",
                    "warning": "来自其他项目",
                    "requires_confirmation": true,
                    results
                }));
            }
            _ => {}
        }
        Ok(())
    }
}
```

---

## 主线D: 集成与测试 (2人)

### D1: CLI命令
**ID**: T-D1  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-C3, T-B3  

```bash
# 实现命令
cis skill do "..."              # T-C3
cis skill search-global "..."   # T-C3 route_global
cis skill chain "..."           # T-C4
cis memory search "..."         # T-A2
cis session find "..."          # T-B3
```

---

### D2: AI Provider RAG
**ID**: T-D2  
**工时**: 2天  
**负责人**: ____________  
**依赖**: T-B3  

```rust
// 更新AiProvider trait
async fn chat_with_context(&self, prompt: &str, ctx: &ConversationContext) -> Result<String>;

// 在Claude/Kimi Provider实现
```

---

### D3: 单元测试
**ID**: T-D3  
**工时**: 3天  
**负责人**: ____________  
**依赖**: 全部  

- VectorStorage: 10个测试
- ConversationContext: 8个测试
- SkillVectorRouter: 12个测试
- IntentParser: 6个测试

---

### D4: 集成测试
**ID**: T-D4  
**工时**: 3天  
**负责人**: ____________  
**依赖**: T-D3  

```rust
// 测试场景
#[tokio::test]
async fn test_cross_project_context_recovery() { ... }

#[tokio::test]
async fn test_skill_natural_language_routing() { ... }

#[tokio::test]
async fn test_skill_chain_discovery() { ... }

#[tokio::test]
async fn test_no_hallucination_across_projects() { ... }
```

---

## 任务依赖图

```
T-A1 ─┬── T-A2 ─┬── T-A3
      │         │
      │         ├── T-B1 ─┬── T-B2 ─┬── T-B3 ─┬── T-B4
      │         │         │         │         │
      │         │         │         │         │
      │         └─────────┴─────────┼── T-C1 ─┼── T-C2 ─┬── T-C3 ─┬── T-C4
      │                             │         │         │         │
      │                             └─────────┴─────────┘         │
      │                                                           │
      └───────────────────────────────────────────────────────────┼── T-D1
                                                                  │
T-C5 ─────────────────────────────────────────────────────────────┘
                                                                  │
T-D2 ─────────────────────────────────────────────────────────────┘
                                                                  │
T-D3 ─────────────────────────────────────────────────────────────┘
                                                                  │
T-D4 ─────────────────────────────────────────────────────────────┘
```

---

## 每周站会检查点

### Week 1
- [ ] T-A1: Embedding Service可用
- [ ] T-A2: VectorStorage基础CRUD
- [ ] T-B1: ConversationDb表创建
- [ ] T-C1: IntentParser实体提取

### Week 2
- [ ] T-A3: HNSW索引，性能达标
- [ ] T-B2: Message向量索引
- [ ] T-C2: ProjectSkillRegistry项目隔离
- [ ] 集成: A2+B1+C1联调

### Week 3
- [ ] T-B3: ConversationContext跨项目搜索
- [ ] T-C3: SkillVectorRouter核心路由
- [ ] 集成: B3+C2联调

### Week 4
- [ ] T-B4: 跨项目上下文恢复
- [ ] T-C4: Skill Chain编排
- [ ] T-C5: CisAdminSkill实现
- [ ] 集成: C3+C4+C5联调

### Week 5
- [ ] T-D1: CLI命令完整
- [ ] T-D2: AI Provider RAG
- [ ] 集成测试: 全链路

### Week 6
- [ ] T-D3: 单元测试>80%覆盖
- [ ] T-D4: 集成测试通过
- [ ] 性能测试达标
- [ ] 文档完成

---

## 风险应对

| 风险 | 应对 |
|------|------|
| Embedding模型太大 | 使用MiniLM(80MB)，延迟加载 |
| 向量搜索慢 | HNSW索引+异步索引 |
| Skill匹配误判 | 置信度阈值0.7+用户确认 |
| 项目DB膨胀 | 自动清理旧向量+摘要压缩 |

---

**文档版本**: 1.0  
**开始日期**: ____________  
**预计完成**: ____________
