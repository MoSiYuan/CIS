//! # Matrix Register Skill
//!
//! 提供灵活的 Matrix 用户注册功能，支持：
//! - 开放注册
//! - 邀请码注册
//! - 自定义注册策略
//! - 用户资料管理
//!
//! ## 架构
//!
//! 此 Skill 使用独立的 `matrix-social.db` 存储用户数据，
//! 与 Matrix 协议事件存储分离，允许：
//! 1. 独立备份用户数据
//! 2. 灵活扩展注册策略
//! 3. 卸载人类功能而不影响联邦

pub mod config;
pub mod error;
pub mod handler;
pub mod types;

pub use config::{RegistrationConfig, RegistrationPolicy};
pub use error::{RegisterError, Result};
pub use types::*;

use std::path::Path;
use std::sync::Arc;

use cis_core::matrix::store_social::MatrixSocialStore;

/// Matrix 注册 Skill 主结构
pub struct MatrixRegisterSkill {
    social_store: Arc<MatrixSocialStore>,
    config: RegistrationConfig,
}

impl MatrixRegisterSkill {
    /// 使用指定路径创建 Skill
    pub fn open(db_path: &Path) -> Result<Self> {
        let social_store = MatrixSocialStore::open(db_path.to_str().ok_or_else(|| {
            RegisterError::InvalidPath("Invalid database path".to_string())
        })?)?;
        
        Ok(Self {
            social_store: Arc::new(social_store),
            config: RegistrationConfig::default(),
        })
    }
    
    /// 使用内存存储创建（用于测试）
    pub fn open_in_memory() -> Result<Self> {
        let social_store = MatrixSocialStore::open_in_memory()?;
        
        Ok(Self {
            social_store: Arc::new(social_store),
            config: RegistrationConfig::default(),
        })
    }
    
    /// 使用自定义配置
    pub fn with_config(mut self, config: RegistrationConfig) -> Self {
        self.config = config;
        self
    }
    
    /// 获取社交存储引用
    pub fn social_store(&self) -> &Arc<MatrixSocialStore> {
        &self.social_store
    }
    
    /// 获取配置
    pub fn config(&self) -> &RegistrationConfig {
        &self.config
    }
    
