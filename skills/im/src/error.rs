//! IM Skill 错误类型

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ImError>;

#[derive(Error, Debug)]
pub enum ImError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
    
    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),
    
    #[error("User not found: {0}")]
    UserNotFound(String),
    
    #[error("Unauthorized")]
    Unauthorized,
    
    #[error("Message too large: {size} > {max}")]
    MessageTooLarge { size: usize, max: usize },
    
    #[error("Other: {0}")]
    Other(String),
}

impl From<rusqlite::Error> for ImError {
    fn from(e: rusqlite::Error) -> Self {
        ImError::Database(e.to_string())
    }
}

impl From<serde_json::Error> for ImError {
    fn from(e: serde_json::Error) -> Self {
        ImError::Serialization(e.to_string())
    }
}
