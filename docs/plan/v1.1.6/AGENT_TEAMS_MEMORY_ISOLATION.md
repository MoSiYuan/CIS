# Agent Teams 记忆隔离设计

> **版本**: v1.1.7
> **创建日期**: 2026-02-14
> **核心问题**: Agent Teams 环境下的记忆共享与污染防护

---

## 问题本质

### 当前设计的根本矛盾

**用户明确指出**:
> 问题的本质一直是 agent teams 环境下，记忆共享和防止污染

### 核心需求

1. **Agent Teams 环境**
   - 多个 AI Agent 协同工作
   - Agent 可以属于不同的 Team
   - Agent 可以动态加入/离开 Team
   - Agent 需要长期运行和状态保持

2. **记忆共享**
   - 同一 Team 的 Agent 需要共享记忆
   - 不同 Team 的 Agent 需要隔离
   - 跨 Team 的项目需要特殊的共享机制

3. **防止污染**
   - AI 推断的记忆不能污染用户指定的记忆
   - 使用 MemorySource confidence 系统区分可信度
   - 向量搜索时优先高可信度记忆

### 当前设计的问题

**路径方案** (`/user-alice/team-dev/project-a/module-db`):
```
❌ 物理路径: /user-alice/team-dev/project-a ≠ /user-bob/team-dev/project-a
❌ 逻辑需求: Alice 和 Bob 在同一 team,应该共享记忆
❌ 结果: 需要复杂的 "逻辑共享层" 或 "路径映射"
```

**SharedMode 枚举**:
```rust
pub enum SharedMode {
    GroupShared,      // 承认了设计不能原生处理
    ProjectShared,     // 承认了设计不能原生处理
    Private,           // 承认了设计不能原生处理
}
```
这本身就证明了当前设计**无法同时满足**:
- 物理隔离 (不同的用户路径)
- 逻辑共享 (同一团队成员)

---

## Agent Teams 记忆架构设计

### 核心思想

**放弃 "User 维度"**,使用 **Team + Agent + MemoryKey** 三维命名空间:

```
当前方案 (User + Group + Path):
❌ /user-alice/team-dev/project-a
❌ /user-bob/team-dev/project-a
   → 物理隔离,逻辑需要共享

新方案 (Team + Agent + Key):
✅ /team-dev/agent-alice/project-a
✅ /team-dev/agent-bob/project-a
   → 物理共享,逻辑隔离
```

### 设计原则

1. **Team 是一级隔离单位**
   - 不同 Team 的记忆完全隔离
   - Team 是最小的共享单位

2. **Agent 是二级隔离单位**
   - 同一 Team 内,不同 Agent 的记忆互相隔离
   - 但可以显式共享到 Team 级别

3. **MemoryKey 是三级标识**
   - `team/agent/{agent-id}/{key}` → Agent 私有记忆
   - `team/shared/{key}` → Team 共享记忆
   - `team/project/{project-id}/{key}` → 项目共享记忆

---

## 架构设计

### 1. 命名空间结构

```rust
/// Agent Teams 记忆命名空间
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TeamMemoryScope {
    /// Team ID (一级隔离)
    pub team_id: String,

    /// Agent ID (可选,二级隔离)
    pub agent_id: Option<String>,

    /// 记忆键 (可选,三级标识)
    pub key: Option<String>,
}

impl TeamMemoryScope {
    /// Team 共享记忆 (所有 Agent 可见)
    pub fn team_shared(team_id: &str, key: &str) -> Self {
        Self {
            team_id: team_id.to_string(),
            agent_id: None,  // 无 Agent ID = Team 级别
            key: Some(key.to_string()),
        }
    }

    /// Agent 私有记忆 (只有该 Agent 可见)
    pub fn agent_private(team_id: &str, agent_id: &str, key: &str) -> Self {
        Self {
            team_id: team_id.to_string(),
            agent_id: Some(agent_id.to_string()),
            key: Some(key.to_string()),
        }
    }

    /// 项目共享记忆 (同一 Team 的所有 Agent 可见)
    pub fn project_shared(team_id: &str, project_id: &str, key: &str) -> Self {
        Self {
            team_id: team_id.to_string(),
            agent_id: None,  // 项目级别共享
            key: Some(format!("project/{}/{}", project_id, key)),
        }
    }

    /// 转换为路径字符串 (用于存储)
    pub fn as_path(&self) -> String {
        match (&self.agent_id, &self.key) {
            (None, None) => format!("/team-{}", self.team_id),
            (Some(agent), None) => format!("/team-{}/agent-{}", self.team_id, agent),
            (None, Some(key)) => format!("/team-{}/shared/{}", self.team_id, key),
            (Some(agent), Some(key)) => {
                format!("/team-{}/agent-{}/{}", self.team_id, agent, key)
            }
        }
    }

    /// 是否是 Team 共享记忆
    pub fn is_team_shared(&self) -> bool {
        self.agent_id.is_none()
    }

    /// 是否是 Agent 私有记忆
    pub fn is_agent_private(&self) -> bool {
        self.agent_id.is_some()
    }
}
```

