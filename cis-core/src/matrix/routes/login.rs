//! # Matrix Login Endpoint
//!
//! Implements `POST /_matrix/client/v3/login` for user authentication.
//!
//! ## Architecture Note
//!
//! 登录逻辑使用 `MatrixSocialStore`（matrix-social.db）进行用户验证和令牌管理，
//! 与协议事件存储分离。
//!
//! ## Verification Code Flow
//!
//! 首次登录的用户需要验证码（类似节点配对）：
//! 1. 用户在 Element 客户端输入用户名和任意密码
//! 2. 后端检测到是新用户/需要验证，在日志中输出验证码
//! 3. 用户再次登录，密码字段输入特殊格式 `otp:123456` 传递验证码
//! 4. 验证通过后，用户后续登录不再需要验证码（密码任意）

use axum::{extract::State, Json};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::matrix::error::{MatrixError, MatrixResult};

use super::AppState;

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
/// 使用 MatrixSocialStore 进行用户管理和令牌生成。
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> MatrixResult<Json<LoginResponse>> {
    let social_store = &state.social_store;
    
    match req {
        LoginRequest::Password {
            identifier,
            user,
            password,
            device_id,
            initial_device_display_name,
        } => {
            // Extract user ID from identifier or direct user field
            let user_id = extract_user_id(identifier, user)?;
            
            // Check if user needs verification code
            let needs_verification = social_store.needs_verification_code(&user_id)?;
            
            if needs_verification {
                // Check if password contains verification code (format: "otp:123456")
                if let Some(code) = extract_otp_code(&password) {
                    // Verify the code
                    match social_store.verify_login_code(&user_id, &code) {
                        Ok(true) => {
                            tracing::info!(user_id, "Login verification code accepted");
                        }
                        Ok(false) => {
                            return Err(MatrixError::Forbidden(
                                "Invalid verification code".to_string()
                            ));
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                } else {
                    // User needs verification but didn't provide code
                    // Generate a new code and return error
                    let (code, is_new) = social_store.generate_login_code(&user_id)?;
                    
                    if is_new {
                        tracing::info!(
                            user_id,
                            code = %code,
                            "New user login - verification code generated (expires in 5 minutes)"
                        );
                    } else {
                        tracing::info!(
                            user_id,
                            code = %code,
                            "User login requires verification - code generated (expires in 5 minutes)"
                        );
                    }
                    
                    return Err(MatrixError::Forbidden(
                        "Login requires verification code. Please check server logs for the 6-digit code and retry with password 'otp:XXXXXX'".to_string()
                    ));
                }
            }
            
            // Ensure user exists (create if not) using social_store
            if !social_store.user_exists(&user_id)? {
                social_store.create_user(&user_id, None)?;
            }
            
            // Generate or use provided device ID
            let device_id = device_id.unwrap_or_else(generate_device_id);
            
            // 如果没有提供设备显示名称，使用设备名（hostname）作为默认值
            let device_display_name = initial_device_display_name.unwrap_or_else(|| {
                gethostname::gethostname().to_string_lossy().to_string()
            });
            
            // Register the device using social_store
            social_store.register_device(
                &device_id, 
                &user_id, 
                Some(&device_display_name),
                None, // ip_address
            )?;
            
            // Generate and store access token using social_store
            let access_token = social_store.create_token(&user_id, Some(&device_id), None)?;
            
            tracing::info!(user_id, device_id, "User logged in successfully");
            
            Ok(Json(LoginResponse {
                access_token,
                device_id,
                user_id,
                expires_in_ms: None,
                refresh_token: None,
                well_known: None,
            }))
        }
        LoginRequest::Token { token } => {
            // Token login: validate token and get user_id
            match social_store.validate_token(&token)? {
                Some(info) => {
                    Ok(Json(LoginResponse {
                        access_token: token,
                        device_id: info.device_id.unwrap_or_default(),
                        user_id: info.user_id,
                        expires_in_ms: None,
                        refresh_token: None,
                        well_known: None,
                    }))
                }
                None => Err(MatrixError::Unauthorized("Invalid token".to_string())),
            }
        }
    }
}

/// Extract OTP code from password field
/// Format: "otp:123456" or "OTP:123456"
fn extract_otp_code(password: &str) -> Option<String> {
    let password = password.trim();
    if password.len() == 10 {
        // Check for "otp:123456" format (case insensitive)
        if password[..4].eq_ignore_ascii_case("otp:") {
            let code = &password[4..];
            // Verify it's exactly 6 digits
            if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
                return Some(code.to_string());
            }
        }
    }
    None
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

/// Generate a device ID
fn generate_device_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("CIS_{}", hex::encode(&bytes))
}

/// Simple hex encoding
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_id() {
        // Test with full MXID
        let ident = Some(UserIdentifier::MxidUser { 
            user: "@test:cis.local".to_string() 
        });
        assert_eq!(
            extract_user_id(ident, None).unwrap(),
            "@test:cis.local"
        );

        // Test with localpart
        let ident = Some(UserIdentifier::MxidLocalpart { 
            localpart: "alice".to_string() 
        });
        assert_eq!(
            extract_user_id(ident, None).unwrap(),
            "@alice:cis.local"
        );

        // Test with user field
        assert_eq!(
            extract_user_id(None, Some("@bob:cis.local".to_string())).unwrap(),
            "@bob:cis.local"
        );

        // Test with localpart in user field
        assert_eq!(
            extract_user_id(None, Some("charlie".to_string())).unwrap(),
            "@charlie:cis.local"
        );
    }
}
