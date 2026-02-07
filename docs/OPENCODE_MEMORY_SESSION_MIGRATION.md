# OpenCode 接入 CIS 的记忆与会话管理改造方案

## 📋 文档概览

**目的**: 分析 CIS 与 OpenCode 在记忆和会话管理方面的差异，制定集成改造方案

**分析日期**: 2026-02-07

**CIS 版本**: main分支

**OpenCode 版本**: 1.1.53

---

## 🏗️ CIS 记忆与会话管理架构

### 1. 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS Application Layer                   │
│  (DAG执行、Agent Cluster、CLI 交互)                          │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                  Conversation Context                        │
│  cis-core/src/conversation/context.rs                       │
│  - 对话历史管理 (ContextMessage)                              │
│  - 会话摘要与话题                                            │
│  - RAG 增强 Prompt 构建                                      │
│  - 跨项目会话恢复                                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│              Vector Storage (向量存储)                       │
│  cis-core/src/vector/storage.rs                             │
│  - 记忆嵌入索引 (memory_embeddings)                          │
│  - 消息语义检索 (message_embeddings)                         │
│  - 对话摘要索引 (summary_embeddings)                         │
│  - HNSW 索引优化                                            │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│              Memory Storage (记忆存储)                       │
│  cis-core/src/memory/mod.rs                                 │
│  - 私域/公域记忆 (MemoryDomain)                              │
│  - 分类管理 (MemoryCategory)                                │
│  - 加密存储 (MemoryEncryption)                              │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│              Persistent Storage (持久化)                     │
│  cis-core/src/storage/                                      │
│  - conversation_db: 对话和消息                               │
│  - memory_db: 记忆数据                                      │
│  - vector.db: 向量索引                                      │
└─────────────────────────────────────────────────────────────┘
```

---

### 2. CIS 记忆管理机制

#### 2.1 记忆存储结构

**文件**: `cis-core/src/memory/mod.rs`

```rust
/// 记忆条目
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,      // 私域/公域
    pub category: MemoryCategory,  // Context/Result/Skill等
    pub created_at: i64,
    pub updated_at: i64,
}

/// 记忆域
pub enum MemoryDomain {
    Private,  // 加密存储，仅本节点访问
    Public,   // 明文存储，跨节点共享
}

/// 记忆分类
pub enum MemoryCategory {
    Context,      // 上下文记忆
    Result,       // 执行结果
    Skill,        // 技能相关
    Preference,   // 用户偏好
    Knowledge,    // 知识库
}
```

#### 2.2 向量存储集成

**文件**: `cis-core/src/vector/storage.rs`

```rust
pub struct VectorStorage {
    conn: Arc<Mutex<Connection>>,
    embedding: Arc<dyn EmbeddingService>,
    config: VectorConfig,
}

impl VectorStorage {
    /// 索引记忆（自动生成嵌入）
    pub async fn index_memory(
        &self,
        key: &str,
        value: &[u8],
        category: Option<&str>,
    ) -> Result<String> {
        // 1. 生成嵌入向量
        let text = String::from_utf8_lossy(value);
        let embedding = self.embedding.embed_text(&text).await?;

        // 2. 存储到 HNSW 索引
        // 3. 保存元数据
    }

    /// 语义搜索记忆
    pub async fn search_memory(
        &self,
        query: &str,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<MemoryResult>> {
        // 1. 查询嵌入
        let query_embedding = self.embedding.embed_text(query).await?;

        // 2. HNSW 最近邻搜索
        // 3. 返回相似度排序的结果
    }
}
```

---

### 3. CIS 会话管理机制

#### 3.1 ConversationContext

**文件**: `cis-core/src/conversation/context.rs`

```rust
pub struct ConversationContext {
    /// 对话ID
    pub conversation_id: String,
    /// 会话ID
    pub session_id: String,
    /// 对话标题
    pub title: Option<String>,
    /// 项目路径
    pub project_path: Option<PathBuf>,
    /// 对话摘要
    pub summary: Option<String>,
    /// 话题标签
    pub topics: Vec<String>,
    /// 消息历史
    pub messages: Vec<ContextMessage>,
    /// 最大历史消息数
    max_history: usize,
    /// 向量存储
    vector_storage: Option<Arc<VectorStorage>>,
}

pub struct ContextMessage {
    pub id: String,
    pub role: MessageRole,  // User/Assistant/System/Tool
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}
```

#### 3.2 核心功能

**1. 对话历史管理**

```rust
impl ConversationContext {
    /// 添加用户消息（带向量索引）
    pub async fn add_user_message_with_index(
        &mut self,
        content: impl Into<String>,
    ) -> Result<String> {
        let content = content.into();
        let id = Uuid::new_v4().to_string();

        // 向量索引
        if let Some(storage) = &self.vector_storage {
            storage.index_message(&conv_msg).await?;
        }

        self.add_message(msg);
        Ok(id)
    }

