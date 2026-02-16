# Path-Based 记忆隔离设计 (防止跨项目/跨目录幻觉)

> **版本**: v1.1.7
> **创建日期**: 2026-02-14
> **最后更新**: 2026-02-15 (采用稳定哈希绑定机制)
> **核心问题**: 防止 AI 跨项目/跨目录幻觉
> **设计原则**: 目录哈希作为作用域 ID，解耦物理路径
> **三层架构**: 私域记忆 (目录哈希 + MemoryDomain::Private) + 公域记忆 (MemoryDomain::Public + P2P 同步) + AI 整理 (公域 → 私域)

> **说明**: CIS 原有的记忆处理模块已经实现了私域/公域区分 (使用 `MemoryDomain` 枚举)，详见 [CIS_MEMORY_DOMAIN_EXPLAINED.md](./CIS_MEMORY_DOMAIN_EXPLAINED.md)

> **🔥 v1.1.7 更新**: 目录哈希绑定作用域机制 [MEMORY_SCOPE_STABLE_HASH_DESIGN.md](./MEMORY_SCOPE_STABLE_HASH_DESIGN.md)

---

## 核心改进：目录哈希绑定 (v1.1.7)

### 问题：物理路径变动导致记忆失效

**原方案问题** (v1.1.6):
```rust
pub struct MemoryScope {
    pub path: PathBuf,  // 🔴 物理路径直接作为作用域
}
```

**场景**：
- 项目移动：`~/project-a` → `~/projects/project-a`
- 目录重命名：`my-project` → `my-project-v2`
- 不同机器：`/Users/alice/work` vs `/home/bob/work`

**结果**：🔴 **记忆失效**（新的 path = 新的作用域）

---

### 解决方案：稳定哈希绑定 (v1.1.7)

**设计思想** (详见 [MEMORY_SCOPE_STABLE_HASH_DESIGN.md](./MEMORY_SCOPE_STABLE_HASH_DESIGN.md)):
- ✅ **生成一次哈希**，永久绑定到项目
- ✅ **保存到配置文件** `.cis/project.toml`
- ✅ **移动/重命名后**：从配置文件读取（哈希不变）
- ✅ **支持自定义**：用户可指定自定义 scope_id

**核心实现**：
```rust
pub struct MemoryScope {
    /// 🔥 作用域 ID（目录哈希或用户自定义）
    ///
    /// # 稳定性保证
    ///
    /// - **第一次初始化**：生成哈希并保存到 `.cis/project.toml`
    /// - **移动/重命名后**：从配置文件读取（不会重新计算）
    /// - **用户自定义**：支持手动指定 scope_id
    pub scope_id: String,

    /// 人类可读名称（可选，用于调试和 UI）
    pub display_name: Option<String>,

    /// 物理路径（可选，仅用于默认值）
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// 记忆域（私域/公域）
    pub domain: MemoryDomain,
}
```

---

### 配置文件示例 (.cis/project.toml)

```toml
[memory]
# 方式 1: 自动生成（第一次初始化）
# cis project init 会自动生成并保存：
# scope_id = "a3f7e9c2b1d4f8a5"

# 方式 2: 用户自定义
scope_id = "my-workspace"

# 方式 3: 跨项目共享
# scope_id = "team-shared-alpha"
```

---

### 稳定性保证

| 场景 | 原方案（Path-Based） | 新方案（稳定哈希） |
|------|----------|----------|
| **第一次初始化** | 使用 path | ✅ 生成哈希并保存 |
| **移动项目** | 🔴 path 变化，记忆失效 | ✅ 哈希不变（从配置读取） |
| **重命名目录** | 🔴 path 变化，记忆失效 | ✅ 哈希不变（从配置读取） |
| **不同机器** | 🔴 path 不同，无法共享 | ✅ 哈希相同（配置文件同步） |

---

### 记忆键示例

**原方案** (v1.1.6):
```text
/home/user/repos/project-CIS::project/config
(冗长，path 变化后失效)
```

**新方案** (v1.1.7):
```text
a3f7e9c2b1d4f8a5::project/config
(简短，稳定，移动后不变)
```

---

---

## 原有 MemoryDomain 机制

CIS 的记忆处理模块 (`cis-core/src/storage/memory_db.rs`) 已经实现了私域和公域的区分：

### MemoryDomain 枚举 (cis-core/src/types.rs:313)

```rust
pub enum MemoryDomain {
    /// Private encrypted memory (私域加密记忆)
    Private,
    /// Public shared memory (公域共享记忆)
    Public,
}
```

### 数据库表结构 (cis-core/src/storage/memory_db.rs:78-99)

```sql
-- 私域记忆表 (加密存储，永不同步)
CREATE TABLE IF NOT EXISTS private_entries (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    category TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    encrypted INTEGER DEFAULT 1  -- 加密存储
);

-- 公域记忆表 (支持联邦同步)
CREATE TABLE IF NOT EXISTS public_entries (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    category TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    federate INTEGER DEFAULT 1,       -- 支持联邦同步
    sync_status TEXT DEFAULT 'pending'  -- 同步状态: pending/synced/failed
);
```

### 关键差异

**私域** (`MemoryDomain::Private`):
- 存储到 `private_entries` 表
- `encrypted=1` (加密存储)
- **永不同步** (不参与 P2P 联邦同步)
- 用途: 敏感信息 (API Keys, 个人偏好)

**公域** (`MemoryDomain::Public`):
- 存储到 `public_entries` 表
- `federate=1, sync_status='pending'`
- P2P 模块自动同步给其他节点
- 用途: 跨项目共享配置、团队最佳实践、跨节点共享知识

### 存储操作 (cis-core/src/storage/memory_db.rs:195)

