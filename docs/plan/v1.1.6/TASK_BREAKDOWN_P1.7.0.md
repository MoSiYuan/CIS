# CIS v1.1.7 Phase 0 开发任务拆分

> **版本**: v1.1.7 Phase 0
> **创建日期**: 2026-02-14
> **状态**: 🚧 准备开始开发
> **优先级**: 🔴 P0 (最高优先级)

---

## 任务概览

Phase 0: **冲突检测前置 (强制执行保障)**

**总任务数**: 47 个子任务
**预计复杂度**: 高 (涉及类型系统、API 设计、测试、配置)
**关键约束**: 必须确保没有任何绕过路径

---

## 任务组 0.1: 类型系统强制 (编译时保证)

**目标**: 只有通过冲突检查的 `SafeMemoryContext` 才能传给 Agent

**文件**: `cis-core/src/memory/guard/types.rs` (新建)

### 0.1.1 创建标记类型

```rust
// cis-core/src/memory/guard/types.rs

use std::marker::PhantomData;

/// 🔥 冲突已检查的标记（编译时保证）
///
/// 用于在类型系统中标记 MemoryContext 已经通过了冲突检查
pub struct ConflictChecked;
```

**验收标准**:
- [ ] 编译通过
- [ ] 只有 PhantomData，无其他字段
- [ ] 文档注释完整

---

### 0.1.2 创建 SafeMemoryContext

```rust
// cis-core/src/memory/guard/types.rs

/// 🔥 只有通过冲突检查才能创建的 Memory Context
///
/// 编译时保证：只有 ConflictGuard 能创建此类型
/// 用户代码无法直接构造（避免绕过冲突检测）
pub struct SafeMemoryContext {
    _phantom: PhantomData<ConflictChecked>,
    pub(crate) memories: HashMap<String, MemoryEntry>,
}

impl SafeMemoryContext {
    /// 🔥 私有构造函数，只有 ConflictGuard 能创建
    pub(crate) fn new(memories: HashMap<String, MemoryEntry>) -> Self {
        Self {
            _phantom: PhantomData,
            memories,
        }
    }

    /// 获取记忆条目
    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.memories.get(key)
    }

    /// 迭代所有记忆
    pub fn iter(&self) -> impl Iterator<Item = (&String, &MemoryEntry)> {
        self.memories.iter()
    }

    /// 获取记忆数量
    pub fn len(&self) -> usize {
        self.memories.len()
    }
}
```

**验收标准**:
- [ ] `new()` 函数是 `pub(crate)` (外部无法调用)
- [ ] `_phantom` 字段是私有的
- [ ] 提供安全的查询 API (`get`, `iter`, `len`)
- [ ] 包含完整的文档注释

---

### 0.1.3 单元测试：SafeMemoryContext 无法直接创建

```rust
// cis-core/src/memory/guard/types_tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_memory_context_cannot_be_created_directly() {
        // ❌ 编译错误：SafeMemoryContext::new 是私有的
        // let context = SafeMemoryContext::new(HashMap::new());
        //   ^^^ 编译错误：字段私有

        // ✅ 只能通过 ConflictGuard 创建
        // （在后续 ConflictGuard 任务中测试）
    }
}
```

**验收标准**:
- [ ] 注释掉的代码编译失败（证明类型系统有效）
- [ ] 测试通过 `cargo test`

---

## 任务组 0.2: ConflictGuard 实现 (核心逻辑)

**目标**: 实现冲突检测和 SafeMemoryContext 创建

**文件**: `cis-core/src/memory/guard/conflict_guard.rs` (新建)

### 0.2.1 定义 ConflictGuardConfig

```rust
// cis-core/src/memory/guard/conflict_guard.rs

use crate::types::{Result, CisError};

pub struct ConflictGuardConfig {
    /// 🔥 Agent 执行前是否强制检查冲突（必须为 true，不可配置）
    /// 注意：这个字段不允许修改，始终为 true
    pub enforce_check: bool,  // 强制为 true，编译时断言

    /// 冲突超时时间（秒）
    pub conflict_timeout_secs: u64,

    /// 默认冲突解决策略
    pub default_resolution: ConflictResolutionStrategy,
}

impl ConflictGuardConfig {
    /// 创建默认配置（强制检查冲突）
    pub fn default() -> Self {
        Self {
            enforce_check: true,  // 🔥 强制为 true，不可修改
            conflict_timeout_secs: 300,
            default_resolution: ConflictResolutionStrategy::WaitForUser,
        }
    }

    /// 🔥 禁止创建非强制检查的配置（编译时断言）
    pub fn with_enforce_check(mut self, enforce: bool) -> Self {
        // 编译时断言：enforce 必须为 true
        assert!(enforce, "ConflictGuardConfig.enforce_check MUST be true. No bypass path allowed!");
        self.enforce_check = true;
        self
    }
}

pub enum ConflictResolutionStrategy {
    /// 等待用户手动解决
    WaitForUser,

    /// 自动使用本地版本
    AutoKeepLocal,

    /// 自动使用远程版本
    AutoKeepRemote,
}
```

**验收标准**:
- [ ] `enforce_check` 默认为 `true`
- [ ] `with_enforce_check()` 包含 `assert!` 确保参数为 `true`
- [ ] `ConflictResolutionStrategy` 枚举定义完整

---

### 0.2.2 定义 ConflictNotification

```rust
// cis-core/src/memory/guard/conflict_guard.rs

use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct ConflictNotification {
    pub conflict_id: String,
    pub key: String,
    pub local_version: MemoryVersion,
    pub remote_version: MemoryVersion,
    pub detected_at: DateTime<Utc>,
}

pub struct MemoryVersion {
    pub value: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub vector_clock: Vec<u8>,  // 序列化的 Vector Clock
}
```

**验收标准**:
- [ ] 所有字段定义完整
- [ ] 使用 `chrono` 和 `uuid` crate
- [ ] 添加 `Serialize`/`Deserialize` (如果需要持久化)

---

### 0.2.3 定义 ConflictCheckResult

```rust
// cis-core/src/memory/guard/conflict_guard.rs

pub enum ConflictCheckResult {
    NoConflicts,
    HasConflicts {
        conflicts: Vec<ConflictNotification>,
        required_action: RequiredAction,
    },
}

pub enum RequiredAction {
    BlockAndNotify,  // 阻塞并通知用户
}
```