    /// 向量检索相关历史（RAG支持）
    pub async fn retrieve_relevant_history(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ContextMessage>> {
        if let Some(storage) = &self.vector_storage {
            let results = storage
                .search_messages(query, Some(&self.conversation_id), limit, Some(0.7))
                .await?;
            // 转换为 ContextMessage
        } else {
            // 回退到最近N条
            Ok(self.recent_messages(limit).to_vec())
        }
    }
}
```

**2. RAG 增强 Prompt 构建**

```rust
impl ConversationContext {
    /// 为 AI 准备增强 Prompt
    pub async fn prepare_ai_prompt(&self, user_input: &str) -> Result<String> {
        let mut context_parts = Vec::new();

        // 1. 项目上下文
        if let Some(project_path) = &self.project_path {
            context_parts.push(format!("## 当前项目\n{}", project_path.display()));
        }

        // 2. 对话摘要
        if let Some(summary) = &self.summary {
            context_parts.push(format!("## 对话摘要\n{}", summary));
        }

        // 3. 相关历史消息（语义检索）
        let relevant_history = self.retrieve_relevant_history(user_input, 5).await?;
        if !relevant_history.is_empty() {
            context_parts.push("## 相关历史对话".to_string());
            // ... 添加历史消息
        }

        // 4. 当前对话（最近3轮）
        let recent_dialog = self.recent_dialog(3);
        // ... 添加当前对话

        // 5. 组合最终 Prompt
        format!("{context}\n\n## 用户问题\n{input}")
    }
}
```

**3. 会话持久化**

```rust
impl ConversationContext {
    /// 保存并生成摘要
    pub async fn save_with_summary(&self, conversation_db: Arc<ConversationDb>) -> Result<()> {
        // 1. 生成摘要
        let summary = self.generate_summary_internal().await?;

        // 2. 提取话题
        let topics = self.extract_topics_internal().await?;

        // 3. 保存到 conversation_db
        conversation_db.save_conversation(&conv)?;

        // 4. 保存所有消息
        for msg in &self.messages {
            conversation_db.save_message(&db_msg)?;
        }

        // 5. 建立摘要向量索引
        if let Some(storage) = &self.vector_storage {
            storage.index_summary(&summary_id, &self.conversation_id, &summary, start_time, end_time).await?;
        }
    }
}
```

**4. 跨项目会话恢复**

```rust
pub struct SessionRecovery {
    conversation_db: Arc<ConversationDb>,
    vector_storage: Arc<VectorStorage>,
}

impl SessionRecovery {
    /// 搜索可恢复的历史会话
    pub fn find_recoverable_sessions(
        &self,
        session_id: &str,
        current_project: &str,
        limit: usize,
    ) -> Result<Vec<RecoverableSession>> {
        // 从不同项目的历史会话中查找
    }

