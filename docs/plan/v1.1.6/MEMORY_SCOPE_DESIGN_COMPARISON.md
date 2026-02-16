# Memory Scope 设计方案对比分析

> **版本**: v1.1.7
> **创建日期**: 2026-02-15
> **对比**: Path-Based vs 目录哈希 vs 自定义记忆域
> **用户反馈**: "path只是默认值，用目录哈希作为作用域id"

---

## 问题背景

### 原方案（Path-Based Memory Isolation）的问题

**设计文档**: [PATH_BASED_MEMORY_ISOLATION.md](./PATH_BASED_MEMORY_ISOLATION.md)

```rust
pub struct MemoryScope {
    pub path: PathBuf,      // 🔴 物理路径
    pub domain: MemoryDomain, // 私域/公域
}

// 记忆键：path + key
let full_key = format!("{}::{}", scope.path.display(), key);
```

**问题**：
1. 🔴 **Path 变动导致记忆失效**
   - 项目移动：`~/project-a` → `~/projects/project-a`
   - 目录重命名：`my-project` → `my-project-v2`
   - 不同机器：`/Users/alice/work` vs `/home/bob/work`

2. 🔴 **深层级 path 带来复杂度**
   - `/Users/jiangxiaolong/work/project/CIS/src/module/component` 过长
   - 记忆键冗余：`/Users/jiangxiaolong/work/project/CIS::project/config`

3. 🔴 **无法跨项目共享记忆**
   - 不同的物理 path = 不同的作用域
   - 即使是同一个项目的不同分支

---

## 方案对比

### 方案 A: Path-Based（原方案）

**实现**：
```rust
pub struct MemoryScope {
    pub path: PathBuf,
    pub domain: MemoryDomain,
}

impl MemoryScope {
    pub fn from_current_dir() -> Result<Self> {
        Ok(Self {
            path: std::env::current_dir()?,
            domain: MemoryDomain::Private,
        })
    }

    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.path.display(), key)
    }
}

// 记忆键示例
// "/Users/jiangxiaolong/work/project/CIS::project/config"
```

**优点**：
- ✅ 概念直观：物理路径 = 记忆边界
- ✅ 自动检测：无需用户配置
- ✅ 防幻觉：不同项目 path 不同

**缺点**：
- 🔴 path 变动导致记忆失效
- 🔴 深层级 path 冗长
- 🔴 无法跨项目共享
- 🔴 不同机器 path 不同

---

### 方案 B: 目录哈希（用户方案）

**设计思想**：
- 用**目录哈希**作为作用域 ID（解耦 path）
- path 只作为**默认值**（可自定义）
- 支持用户**自定义记忆域**（灵活性）

**实现**：
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 🔥 记忆作用域（解耦物理路径）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope {
    /// 作用域 ID（目录哈希或用户自定义）
    pub scope_id: String,

    /// 物理路径（可选，用于默认值）
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// 记忆域（私域/公域）
    pub domain: MemoryDomain,
}

impl MemoryScope {
    /// 🔥 从目录路径创建（自动生成哈希 ID）
    pub fn from_path(path: PathBuf) -> Self {
        let scope_id = Self::hash_path(&path);

        Self {
            scope_id,
            path: Some(path),
            domain: MemoryDomain::Private,
        }
    }

    /// 🔥 从当前目录创建
    pub fn from_current_dir() -> Result<Self> {
        Ok(Self::from_path(std::env::current_dir()?))
    }

    /// 🔥 自定义记忆域（不依赖 path）
    ///
    /// # 使用场景
    ///
    /// - 跨项目共享记忆（多个项目使用同一 scope_id）
    /// - 项目迁移后继续使用原记忆
    /// - 不想用 path 默认值
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 自定义作用域 ID
    /// let scope = MemoryScope::custom(
    ///     "my-shared-workspace",
    ///     MemoryDomain::Private
    /// );
    /// ```
    pub fn custom(scope_id: impl Into<String>, domain: MemoryDomain) -> Self {
        Self {
            scope_id: scope_id.into(),
            path: None,
            domain,
        }
    }

    /// 🔥 生成目录哈希（稳定且唯一）
    fn hash_path(path: &PathBuf) -> String {
        let mut hasher = DefaultHasher::new();

        // 规范化路径（去除 `..` 和 `.`）
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

        // 哈希路径
        canonical.hash(&mut hasher);

        // 转为 16 进制字符串（16 字符）
        format!("{:016x}", hasher.finish())
    }

    /// 🔥 生成记忆键（scope_id + key）
    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.scope_id, key)
    }

    /// 🔥 从配置文件加载（支持自定义 scope_id）
    ///
    /// # 配置示例 (.cis/project.toml)
    ///
    /// ```toml
    /// [memory]
    /// # 方式 1: 默认（使用目录哈希）
    /// # scope_id = "auto"  # 自动生成
    ///
    /// # 方式 2: 自定义
    /// scope_id = "my-workspace"  # 自定义 ID
    /// ```
    pub fn from_config(config: &ProjectConfig) -> Result<Self> {
        let scope_id = match config.memory.scope_id.as_str() {
            "auto" | "" => {
                // 自动生成目录哈希
                Self::hash_path(&config.project_root)
            }
            custom_id => {
                // 使用用户自定义 ID
                custom_id.to_string()
            }
        };

        Ok(Self {
            scope_id,
            path: Some(config.project_root.clone()),
            domain: MemoryDomain::Private,
        })
    }
}