**验收标准**:
- [ ] 枚举变体定义清晰
- [ ] `HasConflicts` 包含冲突详情和必需操作

---

### 0.2.4 实现 ConflictGuard 结构

```rust
// cis-core/src/memory/guard/conflict_guard.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::memory::{MemoryService, MemoryEntry};
use crate::types::Result;

pub struct ConflictGuard {
    memory_service: Arc<MemoryService>,
    unresolved_conflicts: Arc<RwLock<HashMap<String, ConflictNotification>>>,
    config: ConflictGuardConfig,
}

impl ConflictGuard {
    pub fn new(
        memory_service: Arc<MemoryService>,
        config: ConflictGuardConfig,
    ) -> Self {
        Self {
            memory_service,
            unresolved_conflicts: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
}
```

**验收标准**:
- [ ] 结构定义完整
- [ ] 使用 `Arc<RwLock<>>` 保证线程安全
- [ ] `new()` 函数公开

---

### 0.2.5 实现 check_conflicts_before_delivery

```rust
// cis-core/src/memory/guard/conflict_guard.rs

impl ConflictGuard {
    /// 检查公域记忆冲突（Agent 执行前调用）
    pub async fn check_conflicts_before_delivery(
        &self,
        keys: &[String],  // Agent 需要的记忆键
    ) -> Result<ConflictCheckResult> {
        // 1. 检查是否有未解决的冲突
        let conflicts = self.get_unresolved_conflicts_for_keys(keys).await?;

        if !conflicts.is_empty() {
            tracing::warn!(
                "Found {} unresolved conflicts before agent delivery",
                conflicts.len()
            );

            return Ok(ConflictCheckResult::HasConflicts {
                conflicts,
                required_action: RequiredAction::BlockAndNotify,
            });
        }

        // 2. 检查是否有新的潜在冲突
        let new_conflicts = self.detect_new_conflicts(keys).await?;

        if !new_conflicts.is_empty() {
            tracing::warn!(
                "Detected {} new conflicts for keys: {:?}",
                new_conflicts.len(),
                keys
            );

            // 记录新冲突
            for conflict in new_conflicts {
                self.unresolved_conflicts.write().await.insert(
                    conflict.conflict_id.clone(),
                    conflict
                );
            }

            return Ok(ConflictCheckResult::HasConflicts {
                conflicts: new_conflicts,
                required_action: RequiredAction::BlockAndNotify,
            });
        }

        // 3. 无冲突，可以下发
        Ok(ConflictCheckResult::NoConflicts)
    }
}
```

**验收标准**:
- [ ] 返回类型正确 (`Result<ConflictCheckResult>`)
- [ ] 记录日志使用 `tracing::warn`
- [ ] 检查顺序：未解决冲突 → 新冲突 → 无冲突

---

### 0.2.6 实现 get_unresolved_conflicts_for_keys

```rust
// cis-core/src/memory/guard/conflict_guard.rs

impl ConflictGuard {
    /// 获取指定键的未解决冲突
    async fn get_unresolved_conflicts_for_keys(
        &self,
        keys: &[String],
    ) -> Result<Vec<ConflictNotification>> {
        let all_conflicts = self.unresolved_conflicts.read().await;

        let conflicts: Vec<ConflictNotification> = keys.iter()
            .filter_map(|key| all_conflicts.get(key).cloned())
            .collect();

        Ok(conflicts)
    }
}
```

**验收标准**:
- [ ] 使用 `read().await` 访问 `unresolved_conflicts`
- [ ] 正确过滤指定键
- [ ] 返回 `Vec<ConflictNotification>`

---

### 0.2.7 实现 detect_new_conflicts (基于公域记忆)

```rust
// cis-core/src/memory/guard/conflict_guard.rs

impl ConflictGuard {
    /// 检测新的冲突（基于公域记忆）
    async fn detect_new_conflicts(&self, keys: &[String]) -> Result<Vec<ConflictNotification>> {
        let mut new_conflicts = Vec::new();

        for key in keys {
            // ✅ 只检查公域记忆
            let public_entry = self.memory_service.get_public(key).await?;

            if let Some(entry) = public_entry {
                // 检查是否有时间戳接近的多个版本（并发编辑迹象）
                let versions = self.get_all_versions(key).await?;

                if versions.len() > 1 {
                    // 检查时间差
                    let timestamps: Vec<_> = versions.iter()
                        .map(|v| v.timestamp)
                        .collect();

                    for (i, ts1) in timestamps.iter().enumerate() {
                        for ts2 in timestamps.iter().skip(i + 1) {
                            let diff = ts1.signed_duration_since(*ts2).num_seconds().abs();

                            if diff < 5 {
                                // 5秒内的多个版本 = 可能的冲突
                                let conflict = ConflictNotification {
                                    conflict_id: Uuid::new_v4().to_string(),
                                    key: key.clone(),
                                    local_version: versions[0].clone(),
                                    remote_version: versions[1].clone(),
                                    detected_at: Utc::now(),
                                };

                                new_conflicts.push(conflict);
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(new_conflicts)
    }

    /// 获取公域记忆的所有版本
    async fn get_all_versions(&self, key: &str) -> Result<Vec<MemoryVersion>> {
        // 查询公域记忆的所有版本（包括来自不同节点的版本）
        let versions = self.memory_service.get_public_versions(key).await?;
        Ok(versions)
    }
}
```

**验收标准**:
- [ ] 只调用 `get_public()` (不检查私域记忆)
- [ ] 5秒阈值定义为常量 `CONFLICT_WINDOW_SECS = 5`
- [ ] 使用 `Uuid::new_v4()` 生成唯一 ID

---

### 0.2.8 实现 check_and_create_context