    /// 恢复指定项目的上下文
    pub fn recover_context(&self, conversation_id: &str) -> Result<ConversationContext> {
        // 重建完整的 ConversationContext
    }
}
```

---

## 🔍 OpenCode 会话管理能力

### 1. 会话存储格式

**存储位置**: `~/.opencode/sessions/`

**存储格式**: JSON 文件

```json
{
  "id": "session-uuid",
  "created_at": "2026-02-07T10:30:00Z",
  "updated_at": "2026-02-07T11:00:00Z",
  "title": "Session about X",
  "messages": [
    {
      "id": "msg-uuid",
      "role": "user|assistant|system",
      "content": "...",
      "timestamp": "2026-02-07T10:31:00Z",
      "model": "anthropic/claude-3-opus-20240229"
    }
  ],
  "metadata": {
    "project_path": "/path/to/project",
    "model_used": "anthropic/claude-3-opus-20240229",
    "total_tokens": 12345
  }
}
```

### 2. 会话管理命令

**导出会话**:
```bash
opencode export [sessionID]
# 输出: JSON 格式会话数据
```

**导入会话**:
```bash
opencode import <file>
# 输入: JSON 格式会话数据
```

**列出会话**:
```bash
opencode session list
# 输出: 会话列表（ID、标题、时间）
```

### 3. 限制与差异

| 功能 | CIS | OpenCode |
|------|-----|----------|
| **向量检索** | ✅ HNSW 索引 | ❌ 无 |
| **会话摘要** | ✅ 自动生成 | ⚠️ 手动标题 |
| **跨项目恢复** | ✅ 语义搜索 | ❌ 无 |
| **记忆管理** | ✅ 私域/公域 | ❌ 无 |
| **话题提取** | ✅ 自动提取 | ❌ 无 |
| **RAG 增强** | ✅ 完整支持 | ❌ 无 |
| **持久化** | ✅ SQLite + 向量 | ✅ JSON 文件 |
| **导出/导入** | ✅ 自定义格式 | ✅ JSON |

---

## 📊 差异对比与影响分析

### 1. 架构差异

| 维度 | CIS | OpenCode | 兼容性 |
|------|-----|----------|--------|
| **数据存储** | 3层结构 (ConversationDB + VectorDB + MemoryDB) | 1层结构 (JSON文件) | ⚠️ 需要适配层 |
| **检索方式** | 语义向量检索 | 线性列表 | ⚠️ 性能差异大 |
| **上下文增强** | RAG 自动增强 | 手动管理 | ⚠️ 功能缺失 |
| **跨会话** | 语义搜索关联 | 无关联 | ❌ 完全缺失 |
| **项目管理** | 多项目切换 | 单项目 | ⚠️ 需扩展 |

### 2. 数据流差异

**CIS 数据流**:
```
用户输入
  ↓
ConversationContext.prepare_ai_prompt()
  ↓
向量检索相关历史 + 记忆 + 技能
  ↓
构建增强 Prompt
  ↓
发送给 AI Provider
```

**OpenCode 数据流**:
```
用户输入
  ↓
直接发送（或手动附加历史）
  ↓
OpenCode 内部处理
  ↓
返回响应
```

### 3. 关键差异点

#### 差异点 1: 缺少向量存储

**影响**:
- ❌ 无法语义检索历史消息
- ❌ 无法 RAG 增强 Prompt
- ❌ 性能下降（需加载全部历史）

**解决方案**:
- ✅ 保留 CIS VectorStorage
- ✅ 在 Agent Provider 层拦截消息
- ✅ 自动建立向量索引

#### 差异点 2: 会话格式不兼容

**影响**:
- ❌ OpenCode JSON 无法直接导入 CIS
- ❌ CIS ConversationContext 无法直接导出给 OpenCode

**解决方案**:
- ✅ 实现双向转换器
- ✅ `CIS ConversationContext ↔ OpenCode JSON`

#### 差异点 3: 记忆系统缺失

**影响**:
- ❌ OpenCode 无法使用 CIS 记忆
- ❌ 跨会话记忆共享失败

**解决方案**:
- ✅ 通过 Prompt 注入传递记忆
- ✅ 或实现 OpenCode Skill

---

## 🔧 改造方案

### 方案 A: 适配层模式 (推荐)

**目标**: 最小化 OpenCode 改动，在 CIS 层适配

#### 架构设计

```
┌──────────────────────────────────────────────────────────┐
│                    CIS Application                      │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│          OpenCode Adapter Layer (新增)                  │
│  - 拦截 OpenCode 输入/输出                                 │
│  - 维护 ConversationContext                               │
│  - 同步向量存储                                           │
│  - 注入 RAG 上下文                                        │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│              OpenCode Agent Provider                     │
│  - 标准 AgentProvider 接口                                │
│  - 调用 opencode run 命令                                 │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│                   OpenCode CLI                           │
└──────────────────────────────────────────────────────────┘
```

#### 实现细节

**1. 创建 OpenCodeAgentAdapter**

**文件**: `cis-core/src/agent/providers/opencode_adapter.rs`

```rust
//! OpenCode Agent 适配器
//!
//! 桥接 CIS 会话管理与 OpenCode CLI，维护 ConversationContext 同步

use crate::conversation::ConversationContext;
use crate::vector::VectorStorage;
use std::sync::Arc;

pub struct OpenCodeAgentAdapter {
    /// CIS 对话上下文
    context: Arc<RwLock<ConversationContext>>,
    /// 向量存储
    vector_storage: Arc<VectorStorage>,
    /// OpenCode 工作目录
    work_dir: PathBuf,
}

impl OpenCodeAgentAdapter {
    /// 创建新适配器
    pub fn new(
        work_dir: PathBuf,
        vector_storage: Arc<VectorStorage>,
    ) -> Self {
        let context = ConversationContext::with_vector_storage(
            Uuid::new_v4().to_string(),
            Uuid::new_v4().to_string(),
            Some(work_dir.clone()),
            vector_storage.clone(),
        );

        Self {
            context: Arc::new(RwLock::new(context)),
            vector_storage,
            work_dir,
        }
    }