```rust
pub fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
    match domain {
        MemoryDomain::Private => self.set_private(key, value, category),
        MemoryDomain::Public => self.set_public(key, value, category),
    }
}
```

---

## 三层记忆架构 (基于 MemoryDomain)

### 架构图

```
┌─────────────────────────────────────────────────────────┐
│ CIS 三层记忆模型 (Path-Based + P2P + AI 整理)         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  Layer 1: 私域记忆 (物理路径隔离)                    │
│  ├── ~/repos/project-a/... (当前项目)                  │
│  ├── ~/repos/project-b/... (其他项目)                  │
│  └── ~/agents/worker-1/... (Agent 私有)                │
│                                                         │
│  Layer 2: 公域记忆 (P2P 同步，~/CIS 作用域)          │
│  ├── ~/CIS/peers/node-1/... (节点 1 的共享记忆)      │
│  ├── ~/CIS/peers/node-2/... (节点 2 的共享记忆)      │
│  └── ~/CIS/team/team-dev/... (团队共享记忆)            │
│                                                         │
│  Layer 3: AI 整理记忆 (公域 → 私域迁移)              │
│  └── ~/repos/project-a/ai-curated/... (AI 整理后)     │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 层级关系

**Layer 1 → Layer 2 (P2P 发布)**
```rust
// 用户显式发布到 P2P
service.publish_to_cis(
    "project/architecture",
    PublishMode::Public,  // 公域记忆
).await?;
// 复制到: ~/CIS/shared/project/architecture
// P2P 同步给其他节点
```

**Layer 2 → Layer 1 (AI 整理)**
```rust
// AI 从公域记忆学习，整理到私域
service.curate_from_public(
    "project/architecture",
    CurateMode::Summarize,  // AI 总结
).await?;
// 从 ~/CIS/shared/project/architecture 读取
// AI 总结后写入: ~/repos/project-a/ai-curated/architecture
```

---

## 问题背景

### 三层记忆模型

**Layer 1: 私域记忆 (物理路径隔离)**
```rust
~/repos/project-a/project/config  → 项目 A 私有
~/repos/project-b/project/config  → 项目 B 私有 (物理隔离)
~/agents/worker-1/task/status     → Agent 私有
```

**Layer 2: 公域记忆 (P2P 同步, ~/CIS 作用域)**
```rust
~/CIS/shared/project/config    → 跨项目共享 (通过 P2P 同步)
~/CIS/peers/node-1/project/...  → 节点 1 的共享记忆
~/CIS/peers/node-2/project/...  → 节点 2 的共享记忆
```

**Layer 3: AI 整理 (公域 → 私域迁移)**
```rust
// AI 从公域记忆学习，整理到私域
~/repos/project-a/ai-curated/architecture  → AI 总结后写入
```

### AI 幻觉问题

**场景 1: 跨项目幻觉**
```
用户在项目 A 工作时:
项目 A: ~/repos/project-a/ (使用 Rust)
用户: "用什么语言开发？"
AI: "根据记忆，项目使用 Python"  ← ❌ 幻觉！这是项目 B 的记忆

原因: AI 搜索到 ~/repos/project-b/ 的记忆
      但当前在 project-a 工作
```

**场景 2: 跨目录幻觉**
```
用户在 src/ 目录:
src/database.rs: 使用 SQLite
AI: "根据记忆，这里用 PostgreSQL"  ← ❌ 幻觉！这是 tests/ 的记忆

原因: AI 搜索到 tests/ 的记忆 (测试用 PostgreSQL)
      但当前在 src/ 工作
```

**场景 3: Agent Teams 幻觉**
```
Agent A 在 ~/repos/project-a/ 执行任务
Agent B 在 ~/repos/project-b/ 执行任务
Agent A 读取到 Agent B 的记忆
→ ❌ 跨 Agent 幻觉
```

### 核心需求

**用户明确**:
> 直接用 path 很合理

**原因**:
1. **物理隔离 = 逻辑隔离**
   - 不同项目 = 不同路径
   - 不同目录 = 不同路径
   - 不同 Agent = 不同路径

2. **避免幻觉**
   - 当前路径决定了记忆范围
   - 不会"误用"其他路径的记忆

3. **简单直接**
   - 不需要抽象层 (Team/Group/User)
   - 不需要逻辑映射
   - Path 就是 Scope

---

## 设计方案

### 核心思想

**使用完整文件系统路径作为记忆作用域**:

```rust
/// 记忆作用域 = 文件系统路径
pub struct MemoryScope {
    pub path: PathBuf,  // 绝对路径
}

