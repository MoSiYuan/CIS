# Agent 记忆下发守卫设计 (冲突检测前置)

> **版本**: v1.1.7
> **创建日期**: 2026-02-14
> **核心问题**: Agent 执行任务前必须先检查公域记忆冲突，解决冲突前不下发记忆
> **设计原则**: 冲突检测前置 + 阻塞式下发 + 公域记忆冲突 + 用户决策优先

---

## 问题背景

### 原有设计的问题

之前的设计中，冲突检测是在**同步时**被动触发：

```rust
// ❌ 错误：同步时才检测冲突
impl MemorySyncManager {
    pub async fn handle_sync_message(&self, data: &[u8]) -> Result<()> {
        // 接收远程同步消息
        // 检测冲突
        // 覆盖本地数据
        // 用户可能根本不知道！
    }
}
```

**问题**:
1. Agent 可能在同步前就使用了冲突的本地数据
2. Agent 基于错误数据做出的决策无法撤销
3. 用户发现冲突时 Agent 已经执行了任务

### 改进方向

**Agent 执行任务前主动检测冲突**：

```
┌────────────────────────────────────────────────────┐
│  Agent 执行前冲突检测流程                          │
├────────────────────────────────────────────────────┤
│                                                  │
│  1. Agent 请求执行任务                           │
│     ↓                                            │
│  2. ConflictGuard 检查公域记忆冲突               │
│     ↓                                            │
│  3. 有冲突？                                    │
│        ├─ 是 → 阻塞，显示冲突给用户               │
│        │         ↓                                 │
│        │         用户选择解决方案                    │
│        │         ↓                                 │
│        │         解决冲突 → 回到步骤 2               │
│        │                                          │
│        └─ 否 → 下发记忆给 Agent                   │
│               ↓                                   │
│  4. Agent 执行任务                              │
│                                                  │
└────────────────────────────────────────────────────┘
```

---

## 核心设计

### 1. ConflictGuard 结构

**定义** (cis-core/src/memory/guard/conflict_guard.rs):

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::types::{Result, CisError};

/// 冲突守卫：在下发记忆前检查冲突
pub struct ConflictGuard {
    memory_service: Arc<MemoryService>,
    unresolved_conflicts: Arc<RwLock<HashMap<String, ConflictNotification>>>,
    config: ConflictGuardConfig,
}