```rust
// cis-core/src/memory/guard/conflict_guard.rs

use crate::memory::guard::types::SafeMemoryContext;

impl ConflictGuard {
    /// 🔥 强制冲突检查后创建 SafeMemoryContext（编译时保证）
    pub async fn check_and_create_context(
        &self,
        keys: &[String],
    ) -> Result<SafeMemoryContext> {
        // 1. 强制检查冲突
        let check_result = self.check_conflicts_before_delivery(keys).await?;

        match check_result {
            ConflictCheckResult::NoConflicts => {
                // 2. 只有检查通过才构建 context
                let mut memories = HashMap::new();

                for key in keys {
                    if let Some(entry) = self.memory_service.get(key).await? {
                        memories.insert(key.clone(), entry);
                    }
                }

                // 3. 🔥 创建 SafeMemoryContext（证明已检查冲突）
                Ok(SafeMemoryContext::new(memories))
            }

            ConflictCheckResult::HasConflicts { .. } => {
                // 4. 有冲突，无法创建 SafeMemoryContext
                Err(CisError::conflict_blocked(
                    "Cannot create SafeMemoryContext: conflicts detected"
                ))
            }
        }
    }
}
```

**验收标准**:
- [ ] 返回类型为 `Result<SafeMemoryContext>`
- [ ] 只有 `NoConflicts` 才创建 context
- [ ] `HasConflicts` 返回错误 `CisError::conflict_blocked`

---

### 0.2.9 实现 resolve_conflict

```rust
// cis-core/src/memory/guard/conflict_guard.rs

pub enum ConflictResolutionChoice {
    KeepLocal,
    KeepRemote,
    KeepBoth,
    AIMerge,
}

impl ConflictGuard {
    /// 用户手动解决冲突
    pub async fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: ConflictResolutionChoice,
    ) -> Result<()> {
        let conflict = self.unresolved_conflicts.read().await.get(conflict_id).cloned()
            .ok_or_else(|| CisError::not_found("Conflict not found"))?;

        match resolution {
            ConflictResolutionChoice::KeepLocal => {
                // 保留本地版本，删除冲突记录
                self.unresolved_conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::KeepRemote => {
                // 应用远程版本
                self.memory_service.set(
                    &conflict.key,
                    &conflict.remote_version.value,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.unresolved_conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::KeepBoth => {
                // 保留两个版本（重命名远程版本）
                let new_key = format!("{}_conflict_{}", conflict.key, conflict.conflict_id);
                self.memory_service.set(
                    &new_key,
                    &conflict.remote_version.value,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.unresolved_conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::AIMerge => {
                // AI 合并两个版本
                let merged = self.memory_service.ai_merge(
                    &conflict.local_version.value,
                    &conflict.remote_version.value,
                ).await?;

                self.memory_service.set(
                    &conflict.key,
                    &merged,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.unresolved_conflicts.write().await.remove(conflict_id);
            }
        }

        Ok(())
    }
}
```

**验收标准**:
- [ ] 4种解决策略都实现
- [ ] 解决后从 `unresolved_conflicts` 删除记录
- [ ] `KeepRemote`, `KeepBoth`, `AIMerge` 都调用 `memory_service.set()`

---

## 任务组 0.3: AgentExecutor 集成 (强制 SafeMemoryContext)

**目标**: 修改 Agent 执行 API，强制要求 SafeMemoryContext

**文件**: `cis-core/src/agent/executor.rs` (修改)

### 0.3.1 修改 execute 函数签名

```rust
// cis-core/src/agent/executor.rs

use crate::memory::guard::types::SafeMemoryContext;

impl AgentExecutor {
    /// 🔥 执行 Agent 任务（强制要求 SafeMemoryContext）
    ///
    /// 编译时保证：只有通过冲突检查的 SafeMemoryContext 才能传入
    pub async fn execute(
        &self,
        task: AgentTask,
        memory: SafeMemoryContext,  // ← 🔥 编译时强制，无法绕过
    ) -> Result<AgentResult> {
        // Agent 执行逻辑
        // ❌ 无法绕过冲突检测，因为 SafeMemoryContext 只能通过 ConflictGuard::check_and_create_context 创建

        // 示例：下发记忆给 Agent
        for (key, entry) in memory.iter() {
            tracing::debug!("Delivering memory to agent: {} = {}", key, entry.key);
        }

        self.agent.execute(task, memory).await
    }
}
```

**验收标准**:
- [ ] 函数签名接受 `SafeMemoryContext`
- [ ] 文档注释说明编译时保证
- [ ] 注释说明"❌ 无法绕过冲突检测"

---

### 0.3.2 删除不安全的 API（如果存在）

```rust
// cis-core/src/agent/executor.rs

impl AgentExecutor {
    /// ❌ 删除不安全的 API（不允许绕过冲突检测）
    ///
    /// 以下 API 已废弃，编译时会报错：
    /// pub async fn execute_unsafe(
    ///     &self,
    ///     task: AgentTask,
    ///     memory: HashMap<String, MemoryEntry>,  // ← ❌ 不允许
    /// ) -> Result<AgentResult>
}
```

**验收标准**:
- [ ] 搜索代码中是否有 `execute_unsafe` 类似函数
- [ ] 如果存在，删除并添加编译错误 `#[deprecated]`
- [ ] 确保没有其他绕过路径

---

### 0.3.3 添加 is_key_conflicted 辅助函数

```rust
// cis-core/src/agent/executor.rs

impl AgentExecutor {
    /// 检查键是否冲突（不下发时使用）
    async fn is_key_conflicted(&self, key: &str) -> Result<bool> {
        // 检查是否有未解决的冲突
        let conflicts = self.conflict_guard
            .get_unresolved_conflicts_for_keys(&[key.to_string()])
            .await?;

        Ok(!conflicts.is_empty())
    }
}
```

**验收标准**:
- [ ] 调用 `ConflictGuard::get_unresolved_conflicts_for_keys`
- [ ] 返回 `bool` (true = 有冲突)

---

## 任务组 0.4: Builder 模式强制 (API 层)

**目标**: 提供 Builder API，强制调用冲突检查

**文件**: `cis-core/src/agent/builder.rs` (新建)

### 0.4.1 定义 AgentTaskBuilder

```rust
// cis-core/src/agent/builder.rs

use crate::agent::{AgentExecutor, AgentTask};
use crate::types::Result;

pub struct AgentTaskBuilder<'a> {
    executor: &'a AgentExecutor,
    task: Option<AgentTask>,
    required_keys: Option<Vec<String>>,
    conflict_checked: bool,  // 🔥 标记是否已检查冲突
}

impl<'a> AgentTaskBuilder<'a> {
    pub fn new(executor: &'a AgentExecutor) -> Self {
        Self {
            executor,
            task: None,
            required_keys: None,
            conflict_checked: false,  // 初始为 false
        }
    }

    pub fn with_task(mut self, task: AgentTask) -> Self {
        self.task = Some(task);
        self
    }

    pub fn with_memory_keys(mut self, keys: Vec<String>) -> Self {
        self.required_keys = Some(keys);
        self
    }
}
```

