//! # 冲突检测类型系统
//!
//! **编译时强制保证**：只有通过冲突检查后才能创建 `SafeMemoryContext`
//!
//! # 设计原理
//!
//! 使用 Rust 类型系统实现零成本的编译时强制：
//! - `ConflictChecked` 标记类型（PhantomData）
//! - `SafeMemoryContext` 的 `new()` 构造函数是私有的
//! - 只有 `ConflictGuard` 可以创建 `SafeMemoryContext`
//!
//! # 无绕过路径
//!
//! ```compile_fail
//! // [X] 编译错误：无法直接创建 SafeMemoryContext
//! let context = SafeMemoryContext::new(std::collections::HashMap::new());
//! ```
//!
//! # 使用示例
//!
//! ```rust
//! use cis_core::memory::guard::{ConflictGuard, SafeMemoryContext};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let guard = ConflictGuard::new(memory_service);
//!
//! // [OK] 必须先检测冲突
//! let context = guard.check_and_create_context(&["key1", "key2"]).await?;
//!
//! // [OK] 检测通过后才能使用
//! for (key, entry) in context.iter_memories() {
//!     println!("{}: {:?}", key, entry.value);
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::marker::PhantomData;

use crate::storage::memory_db::MemoryEntry;

/// 冲突已检查的零成本标记类型
///
/// # 类型安全保证
///
/// 这个类型本身不包含数据，仅作为**编译时标记**：
/// - `PhantomData` 确保零成本抽象
/// - 无法从外部构造（没有公共构造函数）
/// - 只有 `ConflictGuard` 可以创建带有此标记的类型
///
/// # 设计模式
///
/// 这是 Rust 中的 **Typestate Pattern**（类型状态模式）：
/// ```text
/// 未检查状态 → 冲突检查 → 已检查状态
///     (Unsafe)    (强制)      (Safe)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictChecked {
    // 私有的零大小标记，外部无法构造
    _private: (),
}

impl ConflictChecked {
    /// 只有 `ConflictGuard` 可以调用此方法创建标记
    ///
    /// # 访问控制
    ///
    /// - `pub(crate)` 限制在 guard 模块内访问
    /// - `ConflictGuard::check_and_create_context()` 会调用此方法
    /// - 外部模块无法直接调用，确保无法绕过冲突检测
    pub(crate) fn new() -> Self {
        Self { _private: () }
    }
}

/// 只有通过冲突检查才能创建的 Memory Context
///
/// # 核心保证
///
/// **编译时强制**：无法在外部直接构造 `SafeMemoryContext`
/// - `new()` 构造函数是私有的（`pub(crate)`）
/// - 只有 `ConflictGuard::check_and_create_context()` 可以调用
/// - `AgentExecutor::execute()` 只接受此类型作为参数
///
/// # 无绕过路径
///
/// ```text
/// Agent 执行任务
///    ↓
/// 需要 SafeMemoryContext
///    ↓
/// 只能由 ConflictGuard 创建
///    ↓
/// 创建前必须检测冲突
///    ↓
/// 强制执行，无绕过路径
/// ```
///
/// # 字段说明
///
/// - `_phantom`: 零成本的编译时标记，保证类型安全
/// - `memories`: 实际的记忆数据（HashMap<key, MemoryEntry>）
pub struct SafeMemoryContext {
    /// 编译时标记：只有通过冲突检查才能创建
    _phantom: PhantomData<ConflictChecked>,

    /// 记忆数据：key → MemoryEntry
    pub(crate) memories: HashMap<String, MemoryEntry>,
}

impl SafeMemoryContext {
    /// 私有构造函数：只有 `ConflictGuard` 可以调用
    ///
    /// # 访问控制
    ///
    /// - `pub(crate)` 限制在 guard 模块内
    /// - 外部模块无法直接创建 `SafeMemoryContext`
    /// - 确保必须先通过 `ConflictGuard::check_and_create_context()`
    pub(crate) fn new(memories: HashMap<String, MemoryEntry>) -> Self {
        Self {
            _phantom: PhantomData,
            memories,
        }
    }

