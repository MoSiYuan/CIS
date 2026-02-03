# CIS Vector Intelligence 最终实施方案

**版本**: 1.0-FINAL  
**日期**: 2026-02-02  
**状态**: 待实施  

---

## 一、方案概述

### 1.1 核心目标

构建 **CIS Vector Intelligence (CVI)** - 基于 sqlite-vec 的语义智能层，实现：

| 目标 | 当前 | 目标值 | 提升 |
|------|------|--------|------|
| 记忆检索准确率 | ~30% (关键词) | ~85% (语义) | **+183%** |
| AI 回复相关性 | 60-70% | 85-95% | **+40-60%** |
| Skill 自动化程度 | 低 (硬编码) | 高 (自然语言) | **+200%** |
| 跨会话上下文 | 0% | 90%+ | **+∞** |

### 1.2 融合创新点

```
┌─────────────────────────────────────────────────────────────────┐
│                     CIS Vector Intelligence                      │
├─────────────────────────────────────────────────────────────────┤
│  基础层: sqlite-vec 向量存储 (Claude + Kimi)                    │
│  ├── memory_vec (记忆语义)                                       │
│  ├── task_vec (任务语义)                                         │
│  ├── session_vec (对话语义)                                      │
│  └── skill_vec (能力语义) ⭐ Kimi 独有创新                       │
├─────────────────────────────────────────────────────────────────┤
│  服务层: 语义服务 (融合设计)                                     │
│  ├── ConversationContext (Claude 对话持久化)                     │
│  ├── TaskVectorIndex (Kimi 任务向量)                             │
│  └── SkillVectorRouter ⭐ (Kimi Skill 自动化 - 核心创新)         │
├─────────────────────────────────────────────────────────────────┤
│  应用层: 智能接口                                                │
│  ├── RAG Service (统一检索增强)                                  │
│  ├── Intent Parser (意图解析) ⭐                                 │
│  └── Skill Chain Orchestrator (链式编排) ⭐                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 二、系统架构

### 2.1 完整数据流

```
用户输入: "帮我分析今天的销售数据并生成PDF报表"
    ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 1: Intent Parser (意图解析)                                │
│ ├── 文本嵌入 → [0.12, 0.85, -0.33, ...]                        │
│ ├── NER 提取 → {date: "today", data: "sales", output: "pdf"}     │
│ └── 动作分类 → Analyze + Generate                               │
└─────────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 2: Skill Vector Router (Skill 向量路由) ⭐                  │
│                                                                 │
│  Query: "分析销售数据"                                           │
│     ↓ 相似度搜索 skill_intent_vec                                │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ data-analyzer │ intent: [0.92] │ cap: [CSV, JSON]          ││
│  │ report-gen    │ intent: [0.89] │ cap: [PDF, Excel]         ││
│  │ email-skill   │ intent: [0.45] │ cap: [send]               ││
│  └─────────────────────────────────────────────────────────────┘│
│     ↓                                                           │
│  主匹配: data-analyzer (0.92)                                   │
│  链候选: report-gen (0.89)                                      │
└─────────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 3: Parameter Resolver (参数解析)                           │
│ ├── data_source: "sales_data" (from NER date=today)             │
│ ├── analysis_type: "summary" (默认)                              │
│ └── output_format: "pdf" (from NER)                             │
└─────────────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────────────┐
│ Step 4: Skill Chain Orchestrator (链式编排) ⭐                   │
│                                                                 │
│  Chain: data-analyzer → report-gen                              │
│  Step 1: data-analyzer.analyze(data_source="sales", ...)        │
│     ↓ result: {charts, summary}                                  │
│  Step 2: report-gen.generate(data=step1_result, format="pdf")   │
│     ↓ result: /reports/sales_20240115.pdf                       │
└─────────────────────────────────────────────────────────────────┘
    ↓
输出: "✅ 分析完成！今日销售额同比增长15%，报表已生成: /reports/sales_20240115.pdf"
```

### 2.2 数据库架构

```sql
-- ============================================
-- 1. 向量核心表 (sqlite-vec)
-- ============================================

-- 记忆向量表
CREATE VIRTUAL TABLE memory_embeddings USING vec0(
    memory_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    key TEXT,
    category TEXT,
    created_at INTEGER
);

-- 任务向量表 (多字段)
CREATE VIRTUAL TABLE task_title_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[768]
);
CREATE VIRTUAL TABLE task_description_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[768]
);
CREATE VIRTUAL TABLE task_result_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[768]
);

-- 对话消息向量表
CREATE VIRTUAL TABLE message_embeddings USING vec0(
    message_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    conversation_id TEXT,
    role TEXT,
    content TEXT
);

-- 对话摘要向量表
CREATE VIRTUAL TABLE summary_embeddings USING vec0(
    conversation_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    summary TEXT,
    project_path TEXT
);

-- Skill 意图向量表 ⭐ 核心创新
CREATE VIRTUAL TABLE skill_intent_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    skill_name TEXT,
    description TEXT
);

-- Skill 能力向量表 ⭐ 核心创新
CREATE VIRTUAL TABLE skill_capability_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768],
    capabilities TEXT -- JSON
);