**验收标准**:
- [ ] 结构定义完整
- [ ] `conflict_checked` 初始为 `false`

---

### 0.4.2 实现 check_conflicts 方法

```rust
// cis-core/src/agent/builder.rs

impl<'a> AgentTaskBuilder<'a> {
    /// 🔥 强制冲突检查（不可跳过）
    pub async fn check_conflicts(mut self) -> Result<Self> {
        let keys = self.required_keys.as_ref()
            .ok_or_else(|| CisError::invalid("Memory keys not specified"))?;

        // 强制调用 ConflictGuard 检查
        let check_result = self.executor.conflict_guard
            .check_conflicts_before_delivery(keys)
            .await?;

        match check_result {
            ConflictCheckResult::NoConflicts => {
                self.conflict_checked = true;  // 标记为已检查
                Ok(self)
            }

            ConflictCheckResult::HasConflicts { conflicts, .. } => {
                Err(CisError::conflict_blocked(format!(
                    "{} conflicts detected. Resolve conflicts before executing agent task.",
                    conflicts.len()
                )))
            }
        }
    }
}
```

**验收标准**:
- [ ] 调用 `conflict_guard.check_conflicts_before_delivery()`
- [ ] `NoConflicts` 时设置 `conflict_checked = true`
- [ ] `HasConflicts` 时返回错误并包含冲突数量

---

### 0.4.3 实现 execute 方法（强制要求 conflict_checked）

```rust
// cis-core/src/agent/builder.rs

impl<'a> AgentTaskBuilder<'a> {
    /// 🔥 执行任务（强制要求 conflict_checked == true）
    pub async fn execute(self) -> Result<AgentResult> {
        // 运行时断言（双重保险）
        assert!(self.conflict_checked, "Conflict check is mandatory. No bypass path allowed!");

        let task = self.task.ok_or_else(|| CisError::invalid("Task not specified"))?;

        // 执行任务
        self.executor.agent.execute(task).await
    }
}
```

**验收标准**:
- [ ] `assert!` 确保必须先调用 `check_conflicts()`
- [ ] 断言消息清晰："No bypass path allowed!"

---

## 任务组 0.5: 配置文件强制 (运行时验证)

**目标**: 启动时强制验证 `enforce_check` 为 true

**文件**: `cis-core/src/config/mod.rs` (修改)

### 0.5.1 实现 Config::load 验证

```rust
// cis-core/src/config/mod.rs

impl Config {
    pub fn load() -> Result<Self> {
        let mut config = Self::load_from_file("~/.cis/config.toml")?;

        // 🔥 启动时强制验证
        if config.memory_conflict.enforce_check != true {
            // 强制为 true，忽略配置文件的值
            tracing::warn!(
                "Config error: memory.conflict.enforce_check must be true. Forcing to true."
            );
            config.memory_conflict.enforce_check = true;

            // 或直接拒绝启动（更严格）
            // return Err(CisError::invalid(
            //     "memory.conflict.enforce_check cannot be set to false. \
            //      Conflict check is mandatory and cannot be bypassed."
            // ));
        }

        Ok(config)
    }
}
```

**验收标准**:
- [ ] 检查 `memory_conflict.enforce_check != true`
- [ ] 记录警告日志 `tracing::warn!`
- [ ] 强制设置为 `true`

---

### 0.5.2 定义 MemoryConflictConfig 结构

```rust
// cis-core/src/config/mod.rs

/// 内存冲突配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConflictConfig {
    /// 🔥 Agent 执行前是否强制检查冲突（不可修改）
    pub enforce_check: bool,

    /// 冲突超时时间（秒）
    pub conflict_timeout_secs: u64,
}

impl Default for MemoryConflictConfig {
    fn default() -> Self {
        Self {
            enforce_check: true,  // 🔥 硬编码为 true
            conflict_timeout_secs: 300,
        }
    }
}
```

**验收标准**:
- [ ] `Default` 实现设置 `enforce_check = true`
- [ ] 文档注释说明"不可修改"

---

## 任务组 0.6: 单元测试强制 (CI/CD)

**目标**: 自动检测任何绕过冲突检测的代码路径

**文件**: `cis-core/src/memory/guard/enforcement_tests.rs` (新建)

### 0.6.1 测试无法绕过 SafeMemoryContext

```rust
// cis-core/src/memory/guard/enforcement_tests.rs

#[cfg(test)]
mod enforcement_tests {
    use super::*;

    #[tokio::test]
    async fn test_cannot_bypass_conflict_check() {
        let executor = AgentExecutor::new_test().await;

        // 尝试绕过冲突检测执行任务
        let task = AgentTask::default();

        // ❌ 应该失败：没有 SafeMemoryContext 无法执行
        let result = executor.execute_unsafe(task, HashMap::new()).await;
        assert!(result.is_err(), "Should fail without SafeMemoryContext");

        // ✅ 应该成功：通过 ConflictGuard 检查
        let keys = vec!["project/config".to_string()];
        let context = executor.conflict_guard.check_and_create_context(&keys).await.unwrap();
        let result = executor.execute(task, context).await;
        assert!(result.is_ok(), "Should succeed with SafeMemoryContext");
    }
}
```

**验收标准**:
- [ ] 测试通过 `cargo test`
- [ ] `execute_unsafe` 失败，`execute(SafeMemoryContext)` 成功

---

### 0.6.2 测试 Builder 强制调用 check_conflicts

```rust
// cis-core/src/memory/guard/enforcement_tests.rs

#[tokio::test]
async fn test_builder_requires_conflict_check() {
    let executor = AgentExecutor::new_test().await;
    let task = AgentTask::default();
    let keys = vec!["project/config".to_string()];

    // ❌ 应该 panic：不调用 check_conflicts
    let result = async {
        AgentTaskBuilder::new(&executor)
            .with_task(task)
            .with_memory_keys(keys)
            // .check_conflicts()  // ← 故意不调用
            .execute()
            .await
    }.await;

    assert!(result.is_err(), "Should panic without conflict check");
}
```