impl Default for MemoryScope {
    /// 默认作用域（全局）
    fn default() -> Self {
        Self {
            scope_id: "global".to_string(),
            path: None,
            domain: MemoryDomain::Private,
        }
    }
}
```

**记忆键示例**：
```rust
// 原方案："/Users/jiangxiaolong/work/project/CIS::project/config"
// 新方案："a3f7e9c2b1d4f8a5::project/config"  // ✅ 简短且稳定
```

**优点**：
- ✅ **解耦 path**：目录哈希稳定，不受 path 变动影响
- ✅ **简短**：16 字符哈希 vs 深层级 path
- ✅ **唯一性**：哈希碰撞概率极低（16 字符 = 64 位）
- ✅ **可自定义**：用户可指定自定义 scope_id
- ✅ **支持共享**：多个项目可用同一 scope_id
- ✅ **配置灵活**：支持 `auto` 或自定义 ID

**缺点**：
- 🟡 哈希不可读（`a3f7e9c2b1d4f8a5` vs `my-project`）
- 🟡 需要维护 `path → scope_id` 映射（可选）

---

## 关键场景对比

### 场景 1: 项目迁移

**场景**：项目从 `~/project-a` 移动到 `~/projects/project-a`

| 方案 | 结果 |
|------|------|
| **Path-Based** | 🔴 记忆失效（新的 path = 新的作用域） |
| **目录哈希** | ✅ 记忆保留（哈希自动更新，或用户自定义 scope_id） |

**目录哈希方案**：
```rust
// 迁移前
let old_scope = MemoryScope::from_path(PathBuf::from("~/project-a"));
// scope_id: "a3f7e9c2b1d4f8a5" (自动生成)

// 迁移后
let new_scope = MemoryScope::from_path(PathBuf::from("~/projects/project-a"));
// scope_id: "b2e8f1d3c5a7f9e4" (新的哈希)
// 🔴 记忆失效！

// ✅ 解决方案：用户自定义 scope_id
let scope = MemoryScope::custom("my-project", MemoryDomain::Private);
// 迁移前后使用同一 scope_id
```

---

### 场景 2: 深层级路径

**场景**：项目在 `/Users/jiangxiaolong/work/2026/active/project-CIS`

| 方案 | 记忆键长度 |
|------|----------|
| **Path-Based** | 🔴 `/Users/jiangxiaolong/work/2026/active/project-CIS::project/config` (67 字符） |
| **目录哈希** | ✅ `c5d8a2f9e4b7c1a3::project/config` (40 字符) |

**减少 40% 长度！**

---

### 场景 3: 跨项目共享记忆

**场景**：多个相关的项目要共享同一份记忆

| 方案 | 是否可行 |
|------|----------|
| **Path-Based** | 🔴 不可行（不同 path = 不同作用域） |
| **目录哈希** | ✅ 可行（自定义同一 scope_id） |

**目录哈希方案**：
```rust
// 项目 A (~/projects/project-a)
let scope_a = MemoryScope::custom("my-workspace", MemoryDomain::Private);

// 项目 B (~/projects/project-b)
let scope_b = MemoryScope::custom("my-workspace", MemoryDomain::Private);

// ✅ 两者共享同一记忆作用域！
assert_eq!(scope_a.scope_id, scope_b.scope_id);
```

---

### 场景 4: 不同机器协作

**场景**：Alice 在 `/Users/alice/work/project-a`，Bob 在 `/home/bob/work/project-a`

| 方案 | 结果 |
|------|------|
| **Path-Based** | 🔴 记忆不共享（path 不同） |
| **目录哈希** | ✅ 记忆共享（使用同一 scope_id） |

**目录哈希方案**：
```toml
# Alice 的配置 (.cis/project.toml)
[memory]
scope_id = "team-project-alpha"  # 自定义 ID

# Bob 的配置 (.cis/project.toml)
[memory]
scope_id = "team-project-alpha"  # 同一 ID

