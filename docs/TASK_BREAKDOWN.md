# CIS Vector Intelligence - 执行任务清单

**项目**: CIS Vector Intelligence (CVI)  
**总工期**: 6 周 (24 工作日)  
**任务总数**: 20 个  

---

## 快速导航

| 阶段 | 周次 | 任务数 | 关键产出 |
|------|------|--------|---------|
| Phase 1 | Week 1 | 2 | sqlite-vec 集成、Embedding Service |
| Phase 2 | Week 2 | 2 | Memory 向量、Task 向量 |
| Phase 3 | Week 3 | 2 | ConversationDb、ConversationContext |
| Phase 4 | Week 4 | 4 | Skill 向量注册表、Intent Parser、Router、Chain |
| Phase 5 | Week 5 | 3 | RAG 集成、CLI、性能优化 |
| Phase 6 | Week 6 | 3 | 单元测试、集成测试、文档 |

---

## Phase 1: 基础设施 (Week 1)

### Task 1.1: 添加 sqlite-vec 依赖和基础集成
**ID**: CVI-001  
**优先级**: P0  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  

**描述**: 集成 sqlite-vec 扩展，创建 VectorStorage 基础结构

**子任务**:
- [ ] 在 cis-core/Cargo.toml 添加 sqlite-vec = "0.1" 依赖
- [ ] 创建 cis-core/src/vector/mod.rs 模块
- [ ] 创建 VectorStorage 结构体
- [ ] 实现 sqlite-vec 扩展加载
- [ ] 创建向量虚拟表

**验收标准**: VectorStorage::open_default() 成功

**相关文件**:
- cis-core/Cargo.toml
- cis-core/src/vector/mod.rs
- cis-core/src/vector/storage.rs

---

### Task 1.2: 实现 Embedding Service
**ID**: CVI-002  
**优先级**: P0  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-001

**描述**: 实现文本嵌入服务，支持本地模型和云端 API

**子任务**:
- [ ] 定义 EmbeddingService trait
- [ ] 实现 LocalEmbeddingService (MiniLM-L6-v2)
- [ ] 实现 OpenAIEmbeddingService
- [ ] 实现降级机制 (本地失败→云端)

**验收标准**: embed("测试文本") 返回 768 维向量

**相关文件**:
- cis-core/src/ai/embedding.rs

---

## Phase 2: 记忆与 Task 向量 (Week 2)

### Task 2.1: Memory 向量索引
**ID**: CVI-003  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-001, CVI-002

**描述**: 为 Memory 系统添加向量索引和语义搜索

**子任务**:
- [ ] 实现 MemoryDb.set_with_embedding()
- [ ] 实现 MemoryDb.semantic_search()
- [ ] 扩展 SkillContext 添加 memory_search()
- [ ] 添加批量向量化支持

**验收标准**:
```rust
memory.set_with_embedding("key", "用户喜欢深色主题").await?;
let results = memory.semantic_search("暗黑模式", 5).await?;
assert!(results[0].similarity > 0.85);
```

**相关文件**:
- cis-core/src/storage/memory_db.rs
- cis-core/src/memory/mod.rs

---

### Task 2.2: Task 向量索引
**ID**: CVI-004  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-001, CVI-002

**描述**: 为 Task 系统添加多字段向量索引 (title/description/result)

**子任务**:
- [ ] 创建 Task 向量表 (task_title_vec, task_description_vec, task_result_vec)
- [ ] 实现 Task.index_vectors()
- [ ] 实现 Task.semantic_search()
- [ ] 实现相似 Task 自动发现

**验收标准**:
```rust
let task = Task::new("优化数据库查询性能");
task.index_vectors().await?;
let similar = Task::find_similar(&task.id, 0.8).await?;
assert!(!similar.is_empty());
```

**相关文件**:
- cis-core/src/types.rs
- cis-core/src/scheduler/mod.rs

---

## Phase 3: 对话持久化 (Week 3)

### Task 3.1: ConversationDb 实现
**ID**: CVI-005  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-001

