//! # Network Configuration
//!
//! Network-related configuration including TCP/UDP ports, bind addresses, and TLS settings.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{validation_error, validate_port, ValidateConfig};
use crate::error::Result;

/// Default TCP port for CIS communication
pub const DEFAULT_TCP_PORT: u16 = 6767;

/// Default UDP port for CIS discovery
pub const DEFAULT_UDP_PORT: u16 = 7677;

/// Default HTTP API port
pub const DEFAULT_HTTP_PORT: u16 = 8080;

/// Default bind address
pub const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";

/// Default connection timeout in seconds
pub const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 30;

/// Default request timeout in seconds
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 60;

/// Default keep-alive interval in seconds
pub const DEFAULT_KEEP_ALIVE_SECS: u64 = 30;

/// Default WebSocket port
pub const DEFAULT_WEBSOCKET_PORT: u16 = 6768;

/// Network configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    /// TCP port for peer-to-peer communication
    #[serde(default = "default_tcp_port")]
    pub tcp_port: u16,

    /// UDP port for discovery
    #[serde(default = "default_udp_port")]
    pub udp_port: u16,

    /// HTTP API port
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// WebSocket port for real-time communication
    #[serde(default = "default_websocket_port")]
    pub websocket_port: u16,

    /// Bind address (0.0.0.0 for all interfaces, 127.0.0.1 for localhost only)
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Connection timeout
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: std::time::Duration,

    /// Request timeout
    #[serde(default = "default_request_timeout")]
    pub request_timeout: std::time::Duration,

    /// Keep-alive interval
    #[serde(default = "default_keep_alive")]
    pub keep_alive: std::time::Duration,

    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// TLS configuration
    #[serde(default)]
    pub tls: TlsConfig,

    /// CORS allowed origins (empty means allow all in development)
    #[serde(default)]
    pub cors_origins: Vec<String>,

    /// Rate limiting: requests per minute per IP
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            tcp_port: default_tcp_port(),
            udp_port: default_udp_port(),
            http_port: default_http_port(),
            websocket_port: default_websocket_port(),
            bind_address: default_bind_address(),
            connection_timeout: default_connection_timeout(),
            request_timeout: default_request_timeout(),
            keep_alive: default_keep_alive(),
            max_connections: default_max_connections(),
            tls: TlsConfig::default(),
            cors_origins: Vec::new(),
            rate_limit: default_rate_limit(),
        }
    }
}

impl ValidateConfig for NetworkConfig {
    fn validate(&self) -> Result<()> {
        // Validate ports
        validate_port(self.tcp_port, "tcp_port")?;
        validate_port(self.udp_port, "udp_port")?;
        validate_port(self.http_port, "http_port")?;
        validate_port(self.websocket_port, "websocket_port")?;

        // Ensure all ports are different
        let ports = [
            self.tcp_port,
            self.udp_port,
            self.http_port,
            self.websocket_port,
        ];
        for (i, port1) in ports.iter().enumerate() {
            for (j, port2) in ports.iter().enumerate() {
                if i < j && port1 == port2 {
                    return Err(validation_error(format!(
                        "Network ports must be unique: port {} is used multiple times",
                        port1
                    )));
                }
            }
        }

        // Validate bind address
        if self.bind_address.is_empty() {
            return Err(validation_error("bind_address cannot be empty"));
        }

        // Validate timeouts are not zero
        if self.connection_timeout.is_zero() {
            return Err(validation_error("connection_timeout cannot be zero"));
        }
        if self.request_timeout.is_zero() {
            return Err(validation_error("request_timeout cannot be zero"));
        }
        if self.keep_alive.is_zero() {
            return Err(validation_error("keep_alive cannot be zero"));
        }

        // Validate max connections
        if self.max_connections == 0 {
            return Err(validation_error("max_connections cannot be zero"));
        }

        // Validate TLS config
        self.tls.validate()?;

        Ok(())
    }
}

/// TLS configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Enable TLS
    #[serde(default = "default_tls_enabled")]
    pub enabled: bool,

    /// Path to certificate file
    #[serde(default)]
    pub cert_path: Option<PathBuf>,

    /// Path to private key file
    #[serde(default)]
    pub key_path: Option<PathBuf>,

    /// Path to CA certificate for client verification
    #[serde(default)]
    pub ca_path: Option<PathBuf>,

    /// Require client certificate
    #[serde(default)]
    pub require_client_cert: bool,

    /// Minimum TLS version
    #[serde(default = "default_min_tls_version")]
    pub min_version: String,

    /// Allowed cipher suites
    #[serde(default)]
    pub cipher_suites: Vec<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: default_tls_enabled(),
            cert_path: None,
            key_path: None,
            ca_path: None,
            require_client_cert: false,
            min_version: default_min_tls_version(),
            cipher_suites: Vec::new(),
        }
    }
}