### 2. 记忆存储结构

```rust
/// Team 记忆条目
#[derive(Debug, Clone)]
pub struct TeamMemoryEntry {
    /// 记忆键 (相对路径)
    pub key: String,

    /// 记忆值
    pub value: Vec<u8>,

    /// 记忆来源 (用于污染防护)
    pub source: MemorySource,

    /// 可信度 (0.0 - 1.0)
    pub confidence: f32,

    /// 所属 Team
    pub team_id: String,

    /// 所属 Agent (None = Team 共享)
    pub agent_id: Option<String>,

    /// 记忆域 (Private/Public)
    pub domain: MemoryDomain,

    /// 分类
    pub category: MemoryCategory,

    /// 时间戳
    pub created_at: i64,
    pub updated_at: i64,

    /// 向量索引 (是否已建立)
    pub vector_indexed: bool,

    /// 访问次数
    pub access_count: i64,
}
```

### 3. 共享与隔离机制

```rust
impl TeamMemoryService {
    /// Agent 存储私有记忆
    pub async fn agent_set(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()> {
        let scope = TeamMemoryScope::agent_private(team_id, agent_id, key);
        self.store_with_scope(value, source, scope).await
    }

    /// Agent 提升记忆到 Team 共享
    pub async fn promote_to_team(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
        reason: &str,  // 提升原因 (记录审计)
    ) -> Result<()> {
        // 1. 读取 Agent 私有记忆
        let agent_scope = TeamMemoryScope::agent_private(team_id, agent_id, key);
        let entry = self.get_by_scope(&agent_scope).await?
            .ok_or_else(|| CisError::memory("Agent memory not found"))?;

        // 2. 验证权限 (Agent 是否有权限提升到 Team)
        if !self.agent_can_promote(team_id, agent_id).await? {
            return Err(CisError::permission("Agent cannot promote to team"));
        }

        // 3. 提升到 Team 共享
        let team_scope = TeamMemoryScope::team_shared(team_id, key);
        self.store_with_scope(&entry.value, entry.source, team_scope).await?;

        // 4. 记录审计
        self.audit_promotion(team_id, agent_id, key, reason).await?;

        // 5. 删除 Agent 私有记忆
        self.delete_by_scope(&agent_scope).await?;

        Ok(())
    }

    /// Agent 读取记忆 (支持继承)
    pub async fn agent_get(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
    ) -> Result<Option<TeamMemoryEntry>> {
        // 1. 先尝试 Agent 私有记忆
        let agent_scope = TeamMemoryScope::agent_private(team_id, agent_id, key);
        if let Some(entry) = self.get_by_scope(&agent_scope).await? {
            return Ok(Some(entry));
        }

        // 2. 尝试 Team 共享记忆
        let team_scope = TeamMemoryScope::team_shared(team_id, key);
        if let Some(entry) = self.get_by_scope(&team_scope).await? {
            return Ok(Some(entry));
        }

        // 3. 未找到
        Ok(None)
    }
}
```

---

## 污染防护机制

### 1. MemorySource 集成