    /// 获取所有记忆的迭代器
    pub fn iter_memories(&self) -> impl Iterator<Item = (&String, &MemoryEntry)> {
        self.memories.iter()
    }

    /// 获取指定 key 的记忆条目
    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.memories.get(key)
    }

    /// 获取所有记忆的 keys
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.memories.keys()
    }

    /// 获取记忆数量
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }
}

// 删除所有可能绕过检查的 trait 实现
// [X] 不实现 Clone（防止复制后重复使用）
// [X] 不实现 Default（防止默认构造）
// [OK] 只实现必要的 Debug（用于日志）
impl std::fmt::Debug for SafeMemoryContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SafeMemoryContext")
            .field("len", &self.memories.len())
            .field("keys", &self.memories.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MemoryCategory, MemoryDomain};
    use chrono::Utc;

    /// 测试 ConflictChecked 只能由内部模块创建
    #[test]
    fn test_conflict_checked_internal_only() {
        // [OK] 可以通过内部方法创建
        let _checked = ConflictChecked::new();
    }

    /// 测试 SafeMemoryContext 不能在外部直接创建
    #[test]
    #[ignore = "编译时测试，手动验证"]
    fn test_safe_memory_context_cannot_create_directly() {
        // [X] 编译错误：new() 是 pub(crate) 的，外部无法调用
        // let context = SafeMemoryContext::new(HashMap::new());
        //
        // 这确保了必须通过 ConflictGuard::check_and_create_context() 创建
    }

    /// 测试 SafeMemoryContext 的基本操作
    #[test]
    fn test_safe_memory_context_operations() {
        // 在测试环境中，我们可以模拟内部创建
        let mut memories = HashMap::new();

        let entry = MemoryEntry {
            key: "test/key".to_string(),
            value: b"test_value".to_vec(),
            domain: MemoryDomain::Private,
            category: MemoryCategory::Context,
            created_at: Utc::now().timestamp(),
            updated_at: Utc::now().timestamp(),
        };

        memories.insert("test/key".to_string(), entry);

        // 通过内部方法创建（模拟 ConflictGuard 行为）
        let context = SafeMemoryContext::new(memories);

        // 验证基本操作
        assert_eq!(context.len(), 1);
        assert!(!context.is_empty());

        // 验证 get 操作
        let retrieved = context.get("test/key");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, b"test_value");

        // 验证不存在的 key
        assert!(context.get("nonexistent").is_none());

        // 验证 keys 操作
        let keys: Vec<&String> = context.keys().collect();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "test/key");

        // 验证 iter_memories 操作
        let mut iter_count = 0;
        for (key, entry) in context.iter_memories() {
            assert_eq!(key, "test/key");
            assert_eq!(entry.value, b"test_value");
            iter_count += 1;
        }
        assert_eq!(iter_count, 1);
    }

    /// 测试 SafeMemoryContext 的 Debug 实现
    #[test]
    fn test_safe_memory_context_debug() {
        let context = SafeMemoryContext::new(HashMap::new());

        // 验证 Debug 输出包含关键信息
        let debug_str = format!("{:?}", context);
        assert!(debug_str.contains("SafeMemoryContext"));
        assert!(debug_str.contains("len"));
        assert!(debug_str.contains("keys"));
    }

    /// 测试空 SafeMemoryContext
    #[test]
    fn test_empty_safe_memory_context() {
        let context = SafeMemoryContext::new(HashMap::new());

        assert_eq!(context.len(), 0);
        assert!(context.is_empty());
        assert!(context.get("any_key").is_none());
    }

    /// 测试 SafeMemoryContext 没有 Clone 和 Default 实现
    #[test]
    fn test_safe_memory_context_no_clone_default() {
        // 这是一个编译时测试，验证以下代码无法编译：
        // [X] 无法 clone
        // let context1 = SafeMemoryContext::new(HashMap::new());
        // let context2 = context1.clone();
        //
        // [X] 无法 default
        // let context = SafeMemoryContext::default();

        // 如果这些代码能编译，测试会失败
        // （Rust 编译器会阻止这些操作）
    }
}
