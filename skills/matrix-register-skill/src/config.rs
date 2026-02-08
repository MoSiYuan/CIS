//! # 注册配置
//!
//! 定义注册策略和配置选项。

/// 注册策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationPolicy {
    /// 开放注册
    Open,
    /// 仅邀请注册
    InviteOnly,
    /// 禁用注册
    Disabled,
}

impl Default for RegistrationPolicy {
    fn default() -> Self {
        RegistrationPolicy::Open
    }
}

/// 注册配置
#[derive(Debug, Clone)]
pub struct RegistrationConfig {
    /// 注册策略
    pub policy: RegistrationPolicy,
    /// 有效的邀请码列表
    pub valid_invite_codes: Vec<String>,
    /// 保留的用户名（不允许注册）
    pub reserved_usernames: Vec<String>,
    /// Homeserver 名称
    pub home_server: String,
    /// 默认 token 过期时间（秒）
    pub token_expiry_secs: Option<i64>,
    /// 是否允许多设备
    pub allow_multiple_devices: bool,
    /// 最大设备数量（每个用户）
    pub max_devices_per_user: usize,
    /// 是否需要邮箱验证
    pub require_email_verification: bool,
    /// 是否需要手机号验证
    pub require_phone_verification: bool,
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            policy: RegistrationPolicy::Open,
            valid_invite_codes: Vec::new(),
            reserved_usernames: vec![
                "admin".to_string(),
                "administrator".to_string(),
                "root".to_string(),
                "system".to_string(),
                "support".to_string(),
                "help".to_string(),
                "info".to_string(),
                "postmaster".to_string(),
                "webmaster".to_string(),
                "hostmaster".to_string(),
                "abuse".to_string(),
                "security".to_string(),
                "noc".to_string(),
                "null".to_string(),
                "undefined".to_string(),
                "api".to_string(),
                "www".to_string(),
                "mail".to_string(),
                "ftp".to_string(),
            ],
            home_server: "localhost".to_string(),
            token_expiry_secs: Some(30 * 24 * 60 * 60), // 30 天
            allow_multiple_devices: true,
            max_devices_per_user: 10,
            require_email_verification: false,
            require_phone_verification: false,
        }
    }
}

impl RegistrationConfig {
    /// 创建开放注册配置
    pub fn open() -> Self {
        Self::default()
    }
    
    /// 创建仅邀请注册配置
    pub fn invite_only(invite_codes: Vec<String>) -> Self {
        Self {
            policy: RegistrationPolicy::InviteOnly,
            valid_invite_codes: invite_codes,
            ..Default::default()
        }
    }
    
    /// 创建禁用注册配置
    pub fn disabled() -> Self {
        Self {
            policy: RegistrationPolicy::Disabled,
            ..Default::default()
        }
    }
    
    /// 添加保留用户名
    pub fn with_reserved_username(mut self, username: impl Into<String>) -> Self {
        self.reserved_usernames.push(username.into());
        self
    }
    
    /// 添加邀请码
    pub fn with_invite_code(mut self, code: impl Into<String>) -> Self {
        self.valid_invite_codes.push(code.into());
        self
    }
    
    /// 设置 homeserver
    pub fn with_home_server(mut self, server: impl Into<String>) -> Self {
        self.home_server = server.into();
        self
    }
    
    /// 设置 token 过期时间
    pub fn with_token_expiry(mut self, secs: i64) -> Self {
        self.token_expiry_secs = Some(secs);
        self
    }
    
    /// 禁用 token 过期
    pub fn without_token_expiry(mut self) -> Self {
        self.token_expiry_secs = None;
        self
    }
}
