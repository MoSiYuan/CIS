//! # Matrix Login Endpoint
//!
//! Implements `POST /_matrix/client/v3/login` for user authentication.
//!
//! ## Phase 0 Implementation
//!
//! This is a simplified login that:
//! - Accepts any username/password combination
//! - Creates the user if not exists
//! - Generates a random access token
//!
//! ## Future Improvements
//!
//! - Password hashing and verification
//! - Multiple login types (password, token, SSO)
//! - Rate limiting
//! - Device management

use axum::{extract::State, Json};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::matrix::error::{MatrixError, MatrixResult};
use crate::matrix::store::MatrixStore;

/// Login request body
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum LoginRequest {
    #[serde(rename = "m.login.password")]
    Password {
        identifier: Option<UserIdentifier>,
        user: Option<String>,
        password: String,
        device_id: Option<String>,
        initial_device_display_name: Option<String>,
    },
    #[serde(rename = "m.login.token")]
    Token { token: String },
}

/// User identifier for login
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum UserIdentifier {
    MxidUser { user: String },
    MxidLocalpart { localpart: String },
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub device_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<DiscoveryInfo>,
}

/// Discovery info for well-known
#[derive(Debug, Serialize)]
pub struct DiscoveryInfo {
    #[serde(rename = "m.homeserver")]
    pub homeserver: HomeserverInfo,
}

/// Homeserver info
#[derive(Debug, Serialize)]
pub struct HomeserverInfo {
    pub base_url: String,
}

/// POST /_matrix/client/v3/login
///
/// Authenticate a user and return an access token.
/// Phase 0: Simplified - accepts any username/password.
pub async fn login(
    State(store): State<Arc<MatrixStore>>,
    Json(req): Json<LoginRequest>,
) -> MatrixResult<Json<LoginResponse>> {
    match req {
        LoginRequest::Password {
            identifier,
            user,
            password: _,
            device_id,
            initial_device_display_name,
        } => {
            // Extract user ID from identifier or direct user field
            let user_id = extract_user_id(identifier, user)?;
            
            // Ensure user exists (create if not)
            store.ensure_user(&user_id)?;
            
            // Generate or use provided device ID
            let device_id = device_id.unwrap_or_else(generate_device_id);
            
            // Register the device
            store.register_device(&device_id, &user_id, initial_device_display_name.as_deref())?;
            
            // Generate access token
            let access_token = generate_token();
            
            // Store the token
            store.store_token(&access_token, &user_id, Some(&device_id))?;
            
            Ok(Json(LoginResponse {
                access_token,
                device_id,
                user_id,
                expires_in_ms: None,
                refresh_token: None,
                well_known: None,
            }))
        }
        LoginRequest::Token { .. } => {
            Err(MatrixError::NotImplemented("Token login not implemented".to_string()))
        }
    }
}

/// Extract user ID from identifier or user field
fn extract_user_id(
    identifier: Option<UserIdentifier>,
    user: Option<String>,
) -> MatrixResult<String> {
    if let Some(ident) = identifier {
        match ident {
            UserIdentifier::MxidUser { user } => Ok(user),
            UserIdentifier::MxidLocalpart { localpart } => {
                Ok(format!("@{}:cis.local", localpart))
            }
        }
    } else if let Some(user) = user {
        // If user doesn't start with @, treat as localpart
        if user.starts_with('@') {
            Ok(user)
        } else {
            Ok(format!("@{}:cis.local", user))
        }
    } else {
        Err(MatrixError::InvalidParameter(
            "Missing user identifier".to_string(),
        ))
    }
}

/// Generate a secure random token
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::encode(&bytes)
}

/// Generate a device ID
fn generate_device_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("CIS_{}", hex::encode(&bytes))
}

// Simple base64 encoding for token generation
mod base64 {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(input: &[u8]) -> String {
        let mut result = String::new();
        let chunks = input.chunks_exact(3);
        let remainder = chunks.remainder();

        for chunk in chunks {
            let b = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
            result.push(ALPHABET[((b >> 18) & 0x3F) as usize] as char);
            result.push(ALPHABET[((b >> 12) & 0x3F) as usize] as char);
            result.push(ALPHABET[((b >> 6) & 0x3F) as usize] as char);
            result.push(ALPHABET[(b & 0x3F) as usize] as char);
        }

        match remainder.len() {
            1 => {
                let b = (remainder[0] as u32) << 16;
                result.push(ALPHABET[((b >> 18) & 0x3F) as usize] as char);
                result.push(ALPHABET[((b >> 12) & 0x3F) as usize] as char);
                result.push('=');
                result.push('=');
            }
            2 => {
                let b = ((remainder[0] as u32) << 16) | ((remainder[1] as u32) << 8);
                result.push(ALPHABET[((b >> 18) & 0x3F) as usize] as char);
                result.push(ALPHABET[((b >> 12) & 0x3F) as usize] as char);
                result.push(ALPHABET[((b >> 6) & 0x3F) as usize] as char);
                result.push('=');
            }
            _ => {}
        }

        result
    }
}

// Simple hex encoding
mod hex {
    const HEX_CHARS: &[u8] = b"0123456789abcdef";

    pub fn encode(input: &[u8]) -> String {
        let mut result = String::with_capacity(input.len() * 2);
        for &byte in input {
            result.push(HEX_CHARS[(byte >> 4) as usize] as char);
            result.push(HEX_CHARS[(byte & 0xF) as usize] as char);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_id() {
        // From identifier
        let ident = Some(UserIdentifier::MxidUser {
            user: "@alice:cis.local".to_string(),
        });
        assert_eq!(
            extract_user_id(ident, None).unwrap(),
            "@alice:cis.local"
        );

        // From localpart
        let ident = Some(UserIdentifier::MxidLocalpart {
            localpart: "bob".to_string(),
        });
        assert_eq!(
            extract_user_id(ident, None).unwrap(),
            "@bob:cis.local"
        );

        // From user field (full MXID)
        assert_eq!(
            extract_user_id(None, Some("@charlie:cis.local".to_string())).unwrap(),
            "@charlie:cis.local"
        );

        // From user field (localpart only)
        assert_eq!(
            extract_user_id(None, Some("dave".to_string())).unwrap(),
            "@dave:cis.local"
        );
    }

    #[test]
    fn test_token_generation() {
        let token1 = generate_token();
        let token2 = generate_token();
        
        // Tokens should be unique
        assert_ne!(token1, token2);
        
        // Tokens should be non-empty
        assert!(!token1.is_empty());
        assert!(!token2.is_empty());
    }

    #[test]
    fn test_device_id_generation() {
        let device_id = generate_device_id();
        
        // Should start with CIS_
        assert!(device_id.starts_with("CIS_"));
        
        // Should be non-empty after prefix
        assert!(device_id.len() > 4);
    }
}