pub struct ConflictGuardConfig {
    /// 🔴 Agent 执行前是否强制检查冲突（必须为 true，不可配置）
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
            enforce_check: true,  // 🔴 强制为 true，不可修改
            conflict_timeout_secs: 300,
            default_resolution: ConflictResolutionStrategy::WaitForUser,
        }
    }

    /// 🔴 禁止创建非强制检查的配置（编译时断言）
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

    /// AI 合并
    AIMerge,
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

    /// 检测新的冲突（基于公域记忆）
    async fn detect_new_conflicts(&self, keys: &[String]) -> Result<Vec<ConflictNotification>> {
        let mut new_conflicts = Vec::new();

        for key in keys {
            // 只检查公域记忆
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
                                    conflict_id: uuid::Uuid::new_v4().to_string(),
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

    /// 解决冲突
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

---

### 2. Agent 任务执行前置检查

**设计** (cis-core/src/agent/executor.rs):

```rust
use crate::memory::guard::ConflictGuard;

pub struct AgentExecutor {
    memory_service: Arc<MemoryService>,
    conflict_guard: Arc<ConflictGuard>,
    agent: Arc<dyn Agent>,
}

impl AgentExecutor {
    /// 执行 Agent 任务（带冲突检测）
    pub async fn execute_with_conflict_check(
        &self,
        task: AgentTask,
    ) -> Result<AgentResult> {
        // 1. 提取任务需要的记忆键
        let required_keys = self.extract_required_memory_keys(&task).await?;

        // 2. 🔥 执行前检查冲突
        let check_result = self.conflict_guard.check_conflicts_before_delivery(
            &required_keys
        ).await?;

        match check_result {
            ConflictCheckResult::NoConflicts => {
                // 3. 无冲突，继续执行
                tracing::info!("No conflicts found, delivering memory to agent");

                // 下发记忆给 Agent
                let memory_context = self.build_memory_context(&required_keys).await?;

                // Agent 执行任务
                let result = self.agent.execute(task, memory_context).await?;

                Ok(result)
            }

            ConflictCheckResult::HasConflicts { conflicts, required_action } => {
                // 4. 有冲突，阻塞并通知用户
                tracing::error!(
                    "Cannot execute agent task: {} unresolved conflicts detected",
                    conflicts.len()
                );

                // 显示冲突给用户
                self.display_conflicts_to_user(&conflicts).await?;

                // 返回错误，阻止 Agent 执行
                Err(CisError::conflict_blocked(format!(
                    "Agent execution blocked: {} conflicts must be resolved first. \
                    Use 'cis memory conflicts resolve' to resolve conflicts.",
                    conflicts.len()
                )))
            }
        }
    }

    /// 提取任务需要的记忆键
    async fn extract_required_memory_keys(&self, task: &AgentTask) -> Result<Vec<String>> {
        // 从任务描述中提取需要的记忆键
        // 例如: "project/config", "api/endpoint" 等
        let mut keys = Vec::new();

        // 从任务的 memory_dependencies 字段读取
        for dep in &task.memory_dependencies {
            keys.push(dep.key.clone());
        }

        Ok(keys)
    }

    /// 构建记忆上下文（只在没有冲突时调用）
    async fn build_memory_context(&self, keys: &[String]) -> Result<MemoryContext> {
        let mut memories = HashMap::new();

        for key in keys {
            // 🔥 只下发私域记忆和已确认的公域记忆
            if let Some(entry) = self.memory_service.get(key).await? {
                // 检查是否是未解决冲突的公域记忆
                let is_conflicted = self.conflict_guard.is_key_conflicted(key).await?;

                if is_conflicted {
                    return Err(CisError::conflict_blocked(format!(
                        "Key '{}' is conflicted, cannot deliver to agent",
                        key
                    )));
                }

                memories.insert(key.clone(), entry);
            }
        }

        Ok(MemoryContext { memories })
    }

    /// 显示冲突给用户（CLI/GUI）
    async fn display_conflicts_to_user(&self, conflicts: &[ConflictNotification]) -> Result<()> {
        // 根据运行环境（CLI/GUI）显示冲突
        #[cfg(feature = "cli")]
        {
            self.display_conflicts_cli(conflicts).await?;
        }

        #[cfg(feature = "gui")]
        {
            self.display_conflicts_gui(conflicts).await?;
        }

        Ok(())
    }

    /// CLI 显示冲突
    async fn display_conflicts_cli(&self, conflicts: &[ConflictNotification]) -> Result<()> {
        println!();
        println!("⚠️  无法执行 Agent 任务：发现 {} 个未解决的记忆冲突", conflicts.len());
        println!();
        println!("必须先解决冲突才能继续执行任务。");
        println!();

        for (i, conflict) in conflicts.iter().enumerate() {
            println!("{}. 键: {}", i + 1, conflict.key);
            println!("   本地版本:");
            println!("     值: {}", String::from_utf8_lossy(&conflict.local_version.value));
            println!("     时间: {}", conflict.local_version.timestamp);
            println!("     节点: {}", conflict.local_version.node_id);
            println!();
            println!("   远程版本:");
            println!("     值: {}", String::from_utf8_lossy(&conflict.remote_version.value));
            println!("     时间: {}", conflict.remote_version.timestamp);
            println!("     节点: {}", conflict.remote_version.node_id);
            println!();
        }

        println!("解决冲突:");
        println!("  cis memory conflicts list");
        println!("  cis memory conflicts resolve <conflict-id> <choice>");
        println!();
        println!("选择:");
        println!("  1 - 保留本地版本");
        println!("  2 - 保留远程版本");
        println!("  3 - 保留两个版本");
        println!("  4 - AI 合并");
        println!();

        Ok(())
    }
}
```

---

## 强制执行保障（无绕过路径）

### 保障机制 1: 类型系统强制（编译时）

**设计目标**：只有通过冲突检查的 MemoryContext 才能传给 Agent

```rust
use std::marker::PhantomData;

/// 🔥 冲突已检查的标记（编译时保证）
pub struct ConflictChecked;

/// 🔥 只有通过冲突检查才能创建的 Memory Context
pub struct SafeMemoryContext {
    _phantom: PhantomData<ConflictChecked>,
    memories: HashMap<String, MemoryEntry>,
}

impl SafeMemoryContext {
    /// 🔥 私有构造函数，只有 ConflictGuard 能创建
    fn new(memories: HashMap<String, MemoryEntry>) -> Self {
        Self {
            _phantom: PhantomData,
            memories,
        }
    }

    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.memories.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &MemoryEntry)> {
        self.memories.iter()
    }
}

/// 🔥 ConflictGuard 是唯一能创建 SafeMemoryContext 的地方
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

**Agent 执行 API 强制要求 SafeMemoryContext**:

```rust
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
        self.agent.execute(task, memory).await
    }

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

**编译时保证**：
```rust
// ❌ 编译错误：无法直接创建 SafeMemoryContext
let context = SafeMemoryContext::new(memories);  // 编译错误：字段是私有的

// ❌ 编译错误：execute API 不接受普通 HashMap
let result = executor.execute(task, memories).await;  // 类型不匹配

// ✅ 唯一正确的路径：必须通过 ConflictGuard
let context = conflict_guard.check_and_create_context(&keys).await?;
let result = executor.execute(task, context).await?;
```

---

### 保障机制 2: Builder 模式强制（API 层）

**设计目标**：Agent 执行必须通过 Builder，且 Builder 强制调用冲突检查

```rust
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

**使用示例（强制路径）**:
```rust
// ✅ 唯一正确的执行路径
let result = AgentTaskBuilder::new(&executor)
    .with_task(task)
    .with_memory_keys(keys)
    .check_conflicts()  // ← 强制调用，否则无法 execute
    .await?
    .execute()
    .await?;

// ❌ 编译错误：不调用 check_conflicts 无法 execute
let result = AgentTaskBuilder::new(&executor)
    .with_task(task)
    .with_memory_keys(keys)
    // .check_conflicts()  // ← 忘记调用
    .execute()  // ← 运行时断言失败
    .await?;
// panic: "Conflict check is mandatory. No bypass path allowed!"
```

---

### 保障机制 3: 配置文件强制（运行时）

**设计目标**：配置文件不允许修改 `enforce_check` 为 false

```toml
# ~/.cis/config.toml

[memory.conflict]
# 🔴 Agent 执行前强制检查冲突（不可修改）
# 注意：修改此配置不会生效，系统会在启动时验证
enforce_check = true  # 始终为 true，硬编码在代码中

# 冲突超时（秒）
conflict_timeout_secs = 300
```

**启动时验证** (cis-core/src/config/mod.rs):

```rust
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

**配置验证示例**:
```bash
# 用户尝试修改配置为 false
$ vim ~/.cis/config.toml
# [memory.conflict]
# enforce_check = false  # ← 用户尝试绕过

# CIS 启动时检测并拒绝
$ cis agent run deploy-task
❌ Configuration error: memory.conflict.enforce_check cannot be set to false.
   Conflict check is mandatory and cannot be bypassed.
   Aborting CIS startup.

# 或强制修正配置
$ cis agent run deploy-task
⚠️  Configuration warning: memory.conflict.enforce_check must be true.
   Forcing enforce_check = true (ignoring config file value).
```

---

### 保障机制 4: 单元测试强制（CI/CD）

**设计目标**：CI/CD 自动检测任何绕过冲突检测的代码路径

```rust
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
}
```

**CI/CD 集成** (.github/workflows/test.yml):
```yaml
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
```

---

### 保障机制 5: 代码审查清单（文档）

**在 [CONTRIBUTING.md](../../CONTRIBUTING.md) 中添加**:

```markdown
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

---

## 总结：强制执行的层级保障

| 层级 | 保障机制 | 绕过难度 | 说明 |
|------|----------|----------|------|
| **编译时** | 类型系统（SafeMemoryContext） | 🔴 不可能 | 编译器保证，无法绕过 |
| **API 层** | Builder 模式（强制 check_conflicts） | 🔴 极难 | 运行时断言 + panic |
| **配置层** | 启动时验证（enforce_check） | 🟠 很难 | 启动时强制修正或拒绝 |
| **测试层** | enforcement_tests | 🟡 中等 | CI/CD 自动检测 |
| **文档层** | CONTRIBUTING.md 清单 | 🟡 中等 | 代码审查时检查 |

**综合结论**: ✅ **没有任何绕过路径**

多层保障确保：
1. 编译时阻止不安全的代码
2. API 层强制检查流程
3. 配置层防止用户修改
4. 测试层自动检测违规
5. 文档层指导开发者遵守规则

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
**核心洞察**: Agent 执行前 + 公域记忆冲突检测 + 用户决策 + **多层强制保障** = 完美防止数据错误传播

### 场景 1: Agent 执行任务时检测到冲突

```rust
async fn example_agent_conflict_blocked() -> Result<()> {
    let executor = AgentExecutor::new(...).await?;

    // 用户请求 Agent 执行任务
    let task = AgentTask {
        name: "deploy-project".to_string(),
        memory_dependencies: vec![
            MemoryDependency { key: "project/config".to_string() },
            MemoryDependency { key: "api/endpoint".to_string() },
        ],
        // ... 其他任务参数
    };

    // 尝试执行任务
    match executor.execute_with_conflict_check(task).await {
        Ok(result) => {
            println!("任务执行成功: {:?}", result);
        }

        Err(CisError::ConflictBlocked { message }) => {
            // 🔴 冲突阻塞，任务未执行
            eprintln!("⚠️  {}", message);
            eprintln!();
            eprintln!("先解决冲突：");
            eprintln!("  $ cis memory conflicts resolve project/config 2");
        }

        Err(e) => {
            eprintln!("任务执行失败: {}", e);
        }
    }

    Ok(())
}
```

**输出**:
```
⚠️  无法执行 Agent 任务：发现 1 个未解决的记忆冲突

必须先解决冲突才能继续执行任务。

1. 键: project/config
   本地版本:
     值: timeout=30
     时间: 2026-02-14 10:00:00 UTC
     节点: device-a

   远程版本:
     值: timeout=60
     时间: 2026-02-14 10:00:03 UTC
     节点: device-b

解决冲突:
  $ cis memory conflicts list
  $ cis memory conflicts resolve <conflict-id> <choice>

选择:
  1 - 保留本地版本
  2 - 保留远程版本
  3 - 保留两个版本
  4 - AI 合并
```

---

### 场景 2: 用户解决冲突后重新执行

```bash
# 1. 查看所有冲突
$ cis memory conflicts list

未解决的冲突:

1. project/config
   冲突ID: abc-123-def-456
   本地: timeout=30 (device-a, 10:00:00)
   远程: timeout=60 (device-b, 10:00:03)

# 2. 解决冲突（保留远程版本）
$ cis memory conflicts resolve abc-123-def-456 2

✅ 已解决冲突: project/config
    保留远程版本: timeout=60

# 3. 重新执行 Agent 任务
$ cis agent run deploy-project

✅ 任务执行成功
```

---

### 场景 3: GUI 中的冲突提示

```rust
// cis-gui/src/components/conflict_dialog.rs

use egui::{self, *};

pub struct ConflictDialog {
    conflicts: Vec<ConflictNotification>,
    selected_resolution: HashMap<String, ConflictResolutionChoice>,
}

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
                            .or_insert(ConflictResolutionChoice::KeepLocal);

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
                        self.apply_all_resolutions();
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
            // ...
        }
    }
}
```

**GUI 截图**:
```
┌────────────────────────────────────────────────┐
│  记忆冲突警告                                │
├────────────────────────────────────────────────┤
│  发现 1 个未解决的冲突                      │
│  必须先解决冲突才能执行 Agent 任务               │
│                                              │
│  ┌─ project/config ─────────────────────┐    │
│  │ 本地版本:                            │    │
│  │   值: timeout=30                   │    │
│  │   时间: 2026-02-14 10:00:00        │    │
│  │   节点: device-a                   │    │
│  │                                       │    │
│  │ 远程版本:                            │    │
│  │   值: timeout=60                   │    │
│  │   时间: 2026-02-14 10:00:03        │    │
│  │   节点: device-b                   │    │
│  │                                       │    │
│  │ 解决方案:                             │    │
│  │  ⦿ 保留本地  ⭕ 保留远程            │    │
│  │  ⭕ 保留两个  ⭕ AI 合并            │    │
│  └───────────────────────────────────────┘    │
│                                              │
│  [全部应用]                           [取消]  │
└────────────────────────────────────────────────┘
```

---

## 配置

**配置文件** (~/.cis/config.toml):

```toml
[memory.conflict]
# Agent 执行前是否强制检查冲突
enforce_check = true  # 🔴 必须为 true