**描述**: 创建对话数据库，支持对话和消息的 CRUD 操作

**子任务**:
- [ ] 创建 ConversationDb 结构体
- [ ] 创建 conversations 表
- [ ] 创建 conversation_messages 表
- [ ] 实现消息向量索引
- [ ] 实现对话摘要存储

**验收标准**:
```rust
let db = ConversationDb::open_default()?;
let conv = Conversation::new(session_id);
db.save_conversation(&conv).await?;
let loaded = db.get_conversation(&conv.id).await?;
assert_eq!(loaded.id, conv.id);
```

**相关文件**:
- cis-core/src/storage/conversation_db.rs

---

### Task 3.2: ConversationContext 实现
**ID**: CVI-006  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-005, CVI-003

**描述**: 实现对话上下文管理，支持跨目录恢复和摘要生成

**子任务**:
- [ ] 创建 ConversationContext 结构体
- [ ] 实现对话生命周期管理 (start/stop)
- [ ] 实现消息添加 (User/Assistant)
- [ ] 实现相似对话搜索 (跨目录)
- [ ] 实现摘要生成和主题提取

**验收标准**:
```rust
let ctx = ConversationContext::new();
ctx.start_conversation(session_id, project_path).await?;
ctx.add_user_message("如何设置导航？").await?;
ctx.add_assistant_message("导航已设置...").await?;
ctx.save_with_summary().await?;
```

**相关文件**:
- cis-core/src/conversation/context.rs

---

## Phase 4: Skill 向量自动化 (Week 4) - 核心

### Task 4.1: Skill 向量注册表
**ID**: CVI-007  
**优先级**: P0  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-001, CVI-002

**描述**: 创建 Skill 语义注册表，支持意图和能力向量 (核心创新)

**子任务**:
- [ ] 创建 skill_semantics 表
- [ ] 创建 skill_intent_vec 虚拟表
- [ ] 创建 skill_capability_vec 虚拟表
- [ ] 实现 SkillVectorRegistry.register_semantics()
- [ ] 扩展 Skill trait 添加 semantics() 方法

**验收标准**:
```rust
impl Skill for DataAnalyzerSkill {
    fn semantics(&self) -> SkillSemantics {
        SkillSemantics {
            skill_id: "data-analyzer".to_string(),
            example_intents: vec!["分析今天的销售数据".to_string()],
            // ...
        }
    }
}
```

**相关文件**:
- cis-core/src/skill/vector_registry.rs
- cis-core/src/skill/mod.rs

---

### Task 4.2: Intent Parser 实现
**ID**: CVI-008  
**优先级**: P0  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-007

**描述**: 实现意图解析器，支持 NER 实体提取和动作分类

**子任务**:
- [ ] 创建 IntentParser 结构体
- [ ] 实现实体提取 (NER): 时间、文件路径、数字
- [ ] 实现动作分类 (Analyze/Generate/Commit/Query/Send)
- [ ] 实现意图规范化

**验收标准**:
```rust
let parser = IntentParser::new();
let parsed = parser.parse("分析今天的销售数据").await?;
assert_eq!(parsed.action_type, ActionType::Analyze);
assert!(parsed.entities.contains_key("time"));
```

**相关文件**:
- cis-core/src/intent/parser.rs

---

### Task 4.3: Skill Vector Router 实现
**ID**: CVI-009  
**优先级**: P0  
**工时**: 3 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-007, CVI-008

**描述**: 实现 Skill 向量路由器，核心创新点 - 自然语言调用 Skill

**子任务**:
- [ ] 创建 SkillVectorRouter 结构体
- [ ] 实现 route_by_intent() 核心方法
- [ ] 实现 Skill 语义搜索 (search_skills_by_intent)
- [ ] 实现 execute_chain() 链式执行
- [ ] 实现置信度计算

**验收标准**:
```rust
let router = SkillVectorRouter::new();
let result = router.route_by_intent("分析今天的销售数据").await?;
assert_eq!(result.primary_skill.skill_id, "data-analyzer");
assert!(result.confidence > 0.8);
```

