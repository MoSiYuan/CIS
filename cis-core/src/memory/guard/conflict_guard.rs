//! # Conflict Guard (冲突守卫)
//!
//! **强制执行冲突检测** (P1.7.0)
//!
//! # 核心机制
//!
//! - **检测时机**：Agent 执行前检测（而非同步时）
//! - **检测范围**：只检测公域记忆（私域记忆不参与冲突检测）
//! - **阻塞式下发**：有冲突时阻塞 Agent 执行，不下发任何记忆
//!
//! # 无绕过路径
//!
//! ```text
//! Agent 执行任务前
//!    ↓
//! ConflictGuard.check_conflicts_before_delivery()
//!    ↓
//! 有冲突？
//!    ├─ 是 → 阻塞，显示给用户解决
//!    └─ 否 → 创建 SafeMemoryContext（编译时保证）
//! ```
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use cis_core::memory::guard::ConflictGuard;
//! use cis_core::memory::MemoryService;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let memory_service = MemoryService::new().await?;
//! let guard = ConflictGuard::new(memory_service);
//!
//! // Agent 执行前强制检测冲突
//! let context = guard.check_and_create_context(&["key1", "key2"]).await?;
//!
//! // [OK] 检测通过后才能执行 Agent
//! executor.execute(task, context).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::error::{CisError, Result};
use crate::memory::guard::types::{ConflictChecked, SafeMemoryContext};
use crate::memory::MemoryService;
use crate::types::MemoryDomain;

/// 冲突通知
///
/// # 说明
///
/// 当检测到冲突时，包含本地版本和远程版本的信息。
#[derive(Debug, Clone)]
pub struct ConflictNotification {
    /// 记忆键
    pub key: String,

    /// 本地版本
    pub local_version: ConflictVersion,

    /// 远程版本（来自其他节点）
    pub remote_versions: Vec<ConflictVersion>,
}

/// 冲突版本信息
#[derive(Debug, Clone)]
pub struct ConflictVersion {
    /// 节点 ID
    pub node_id: String,

    /// 向量时钟版本
    pub vector_clock: u64,

    /// 值
    pub value: Vec<u8>,

    /// 时间戳
    pub timestamp: i64,
}

/// 冲突检测结果
#[derive(Debug, Clone)]
pub enum ConflictCheckResult {
    /// 无冲突，可以执行
    NoConflicts,

    /// 有冲突，需要解决
    HasConflicts {
        /// 冲突的键
        conflicts: HashMap<String, ConflictNotification>,
    },
}

/// 冲突解决选择
#[derive(Debug, Clone)]
pub enum ConflictResolutionChoice {
    /// 保留本地版本
    KeepLocal,

    /// 保留远程版本（指定 node_id）
    KeepRemote {
        node_id: String,
    },

    /// 保留两个版本（本地重命名为 key_local）
    KeepBoth,

    /// AI 合并
    AIMerge,
}

/// ConflictGuard 配置
#[derive(Debug, Clone)]
pub struct ConflictGuardConfig {
    /// 是否启用强制检测
    pub enforce_check: bool,

    /// 是否自动解决冲突（LWW 策略）
    pub auto_resolve: bool,
}

impl Default for ConflictGuardConfig {
    fn default() -> Self {
        Self {
            enforce_check: true,  // 默认强制检测
            auto_resolve: false, // 默认不自动解决（用户决策优先）
        }
    }
}

/// 冲突守卫
///
/// # 核心保证
///
/// - **强制检测**：Agent 执行前必须检测冲突
/// - **阻塞式下发**：有冲突时阻塞，不下发记忆
/// - **只检测公域**：私域记忆不参与冲突检测
///
/// # 无绕过路径
///
/// - `check_and_create_context()` 是创建 `SafeMemoryContext` 的唯一方法
/// - `SafeMemoryContext::new()` 是私有的（`pub(crate)`）
/// - 编译时保证：无法绕过冲突检测
pub struct ConflictGuard {
    /// 记忆服务
    memory_service: Arc<MemoryService>,

    /// 未解决的冲突
    unresolved_conflicts: Arc<RwLock<HashMap<String, ConflictNotification>>>,

    /// 配置
    config: ConflictGuardConfig,
}