```rust
impl TeamMemoryService {
    /// Agent 存储记忆 (自动处理污染防护)
    pub async fn agent_set_with_source(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()> {
        let confidence = source.confidence();

        // 1. 存储到数据库
        let entry = TeamMemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            source: source.clone(),
            confidence,
            team_id: team_id.to_string(),
            agent_id: Some(agent_id.to_string()),
            domain: MemoryDomain::Private,  // Agent 私有
            category: MemoryCategory::Context,
            created_at: now(),
            updated_at: now(),
            vector_indexed: false,
            access_count: 0,
        };

        self.store_entry(entry).await?;

        // 2. 条件化向量索引 (防止污染)
        match source {
            MemorySource::UserForced => {
                // ✅ 立即索引
                self.index_memory(team_id, agent_id, key, value).await?;
            }

            MemorySource::UserInput => {
                // ✅ 立即索引
                self.index_memory(team_id, agent_id, key, value).await?;
            }

            MemorySource::AIInferred => {
                // 🔴 不索引 (防止污染)
                tracing::debug!("Skipping vector index for AI-inferred memory");
            }

            MemorySource::AIConfirmed => {
                // ⚠️ 根据 confidence 决定
                if confidence >= 0.5 {
                    self.index_memory(team_id, agent_id, key, value).await?;
                }
            }

            _ => {
                // 其他 source: 不索引
            }
        }

        Ok(())
    }
}
```

### 2. Team 级别的向量搜索

```rust
impl TeamMemoryService {
    /// Team 级别的向量搜索 (优先高可信度)
    pub async fn team_semantic_search(
        &self,
        team_id: &str,
        query: &str,
        top_k: usize,
        min_confidence: Option<f32>,
    ) -> Result<Vec<TeamMemoryEntry>> {
        // 1. 嵌入查询向量
        let query_vec = self.embedding.embed(query).await?;

        // 2. HNSW 搜索 (只搜索该 Team 的记忆)
        let mut results = self.vector_storage.search_by_team(
            team_id,
            &query_vec,
            top_k * 2,  // 获取更多候选
        ).await?;

        // 3. 过滤低可信度
        if let Some(min_conf) = min_confidence {
            results.retain(|r| r.confidence >= min_conf);
        }

        // 4. 排序: confidence * 0.7 + similarity * 0.3
        results.sort_by(|a, b| {
            let score_a = a.confidence * 0.7 + a.similarity * 0.3;
            let score_b = b.confidence * 0.7 + b.similarity * 0.3;
            score_b.partial_cmp(&score_a).unwrap()
        });

        // 5. 截断到 top_k
        results.truncate(top_k);

        Ok(results)
    }
}
```

---

## 数据库 Schema

```sql
-- ================================================================
-- Team Memory Schema (v1.1.7)
-- ================================================================

CREATE TABLE IF NOT EXISTS team_memories (
    -- 主键
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- 命名空间 (Team + Agent + Key)
    team_id TEXT NOT NULL,
    agent_id TEXT,                    -- NULL = Team 共享

    -- 记忆键和值
    key TEXT NOT NULL,
    value BLOB NOT NULL,

    -- 记忆来源 (污染防护)
    source TEXT NOT NULL,               -- 'UserForced', 'AIInferred', ...
    confidence REAL NOT NULL,            -- 0.0 - 1.0

    -- 记忆元数据
    domain TEXT NOT NULL,               -- 'Private', 'Public'
    category TEXT NOT NULL,             -- 'Execution', 'Result', ...

    -- 时间戳
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    -- 向量索引
    vector_indexed INTEGER DEFAULT 0,

    -- 访问统计
    access_count INTEGER DEFAULT 0,

    -- 审计
    promoted_by_agent TEXT,             -- 哪个 Agent 提升到 Team
    promoted_reason TEXT,               -- 提升原因
    promoted_at INTEGER                 -- 提升时间
);

-- Team 隔离索引 (一级)
CREATE INDEX idx_team_memories_team
    ON team_memories(team_id);

-- Agent 隔离索引 (二级)
CREATE INDEX idx_team_memories_agent
    ON team_memories(team_id, agent_id);

-- Team 共享记忆索引 (agent_id IS NULL)
CREATE INDEX idx_team_memories_team_shared
    ON team_memories(team_id)
    WHERE agent_id IS NULL;

-- 唯一键索引 (Team + Agent + Key)
CREATE UNIQUE INDEX idx_team_memories_unique
    ON team_memories(team_id, agent_id, key);

-- 污染防护索引 (source + confidence)
CREATE INDEX idx_team_memories_confidence
    ON team_memories(team_id, source, confidence);

-- 向量搜索索引
CREATE INDEX idx_team_memories_vector_indexed
    ON team_memories(team_id, vector_indexed)
    WHERE vector_indexed = 1;
```