**相关文件**:
- cis-core/src/skill/vector_router.rs

---

### Task 4.4: Skill Chain Orchestrator
**ID**: CVI-010  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-009

**描述**: 实现 Skill 链编排，自动发现多步调用链

**子任务**:
- [ ] 创建 SkillChain 结构体
- [ ] 实现 discover_skill_chain()
- [ ] 实现 skill_compatibility 自动发现
- [ ] 实现参数传递映射 (InputMapping/OutputMapping)

**验收标准**:
```rust
// "分析并生成报告" 应该触发链式调用
let chain = router.discover_skill_chain("data-analyzer", &parsed).await?;
assert_eq!(chain.steps.len(), 2);
assert_eq!(chain.steps[0].skill_id, "data-analyzer");
assert_eq!(chain.steps[1].skill_id, "report-gen");
```

**相关文件**:
- cis-core/src/skill/chain.rs

---

## Phase 5: 集成与优化 (Week 5)

### Task 5.1: AI Provider RAG 集成
**ID**: CVI-011  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-006

**描述**: 在 AI Provider 集成 RAG 上下文增强

**子任务**:
- [ ] 更新 AiProvider trait 添加 chat_with_context()
- [ ] 在 Claude Provider 集成 RAG
- [ ] 在 Kimi Provider 集成 RAG
- [ ] 实现上下文增强 Prompt 构建

**验收标准**:
```rust
let ai = ClaudeProvider::new();
let response = ai.chat_with_context(
    "如何优化查询？",
    Some(&conversation_context)
).await?;
// AI 应该基于历史上下文回答
```

**相关文件**:
- cis-core/src/ai/mod.rs
- cis-core/src/agent/providers/claude.rs
- cis-core/src/agent/providers/kimi.rs

---

### Task 5.2: CLI 命令实现
**ID**: CVI-012  
**优先级**: P2  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-009, CVI-011

**描述**: 实现自然语言 CLI 命令

**子任务**:
- [ ] 实现 cis skill do <自然语言>
- [ ] 实现 cis skill chain <描述> --preview
- [ ] 实现 cis agent context <描述>
- [ ] 实现 cis memory search <查询>

**验收标准**:
```bash
cis skill do "分析今天的销售数据"
# 输出: 已匹配 data-analyzer skill (相似度: 0.92)
# 输出: 执行结果: ...
```

**相关文件**:
- cis-node/src/commands/skill.rs
- cis-node/src/commands/agent.rs
- cis-node/src/commands/memory.rs
- cis-node/src/main.rs

---

### Task 5.3: 性能优化
**ID**: CVI-013  
**优先级**: P2  
**工时**: 1 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: 全部

**描述**: 添加 HNSW 索引，实现批量向量化和异步索引

**子任务**:
- [ ] 添加 HNSW 索引到向量表
- [ ] 实现批量向量化 (batch_embed)
- [ ] 实现异步向量索引 (tokio::spawn)
- [ ] 性能基准测试

**验收标准**:
- 10k 向量搜索 < 50ms
- 100k 向量搜索 < 100ms
- 批量向量化 1000 条 < 5s

**相关文件**:
- cis-core/src/vector/storage.rs

---

## Phase 6: 测试与文档 (Week 6)

### Task 6.1: 单元测试
**ID**: CVI-014  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: 全部

**描述**: 为核心组件编写单元测试

**子任务**:
- [ ] VectorStorage 单元测试
- [ ] ConversationContext 单元测试
- [ ] SkillVectorRouter 单元测试
- [ ] IntentParser 单元测试
- [ ] ParameterResolver 单元测试

**验收标准**:
- 测试覆盖率 > 80%
- 所有测试通过: cargo test

---

### Task 6.2: 集成测试
**ID**: CVI-015  
**优先级**: P1  
**工时**: 2 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: CVI-014

**描述**: 端到端集成测试

