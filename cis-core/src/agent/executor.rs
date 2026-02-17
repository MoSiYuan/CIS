//! # Agent Executor (单个任务执行)
//!
//! **强制 SafeMemoryContext** (P1.7.0 任务组 0.3)
//!
//! # 核心机制
//!
//! - **编译时强制**：`execute()` 只接受 `SafeMemoryContext`
//! - **无绕过路径**：无法传递未检查的记忆
//!
//! # 无绕过路径
//!
//! ```text
//! Agent 执行任务
//!     ↓
//! 需要 SafeMemoryContext
//!     ↓
//! 只能由 ConflictGuard::check_and_create_context() 创建
//!     ↓
//! [OK] 强制执行，无绕过路径
//! ```

use crate::error::{CisError, Result};
use crate::memory::guard::types::SafeMemoryContext;
use crate::types::Task;

/// Agent Executor (单个任务执行）
///
/// # 核心职责
///
/// 执行单个 Agent 任务，强制要求 SafeMemoryContext。
///
/// # 编译时保证
///
/// `execute()` 方法只接受 `SafeMemoryContext`，这是编译时强制：
/// - 外部代码无法直接构造 `SafeMemoryContext`（new() 是私有的）
/// - 必须通过 `ConflictGuard::check_and_create_context()` 创建
/// - 确保在 Agent 执行前检测冲突
pub struct AgentExecutor {
    // 当前没有字段（纯函数式结构）
}

impl AgentExecutor {
    /// 执行 Agent 任务（强制要求 SafeMemoryContext）
    ///
    /// # 编译时保证
    ///
    /// - **强制参数**：`memory: SafeMemoryContext`
    /// - **编译时检查**：只有通过冲突检查才能创建 SafeMemoryContext
    /// - **无绕过路径**：外部代码无法直接创建 SafeMemoryContext
    ///
    /// # 参数
    ///
    /// - `task`: 要执行的任务
    /// - `memory`: 已通过冲突检查的记忆上下文
    ///
    /// # 返回
    ///
    /// 返回 `AgentResult`（执行结果）
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let executor = AgentExecutor;
    ///
    /// let task = Task {
    ///     id: "task-123".to_string(),
    ///     title: "Test task".to_string(),
    ///     // ...
    /// };
    ///
    /// let guard = ConflictGuard::new(memory_service);
    ///
    /// // 强制检测冲突后才能执行
    /// let memory = guard.check_and_create_context(&["key1", "key2"]).await?;
    ///
    /// let result = executor.execute(task, memory).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 错误示例
    ///
    /// [X] 以下代码无法编译（绕过路径）：
    /// ```rust,compile_fail
    /// let memories = std::collections::HashMap::new();
    /// let memory = SafeMemoryContext::new(memories);  // ← 编译错误
    /// executor.execute(task, memory).await?;
    /// ```
    pub async fn execute(
        &self,
        task: Task,
        memory: SafeMemoryContext,  // ← 编译时强制
    ) -> Result<AgentResult> {
        println!("[INFO] Executing task: {}", task.id);

        // TODO: 实际执行 Agent 任务
        // 1. 下发记忆给 Agent
        for (key, entry) in memory.iter_memories() {
            println!("[DEBUG] Delivering memory: {} = {}", key, String::from_utf8_lossy(&entry.value));
        }

        // 2. 模拟执行结果
        let result = AgentResult {
            task_id: task.id.clone(),
            exit_code: 0,
            success: true,
            output: format!("Task {} completed", task.id),
        };

        println!("[INFO] Task {} completed", task.id);

        Ok(result)
    }

    /// 检查键是否冲突
    ///
    /// # 参数
    ///
    /// - `key`: 要检查的记忆键
    ///
    /// # 返回
    ///
    /// 返回 `true` 如果键有未解决的冲突。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let executor = AgentExecutor;
    ///
    /// if executor.is_key_conflicted("project/config").await? {
    ///     println!("Key has unresolved conflicts, cannot execute");
    /// } else {
    ///     println!("Key has no conflicts, can execute");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 实现说明
    ///
    /// TODO: 实现实际的冲突检查逻辑
    /// - 使用 `ConflictGuard::get_unresolved_conflicts_for_keys()`
    /// - 检查返回的 HashMap 是否为空
    pub async fn is_key_conflicted(&self, key: &str) -> Result<bool> {
        // TODO: 实现冲突检查
        println!("[DEBUG] Checking if key '{}' is conflicted", key);

        // 临时实现：假设无冲突
        Ok(false)
    }
}

/// Agent 执行结果
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// 任务 ID
    pub task_id: String,

    /// 退出码
    pub exit_code: i32,

    /// 是否成功
    pub success: bool,

    /// 输出
    pub output: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 execute 方法
    #[test]
    fn test_execute() {
        // TODO: 实现
    }

    /// 测试 is_key_conflicted 方法
    #[test]
    fn test_is_key_conflicted() {
        // TODO: 实现
    }
}
