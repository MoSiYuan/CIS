# Memory Scope 稳定哈希绑定设计

> **版本**: v1.1.7
> **创建日期**: 2026-02-15
> **核心思想**: 目录哈希绑定作用域，移动和改名后哈希不变
> **用户反馈**: "目录哈希绑定作用域，这样移动和改名，目录哈希也会不变"

---

## 问题重新定义

### 用户的核心需求

**原文反馈**："目录哈希绑定作用域，这样移动和改名，目录哈希也会不变"

**关键理解**：
- ✅ **生成一次哈希**，永久绑定到项目
- ✅ **移动项目**：哈希不变（从配置文件读取）
- ✅ **重命名目录**：哈希不变（从配置文件读取）
- ✅ **第一次初始化**：自动生成哈希（用户友好）

---

## 错误设计 vs 正确设计

### ❌ 错误设计：每次都重新计算哈希

```rust
impl MemoryScope {
    pub fn from_path(path: PathBuf) -> Self {
        // 🔴 每次都重新计算哈希！
        let scope_id = Self::hash_path(&path);

        Self {
            scope_id,
            path: Some(path),
            domain: MemoryDomain::Private,
        }
    }
}

// 问题：
// 第一次：/Users/alice/project-A → hash → "a3f7e9c2b1d4f8a5"
// 移动后：/Users/alice/projects/project-A → hash → "b2e8f1d3c5a7f9e4"
// 🔴 哈希变了！记忆失效！
```

---

### ✅ 正确设计：哈希绑定到配置文件

```rust
impl MemoryScope {
    /// 🔥 从配置文件加载（哈希已绑定）
    pub fn from_config(config: &ProjectConfig) -> Result<Self> {
        let scope_id = match config.memory.scope_id.as_str() {
            // 配置文件中已有哈希 → 直接使用
            "" | "auto" if !config.memory.scope_id.is_empty() => {
                // 🔴 错误：不应该重新计算！
                // Self::hash_path(&config.project_root)

                // ✅ 正确：从配置文件读取（已绑定）
                config.memory.scope_id.clone()
            }

            // 🔴 配置文件为空 → 第一次初始化（生成并保存）
            "" | "auto" => {
                // 1. 生成哈希
                let hash = Self::hash_path(&config.project_root);

                // 2. 保存到配置文件
                config.memory.scope_id = hash.clone();
                config.save()?;

                hash
            }

            // 用户自定义的 scope_id → 直接使用
            custom => custom.to_string()
        };

        Ok(Self {
            scope_id,
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
}
```

---

## 完整工作流程

### 第一次初始化项目

```bash
# 1. 进入项目目录
cd ~/project-A

# 2. 初始化 CIS
cis project init

# 3. 自动生成哈希并保存
# .cis/project.toml
[memory]
scope_id = "a3f7e9c2b1d4f8a5"  # ← 自动生成并绑定
```

**代码流程**：
```rust
let config = ProjectConfig::load(".cis/project.toml")?;

// scope_id 为空（第一次）
assert_eq!(config.memory.scope_id, "");

// 1. 自动生成哈希
let scope = MemoryScope::from_config(&config)?;

// 2. 哈希保存到配置文件
assert_eq!(scope.scope_id, "a3f7e9c2b1d4f8a5");

// 3. 配置文件已更新
assert_eq!(config.memory.scope_id, "a3f7e9c2b1d4f8a5");
config.save()?;
```

---

### 移动项目后

```bash
# 1. 移动项目
mv ~/project-A ~/projects/project-A

# 2. 进入新目录
cd ~/projects/project-A

# 3. 读取配置文件
cis project status

# ✅ scope_id 不变！
# .cis/project.toml
[memory]
scope_id = "a3f7e9c2b1d4f8a5"  # ← 仍然是原来的哈希
```

