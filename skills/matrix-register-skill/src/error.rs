//! # 错误类型

use thiserror::Error;

pub type Result<T> = std::result::Result<T, RegisterError>;

/// Matrix 注册 Skill 错误类型
#[derive(Error, Debug)]
pub enum RegisterError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("User already exists: {0}")]
    UserExists(String),
    
    #[error("User not found: {0}")]
    UserNotFound(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Registration not allowed")]
    RegistrationNotAllowed,
    
    #[error("Invalid invite code")]
    InvalidInviteCode,
    
    #[error("Username reserved")]
    UsernameReserved,
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Invalid DID format")]
    InvalidDIDFormat,
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Too many devices")]
    TooManyDevices,
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<cis_core::matrix::error::MatrixError> for RegisterError {
    fn from(e: cis_core::matrix::error::MatrixError) -> Self {
        match e {
            cis_core::matrix::error::MatrixError::Store(msg) => RegisterError::Database(msg),
            cis_core::matrix::error::MatrixError::NotFound(msg) => RegisterError::UserNotFound(msg),
            cis_core::matrix::error::MatrixError::BadRequest(msg) => RegisterError::InvalidRequest(msg),
            cis_core::matrix::error::MatrixError::UserInUse(user_id) => RegisterError::UserExists(user_id),
            _ => RegisterError::Internal(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for RegisterError {
    fn from(e: serde_json::Error) -> Self {
        RegisterError::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for RegisterError {
    fn from(e: std::io::Error) -> Self {
        RegisterError::Internal(e.to_string())
    }
}
