# CIS sqlite-vec 集成方案：Task + Session 向量记忆

## 一、当前架构研判

### 1.1 Agent Provider (Kimi/Claude) 现状

```rust
// cis-core/src/agent/providers/kimi.rs
pub struct KimiProvider {
    config: AgentConfig,  // ← 仅有配置，无状态
}

#[async_trait]
impl AgentProvider for KimiProvider {
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        // 直接调用 CLI，无任何记录
        let output = Command::new("kimi")
            .arg("chat")
            .arg(&req.prompt)
            .output().await?;
        
        Ok(AgentResponse { content: ... })
        // ↑ 返回后即丢失，无持久化
    }
}
```

**问题分析**:
- ❌ 每次调用都是无状态的一次性操作
- ❌ 无 Session ID 追踪
- ❌ 无对话历史记录
- ❌ 无法语义检索过去的 Agent 交互

### 1.2 ProjectSession 现状

```rust
// cis-core/src/project/session.rs
pub struct ProjectSession {
    project: Arc<Project>,
    agent_manager: Arc<AgentManager>,
    skill_manager: Arc<SkillManager>,
    db_manager: Arc<DbManager>,
}

impl ProjectSession {
    pub async fn call_agent(&self, prompt: impl Into<String>) -> Result<String> {
        // 创建请求
        let req = AgentRequest {
            prompt: prompt.into(),
            history: vec![],  // ← 始终为空！
            ...
        };
        
        // 调用后无任何记录
        let response = agent.execute(req).await?;
        Ok(response.content)
    }
}
```

**问题分析**:
- ❌ `history` 字段始终为空，未实现多轮对话
- ❌ 无 Session 级别的记忆累积
- ❌ 无法基于历史上下文进行语义检索

### 1.3 Task 系统现状

```rust
// cis-core/src/types.rs
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
    // ...
}
```

```rust
// cis-core/src/memory/mod.rs
pub fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<MemoryEntryExt>> {
    let _ = (query, options);
    // TODO: 实现更复杂的搜索
    Ok(vec![])  // ← 完全未实现
}
```

**问题分析**:
- ❌ Task 元数据仅能精确匹配 key
- ❌ 无法通过语义描述找到相关 Task
- ❌ Task 结果无法被后续任务语义关联

---

## 二、集成目标

### 2.1 核心能力提升

| 能力 | 当前 | 集成后 |
|------|------|--------|
| Agent 记录 | ❌ 无 | ✅ 每次交互完整记录 |
| Session 记忆 | ❌ 无状态 | ✅ 多轮对话 + 向量索引 |
| Task 检索 | ❌ 仅 key 匹配 | ✅ 语义相似度搜索 |
| 跨 Session 关联 | ❌ 无 | ✅ 语义发现相关历史 |

### 2.2 数据流向设计

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Agent 调用层                                 │
│  ┌──────────────────┐    ┌──────────────────┐                       │
│  │ KimiProvider     │    │ ClaudeProvider   │                       │
│  │ execute()        │    │ execute()        │                       │
│  └────────┬─────────┘    └────────┬─────────┘                       │
│           │                       │                                  │
│           ▼                       ▼                                  │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │              SessionRecorder (NEW)                           │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │   │
│  │  │ Session ID  │  │ Turn ID     │  │ Timestamp           │  │   │
│  │  │ Request     │  │ Response    │  │ Embedding Vector    │  │   │
│  │  │ Metadata    │  │ Token Usage │  │                     │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │   │
│  └────────────────────────┬────────────────────────────────────┘   │
└───────────────────────────┼─────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Vector Storage Layer                            │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  agent_sessions_vec (sqlite-vec virtual table)              │   │
│  │  ┌──────────────────────────────────────────────────────┐   │   │
│  │  │ rowid │ session_id │ agent_type │ turn_idx │ vector   │   │   │
│  │  │ 1     │ sess_abc   │ kimi       │ 0        │ [0.1...] │   │   │
│  │  │ 2     │ sess_abc   │ kimi       │ 1        │ [0.3...] │   │   │
│  │  │ 3     │ sess_def   │ claude     │ 0        │ [0.2...] │   │   │
│  │  └──────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  task_vec (sqlite-vec virtual table)                        │   │
│  │  ┌──────────────────────────────────────────────────────┐   │   │
│  │  │ rowid │ task_id │ title_vec │ desc_vec │ result_vec  │   │   │
│  │  │ 1     │ task_01 │ [0.5...]  │ [0.3...] │ [0.7...]    │   │   │
│  │  │ 2     │ task_02 │ [0.4...]  │ [0.2...] │ [0.6...]    │   │   │
│  │  └──────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 三、详细设计方案

