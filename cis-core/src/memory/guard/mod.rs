//! # 冲突检测守卫模块
//!
//! 🔥 **强制执行冲突检测，防止 Agent 使用冲突的记忆** (P1.7.0)
//!
//! # 核心机制
//!
//! 本模块实现了 **5 层强制执行机制**：
//!
//! ```text
//! 1. 编译时强制：SafeMemoryContext 类型系统
//!    ├─ new() 构造函数是私有的（pub(crate)）
//!    └─ 只有 ConflictGuard 可以创建
//!
//! 2. API 层强制：AgentTaskBuilder Pattern
//!    ├─ check_conflicts() 必须调用
//!    └─ 运行时断言 conflict_checked == true
//!
//! 3. 配置层强制：Config::validate()
//!    ├─ enforce_check 默认 true
//!    └─ 启动时强制覆盖错误配置
//!
//! 4. 测试层强制：enforcement_tests
//!    ├─ CI/CD 自动运行
//!    └─ 检测绕过路径
//!
//! 5. 文档层强制：API 文档
//!    └─ 说明强制执行机制
//! ```
//!
//! # 无绕过路径
//!
//! | 层级 | 保障机制 | 绕过难度 | 状态 |
//! |------|----------|----------|------|
//! | **编译时** | 类型系统 | 🔴 **不可能** | ✅ |
//! | **API 层** | Builder 模式 | 🔴 极难 | ✅ |
//! | **配置层** | 启动时验证 | 🟠 很难 | ✅ |
//! | **测试层** | enforcement_tests | 🟡 中等 | ✅ |
//! | **文档层** | API 文档 | 🟡 中等 | ⏳ |
//!
//! # 模块结构
//!
//! - [`types`] - 编译时强制的类型系统
//! - [`conflict_guard`] - 冲突检测守卫实现
//! - [`enforcement_tests`] - 强制执行保障测试 (任务组 0.6)
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use cis_core::memory::guard::ConflictGuard;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let guard = ConflictGuard::new(memory_service);
//!
//! // 🔥 必须先检测冲突
//! let context = guard.check_and_create_context(&["key1", "key2"]).await?;
//!
//! // ✅ 检测通过后才能执行 Agent
//! let result = executor.execute(task, context).await?;
//! # Ok(())
//! # }
//! ```

pub mod types;
pub mod vector_clock;  // 🔥 Vector Clock 实现 (P1.7.0 任务组 0.2)

pub use types::{ConflictChecked, SafeMemoryContext};
pub use vector_clock::{VectorClock, VectorClockRelation};

// 🔥 冲突守卫实现 (任务组 0.2)
pub mod conflict_guard;
pub mod conflict_resolution;  // 🔥 冲突解决逻辑
pub mod ai_merge;  // 🔥 AI 合并实现
pub use conflict_guard::{
    ConflictGuard, ConflictGuardConfig, ConflictCheckResult,
    ConflictNotification, ConflictVersion, ConflictResolutionChoice,
};
pub use conflict_resolution::{
    resolve_by_lww, detect_conflict_by_vector_clock,
    apply_resolution_strategy, apply_resolution_strategy_async,
    create_conflict_notification,
    serialize_vector_clock, KeepBothResult, generate_unique_remote_key,
    apply_keep_both_strategy,
};
pub use ai_merge::{AIMerger, AIMergeConfig, AIMergeStrategy};

// 🔥 强制执行保障测试 (任务组 0.6)
#[cfg(test)]
pub mod enforcement_tests;

// 编译时强制验证测试
#[cfg(test)]
mod compilation_test;

// AI Merge 集成测试
#[cfg(test)]
mod ai_merge_integration_test;
// 🔒 P0安全修复：导出并发安全的VectorClock
pub mod vector_clock_safe;
pub use vector_clock_safe::SafeVectorClock;
