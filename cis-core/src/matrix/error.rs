//! # Matrix Error Types
//!
//! Error types specific to Matrix protocol integration.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Result type alias for Matrix operations
pub type MatrixResult<T> = std::result::Result<T, MatrixError>;

/// Matrix-specific error types
#[derive(Error, Debug)]
pub enum MatrixError {
    /// Invalid JSON request body
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    /// Missing or invalid authorization
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Forbidden access
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Rate limited
    #[error("Rate limited")]
    RateLimited,

    /// Server error
    #[error("Server error: {0}")]
    ServerError(String),

    /// Store/database error
    #[error("Store error: {0}")]
    Store(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Unsupported endpoint or feature
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl MatrixError {
    /// Get the Matrix error code for this error
    fn error_code(&self) -> &'static str {
        match self {
            MatrixError::InvalidJson(_) => "M_NOT_JSON",
            MatrixError::Unauthorized(_) => "M_UNKNOWN_TOKEN",
            MatrixError::Forbidden(_) => "M_FORBIDDEN",
            MatrixError::NotFound(_) => "M_NOT_FOUND",
            MatrixError::RateLimited => "M_LIMIT_EXCEEDED",
            MatrixError::ServerError(_) => "M_UNKNOWN",
            MatrixError::Store(_) => "M_DATABASE_ERROR",
            MatrixError::InvalidParameter(_) => "M_INVALID_PARAM",
            MatrixError::NotImplemented(_) => "M_UNRECOGNIZED",
            MatrixError::Internal(_) => "M_UNKNOWN",
        }
    }

    /// Get the HTTP status code for this error
    fn status_code(&self) -> StatusCode {
        match self {
            MatrixError::InvalidJson(_) => StatusCode::BAD_REQUEST,
            MatrixError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            MatrixError::Forbidden(_) => StatusCode::FORBIDDEN,
            MatrixError::NotFound(_) => StatusCode::NOT_FOUND,
            MatrixError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            MatrixError::ServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            MatrixError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            MatrixError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
            MatrixError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            MatrixError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for MatrixError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_code = self.error_code();
        let message = self.to_string();

        let body = Json(json!({
            "errcode": error_code,
            "error": message,
        }));

        (status, body).into_response()
    }
}

impl From<rusqlite::Error> for MatrixError {
    fn from(err: rusqlite::Error) -> Self {
        MatrixError::Store(err.to_string())
    }
}

impl From<std::io::Error> for MatrixError {
    fn from(err: std::io::Error) -> Self {
        MatrixError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for MatrixError {
    fn from(err: serde_json::Error) -> Self {
        MatrixError::InvalidJson(err.to_string())
    }
}