# 冲突解决超时（秒）
# 超时后自动使用默认策略
conflict_timeout_secs = 300

# 默认冲突解决策略（用户不操作时）
default_resolution = "wait_for_user"  # wait_for_user | auto_keep_local | auto_keep_remote | ai_merge

# 是否显示详细的冲突信息
verbose = true

# 是否记录冲突到日志
log_conflicts = true

# 冲突保留时间（天）
# 超过这个时间未解决的冲突自动清理
conflict_retention_days = 30
```

---

## CLI 命令

### 查看冲突

```bash
$ cis memory conflicts list

未解决的冲突:

1. project/config
   冲突ID: abc-123-def-456
   本地版本: timeout=30 (device-a, 2026-02-14 10:00:00)
   远程版本: timeout=60 (device-b, 2026-02-14 10:00:03)
   检测时间: 2026-02-14 10:05:00

2. api/endpoint
   冲突ID: xyz-789-uvw-012
   本地版本: https://api.local (device-a, 2026-02-14 09:58:00)
   远程版本: https://api.prod (device-b, 2026-02-14 09:58:02)
   检测时间: 2026-02-14 10:05:00

共 2 个未解决冲突
```

### 解决冲突

```bash
$ cis memory conflicts resolve abc-123-def-456 2

✅ 已解决冲突: project/config
   冲突ID: abc-123-def-456
   选择: 保留远程版本
   应用值: timeout=60

