//! # Security Configuration
//!
//! Security-related configuration including access control, rate limiting, and command restrictions.

use serde::{Deserialize, Serialize};

use super::{validation_error, ValidateConfig};
use crate::error::Result;

/// Default maximum request size in bytes (10 MB)
pub const DEFAULT_MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;

/// Default rate limit (requests per minute)
pub const DEFAULT_RATE_LIMIT: u32 = 100;

/// Default rate limit burst size
pub const DEFAULT_RATE_LIMIT_BURST: u32 = 10;

/// Default authentication token expiration in hours
pub const DEFAULT_TOKEN_EXPIRATION_HOURS: u64 = 24;

/// Default session timeout in minutes
pub const DEFAULT_SESSION_TIMEOUT_MINUTES: u64 = 30;

/// Default password minimum length
pub const DEFAULT_MIN_PASSWORD_LENGTH: usize = 8;

/// Default number of failed login attempts before lockout
pub const DEFAULT_MAX_LOGIN_ATTEMPTS: u32 = 5;

/// Default lockout duration in minutes
pub const DEFAULT_LOCKOUT_DURATION_MINUTES: u64 = 15;

/// Default token refresh threshold in minutes (refresh if less than this time remains)
pub const DEFAULT_TOKEN_REFRESH_THRESHOLD_MINUTES: u64 = 5;

/// Security configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Command whitelist (empty means allow all)
    #[serde(default)]
    pub command_whitelist: Vec<String>,

    /// Maximum request size in bytes
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize,

    /// Rate limit: requests per minute per IP
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,

    /// Rate limit burst size (allow burst of requests)
    #[serde(default = "default_rate_limit_burst")]
    pub rate_limit_burst: u32,

    /// Authentication token expiration time
    #[serde(default = "default_token_expiration")]
    pub token_expiration: std::time::Duration,

    /// Session timeout for inactive sessions
    #[serde(default = "default_session_timeout")]
    pub session_timeout: std::time::Duration,

    /// Minimum password length
    #[serde(default = "default_min_password_length")]
    pub min_password_length: usize,

    /// Require strong passwords (mixed case, numbers, symbols)
    #[serde(default = "default_require_strong_password")]
    pub require_strong_password: bool,

    /// Maximum failed login attempts before lockout
    #[serde(default = "default_max_login_attempts")]
    pub max_login_attempts: u32,

    /// Account lockout duration
    #[serde(default = "default_lockout_duration")]
    pub lockout_duration: std::time::Duration,

    /// Token refresh threshold (refresh if less than this time remains)
    #[serde(default = "default_token_refresh_threshold")]
    pub token_refresh_threshold: std::time::Duration,

    /// Enable audit logging
    #[serde(default = "default_audit_logging_enabled")]
    pub audit_logging_enabled: bool,

    /// Audit log retention days
    #[serde(default = "default_audit_retention_days")]
    pub audit_retention_days: u32,

    /// Allowed CORS origins
    #[serde(default)]
    pub allowed_origins: Vec<String>,

    /// Require HTTPS for API access
    #[serde(default)]
    pub require_https: bool,

    /// HSTS max age in seconds
    #[serde(default = "default_hsts_max_age")]
    pub hsts_max_age: u32,

    /// Content Security Policy header
    #[serde(default = "default_csp_policy")]
    pub csp_policy: String,

    /// Allowed DID methods for authentication
    #[serde(default = "default_allowed_did_methods")]
    pub allowed_did_methods: Vec<String>,

    /// Enable DID verification
    #[serde(default = "default_did_verification_enabled")]
    pub did_verification_enabled: bool,

    /// Network access control list
    #[serde(default)]
    pub acl: AclConfig,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            command_whitelist: Vec::new(),
            max_request_size: default_max_request_size(),
            rate_limit: default_rate_limit(),
            rate_limit_burst: default_rate_limit_burst(),
            token_expiration: default_token_expiration(),
            session_timeout: default_session_timeout(),
            min_password_length: default_min_password_length(),
            require_strong_password: default_require_strong_password(),
            max_login_attempts: default_max_login_attempts(),
            lockout_duration: default_lockout_duration(),
            token_refresh_threshold: default_token_refresh_threshold(),
            audit_logging_enabled: default_audit_logging_enabled(),
            audit_retention_days: default_audit_retention_days(),
            allowed_origins: Vec::new(),
            require_https: false,
            hsts_max_age: default_hsts_max_age(),
            csp_policy: default_csp_policy(),
            allowed_did_methods: default_allowed_did_methods(),
            did_verification_enabled: default_did_verification_enabled(),
            acl: AclConfig::default(),
        }
    }
}

