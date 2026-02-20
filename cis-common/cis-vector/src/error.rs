use std::error::Error;

#[derive(Debug)]
pub enum VectorError {
    InvalidDimension,
    NotFound,
    StorageError(String),
    SearchError(String),
}

impl std::fmt::Display for VectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorError::InvalidDimension => write!(f, "Invalid vector dimension"),
            VectorError::NotFound => write!(f, "Vector not found"),
            VectorError::StorageError(e) => write!(f, "Storage error: {}", e),
            VectorError::SearchError(e) => write!(f, "Search error: {}", e),
        }
    }
}

impl Error for VectorError {}
