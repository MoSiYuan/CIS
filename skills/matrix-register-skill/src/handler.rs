//! # 事件处理器
//!
//! 处理来自 SDK 的各种注册相关事件。

use serde_json::Value;

use crate::{MatrixRegisterSkill, types::*, Result};
use cis_core::matrix::store_social::UserProfile;

/// 处理注册请求
pub fn handle_register(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: RegistrationRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    let resp = skill.register_user(req)?;
    
    Ok(serde_json::json!({
        "success": true,
        "user_id": resp.user_id,
        "access_token": resp.access_token,
        "device_id": resp.device_id,
        "home_server": resp.home_server,
    }))
}

/// 处理检查用户名可用性
pub fn handle_check_username(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: CheckUsernameRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    let available = skill.check_username_available(&req.username)?;
    
    Ok(serde_json::json!({
        "available": available,
    }))
}

/// 处理获取用户信息
pub fn handle_get_user_info(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: GetUserInfoRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    match skill.get_user_info(&req.user_id)? {
        Some(info) => {
            Ok(serde_json::json!({
                "found": true,
                "user": info,
            }))
        }
        None => {
            Ok(serde_json::json!({
                "found": false,
            }))
        }
    }
}

/// 处理更新用户资料
pub fn handle_update_profile(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: UpdateProfileRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    let profile = UserProfile {
        display_name: req.display_name,
        avatar_url: req.avatar_url,
        status_msg: req.status_msg,
    };
    
    skill.update_profile(&req.user_id, profile)?;
    
    Ok(serde_json::json!({
        "success": true,
    }))
}

/// 处理获取注册流程
pub fn handle_get_flows(
    skill: &MatrixRegisterSkill,
    _data: Value,
) -> Result<Value> {
    let flows = skill.get_available_flows();
    
    Ok(serde_json::json!({
        "flows": flows,
    }))
}

/// 处理获取用户设备列表
pub fn handle_get_devices(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: GetDevicesRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    let devices = skill.social_store().get_user_devices(&req.user_id)?;
    
    let device_infos: Vec<DeviceInfo> = devices.into_iter()
        .map(|d| DeviceInfo {
            device_id: d.device_id,
            display_name: d.display_name,
            last_seen: d.last_seen,
        })
        .collect();
    
    Ok(serde_json::json!({
        "user_id": req.user_id,
        "devices": device_infos,
    }))
}

/// 处理删除设备
pub fn handle_delete_device(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: DeleteDeviceRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    skill.social_store().delete_device(&req.device_id)?;
    
    Ok(serde_json::json!({
        "success": true,
        "device_id": req.device_id,
    }))
}

/// 处理吊销令牌
pub fn handle_revoke_token(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: RevokeTokenRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    skill.social_store().revoke_token(&req.token)?;
    
    Ok(serde_json::json!({
        "success": true,
    }))
}

/// 处理删除用户
pub fn handle_delete_user(
    skill: &MatrixRegisterSkill,
    data: Value,
) -> Result<Value> {
    let req: DeleteUserRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::RegisterError::Serialization(e.to_string()))?;
    
    skill.social_store().delete_user(&req.user_id)?;
    
    Ok(serde_json::json!({
        "success": true,
        "user_id": req.user_id,
    }))
}

/// 处理获取配置
pub fn handle_get_config(
    skill: &MatrixRegisterSkill,
    _data: Value,
) -> Result<Value> {
    let config = skill.config();
    
    Ok(serde_json::json!({
        "policy": format!("{:?}", config.policy),
        "home_server": config.home_server,
        "allow_multiple_devices": config.allow_multiple_devices,
        "max_devices_per_user": config.max_devices_per_user,
        "require_email_verification": config.require_email_verification,
        "require_phone_verification": config.require_phone_verification,
    }))
}

/// 处理清理过期令牌
pub fn handle_cleanup_tokens(
    skill: &MatrixRegisterSkill,
    _data: Value,
) -> Result<Value> {
    let count = skill.social_store().cleanup_expired_tokens()?;
    
    Ok(serde_json::json!({
        "cleaned": count,
    }))
}

/// 路由处理函数
/// 
/// 根据 action 名称分发到对应的处理器
pub fn handle_action(
    skill: &MatrixRegisterSkill,
    action: &str,
    data: Value,
) -> Result<Value> {
    match action {
        "register" => handle_register(skill, data),
        "check_username" => handle_check_username(skill, data),
        "get_user_info" => handle_get_user_info(skill, data),
        "update_profile" => handle_update_profile(skill, data),
        "get_flows" => handle_get_flows(skill, data),
        "get_devices" => handle_get_devices(skill, data),
        "delete_device" => handle_delete_device(skill, data),
        "revoke_token" => handle_revoke_token(skill, data),
        "delete_user" => handle_delete_user(skill, data),
        "get_config" => handle_get_config(skill, data),
        "cleanup_tokens" => handle_cleanup_tokens(skill, data),
        _ => Err(crate::error::RegisterError::InvalidRequest(
            format!("Unknown action: {}", action)
        )),
    }
}