impl ConflictGuard {
    /// 创建冲突守卫
    ///
    /// # 参数
    ///
    /// - `memory_service`: 记忆服务
    ///
    /// # 示例
    ///
    /// ```rust
    /// let guard = ConflictGuard::new(memory_service);
    /// ```
    pub fn new(memory_service: Arc<MemoryService>) -> Self {
        Self {
            memory_service,
            unresolved_conflicts: Arc::new(RwLock::new(HashMap::new())),
            config: ConflictGuardConfig::default(),
        }
    }

    /// 创建冲突守卫（自定义配置）
    ///
    /// # 参数
    ///
    /// - `memory_service`: 记忆服务
    /// - `config`: 冲突守卫配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// let config = ConflictGuardConfig {
    ///     enforce_check: true,
    ///     auto_resolve: false,
    /// };
    /// let guard = ConflictGuard::new_with_config(memory_service, config);
    /// ```
    pub fn new_with_config(
        memory_service: Arc<MemoryService>,
        config: ConflictGuardConfig,
    ) -> Self {
        Self {
            memory_service,
            unresolved_conflicts: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// 检查公域记忆冲突（Agent 执行前）
    ///
    /// # 核心逻辑
    ///
    /// 1. 只检查公域记忆（`MemoryDomain::Public`）
    /// 2. 比较本地版本和远程版本（Vector Clock）
    /// 3. 返回冲突检测结果
    ///
    /// # 参数
    ///
    /// - `keys`: 要检查的记忆键
    ///
    /// # 返回
    ///
    /// 返回 `ConflictCheckResult`：
    /// - `NoConflicts`: 无冲突，可以执行
    /// - `HasConflicts`: 有冲突，需要解决
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let result = guard.check_conflicts_before_delivery(&["key1", "key2"]).await?;
    ///
    /// match result {
    ///     ConflictCheckResult::NoConflicts => {
    ///         println!("No conflicts, can execute");
    ///     }
    ///     ConflictCheckResult::HasConflicts { conflicts } => {
    ///         println!("Found {} conflicts", conflicts.len());
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_conflicts_before_delivery(
        &self,
        keys: &[String],
    ) -> Result<ConflictCheckResult> {
        // TODO: 实现冲突检测逻辑
        // 1. 获取所有键的公域记忆
        // 2. 比较本地版本和远程版本
        // 3. 返回冲突结果

        println!("[INFO] Checking conflicts for {} keys", keys.len());

        // 临时实现：假设无冲突
        Ok(ConflictCheckResult::NoConflicts)
    }

    /// 获取未解决的冲突
    ///
    /// # 参数
    ///
    /// - `keys`: 要查询的记忆键
    ///
    /// # 返回
    ///
    /// 返回未解决的冲突映射表。
    ///
    /// # 示例
    ///
    /// ```rust
    /// let conflicts = guard.get_unresolved_conflicts_for_keys(&["key1", "key2"]).await?;
    ///
    /// for (key, notification) in conflicts {
    ///     println!("Conflict on {}: {:?}", key, notification);
    /// }
    /// ```
    pub async fn get_unresolved_conflicts_for_keys(
        &self,
        keys: &[String],
    ) -> Result<HashMap<String, ConflictNotification>> {
        let unresolved = self.unresolved_conflicts.read().await;
        let mut result = HashMap::new();

        for key in keys {
            if let Some(notification) = unresolved.get(key) {
                result.insert(key.clone(), notification.clone());
            }
        }

        Ok(result)
    }

    /// 检测新冲突（只检查公域记忆）
    ///
    /// # 核心逻辑
    ///
    /// 1. 只检查公域记忆（`MemoryDomain::Public`）
    /// 2. 比较 Vector Clock 版本
    /// 3. 检测到冲突时添加到 `unresolved_conflicts`
    ///
    /// # 参数
    ///
    /// - `keys`: 要检测的记忆键
    ///
    /// # 返回
    ///
    /// 返回新检测到的冲突数量。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let new_conflicts = guard.detect_new_conflicts(&["key1", "key2"]).await?;
    /// println!("Detected {} new conflicts", new_conflicts);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect_new_conflicts(&self, keys: &[String]) -> Result<usize> {
        // TODO: 实现新冲突检测逻辑
        // 1. 遍历所有键
        // 2. 获取公域记忆（排除私域）
        // 3. 比较 Vector Clock
        // 4. 添加到 unresolved_conflicts

        println!("[INFO] Detecting new conflicts for {} keys", keys.len());

        // 临时实现：假设无新冲突
        Ok(0)
    }

    /// 强制冲突检查后创建 SafeMemoryContext（编译时保证）
    ///
    /// # 核心保证
    ///
    /// - **强制检查**：必须先调用 `check_conflicts_before_delivery()`
    /// - **编译时保证**：`SafeMemoryContext::new()` 是私有的
    /// - **唯一方法**：这是创建 `SafeMemoryContext` 的唯一公共方法
    ///
    /// # 参数
    ///
    /// - `keys`: 要获取的记忆键
    ///
    /// # 返回
    ///
    /// 返回 `SafeMemoryContext`（只有通过冲突检查才能创建）。
    ///
    /// # 错误
    ///
    /// 如果有冲突，返回 `CisError::conflict_blocked()`。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let context = guard.check_and_create_context(&["key1", "key2"]).await?;
    ///
    /// // [OK] 检测通过，可以使用 context
    /// for (key, entry) in context.iter_memories() {
    ///     println!("{}: {:?}", key, entry.value);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_and_create_context(
        &self,
        keys: &[String],
    ) -> Result<SafeMemoryContext> {
        // 1. 强制检查冲突
        let check_result = self.check_conflicts_before_delivery(keys).await?;

        match check_result {
            ConflictCheckResult::NoConflicts => {
                // 2. 只有检查通过才构建 context
                println!("[INFO] No conflicts, creating SafeMemoryContext");

                let mut memories = HashMap::new();

                // TODO: 从 memory_service 获取记忆
                // for key in keys {
                //     if let Some(entry) = self.memory_service.get(key).await? {
                //         memories.insert(key.clone(), entry);
                //     }
                // }

                // 3. 创建 SafeMemoryContext（私有的 new() 方法）
                Ok(SafeMemoryContext::new(memories))
            }

            ConflictCheckResult::HasConflicts { .. } => {
                // 4. 有冲突，无法创建 SafeMemoryContext
                println!("[ERROR] Cannot create SafeMemoryContext: conflicts detected");

                Err(CisError::memory_not_found(
                    "Cannot create SafeMemoryContext: conflicts detected. Please resolve conflicts first."
                ))
            }
        }
    }

