//! # Matrix Authentication Middleware
//!
//! Provides authentication extraction and validation for Matrix API endpoints.
//!
//! ## Architecture Note
//!
//! 认证逻辑使用 `MatrixSocialStore`（matrix-social.db）进行令牌验证，
//! 与协议事件存储分离。

use axum::http::HeaderMap;

use crate::matrix::error::MatrixError;
use crate::matrix::store_social::MatrixSocialStore;

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
        .map(|auth| {
            // Support "Bearer <token>" format
            if auth.to_lowercase().starts_with("bearer ") {
                auth[7..].trim().to_string()
            } else {
                auth.to_string()
            }
        })
        .or({
            // Also check query param (for some endpoints)
            None
        })
}

/// Authenticate a request using headers and social store
/// 
/// 使用 MatrixSocialStore 验证令牌。
/// 
/// Example usage in handler:
/// ```ignore
/// use axum::http::HeaderMap;
/// use axum::extract::State;
/// 
/// pub async fn handler(
///     headers: HeaderMap,
///     State(state): State<AppState>,
/// ) -> Result<String, Box<dyn std::error::Error>> {
///     let user = authenticate(&headers, &state.social_store)?;
///     // ...
///     Ok(String::new())
/// }
/// ```
pub fn authenticate(
    headers: &HeaderMap,
    social_store: &MatrixSocialStore,
) -> Result<AuthenticatedUser, MatrixError> {
    // Extract token from Authorization header
    let token = extract_token(headers)
        .ok_or_else(|| MatrixError::Unauthorized("Missing authorization token".to_string()))?;

    // Validate token using social_store
    let token_info = social_store
        .validate_token(&token)
        .map_err(|e| MatrixError::Unauthorized(format!("Invalid token: {}", e)))?
        .ok_or_else(|| MatrixError::Unauthorized("Invalid or expired token".to_string()))?;

    Ok(AuthenticatedUser {
        user_id: token_info.user_id,
        device_id: token_info.device_id,
    })
}

/// Async version of authenticate for use in async handlers
pub async fn authenticate_async(
    headers: &HeaderMap,
    social_store: &MatrixSocialStore,
) -> Result<AuthenticatedUser, MatrixError> {
    authenticate(headers, social_store)
}

/// 便捷函数：从 AppState 认证
/// 
/// 用于处理函数中快速获取认证用户
pub fn authenticate_from_state(
    headers: &HeaderMap,
    social_store: &MatrixSocialStore,
) -> Result<AuthenticatedUser, MatrixError> {
    authenticate(headers, social_store)
}
