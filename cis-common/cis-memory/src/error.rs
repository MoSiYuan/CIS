use std::error::Error;

#[derive(Debug)]
pub enum MemoryError {
    NotFound,
    StorageError(String),
    SerializationError(String),
    IndexError(String),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::NotFound => write!(f, "Memory entry not found"),
            MemoryError::StorageError(e) => write!(f, "Storage error: {}", e),
            MemoryError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            MemoryError::IndexError(e) => write!(f, "Index error: {}", e),
        }
    }
}

impl Error for MemoryError {}