impl ValidateConfig for SecurityConfig {
    fn validate(&self) -> Result<()> {
        // Validate max request size
        if self.max_request_size == 0 {
            return Err(validation_error("max_request_size cannot be zero"));
        }
        if self.max_request_size > 100 * 1024 * 1024 {
            // Max 100 MB
            return Err(validation_error(
                "max_request_size cannot exceed 100 MB",
            ));
        }

        // Validate rate limit
        if self.rate_limit == 0 {
            return Err(validation_error("rate_limit cannot be zero"));
        }
        if self.rate_limit > 10000 {
            return Err(validation_error("rate_limit cannot exceed 10000"));
        }

        // Validate rate limit burst
        if self.rate_limit_burst == 0 {
            return Err(validation_error("rate_limit_burst cannot be zero"));
        }
        if self.rate_limit_burst > self.rate_limit {
            return Err(validation_error(
                "rate_limit_burst cannot exceed rate_limit",
            ));
        }

        // Validate timeouts are not zero
        if self.token_expiration.is_zero() {
            return Err(validation_error("token_expiration cannot be zero"));
        }
        if self.session_timeout.is_zero() {
            return Err(validation_error("session_timeout cannot be zero"));
        }
        if self.lockout_duration.is_zero() {
            return Err(validation_error("lockout_duration cannot be zero"));
        }
        if self.token_refresh_threshold.is_zero() {
            return Err(validation_error(
                "token_refresh_threshold cannot be zero",
            ));
        }

        // Validate password length
        if self.min_password_length < 6 {
            return Err(validation_error(
                "min_password_length must be at least 6",
            ));
        }
        if self.min_password_length > 128 {
            return Err(validation_error(
                "min_password_length cannot exceed 128",
            ));
        }

        // Validate max login attempts
        if self.max_login_attempts == 0 {
            return Err(validation_error("max_login_attempts cannot be zero"));
        }
        if self.max_login_attempts > 100 {
            return Err(validation_error(
                "max_login_attempts cannot exceed 100",
            ));
        }

        // Validate audit retention
        if self.audit_retention_days == 0 {
            return Err(validation_error("audit_retention_days cannot be zero"));
        }
        if self.audit_retention_days > 3650 {
            // Max 10 years
            return Err(validation_error(
                "audit_retention_days cannot exceed 3650",
            ));
        }

        // Validate HSTS max age
        if self.require_https && self.hsts_max_age == 0 {
            return Err(validation_error(
                "hsts_max_age cannot be zero when require_https is enabled",
            ));
        }

        // Validate allowed DID methods
        if self.did_verification_enabled && self.allowed_did_methods.is_empty() {
            return Err(validation_error(
                "allowed_did_methods cannot be empty when did_verification is enabled",
            ));
        }

        // Validate ACL
        self.acl.validate()?;

        Ok(())
    }
}

/// Access Control List configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AclConfig {
    /// Enable ACL
    #[serde(default)]
    pub enabled: bool,

    /// Default policy (allow or deny)
    #[serde(default = "default_acl_policy")]
    pub default_policy: String,

    /// Allowed IP addresses/networks (CIDR notation)
    #[serde(default)]
    pub allowed_ips: Vec<String>,

    /// Denied IP addresses/networks (CIDR notation)
    #[serde(default)]
    pub denied_ips: Vec<String>,

    /// Allowed DID patterns
    #[serde(default)]
    pub allowed_dids: Vec<String>,

    /// Denied DID patterns
    #[serde(default)]
    pub denied_dids: Vec<String>,
}

