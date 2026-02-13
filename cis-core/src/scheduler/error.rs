//! # Scheduler 错误类型
//!
//! 定义调度器模块的专用错误类型。
//!
//! ## 设计原则
//! - 复用现有的 CisError 类型
//! - 不引入新的错误类型，保持一致性

use crate::error::Result as CisResult;

/// Scheduler Result 类型
///
/// 重新导出 CisResult 作为模块的 Result 类型。
pub type Result<T> = CisResult<T>;