-- ============================================
-- 2. 关系与元数据表 (标准 SQLite)
-- ============================================

-- 对话元数据
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    project_path TEXT,
    summary TEXT,
    topics TEXT, -- JSON array
    created_at INTEGER,
    updated_at INTEGER
);

-- 对话消息
CREATE TABLE conversation_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT, -- 'User', 'Assistant', 'System'
    content TEXT,
    timestamp INTEGER,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

-- Skill 语义描述 ⭐ 核心创新
CREATE TABLE skill_semantics (
    skill_id TEXT PRIMARY KEY,
    skill_name TEXT NOT NULL,
    description TEXT,
    example_intents TEXT, -- JSON array
    parameter_schema TEXT, -- JSON schema
    io_signature TEXT, -- JSON
    related_skills TEXT, -- JSON array
    registered_at INTEGER,
    updated_at INTEGER
);

-- Skill 兼容性矩阵 (自动发现)
CREATE TABLE skill_compatibility (
    source_skill_id TEXT,
    target_skill_id TEXT,
    compatibility_score REAL,
    data_flow_types TEXT,
    discovered_at INTEGER,
    PRIMARY KEY (source_skill_id, target_skill_id)
);

-- 意图执行历史 (用于优化)
CREATE TABLE intent_execution_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_input TEXT,
    input_vector BLOB,
    matched_skill_id TEXT,
    match_score REAL,
    parameters TEXT,
    execution_success BOOLEAN,
    feedback_score INTEGER,
    executed_at INTEGER
);
```

---

## 三、核心组件实现

### 3.1 VectorStorage (统一向量存储)

```rust
// cis-core/src/vector/storage.rs