**代码流程**：
```rust
let config = ProjectConfig::load(".cis/project.toml")?;

// scope_id 已存在（从配置文件读取）
assert_eq!(config.memory.scope_id, "a3f7e9c2b1d4f8a5");

// 1. 不会重新计算哈希！
let scope = MemoryScope::from_config(&config)?;

// 2. scope_id 保持不变
assert_eq!(scope.scope_id, "a3f7e9c2b1d4f8a5");

// 3. 记忆仍然有效
let memory_key = scope.memory_key("project/config");
// "a3f7e9c2b1d4f8a5::project/config"（与移动前相同）
```

---

### 重命名目录后

```bash
# 1. 重命名目录
mv ~/projects/project-A ~/projects/project-A-v2

# 2. 进入新目录
cd ~/projects/project-A-v2

# 3. 读取配置文件
cis project status

# ✅ scope_id 仍然不变！
# .cis/project.toml
[memory]
scope_id = "a3f7e9c2b1d4f8a5"  # ← 仍然是原来的哈希
```

---

## 配置文件示例

### .cis/project.toml

```toml
[project]
name = "my-project"
id = "proj-abc-123"

[memory]
# 🔥 稳定哈希绑定（自动生成或用户自定义）

# 方式 1: 自动生成（第一次初始化时）
# cis project init 会自动生成并保存：
# scope_id = "a3f7e9c2b1d4f8a5"

# 方式 2: 用户自定义
scope_id = "my-custom-workspace"

# 方式 3: 跨项目共享（多个项目使用同一 scope_id）
# scope_id = "team-shared-alpha"

# 可选：人类可读名称（用于调试和 UI）
display_name = "My Project Workspace"

# 可选：记忆命名空间（默认: project/{scope_id}）
namespace = "project/my-custom-workspace"
```

---

## 实现细节

### MemoryScope 完整实现

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

    /// 🔥 从目录路径创建（仅用于第一次初始化）
    ///
    /// # ⚠️ 重要提示
    ///
    /// **不要在移动项目后调用此方法**！
    /// 这会重新计算哈希，导致 scope_id 变化。
    ///
    /// **正确用法**：
    /// 1. 第一次初始化项目时调用
    /// 2. 保存 scope_id 到配置文件
    /// 3. 后续使用 `from_config()` 加载
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
```

---

## 配置文件结构

### ProjectConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    pub memory: MemoryConfig,
    // ...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 🔥 作用域 ID（稳定绑定）
    pub scope_id: String,

    /// 人类可读名称（可选）
    pub display_name: Option<String>,

    /// 记忆命名空间（默认: project/{scope_id}）
    pub namespace: Option<String>,
}

impl ProjectConfig {
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| CisError::config(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(".cis/project.toml", content)
            .map_err(|e| CisError::config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }
}
```

---

## CLI 命令支持

### cis project init

```bash
# 第一次初始化项目
cis project init

# 输出：
# ✅ Initialized CIS project
# 📝 Config file: .cis/project.toml
# 🔐 Scope ID: a3f7e9c2b1d4f8a5 (auto-generated)
```

---

### cis project status

```bash
# 查看项目状态
cis project status

# 输出：
# 📋 Project: my-project
# 🔐 Scope ID: a3f7e9c2b1d4f8a5
# 📂 Path: ~/projects/project-A
# 💾 Memory keys: 12
```

---

### cis project set-scope

```bash
# 修改 scope_id（高级用法）
cis project set-scope "my-custom-workspace"

# 输出：
# ✅ Scope ID updated: a3f7e9c2b1d4f8a5 → my-custom-workspace
# ⚠️  Warning: Previous memory keys will be inaccessible
```

---

## 测试用例

### 测试 1: 第一次初始化

```rust
#[test]
fn test_first_time_initialization() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path().join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    // 1. 创建配置文件（scope_id 为空）
    let config = ProjectConfig {
        project: ProjectSection {
            name: "my-project".into(),
            root: project_dir.clone(),
        },
        memory: MemoryConfig {
            scope_id: "".into(),  // ← 第一次初始化
            display_name: None,
            namespace: None,
        },
    };

    // 2. 生成 MemoryScope
    let scope = MemoryScope::from_config(&config).unwrap();

    // 3. 验证哈希已生成并保存
    assert!(!config.memory.scope_id.is_empty());
    assert_eq!(scope.scope_id, config.memory.scope_id);

    // 4. 验证配置文件已保存
    let config_path = project_dir.join(".cis/project.toml");
    assert!(config_path.exists());
}
```

