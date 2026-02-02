//! 错误处理

use serde::{Deserialize, Serialize};

/// Skill 错误类型
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    #[error("Skill not initialized: {0}")]
    NotInitialized(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Host API error: {0}")]
    HostError(String),
    
    #[error("AI service error: {0}")]
    AiError(String),
    
    #[error("Memory operation failed: {0}")]
    MemoryError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Other: {0}")]
    Other(String),
}

impl Error {
    /// 创建 NotFound 错误
    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }
    
    /// 创建 InvalidInput 错误
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Error::InvalidInput(msg.into())
    }
    
    /// 创建 HostError 错误
    pub fn host_error(msg: impl Into<String>) -> Self {
        Error::HostError(msg.into())
    }
}

/// Result 类型别名
pub type Result<T> = std::result::Result<T, Error>;

/// 错误码映射（WASM 边界使用）
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Success = 0,
    NotInitialized = -1,
    InvalidInput = -2,
    HostError = -3,
    AiError = -4,
    MemoryError = -5,
    NetworkError = -6,
    SerializationError = -7,
    PermissionDenied = -8,
    NotFound = -9,
    Internal = -10,
    Other = -99,
}

impl From<&Error> for ErrorCode {
    fn from(err: &Error) -> Self {
        match err {
            Error::NotInitialized(_) => ErrorCode::NotInitialized,
            Error::InvalidInput(_) => ErrorCode::InvalidInput,
            Error::HostError(_) => ErrorCode::HostError,
            Error::AiError(_) => ErrorCode::AiError,
            Error::MemoryError(_) => ErrorCode::MemoryError,
            Error::NetworkError(_) => ErrorCode::NetworkError,
            Error::SerializationError(_) => ErrorCode::SerializationError,
            Error::PermissionDenied(_) => ErrorCode::PermissionDenied,
            Error::NotFound(_) => ErrorCode::NotFound,
            Error::Internal(_) => ErrorCode::Internal,
            Error::Other(_) => ErrorCode::Other,
        }
    }
}

impl From<ErrorCode> for i32 {
    fn from(code: ErrorCode) -> Self {
        code as i32
    }
}