pub struct VectorStorage {
    conn: Connection,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl VectorStorage {
    // ========== 记忆向量操作 ==========
    
    pub async fn index_memory(&self, key: &str, value: &[u8]) -> Result<()> {
        let text = String::from_utf8_lossy(value);
        let embedding = self.embedding_service.embed(&text).await?;
        
        self.conn.execute(
            "INSERT INTO memory_embeddings (memory_id, embedding, key)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(memory_id) DO UPDATE SET embedding = excluded.embedding",
            (Uuid::new_v4().to_string(), &embedding as &[f32], key),
        )?;
        Ok(())
    }
    
    pub async fn search_memory(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>> {
        let query_vec = self.embedding_service.embed(query).await?;
        
        let mut stmt = self.conn.prepare(
            "SELECT m.key, m.content, v.distance
             FROM memory_embeddings e
             JOIN memory_entries m ON e.key = m.key
             JOIN vec_memory_embeddings v ON e.memory_id = v.memory_id
             WHERE v.embedding MATCH ?1
             AND k = ?2
             ORDER BY v.distance
             LIMIT ?2"
        )?;
        
        let results = stmt.query_map((&query_vec as &[f32], limit as i32), |row| {
            Ok(MemoryResult {
                key: row.get(0)?,
                content: row.get(1)?,
                similarity: 1.0 - row.get::<_, f32>(2)?,
            })
        })?;
        
        results.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }
    
    // ========== Task 向量操作 ==========
    
    pub async fn index_task(&self, task: &Task) -> Result<()> {
        // 标题向量
        let title_vec = self.embedding_service.embed(&task.title).await?;
        self.conn.execute(
            "INSERT INTO task_title_vec (task_id, embedding) VALUES (?1, ?2)",
            (&task.id, &title_vec as &[f32]),
        )?;
        
        // 描述向量
        if let Some(desc) = &task.description {
            let desc_vec = self.embedding_service.embed(desc).await?;
            self.conn.execute(
                "INSERT INTO task_description_vec (task_id, embedding) VALUES (?1, ?2)",
                (&task.id, &desc_vec as &[f32]),
            )?;
        }
        
        // 结果向量
        if let Some(result) = &task.result {
            let result_vec = self.embedding_service.embed(result).await?;
            self.conn.execute(
                "INSERT INTO task_result_vec (task_id, embedding) VALUES (?1, ?2)",
                (&task.id, &result_vec as &[f32]),
            )?;
        }
        
        Ok(())
    }
    
    // ========== Session/对话向量操作 ==========
    
    pub async fn index_message(&self, msg: &ConversationMessage) -> Result<()> {
        let embedding = self.embedding_service.embed(&msg.content).await?;
        
        self.conn.execute(
            "INSERT INTO message_embeddings (message_id, embedding, conversation_id, role, content)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (&msg.id, &embedding as &[f32], &msg.conversation_id, &msg.role, &msg.content),
        )?;
        Ok(())
    }
    
    pub async fn index_conversation_summary(&self, conv: &Conversation) -> Result<()> {
        if let Some(summary) = &conv.summary {
            let embedding = self.embedding_service.embed(summary).await?;
            
            self.conn.execute(
                "INSERT INTO summary_embeddings (conversation_id, embedding, summary, project_path)
                 VALUES (?1, ?2, ?3, ?4)",
                (&conv.id, &embedding as &[f32], summary, &conv.project_path),
            )?;
        }
        Ok(())
    }
    
    // ========== Skill 向量操作 ⭐ 核心创新 ==========
    
    pub async fn register_skill_semantics(&self, semantics: &SkillSemantics) -> Result<()> {
        // 1. 保存元数据
        self.conn.execute(
            "INSERT INTO skill_semantics 
             (skill_id, skill_name, description, example_intents, parameter_schema, io_signature, related_skills, registered_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                semantics.skill_id,
                semantics.skill_name,
                semantics.description,
                serde_json::to_string(&semantics.example_intents)?,
                serde_json::to_string(&semantics.parameter_schema)?,
                serde_json::to_string(&semantics.io_signature)?,
                serde_json::to_string(&semantics.related_skills)?,
                semantics.registered_at.timestamp(),
                semantics.updated_at.timestamp(),
            ],
        )?;
        
        // 2. 意图向量
        let intent_text = semantics.example_intents.join("; ");
        let intent_vec = self.embedding_service.embed(&intent_text).await?;
        self.conn.execute(
            "INSERT INTO skill_intent_vec (skill_id, embedding, skill_name, description)
             VALUES (?1, ?2, ?3, ?4)",
            (&semantics.skill_id, &intent_vec as &[f32], &semantics.skill_name, &semantics.description),
        )?;
        
        // 3. 能力向量
        let capabilities = format!("{:?}", semantics.io_signature.input_types);
        let cap_vec = self.embedding_service.embed(&capabilities).await?;
        self.conn.execute(
            "INSERT INTO skill_capability_vec (skill_id, embedding, capabilities)
             VALUES (?1, ?2, ?3)",
            (&semantics.skill_id, &cap_vec as &[f32], capabilities),
        )?;
        
        Ok(())
    }
    
    /// 语义搜索 Skill ⭐ 核心方法
    pub async fn search_skills_by_intent(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SkillMatchResult>> {
        let query_vec = self.embedding_service.embed(query).await?;
        
        let mut stmt = self.conn.prepare(
            "SELECT s.skill_id, s.skill_name, s.description, v.distance
             FROM skill_intent_vec si
             JOIN skill_semantics s ON si.skill_id = s.skill_id
             JOIN vec_skill_intent_vec v ON si.skill_id = v.skill_id
             WHERE v.embedding MATCH ?1
             AND k = ?2
             ORDER BY v.distance
             LIMIT ?2"
        )?;
        
        let results = stmt.query_map((&query_vec as &[f32], limit as i32), |row| {
            let distance: f32 = row.get(3)?;
            Ok(SkillMatchResult {
                skill_id: row.get(0)?,
                skill_name: row.get(1)?,
                description: row.get(2)?,
                similarity: 1.0 - distance,
            })
        })?;
        
        results
            .filter_map(|r| r.ok())
            .filter(|r| r.similarity >= threshold)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
}
```

### 3.2 ConversationContext (对话上下文管理)

```rust
// cis-core/src/conversation/context.rs

pub struct ConversationContext {
    db: Arc<ConversationDb>,
    vector_storage: Arc<VectorStorage>,
    current_session_id: Arc<RwLock<Option<String>>>,
    current_conversation: Arc<RwLock<Option<Conversation>>>,
}

impl ConversationContext {
    /// 开始新对话
    pub async fn start_conversation(&self, session_id: String, project_path: Option<String>) -> Result<()> {
        let conv = Conversation {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            project_path,
            messages: Vec::new(),
            summary: None,
            topics: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        self.db.save_conversation(&conv).await?;
        
        *self.current_session_id.write().await = Some(session_id);
        *self.current_conversation.write().await = Some(conv);
        
        Ok(())
    }
    
    /// 添加用户消息
    pub async fn add_user_message(&self, content: &str) -> Result<()> {
        let msg = ConversationMessage {
            id: Uuid::new_v4().to_string(),
            conversation_id: self.get_current_conversation_id().await?,
            role: "User".to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        };
        
        // 保存消息
        self.db.save_message(&msg).await?;
        
        // 向量索引 (异步)
        let storage = self.vector_storage.clone();
        tokio::spawn(async move {
            if let Err(e) = storage.index_message(&msg).await {
                tracing::error!("Failed to index message: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 添加助手消息
    pub async fn add_assistant_message(&self, content: &str) -> Result<()> {
        let msg = ConversationMessage {
            id: Uuid::new_v4().to_string(),
            conversation_id: self.get_current_conversation_id().await?,
            role: "Assistant".to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        };
        
        self.db.save_message(&msg).await?;
        
        // 向量索引
        let storage = self.vector_storage.clone();
        tokio::spawn(async move {
            if let Err(e) = storage.index_message(&msg).await {
                tracing::error!("Failed to index message: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 查找相似对话 (跨目录恢复核心)
    pub async fn find_similar_conversations(&self, query: &str, limit: usize) -> Result<Vec<Conversation>> {
        let query_vec = self.vector_storage.embedding_service().embed(query).await?;
        
        let mut stmt = self.db.conn().prepare(
            "SELECT c.*, v.distance
             FROM summary_embeddings se
             JOIN conversations c ON se.conversation_id = c.id
             JOIN vec_summary_embeddings v ON se.conversation_id = v.conversation_id
             WHERE v.embedding MATCH ?1
             AND k = ?2
             ORDER BY v.distance
             LIMIT ?2"
        )?;
        
        let results = stmt.query_map((&query_vec as &[f32], limit as i32), |row| {
            // 解析 Conversation 结构
            Ok(Conversation {
                id: row.get(0)?,
                session_id: row.get(1)?,
                project_path: row.get(2)?,
                summary: row.get(3)?,
                topics: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                messages: Vec::new(),
            })
        })?;
        
        results.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }
    
    /// 保存并生成摘要
    pub async fn save_with_summary(&self) -> Result<()> {
        let conversation = self.current_conversation.read().await.clone()
            .ok_or_else(|| CisError::conversation("No active conversation"))?;
        
        // 1. 生成摘要
        let summary = self.generate_summary(&conversation).await?;
        
        // 2. 提取主题
        let topics = self.extract_topics(&conversation).await?;
        
        // 3. 更新对话
        self.db.update_summary(&conversation.id, &summary, &topics).await?;
        
        // 4. 向量索引摘要
        let conv_with_summary = Conversation {
            summary: Some(summary),
            topics,
            ..conversation
        };
        self.vector_storage.index_conversation_summary(&conv_with_summary).await?;
        
        Ok(())
    }
    
    /// 为 AI 准备增强 Prompt
    pub async fn prepare_ai_prompt(&self, user_input: &str) -> Result<String> {
        // 1. 查找相关历史对话
        let similar = self.find_similar_conversations(user_input, 3).await?;
        
        // 2. 构建上下文
        let mut context_parts = Vec::new();
        
        if !similar.is_empty() {
            context_parts.push("=== 相关历史对话 ===".to_string());
            for conv in similar.iter().take(2) {
                if let Some(summary) = &conv.summary {
                    context_parts.push(format!("- {}", summary));
                }
            }
        }
        
        // 3. 构建最终 prompt
        let context = context_parts.join("\n");
        let enhanced = if context.is_empty() {
            user_input.to_string()
        } else {
            format!("{}\n\n=== 当前问题 ===\n{}", context, user_input)
        };
        
        Ok(enhanced)
    }
    
    async fn generate_summary(&self, conversation: &Conversation) -> Result<String> {
        // 简化的摘要生成，实际可以调用 LLM
        let message_count = conversation.messages.len();
        let first_msg = conversation.messages.first()
            .map(|m| m.content.chars().take(50).collect::<String>())
            .unwrap_or_default();
        
        Ok(format!(
            "对话包含 {} 条消息，主题: {}",
            message_count,
            first_msg
        ))
    }
    
    async fn extract_topics(&self, conversation: &Conversation) -> Result<Vec<String>> {
        // 简化的主题提取
        Ok(vec!["general".to_string()])
    }
}
```

### 3.3 SkillVectorRouter ⭐ 核心创新

```rust
// cis-core/src/skill/vector_router.rs

pub struct SkillVectorRouter {
    vector_storage: Arc<VectorStorage>,
    skill_manager: Arc<SkillManager>,
    intent_parser: Arc<IntentParser>,
    param_resolver: Arc<ParameterResolver>,
}

impl SkillVectorRouter {
    /// 自然语言调用 Skill (核心方法)
    pub async fn route_by_intent(&self, user_input: &str) -> Result<SkillRoutingResult> {
        // 1. 解析意图
        let parsed = self.intent_parser.parse(user_input).await?;
        tracing::info!("Parsed intent: {:?}, confidence: {}", parsed.action_type, parsed.confidence);
        
        // 2. 语义搜索匹配 Skill
        let matches = self.vector_storage
            .search_skills_by_intent(&parsed.normalized_intent, 5, 0.6)
            .await?;
        
        if matches.is_empty() {
            return Err(CisError::skill(format!("No matching skill for: {}", user_input)));
        }
        
        // 3. 发现 Skill 链
        let primary_match = &matches[0];
        let chain = self.discover_skill_chain(&primary_match.skill_id, &parsed).await?;
        
        // 4. 解析参数
        let params = self.param_resolver
            .resolve(&primary_match.skill_id, &parsed)
            .await?;
        
        Ok(SkillRoutingResult {
            primary_skill: primary_match.clone(),
            skill_chain: chain,
            parameters: params,
            confidence: parsed.confidence * primary_match.similarity,
        })
    }
    
    /// 发现 Skill 链 (多步编排)
    async fn discover_skill_chain(
        &self,
        primary_skill_id: &str,
        parsed_intent: &ParsedIntent,
    ) -> Result<SkillChain> {
        // 1. 获取主 Skill 的 IO 签名
        let io_sig = self.get_skill_io_signature(primary_skill_id).await?;
        
        // 2. 如果已经是 sink，无需链式
        if io_sig.sink {
            return Ok(SkillChain::single(primary_skill_id));
        }
        
        // 3. 查找兼容的后续 Skills
        let output_type = &io_sig.output_types[0];
        let compatible = self.find_compatible_skills(primary_skill_id, output_type).await?;
        
        // 4. 根据意图匹配度排序
        let mut candidates = Vec::new();
        for skill in compatible {
            let intent_sim = self.calculate_intent_similarity(
                &parsed_intent.normalized_intent,
                &skill.skill_id
            ).await?;
            candidates.push((skill, intent_sim));
        }
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // 5. 构建链
        if let Some((next_skill, _)) = candidates.first() {
            Ok(SkillChain {
                steps: vec![
                    ChainStep {
                        skill_id: primary_skill_id.to_string(),
                        input_mapping: InputMapping::Direct,
                        output_mapping: OutputMapping::PassThrough,
                    },
                    ChainStep {
                        skill_id: next_skill.skill_id.clone(),
                        input_mapping: InputMapping::FromPrevious("data".to_string()),
                        output_mapping: OutputMapping::Final,
                    },
                ],
            })
        } else {
            Ok(SkillChain::single(primary_skill_id))
        }
    }
    
    /// 执行 Skill 链
    pub async fn execute_chain(&self, chain: &SkillChain, params: &ResolvedParameters) -> Result<ChainExecutionResult> {
        let mut context = serde_json::json!({});
        let mut step_results = Vec::new();
        
        for (idx, step) in chain.steps.iter().enumerate() {
            tracing::info!("Executing step {}: {}", idx, step.skill_id);
            
            // 准备输入
            let input = self.prepare_step_input(step, &context, params)?;
            
            // 执行 Skill
            let result = self.execute_skill(&step.skill_id, &input).await?;
            
            // 更新上下文
            context = self.update_context(context, step, &result)?;
            
            step_results.push(StepResult {
                step_idx: idx,
                skill_id: step.skill_id.clone(),
                success: true,
                output: result.clone(),
            });
        }
        
        Ok(ChainExecutionResult {
            final_output: context,
            step_results,
        })
    }
    
    async fn execute_skill(&self, skill_id: &str, input: &serde_json::Value) -> Result<serde_json::Value> {
        // 确保 Skill 已加载
        if !self.skill_manager.is_loaded(skill_id)? {
            self.skill_manager.load(skill_id, LoadOptions::default()).await?;
        }
        
        // 构造执行事件
        let event = Event::Custom {
            name: "execute".to_string(),
            data: input.clone(),
        };
        
        // 获取 Skill 实例
        let skill = self.skill_manager.get_active_skill(skill_id)?;
        let ctx = self.create_execution_context(skill_id);
        
        // 执行
        skill.handle_event(&ctx, event).await?;
        
        // 返回结果 (通过上下文获取)
        Ok(ctx.take_result().unwrap_or(serde_json::json!({"status": "ok"})))
    }
    
    /// 自动发现 Skill 兼容性 (后台任务)
    pub async fn auto_discover_compatibility(&self) -> Result<()> {
        let skills = self.vector_storage.list_all_skills().await?;
        
        for source in &skills {
            for target in &skills {
                if source.skill_id == target.skill_id {
                    continue;
                }
                
                // 检查 IO 兼容性
                let source_outputs = &source.io_signature.output_types;
                let target_inputs = &target.io_signature.input_types;
                
                for output in source_outputs {
                    if target_inputs.contains(output) {
                        let score = 0.85; // 可以计算更复杂的分数
                        
                        self.vector_storage.conn().execute(
                            "INSERT INTO skill_compatibility 
                             (source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)
                             ON CONFLICT DO UPDATE SET compatibility_score = excluded.compatibility_score",
                            params![
                                source.skill_id,
                                target.skill_id,
                                score,
                                serde_json::json!({"input": output, "output": target.io_signature.output_types[0]}).to_string(),
                                Utc::now().timestamp(),
                            ],
                        )?;
                    }
                }
            }
        }
        
        Ok(())
    }
}
```

### 3.4 IntentParser (意图解析)

```rust
// cis-core/src/intent/parser.rs

pub struct IntentParser {
    embedding_service: Arc<dyn EmbeddingService>,
    ner_model: Option<Arc<dyn NerModel>>,
}

impl IntentParser {
    pub async fn parse(&self, input: &str) -> Result<ParsedIntent> {
        // 1. 生成嵌入
        let embedding = self.embedding_service.embed(input).await?;
        
        // 2. 实体提取 (NER)
        let entities = self.extract_entities(input).await?;
        
        // 3. 规范化意图
        let normalized = self.normalize_intent(input, &entities);
        
        // 4. 动作分类
        let action_type = self.classify_action(input);
        
        // 5. 计算置信度
        let confidence = self.calculate_confidence(input, &entities);
        
        Ok(ParsedIntent {
            raw_input: input.to_string(),
            normalized_intent: normalized,
            embedding,
            entities,
            action_type,
            confidence,
        })
    }
    
    async fn extract_entities(&self, input: &str) -> Result<HashMap<String, EntityValue>> {
        let mut entities = HashMap::new();
        
        // 时间实体
        if let Some(date) = self.extract_datetime(input) {
            entities.insert("time".to_string(), EntityValue::DateTime(date));
        }
        
        // 文件路径
        if let Some(path) = self.extract_file_path(input) {
            entities.insert("file".to_string(), EntityValue::FilePath(path));
        }
        
        // 数字
        for (i, num) in self.extract_numbers(input).iter().enumerate() {
            entities.insert(format!("number_{}", i), EntityValue::Number(*num));
        }
        
        // 使用 NER 模型
        if let Some(ner) = &self.ner_model {
            let ner_results = ner.extract(input).await?;
            for (key, value) in ner_results {
                entities.insert(key, EntityValue::String(value));
            }
        }
        
        Ok(entities)
    }
    
    fn classify_action(&self, input: &str) -> ActionType {
        let lower = input.to_lowercase();
        
        if lower.contains("分析") || lower.contains("analyze") {
            ActionType::Analyze
        } else if lower.contains("生成") || lower.contains("create") || lower.contains("generate") {
            ActionType::Generate
        } else if lower.contains("提交") || lower.contains("commit") || lower.contains("push") {
            ActionType::Commit
        } else if lower.contains("查询") || lower.contains("search") || lower.contains("find") {
            ActionType::Query
        } else if lower.contains("发送") || lower.contains("send") || lower.contains("email") {
            ActionType::Send
        } else {
            ActionType::Execute
        }
    }
    
    fn normalize_intent(&self, input: &str, entities: &HashMap<String, EntityValue>) -> String {
        let mut normalized = input.to_string();
        
        // 替换实体为占位符
        for (key, value) in entities {
            let placeholder = format!("[{}]", key.to_uppercase());
            let value_str = match value {
                EntityValue::String(s) => s.clone(),
                EntityValue::DateTime(dt) => dt.to_rfc3339(),
                EntityValue::FilePath(p) => p.to_string_lossy().to_string(),
                _ => continue,
            };
            normalized = normalized.replace(&value_str, &placeholder);
        }
        
        normalized
    }
    
    fn extract_datetime(&self, input: &str) -> Option<DateTime<Utc>> {
        // 简化实现，实际可以使用 dateparser crate
        let lower = input.to_lowercase();
        
        if lower.contains("今天") || lower.contains("today") {
            Some(Utc::now())
        } else if lower.contains("明天") || lower.contains("tomorrow") {
            Some(Utc::now() + chrono::Duration::days(1))
        } else {
            None
        }
    }
    
    fn extract_file_path(&self, input: &str) -> Option<std::path::PathBuf> {
        // 简单的路径匹配
        let path_regex = regex::Regex::new(r"[\w./~_-]+\.(csv|json|pdf|txt|md)").ok()?;
        path_regex.find(input).map(|m| std::path::PathBuf::from(m.as_str()))
    }
    
    fn extract_numbers(&self, input: &str) -> Vec<f64> {
        let num_regex = regex::Regex::new(r"\d+\.?\d*").unwrap();
        num_regex.find_iter(input)
            .filter_map(|m| m.as_str().parse().ok())
            .collect()
    }
}
```

---

## 四、执行计划 (已拆解为 Task)

### Phase 1: 基础设施 (Week 1)

#### Task 1.1: 添加 sqlite-vec 依赖和基础集成
**优先级**: P0  
**负责人**: TBD  
**工时**: 2 天  
**依赖**: 无  

**子任务**:
- [ ] 在 `cis-core/Cargo.toml` 添加 `sqlite-vec` 依赖
- [ ] 创建 `VectorStorage` 基础结构
- [ ] 实现 sqlite-vec 扩展加载
- [ ] 创建向量表 (memory/task/session/skill)

**验收标准**:
```rust
let storage = VectorStorage::open_default()?;
let vec = storage.embedding_service().embed("测试文本").await?;
assert_eq!(vec.len(), 768); // 或模型对应的维度
```

---

#### Task 1.2: 实现 Embedding Service
**优先级**: P0  
**工时**: 2 天  
**依赖**: Task 1.1  

**子任务**:
- [ ] 定义 `EmbeddingService` trait
- [ ] 实现本地模型 (MiniLM-L6-v2)
- [ ] 实现云端 API 适配器 (OpenAI)
- [ ] 实现降级机制 (本地失败→云端)

**验收标准**:
```rust
let service = EmbeddingService::local()?;
let vec1 = service.embed("分析数据").await?;
let vec2 = service.embed("数据分析").await?;
let similarity = cosine_similarity(&vec1, &vec2);
assert!(similarity > 0.8); // 语义相似
```

---

### Phase 2: 记忆与 Task 向量 (Week 2)

#### Task 2.1: Memory 向量索引
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 1.1, 1.2  

**子任务**:
- [ ] 实现 `MemoryDb.set_with_embedding()`
- [ ] 实现 `MemoryDb.semantic_search()`
- [ ] 扩展 `SkillContext` 添加语义搜索接口
- [ ] 添加批量向量化支持

**验收标准**:
```rust
memory.set_with_embedding("key", "用户喜欢深色主题").await?;
let results = memory.semantic_search("暗黑模式", 5).await?;
assert!(results[0].similarity > 0.85);
```

---

#### Task 2.2: Task 向量索引
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 1.1, 1.2  

**子任务**:
- [ ] 创建 Task 向量表 (title/description/result)
- [ ] 实现 `Task.index_vectors()`
- [ ] 实现 `Task.semantic_search()`
- [ ] 实现相似 Task 自动发现

**验收标准**:
```rust
let task = Task::new("优化数据库查询性能");
task.index_vectors().await?;
let similar = Task::find_similar(&task.id, 0.8).await?;
// 应该找到 "PostgreSQL 索引优化" 等相关任务
```

---

### Phase 3: 对话持久化 (Week 3)

#### Task 3.1: ConversationDb 实现
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 1.1  

**子任务**:
- [ ] 创建 `ConversationDb` 结构
- [ ] 实现 `conversations` 表 CRUD
- [ ] 实现 `conversation_messages` 表 CRUD
- [ ] 实现消息向量索引

**验收标准**:
```rust
let db = ConversationDb::open_default()?;
let conv = Conversation::new(session_id);
db.save_conversation(&conv).await?;
let loaded = db.get_conversation(&conv.id).await?;
assert_eq!(loaded.id, conv.id);
```

---

#### Task 3.2: ConversationContext 实现
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 3.1, 2.1  

**子任务**:
- [ ] 实现 `ConversationContext` 结构
- [ ] 实现对话生命周期管理
- [ ] 实现相似对话搜索 (跨目录)
- [ ] 实现摘要生成和主题提取

**验收标准**:
```rust
let ctx = ConversationContext::new();
ctx.start_conversation(session_id, project_path).await?;
ctx.add_user_message("如何设置导航到沙发？").await?;
ctx.add_assistant_message("导航已设置...").await?;
ctx.save_with_summary().await?;
```

---

### Phase 4: Skill 向量自动化 ⭐ 核心创新 (Week 4)

#### Task 4.1: Skill 向量注册表
**优先级**: P0  
**工时**: 2 天  
**依赖**: Task 1.1, 1.2  

**子任务**:
- [ ] 创建 `skill_semantics` 表
- [ ] 创建 `skill_intent_vec` 虚拟表
- [ ] 创建 `skill_capability_vec` 虚拟表
- [ ] 实现 `SkillVectorRegistry.register_semantics()`
- [ ] 扩展 `Skill` trait 添加 `semantics()` 方法

**验收标准**:
```rust
impl Skill for DataAnalyzerSkill {
    fn semantics(&self) -> SkillSemantics {
        SkillSemantics {
            skill_id: "data-analyzer".to_string(),
            example_intents: vec![
                "分析今天的销售数据".to_string(),
                "帮我看看这份CSV文件".to_string(),
            ],
            // ...
        }
    }
}

let registry = SkillVectorRegistry::open_default()?;
registry.register_semantics(skill.semantics()).await?;
```

---

#### Task 4.2: Intent Parser 实现
**优先级**: P0  
**工时**: 2 天  
**依赖**: Task 4.1  

**子任务**:
- [ ] 实现 `IntentParser` 结构
- [ ] 实现实体提取 (NER)
- [ ] 实现动作分类
- [ ] 实现意图规范化

**验收标准**:
```rust
let parser = IntentParser::new();
let parsed = parser.parse("分析今天的销售数据").await?;
assert_eq!(parsed.action_type, ActionType::Analyze);
assert!(parsed.entities.contains_key("time"));
```

---

#### Task 4.3: Skill Vector Router 实现
**优先级**: P0  
**工时**: 3 天  
**依赖**: Task 4.1, 4.2  

**子任务**:
- [ ] 实现 `SkillVectorRouter` 结构
- [ ] 实现 `route_by_intent()` 核心方法
- [ ] 实现 Skill 语义搜索
- [ ] 实现 `execute_chain()` 链式执行

**验收标准**:
```rust
let router = SkillVectorRouter::new();
let result = router.route_by_intent("分析今天的销售数据").await?;
assert_eq!(result.primary_skill.skill_id, "data-analyzer");
assert!(result.confidence > 0.8);
```

---

#### Task 4.4: Skill Chain Orchestrator
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 4.3  

**子任务**:
- [ ] 实现 `SkillChain` 结构
- [ ] 实现 `discover_skill_chain()`
- [ ] 实现 `skill_compatibility` 自动发现
- [ ] 实现参数传递映射

**验收标准**:
```rust
// "分析并生成报告" 应该触发链式调用
let chain = router.discover_skill_chain("data-analyzer", &parsed).await?;
assert_eq!(chain.steps.len(), 2);
assert_eq!(chain.steps[0].skill_id, "data-analyzer");
assert_eq!(chain.steps[1].skill_id, "report-gen");
```

---

### Phase 5: 集成与优化 (Week 5)

#### Task 5.1: AI Provider RAG 集成
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 3.2  

**子任务**:
- [ ] 更新 `AiProvider` trait 添加 `chat_with_context()`
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

---

#### Task 5.2: CLI 命令实现
**优先级**: P2  
**工时**: 2 天  
**依赖**: Task 4.3, 5.1  

**子任务**:
- [ ] 实现 `cis skill do <自然语言>` 命令
- [ ] 实现 `cis skill chain <描述>` 命令
- [ ] 实现 `cis agent context <描述>` 命令
- [ ] 实现 `cis memory search <查询>` 命令

**验收标准**:
```bash
cis skill do "分析今天的销售数据"
# 输出: ✅ 已匹配 data-analyzer skill (相似度: 0.92)
# 输出: ✅ 执行结果: ...
```

---

#### Task 5.3: 性能优化
**优先级**: P2  
**工时**: 1 天  
**依赖**: 全部  

**子任务**:
- [ ] 添加 HNSW 索引
- [ ] 实现批量向量化
- [ ] 实现异步向量索引
- [ ] 性能基准测试

**验收标准**:
- 10k 向量搜索 < 50ms
- 100k 向量搜索 < 100ms
- 批量向量化 1000 条 < 5s

---

### Phase 6: 测试与文档 (Week 6)

#### Task 6.1: 单元测试
**优先级**: P1  
**工时**: 2 天  
**依赖**: 全部  

**子任务**:
- [ ] `VectorStorage` 单元测试
- [ ] `ConversationContext` 单元测试
- [ ] `SkillVectorRouter` 单元测试
- [ ] `IntentParser` 单元测试

**验收标准**:
- 测试覆盖率 > 80%
- 所有测试通过

---

#### Task 6.2: 集成测试
**优先级**: P1  
**工时**: 2 天  
**依赖**: Task 6.1  

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

#### Task 6.3: 文档
**优先级**: P2  
**工时**: 1 天  
**依赖**: 全部  

**子任务**:
- [ ] API 文档
- [ ] 使用指南
- [ ] Skill 开发文档 (如何添加语义描述)
- [ ] 部署文档

---

## 五、任务依赖图

```
Week 1: 基础设施
├── Task 1.1: sqlite-vec 依赖和基础集成
└── Task 1.2: Embedding Service

Week 2: 记忆与 Task 向量
├── Task 2.1: Memory 向量索引 (依赖 1.1, 1.2)
└── Task 2.2: Task 向量索引 (依赖 1.1, 1.2)

Week 3: 对话持久化
├── Task 3.1: ConversationDb (依赖 1.1)
└── Task 3.2: ConversationContext (依赖 3.1, 2.1)

Week 4: Skill 向量自动化 ⭐ 核心
├── Task 4.1: Skill 向量注册表 (依赖 1.1, 1.2)
├── Task 4.2: Intent Parser (依赖 4.1)
├── Task 4.3: Skill Vector Router (依赖 4.1, 4.2)
└── Task 4.4: Skill Chain Orchestrator (依赖 4.3)

Week 5: 集成与优化
├── Task 5.1: AI Provider RAG 集成 (依赖 3.2)
├── Task 5.2: CLI 命令 (依赖 4.3, 5.1)
└── Task 5.3: 性能优化 (依赖 全部)

Week 6: 测试与文档
├── Task 6.1: 单元测试 (依赖 全部)
├── Task 6.2: 集成测试 (依赖 6.1)
└── Task 6.3: 文档 (依赖 全部)
```

---

## 六、成功指标

| 指标 | 目标值 | 验证方法 |
|------|--------|---------|
| 记忆语义搜索准确率 | > 85% | 测试查询 100 条，人工验证相关性 |
| Skill 意图匹配准确率 | > 80% | 测试 50 个自然语言指令 |
| Skill 链发现准确率 | > 75% | 测试 20 个多步骤场景 |
| 向量搜索延迟 (10k) | < 50ms | 基准测试 |
| 跨目录上下文恢复率 | > 90% | 模拟 10 个跨项目场景 |
| 端到端调用延迟 | < 2s | 完整流程测试 |

---

## 七、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| 本地模型性能不足 | 嵌入延迟高 | 降级到云端 API；使用更小的模型 |
| 向量索引空间膨胀 | 数据库过大 | 定期清理低价值向量；摘要压缩 |
| Skill 匹配误判 | 执行错误 Skill | 置信度阈值；用户确认机制；反馈优化 |
| 隐私泄露 (云端模式) | 数据离开本地 | 默认本地；可选云端；敏感数据过滤 |

---

## 八、附录

### A. 关键设计决策

1. **向量维度选择**: 768 (MiniLM-L6-v2 平衡性能和效果)
2. **相似度阈值**: 0.6 (Skill 匹配), 0.7 (记忆搜索)
3. **HNSW 索引**: 默认启用，10k 以上向量
4. **嵌入模型**: 本地优先，云端降级

### B. 参考资源

- [sqlite-vec GitHub](https://github.com/asg017/sqlite-vec)
- [sentence-transformers](https://www.sbert.net/)
- [HNSW 算法](https://arxiv.org/abs/1603.09320)
- [RAG 最佳实践](https://www.anthropic.com/index/retrieval-augmented-generation)

---

**文档版本**: 1.0-FINAL  
**最后更新**: 2026-02-02  
**状态**: 待实施