### 3.1 核心表结构

```sql
-- ============================================
-- Agent Session 向量存储
-- ============================================

-- 1. Session 主表
CREATE TABLE agent_sessions (
    session_id TEXT PRIMARY KEY,
    project_id TEXT,
    agent_type TEXT NOT NULL,  -- 'kimi' | 'claude' | 'aider'
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    status TEXT DEFAULT 'active',  -- 'active' | 'closed' | 'error'
    context_json TEXT,  -- 序列化的 AgentContext
    
    -- 统计信息
    total_turns INTEGER DEFAULT 0,
    total_tokens_in INTEGER DEFAULT 0,
    total_tokens_out INTEGER DEFAULT 0
);

-- 2. Session Turn 详情表（原始数据）
CREATE TABLE agent_session_turns (
    turn_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_idx INTEGER NOT NULL,
    
    -- 请求
    request_prompt TEXT NOT NULL,
    request_system_prompt TEXT,
    request_skills TEXT,  -- JSON array
    
    -- 响应
    response_content TEXT NOT NULL,
    response_exit_code INTEGER,
    response_token_in INTEGER,
    response_token_out INTEGER,
    
    -- 时间
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    duration_ms INTEGER,
    
    -- 工作目录等上下文
    work_dir TEXT,
    
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);

-- 3. sqlite-vec 向量表 - Session Turn 语义索引
CREATE VIRTUAL TABLE agent_session_turns_vec USING vec0(
    turn_id TEXT PRIMARY KEY,           -- 关联到主表
    embedding FLOAT[1536] distance_metric=cosine  -- 向量维度
);

-- 4. Session 摘要向量表（用于快速检索相关 Session）
CREATE VIRTUAL TABLE agent_sessions_vec USING vec0(
    session_id TEXT PRIMARY KEY,
    summary_embedding FLOAT[1536] distance_metric=cosine
);

-- ============================================
-- Task 向量存储
-- ============================================

-- 1. Task 向量表（多字段独立索引，支持不同查询场景）
CREATE VIRTUAL TABLE task_title_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[1536] distance_metric=cosine
);

CREATE VIRTUAL TABLE task_description_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[1536] distance_metric=cosine
);

CREATE VIRTUAL TABLE task_result_vec USING vec0(
    task_id TEXT PRIMARY KEY,
    embedding FLOAT[1536] distance_metric=cosine
);

-- 2. Task 关联关系表（通过向量相似度自动发现）
CREATE TABLE task_relationships (
    source_task_id TEXT NOT NULL,
    target_task_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,  -- 'similar' | 'depends' | 'child'
    similarity_score REAL NOT NULL,   -- 0.0 ~ 1.0
    discovered_at INTEGER NOT NULL,
    
    PRIMARY KEY (source_task_id, target_task_id),
    FOREIGN KEY (source_task_id) REFERENCES tasks(id),
    FOREIGN KEY (target_task_id) REFERENCES tasks(id)
);

-- 3. Task 与 Session 关联
CREATE TABLE task_sessions (
    task_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    turn_range TEXT,  -- JSON: {"start": 0, "end": 5}
    
    PRIMARY KEY (task_id, session_id),
    FOREIGN KEY (task_id) REFERENCES tasks(id),
    FOREIGN KEY (session_id) REFERENCES agent_sessions(session_id)
);
```

### 3.2 Rust 实现结构