**验收标准**:
- [ ] 故意不调用 `check_conflicts()`
- [ ] 断言捕获 `panic` 或返回错误

---

### 0.6.3 测试 SafeMemoryContext 无法直接创建

```rust
// cis-core/src/memory/guard/enforcement_tests.rs

#[tokio::test]
async fn test_safe_memory_context_cannot_be_created_directly() {
    // ❌ 编译错误：SafeMemoryContext::new 是私有的
    // let context = SafeMemoryContext::new(HashMap::new());

    // ✅ 只能通过 ConflictGuard 创建
    let guard = ConflictGuard::new_test();
    let keys = vec!["project/config".to_string()];
    let context = guard.check_and_create_context(&keys).await.unwrap();
    assert!(context.memories.len() > 0);
}
```

**验收标准**:
- [ ] 注释说明编译错误
- [ ] `check_and_create_context` 成功创建 context

---

## 任务组 0.7: 模块导出 (公开 API)

**目标**: 导出所有必要的类型和函数

**文件**: `cis-core/src/memory/guard/mod.rs` (新建)

### 0.7.1 创建模块导出

```rust
// cis-core/src/memory/guard/mod.rs

mod types;
mod conflict_guard;

pub use types::{
    ConflictChecked,
    SafeMemoryContext,
};

pub use conflict_guard::{
    ConflictGuard,
    ConflictGuardConfig,
    ConflictResolutionStrategy,
    ConflictNotification,
    MemoryVersion,
    ConflictCheckResult,
    RequiredAction,
    ConflictResolutionChoice,
};
```

**验收标准**:
- [ ] 所有必要的类型都导出
- [ ] 模块结构清晰（types + conflict_guard）

---

## 任务组 0.8: CLI 命令实现

**目标**: 提供冲突管理的 CLI 接口

**文件**: `cis-node/src/commands/memory_conflicts.rs` (新建)

### 0.8.1 实现 list 命令

```rust
// cis-node/src/commands/memory_conflicts.rs

use clap::{Subcommand, ArgMatches};
use cis_core::memory::guard::ConflictGuard;

pub struct MemoryConflictsListCommand {
    conflict_guard: Arc<ConflictGuard>,
}

impl MemoryConflictsListCommand {
    pub fn new(conflict_guard: Arc<ConflictGuard>) -> Self {
        Self { conflict_guard }
    }

    pub async fn run(&self) -> Result<()> {
        let conflicts = self.conflict_guard.get_all_unresolved().await?;

        if conflicts.is_empty() {
            println!("✅ 没有未解决的冲突");
            return Ok(());
        }

        println!("⚠️  未解决的冲突：\n");

        for (i, conflict) in conflicts.iter().enumerate() {
            println!("{}. 键: {}", i + 1, conflict.key);
            println!("   冲突ID: {}", conflict.conflict_id);
            println!("   本地版本: {} (时间: {})",
                String::from_utf8_lossy(&conflict.local_version.value),
                conflict.local_version.timestamp);
            println!("   远程版本: {} (时间: {})",
                String::from_utf8_lossy(&conflict.remote_version.value),
                conflict.remote_version.timestamp);
            println!();
        }

        println!("共 {} 个未解决冲突", conflicts.len());
        println!();
        println!("解决冲突:");
        println!("  $ cis memory conflicts resolve <id> <choice>");
        println!();
        println!("选择:");
        println!("  1 - 保留本地");
        println!("  2 - 保留远程");
        println!("  3 - 保留两个");
        println!("  4 - AI 合并");

        Ok(())
    }
}
```

**验收标准**:
- [ ] 列出所有冲突详情
- [ ] 提供解决命令示例
- [ ] 无冲突时显示友好消息

---

### 0.8.2 实现 resolve 命令

```rust
// cis-node/src/commands/memory_conflicts.rs

use clap::ArgMatches;
use cis_core::memory::guard::{ConflictGuard, ConflictResolutionChoice};

pub struct MemoryConflictsResolveCommand {
    conflict_guard: Arc<ConflictGuard>,
    conflict_id: String,
    choice: u8,
}

impl MemoryConflictsResolveCommand {
    pub fn new(conflict_guard: Arc<ConflictGuard>, args: &ArgMatches) -> Self {
        Self {
            conflict_guard,
            conflict_id: args.value_of("id").unwrap().to_string(),
            choice: args.value_of("choice").unwrap(),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let choice = match self.choice {
            1 => ConflictResolutionChoice::KeepLocal,
            2 => ConflictResolutionChoice::KeepRemote,
            3 => ConflictResolutionChoice::KeepBoth,
            4 => ConflictResolutionChoice::AIMerge,
            _ => return Err("Invalid choice. Must be 1-4".into()),
        };

        self.conflict_guard.resolve_conflict(&self.conflict_id, choice).await?;

        let choice_name = match choice {
            ConflictResolutionChoice::KeepLocal => "保留本地",
            ConflictResolutionChoice::KeepRemote => "保留远程",
            ConflictResolutionChoice::KeepBoth => "保留两个",
            ConflictResolutionChoice::AIMerge => "AI 合并",
        };

        println!("✅ 已解决冲突: {}", self.conflict_id);
        println!("   选择: {}", choice_name);

        Ok(())
    }
}
```

**验收标准**:
- [ ] 参数解析正确 (`id` 和 `choice`)
- [ ] 调用 `conflict_guard.resolve_conflict()`
- [ ] 显示成功消息

---

### 0.8.3 注册到 CLI 主程序

```rust
// cis-node/src/main.rs

use cis_core::memory::guard::ConflictGuard;

// 在 Args 中添加子命令
SubCommand::MemoryConflicts(sub) => match sub {
    MemoryConflicts::List => {
        let cmd = MemoryConflictsListCommand::new(conflict_guard);
        cmd.run().await?
    }

    MemoryConflicts::Resolve { id, choice } => {
        let cmd = MemoryConflictsResolveCommand::new(conflict_guard, args);
        cmd.run().await?
    }
}
```

**验收标准**:
- [ ] 子命令注册到 clap
- [ ] 使用方法与现有 CLI 命令一致

---

## 任务组 0.9: GUI 组件实现

**目标**: 提供冲突提醒对话框

**文件**: `cis-gui/src/components/conflict_dialog.rs` (新建)

