//! # Matrix Client-Server Registration API
//!
//! Implements user registration endpoints for Element client compatibility.
//! CIS uses DID-based identity, so registration is essentially "claiming" your DID.

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::matrix::{
    error::{MatrixError, MatrixResult},
    store::MatrixStore,
};

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
pub async fn register(
    State(store): State<Arc<MatrixStore>>,
    Json(request): Json<RegisterRequest>,
) -> MatrixResult<Json<RegisterResponse>> {
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
    
    // Check if user already exists
    if store.user_exists(&user_id)? {
        return Err(MatrixError::UserInUse(format!(
            "User {} is already registered", user_id
        )));
    }
    
    // Generate device ID if not provided
    let device_id = request.device_id.unwrap_or_else(|| {
        format!("CIS{}", uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase())
    });
    
    // Generate access token
    let access_token = generate_access_token();
    
    // Register the user in store
    store.register_user(&user_id, &access_token, &device_id)?;
    
    // Register device if display name provided
    if let Some(display_name) = request.initial_device_display_name {
        store.register_device(&device_id, &user_id, Some(&display_name))?;
    }
    
    tracing::info!("User registered: {} with device {}", user_id, device_id);
    
    Ok(Json(RegisterResponse {
        user_id: user_id.clone(),
        access_token,
        home_server: "localhost".to_string(),
        device_id,
    }))
}

/// Check username availability
/// 
/// GET /_matrix/client/v3/register/available?username=xxx
pub async fn check_username_available(
    State(store): State<Arc<MatrixStore>>,
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
    
    let available = !store.user_exists(username)?;
    
    Ok(Json(AvailableResponse { available }))
}

/// Validate DID format: @did:cis:<node_id>:<key>
fn is_valid_did_format(username: &str) -> bool {
    // Basic validation - starts with @ and has DID structure
    if !username.starts_with('@') {
        return false;
    }
    
    let parts: Vec<&str> = username[1..].split(':').collect();
    
    // Should be: did:cis:<node_id>:<key>
    if parts.len() != 4 {
        return false;
    }
    
    parts[0] == "did" && parts[1] == "cis"
}

/// Generate a random access token
fn generate_access_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let token: String = (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    token
}

/// Get registration fallback URL
/// 
/// GET /_matrix/client/v3/register/{login_type}/fallback/web
pub async fn register_fallback() -> MatrixResult<Json<serde_json::Value>> {
    // Return a simple HTML page or redirect URL
    Ok(Json(serde_json::json!({
        "error": "Fallback registration not implemented",
        "errcode": "M_NOT_FOUND"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_did_format() {
        assert!(is_valid_did_format("@did:cis:abc123:def456"));
        assert!(is_valid_did_format("@did:cis:node1:key123"));
    }

    #[test]
    fn test_invalid_did_format() {
        assert!(!is_valid_did_format("did:cis:abc123:def456")); // Missing @
        assert!(!is_valid_did_format("@user:example.com")); // Not a DID
        assert!(!is_valid_did_format("@did:other:abc123:def456")); // Wrong method
        assert!(!is_valid_did_format("@did:cis:onlythree")); // Missing part
    }
}