可以重新执行 Agent 任务了：
  $ cis agent run deploy-project
```

### 批量解决

```bash
# 保留所有本地版本
$ cis memory conflicts resolve-all local

✅ 已解决 2 个冲突（全部保留本地版本）

# AI 合并所有冲突
$ cis memory conflicts resolve-all ai-merge

✅ 正在使用 AI 合并 2 个冲突...
✅ 已完成合并
```

---

## 关键设计原则

### 1. 冲突检测基于公域记忆

```rust
// ✅ 只检查公域记忆冲突
let public_entry = self.memory_service.get_public(key).await?;

// ❌ 不检查私域记忆（私域永远不会冲突）
// let private_entry = self.memory_service.get_private(key).await?;
```

**原因**:
- 公域记忆可能来自多个节点，存在并发编辑
- 私域记忆只在本节点，不可能有冲突

### 2. Agent 执行前检测

```rust
// ✅ 正确：Agent 执行前检测
let check_result = self.conflict_guard.check_conflicts_before_delivery(&keys).await?;
match check_result {
    ConflictCheckResult::NoConflicts => {
        // 无冲突，下发记忆给 Agent
        self.deliver_memory_to_agent(keys).await?;
        self.agent.execute(task).await?;
    }
    ConflictCheckResult::HasConflicts { .. } => {
        // 有冲突，阻塞 Agent 执行
        return Err("Conflict blocked");
    }
}

