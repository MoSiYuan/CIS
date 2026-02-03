//! # Cloud Anchor 错误类型

use thiserror::Error;

/// Cloud Anchor 操作结果类型
pub type CloudAnchorResult<T> = std::result::Result<T, CloudAnchorError>;

/// Cloud Anchor 错误类型
#[derive(Error, Debug, Clone)]
pub enum CloudAnchorError {
    /// HTTP 请求错误
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// 网络连接错误
    #[error("Network error: {0}")]
    Network(String),

    /// 认证错误
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// 注册错误
    #[error("Registration failed: {0}")]
    Registration(String),

    /// 发现错误
    #[error("Discovery failed: {0}")]
    Discovery(String),

    /// 打洞协调错误
    #[error("Hole punch coordination failed: {0}")]
    HolePunch(String),

    /// 中继错误
    #[error("Relay failed: {0}")]
    Relay(String),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// 未找到节点
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// 服务器错误
    #[error("Server error: {status} - {message}")]
    ServerError { status: u16, message: String },

    /// 超时错误
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// 配额超限
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    /// 未初始化
    #[error("Cloud Anchor client not initialized")]
    NotInitialized,

    /// 已存在
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// 其他错误
    #[error("Cloud Anchor error: {0}")]
    Other(String),
}

impl CloudAnchorError {
    /// 创建 HTTP 错误
    pub fn http(msg: impl Into<String>) -> Self {
        Self::Http(msg.into())
    }

    /// 创建网络错误
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    /// 创建认证错误
    pub fn authentication(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    /// 创建注册错误
    pub fn registration(msg: impl Into<String>) -> Self {
        Self::Registration(msg.into())
    }

    /// 创建发现错误
    pub fn discovery(msg: impl Into<String>) -> Self {
        Self::Discovery(msg.into())
    }

    /// 创建打洞协调错误
    pub fn hole_punch(msg: impl Into<String>) -> Self {
        Self::HolePunch(msg.into())
    }

    /// 创建中继错误
    pub fn relay(msg: impl Into<String>) -> Self {
        Self::Relay(msg.into())
    }

    /// 创建配置错误
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    /// 创建序列化错误
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// 创建节点未找到错误
    pub fn node_not_found(node_id: impl Into<String>) -> Self {
        Self::NodeNotFound(node_id.into())
    }

    /// 创建超时错误
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// 创建配额超限错误
    pub fn quota_exceeded(msg: impl Into<String>) -> Self {
        Self::QuotaExceeded(msg.into())
    }

    /// 创建已存在错误
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::AlreadyExists(msg.into())
    }

    /// 创建其他错误
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// 判断是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Http(_)
                | Self::Network(_)
                | Self::Timeout(_)
                | Self::ServerError { .. }
        )
    }

    /// 判断是否需要重新注册
    pub fn needs_reregistration(&self) -> bool {
        matches!(self, Self::Authentication(_) | Self::Registration(_))
    }
}

impl From<reqwest::Error> for CloudAnchorError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout(e.to_string())
        } else if e.is_connect() {
            Self::Network(e.to_string())
        } else {
            Self::Http(e.to_string())
        }
    }
}

impl From<serde_json::Error> for CloudAnchorError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for CloudAnchorError {
    fn from(e: std::io::Error) -> Self {
        Self::Network(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = CloudAnchorError::http("connection failed");
        assert!(matches!(err, CloudAnchorError::Http(_)));
        assert!(err.is_retryable());

        let err = CloudAnchorError::authentication("invalid token");
        assert!(matches!(err, CloudAnchorError::Authentication(_)));
        assert!(!err.is_retryable());
        assert!(err.needs_reregistration());
    }

    #[test]
    fn test_retryable_errors() {
        assert!(CloudAnchorError::Http("test".to_string()).is_retryable());
        assert!(CloudAnchorError::Network("test".to_string()).is_retryable());
        assert!(CloudAnchorError::Timeout("test".to_string()).is_retryable());
        assert!(CloudAnchorError::ServerError {
            status: 500,
            message: "error".to_string()
        }
        .is_retryable());

        assert!(
            !CloudAnchorError::Authentication("test".to_string()).is_retryable()
        );
        assert!(!CloudAnchorError::node_not_found("test").is_retryable());
    }
}
