//! # Matrix Client-Server Registration API
//!
//! Implements user registration endpoints for Element client compatibility.
//! CIS uses DID-based identity, so registration is essentially "claiming" your DID.
//!
//! ## Architecture Note
//!
//! 注册逻辑已迁移到使用 `MatrixSocialStore`（matrix-social.db），
//! 与协议事件存储分离。这是 Skill 化注册的基础。

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::matrix::{
    error::{MatrixError, MatrixResult},
    store_social::MatrixSocialStore,
};

use super::AppState;

/// Registration request
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    /// Username (DID format: @did:cis:<node_id>:<key>)
    #[serde(rename = "username", default)]
    pub username: Option<String>,
    
    /// Password (optional for CIS, may use DID signature instead)
    #[serde(rename = "password", default)]
    pub password: Option<String>,
    
    /// Device ID
    #[serde(rename = "device_id", default)]
    pub device_id: Option<String>,
    
    /// Initial device display name
    #[serde(rename = "initial_device_display_name", default)]
    pub initial_device_display_name: Option<String>,
    
    /// Authentication flows (for UIAA)
    #[serde(rename = "auth", default)]
    pub auth: Option<serde_json::Value>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    /// User ID (DID)
    pub user_id: String,
    
    /// Access token
    pub access_token: String,
    
    /// Homeserver
    pub home_server: String,
    
    /// Device ID
    pub device_id: String,
}

/// Available flows response (UIAA - User-Interactive Authentication API)
#[derive(Debug, Serialize)]
pub struct FlowsResponse {
    /// Available authentication flows
    pub flows: Vec<AuthFlow>,
}

/// Authentication flow
#[derive(Debug, Serialize)]
pub struct AuthFlow {
    /// Flow type
    #[serde(rename = "type")]
    pub flow_type: String,
    /// Stages (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<String>>,
}

/// Username availability response
#[derive(Debug, Serialize)]
pub struct AvailableResponse {
    pub available: bool,
}

/// Get available registration flows
/// 
/// GET /_matrix/client/v3/register
pub async fn get_register_flows() -> MatrixResult<Json<FlowsResponse>> {
    // CIS uses DID-based identity
    // For now, we support "m.login.dummy" (no auth required) for simplicity
    // In production, this should use DID signature verification
    let flows = FlowsResponse {
        flows: vec![
            AuthFlow {
                flow_type: "m.login.dummy".to_string(),
                stages: None,
            },
        ],
    };
    
    Ok(Json(flows))
}

/// Perform user registration
/// 
/// POST /_matrix/client/v3/register
/// 
/// 使用 `MatrixSocialStore` 进行用户注册，与协议事件存储分离。
/// 这为后续迁移到 Skill 化注册奠定了基础。
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> MatrixResult<Json<RegisterResponse>> {
    let social_store = &state.social_store;
    
    // Validate username format (should be a DID)
    let user_id = request.username.ok_or_else(|| {
        MatrixError::BadRequest("Username is required".to_string())
    })?;
    
    // Validate DID format: @did:cis:<node_id>:<key>
    if !is_valid_did_format(&user_id) {
        return Err(MatrixError::BadRequest(
            "Username must be a valid DID: @did:cis:<node_id>:<key>".to_string()
        ));
    }
    
    // Check if user already exists (使用 social_store)
    if social_store.user_exists(&user_id)? {
        return Err(MatrixError::UserInUse(format!(
            "User {} is already registered", user_id
        )));
    }
    
    // Use device_id from request or generate new one
    let device_id = request.device_id;
    
    // 使用 MatrixSocialStore 的完整注册流程
    let (user_id, access_token, device_id) = social_store.register_user_complete(
        &user_id,
        device_id.as_deref(),
        request.initial_device_display_name.as_deref(),
    )?;
    
    tracing::info!("User registered: {} with device {}", user_id, device_id);
    
    Ok(Json(RegisterResponse {
        user_id,
        access_token,
        home_server: "localhost".to_string(),
        device_id,
    }))
}

/// Check username availability
/// 
/// GET /_matrix/client/v3/register/available?username=xxx
pub async fn check_username_available(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> MatrixResult<Json<AvailableResponse>> {
    let username = params.get("username").ok_or_else(|| {
        MatrixError::BadRequest("username parameter is required".to_string())
    })?;
    
    // Validate DID format
    if !is_valid_did_format(username) {
        return Err(MatrixError::InvalidUsername(
            "Username must be a valid DID format".to_string()
        ));
    }
    
    // 使用 social_store 检查用户是否存在
    let available = !state.social_store.user_exists(username)?;
    
    Ok(Json(AvailableResponse { available }))
}

/// Validate DID format: @did:cis:<node_id>:<key>
fn is_valid_did_format(username: &str) -> bool {
    // Basic validation - starts with @ and has DID structure
    if !username.starts_with('@') {
        return false;
    }
    
    let parts: Vec<&str> = username[1..].split(':').collect();
    
    // Expected: did, cis, <node_id>, <key>
    if parts.len() < 4 {
        return false;
    }
    
    parts[0] == "did" && parts[1] == "cis"
}

/// Generate random access token
fn _generate_access_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const TOKEN_LEN: usize = 64;

    let mut rng = rand::thread_rng();
    (0..TOKEN_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_format_validation() {
        assert!(is_valid_did_format("@did:cis:node1:abc123"));
        assert!(is_valid_did_format("@did:cis:localhost:key1"));
        assert!(!is_valid_did_format("did:cis:node1:abc123")); // Missing @
        assert!(!is_valid_did_format("@user:example.com")); // Not a CIS DID
        assert!(!is_valid_did_format("@did:other:node1:abc")); // Not cis method
        assert!(!is_valid_did_format("@did:cis:node1")); // Missing key part
    }
}