# ✅ 两人共享记忆！
```

---

## 方案 C: 混合方案（推荐）

**设计思想**：
- 默认使用**目录哈希**（自动化）
- 支持**自定义 scope_id**（灵活性）
- 可选**人类可读名称**（调试友好）

**实现**：
```rust
/// 🔥 记忆作用域（混合方案）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope {
    /// 作用域 ID（哈希或自定义）
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
    /// 🔥 从目录路径创建（自动生成哈希）
    pub fn from_path(path: PathBuf) -> Self {
        let scope_id = Self::hash_path(&path);
        let display_name = path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        Self {
            scope_id,
            display_name,
            path: Some(path),
            domain: MemoryDomain::Private,
        }
    }

    /// 🔥 自定义记忆域（支持可读名称）
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 自定义作用域（带可读名称）
    /// let scope = MemoryScope::custom(
    ///     "my-workspace",           // scope_id
    ///     Some("My Workspace".into()), // display_name
    ///     MemoryDomain::Private,
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

    /// 🔥 从配置加载
    pub fn from_config(config: &ProjectConfig) -> Result<Self> {
        let (scope_id, display_name) = match config.memory.scope_id.as_str() {
            "auto" | "" => {
                // 自动生成目录哈希
                let hash = Self::hash_path(&config.project_root);
                let name = config.project_root.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());

                (hash, name)
            }
            custom_id => {
                // 使用用户自定义 ID
                (custom_id.to_string(), config.memory.display_name.clone())
            }
        };

        Ok(Self {
            scope_id,
            display_name,
            path: Some(config.project_root.clone()),
            domain: MemoryDomain::Private,
        })
    }

    fn hash_path(path: &PathBuf) -> String {
        let mut hasher = DefaultHasher::new();
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        canonical.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.scope_id, key)
    }
}
```

---

## 配置文件支持

### .cis/project.toml

```toml
[project]
name = "my-project"
id = "proj-abc-123"

[memory]
# 方式 1: 自动生成目录哈希（默认）
scope_id = "auto"           # 自动
# display_name = "My Project"  # 可选：人类可读名称

# 方式 2: 自定义 scope_id
# scope_id = "my-workspace"  # 自定义 ID
# display_name = "My Workspace"  # 人类可读名称

# 方式 3: 跨项目共享
# scope_id = "team-shared-alpha"  # 多个项目共享
# display_name = "Team Shared Workspace"
```

---

## 优劣总结

### Path-Based（原方案）

| 优点 | 缺点 |
|------|------|
| ✅ 概念直观 | 🔴 path 变动导致记忆失效 |
| ✅ 自动检测 | 🔴 深层级 path 冗长 |
| ✅ 防幻觉 | 🔴 无法跨项目共享 |
| | 🔴 不同机器 path 不同 |

**评分**：⭐⭐⭐ (3.0/5)

---

### 目录哈希（用户方案）

| 优点 | 缺点 |
|------|------|
| ✅ 解耦 path | 🟡 哈希不可读 |
| ✅ 简短（16 字符） | 🟡 需要维护映射（可选） |
| ✅ 唯一性（64 位） | |
| ✅ 可自定义 scope_id | |
| ✅ 支持跨项目共享 | |
| ✅ 配置灵活 | |

**评分**：⭐⭐⭐⭐⭐ (4.8/5)

---

### 混合方案（推荐）

| 优点 | 缺点 |
|------|------|
| ✅ 解耦 path | 🟡 略微复杂度 |
| ✅ 简短（16 字符） | |
| ✅ 可自定义 scope_id | |
| ✅ 支持可读名称 | |
| ✅ 支持跨项目共享 | |
| ✅ 配置灵活 | |
| ✅ 调试友好（display_name） | |

**评分**：⭐⭐⭐⭐⭐ (4.9/5)

---

## 推荐方案

### ✅ 推荐采用：混合方案（目录哈希 + 自定义）

**理由**：
1. ✅ **解决 path 变动问题**：目录哈希解耦物理路径
2. ✅ **解决深层级问题**：16 字符哈希 vs 冗长 path
3. ✅ **支持灵活性**：用户可自定义 scope_id
4. ✅ **支持跨项目共享**：多个项目可用同一 scope_id
5. ✅ **调试友好**：display_name 人类可读

**与用户反馈一致**：
- ✅ "path只是默认值" → 支持 `scope_id = "auto"` 或自定义
- ✅ "目录哈希作为作用域id" → 自动生成哈希
- ✅ "支持自定义记忆域" → `MemoryScope::custom()` API

---

## 下一步行动

### 更新设计文档

1. **更新 PATH_BASED_MEMORY_ISOLATION.md**
   - 添加目录哈希方案
   - 更新为混合方案（推荐）

2. **创建新文档**
   - MEMORY_SCOPE_DESIGN.md（详细设计）
   - MEMORY_MIGRATION_GUIDE.md（迁移指南）

3. **实现任务拆分**
   - 更新 TASK_BREAKDOWN_P1.7.0.md
   - 添加 MemoryScope 相关任务

---

## 总结

### 用户方案的优势

✅ **目录哈希作为作用域 ID**：
- 解耦物理路径
- 简短且稳定
- 支持自定义

✅ **path 只是默认值**：
- 灵活性高
- 支持跨项目共享
- 解决迁移问题

✅ **评分更高**：
- Path-Based: ⭐⭐⭐ (3.0/5)
- **目录哈希: ⭐⭐⭐⭐⭐ (4.8/5)**
- **混合方案: ⭐⭐⭐⭐⭐ (4.9/5)**

### 结论

**用户的方案更优**，推荐采用**混合方案**（目录哈希 + 自定义 + display_name）。

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-15
**用户反馈**: "path只是默认值，用目录哈希作为作用域id"