    /// 检查注册是否允许
    pub fn check_registration_allowed(&self, req: &RegistrationRequest) -> Result<bool> {
        // 检查全局策略
        match self.config.policy {
            RegistrationPolicy::Open => {
                // 开放注册，检查保留用户名
                if let Some(ref username) = req.username {
                    let localpart = extract_localpart(username);
                    if self.config.reserved_usernames.contains(&localpart) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            RegistrationPolicy::InviteOnly => {
                // 需要邀请码
                match req.invite_code {
                    Some(ref code) => Ok(self.config.valid_invite_codes.contains(code)),
                    None => Ok(false),
                }
            }
            RegistrationPolicy::Disabled => Ok(false),
        }
    }
    
    /// 注册用户（完整流程）
    pub fn register_user(&self, req: RegistrationRequest) -> Result<RegistrationResponse> {
        // 检查注册策略
        if !self.check_registration_allowed(&req)? {
            return Err(RegisterError::RegistrationNotAllowed);
        }
        
        // 验证 DID 格式
        let user_id = req.username.ok_or_else(|| {
            RegisterError::InvalidRequest("Username is required".to_string())
        })?;
        
        if !is_valid_did_format(&user_id) {
            return Err(RegisterError::InvalidRequest(
                "Username must be a valid DID: @did:cis:<node_id>:<key>".to_string()
            ));
        }
        
        // 检查用户是否已存在
        if self.social_store.user_exists(&user_id)? {
            return Err(RegisterError::UserExists(user_id));
        }
        
        // 创建用户资料
        let profile = UserProfile {
            display_name: req.display_name.clone(),
            avatar_url: req.avatar_url.clone(),
            status_msg: None,
        };
        
        // 执行完整注册流程
        let (user_id, access_token, device_id) = self.social_store.register_user_complete(
            &user_id,
            req.device_id.as_deref(),
            req.display_name.as_deref(),
        )?;
        
        // 更新资料（如果有额外字段）
        if req.avatar_url.is_some() || req.display_name.is_some() {
            self.social_store.update_profile(&user_id, profile)?;
        }
        
        tracing::info!(
            "User registered via Skill: {} with device {}",
            user_id, device_id
        );
        
        Ok(RegistrationResponse {
            user_id,
            access_token,
            device_id,
            home_server: self.config.home_server.clone(),
        })
    }
    
    /// 检查用户名是否可用
    pub fn check_username_available(&self, username: &str) -> Result<bool> {
        // 验证 DID 格式
        if !is_valid_did_format(username) {
            return Err(RegisterError::InvalidRequest(
                "Username must be a valid DID format".to_string()
            ));
        }
        
        // 检查保留用户名
        let localpart = extract_localpart(username);
        if self.config.reserved_usernames.contains(&localpart) {
            return Ok(false);
        }
        
        // 检查是否已存在
        Ok(!self.social_store.user_exists(username)?)
    }
    
    /// 获取用户信息
    pub fn get_user_info(&self, user_id: &str) -> Result<Option<UserInfo>> {
        let user = self.social_store.get_user(user_id)?;
        let profile = self.social_store.get_profile(user_id)?;
        let devices = self.social_store.get_user_devices(user_id)?;
        
        match user {
            Some(user) => {
                Ok(Some(UserInfo {
                    user_id: user.user_id,
                    display_name: profile.as_ref().and_then(|p| p.display_name.clone())
                        .or(user.display_name)
                        .unwrap_or_else(|| user_id.to_string()),
                    avatar_url: profile.and_then(|p| p.avatar_url),
                    created_at: user.created_at,
                    devices: devices.into_iter().map(|d| DeviceInfo {
                        device_id: d.device_id,
                        display_name: d.display_name,
                        last_seen: d.last_seen,
                    }).collect(),
                }))
            }
            None => Ok(None),
        }
    }
    
    /// 更新用户资料
    pub fn update_profile(&self, user_id: &str, profile: UserProfile) -> Result<()> {
        self.social_store.update_profile(user_id, profile)?;
        Ok(())
    }
    
    /// 获取可用注册流程（UIAA）
    pub fn get_available_flows(&self) -> Vec<AuthFlow> {
        match self.config.policy {
            RegistrationPolicy::Open => {
                vec![
                    AuthFlow {
                        flow_type: "m.login.dummy".to_string(),
                        stages: None,
                    },
                ]
            }
            RegistrationPolicy::InviteOnly => {
                vec![
                    AuthFlow {
                        flow_type: "m.login.dummy".to_string(),
                        stages: Some(vec!["invite_code".to_string()]),
                    },
                ]
            }
            RegistrationPolicy::Disabled => vec![],
        }
    }
}

/// 验证 DID 格式: @did:cis:<node_id>:<key>
fn is_valid_did_format(username: &str) -> bool {
    if !username.starts_with('@') {
        return false;
    }
    
    let parts: Vec<&str> = username[1..].split(':').collect();
    
    if parts.len() < 4 {
        return false;
    }
    
    parts[0] == "did" && parts[1] == "cis"
}

/// 提取 localpart 从 DID
fn extract_localpart(username: &str) -> String {
    // 从 DID 格式 @did:cis:<node_id>:<key> 中提取 key 部分
    username
        .trim_start_matches('@')
        .split(':')
        .last()
        .unwrap_or(username)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_skill_creation() {
        let skill = MatrixRegisterSkill::open_in_memory().unwrap();
        assert_eq!(skill.config().policy, RegistrationPolicy::Open);
    }

    #[test]
    fn test_user_registration() {
        let skill = MatrixRegisterSkill::open_in_memory().unwrap();
        
        let req = RegistrationRequest {
            username: Some("@did:cis:test:user1".to_string()),
            password: None,
            device_id: Some("DEVICE001".to_string()),
            display_name: Some("Test User".to_string()),
            avatar_url: None,
            invite_code: None,
        };
        
        let resp = skill.register_user(req).unwrap();
        assert_eq!(resp.user_id, "@did:cis:test:user1");
        assert_eq!(resp.device_id, "DEVICE001");
        assert!(!resp.access_token.is_empty());
        
        // 检查用户已存在
        let req2 = RegistrationRequest {
            username: Some("@did:cis:test:user1".to_string()),
            password: None,
            device_id: None,
            display_name: None,
            avatar_url: None,
            invite_code: None,
        };
        
        assert!(matches!(
            skill.register_user(req2).unwrap_err(),
            RegisterError::UserExists(_)
        ));
    }

    #[test]
    fn test_username_availability() {
        let skill = MatrixRegisterSkill::open_in_memory().unwrap();
        
        // 新用户名可用
        assert!(skill.check_username_available("@did:cis:test:newuser").unwrap());
        
        // 注册后不可用
        let req = RegistrationRequest {
            username: Some("@did:cis:test:existing".to_string()),
            password: None,
            device_id: None,
            display_name: None,
            avatar_url: None,
            invite_code: None,
        };
        skill.register_user(req).unwrap();
        
        assert!(!skill.check_username_available("@did:cis:test:existing").unwrap());
    }

    #[test]
    fn test_invite_only_policy() {
        let skill = MatrixRegisterSkill::open_in_memory()
            .unwrap()
            .with_config(RegistrationConfig {
                policy: RegistrationPolicy::InviteOnly,
                valid_invite_codes: vec!["SECRET123".to_string()],
                ..Default::default()
            });
        
        // 无邀请码不允许
        let req = RegistrationRequest {
            username: Some("@did:cis:test:invited".to_string()),
            password: None,
            device_id: None,
            display_name: None,
            avatar_url: None,
            invite_code: None,
        };
        
        assert!(matches!(
            skill.register_user(req).unwrap_err(),
            RegisterError::RegistrationNotAllowed
        ));
        
        // 有效邀请码允许
        let req = RegistrationRequest {
            username: Some("@did:cis:test:invited2".to_string()),
            password: None,
            device_id: None,
            display_name: None,
            avatar_url: None,
            invite_code: Some("SECRET123".to_string()),
        };
        
        let resp = skill.register_user(req).unwrap();
        assert_eq!(resp.user_id, "@did:cis:test:invited2");
    }

    #[test]
    fn test_reserved_usernames() {
        let skill = MatrixRegisterSkill::open_in_memory()
            .unwrap()
            .with_config(RegistrationConfig {
                reserved_usernames: vec!["admin".to_string(), "root".to_string()],
                ..Default::default()
            });
        
        assert!(!skill.check_username_available("@did:cis:test:admin").unwrap());
        assert!(!skill.check_username_available("@did:cis:test:root").unwrap());
        assert!(skill.check_username_available("@did:cis:test:normal").unwrap());
    }
}
