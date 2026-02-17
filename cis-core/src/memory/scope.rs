//! # Memory Scope (记忆作用域)
//!
//! **稳定哈希绑定机制** (v1.1.7)
//!
//! # 设计原理
//!
//! - **第一次初始化**：生成目录哈希并保存到 `.cis/project.toml`
//! - **移动/重命名后**：从配置文件读取（哈希不变）
//! - **用户自定义**：支持手动指定 scope_id
//!
//! # 核心保证
//!
//! | 场景 | 行为 | scope_id |
//! |------|------|----------|
//! | **第一次初始化** | 生成哈希并保存 | [OK] "a3f7e9c2b1d4f8a5" |
//! | **移动项目** | 从配置文件读取 | [OK] 仍然是 "a3f7e9c2b1d4f8a5" |
//! | **重命名目录** | 从配置文件读取 | [OK] 仍然是 "a3f7e9c2b1d4f8a5" |
//! | **不同机器协作** | 配置文件同步 | [OK] 两台机器相同 |

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CisError, Result};
use crate::types::MemoryDomain;

/// 记忆作用域（稳定哈希绑定）
///
/// # 稳定性保证
///
/// - **自动生成的哈希**：永久绑定到项目（移动/重命名后不变）
/// - **用户自定义 ID**：用户控制的稳定性
/// - **第一次初始化**：生成哈希并保存到配置文件
/// - **后续加载**：从配置文件读取（不会重新计算）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope {
    /// 作用域 ID（哈希或用户自定义）
    ///
    /// # 稳定性
    ///
    /// - 自动生成的哈希：16 字符 16 进制字符串（如 "a3f7e9c2b1d4f8a5"）
    /// - 用户自定义 ID：人类可读的字符串（如 "my-workspace"）
    /// - 一旦生成/设置，永久绑定到项目
    pub scope_id: String,

    /// 人类可读名称（可选，用于调试和 UI）
    ///
    /// # 示例
    ///
    /// - `Some("My Project".into())` - 项目名称
    /// - `Some("Team Shared".into())` - 团队共享名称
    /// - `None` - 无可读名称
    pub display_name: Option<String>,

    /// 物理路径（可选，仅用于第一次初始化）
    ///
    /// # 注意
    ///
    /// **不作为记忆键的一部分**！
    /// 记忆键只使用 `scope_id`，解耦物理路径。
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// 记忆域（私域/公域）
    pub domain: MemoryDomain,
}

impl MemoryScope {
    /// 从配置文件加载（核心方法）
    ///
    /// # 稳定性保证
    ///
    /// - **第一次初始化**：生成哈希并保存到配置文件
    /// - **后续加载**：从配置文件读取（不会重新计算）
    /// - **移动/重命名**：scope_id 不变（从配置文件读取）
    ///
    /// # 参数
    ///
    /// - `config`: 项目配置（`.cis/project.toml`）
    ///
    /// # 返回
    ///
    /// 返回 `MemoryScope`，其中 `scope_id` 从配置文件读取或生成。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = ProjectConfig::load(".cis/project.toml")?;
    ///
    /// // [OK] 第一次：生成哈希并保存
    /// // [OK] 移动后：从配置读取（哈希不变）
    /// let scope = MemoryScope::from_config(&config)?;
    ///
    /// println!("Scope ID: {}", scope.scope_id);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config(config: &crate::project::ProjectConfig) -> Result<Self> {
        let scope_id = Self::load_or_generate_scope_id(config)?;

        let display_name = config.memory.display_name.clone();
        let path = Some(config.root_dir.clone());
        let domain = MemoryDomain::Private;