---

## 完整场景示例

### 场景 1: Agent 私有记忆 → Team 共享

```rust
async fn example_agent_to_team_promotion() -> Result<()> {
    let service = TeamMemoryService::new().await?;

    // ========== 阶段 1: Agent Alice 发现有用信息 ==========
    service.agent_set_with_source(
        "team-dev",
        "agent-alice",
        "database/connection-pool",
        b"max_connections=100",
        MemorySource::UserInput,  // Alice 输入
    ).await?;

    // ✅ 存储到: /team-dev/agent-alice/database/connection-pool
    // ✅ 立即建立向量索引 (UserInput)
    // ✅ 只有 Alice 能看到

    // ========== 阶段 2: Alice 提升到 Team 共享 ==========
    service.promote_to_team(
        "team-dev",
        "agent-alice",
        "database/connection-pool",
        "This configuration works well for our workload",
    ).await?;

    // ✅ 移动到: /team-dev/shared/database/connection-pool
    // ✅ Team 中所有 Agent 都能看到
    // ✅ 记录审计: promoted_by_agent=agent-alice

    // ========== 阶段 3: Agent Bob 读取 ==========
    let entry = service.agent_get(
        "team-dev",
        "agent-bob",
        "database/connection-pool",
    ).await?.unwrap();

    // ✅ Bob 能看到 Alice 提升的记忆
    assert_eq!(entry.value, b"max_connections=100");

    Ok(())
}
```

### 场景 2: AI 推断不污染 Team 共享记忆

```rust
async fn example_ai_inferred_isolation() -> Result<()> {
    let service = TeamMemoryService::new().await?;

    // ========== 阶段 1: 用户指定记忆 ==========
    service.agent_set_with_source(
        "team-dev",
        "agent-alice",
        "project/architecture",
        b"Microservices with Rust",
        MemorySource::UserForced,  // 🔥 用户强制指定
    ).await?;

    // ✅ 存储到: /team-dev/agent-alice/project/architecture
    // ✅ confidence = 1.0
    // ✅ 立即建立向量索引

    // ========== 阶段 2: AI 推断 (不污染) ==========
    service.agent_set_with_source(
        "team-dev",
        "agent-alice",
        "project/architecture-guess",  // 不同 key
        b"Maybe monolith would be better",
        MemorySource::AIInferred,  // 🔴 AI 推断
    ).await?;

    // ✅ 存储到: /team-dev/agent-alice/project/architecture-guess
    // ✅ confidence = 0.0
    // 🔴 不建立向量索引 (不会污染搜索结果)

    // ========== 阶段 3: 向量搜索 (优先用户指定) ==========
    let results = service.team_semantic_search(
        "team-dev",
        "project architecture",
        10,
        Some(0.5),  // min_confidence
    ).await?;

    // ✅ 结果:
    // 1. "Microservices with Rust" (UserForced, confidence=1.0)
    // 🔴 不包含 "Maybe monolith would be better" (AIInferred, confidence=0.0)

    Ok(())
}
```

### 场景 3: Team 隔离 (不同 Team 完全独立)

```rust
async fn example_team_isolation() -> Result<()> {
    let service = TeamMemoryService::new().await?;

    // ========== Team A ==========
    service.agent_set_with_source(
        "team-dev",
        "agent-alice",
        "team/coding-standard",
        b"Follow Rust API guidelines",
        MemorySource::UserForced,
    ).await?;

    // ========== Team B ==========
    service.agent_set_with_source(
        "team-design",
        "agent-bob",
        "team/coding-standard",
        b"Use TypeScript with strict mode",
        MemorySource::UserForced,
    ).await?;

    // ========== 查询: Team A ==========
    let entry = service.agent_get(
        "team-dev",
        "agent-alice",
        "team/coding-standard",
    ).await?.unwrap();

    assert_eq!(entry.value, b"Follow Rust API guidelines");

    // ========== 查询: Team B ==========
    let entry = service.agent_get(
        "team-design",
        "agent-bob",
        "team/coding-standard",
    ).await?.unwrap();

    assert_eq!(entry.value, b"Use TypeScript with strict mode");

    // ✅ 完全隔离,互不影响

    Ok(())
}
```

