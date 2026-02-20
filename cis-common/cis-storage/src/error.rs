use std::error::Error;

#[derive(Debug)]
pub enum StorageError {
    NotFound,
    AlreadyExists,
    IoError(std::io::Error),
    DatabaseError(String),
    SerializationError(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::NotFound => write!(f, "Key not found"),
            StorageError::AlreadyExists => write!(f, "Key already exists"),
            StorageError::IoError(e) => write!(f, "IO error: {}", e),
            StorageError::DatabaseError(e) => write!(f, "Database error: {}", e),
            StorageError::SerializationError(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::IoError(err)
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}
