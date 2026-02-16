# CIS 原有私域/公域机制说明

> **版本**: v1.1.7
> **创建日期**: 2026-02-14
> **关联**: [PATH_BASED_MEMORY_ISOLATION.md](./PATH_BASED_MEMORY_ISOLATION.md)

---

## MemoryDomain 枚举

CIS 的记忆处理模块已经实现了私域和公域的区分：

**定义** (cis-core/src/types.rs:313):
```rust
pub enum MemoryDomain {
    /// Private encrypted memory (私域加密记忆)
    Private,
    /// Public shared memory (公域共享记忆)
    Public,
}
```

---

## 数据库表结构

**定义** (cis-core/src/storage/memory_db.rs:78-99):

```sql
-- ================================================================
-- 私域记忆表（加密存储，不同步）
-- ================================================================
CREATE TABLE IF NOT EXISTS private_entries (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    category TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    encrypted INTEGER DEFAULT 1  -- 加密存储
);

-- ================================================================
-- 公域记忆表（支持联邦同步）
-- ================================================================
CREATE TABLE IF NOT EXISTS public_entries (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL,
    category TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    federate INTEGER DEFAULT 1,       -- 支持联邦同步
    sync_status TEXT DEFAULT 'pending'  -- 同步状态: pending/synced/failed
);

-- ================================================================
-- 记忆索引表（跨表查询）
-- ================================================================
CREATE TABLE IF NOT EXISTS memory_index (
    key TEXT PRIMARY KEY,
    domain TEXT,  -- 'private' or 'public'
    category TEXT,
    skill_name TEXT,
    created_at INTEGER
);
```

---

## 关键差异

### 私域记忆 (MemoryDomain::Private)

- **存储表**: `private_entries`
- **加密**: `encrypted=1` (加密存储)
- **同步**: **永不同步** (不参与 P2P 联邦同步)
- **用途**:
  - 敏感信息 (API Keys, 个人偏好)
  - 项目私有配置
  - Agent 私有状态
  - 临时会话数据

### 公域记忆 (MemoryDomain::Public)

- **存储表**: `public_entries`
- **同步**: `federate=1, sync_status='pending'`
- **P2P 模块**: 自动同步给其他节点
- **用途**:
  - 跨项目共享配置
  - 团队共享最佳实践
  - 跨节点共享知识
  - 需要联邦同步的数据

---

## 存储操作

**set() 函数** (cis-core/src/storage/memory_db.rs:195):
```rust
pub fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
    match domain {
        MemoryDomain::Private => self.set_private(key, value, category),
        MemoryDomain::Public => self.set_public(key, value, category),
    }
}
```

**set_private() 实现** (cis-core/src/storage/memory_db.rs:152):
```rust
pub fn set_private(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let category_str = format!("{:?}", category);

    self.conn.execute(
        "INSERT INTO private_entries (key, value, category, created_at, updated_at, encrypted)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)
         ON CONFLICT(key) DO UPDATE SET
         value = excluded.value,
         category = excluded.category,
         updated_at = excluded.updated_at",
        rusqlite::params![key, value, category_str, now, now],
    )?;
}
```

**set_public() 实现** (cis-core/src/storage/memory_db.rs:173):
```rust
pub fn set_public(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let category_str = format!("{:?}", category);

    self.conn.execute(
        "INSERT INTO public_entries (key, value, category, created_at, updated_at, federate, sync_status)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, 'pending')
         ON CONFLICT(key) DO UPDATE SET
         value = excluded.value,
         category = excluded.category,
         updated_at = excluded.updated_at,
         federate = excluded.federate,
         sync_status = excluded.sync_status",
        rusqlite::params![key, value, category_str, now, now],
    )?;
}
```

---

## P2P 同步机制

### sync_status 字段

公域记忆表有 `sync_status` 字段，用于跟踪同步状态：

| 状态 | 说明 |
|------|------|
| `pending` | 等待同步 (默认状态) |
| `syncing` | 正在同步 |
| `synced` | 已同步 |
| `failed` | 同步失败 (需要重试) |

### get_sync_status() 函数

**定义** (cis-core/src/memory/ops/sync.rs:161):
```rust
pub async fn get_sync_status(&self, key: &str) -> Result<Option<DateTime<Utc>>> {
    let full_key = self.state.full_key(key);
    let db = self.state.memory_db.lock().await;

    let mut stmt = db.conn.prepare(
        "SELECT sync_status FROM public_entries WHERE key = ?1"
    )?;

    let result = stmt.query_row(rusqlite::params![full_key], |row| {
        let status: String = row.get(0)?;
        match status.as_str() {
            "synced" => Some(DateTime::from_timestamp(
                row.get::<_, i64>(1)? * 1000
            )?),
            _ => None,  // pending 或 syncing 都返回 None
        }
    });

    match result {
        Ok(status) => Ok(status),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(CisError::storage(format!("Failed to get sync status: {}", e))),
    }
}
```