impl Default for AclConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_policy: default_acl_policy(),
            allowed_ips: Vec::new(),
            denied_ips: Vec::new(),
            allowed_dids: Vec::new(),
            denied_dids: Vec::new(),
        }
    }
}

impl ValidateConfig for AclConfig {
    fn validate(&self) -> Result<()> {
        if self.enabled {
            // Validate default policy
            let valid_policies = ["allow", "deny"];
            if !valid_policies.contains(&self.default_policy.to_lowercase().as_str()) {
                return Err(validation_error(format!(
                    "Invalid ACL default_policy: {}. Must be 'allow' or 'deny'",
                    self.default_policy
                )));
            }

            // Validate IP addresses/networks in CIDR format
            for ip in &self.allowed_ips {
                if ip.parse::<ipnetwork::IpNetwork>().is_err() {
                    return Err(validation_error(format!(
                        "Invalid allowed IP/network: {}",
                        ip
                    )));
                }
            }

            for ip in &self.denied_ips {
                if ip.parse::<ipnetwork::IpNetwork>().is_err() {
                    return Err(validation_error(format!(
                        "Invalid denied IP/network: {}",
                        ip
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Encryption configuration for data at rest
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EncryptionConfig {
    /// Enable data encryption at rest
    #[serde(default)]
    pub enabled: bool,

    /// Encryption algorithm (aes-256-gcm, chacha20-poly1305)
    #[serde(default = "default_encryption_algorithm")]
    pub algorithm: String,

    /// Key derivation function (argon2id)
    #[serde(default = "default_kdf")]
    pub kdf: String,

    /// Key file path (if not specified, derive from password)
    #[serde(default)]
    pub key_file: Option<std::path::PathBuf>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            algorithm: default_encryption_algorithm(),
            kdf: default_kdf(),
            key_file: None,
        }
    }
}

impl ValidateConfig for EncryptionConfig {
    fn validate(&self) -> Result<()> {
        if self.enabled {
            // Validate encryption algorithm
            let valid_algorithms = ["aes-256-gcm", "aes-192-gcm", "aes-128-gcm", "chacha20-poly1305"];
            if !valid_algorithms.contains(&self.algorithm.to_lowercase().as_str()) {
                return Err(validation_error(format!(
                    "Invalid encryption algorithm: {}. Must be one of: {:?}",
                    self.algorithm, valid_algorithms
                )));
            }

            // Validate KDF
            let valid_kdfs = ["argon2id", "pbkdf2", "scrypt"];
            if !valid_kdfs.contains(&self.kdf.to_lowercase().as_str()) {
                return Err(validation_error(format!(
                    "Invalid KDF: {}. Must be one of: {:?}",
                    self.kdf, valid_kdfs
                )));
            }
        }

        Ok(())
    }
}

// Default value functions
fn default_max_request_size() -> usize {
    DEFAULT_MAX_REQUEST_SIZE
}

fn default_rate_limit() -> u32 {
    DEFAULT_RATE_LIMIT
}

fn default_rate_limit_burst() -> u32 {
    DEFAULT_RATE_LIMIT_BURST
}

fn default_token_expiration() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_TOKEN_EXPIRATION_HOURS * 3600)
}

fn default_session_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_SESSION_TIMEOUT_MINUTES * 60)
}

fn default_min_password_length() -> usize {
    DEFAULT_MIN_PASSWORD_LENGTH
}

fn default_require_strong_password() -> bool {
    true
}

fn default_max_login_attempts() -> u32 {
    DEFAULT_MAX_LOGIN_ATTEMPTS
}

fn default_lockout_duration() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_LOCKOUT_DURATION_MINUTES * 60)
}

fn default_token_refresh_threshold() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_TOKEN_REFRESH_THRESHOLD_MINUTES * 60)
}

fn default_audit_logging_enabled() -> bool {
    true
}

fn default_audit_retention_days() -> u32 {
    90
}

fn default_hsts_max_age() -> u32 {
    31536000 // 1 year
}

fn default_csp_policy() -> String {
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';".to_string()
}