### 场景 4: 跨 Team 项目 (需要特殊处理)

```rust
async fn example_cross_team_project() -> Result<()> {
    let service = TeamMemoryService::new().await?;

    // ========== Team A 创建项目 ==========
    service.agent_set_with_source(
        "team-dev",
        "agent-alice",
        "project-x/deadline",
        b"2026-03-01",
        MemorySource::UserForced,
    ).await?;

    // ✅ 存储到: /team-dev/agent-alice/project-x/deadline
    // ✅ 只有 team-dev 的 Agent 能看到

    // ========== Team B 需要访问同一项目 ==========
    // 方案 1: 创建跨 Team 共享项目
    service.create_cross_team_project(
        "project-x",
        vec!["team-dev", "team-design"],  // 参与的 Teams
        CrossTeamMode::ReadOnly,  // 其他 Team 只读
    ).await?;

    // 方案 2: Team B 复制记忆到自己的 Team
    service.agent_set_with_source(
        "team-design",
        "agent-bob",
        "project-x/deadline",
        b"2026-03-01",  // 从 team-dev 复制
        MemorySource::External {
            source: "team-dev/agent-alice".to_string(),
            confidence: 0.7,  // 外部来源,confidence 较低
        },
    ).await?;

    // ✅ 存储到: /team-design/agent-bob/project-x/deadline
    // ✅ confidence = 0.7 (低于 UserForced)

    Ok(())
}
```

---

## 数据库 Schema (完整版)

```sql
-- ================================================================
-- Team Memory Schema (Agent Teams 环境)
-- ================================================================

CREATE TABLE IF NOT EXISTS team_memories (
    -- 主键
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- 命名空间: Team + Agent + Key
    team_id TEXT NOT NULL,              -- Team ID (一级隔离)
    agent_id TEXT,                      -- Agent ID (二级隔离, NULL = Team 共享)

    -- 记忆键和值
    key TEXT NOT NULL,
    value BLOB NOT NULL,

    -- 记忆来源 (污染防护)
    source TEXT NOT NULL,                -- 'UserForced', 'AIInferred', ...
    confidence REAL NOT NULL DEFAULT 1.0, -- 0.0 - 1.0

    -- 记忆元数据
    domain TEXT NOT NULL DEFAULT 'Private',
    category TEXT NOT NULL DEFAULT 'Context',

    -- 时间戳
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    -- 向量索引
    vector_indexed INTEGER DEFAULT 0,

    -- 访问统计
    access_count INTEGER DEFAULT 0,

    -- 审计 (提升记录)
    promoted_by_agent TEXT,             -- 哪个 Agent 提升到 Team
    promoted_reason TEXT,                 -- 提升原因
    promoted_at INTEGER,                 -- 提升时间

    -- 跨 Team 项目
    cross_team_sharing INTEGER DEFAULT 0  -- 是否跨 Team 共享
);

-- ================================================================
-- 索引
-- ================================================================

-- Team 隔离 (一级)
CREATE INDEX idx_team_memories_team_id
    ON team_memories(team_id);

-- Team + Agent 隔离 (二级)
CREATE INDEX idx_team_memories_team_agent
    ON team_memories(team_id, agent_id);

-- Team 共享记忆 (agent_id IS NULL)
CREATE INDEX idx_team_memories_team_shared
    ON team_memories(team_id)
    WHERE agent_id IS NULL;

-- 唯一键 (Team + Agent + Key)
CREATE UNIQUE INDEX idx_team_memories_unique_key
    ON team_memories(team_id, agent_id, key);

-- 污染防护 (source + confidence)
CREATE INDEX idx_team_memories_source_confidence
    ON team_memories(team_id, source, confidence);

-- 向量搜索 (只搜索已索引的记忆)
CREATE INDEX idx_team_memories_vector_search
    ON team_memories(team_id, vector_indexed)
    WHERE vector_indexed = 1;

-- 跨 Team 项目
CREATE INDEX idx_team_memories_cross_team
    ON team_memories(cross_team_sharing)
    WHERE cross_team_sharing = 1;
```

