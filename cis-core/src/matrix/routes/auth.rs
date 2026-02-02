//! # Matrix Authentication Middleware
//!
//! Provides authentication extraction and validation for Matrix API endpoints.

use axum::{
    extract::State,
    http::HeaderMap,
};
use std::sync::Arc;

use crate::matrix::error::MatrixError;
use crate::matrix::store::MatrixStore;

/// Authenticated user information
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub device_id: Option<String>,
}

impl AuthenticatedUser {
    /// Create a new authenticated user
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            device_id: None,
        }
    }
}

/// Extract authorization token from headers
pub fn extract_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth| {
            // Support "Bearer <token>" format
            if auth.to_lowercase().starts_with("bearer ") {
                Some(auth[7..].trim().to_string())
            } else {
                Some(auth.to_string())
            }
        })
        .or_else(|| {
            // Also check query param (for some endpoints)
            None
        })
}

/// Authenticate a request using headers and store
/// 
/// This function is used by handlers to authenticate requests.
/// Example usage in handler:
/// ```rust
/// pub async fn handler(
///     headers: HeaderMap,
///     State(store): State<Arc<MatrixStore>>,
/// ) -> Result<...> {
///     let user = authenticate(&headers, &store)?;
///     // ...
/// }
/// ```
pub fn authenticate(
    headers: &HeaderMap,
    store: &MatrixStore,
) -> Result<AuthenticatedUser, MatrixError> {
    // Extract token from Authorization header
    let token = extract_token(headers)
        .ok_or_else(|| MatrixError::Unauthorized("Missing authorization token".to_string()))?;

    // Validate token
    let user_id = store
        .validate_token(&token)
        .map_err(|e| MatrixError::Unauthorized(format!("Invalid token: {}", e)))?
        .ok_or_else(|| MatrixError::Unauthorized("Invalid or expired token".to_string()))?;

    Ok(AuthenticatedUser {
        user_id,
        device_id: None, // Could be extended to extract device_id from token
    })
}

/// Async version of authenticate for use in async handlers
pub async fn authenticate_async(
    headers: &HeaderMap,
    store: &MatrixStore,
) -> Result<AuthenticatedUser, MatrixError> {
    authenticate(headers, store)
}