```rust
// ============================================
// 1. Vector Storage 核心模块
// cis-core/src/vector/mod.rs
// ============================================

use rusqlite::Connection;
use sqlite_vec::VectorIndex;

pub struct VectorStorage {
    conn: Connection,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl VectorStorage {
    /// 索引 Agent Session Turn
    pub async fn index_session_turn(&self, turn: &SessionTurn) -> Result<()> {
        let text = format!("{} {}", turn.request_prompt, turn.response_content);
        let embedding = self.embedding_service.embed(&text).await?;
        
        self.conn.execute(
            "INSERT INTO agent_session_turns_vec (turn_id, embedding) 
             VALUES (?1, ?2)
             ON CONFLICT(turn_id) DO UPDATE SET embedding = excluded.embedding",
            (&turn.turn_id, &embedding as &[f32]),
        )?;
        
        Ok(())
    }
    
    /// 语义搜索 Session Turns
    pub async fn search_session_turns(
        &self,
        query: &str,
        session_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SessionTurnSearchResult>> {
        let query_embedding = self.embedding_service.embed(query).await?;
        
        let sql = match session_filter {
            Some(session_id) => format!(
                "SELECT t.*, v.distance 
                 FROM agent_session_turns_vec vec
                 JOIN agent_session_turns t ON vec.turn_id = t.turn_id
                 JOIN vec_agent_session_turns_vec v ON vec.turn_id = v.turn_id
                 WHERE t.session_id = ?1
                 AND v.embedding MATCH ?2
                 AND k = ?3
                 ORDER BY v.distance
                 LIMIT ?3",
            ),
            None => format!(
                "SELECT t.*, v.distance 
                 FROM agent_session_turns_vec vec
                 JOIN agent_session_turns t ON vec.turn_id = t.turn_id
                 JOIN vec_agent_session_turns_vec v ON vec.turn_id = v.turn_id
                 WHERE v.embedding MATCH ?1
                 AND k = ?2
                 ORDER BY v.distance
                 LIMIT ?2",
            ),
        };
        
        // ... execute and map results
    }
    
    /// 索引 Task（多字段）
    pub async fn index_task(&self, task: &Task) -> Result<()> {
        // 标题向量
        if let Ok(emb) = self.embedding_service.embed(&task.title).await {
            self.conn.execute(
                "INSERT INTO task_title_vec (task_id, embedding) VALUES (?1, ?2)",
                (&task.id, &emb as &[f32]),
            )?;
        }
        
        // 描述向量
        if let Some(desc) = &task.description {
            if let Ok(emb) = self.embedding_service.embed(desc).await {
                self.conn.execute(
                    "INSERT INTO task_description_vec (task_id, embedding) VALUES (?1, ?2)",
                    (&task.id, &emb as &[f32]),
                )?;
            }
        }
        
        // 结果向量
        if let Some(result) = &task.result {
            if let Ok(emb) = self.embedding_service.embed(result).await {
                self.conn.execute(
                    "INSERT INTO task_result_vec (task_id, embedding) VALUES (?1, ?2)",
                    (&task.id, &emb as &[f32]),
                )?;
            }
        }
        
        Ok(())
    }
    
    /// 语义搜索 Task
    pub async fn search_tasks(
        &self,
        query: &str,
        search_in: TaskSearchField,
        limit: usize,
    ) -> Result<Vec<TaskSearchResult>> {
        let table = match search_in {
            TaskSearchField::Title => "task_title_vec",
            TaskSearchField::Description => "task_description_vec",
            TaskSearchField::Result => "task_result_vec",
            TaskSearchField::All => "(
                SELECT task_id, embedding, 'title' as field FROM task_title_vec
                UNION ALL
                SELECT task_id, embedding, 'description' as field FROM task_description_vec
                UNION ALL
                SELECT task_id, embedding, 'result' as field FROM task_result_vec
            )",
        };
        
        let embedding = self.embedding_service.embed(query).await?;
        
        // ... execute search
    }
    
    /// 发现相似 Task（自动关联）
    pub async fn discover_similar_tasks(
        &self,
        task_id: &str,
        threshold: f32,
    ) -> Result<Vec<SimilarTask>> {
        // 获取源 task 的向量
        let source_vec: Vec<f32> = self.conn.query_row(
            "SELECT embedding FROM task_title_vec WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;
        
        // 搜索相似
        let mut stmt = self.conn.prepare(
            "SELECT t.task_id, t.title, v.distance 
             FROM task_title_vec vec
             JOIN tasks t ON vec.task_id = t.id
             JOIN vec_task_title_vec v ON vec.task_id = v.task_id
             WHERE vec.task_id != ?1
             AND v.embedding MATCH ?2
             AND k = 10
             ORDER BY v.distance"
        )?;
        
        let results = stmt.query_map((task_id, &source_vec as &[f32]), |row| {
            Ok(SimilarTask {
                task_id: row.get(0)?,
                title: row.get(1)?,
                similarity: 1.0 - row.get::<_, f32>(2)?,
            })
        })?;
        
        // 过滤阈值并保存关系
        let similar: Vec<_> = results
            .filter_map(|r| r.ok())
            .filter(|t| t.similarity >= threshold)
            .collect();
        
        // 保存关系到数据库
        for t in &similar {
            self.conn.execute(
                "INSERT INTO task_relationships 
                 (source_task_id, target_task_id, relationship_type, similarity_score, discovered_at)
                 VALUES (?1, ?2, 'similar', ?3, ?4)
                 ON CONFLICT(source_task_id, target_task_id) DO UPDATE SET
                 similarity_score = excluded.similarity_score",
                (task_id, &t.task_id, t.similarity, chrono::Utc::now().timestamp()),
            )?;
        }
        
        Ok(similar)
    }
}

// ============================================
// 2. Session Recorder 模块
// cis-core/src/agent/recorder.rs
// ============================================

use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct SessionRecorder {
    storage: Arc<VectorStorage>,
    active_sessions: Arc<Mutex<HashMap<String, AgentSession>>>,
}

impl SessionRecorder {
    /// 开始新 Session
    pub async fn start_session(
        &self,
        project_id: Option<&str>,
        agent_type: &str,
        context: AgentContext,
    ) -> Result<String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        
        let session = AgentSession {
            session_id: session_id.clone(),
            project_id: project_id.map(|s| s.to_string()),
            agent_type: agent_type.to_string(),
            started_at: chrono::Utc::now(),
            status: SessionStatus::Active,
            context,
            turns: Vec::new(),
        };
        
        // 保存到数据库
        self.storage.save_session(&session).await?;
        
        // 加入活跃列表
        self.active_sessions.lock().await.insert(session_id.clone(), session);
        
        Ok(session_id)
    }
    
    /// 记录一次交互 Turn
    pub async fn record_turn(
        &self,
        session_id: &str,
        request: &AgentRequest,
        response: &AgentResponse,
        duration_ms: u64,
    ) -> Result<()> {
        let turn_id = format!("turn_{}", Uuid::new_v4().simple());
        
        let turn = SessionTurn {
            turn_id: turn_id.clone(),
            session_id: session_id.to_string(),
            turn_idx: self.get_next_turn_idx(session_id).await?,
            request_prompt: request.prompt.clone(),
            request_system_prompt: request.system_prompt.clone(),
            request_skills: request.skills.clone(),
            response_content: response.content.clone(),
            response_token_in: response.token_usage.as_ref().map(|t| t.prompt),
            response_token_out: response.token_usage.as_ref().map(|t| t.completion),
            started_at: chrono::Utc::now(),
            duration_ms,
            work_dir: request.context.work_dir.as_ref().map(|p| p.to_string_lossy().to_string()),
        };
        
        // 保存原始数据
        self.storage.save_session_turn(&turn).await?;
        
        // 索引向量（异步）
        let storage = self.storage.clone();
        tokio::spawn(async move {
            if let Err(e) = storage.index_session_turn(&turn).await {
                tracing::error!("Failed to index session turn: {}", e);
            }
        });
        
        Ok(())
    }
    
    /// 结束 Session
    pub async fn end_session(&self, session_id: &str) -> Result<()> {
        // 生成 Session 摘要
        let summary = self.generate_session_summary(session_id).await?;
        
        // 索引 Session 摘要向量
        let storage = self.storage.clone();
        tokio::spawn(async move {
            if let Err(e) = storage.index_session_summary(session_id, &summary).await {
                tracing::error!("Failed to index session summary: {}", e);
            }
        });
        
        // 更新状态
        self.storage.update_session_status(session_id, SessionStatus::Closed).await?;
        self.active_sessions.lock().await.remove(session_id);
        
        Ok(())
    }
    
    /// 关联 Task 与 Session
    pub async fn link_task_session(
        &self,
        task_id: &str,
        session_id: &str,
        turn_range: Option<(usize, usize)>,
    ) -> Result<()> {
        let range_json = turn_range.map(|(s, e)| {
            serde_json::json!({"start": s, "end": e}).to_string()
        });
        
        self.storage.conn().execute(
            "INSERT INTO task_sessions (task_id, session_id, turn_range) VALUES (?1, ?2, ?3)",
            (task_id, session_id, range_json),
        )?;
        
        Ok(())
    }
}

// ============================================
// 3. 增强的 Agent Provider
// cis-core/src/agent/providers/kimi.rs (修改后)
// ============================================

pub struct KimiProvider {
    config: AgentConfig,
    recorder: Option<Arc<SessionRecorder>>,
    current_session: Arc<Mutex<Option<String>>>,
}

impl KimiProvider {
    pub fn with_recorder(mut self, recorder: Arc<SessionRecorder>) -> Self {
        self.recorder = Some(recorder);
        self
    }
    
    pub async fn start_session(&self, context: AgentContext) -> Result<String> {
        if let Some(recorder) = &self.recorder {
            recorder.start_session(None, "kimi", context).await
        } else {
            Err(CisError::agent("Recorder not configured"))
        }
    }
}

#[async_trait]
impl AgentProvider for KimiProvider {
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let start = Instant::now();
        
        // 执行原始调用
        let response = self.execute_internal(&req).await?;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        // 记录到 Session
        if let Some(recorder) = &self.recorder {
            if let Some(session_id) = self.current_session.lock().await.as_ref() {
                recorder.record_turn(session_id, &req, &response, duration_ms).await?;
            }
        }
        
        Ok(response)
    }
}

// ============================================
// 4. RAG 检索服务
// cis-core/src/rag/mod.rs
// ============================================

pub struct RagService {
    vector_storage: Arc<VectorStorage>,
}

impl RagService {
    /// 为当前任务检索相关上下文
    pub async fn retrieve_task_context(
        &self,
        task_description: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<RagContext> {
        let mut context = RagContext::default();
        
        // 1. 检索相似的历史 Task
        let similar_tasks = self.vector_storage
            .search_tasks(task_description, TaskSearchField::All, limit / 3)
            .await?;
        
        for task in similar_tasks {
            context.add_task_reference(task);
        }
        
        // 2. 检索相关的 Agent Session Turns
        let relevant_turns = self.vector_storage
            .search_session_turns(task_description, None, limit / 3)
            .await?;
        
        for turn in relevant_turns {
            context.add_session_turn(turn);
        }
        
        // 3. 检索项目特定的 Session（如果有 project_id）
        if let Some(pid) = project_id {
            let project_turns = self.search_project_sessions(pid, task_description, limit / 3).await?;
            context.add_project_experience(project_turns);
        }
        
        Ok(context)
    }
    
    /// 构建 LLM 提示词上下文
    pub fn build_prompt(&self, user_query: &str, context: &RagContext) -> String {
        let mut prompt = String::new();
        
        // 添加相关历史任务
        if !context.related_tasks.is_empty() {
            prompt.push_str("## 相关历史任务\n\n");
            for task in &context.related_tasks {
                prompt.push_str(&format!("- {}: {}\n", task.id, task.title));
                if let Some(result) = &task.result_summary {
                    prompt.push_str(&format!("  结果: {}\n", result));
                }
            }
            prompt.push('\n');
        }
        
        // 添加相关会话经验
        if !context.relevant_sessions.is_empty() {
            prompt.push_str("## 相关经验参考\n\n");
            for turn in &context.relevant_sessions[..3.min(context.relevant_sessions.len())] {
                prompt.push_str(&format!("Q: {}\nA: {}\n\n", 
                    truncate(&turn.request, 200),
                    truncate(&turn.response, 300)
                ));
            }
        }
        
        // 用户当前查询
        prompt.push_str("## 当前任务\n\n");
        prompt.push_str(user_query);
        
        prompt
    }
}
```

