//! 错误处理

use serde::{Deserialize, Serialize};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use alloc::string::String;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use alloc::vec::Vec;

/// Skill 错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    NotInitialized(String),
    InvalidInput(String),
    HostError(String),
    AiError(String),
    MemoryError(String),
    NetworkError(String),
    SerializationError(String),
    PermissionDenied(String),
    NotFound(String),
    Internal(String),
    Other(String),
}

#[cfg(feature = "native")]
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotInitialized(msg) => write!(f, "Skill not initialized: {}", msg),
            Error::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Error::HostError(msg) => write!(f, "Host API error: {}", msg),
            Error::AiError(msg) => write!(f, "AI service error: {}", msg),
            Error::MemoryError(msg) => write!(f, "Memory operation failed: {}", msg),
            Error::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Error::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
            Error::Other(msg) => write!(f, "Other: {}", msg),
        }
    }
}

#[cfg(all(feature = "wasm", not(feature = "native")))]
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NotInitialized(msg) => write!(f, "Skill not initialized: {}", msg),
            Error::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Error::HostError(msg) => write!(f, "Host API error: {}", msg),
            Error::AiError(msg) => write!(f, "AI service error: {}", msg),
            Error::MemoryError(msg) => write!(f, "Memory operation failed: {}", msg),
            Error::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Error::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Error::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
            Error::Other(msg) => write!(f, "Other: {}", msg),
        }
    }
}

#[cfg(feature = "native")]
impl std::error::Error for Error {}

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
#[cfg(feature = "native")]
pub type Result<T> = std::result::Result<T, Error>;

/// Result 类型别名 (WASM)
#[cfg(all(feature = "wasm", not(feature = "native")))]
pub type Result<T> = core::result::Result<T, Error>;

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