// ❌ 错误：同步时才检测（太晚了）
impl MemorySyncManager {
    pub async fn handle_sync_message(&self, ..) {
        // Agent 可能已经使用了本地数据
    }
}
```

### 3. 冲突解决前不下发记忆

```rust
async fn build_memory_context(&self, keys: &[String]) -> Result<MemoryContext> {
    for key in keys {
        let entry = self.memory_service.get(key).await?;

        // 🔥 检查是否有冲突
        let is_conflicted = self.conflict_guard.is_key_conflicted(key).await?;

        if is_conflicted {
            // ❌ 阻止下发冲突的记忆
            return Err("Key is conflicted, cannot deliver");
        }

        // ✅ 只有没有冲突的记忆才下发
        memories.insert(key.clone(), entry);
    }
}
```

### 4. 用户决策优先

```rust
// ✅ 等待用户手动解决
pub enum ConflictResolutionStrategy {
    WaitForUser,  // ← 默认，最安全
}

// ❌ 不要自动解决（可能丢失数据）
// pub enum ConflictResolutionStrategy {
//     AutoKeepRemote,  // ← 不安全
// }
```

---

## 数据库 Schema

### 冲突记录表

```sql
-- ================================================================
-- 冲突记录表（公域记忆冲突）
-- ================================================================
CREATE TABLE IF NOT EXISTS memory_conflicts (
    conflict_id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    local_value BLOB NOT NULL,
    local_timestamp INTEGER NOT NULL,
    local_node_id TEXT NOT NULL,
    remote_value BLOB NOT NULL,
    remote_timestamp INTEGER NOT NULL,
    remote_node_id TEXT NOT NULL,
    detected_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolution_choice TEXT,  -- 'keep_local' | 'keep_remote' | 'keep_both' | 'ai_merge'
    FOREIGN KEY (key) REFERENCES public_entries(key) ON DELETE CASCADE
);