    /// 增强 Prompt（RAG 注入）
    pub async fn prepare_prompt(&self, user_input: &str) -> Result<String> {
        let ctx = self.context.read().await;
        ctx.prepare_ai_prompt(user_input).await
    }

    /// 记录用户消息到 CIS
    pub async fn log_user_message(&self, content: &str) -> Result<()> {
        let mut ctx = self.context.write().await;
        ctx.add_user_message_with_index(content).await?;
        Ok(())
    }

    /// 记录助手响应到 CIS
    pub async fn log_assistant_message(&self, content: &str, metadata: Option<serde_json::Value>) -> Result<()> {
        let mut ctx = self.context.write().await;
        ctx.add_assistant_message_with_index(content, metadata).await?;
        Ok(())
    }

    /// 保存会话到持久化存储
    pub async fn save_session(&self) -> Result<()> {
        let ctx = self.context.read().await;
        let conversation_db = crate::storage::conversation_db::ConversationDb::open_default()?;
        ctx.save_with_summary(Arc::new(conversation_db)).await
    }

    /// 导出为 OpenCode JSON 格式
    pub async fn export_opencode_json(&self) -> Result<serde_json::Value> {
        let ctx = self.context.read().await;

        let messages: Vec<serde_json::Value> = ctx.messages
            .iter()
            .map(|msg| serde_json::json!({
                "id": msg.id,
                "role": msg.role.to_string(),
                "content": msg.content,
                "timestamp": msg.timestamp.to_rfc3339(),
            }))
            .collect();

        Ok(serde_json::json!({
            "id": ctx.conversation_id,
            "created_at": ctx.created_at.to_rfc3339(),
            "updated_at": ctx.last_updated.to_rfc3339(),
            "title": ctx.title,
            "messages": messages,
            "metadata": {
                "project_path": ctx.project_path,
            }
        }))
    }

    /// 从 OpenCode JSON 导入
    pub async fn import_opencode_json(&self, json: serde_json::Value) -> Result<()> {
        let mut ctx = self.context.write().await;

        // 解析 JSON 并恢复 ConversationContext
        if let Some(messages) = json.get("messages").and_then(|m| m.as_array()) {
            for msg_json in messages {
                let role = match msg_json.get("role").and_then(|r| r.as_str()) {
                    Some("user") => MessageRole::User,
                    Some("assistant") => MessageRole::Assistant,
                    Some("system") => MessageRole::System,
                    _ => continue,
                };

                let content = msg_json.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                let msg = ContextMessage {
                    id: msg_json.get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or(&Uuid::new_v4().to_string())
                        .to_string(),
                    role,
                    content,
                    timestamp: msg_json.get("timestamp")
                        .and_then(|t| t.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    metadata: None,
                };

                ctx.messages.push(msg);
            }
        }

        Ok(())
    }
}
```

**2. 更新 OpenCodeAgentProvider**

**文件**: `cis-core/src/agent/providers/opencode.rs`

```rust
use crate::agent::providers::opencode_adapter::OpenCodeAgentAdapter;

pub struct OpenCodeAgentProvider {
    config: AgentConfig,
    /// 适配器
    adapter: Option<OpenCodeAgentAdapter>,
}

impl OpenCodeAgentProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config, adapter: None }
    }

    /// 初始化适配器
    pub async fn init_adapter(&mut self, work_dir: PathBuf) -> Result<()> {
        let vector_storage = VectorStorage::open_default()?;
        let adapter = OpenCodeAgentAdapter::new(work_dir, Arc::new(vector_storage));
        self.adapter = Some(adapter);
        Ok(())
    }
}

#[async_trait]
impl AgentProvider for OpenCodeAgentProvider {
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        // 1. 增强 Prompt
        let adapter = self.adapter.as_ref()
            .ok_or_else(|| CisError::configuration("Adapter not initialized"))?;

        let enhanced_prompt = adapter.prepare_prompt(&req.prompt).await?;

        // 2. 记录用户消息
        adapter.log_user_message(&req.prompt).await?;