impl ValidateConfig for TlsConfig {
    fn validate(&self) -> Result<()> {
        if self.enabled {
            // When TLS is enabled, cert and key paths should be provided
            if self.cert_path.is_none() {
                return Err(validation_error(
                    "TLS is enabled but cert_path is not specified",
                ));
            }
            if self.key_path.is_none() {
                return Err(validation_error(
                    "TLS is enabled but key_path is not specified",
                ));
            }

            // Validate paths are not empty if provided
            if let Some(ref path) = self.cert_path {
                if path.as_os_str().is_empty() {
                    return Err(validation_error("cert_path cannot be empty"));
                }
            }
            if let Some(ref path) = self.key_path {
                if path.as_os_str().is_empty() {
                    return Err(validation_error("key_path cannot be empty"));
                }
            }

            // Validate TLS version
            let valid_versions = ["1.2", "1.3"];
            if !valid_versions.contains(&self.min_version.as_str()) {
                return Err(validation_error(format!(
                    "Invalid TLS version: {}. Must be one of: {:?}",
                    self.min_version, valid_versions
                )));
            }
        }

        Ok(())
    }
}

// Default value functions
fn default_tcp_port() -> u16 {
    DEFAULT_TCP_PORT
}

fn default_udp_port() -> u16 {
    DEFAULT_UDP_PORT
}

fn default_http_port() -> u16 {
    DEFAULT_HTTP_PORT
}

fn default_websocket_port() -> u16 {
    DEFAULT_WEBSOCKET_PORT
}

fn default_bind_address() -> String {
    DEFAULT_BIND_ADDRESS.to_string()
}

fn default_connection_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS)
}

fn default_request_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS)
}

fn default_keep_alive() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_KEEP_ALIVE_SECS)
}

fn default_max_connections() -> u32 {
    1000
}

fn default_rate_limit() -> u32 {
    1000
}

fn default_tls_enabled() -> bool {
    false
}

fn default_min_tls_version() -> String {
    "1.3".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.tcp_port, DEFAULT_TCP_PORT);
        assert_eq!(config.udp_port, DEFAULT_UDP_PORT);
        assert_eq!(config.http_port, DEFAULT_HTTP_PORT);
        assert_eq!(config.websocket_port, DEFAULT_WEBSOCKET_PORT);
        assert_eq!(config.bind_address, DEFAULT_BIND_ADDRESS);
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.rate_limit, 1000);
        assert!(!config.tls.enabled);
    }

    #[test]
    fn test_network_config_validate_success() {
        let config = NetworkConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_network_config_validate_duplicate_ports() {
        let mut config = NetworkConfig::default();
        config.tcp_port = 8080;
        config.http_port = 8080; // Same port, should fail
        
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ports must be unique"));
    }

    #[test]
    fn test_network_config_validate_empty_bind_address() {
        let mut config = NetworkConfig::default();
        config.bind_address = String::new();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bind_address"));
    }

    #[test]
    fn test_network_config_validate_zero_timeouts() {
        let mut config = NetworkConfig::default();
        config.connection_timeout = std::time::Duration::ZERO;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("connection_timeout"));
    }

    #[test]
    fn test_network_config_validate_zero_max_connections() {
        let mut config = NetworkConfig::default();
        config.max_connections = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_connections"));
    }

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert!(!config.enabled);
        assert!(config.cert_path.is_none());
        assert!(config.key_path.is_none());
        assert_eq!(config.min_version, "1.3");
        assert!(!config.require_client_cert);
    }

    #[test]
    fn test_tls_config_validate_disabled() {
        let config = TlsConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tls_config_validate_enabled_no_cert() {
        let mut config = TlsConfig::default();
        config.enabled = true;
        // Missing cert_path and key_path
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cert_path"));
    }

    #[test]
    fn test_tls_config_validate_enabled_no_key() {
        let mut config = TlsConfig::default();
        config.enabled = true;
        config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        // Missing key_path
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key_path"));
    }

    #[test]
    fn test_tls_config_validate_enabled_valid() {
        let mut config = TlsConfig::default();
        config.enabled = true;
        config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        config.key_path = Some(PathBuf::from("/path/to/key.pem"));
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tls_config_validate_invalid_version() {
        let mut config = TlsConfig::default();
        config.enabled = true;
        config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        config.key_path = Some(PathBuf::from("/path/to/key.pem"));
        config.min_version = "1.1".to_string(); // Invalid
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("TLS version"));
    }

    #[test]
    fn test_tls_config_validate_valid_versions() {
        for version in ["1.2", "1.3"] {
            let mut config = TlsConfig::default();
            config.enabled = true;
            config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
            config.key_path = Some(PathBuf::from("/path/to/key.pem"));
            config.min_version = version.to_string();
            
            assert!(config.validate().is_ok(), "Version {} should be valid", version);
        }
    }

    #[test]
    fn test_network_config_serialize() {
        let config = NetworkConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("tcp_port"));
        assert!(toml.contains("udp_port"));
    }

    #[test]
    fn test_network_config_deserialize() {
        let toml = r#"
            tcp_port = 9000
            udp_port = 9001
            bind_address = "127.0.0.1"
        "#;
        let config: NetworkConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.tcp_port, 9000);
        assert_eq!(config.udp_port, 9001);
        assert_eq!(config.bind_address, "127.0.0.1");
    }
}