### 0.9.1 定义 ConflictDialog 结构

```rust
// cis-gui/src/components/conflict_dialog.rs

use egui::{self, *};
use cis_core::memory::guard::ConflictNotification;

pub struct ConflictDialog {
    conflicts: Vec<ConflictNotification>,
    selected_resolution: HashMap<String, ConflictResolutionChoice>,
    open: bool,
}

impl ConflictDialog {
    pub fn new(conflicts: Vec<ConflictNotification>) -> Self {
        Self {
            conflicts,
            selected_resolution: HashMap::new(),
            open: false,
        }
    }
}
```

**验收标准**:
- [ ] 结构包含冲突列表和用户选择
- [ ] `open` 控制对话框显示

---

### 0.9.2 实现 show 方法

```rust
// cis-gui/src/components/conflict_dialog.rs

impl ConflictDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut resolved = false;

        egui::Window::new("记忆冲突警告")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading(format!("发现 {} 个未解决的冲突", self.conflicts.len()));
                ui.label("必须先解决冲突才能执行 Agent 任务");

                ui.separator();

                for conflict in &self.conflicts {
                    ui.group(|ui| {
                        ui.heading(&conflict.key);

                        // 本地版本
                        ui.label("本地版本:");
                        ui.label(format!("  值: {}",
                            String::from_utf8_lossy(&conflict.local_version.value)
                        ));
                        ui.label(format!("  时间: {}", conflict.local_version.timestamp));
                        ui.label(format!("  节点: {}", conflict.local_version.node_id));

                        ui.separator();

                        // 远程版本
                        ui.label("远程版本:");
                        ui.label(format!("  值: {}",
                            String::from_utf8_lossy(&conflict.remote_version.value)
                        ));
                        ui.label(format!("  时间: {}", conflict.remote_version.timestamp));
                        ui.label(format!("  节点: {}", conflict.remote_version.node_id));

                        ui.separator();

                        // 解决方案选择
                        let choice = self.selected_resolution
                            .entry(conflict.conflict_id.clone())
                            .or_insert(conflict.conflict_id.clone(), ConflictResolutionChoice::KeepLocal);

                        ui.horizontal(|ui| {
                            ui.label("解决方案:");
                            ui.selectable_value(choice, ConflictResolutionChoice::KeepLocal, "保留本地");
                            ui.selectable_value(choice, ConflictResolutionChoice::KeepRemote, "保留远程");
                            ui.selectable_value(choice, ConflictResolutionChoice::KeepBoth, "保留两个");
                            ui.selectable_value(choice, ConflictResolutionChoice::AIMerge, "AI 合并");
                        });
                    });

                    ui.separator();
                }

                // 底部按钮
                ui.horizontal(|ui| {
                    if ui.button("全部应用").clicked() {
                        // 应用所有解决方案
                        resolved = true;
                    }

                    if ui.button("取消").clicked() {
                        // 取消任务执行
                    }
                });
            });

        resolved
    }

    fn apply_all_resolutions(&self) {
        // 调用 resolve_conflict API
        for (conflict_id, choice) in &self.selected_resolution {
            // TODO: 实现解析
        }
    }
}
```

**验收标准**:
- [ ] 显示所有冲突详情
- [ ] 提供 4 种解决选项
- [ ] "全部应用"和"取消"按钮

---

### 0.9.3 集成到 Agent 执行流程

```rust
// cis-gui/src/screens/agent_execute.rs

use crate::components::conflict_dialog::ConflictDialog;

impl AgentExecuteScreen {
    async fn execute_agent_task(&mut self, task: AgentTask, keys: Vec<String>) {
        // 1. 检查冲突
        let check_result = self.conflict_guard.check_conflicts_before_delivery(&keys).await;

        match check_result {
            ConflictCheckResult::NoConflicts => {
                // 2. 无冲突，继续执行
                let context = self.conflict_guard.check_and_create_context(&keys).await.unwrap();
                self.agent_executor.execute(task, context).await?;
            }

            ConflictCheckResult::HasConflicts { conflicts, .. } => {
                // 3. 有冲突，显示对话框
                let mut dialog = ConflictDialog::new(conflicts);
                let resolved = dialog.show(&self.egui_ctx);

                if resolved {
                    // 用户解决了冲突，重新执行
                    self.execute_agent_task(task, keys).await?;
                } else {
                    // 用户取消，不执行
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}
```

**验收标准**:
- [ ] Agent 执行前检查冲突
- [ ] 有冲突时显示 `ConflictDialog`
- [ ] 用户解决后重新执行

---

## 任务组 0.10: 文档更新

**目标**: 更新代码审查清单和贡献指南

**文件**: `CONTRIBUTING.md` (修改)

### 0.10.1 添加 Agent 记忆下发强制规则

```markdown
<!-- CONTRIBUTING.md -->

## Agent 记忆下发强制规则

### 🔴 绝对禁止

1. **禁止提供绕过冲突检测的 API**
   ```rust
   // ❌ 永远不要添加这样的 API
   pub async fn execute_without_conflict_check(
       &self,
       task: AgentTask,
       memory: HashMap<String, MemoryEntry>,  // 不安全
   ) -> Result<AgentResult>
   ```

2. **禁止创建 `unsafe` 后门函数**
   ```rust
   // ❌ 永远不要添加
   pub unsafe fn execute_bypass_check(...) {
       // "unsafe" 关键字仅用于 FFI，不用于绕过安全检查
   }
   ```

3. **禁止修改 `enforce_check` 配置为 false**
   ```toml
   # ❌ 不要允许用户修改
   [memory.conflict]
   enforce_check = false  # 违反设计原则

   # ✅ 正确：硬编码为 true，或启动时强制验证
   ```

### ✅ 必须遵守

1. **所有 Agent 执行 API 必须接受 `SafeMemoryContext`**
2. **`SafeMemoryContext` 只能通过 `ConflictGuard::check_and_create_context` 创建**
3. **Builder 模式必须强制调用 `check_conflicts` 才能 `execute`**
4. **所有修改必须通过 enforcement 测试**

### 代码审查检查项

在 PR 中，确保：
- [ ] 没有新增绕过冲突检测的 API
- [ ] 所有 `AgentExecutor::execute` 调用都使用 `SafeMemoryContext`
- [ ] 所有测试都通过 `cargo test enforcement_tests`
- [ ] 没有修改 `enforce_check` 配置为 `false`
```