fn default_allowed_did_methods() -> Vec<String> {
    vec![
        "key".to_string(),
        "web".to_string(),
        "ethr".to_string(),
    ]
}

fn default_did_verification_enabled() -> bool {
    true
}

fn default_acl_policy() -> String {
    "deny".to_string()
}

fn default_encryption_algorithm() -> String {
    "aes-256-gcm".to_string()
}

fn default_kdf() -> String {
    "argon2id".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.command_whitelist.is_empty());
        assert_eq!(config.max_request_size, 10 * 1024 * 1024);
        assert_eq!(config.rate_limit, 100);
        assert_eq!(config.rate_limit_burst, 10);
        assert_eq!(config.min_password_length, 8);
        assert!(config.require_strong_password);
        assert_eq!(config.max_login_attempts, 5);
        assert!(config.audit_logging_enabled);
        assert_eq!(config.audit_retention_days, 90);
        assert!(!config.require_https);
    }

    #[test]
    fn test_security_config_validate_success() {
        let config = SecurityConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_security_config_validate_zero_max_request_size() {
        let mut config = SecurityConfig::default();
        config.max_request_size = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_request_size"));
    }

    #[test]
    fn test_security_config_validate_max_request_size_too_large() {
        let mut config = SecurityConfig::default();
        config.max_request_size = 101 * 1024 * 1024; // 101 MB
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_request_size"));
    }

    #[test]
    fn test_security_config_validate_burst_exceeds_limit() {
        let mut config = SecurityConfig::default();
        config.rate_limit = 10;
        config.rate_limit_burst = 20;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rate_limit_burst"));
    }

    #[test]
    fn test_security_config_validate_password_too_short() {
        let mut config = SecurityConfig::default();
        config.min_password_length = 5;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("min_password_length"));
    }

    #[test]
    fn test_security_config_validate_zero_max_login_attempts() {
        let mut config = SecurityConfig::default();
        config.max_login_attempts = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_login_attempts"));
    }

    #[test]
    fn test_acl_config_default() {
        let config = AclConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_policy, "deny");
        assert!(config.allowed_ips.is_empty());
        assert!(config.denied_ips.is_empty());
    }

    #[test]
    fn test_acl_config_validate_disabled() {
        let config = AclConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_acl_config_validate_invalid_policy() {
        let mut config = AclConfig::default();
        config.enabled = true;
        config.default_policy = "invalid".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("default_policy"));
    }

    #[test]
    fn test_acl_config_validate_valid_policies() {
        for policy in ["allow", "deny", "ALLOW", "DENY"] {
            let mut config = AclConfig::default();
            config.enabled = true;
            config.default_policy = policy.to_string();
            
            assert!(config.validate().is_ok(), "Policy {} should be valid", policy);
        }
    }

    #[test]
    fn test_acl_config_validate_invalid_ip() {
        let mut config = AclConfig::default();
        config.enabled = true;
        config.allowed_ips = vec!["invalid-ip".to_string()];
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid allowed IP"));
    }

    #[test]
    fn test_acl_config_validate_valid_ips() {
        let mut config = AclConfig::default();
        config.enabled = true;
        config.allowed_ips = vec![
            "192.168.1.0/24".to_string(),
            "10.0.0.1".to_string(),
            "::1/128".to_string(),
        ];
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_encryption_config_validate() {
        let config = EncryptionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_encryption_config_validate_enabled() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_encryption_config_validate_invalid_algorithm() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        config.algorithm = "invalid".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("algorithm"));
    }

    #[test]
    fn test_security_config_serialize() {
        let config = SecurityConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("max_request_size"));
        assert!(toml.contains("rate_limit"));
    }

    #[test]
    fn test_security_config_deserialize() {
        let toml = r#"
            max_request_size = 5242880
            rate_limit = 50
            min_password_length = 12
            require_strong_password = true
        "#;
        let config: SecurityConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_request_size, 5242880);
        assert_eq!(config.rate_limit, 50);
        assert_eq!(config.min_password_length, 12);
        assert!(config.require_strong_password);
    }
}