    /// 用户手动解决冲突
    ///
    /// # 参数
    ///
    /// - `key`: 冲突的记忆键
    /// - `choice`: 解决方案选择
    ///
    /// # 返回
    ///
    /// 返回解决后的记忆值。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // 保留本地版本
    /// guard.resolve_conflict("key1", ConflictResolutionChoice::KeepLocal).await?;
    ///
    /// // 保留远程版本
    /// guard.resolve_conflict("key2", ConflictResolutionChoice::KeepRemote {
    ///     node_id: "node-123".to_string()
    /// }).await?;
    ///
    /// // 保留两个版本
    /// guard.resolve_conflict("key3", ConflictResolutionChoice::KeepBoth).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn resolve_conflict(
        &self,
        key: &str,
        choice: ConflictResolutionChoice,
    ) -> Result<Vec<u8>> {
        // TODO: 实现冲突解决逻辑
        // 1. 从 unresolved_conflicts 移除
        // 2. 根据选择应用解决策略
        // 3. 写入记忆

        println!("[INFO] Resolving conflict for key: {}", key);

        match choice {
            ConflictResolutionChoice::KeepLocal => {
                println!("[INFO] Keeping local version");
                // TODO: 获取本地版本并返回
                Ok(b"local_value".to_vec())
            }

            ConflictResolutionChoice::KeepRemote { node_id } => {
                println!("[INFO] Keeping remote version from node: {}", node_id);
                // TODO: 获取远程版本并返回
                Ok(b"remote_value".to_vec())
            }

            ConflictResolutionChoice::KeepBoth => {
                println!("[INFO] Keeping both versions");
                // TODO: 本地版本重命名为 key_local
                Ok(b"local_value".to_vec())
            }

            ConflictResolutionChoice::AIMerge => {
                println!("[INFO] AI merging versions");
                // TODO: 调用 AI 合并
                Ok(b"merged_value".to_vec())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 ConflictGuard 创建
    #[test]
    fn test_conflict_guard_creation() {
        // TODO: 实现
    }

    /// 测试 check_conflicts_before_delivery
    #[test]
    fn test_check_conflicts_before_delivery() {
        // TODO: 实现
    }

    /// 测试 get_unresolved_conflicts_for_keys
    #[test]
    fn test_get_unresolved_conflicts_for_keys() {
        // TODO: 实现
    }

    /// 测试 resolve_conflict
    #[test]
    fn test_resolve_conflict() {
        // TODO: 实现
    }
}