### 3.3 CLI 集成

```rust
// cis-node/src/commands/agent.rs (新增)

/// 检索相关历史会话
pub async fn retrieve_context(prompt: &str, project: Option<&str>, limit: Option<usize>) -> Result<()> {
    let rag = RagService::new();
    
    let context = rag.retrieve_task_context(
        prompt,
        project,
        limit.unwrap_or(10)
    ).await?;
    
    println!("📚 检索到相关上下文:\n");
    
    // 显示相关任务
    if !context.related_tasks.is_empty() {
        println!("相似任务:");
        for task in &context.related_tasks {
            println!("  • {} (相似度: {:.1}%)", task.title, task.similarity * 100.0);
        }
        println!();
    }
    
    // 显示相关会话
    if !context.relevant_sessions.is_empty() {
        println!("相关经验:");
        for turn in &context.relevant_sessions[..5.min(context.relevant_sessions.len())] {
            println!("  Q: {}", truncate(&turn.request, 80));
            println!("  A: {}\n", truncate(&turn.response, 100));
        }
    }
    
    Ok(())
}

/// 语义搜索 Agent 历史
pub async fn search_history(query: &str, agent: Option<&str>, limit: Option<usize>) -> Result<()> {
    let storage = VectorStorage::open_default()?;
    
    let results = storage.search_session_turns(
        query,
        None,  // 不过滤 session
        limit.unwrap_or(10)
    ).await?;
    
    println!("🔍 搜索 '{}':\n", query);
    
    for (idx, result) in results.iter().enumerate() {
        println!("{}. [{}] 相似度: {:.1}%", 
            idx + 1,
            result.agent_type,
            (1.0 - result.distance) * 100.0
        );
        println!("   问: {}", truncate(&result.request, 100));
        println!("   答: {}\n", truncate(&result.response, 150));
    }
    
    Ok(())
}
```