        // 3. 调用 OpenCode
        let output = tokio::process::Command::new("opencode")
            .arg("run")
            .arg("--format").arg("json")
            .arg("--")
            .arg(&enhanced_prompt)
            .current_dir(req.context.work_dir.as_ref().unwrap())
            .output()
            .await?;

        let content = String::from_utf8_lossy(&output.stdout).to_string();

        // 4. 记录助手响应
        adapter.log_assistant_message(&content, None).await?;

        // 5. 保存会话
        adapter.save_session().await?;

        Ok(AgentResponse {
            content,
            token_usage: None,
            metadata: HashMap::new(),
        })
    }
}
```

**3. Agent Cluster 集成**

**文件**: `cis-core/src/agent/cluster/executor.rs`

```rust
async fn start_task_by_id(
    &self,
    run_id: &str,
    task_id: &str,
    command: &str,
    upstream_deps: &[String],
) -> Result<()> {
    // ... 准备工作目录、上下文 ...

    // 创建 OpenCode 适配器
    let mut provider = providers::OpenCodeAgentProvider::new(config);
    provider.init_adapter(work_dir.clone()).await?;

    // 创建 session（会自动维护 ConversationContext）
    let session_id = self.session_manager.create_session_with_adapter(
        run_id,
        task_id,
        agent_type,
        &full_prompt,
        &work_dir,
        &upstream_context,
        Some(Arc::new(provider)), // 传递带适配器的 provider
    ).await?;

    // ...
}
```

---

### 方案 B: 双向同步模式

**目标**: OpenCode 与 CIS 各自维护会话，定期同步

#### 实现细节

**1. 定期同步任务**

```rust
pub struct OpenCodeSyncTask {
    opencode_session_dir: PathBuf,
    conversation_db: Arc<ConversationDb>,
    vector_storage: Arc<VectorStorage>,
}

impl OpenCodeSyncTask {
    /// 扫描 OpenCode 会话目录
    pub async fn scan_sessions(&self) -> Result<Vec<PathBuf>> {
        let mut sessions = Vec::new();
        let dir = self.opencode_session_dir.join("sessions");

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                sessions.push(entry.path());
            }
        }

        Ok(sessions)
    }

    /// 同步 OpenCode 会话到 CIS
    pub async fn sync_session(&self, path: &Path) -> Result<()> {
        // 1. 读取 OpenCode JSON
        let json_content = tokio::fs::read_to_string(path).await?;
        let json: serde_json::Value = serde_json::from_str(&json_content)?;

        // 2. 转换为 ConversationContext
        let ctx = self.opencode_to_context(json)?;

        // 3. 保存到 CIS
        ctx.save_with_summary(self.conversation_db.clone()).await?;

        Ok(())
    }

    /// 转换 OpenCode JSON 到 ConversationContext
    fn opencode_to_context(&self, json: serde_json::Value) -> Result<ConversationContext> {
        // ... 转换逻辑
    }
}
```

**2. 定时任务**

```rust
/// 每5分钟同步一次
pub async fn start_sync_task() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));

        loop {
            interval.tick().await;

            let sync_task = OpenCodeSyncTask::new();
            if let Ok(sessions) = sync_task.scan_sessions().await {
                for session in sessions {
                    let _ = sync_task.sync_session(&session).await;
                }
            }
        }
    });
}
```

---

### 方案 C: OpenCode Skill 模式

**目标**: 将 CIS 记忆系统封装为 OpenCode Skill

#### 实现细节

**1. 创建 OpenCode Skill**

**文件**: `skills/cis-memory-skill/src/lib.rs`

```toml
[skill]
name = "cis-memory"
version = "0.1.0"
description = "CIS Memory Management for OpenCode"

[[skill.capabilities]]
name = "save_memory"
description = "Save memory to CIS"

[[skill.capabilities]]
name = "search_memory"
description = "Search memory with semantic query"
```

```rust
//! CIS Memory Skill for OpenCode

use opencode_sdk::skill::{Skill, SkillContext};

pub struct CisMemorySkill {
    vector_storage: Arc<VectorStorage>,
}

impl Skill for CisMemorySkill {
    fn name(&self) -> &str {
        "cis-memory"
    }