---

## API 设计

### 核心 API

```rust
impl TeamMemoryService {
    // ========== Agent 级别操作 ==========

    /// Agent 存储记忆 (自动处理污染防护)
    pub async fn agent_set(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()>;

    /// Agent 读取记忆 (支持继承: Agent → Team)
    pub async fn agent_get(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
    ) -> Result<Option<TeamMemoryEntry>>;

    /// Agent 提升记忆到 Team 共享
    pub async fn promote_to_team(
        &self,
        team_id: &str,
        agent_id: &str,
        key: &str,
        reason: &str,
    ) -> Result<()>;

    // ========== Team 级别操作 ==========

    /// Team 级别的向量搜索 (优先高可信度)
    pub async fn team_semantic_search(
        &self,
        team_id: &str,
        query: &str,
        top_k: usize,
        min_confidence: Option<f32>,
    ) -> Result<Vec<TeamMemoryEntry>>;

    /// 获取 Team 共享记忆
    pub async fn team_get(
        &self,
        team_id: &str,
        key: &str,
    ) -> Result<Option<TeamMemoryEntry>>;

    /// 设置 Team 共享记忆
    pub async fn team_set(
        &self,
        team_id: &str,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()>;

    // ========== 跨 Team 操作 ==========

    /// 创建跨 Team 共享项目
    pub async fn create_cross_team_project(
        &self,
        project_id: &str,
        team_ids: Vec<String>,
        mode: CrossTeamMode,
    ) -> Result<()>;
}
```

---

## 与现有设计的对比

| 特性 | 旧设计 (User + Group + Path) | 新设计 (Team + Agent + Key) |
|------|------------------------------|------------------------------|
| **物理隔离** | ✅ 不同用户路径隔离 | ✅ 不同 Team 完全隔离 |
| **逻辑共享** | ❌ 需要复杂的 "逻辑共享层" | ✅ Team 共享直接支持 (agent_id=NULL) |
| **Agent 隔离** | ❌ 没有专门的设计 | ✅ Agent 级别隔离 |
| **污染防护** | ⚠️ 需要 Source + 复杂作用域 | ✅ Source + Team 语义清晰 |
| **跨 Team** | ❌ 需要路径映射 | ⚠️ 需要显式跨 Team 项目 |
| **向量搜索** | ❌ 需要复杂的作用域过滤 | ✅ Team 级别天然隔离 |
| **数据库 Schema** | ❌ 需要多个复合索引 | ✅ 简单的 Team + Agent + Key |
| **审计** | ❌ 没有设计 | ✅ promoted_by_agent/reason |

---

## 实现步骤

### Phase 1: Team Memory 核心 (P1.7.1)
- [ ] 定义 `TeamMemoryScope` 结构
- [ ] 定义 `TeamMemoryEntry` 结构
- [ ] 实现 `TeamMemoryService`
- [ ] 数据库 Schema 迁移
- [ ] 单元测试

### Phase 2: 污染防护 (P1.7.2)
- [ ] 集成 `MemorySource`
- [ ] 条件化向量索引
- [ ] Team 级别向量搜索 (confidence 过滤)
- [ ] 测试污染防护

### Phase 3: Agent 操作 (P1.7.3)
- [ ] `agent_set()` / `agent_get()`
- [ ] `promote_to_team()`
- [ ] 审计日志
- [ ] 权限验证

### Phase 4: 跨 Team 项目 (P1.7.4)
- [ ] 跨 Team 项目共享
- [ ] 复制记忆到外部 Team
- [ ] External Source 处理
- [ ] 集成测试

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
**核心改进**: 放弃 User 维度,使用 Team + Agent 二维命名空间,天然支持共享与隔离