---

## 三层记忆模型 (基于 MemoryDomain)

### Layer 1: 私域记忆 (物理路径 + MemoryDomain::Private)

```rust
// 存储私域记忆
service.set(
    "project/config",
    b"max_connections=100",
    MemoryDomain::Private,  // 私域加密
    MemoryCategory::Context,
).await?;

// 存储: private_entries (encrypted=1)
// 不同步到其他节点
```

### Layer 2: 公域记忆 (MemoryDomain::Public + P2P 同步)

```rust
// 存储公域记忆
service.set(
    "project/best-practice",
    b"使用 Result<T> 处理错误",
    MemoryDomain::Public,  // 公域共享
    MemoryCategory::Context,
).await?;

// 存储: public_entries (federate=1, sync_status='pending')
// P2P 模块自动同步给其他节点
```

### Layer 3: AI 整理 (公域 → 私域迁移)

```rust
// AI 从公域记忆学习，整理到私域
service.curate_from_public(
    "project/best-practice",
    CurateMode::Adapt,  // 适应到当前项目
).await?;

// 1. 从 public_entries 读取
// 2. AI 适应到当前项目上下文
// 3. 写入 private_entries (encrypted=1)
// 4. 不会同步回其他节点 (私域不同步)
```

---

## 与 Path-Based 方案的关系

### MemoryDomain 作为补充

原有的 `MemoryDomain` 枚举可以作为 **Path-Based 方案的补充**：

**场景 1: 私域记忆 + 物理路径隔离**
```rust
// 当前在 ~/repos/project-a/
service.set(
    "project/config",
    b"debug=true",
    MemoryDomain::Private,  // 私域加密
).await?;
// ✅ 加密存储
// ✅ 物理路径隔离
// ✅ 不同步
```

**场景 2: 公域记忆 + 跨节点共享**
```rust
// 当前在 ~/repos/project-a/
service.set(
    "team/best-practice",
    b"使用 Rust API guidelines",
    MemoryDomain::Public,  // 公域同步
).await?;
// ✅ P2P 自动同步给其他节点
// ✅ 其他节点能看到
```

**场景 3: AI 整理 + 公域→私域迁移**
```rust
// AI 从公域学习
service.curate_from_public(
    "team/best-practice",
    CurateMode::Adapt,
).await?;
// ✅ AI 适应后写入私域
// ✅ 不会同步回其他节点
```

---

## 实现建议

### Path-Based + MemoryDomain 结合

```rust
pub struct MemoryScope {
    pub path: PathBuf,      // 物理路径 (防幻觉)
    pub domain: MemoryDomain, // 私域/公域 (控制同步)
}

impl MemoryService {
    pub async fn set_with_scope(
        &self,
        key: &str,
        value: &[u8],
        scope: &MemoryScope,  // 包含 path + domain
        source: MemorySource,
    ) -> Result<()> {
        match scope.domain {
            MemoryDomain::Private => {
                // 1. 加密存储
                let encrypted_value = self.encrypt(value)?;

                // 2. 写入 private_entries
                self.db.set_private(key, &encrypted_value, ...).await?;

                // 3. 建立向量索引 (私域也索引)
                self.vector_storage.index_memory(...).await?;
            }

            MemoryDomain::Public => {
                // 1. 明文存储
                // 2. 写入 public_entries (federate=1, sync_status='pending')
                self.db.set_public(key, value, ...).await?;

                // 3. 建立向量索引
                self.vector_storage.index_memory(...).await?;

                // 4. 触发 P2P 同步
                self.p2p.sync_to_peers(key).await?;
            }
        }
    }
}
```

---

## 总结

**CIS 原有机制**:
- ✅ 已经实现了私域/公域区分
- ✅ 私域加密存储，不同步
- ✅ 公域支持 P2P 联邦同步
- ✅ 使用 `MemoryDomain` 枚举控制

**与 Path-Based 方案结合**:
- **物理路径隔离** → 防止跨项目/跨目录幻觉
- **MemoryDomain** → 控制是否同步 (私域 vs 公域)
- **三层模型** → 私域 → 公域 → 私域 (AI 整理)

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