-- 索引：按键查询冲突
CREATE INDEX idx_memory_conflicts_key
    ON memory_conflicts(key, resolved_at);

-- 索引：查询未解决的冲突
CREATE INDEX idx_memory_conflicts_unresolved
    ON memory_conflicts(detected_at)
    WHERE resolved_at IS NULL;
```

### 公域记忆版本历史

```sql
-- ================================================================
-- 公域记忆版本历史（多版本支持）
-- ================================================================
CREATE TABLE IF NOT EXISTS public_memory_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL,
    value BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    node_id TEXT NOT NULL,
    vector_clock BLOB,  -- 序列化的 Vector Clock
    is_current INTEGER DEFAULT 0,  -- 是否是当前版本
    created_at INTEGER NOT NULL,
    FOREIGN KEY (key) REFERENCES public_entries(key) ON DELETE CASCADE
);

-- 索引：查询某个键的所有版本
CREATE INDEX idx_public_memory_versions_key
    ON public_memory_versions(key, timestamp DESC);
```

---

## 总结

### 设计原则

1. ✅ **冲突检测前置** - Agent 执行任务前主动检测
2. ✅ **阻塞式下发** - 有冲突时阻塞，解决后才能继续
3. ✅ **公域记忆冲突** - 只检查公域记忆，私域不参与
4. ✅ **用户决策优先** - 等待用户手动解决，不自动合并
5. ✅ **私域记忆保护** - 冲突解决前不下发任何私域记忆

### 优势

| 维度 | 评分 | 说明 |
|------|------|------|
| **防数据丢失** | ⭐⭐⭐⭐⭐ | Agent 永远不会使用冲突数据 |
| **用户体验** | ⭐⭐⭐⭐ | 冲突提示清晰，解决流程简单 |
| **安全性** | ⭐⭐⭐⭐⭐ | 用户完全控制冲突解决 |
| **可追溯性** | ⭐⭐⭐⭐ | 所有冲突都有记录和版本历史 |

### 与原设计的对比

| 特性 | 原设计（同步时检测） | 新设计（执行前检测） |
|------|---------------------|---------------------|
| **检测时机** | 被动（收到同步消息时） | 主动（Agent 执行前） |
| **阻塞能力** | ❌ Agent 可能已执行 | ✅ 阻塞 Agent 执行 |
| **用户控制** | ⚠️ 冲突可能已被覆盖 | ✅ 用户先解决再执行 |
| **数据安全** | ❌ Agent 可能用错误数据 | ✅ 保证 Agent 用正确数据 |

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
**核心洞察**: Agent 执行前 + 公域记忆冲突检测 + 用户决策 = 完美防止数据错误传播