    async fn execute(&self, ctx: &SkillContext, input: &str) -> Result<String> {
        match ctx.command {
            "save_memory" => {
                // 解析输入并保存到 CIS
                self.save_memory(input).await
            }
            "search_memory" => {
                // 语义搜索
                self.search_memory(input).await
            }
            _ => Ok("Unknown command".to_string())
        }
    }
}
```

**2. 在 OpenCode 中使用**

```bash
# 在 OpenCode 中调用
/cis-memory save "用户偏好: 暗色主题"
/cis-memory search "主题设置"
```

---

## 🎯 推荐实施方案

### 阶段 1: 基础适配 (2-3天)

**目标**: 最小可用，OpenCode 可执行并同步到 CIS

1. **创建 OpenCodeAgentAdapter**
   - 实现 `prepare_prompt()` - RAG 增强
   - 实现 `log_user_message()` - 记录用户消息
   - 实现 `log_assistant_message()` - 记录助手响应

2. **更新 OpenCodeAgentProvider**
   - 集成适配器
   - 拦截输入/输出
   - 自动同步到 ConversationContext

3. **测试验证**
   - DAG 执行测试
   - 会话同步测试
   - 向量检索测试

### 阶段 2: 双向同步 (3-4天)

**目标**: OpenCode 会话可导入 CIS

1. **实现格式转换**
   - OpenCode JSON → ConversationContext
   - ConversationContext → OpenCode JSON

2. **创建同步任务**
   - 扫描 OpenCode 会话目录
   - 自动导入 CIS

3. **CLI 命令**
   - `cis memory import-opencode <path>`
   - `cis memory export-opencode <session-id>`

### 阶段 3: Skill 模式 (可选, 2-3天)

**目标**: OpenCode 可直接访问 CIS 记忆

1. **创建 cis-memory Skill**
   - 暴露记忆存储 API
   - 暴露向量检索 API

2. **OpenCode 集成**
   - 配置 Skill 路径
   - 测试 Skill 调用

---

## 📊 改造影响评估

### 需要修改的文件

| 文件 | 改动类型 | 复杂度 | 说明 |
|------|----------|--------|------|
| `cis-core/src/agent/providers/opencode_adapter.rs` | 新增 | ⭐⭐⭐ | 核心适配器 |
| `cis-core/src/agent/providers/opencode.rs` | 修改 | ⭐⭐ | 集成适配器 |
| `cis-core/src/agent/cluster/executor.rs` | 修改 | ⭐⭐ | 传递适配器 |
| `cis-node/src/commands/memory.rs` | 修改 | ⭐ | 添加导入/导出命令 |
| `skills/cis-memory-skill/` | 新增 | ⭐⭐ | OpenCode Skill (可选) |

### 兼容性风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **OpenCode 格式变化** | 中 | 低 | 版本检测 + 适配层 |
| **性能下降** | 低 | 低 | 异步索引 + 批处理 |
| **数据不一致** | 高 | 中 | 定期校验 + 事务保护 |
| **同步冲突** | 中 | 中 | 时间戳 + 冲突解决策略 |

---

## 🔄 数据流示意图（改造后）

```
用户输入: "优化查询性能"
    ↓
┌─────────────────────────────────────────────────────────┐
│  OpenCodeAgentAdapter                                   │
│  1. prepare_prompt()                                    │
│     - 向量检索相关历史                                   │
│     - 搜索相关记忆                                       │
│     - 构建 RAG 增强 Prompt                              │
│     → "## 相关历史\n用户: 如何优化数据库？\n..."          │
│        + "\n## 用户问题\n优化查询性能"                   │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  OpenCodeAgentProvider                                  │
│  2. log_user_message() → 索引到 VectorStorage          │
│  3. 调用 opencode run --format json                    │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  OpenCode CLI                                           │
│  4. 返回 JSON 响应                                       │
└─────────────────────────────────────────────────────────┘
    ↓
┌─────────────────────────────────────────────────────────┐
│  OpenCodeAgentProvider                                  │
│  5. log_assistant_message() → 索引到 VectorStorage      │
│  6. save_session() → 保存到 ConversationDB             │
└─────────────────────────────────────────────────────────┘
    ↓
返回给用户
```

---

## 📚 参考文档

- **CIS 记忆系统**: `cis-core/src/memory/mod.rs`
- **CIS 会话管理**: `cis-core/src/conversation/context.rs`
- **CIS 向量存储**: `cis-core/src/vector/storage.rs`
- **OpenCode 文档**: https://github.com/anomalyco/opencode
- **OpenCode 会话管理**: https://qixinbo.github.io/2026/01/18/opencode-3/

---

## 🔄 版本历史

| 版本 | 日期 | 作者 | 说明 |
|------|------|------|------|
| 1.0 | 2026-02-07 | Claude | 初始版本 |

---

**文档结束**
