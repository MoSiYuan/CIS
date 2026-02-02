//! # Conversation Module
//!
//! 对话管理层，处理 Session 对话上下文和历史记录。
//!
//! ## 模块结构
//!
//! - `context`: 对话上下文管理
//! - `db`: 对话数据库存储
//!
//! ## 功能特性
//!
//! - 多轮对话历史追踪
//! - 对话摘要生成
//! - 话题标签管理
//! - 项目关联对话
//! - 向量检索历史消息（RAG支持）
//! - 跨项目会话恢复

pub mod context;

// 从 storage 模块 re-export conversation_db 的内容
pub use crate::storage::conversation_db::{Conversation, ConversationDb, ConversationMessage};

// 从 context 模块导出主要类型
pub use context::{
    ConversationContext, ContextMessage, MessageRole, 
    RecoverableSession, SessionRecovery,
};