**子任务**:
- [ ] 跨目录上下文恢复测试
- [ ] Skill 自动化端到端测试
- [ ] RAG 流程测试
- [ ] 性能压力测试

**验收标准**:
- 语义搜索准确率 > 80%
- Skill 匹配准确率 > 85%
- 端到端延迟 < 2s

---

### Task 6.3: 文档
**ID**: CVI-016  
**优先级**: P2  
**工时**: 1 天  
**负责人**: ____________  
**状态**: 未开始  
**依赖**: 全部

**描述**: 编写项目文档

**子任务**:
- [ ] API 文档 (rustdoc)
- [ ] 使用指南 (docs/USAGE.md)
- [ ] Skill 开发文档 (docs/SKILL_DEVELOPMENT.md)
- [ ] 部署文档 (docs/DEPLOYMENT.md)

---

## 任务依赖图

```
Week 1: 基础设施
├── CVI-001: sqlite-vec 依赖和基础集成
└── CVI-002: Embedding Service (依赖 001)

Week 2: 记忆与 Task 向量
├── CVI-003: Memory 向量索引 (依赖 001, 002)
└── CVI-004: Task 向量索引 (依赖 001, 002)

Week 3: 对话持久化
├── CVI-005: ConversationDb (依赖 001)
└── CVI-006: ConversationContext (依赖 005, 003)

Week 4: Skill 向量自动化 (核心)
├── CVI-007: Skill 向量注册表 (依赖 001, 002)
├── CVI-008: Intent Parser (依赖 007)
├── CVI-009: Skill Vector Router (依赖 007, 008)
└── CVI-010: Skill Chain Orchestrator (依赖 009)

Week 5: 集成与优化
├── CVI-011: AI Provider RAG 集成 (依赖 006)
├── CVI-012: CLI 命令 (依赖 009, 011)
└── CVI-013: 性能优化 (依赖 全部)

Week 6: 测试与文档
├── CVI-014: 单元测试 (依赖 全部)
├── CVI-015: 集成测试 (依赖 014)
└── CVI-016: 文档 (依赖 全部)
```

---

## 成功指标

| 指标 | 目标值 | 验证方法 |
|------|--------|---------|
| 记忆语义搜索准确率 | > 85% | 测试查询 100 条，人工验证 |
| Skill 意图匹配准确率 | > 80% | 测试 50 个自然语言指令 |
| Skill 链发现准确率 | > 75% | 测试 20 个多步骤场景 |
| 向量搜索延迟 (10k) | < 50ms | 基准测试 |
| 跨目录上下文恢复率 | > 90% | 模拟 10 个跨项目场景 |
| 端到端调用延迟 | < 2s | 完整流程测试 |
| 测试覆盖率 | > 80% | cargo tarpaulin |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 本地模型性能不足 | 嵌入延迟高 | 降级到云端 API; 使用更小模型 |
| 向量索引空间膨胀 | 数据库过大 | 定期清理低价值向量; 摘要压缩 |
| Skill 匹配误判 | 执行错误 Skill | 置信度阈值; 用户确认机制; 反馈优化 |
| 隐私泄露 (云端) | 数据离开本地 | 默认本地模式; 可选云端; 敏感数据过滤 |
| 工期延误 | 无法按期交付 | 按优先级分阶段; MVP 先交付核心功能 |

---

## 附录

### A. 关键设计决策

1. **向量维度**: 768 (MiniLM-L6-v2 平衡性能和效果)
2. **相似度阈值**: 0.6 (Skill 匹配), 0.7 (记忆搜索)
3. **HNSW 索引**: 默认启用，10k 以上向量
4. **嵌入模型**: 本地优先，云端降级

### B. 参考资源

- sqlite-vec GitHub: https://github.com/asg017/sqlite-vec
- sentence-transformers: https://www.sbert.net/
- HNSW 算法: https://arxiv.org/abs/1603.09320
- RAG 最佳实践: https://www.anthropic.com/index/retrieval-augmented-generation

---

**文档版本**: 1.0  
**最后更新**: 2026-02-02  
**状态**: 待实施