        Ok(Self {
            scope_id,
            display_name,
            path,
            domain,
        })
    }

    /// 自定义记忆域（不依赖 path）
    ///
    /// # 使用场景
    ///
    /// - **跨项目共享记忆**：多个项目使用同一 scope_id
    /// - **不想用自动生成的哈希**：需要人类可读的 ID
    /// - **团队共享记忆**：团队成员使用同一 scope_id
    ///
    /// # 参数
    ///
    /// - `scope_id`: 自定义作用域 ID（如 "my-workspace"）
    /// - `display_name`: 人类可读名称（可选）
    /// - `domain`: 记忆域（私域/公域）
    ///
    /// # 示例
    ///
    /// ```rust
    /// // 自定义作用域 ID（跨项目共享）
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

    /// 全局作用域（跨所有项目）
    ///
    /// # 示例
    ///
    /// ```rust
    /// let global = MemoryScope::global();
    /// assert_eq!(global.scope_id, "global");
    /// ```
    pub fn global() -> Self {
        Self {
            scope_id: "global".to_string(),
            display_name: Some("Global".into()),
            path: None,
            domain: MemoryDomain::Private,
        }
    }

    /// 生成记忆键（scope_id + key）
    ///
    /// # 格式
    ///
    /// ```text
    /// {scope_id}::{key}
    /// ```
    ///
    /// # 示例
    ///
    /// ```text
    /// scope_id: "a3f7e9c2b1d4f8a5"
    /// key: "project/config"
    /// → "a3f7e9c2b1d4f8a5::project/config"
    /// ```
    ///
    /// # 优势
    ///
    /// - [OK] 简短（16 字符 vs 冗长 path）
    /// - [OK] 稳定（哈希不变，即使 path 变化）
    /// - [OK] 唯一（哈希碰撞概率极低）
    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.scope_id, key)
    }

    /// 判断是否为全局作用域
    ///
    /// # 示例
    ///
    /// ```rust
    /// let global = MemoryScope::global();
    /// assert!(global.is_global());
    ///
    /// let project = MemoryScope::custom("my-project", None, MemoryDomain::Private);
    /// assert!(!project.is_global());
    /// ```
    pub fn is_global(&self) -> bool {
        self.scope_id == "global"
    }

    /// 🔒 生成目录哈希（稳定且唯一，安全加固版）
    ///
    /// # 算法
    ///
    /// 1. **路径遍历检测**：检查 `../` 等模式
    /// 2. **规范路径**：`canonicalize()` 去除 `..` 和 `.`
    /// 3. **哈希计算**：使用 `DefaultHasher`（64 位）
    /// 4. **转 16 进制**：16 字符字符串（如 "a3f7e9c2b1d4f8a5"）
    ///
    /// # 安全修复 (P0)
    ///
    /// - 旧实现：`canonicalize()` 失败时使用原始路径（[WARNING] 不安全）
    /// - 新实现：对不存在的路径使用绝对路径+安全盐值（[OK] 安全）
    ///
    /// # 唯一性
    ///
    /// - 64 位哈希 → 碰撞概率 ≈ 1/2^64
    /// - 16 字符 16 进制 → 足够唯一
    ///
    /// # 稳定性
    ///
    /// - **同一路径**：永远生成相同哈希
    /// - **不同路径**：极大概率生成不同哈希
    fn hash_path(path: &PathBuf) -> String {
        // 1. 🔒 安全检查：检测路径遍历攻击
        let path_str = path.to_string_lossy();
        if path_str.contains("../") || path_str.contains("..\\") {
            tracing::warn!(
                "Path traversal detected in hash_path: {}",
                path.display()
            );
            // 使用安全的前缀防止哈希碰撞
            let mut hasher = DefaultHasher::new();
            "PATH_TRAVERSAL_DETECTED".hash(&mut hasher);
            return format!("{:016x}", hasher.finish());
        }

        // 2. 尝试规范路径
        let canonical = path.canonicalize();

        let hash_input = match canonical {
            Ok(canonical_path) => {
                // 路径存在且成功规范化
                canonical_path
            }
            Err(e) => {
                // 3. 🔒 路径不存在：使用绝对路径+安全盐值
                tracing::debug!(
                    "Path does not exist, using absolute path: {} (error: {})",
                    path.display(),
                    e
                );

                // 转换为绝对路径
                let abs_path = if path.is_absolute() {
                    path.clone()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("/"))
                        .join(path)
                };

                // 4. 🔒 添加特殊标记区分存在的路径和虚拟路径
                //    这样即使路径名相同，哈希也不同
                let mut hasher = DefaultHasher::new();
                "VIRTUAL_PATH_SALT".hash(&mut hasher);
                abs_path.hash(&mut hasher);
                return format!("{:016x}", hasher.finish());
            }
        };

        // 5. 计算最终哈希
        let mut hasher = DefaultHasher::new();
        hash_input.hash(&mut hasher);

        // 转为 16 进制字符串（16 字符）
        format!("{:016x}", hasher.finish())
    }

    /// 从配置加载或生成 scope_id
    ///
    /// # 核心逻辑
    ///
    /// 1. **配置文件中有 scope_id** → 直接使用（稳定绑定）
    /// 2. **配置文件中没有 scope_id** → 生成哈希并保存（第一次初始化）
    ///
    /// # 参数
    ///
    /// - `config`: 项目配置（`.cis/project.toml`）
    ///
    /// # 返回
    ///
    /// 返回 scope_id 字符串。
    ///
    /// # 副作用
    ///
    /// - 第一次初始化：生成哈希并保存到 `config.memory.scope_id`
    /// - 后续加载：直接返回 `config.memory.scope_id`
    fn load_or_generate_scope_id(config: &crate::project::ProjectConfig) -> Result<String> {
        match config.memory.scope_id.as_str() {
            // 配置文件中已有 → 直接使用
            id if !id.is_empty() && id != "auto" => {
                println!("[DEBUG] Using existing scope_id: {}", id);
                Ok(id.to_string())
            }

            // 配置文件中没有 → 生成并保存
            "" | "auto" => {
                println!("[INFO] Generating new scope_id for project: {}", config.name);

                // 1. 生成哈希
                let hash = Self::hash_path(&config.root_dir);

                println!("[INFO] Generated scope_id: {}", hash);

                // 2. 保存到配置文件（通过 clone 确保保存）
                let mut config_clone = config.clone();
                config_clone.memory.scope_id = hash.clone();

                if let Err(e) = config_clone.save() {
                    eprintln!("[ERROR] Failed to save scope_id to config: {}", e);
                    return Err(CisError::config(format!(
                        "Failed to save scope_id to config: {}", e
                    )));
                }

                println!("[INFO] Saved scope_id to .cis/project.toml");
                Ok(hash)
            }

            // 不应该到达
            id => {
                eprintln!("[ERROR] Unexpected scope_id value: {}", id);
                unreachable!("Unexpected scope_id value: {}", id)
            }
        }
    }
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::global()
    }
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.display_name {
            write!(f, "{} ({})", name, self.scope_id)
        } else {
            write!(f, "{}", self.scope_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// 测试目录哈希生成
    #[test]
    fn test_hash_path_generation() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test-project");
        fs::create_dir_all(&path).unwrap();

        let hash1 = MemoryScope::hash_path(&path);
        let hash2 = MemoryScope::hash_path(&path);

        // 同一路径 → 相同哈希
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16);

        // 哈希格式：16 进制
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit() || c == '0'));
    }

    /// 测试不同路径生成不同哈希
    #[test]
    fn test_hash_path_uniqueness() {
        let temp = TempDir::new().unwrap();

        let path1 = temp.path().join("project-a");
        let path2 = temp.path().join("project-b");

        fs::create_dir_all(&path1).unwrap();
        fs::create_dir_all(&path2).unwrap();

        let hash1 = MemoryScope::hash_path(&path1);
        let hash2 = MemoryScope::hash_path(&path2);

        // 不同路径 → 不同哈希（极大概率）
        assert_ne!(hash1, hash2);
    }

    /// 测试自定义作用域
    #[test]
    fn test_custom_scope() {
        let scope = MemoryScope::custom(
            "my-workspace",
            Some("My Workspace"),
            MemoryDomain::Private
        );

        assert_eq!(scope.scope_id, "my-workspace");
        assert_eq!(scope.display_name, Some("My Workspace".to_string()));
        assert_eq!(scope.domain, MemoryDomain::Private);
        assert!(scope.path.is_none());
    }

    /// 测试全局作用域
    #[test]
    fn test_global_scope() {
        let global = MemoryScope::global();

        assert_eq!(global.scope_id, "global");
        assert!(global.is_global());
    }

    /// 测试记忆键生成
    #[test]
    fn test_memory_key_generation() {
        let scope = MemoryScope::custom(
            "a3f7e9c2b1d4f8a5",
            None,
            MemoryDomain::Private
        );

        let key = scope.memory_key("project/config");

        assert_eq!(key, "a3f7e9c2b1d4f8a5::project/config");
    }

    /// 测试 Display 实现
    #[test]
    fn test_display_implementation() {
        let scope_with_name = MemoryScope::custom(
            "test-scope",
            Some("Test Scope"),
            MemoryDomain::Private
        );

        let scope_without_name = MemoryScope::custom(
            "test-scope-2",
            None,
            MemoryDomain::Private
        );

        assert_eq!(format!("{}", scope_with_name), "Test Scope (test-scope)");
        assert_eq!(format!("{}", scope_without_name), "test-scope-2");
    }

    /// 测试 Default 实现
    #[test]
    fn test_default_implementation() {
        let scope = MemoryScope::default();

        assert_eq!(scope.scope_id, "global");
        assert!(scope.is_global());
    }
}