---

### 测试 2: 移动项目后哈希不变

```rust
#[test]
fn test_move_project_scope_id_unchanged() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. 第一次初始化
    let old_path = temp_dir.path().join("project-A");
    std::fs::create_dir_all(&old_path).unwrap();

    let mut config = ProjectConfig {
        project: ProjectSection {
            name: "project-A".into(),
            root: old_path.clone(),
        },
        memory: MemoryConfig {
            scope_id: "".into(),
            display_name: None,
            namespace: None,
        },
    };

    let old_scope = MemoryScope::from_config(&config).unwrap();
    let old_scope_id = old_scope.scope_id.clone();

    // 2. 模拟移动项目
    let new_path = temp_dir.path().join("projects/project-A");
    std::fs::create_dir_all(&new_path).unwrap();
    config.project.root = new_path.clone();

    // 3. 重新加载（scope_id 不变）
    let new_scope = MemoryScope::from_config(&config).unwrap();

    // 4. 验证 scope_id 不变
    assert_eq!(new_scope.scope_id, old_scope_id);
    assert_ne!(new_scope.path, old_scope.path);
}
```

---

### 测试 3: 重命名目录后哈希不变

```rust
#[test]
fn test_rename_directory_scope_id_unchanged() {
    let temp_dir = tempfile::tempdir().unwrap();

    // 1. 第一次初始化
    let old_path = temp_dir.path().join("my-project");
    std::fs::create_dir_all(&old_path).unwrap();

    let mut config = ProjectConfig {
        project: ProjectSection {
            name: "my-project".into(),
            root: old_path.clone(),
        },
        memory: MemoryConfig {
            scope_id: "".into(),
            display_name: None,
            namespace: None,
        },
    };

    let old_scope = MemoryScope::from_config(&config).unwrap();
    let old_scope_id = old_scope.scope_id.clone();

    // 2. 模拟重命名
    let new_path = temp_dir.path().join("my-project-v2");
    std::fs::create_dir_all(&new_path).unwrap();
    config.project.root = new_path.clone();
    config.project.name = "my-project-v2".into();

    // 3. 重新加载（scope_id 不变）
    let new_scope = MemoryScope::from_config(&config).unwrap();

    // 4. 验证 scope_id 不变
    assert_eq!(new_scope.scope_id, old_scope_id);
    assert_ne!(new_scope.path, old_path);
}
```

---

## 总结

### ✅ 稳定哈希绑定机制

| 场景 | 原方案（重新计算） | 新方案（配置绑定） |
|------|----------|----------|
| **第一次初始化** | 生成哈希 | ✅ 生成哈希并保存 |
| **移动项目** | 🔴 哈希变化 | ✅ 哈希不变（从配置读取） |
| **重命名目录** | 🔴 哈希变化 | ✅ 哈希不变（从配置读取） |
| **不同机器** | 🔴 哈希变化 | ✅ 哈希不变（配置文件同步） |

---

### 核心保证

1. ✅ **第一次初始化**：自动生成哈希并保存到 `.cis/project.toml`
2. ✅ **移动/重命名**：scope_id 不变（从配置文件读取）
3. ✅ **跨项目共享**：多个项目使用同一 scope_id
4. ✅ **用户自定义**：支持手动指定 scope_id

---

### 与用户反馈一致

✅ "目录哈希绑定作用域" → 哈希保存到配置文件，永久绑定
✅ "移动和改名，目录哈希也会不变" → 从配置文件读取，不重新计算
✅ "path只是默认值" → 支持用户自定义 scope_id

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-15
**用户反馈**: "目录哈希绑定作用域，这样移动和改名，目录哈希也会不变"