**验收标准**:
- [ ] CONTRIBUTING.md 包含上述章节
- [ ] 检查清单清晰明确
- [ ] 使用正确的代码格式（markdown）

---

## 任务组 0.11: CI/CD 集成

**目标**: 自动运行 enforcement 测试

**文件**: `.github/workflows/test.yml` (修改)

### 0.11.1 添加 enforcement_tests job

```yaml
# .github/workflows/test.yml

name: CIS Tests

on: [push, pull_request]

jobs:
  enforcement-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run enforcement tests
        run: |
          cargo test --package cis-core --lib enforcement_tests::
          # 确保:
          # 1. 所有绕过冲突检测的代码路径都会失败
          # 2. 编译时保证：SafeMemoryContext 无法直接创建
          # 3. 运行时保证：Builder 必须调用 check_conflicts

      - name: Check test results
        if: failure()
        run: |
          echo "Enforcement tests failed! Bypass path detected."
          exit 1
```

**验收标准**:
- [ ] CI 配置文件添加 `enforcement-tests` job
- [ ] 运行 `cargo test enforcement_tests`
- [ ] 失败时显示明确错误消息

---

## 任务优先级

| 任务组 | 优先级 | 预计工作量 | 依赖关系 |
|--------|--------|-----------|---------|
| 0.1 类型系统 | 🔴 P0 | 2 天 | 无 |
| 0.2 ConflictGuard | 🔴 P0 | 5 天 | 0.1 |
| 0.3 AgentExecutor | 🔴 P0 | 1 天 | 0.1, 0.2 |
| 0.4 Builder 模式 | 🟠 P1 | 2 天 | 0.1, 0.2, 0.3 |
| 0.5 配置验证 | 🟠 P1 | 1 天 | 0.2 |
| 0.6 单元测试 | 🔴 P0 | 3 天 | 0.1, 0.2, 0.3, 0.4 |
| 0.7 模块导出 | 🟡 P2 | 0.5 天 | 0.1, 0.2 |
| 0.8 CLI 命令 | 🟡 P2 | 3 天 | 0.2, 0.7 |
| 0.9 GUI 组件 | 🟡 P2 | 3 天 | 0.2, 0.7 |
| 0.10 文档 | 🟢 P3 | 0.5 天 | 0.1-0.9 |
| 0.11 CI/CD | 🟢 P3 | 1 天 | 0.6 |

**总预计**: 21.5 天

---

## 验收标准总结

### 必须满足的约束

1. ✅ **编译时强制**: `SafeMemoryContext` 无法直接创建
2. ✅ **API 层强制**: Builder 必须调用 `check_conflicts()`
3. ✅ **配置层强制**: 启动时验证 `enforce_check = true`
4. ✅ **测试层强制**: CI/CD 自动检测违规
5. ✅ **文档层强制**: 代码审查清单清晰

### 最终验收

- [ ] 所有 11 个任务组完成
- [ ] 所有单元测试通过 (`cargo test`)
- [ ] CI/CD 通过 (`enforcement_tests` job)
- [ ] 文档更新完整
- [ ] 代码审查通过（无绕过路径）

---
## 任务组 0.3: AgentExecutor 集成 (强制 SafeMemoryContext)