新增 CLI 命令：

```bash
# 语义搜索 Agent 历史记录
 cis agent search "数据库优化" --agent kimi --limit 10
 
# 为当前任务检索上下文
 cis agent context "实现用户认证模块" --project myproject
 
# 查看 Session 详情
 cis agent session show <session_id>
 
# 列出活跃 Sessions
 cis agent session list --active
 
# 发现相似任务
 cis task similar <task_id> --threshold 0.8
```

---

## 四、集成价值总结

### 4.1 解决的问题

| 问题 | 解决方案 |
|------|----------|
| Agent 交互无记录 | SessionRecorder 完整记录每次交互 |
| 无法检索历史经验 | sqlite-vec 语义索引 + 相似度搜索 |
| Task 之间无关联 | 自动发现相似 Task 并建立关系 |
| 多轮对话无状态 | Session 级别的上下文累积 |
| 项目经验无法复用 | Project 维度的经验检索 |

### 4.2 使用场景示例

**场景 1: 新任务自动推荐相关经验**
```rust
// 创建新任务
let task = Task::new("优化数据库查询性能");

// 自动检索相关上下文
let context = rag.retrieve_task_context(
    &task.description,
    Some("myproject"),
    10
).await?;

// 发现相似历史任务
// → "PostgreSQL 索引优化" (相似度 92%)
// → "Redis 缓存策略调整" (相似度 78%)

// 检索相关 Kimi/Claude 会话
// → "之前是怎么解决慢查询的？"
```