// 示例:
/home/user/repos/project-a/           → 项目 A 作用域
/home/user/repos/project-a/src/        → 项目 A 源码作用域
/home/user/repos/project-b/              → 项目 B 作用域 (完全独立)
/home/user/.cis/sessions/session-123/  → 会话作用域
```

### 路径层级继承

```
/home/user/repos/project-a/src/database.rs
│
├─ 当前作用域: /home/user/repos/project-a/src/
├─ 父级作用域:
│  └─ /home/user/repos/project-a/          (项目级)
│  └─ /home/user/repos/                   ( repos 级)
│  └─ /home/user/                        (用户级)
│  └─ /                                 (全局)
│
└─ 记忆查询: 从近到远 (当前 → 父级)
```

---

## 架构设计

### 1. MemoryScope 定义 (v1.1.7: 稳定哈希绑定）

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// 🔥 记忆作用域（稳定哈希绑定）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope {
    /// 作用域 ID（哈希或用户自定义）
    ///
    /// # 稳定性保证
    ///
    /// - 自动生成的哈希：**永久绑定到项目**（移动/重命名后不变）
    /// - 用户自定义 ID：**用户控制的稳定性**
    pub scope_id: String,

    /// 人类可读名称（可选，用于调试和 UI）
    pub display_name: Option<String>,

    /// 物理路径（可选，用于默认值）
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// 记忆域（私域/公域）
    pub domain: MemoryDomain,
}

impl MemoryScope {
    /// 🔥 从配置文件加载（核心方法）
    ///
    /// # 稳定性保证
    ///
    /// - **第一次初始化**：生成哈希并保存到配置文件
    /// - **后续加载**：从配置文件读取（不会重新计算）
    /// - **移动/重命名**：scope_id 不变（从配置文件读取）
    ///
    /// # 配置文件示例 (.cis/project.toml)
    ///
    /// ```toml
    /// [memory]
    /// # 第一次初始化后：
    /// scope_id = "a3f7e9c2b1d4f8a5"  # 自动生成并保存
    ///
    /// # 或用户自定义：
    /// # scope_id = "my-workspace"
    /// ```
    pub fn from_config(config: &ProjectConfig) -> Result<Self> {
        let scope_id = self::load_or_generate_scope_id(config)?;

        let display_name = config.memory.display_name.clone();
        let path = Some(config.project_root.clone());
        let domain = MemoryDomain::Private;

        Ok(Self {
            scope_id,
            display_name,
            path,
            domain,
        })
    }

    /// 🔥 自定义记忆域（不依赖 path）
    ///
    /// # 使用场景
    ///
    /// - 跨项目共享记忆（多个项目使用同一 scope_id）
    /// - 不想用自动生成的哈希
    /// - 需要人类可读的 scope_id
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 自定义作用域 ID
    /// let scope = MemoryScope::custom(
    ///     "my-shared-workspace",
    ///     Some("My Shared Workspace".into()),
    ///     MemoryDomain::Private
    /// );
    /// ```
    pub fn custom(
        scope_id: impl Into<String>,
        display_name: Option<impl Into<String>>,
        domain: MemoryDomain,
    ) -> Self {
        Self {
            scope_id: scope_id.into(),
            display_name: display_name.map(|n| n.into()),
            path: None,
            domain,
        }
    }

    /// 全局作用域
    pub fn global() -> Self {
        Self {
            scope_id: "global".to_string(),
            display_name: Some("Global".into()),
            path: None,
            domain: MemoryDomain::Private,
        }
    }

    /// 🔥 生成记忆键（scope_id + key）
    ///
    /// # 示例
    ///
    /// ```text
    /// scope_id: "a3f7e9c2b1d4f8a5"
    /// key: "project/config"
    /// → "a3f7e9c2b1d4f8a5::project/config"
    /// ```
    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.scope_id, key)
    }

    /// 🔥 判断是否为全局作用域
    pub fn is_global(&self) -> bool {
        self.scope_id == "global"
    }
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::global()
    }
}

impl Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.display_name {
            write!(f, "{} ({})", name, self.scope_id)
        } else {
            write!(f, "{}", self.scope_id)
        }
    }
}

/// 🔥 从配置加载或生成 scope_id
///
/// # 核心逻辑
///
/// 1. **配置文件中有 scope_id** → 直接使用（稳定绑定）
/// 2. **配置文件中没有 scope_id** → 生成哈希并保存（第一次初始化）
fn load_or_generate_scope_id(config: &ProjectConfig) -> Result<String> {
    match config.memory.scope_id.as_str() {
        // 配置文件中已有 → 直接使用
        id if !id.is_empty() && id != "auto" => {
            Ok(id.to_string())
        }

        // 配置文件中没有 → 生成并保存
        "" | "auto" => {
            // 1. 生成哈希
            let hash = MemoryScope::hash_path(&config.project_root);

            // 2. 保存到配置文件
            config.memory.scope_id = hash.clone();
            config.save()
                .map_err(|e| CisError::config(format!(
                    "Failed to save scope_id to config: {}", e
                )))?;

            Ok(hash)
        }

        // 不应该到达
        _ => unreachable!(),
    }
}

impl MemoryScope {
    /// 🔥 生成目录哈希（稳定且唯一）
    fn hash_path(path: &PathBuf) -> String {
        let mut hasher = DefaultHasher::new();

        // 规范化路径（去除 `..` 和 `.`）
        let canonical = path.canonicalize()
            .unwrap_or_else(|_| path.clone());

        // 哈希路径
        canonical.hash(&mut hasher);

        // 转为 16 进制字符串（16 字符）
        format!("{:016x}", hasher.finish())
    }
}
```

### 2. 全局记忆 API (粒度控制)

```rust
impl MemoryService {
    /// 默认存储 (使用当前目录作用域)
    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()> {
        self.set_with_scope(
            key,
            value,
            source,
            &self.current_scope,  // 当前目录
        ).await
    }

    /// 全局记忆 (扩展到 ~/)
    pub async fn set_global(
        &self,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()> {
        // 获取用户主目录
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))?;

        let global_scope = MemoryScope::new(format!("{}/", home));
        self.set_with_scope(key, value, source, &global_scope).await
    }

    /// 指定作用域存储
    pub async fn set_with_scope(
        &self,
        key: &str,
        value: &[u8],
        source: MemorySource,
        scope: &MemoryScope,
    ) -> Result<()> {
        // ... 实现同上 ...
    }
}
```

**粒度控制示例**:
```rust
// 场景 1: 当前目录 (默认)
// 当前在 ~/repos/project-a/
service.set("project/language", b"Rust", ...).await?;
// 存储: /home/user/repos/project-a/project/language
// 只有 project-a/ 能看到

// 场景 2: 全局记忆 (用户显式指定)
// 当前在 ~/repos/project-a/
service.set_global("editor/theme", b"dark", ...).await?;
// 存储: /home/user/editor/theme
// 所有目录都能看到 (通过父级继承)

