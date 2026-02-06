//! # CIS Matrix 事件模块
//!
//! 提供强类型的 CIS Skill 事件定义，遵循 io.cis.{skill}.{action} 命名空间规范。
//!
//! ## 事件命名空间
//!
//! - `io.cis.task.*` - 任务相关事件
//! - `io.cis.git.*` - Git 操作事件
//! - `io.cis.im.*` - 即时消息事件
//! - `io.cis.nav.*` - 导航事件
//! - `io.cis.memory.*` - 记忆更新事件
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::matrix::events::{TaskInvokeEventContent, SkillEventRegistry, SkillEvent};
//!
//! // 创建任务调用事件
//! let event = TaskInvokeEventContent {
//!     task_id: "task-123".to_string(),
//!     skill_name: "git".to_string(),
//!     method: "push".to_string(),
//!     params: serde_json::json!({"branch": "main"}),
//!     priority: 1,
//! };
//!
//! // 从 JSON 解析事件
//! let content = serde_json::json!({...});
//! let event = SkillEventRegistry::parse_event("io.cis.task.invoke", &content);
//! ```

pub mod dag;
pub mod event_types;
pub mod skill;

// Re-export dag event types
pub use dag::{
    DagExecuteEvent,
    DagExecuteContent,
    DagStatusEvent,
    DagStatusContent,
    TodoProposalEvent,
    TodoProposalContent,
    TodoProposalResponseEvent,
    TodoProposalResponseContent,
    NodeClaimFilter,
    parse_dag_event,
    parse_todo_proposal_event,
    parse_todo_proposal_response,
};

// Re-export skill event types
pub use skill::{
    DiffStats,
    GitPushEventContent,
    ImMessageEventContent,
    MemoryUpdateEventContent,
    MessageContent,
    NavTargetEventContent,
    SkillEvent,
    SkillEventError,
    SkillEventRegistry,
    TaskInvokeEventContent,
    TaskResultEventContent,
};

// Re-export event type mapping types
pub use event_types::{
    EventCategory, EventTypeError, EventTypeMapper, MatrixEventType, TypedCisMatrixEvent,
};