**场景 2: Agent 自动加载相关上下文**
```rust
// 用户提问
let prompt = "这个错误怎么解决？connection pool exhausted";

// Agent 自动检索相关历史
let history = rag.search_session_turns(
    "connection pool error",
    None,
    5
).await?;

// 构建增强 prompt
let enhanced_prompt = format!("{}\n\n相关历史:\n{}", 
    prompt,
    format_history(&history)
);

// 调用 Agent
let response = agent.execute(enhanced_prompt).await?;
```

**场景 3: 任务完成后自动发现关联**
```rust
// Task 完成后
scheduler.on_task_complete(|task| async move {
    // 自动发现相似任务
    let similar = vector_storage
        .discover_similar_tasks(&task.id, 0.75)
        .await?;
    
    if !similar.is_empty() {
        println!("发现 {} 个相似历史任务:", similar.len());
        for t in similar {
            println!("  - {} (相似度 {:.1}%)", t.title, t.similarity * 100);
        }
    }
    
    Ok(())
}).await;
```

---

## 五、实施建议

### Phase 1: 基础向量存储 (1 周)
1. 添加 sqlite-vec 依赖
2. 创建 `VectorStorage` 核心模块
3. 实现 Task 向量索引

### Phase 2: Agent Session 记录 (1 周)
1. 创建 `SessionRecorder` 模块
2. 修改 Kimi/Claude Provider 集成记录
3. 实现 Session Turn 向量索引

### Phase 3: RAG 服务 (1 周)
1. 实现 `RagService` 检索逻辑
2. 集成到 ProjectSession
3. 添加 CLI 命令

### Phase 4: 高级功能 (1 周)
1. 自动 Task 关联发现
2. Session 摘要生成
3. 项目经验沉淀

---

## 六、技术依赖

```toml
[dependencies]
# SQLite 向量扩展
sqlite-vec = "0.1"

# 嵌入模型（选择其一）
# Option 1: OpenAI API
async-openai = "0.20"

# Option 2: 本地模型 (Ollama)
ollama-rs = "0.2"

# Option 3: HuggingFace (rust-bert)
rust-bert = "0.23"
```

---

**结论**: 通过 sqlite-vec 集成，CIS 将获得强大的语义记忆能力，实现 Agent 经验的持久化、可检索和可复用，这是构建真正"经验积累型" Agent 系统的关键一步。