// 场景 3: 跨项目共享 (提升到全局)
// 项目 A
service.set("api/key", b"key-12345", ...).await?;
// 存储: /home/user/repos/project-a/api/key

// 用户发现其他项目也需要用这个 key
service.promote_to_global("api/key").await?;
// 删除: /home/user/repos/project-a/api/key
// 复制: /home/user/api/key
// 现在所有项目都能看到
```

### 3. 数据库 Schema

-- ================================================================
-- 索引
-- ================================================================

-- 作用域前缀查询 (用于继承查询)
CREATE INDEX idx_memories_scope_prefix
    ON memories(scope_path, key)
    WHERE vector_indexed = 1;

-- 唯一键 (作用域 + 键)
CREATE UNIQUE INDEX idx_memories_unique
    ON memories(scope_path, key);

-- 污染防护 (作用域 + confidence)
CREATE INDEX idx_memories_scope_confidence
    ON memories(scope_path, confidence, source)
    WHERE vector_indexed = 1;

-- 访问统计
CREATE INDEX idx_memories_access_count
    ON memories(scope_path, access_count DESC);
```

### 3. 记忆服务 (路径感知)

```rust
/// Path-based 记忆服务
pub struct MemoryService {
    db: SqliteConnection,
    embedding: Arc<dyn EmbeddingService>,
    vector_storage: Arc<VectorStorage>,
    current_scope: MemoryScope,  // 当前作用域 (从工作目录自动检测)
}

impl MemoryService {
    /// 创建服务 (自动检测当前作用域)
    pub async fn new() -> Result<Self> {
        let current_scope = MemoryScope::from_current_dir()?;
        let db = SqliteConnection::open("~/.cis/memory.db")?;

        // 创建索引
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_scope_prefix
                ON memories(scope_path, key);",
        )?;

        Ok(Self {
            db,
            embedding: Arc::new(OpenAIEmbedding::new()?),
            vector_storage: Arc::new(HNSWStorage::new()?),
            current_scope,
        })
    }

    /// 存储记忆 (自动使用当前作用域)
    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        source: MemorySource,
    ) -> Result<()> {
        self.set_with_scope(
            key,
            value,
            source,
            &self.current_scope,
        ).await
    }

    /// 存储记忆 (指定作用域)
    pub async fn set_with_scope(
        &self,
        key: &str,
        value: &[u8],
        source: MemorySource,
        scope: &MemoryScope,
    ) -> Result<()> {
        let confidence = source.confidence();
        let now = chrono::Utc::now().timestamp();

        // 1. 存储到数据库
        self.db.execute(
            "INSERT INTO memories (scope_path, key, value, source, confidence, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(scope_path, key) DO UPDATE SET
             value = excluded.value,
             source = excluded.source,
             confidence = excluded.confidence,
             updated_at = excluded.updated_at",
            rusqlite::params![
                scope.as_str(),
                key,
                value,
                format!("{:?}", source),
                confidence,
                now,
            ],
        )?;

        // 2. 条件化向量索引 (防止污染)
        match source {
            MemorySource::UserForced | MemorySource::UserInput => {
                // ✅ 立即索引
                self.index_memory(scope, key, value).await?;
            }

            MemorySource::AIInferred => {
                // 🔴 不索引 (防止幻觉)
                tracing::debug!("Skipping vector index for AI-inferred memory");
            }

            MemorySource::AIConfirmed => {
                // ⚠️ 根据 confidence 决定
                if confidence >= 0.5 {
                    self.index_memory(scope, key, value).await?;
                }
            }

            _ => {
                // 其他 source: 不索引
            }
        }

        Ok(())
    }

    /// 读取记忆 (支持作用域继承)
    pub async fn get(
        &self,
        key: &str,
    ) -> Result<Option<MemoryEntry>> {
        self.get_with_scope(key, &self.current_scope).await
    }

    /// 读取记忆 (指定作用域,支持继承)
    pub async fn get_with_scope(
        &self,
        key: &str,
        query_scope: &MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        // 1. 当前作用域精确匹配
        if let Some(entry) = self.get_by_scope(key, query_scope).await? {
            return Ok(Some(entry));
        }

        // 2. 父级作用域继承 (从近到远)
        for parent_scope in query_scope.parents() {
            if let Some(entry) = self.get_by_scope(key, &parent_scope).await? {
                tracing::debug!(
                    "Found {} in parent scope {} (query scope {})",
                    key,
                    parent_scope.relative_to(&MemoryScope::global()).unwrap_or_else(|| parent_scope.path.clone()),
                    query_scope.relative_to(&MemoryScope::global()).unwrap_or_else(|| query_scope.path.clone())
                );
                return Ok(Some(entry));
            }
        }

        // 3. 未找到
        Ok(None)
    }

    /// 按作用域读取 (精确匹配)
    async fn get_by_scope(
        &self,
        key: &str,
        scope: &MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        let mut stmt = self.db.prepare(
            "SELECT key, value, source, confidence, created_at, updated_at
             FROM memories
             WHERE scope_path = ?1 AND key = ?2"
        )?;

        let result = stmt.query_row(
            rusqlite::params![scope.as_str(), key],
            |row| {
                Ok(MemoryEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    source: parse_source(&row.get::<_, String>(2)?),
                    confidence: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    scope: scope.clone(),
                })
            }
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get: {}", e))),
        }
    }

    /// 语义搜索 (限制在当前作用域及父级)
    pub async fn semantic_search(
        &self,
        query: &str,
        top_k: usize,
        min_confidence: Option<f32>,
    ) -> Result<Vec<MemoryEntry>> {
        self.semantic_search_with_scope(
            query,
            top_k,
            min_confidence,
            &self.current_scope,
        ).await
    }

    /// 语义搜索 (指定作用域,支持父级)
    pub async fn semantic_search_with_scope(
        &self,
        query: &str,
        top_k: usize,
        min_confidence: Option<f32>,
        search_scope: &MemoryScope,
    ) -> Result<Vec<MemoryEntry>> {
        // 1. 嵌入查询向量
        let query_vec = self.embedding.embed(query).await?;

        // 2. HNSW 搜索 (限制在作用域前缀)
        let mut results = self.vector_storage.search_by_scope_prefix(
            search_scope.as_str(),
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

### 4. 向量存储 (作用域前缀过滤)

```rust
impl VectorStorage {
    /// 按作用域前缀搜索 (防止跨作用域幻觉)
    pub async fn search_by_scope_prefix(
        &self,
        scope_prefix: &str,  // 例如: /home/user/repos/project-a/
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        // 1. HNSW 搜索获取候选
        let mut candidates = self.hnsw_search(query, top_k * 3).await?;

        // 2. 过滤: 只保留作用域前缀匹配的记忆
        candidates.retain(|r| {
            r.scope.starts_with(scope_prefix) ||
            scope_prefix.starts_with(&r.scope)  // 父级作用域也参与
        });

        // 3. 按相似度排序
        candidates.sort_by(|a, b| {
            b.similarity.partial_cmp(&a.similarity).unwrap()
        });

        candidates.truncate(top_k);
        Ok(candidates)
    }
}
```

---

## 完整使用示例

### 场景 0: 粒度控制 (当前目录 vs 全局)

```rust
async fn example_granularity_control() -> Result<()> {
    let service = MemoryService::new().await?;

    std::env::set_current_dir("~/repos/project-a/");
    service.current_scope = MemoryScope::from_current_dir()?;

    // ========== 默认：当前目录 (局部记忆) ==========
    service.set(
        "project/language",
        b"Rust",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储: /home/user/repos/project-a/project/language
    // ✅ 只有 project-a/ 能看到

    // ========== 全局记忆 (用户显式指定) ==========
    service.set_global(
        "editor/theme",
        b"dark",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储: /home/user/editor/theme
    // ✅ 所有目录都能看到 (通过父级继承)

    // ========== 查询: project-a/ ==========
    let entry = service.get("editor/theme").await?.unwrap();
    assert_eq!(entry.value, b"dark");
    // ✅ 能看到全局记忆 (继承 ~/)

    // ========== 查询: project-b/ ==========
    std::env::set_current_dir("~/repos/project-b/");
    service.current_scope = MemoryScope::from_current_dir()?;

    let entry = service.get("editor/theme").await?.unwrap();
    assert_eq!(entry.value, b"dark");
    // ✅ 也能看到全局记忆

    let entry = service.get("project/language").await?;
    assert_eq!(entry, None);
    // ✅ 看不到 project-a/ 的局部记忆 (防止跨项目幻觉)

    Ok(())
}
```

### 场景 1: 跨项目共享 (提升到全局)

```rust
async fn example_cross_project_sharing() -> Result<()> {
    let service = MemoryService::new().await?;

    // ========== 项目 A: 发现有用的配置 ==========
    std::env::set_current_dir("~/repos/project-a/");
    service.current_scope = MemoryScope::from_current_dir()?;

    service.set(
        "database/connection-pool",
        b"max_connections=100",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储: /home/user/repos/project-a/database/connection-pool

    // ========== 用户发现项目 B 也需要这个配置 ==========
    // 方案: 提升到全局
    service.promote_to_global("database/connection-pool").await?;

    // ✅ 删除: /home/user/repos/project-a/database/connection-pool
    // ✅ 复制: /home/user/database/connection-pool
    // ✅ 现在所有项目都能看到

    // ========== 项目 B: 查询全局记忆 ==========
    std::env::set_current_dir("~/repos/project-b/");
    service.current_scope = MemoryScope::from_current_dir()?;

    let entry = service.get("database/connection-pool").await?.unwrap();
    assert_eq!(entry.value, b"max_connections=100");
    // ✅ 能看到项目 A 提升的全局记忆

    // ========== 继承机制: 从近到远 ==========
    // 查询顺序:
    // 1. /home/user/repos/project-b/database/connection-pool (当前)
    // 2. /home/user/repos/database/connection-pool (全局) ← 找到
    // 3. /home/user/database/connection-pool
    // 4. /home/database/connection-pool
    // 5. /database/connection-pool

    Ok(())
}
```

### 场景 2: 跨项目隔离 (防止跨项目幻觉)

```rust
async fn example_cross_project_isolation() -> Result<()> {
    let service = MemoryService::new().await?;

    // ========== 项目 A: ~/repos/project-a/ ==========
    std::env::set_current_dir("~/repos/project-a/");
    let service_a = MemoryService::new().await?;
    service_a.current_scope = MemoryScope::from_current_dir()?;

    service_a.set(
        "project/language",
        b"Rust",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/repos/project-a/project/language

    // ========== 项目 B: ~/repos/project-b/ ==========
    std::env::set_current_dir("~/repos/project-b/");
    let service_b = MemoryService::new().await?;
    service_b.current_scope = MemoryScope::from_current_dir()?;

    service_b.set(
        "project/language",
        b"Python",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/repos/project-b/project/language

    // ========== 查询: 回到项目 A ==========
    std::env::set_current_dir("~/repos/project-a/");
    let service = MemoryService::new().await?;

    let entry = service.get("project/language").await?.unwrap();
    assert_eq!(entry.value, b"Rust");
    // ✅ 只返回项目 A 的记忆
    // 🔴 不会返回项目 B 的 Python (防止跨项目幻觉)

    // ========== 语义搜索: 项目 A ==========
    let results = service.semantic_search(
        "用什么语言开发",
        10,
        Some(0.8),
    ).await?;

    // ✅ 结果只包含项目 A 的记忆
    for result in results {
        assert!(result.scope.starts_with("/home/user/repos/project-a/"));
        // 🔴 不包含项目 B 的记忆
    }

    Ok(())
}
```

### 场景 2: 跨目录隔离 (防止跨目录幻觉)

```rust
async fn example_cross_directory_isolation() -> Result<()> {
    let service = MemoryService::new().await?;

    // ========== src/ 目录: ~/repos/project-a/src/ ==========
    std::env::set_current_dir("~/repos/project-a/src/");
    service.current_scope = MemoryScope::from_current_dir()?;

    service.set(
        "database/driver",
        b"SQLite",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/repos/project-a/src/database/driver

    // ========== tests/ 目录: ~/repos/project-a/tests/ ==========
    std::env::set_current_dir("~/repos/project-a/tests/");
    service.current_scope = MemoryScope::from_current_dir()?;

    service.set(
        "database/driver",
        b"PostgreSQL",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/repos/project-a/tests/database/driver

    // ========== 查询: 回到 src/ ==========
    std::env::set_current_dir("~/repos/project-a/src/");
    service.current_scope = MemoryScope::from_current_dir()?;

    let entry = service.get("database/driver").await?.unwrap();
    assert_eq!(entry.value, b"SQLite");
    // ✅ 只返回 src/ 的记忆
    // 🔴 不会返回 tests/ 的 PostgreSQL (防止跨目录幻觉)

    // ========== 语义搜索: src/ ==========
    let results = service.semantic_search(
        "数据库驱动",
        10,
        Some(0.8),
    ).await?;

    // ✅ 结果只包含 src/ 的记忆
    for result in results {
        assert!(result.scope.starts_with("/home/user/repos/project-a/src/"));
        // 🔴 不包含 tests/ 的记忆
    }

    Ok(())
}
```

### 场景 3: 作用域继承 (从近到远)

```rust
async fn example_scope_inheritance() -> Result<()> {
    let service = MemoryService::new().await?;

    std::env::set_current_dir("~/repos/project-a/src/module/");
    service.current_scope = MemoryScope::from_current_dir()?;

    // ========== 当前作用域: ~/repos/project-a/src/module/ ==========
    service.set(
        "log-level",
        b"ERROR",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/repos/project-a/src/module/log-level

    // ========== 父级作用域: ~/repos/project-a/src/ ==========
    service.set_with_scope(
        "log-level",
        b"WARN",
        MemorySource::UserForced,
        &MemoryScope::new("~/repos/project-a/src/"),
    ).await?;
    // ✅ 存储到: /home/user/repos/project-a/src/log-level

    // ========== 查询: ~/repos/project-a/src/module/ ==========
    let entry = service.get("log-level").await?.unwrap();
    assert_eq!(entry.value, b"ERROR");
    // ✅ 返回当前作用域的 ERROR (优先级最高)

    // ========== 删除当前作用域的记忆 ==========
    service.delete("log-level").await?;

    // ========== 再次查询 ==========
    let entry = service.get("log-level").await?.unwrap();
    assert_eq!(entry.value, b"WARN");
    // ✅ 继承父级作用域的 WARN

    Ok(())
}
```

### 场景 4: Agent Teams 隔离 (防止跨 Agent 幻觉)

```rust
async fn example_agent_teams_isolation() -> Result<()> {
    // ========== Agent A 工作目录: ~/agents/agent-a/ ==========
    std::env::set_current_dir("~/agents/agent-a/");
    let service_a = MemoryService::new().await?;

    service_a.set(
        "task/status",
        b"in_progress",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/agents/agent-a/task/status

    // ========== Agent B 工作目录: ~/agents/agent-b/ ==========
    std::env::set_current_dir("~/agents/agent-b/");
    let service_b = MemoryService::new().await?;

    service_b.set(
        "task/status",
        b"completed",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储到: /home/user/agents/agent-b/task/status

    // ========== Agent A 查询 ==========
    std::env::set_current_dir("~/agents/agent-a/");
    let service = MemoryService::new().await?;

    let entry = service.get("task/status").await?.unwrap();
    assert_eq!(entry.value, b"in_progress");
    // ✅ Agent A 只能看到自己的记忆
    // 🔴 不会看到 Agent B 的 completed (防止跨 Agent 幻觉)

    Ok(())
}
```

### 场景 5: AI 推断不污染 (防止 AI 幻觉)

```rust
async fn example_ai_inferred_isolation() -> Result<()> {
    let service = MemoryService::new().await?;

    std::env::set_current_dir("~/repos/project-a/");
    service.current_scope = MemoryScope::from_current_dir()?;

    // ========== 用户指定 ==========
    service.set(
        "project/architecture",
        b"Microservices",
        MemorySource::UserForced,  // confidence=1.0
    ).await?;
    // ✅ 立即建立向量索引

    // ========== AI 推断 ==========
    service.set(
        "project/architecture-guess",
        b"Maybe monolith",
        MemorySource::AIInferred,  // confidence=0.0
    ).await?;
    // 🔴 不建立向量索引 (不会参与搜索)

    // ========== 语义搜索 ==========
    let results = service.semantic_search(
        "项目架构",
        10,
        Some(0.5),  // min_confidence
    ).await?;

    // ✅ 结果包含 "Microservices" (UserForced)
    // 🔴 不包含 "Maybe monolith" (AIInferred, confidence=0.0)

    Ok(())
}
```

### 场景 6: P2P 同步 (Layer 1 → Layer 2)

```rust
async fn example_p2p_publishing() -> Result<()> {
    let service = MemoryService::new().await?;

    std::env::set_current_dir("~/repos/project-a/");
    service.current_scope = MemoryScope::from_current_dir()?;

    // ========== Layer 1: 私域记忆 ==========
    service.set(
        "project/architecture",
        b"Microservices with Rust",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储: /home/user/repos/project-a/project/architecture
    // ✅ 只有 project-a/ 能看到

    // ========== 用户显式发布到 P2P ==========
    service.publish_to_cis(
        "project/architecture",
        PublishMode::Public,  // 公域记忆
    ).await?;

    // ✅ 复制到: /home/user/.cis/shared/project/architecture
    // ✅ 标记为 P2P 共享 (federate = 1)
    // ✅ P2P 同步给其他节点

    // ========== 其他节点接收 (node-2) ==========
    // node-2 的 P2P 层接收到同步:
    // {
    //   "from": "node-1",
    //   "key": "project/architecture",
    //   "value": "Microservices with Rust",
    //   "scope": "/home/user/.cis/shared/project/"
    // }

    // node-2 自动存储到:
    // /home/user/.cis/peers/node-1/project/architecture
    // ✅ node-2 能看到 node-1 的共享记忆

    // ========== node-2 查询公域记忆 ==========
    std::env::set_current_dir("~/repos/project-b/");
    service.current_scope = MemoryScope::from_current_dir()?;

    let results = service.search_cis_shared(
        "project architecture",
        10,
    ).await?;

    // ✅ 能看到 node-1 的共享记忆
    // [
    //   {
    //     "key": "project/architecture",
    //     "value": "Microservices with Rust",
    //     "scope": "/home/user/.cis/peers/node-1/project/",
    //     "source": "node-1"  // 来源节点
    //   }
    // ]

    Ok(())
}
```

### 场景 7: AI 整理 (Layer 2 → Layer 1)

```rust
async fn example_ai_curated_learning() -> Result<()> {
    let service = MemoryService::new().await?;

    std::env::set_current_dir("~/repos/project-b/");
    service.current_scope = MemoryScope::from_current_dir()?;

    // ========== 从公域记忆学习 ==========
    let public_memories = service.search_cis_shared(
        "project architecture",
        5,
    ).await?;

    // ✅ 找到 node-1 的共享记忆
    // [
    //   {
    //     "key": "project/architecture",
    //     "value": "Microservices with Rust",
    //     "scope": "/home/user/.cis/peers/node-1/project/",
    //     "source": "node-1"
    //   }
    // ]

    // ========== AI 整理并总结 ==========
    for memory in public_memories {
        service.curate_from_public(
            &memory.key,
            CurateMode::Summarize,  // AI 总结模式
        ).await?;

        // AI 执行:
        // 1. 读取公域记忆
        // 2. 结合当前项目上下文
        // 3. 生成总结或建议
        // 4. 写入私域记忆 (当前项目)
    }

    // ✅ AI 整理后写入私域:
    // /home/user/repos/project-b/ai-curated/architecture-summary
    // value: "参考 node-1 的 Microservices 架构，但本项目使用单体架构..."

    // ========== 查询私域记忆 ==========
    let entry = service.get("architecture-summary").await?.unwrap();
    assert!(entry.scope.contains("/ai-curated/"));
    // ✅ AI 整理的记忆是私域的 (不会同步到其他节点)

    // ========== 公域 vs 私域 ==========
    let public = service.get_with_scope(
        "project/architecture",
        &MemoryScope::new("~/.cis/shared/"),
    ).await?;
    // ✅ 能看到公域记忆 (P2P 同步的)

    let private = service.get("architecture-summary").await?;
    // ✅ 能看到私域记忆 (AI 整理的)
    // 🔴 公域记忆 ≠ 私域记忆 (物理隔离)

    Ok(())
}
```

### 场景 8: 完整三层流程

```rust
async fn example_three_tier_flow() -> Result<()> {
    let service = MemoryService::new().await?;

    // ========== Layer 1: 私域记忆 ==========
    std::env::set_current_dir("~/repos/project-a/");
    service.current_scope = MemoryScope::from_current_dir()?;

    service.set(
        "project/best-practice",
        b"使用 Result<T> 处理错误",
        MemorySource::UserForced,
    ).await?;
    // ✅ 存储: /home/user/repos/project-a/project/best-practice
    // ✅ 只有 project-a/ 能看到

    // ========== Layer 1 → Layer 2: P2P 发布 ==========
    service.publish_to_cis(
        "project/best-practice",
        PublishMode::Public,
    ).await?;
    // ✅ 复制到: /home/user/.cis/shared/project/best-practice
    // ✅ P2P 同步给其他节点

    // ========== Layer 2: 其他节点接收 ==========
    // node-2 接收到同步:
    // /home/user/.cis/peers/node-1/project/best-practice

    // ========== Layer 2 → Layer 1: AI 整理 (node-2) ==========
    std::env::set_current_dir("~/repos/project-b/");
    service.current_scope = MemoryScope::from_current_dir()?;

    service.curate_from_public(
        "project/best-practice",
        CurateMode::Adopt,  // 采用模式 (直接采纳)
    ).await?;

    // ✅ AI 整理后写入私域:
    // /home/user/repos/project-b/project/best-practice
    // value: "使用 Result<T> 处理错误"
    // source: AIConfirmed (confidence=0.8)

    // ========== 结果 ==========
    // node-2 现在有私域记忆 (物理隔离)
    // ✅ /home/user/repos/project-b/project/best-practice
    // ✅ 不会同步回 node-1 (私域不共享)

    Ok(())
}
```

---

## 三层架构实现

### 1. P2P 发布 API

```rust
impl MemoryService {
    /// 发布到 P2P (Layer 1 → Layer 2)
    pub async fn publish_to_cis(
        &self,
        key: &str,
        mode: PublishMode,
    ) -> Result<()> {
        // 1. 读取私域记忆 (当前作用域)
        let entry = self.get(key).await?
            .ok_or_else(|| CisError::memory("Key not found"))?;

        // 2. 复制到 ~/CIS/ 作用域
        let cis_scope = MemoryScope::new("~/.cis/shared/");
        self.set_with_scope(
            key,
            &entry.value,
            entry.source,
            &cis_scope,
        ).await?;

        // 3. 标记为 P2P 共享
        self.db.execute(
            "UPDATE memories
             SET federate = 1, sync_status = 'pending'
             WHERE scope_path = ?1 AND key = ?2",
            rusqlite::params![cis_scope.as_str(), key],
        )?;

        // 4. 触发 P2P 同步
        self.p2p.sync_to_peers(key, &cis_scope).await?;

        Ok(())
    }
}

/// 发布模式
pub enum PublishMode {
    /// 公域记忆 (P2P 共享)
    Public,

    /// 团队记忆 (只同步给团队成员)
    Team { team_id: String },

    /// 私域记忆 (不同步)
    Private,
}
```

### 2. AI 整理 API

```rust
impl MemoryService {
    /// AI 整理 (Layer 2 → Layer 1)
    pub async fn curate_from_public(
        &self,
        key: &str,
        mode: CurateMode,
    ) -> Result<()> {
        // 1. 从 ~/CIS/ 作用域读取公域记忆
        let cis_scope = MemoryScope::new("~/.cis/shared/");
        let public_entry = self.get_with_scope(key, &cis_scope).await?
            .ok_or_else(|| CisError::memory("Public memory not found"))?;

        // 2. AI 处理 (根据模式)
        let (value, source) = match mode {
            CurateMode::Summarize => {
                // AI 总结公域记忆
                let summary = self.ai.summarize(&public_entry.value).await?;
                (summary, MemorySource::AIConfirmed)
            }

            CurateMode::Adopt => {
                // 直接采纳公域记忆
                (public_entry.value.clone(), MemorySource::AIConfirmed)
            }

            CurateMode::Adapt => {
                // AI 适应到当前项目
                let adapted = self.ai.adapt_to_current_project(
                    &public_entry.value,
                    &self.current_scope,
                ).await?;
                (adapted, MemorySource::AIConfirmed)
            }
        };

        // 3. 写入私域记忆 (当前作用域)
        self.set(
            key,
            &value,
            source,
        ).await?;

        Ok(())
    }
}

/// AI 整理模式
pub enum CurateMode {
    /// 总结模式 (AI 总结公域记忆)
    Summarize,

    /// 采用模式 (直接采纳公域记忆)
    Adopt,

    /// 适应模式 (AI 适应到当前项目)
    Adapt,
}
```

### 3. P2P 搜索 API

```rust
impl MemoryService {
    /// 搜索 ~/CIS/ 公域记忆
    pub async fn search_cis_shared(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemoryEntry>> {
        // 1. 生成查询向量
        let query_vec = self.embedding.embed(query).await?;

        // 2. HNSW 搜索 (限制在 ~/CIS/ 前缀)
        let cis_scope = MemoryScope::new("~/.cis/");
        let mut results = self.vector_storage.search_by_scope_prefix(
            cis_scope.as_str(),
            &query_vec,
            top_k,
        ).await?;

        // 3. 过滤只包含公域记忆
        results.retain(|r| {
            r.scope.starts_with("~/.cis/") ||
            r.scope.starts_with("/home/user/.cis/")
        });

        Ok(results)
    }
}
```

---

## 与旧设计对比

| 特性 | Team + Agent 方案 | Path-Based 方案 |
|------|------------------|-----------------|
| **物理隔离** | ✅ Team 隔离 | ✅ 路径隔离 |
| **逻辑隔离** | ⚠️ 需要 agent_id | ✅ 路径天然隔离 |
| **防止幻觉** | ⚠️ 需要复杂的作用域过滤 | ✅ 路径前缀直接过滤 |
| **简单性** | ❌ 抽象层复杂 | ✅ 直接使用文件系统 |
| **可理解性** | ❌ 需要理解 Team/Agent | ✅ 路径即作用域 |
| **数据库** | ⚠️ 需要 team_id + agent_id | ✅ 单一 scope_path |
| **索引** | ❌ 需要复合索引 | ✅ 单一前缀索引 |
| **调试** | ❌ 需要理解抽象层 | ✅ 直接看路径就知道 |

---

## 实现步骤

### Phase 1: 核心 Scope 定义 (P1.7.1)
- [ ] 定义 `MemoryScope` (基于 PathBuf)
- [ ] 实现 `from_current_dir()`
- [ ] 实现 `parents()` (作用域继承)
- [ ] 单元测试

### Phase 2: 数据库 Schema (P1.7.2)
- [ ] 创建 `memories` 表
- [ ] 添加 `scope_path` 索引
- [ ] 数据迁移脚本

### Phase 3: 记忆服务 (P1.7.3)
- [ ] 实现 `set_with_scope()`
- [ ] 实现 `get_with_scope()` (支持继承)
- [ ] 实现 `semantic_search_with_scope()`
- [ ] 集成测试

### Phase 4: 向量存储 (P1.7.4)
- [ ] 实现 `search_by_scope_prefix()`
- [ ] 作用域前缀过滤
- [ ] 性能测试

### Phase 5: 污染防护 (P1.7.5)
- [ ] 集成 MemorySource
- [ ] 条件化向量索引
- [ ] confidence 过滤
- [ ] 幻觉防护测试

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
**核心改进**: Path-Based 隔离,物理隔离 = 逻辑隔离,直接防止跨项目/跨目录幻觉
