//! # Cloud Anchor 配置

use serde::{Deserialize, Serialize};

/// Cloud Anchor 配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudAnchorConfig {
    /// 是否启用 Cloud Anchor
    pub enabled: bool,

    /// Cloud Anchor 服务器 URL
    pub server_url: String,

    /// 认证令牌 (可选，用于受保护的端点)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// 心跳间隔（秒）
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,

    /// 是否自动注册
    #[serde(default = "default_auto_register")]
    pub auto_register: bool,

    /// 连接超时（秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,

    /// 请求超时（秒）
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// 最大重试次数
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// 重试间隔（毫秒）
    #[serde(default = "default_retry_interval")]
    pub retry_interval: u64,

    /// 是否启用中继功能
    #[serde(default = "default_enable_relay")]
    pub enable_relay: bool,

    /// 中继配额（字节/月），None 表示无限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_quota_bytes: Option<u64>,

    /// 房间 ID（用于按房间过滤节点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,

    /// 节点标签（用于发现过滤）
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for CloudAnchorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "https://anchor.cis.dev".to_string(),
            auth_token: None,
            heartbeat_interval: default_heartbeat_interval(),
            auto_register: default_auto_register(),
            connect_timeout: default_connect_timeout(),
            request_timeout: default_request_timeout(),
            max_retries: default_max_retries(),
            retry_interval: default_retry_interval(),
            enable_relay: default_enable_relay(),
            relay_quota_bytes: None,
            room_id: None,
            tags: Vec::new(),
        }
    }
}

impl CloudAnchorConfig {
    /// 创建新的配置
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            enabled: true,
            ..Default::default()
        }
    }

    /// 启用 Cloud Anchor
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置认证令牌
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// 设置心跳间隔
    pub fn with_heartbeat_interval(mut self, interval: u64) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    /// 设置自动注册
    pub fn with_auto_register(mut self, auto: bool) -> Self {
        self.auto_register = auto;
        self
    }

    /// 设置连接超时
    pub fn with_connect_timeout(mut self, timeout: u64) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// 设置请求超时
    pub fn with_request_timeout(mut self, timeout: u64) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// 设置最大重试次数
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// 设置重试间隔
    pub fn with_retry_interval(mut self, interval: u64) -> Self {
        self.retry_interval = interval;
        self
    }

    /// 设置是否启用中继
    pub fn with_relay(mut self, enabled: bool) -> Self {
        self.enable_relay = enabled;
        self
    }

    /// 设置中继配额
    pub fn with_relay_quota(mut self, quota: u64) -> Self {
        self.relay_quota_bytes = Some(quota);
        self
    }

    /// 设置房间 ID
    pub fn with_room_id(mut self, room_id: impl Into<String>) -> Self {
        self.room_id = Some(room_id.into());
        self
    }

    /// 添加标签
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 获取完整的 API 基础 URL
    pub fn api_base_url(&self) -> String {
        let url = self.server_url.trim_end_matches('/');
        format!("{}/api/v1", url)
    }

    /// 获取 WebSocket URL
    pub fn websocket_url(&self) -> String {
        let url = self.server_url.trim_end_matches('/');
        url.replace("https://", "wss://")
            .replace("http://", "ws://")
    }
}

fn default_heartbeat_interval() -> u64 {
    60
}

fn default_auto_register() -> bool {
    true
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_request_timeout() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_interval() -> u64 {
    1000
}

fn default_enable_relay() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CloudAnchorConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.server_url, "https://anchor.cis.dev");
        assert_eq!(config.heartbeat_interval, 60);
        assert!(config.auto_register);
        assert_eq!(config.connect_timeout, 10);
        assert_eq!(config.request_timeout, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval, 1000);
        assert!(config.enable_relay);
    }

    #[test]
    fn test_config_builder() {
        let config = CloudAnchorConfig::new("https://my-anchor.example.com")
            .enabled(true)
            .with_auth_token("test-token")
            .with_heartbeat_interval(30)
            .with_auto_register(false)
            .with_room_id("room-123")
            .with_tag("production")
            .with_tag("eu-west");

        assert!(config.enabled);
        assert_eq!(config.server_url, "https://my-anchor.example.com");
        assert_eq!(config.auth_token, Some("test-token".to_string()));
        assert_eq!(config.heartbeat_interval, 30);
        assert!(!config.auto_register);
        assert_eq!(config.room_id, Some("room-123".to_string()));
        assert_eq!(config.tags, vec!["production", "eu-west"]);
    }

    #[test]
    fn test_api_base_url() {
        let config = CloudAnchorConfig::new("https://anchor.cis.dev");
        assert_eq!(config.api_base_url(), "https://anchor.cis.dev/api/v1");

        let config = CloudAnchorConfig::new("https://anchor.cis.dev/");
        assert_eq!(config.api_base_url(), "https://anchor.cis.dev/api/v1");
    }

    #[test]
    fn test_websocket_url() {
        let config = CloudAnchorConfig::new("https://anchor.cis.dev");
        assert_eq!(config.websocket_url(), "wss://anchor.cis.dev");

        let config = CloudAnchorConfig::new("http://anchor.cis.dev");
        assert_eq!(config.websocket_url(), "ws://anchor.cis.dev");
    }

    #[test]
    fn test_serialization() {
        let config = CloudAnchorConfig::new("https://anchor.example.com")
            .with_auth_token("secret")
            .with_room_id("room-1");

        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: CloudAnchorConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.server_url, deserialized.server_url);
        assert_eq!(config.room_id, deserialized.room_id);
        // auth_token 不应该被序列化（因为有 skip_serializing_if）
    }
}