> **优先级**: 🔴 P0 (最高优先级)
> **预计工作量**: 1 天
> **依赖关系**: 0.1, 0.2
> **状态**: ✅ 已完成 (2026-02-15)
> **关键成果**: Agent 执行 API 强制要求 SafeMemoryContext（编译时保证）
> **文档**: [docs/plan/v1.1.6/TASK_GROUP_0.3_AGENT_EXECUTOR_INTEGRATION.md](docs/plan/v1.1.6/TASK_GROUP_0.3_AGENT_EXECUTOR_INTEGRATION.md)

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-15
**状态**: ✅ 任务组 0.1, 0.2 已完成；准备开始任务组 0.12
**下一步**: 开始任务组 0.12 (Memory Scope: 稳定哈希绑定）

---

## 任务组 0.12: Memory Scope (稳定哈希绑定) (v1.1.7)

> **优先级**: 🔴 P0 (基础依赖)
> **预计工作量**: 2 天
> **依赖关系**: 无
> **状态**: ✅ 已完成 (2026-02-15)
> **关键成果**: 目录哈希绑定作用域，解决 path 变动问题

### 0.12.1 创建 MemoryScope 结构

**目标**: 定义记忆作用域结构（稳定哈希绑定）

**文件**: `cis-core/src/memory/scope.rs` (新建)

**核心代码**:
```rust
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CisError, Result};
use crate::types::MemoryDomain;

/// 🔥 记忆作用域（稳定哈希绑定）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope {
    /// 作用域 ID（哈希或用户自定义）
    pub scope_id: String,

    /// 人类可读名称（可选，用于调试和 UI）
    pub display_name: Option<String>,

    /// 物理路径（可选，仅用于第一次初始化）
    #[serde(skip)]
    pub path: Option<PathBuf>,

    /// 记忆域（私域/公域）
    pub domain: MemoryDomain,
}
```

**验收标准**:
- [x] `scope_id` 字段实现
- [x] `display_name` 字段实现
- [x] `path` 字段实现（#[serde(skip)]）
- [x] `domain` 字段实现
- [x] 单元测试验证结构体定义

---

### 0.12.2 实现 from_config() 方法

**目标**: 从配置文件加载（核心方法）

**文件**: `cis-core/src/memory/scope.rs`

**核心代码**:
```rust
impl MemoryScope {
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
}
```

**验收标准**:
- [x] `from_config()` 方法实现
- [x] 调用 `load_or_generate_scope_id()`
- [x] 设置 `display_name`, `path`, `domain`
- [x] 返回 `Result<MemoryScope>`
- [x] 单元测试验证加载逻辑

---

### 0.12.3 实现 custom() 方法

**目标**: 自定义记忆域（不依赖 path）

**文件**: `cis-core/src/memory/scope.rs`

**核心代码**:
```rust
impl MemoryScope {
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
}
```

**验收标准**:
- [x] `custom()` 方法实现
- [x] `scope_id` 参数支持 `Into<String>`
- [x] `display_name` 参数支持 `Option`
- [x] `path` 设置为 `None`
- [x] 单元测试验证自定义作用域

---

### 0.12.4 实现 memory_key() 方法

**目标**: 生成记忆键（scope_id + key）

**文件**: `cis-core/src/memory/scope.rs`

**核心代码**:
```rust
impl MemoryScope {
    pub fn memory_key(&self, key: &str) -> String {
        format!("{}::{}", self.scope_id, key)
    }
}
```

**验收标准**:
- [x] `memory_key()` 方法实现
- [x] 返回格式：`{scope_id}::{key}`
- [x] 单元测试验证键格式

---

### 0.12.5 实现 hash_path() 方法

**目标**: 生成目录哈希（稳定且唯一）

**文件**: `cis-core/src/memory/scope.rs`

**核心代码**:
```rust
impl MemoryScope {
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

**验收标准**:
- [x] `hash_path()` 私有方法实现
- [x] 规范化路径（`canonicalize()`）
- [x] 使用 `DefaultHasher`（64 位）
- [x] 返回 16 字符 16 进制字符串
- [x] 单元测试验证哈希唯一性和稳定性

---

### 0.12.6 实现 load_or_generate_scope_id() 方法

**目标**: 从配置加载或生成 scope_id

**文件**: `cis-core/src/memory/scope.rs`

**核心代码**:
```rust
impl MemoryScope {
    fn load_or_generate_scope_id(config: &crate::project::ProjectConfig) -> Result<String> {
        match config.memory.scope_id.as_str() {
            // 配置文件中已有 → 直接使用
            id if !id.is_empty() && id != "auto" => {
                Ok(id.to_string())
            }

            // 配置文件中没有 → 生成并保存
            "" | "auto" => {
                // 1. 生成哈希
                let hash = Self::hash_path(&config.root_dir);

                // 2. 保存到配置文件
                let mut config_clone = config.clone();
                config_clone.memory.scope_id = hash.clone();

                if let Err(e) = config_clone.save() {
                    return Err(CisError::config_validation_error(
                        "project_config",
                        format!("Failed to save scope_id: {}", e)
                    ));
                }

                Ok(hash)
            }

            // 不应该到达
            id => {
                unreachable!("Unexpected scope_id value: {}", id)
            }
        }
    }
}
```

**验收标准**:
- [x] `load_or_generate_scope_id()` 私有方法实现
- [x] 配置文件有 scope_id → 直接返回
- [x] 配置文件为空 → 生成哈希并保存
- [x] 调用 `ProjectConfig::save()`
- [x] 单元测试验证加载和生成逻辑

---

### 0.12.7 扩展 ProjectConfig 添加新字段

**目标**: 支持 scope_id 和 display_name

**文件**: `cis-core/src/project/mod.rs`

**核心代码**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub namespace: String,
    #[serde(default)]
    pub shared_keys: Vec<String>,

    /// 🔥 作用域 ID (v1.1.7)
    #[serde(default = "default_scope_id")]
    pub scope_id: String,

    /// 🔥 人类可读名称 (v1.1.7)
    #[serde(default)]
    pub display_name: Option<String>,
}

fn default_scope_id() -> String {
    "".to_string()  // 默认为空，第一次初始化时生成哈希
}
```

**验收标准**:
- [x] `scope_id` 字段添加到 `MemoryConfig`
- [x] `display_name` 字段添加到 `MemoryConfig`
- [x] `default_scope_id()` 函数实现
- [x] `Project::init()` 更新（添加新字段）
- [x] 单元测试验证配置加载

---

### 0.12.8 实现 ProjectConfig::save() 方法

**目标**: 保存配置到 `.cis/project.toml`

**文件**: `cis-core/src/project/mod.rs`

**核心代码**:
```rust
impl ProjectConfig {
    pub fn save(&self) -> Result<()> {
        let config_path = self.root_dir.join(".cis").join("project.toml");

        // 1. 序列化为 TOML
        let content = toml::to_string_pretty(self)
            .map_err(|e| CisError::config_validation_error(
                "project_config",
                format!("Failed to serialize: {}", e)
            ))?;

        // 2. 写入文件
        std::fs::write(&config_path, content)
            .map_err(|e| CisError::config_validation_error(
                "project_config",
                format!("Failed to write to {:?}: {}", config_path, e)
            ))?;

        println!("[INFO] Saved project config to {:?}", config_path);
        Ok(())
    }
}
```

**验收标准**:
- [x] `save()` 方法实现
- [x] 序列化为 TOML
- [x] 写入到 `.cis/project.toml`
- [x] 错误处理使用 `CisError::config_validation_error()`
- [x] 单元测试验证保存逻辑

---

### 0.12.9 更新 memory/mod.rs 导出

**目标**: 导出 MemoryScope 类型

**文件**: `cis-core/src/memory/mod.rs`

**核心代码**:
```rust
pub mod scope;  // 🔥 记忆作用域 (v1.1.7)

pub use scope::MemoryScope;  // 🔥 记忆作用域
```

**验收标准**:
- [x] `pub mod scope` 声明添加
- [x] `pub use scope::MemoryScope` 导出
- [x] 编译通过
- [x] 文档注释完整

---

### 0.12.10 单元测试

**目标**: 测试 MemoryScope 所有功能

**文件**: `cis-core/src/memory/scope.rs`

**测试覆盖**:
```rust
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
```

**验收标准**:
- [x] `test_hash_path_generation` 测试通过
- [x] `test_hash_path_uniqueness` 测试通过
- [x] `test_custom_scope` 测试通过
- [x] `test_global_scope` 测试通过
- [x] `test_memory_key_generation` 测试通过
- [x] `test_display_implementation` 测试通过
- [x] `test_default_implementation` 测试通过
- [x] 所有测试通过（`cargo test`）

---

### 任务组总结

**完成标准**:
- [x] 所有 10 个子任务完成
- [x] 所有单元测试通过
- [x] 编译无警告
- [x] 文档注释完整

**关键成果**:
1. ✅ 实现稳定哈希绑定机制
2. ✅ 解决 path 变动问题
3. ✅ 支持自定义 scope_id
4. ✅ 支持跨项目共享记忆

**预计时间**: 2 天
**实际时间**: 2 天 (已完成 2026-02-15)

---
