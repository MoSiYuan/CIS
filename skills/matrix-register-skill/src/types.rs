//! # 类型定义

use serde::{Deserialize, Serialize};

// 从 cis_core 重新导出 UserProfile
pub use cis_core::matrix::store_social::UserProfile;

/// 注册请求
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationRequest {
    /// 用户名（DID 格式）
    pub username: Option<String>,
    /// 密码（可选，CIS 可能使用 DID 签名）
    pub password: Option<String>,
    /// 设备 ID
    pub device_id: Option<String>,
    /// 显示名称
    pub display_name: Option<String>,
    /// 头像 URL
    pub avatar_url: Option<String>,
    /// 邀请码
    pub invite_code: Option<String>,
}

/// 注册响应
#[derive(Debug, Clone, Serialize)]
pub struct RegistrationResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
    pub home_server: String,
}

/// 用户信息
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub user_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    pub created_at: i64,
    pub devices: Vec<DeviceInfo>,
}

/// 设备信息
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
}

/// 认证流程
#[derive(Debug, Clone, Serialize)]
pub struct AuthFlow {
    #[serde(rename = "type")]
    pub flow_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<String>>,
}

/// 流程响应（UIAA）
#[derive(Debug, Clone, Serialize)]
pub struct FlowsResponse {
    pub flows: Vec<AuthFlow>,
}

/// 用户名可用性响应
#[derive(Debug, Clone, Serialize)]
pub struct AvailableResponse {
    pub available: bool,
}

/// 更新资料请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProfileRequest {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,
}

/// 检查用户名请求
#[derive(Debug, Clone, Deserialize)]
pub struct CheckUsernameRequest {
    pub username: String,
}

/// 获取用户信息请求
#[derive(Debug, Clone, Deserialize)]
pub struct GetUserInfoRequest {
    pub user_id: String,
}

/// 吊销令牌请求
#[derive(Debug, Clone, Deserialize)]
pub struct RevokeTokenRequest {
    pub token: String,
}

/// 删除用户请求
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteUserRequest {
    pub user_id: String,
}

/// 设备信息请求
#[derive(Debug, Clone, Deserialize)]
pub struct GetDevicesRequest {
    pub user_id: String,
}

/// 删除设备请求
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteDeviceRequest {
    pub user_id: String,
    pub device_id: String,
}
