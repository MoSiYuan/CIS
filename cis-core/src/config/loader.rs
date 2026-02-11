//! # Configuration Loader
//!
//! Loads and merges configuration from multiple sources:
//! 1. Default values (lowest priority)
//! 2. Configuration file (middle priority)
//! 3. Environment variables (highest priority)

use std::env;
use std::path::{Path, PathBuf};

use crate::config::{
    Config, DatabaseConfig, EncryptionConfig, NetworkConfig, P2PConfig, 
    SecurityConfig, StorageConfig, TlsConfig, WasmConfig,
};
use crate::config::p2p::{DhtConfig, GossipConfig, NatConfig, QuicConfig, RelayConfig};
use crate::config::wasm::GasCosts;
use serde::Deserialize;
use crate::error::{CisError, Result};

/// Configuration loader with support for file and environment variable overrides
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    /// Path to configuration file
    config_path: PathBuf,
    
    /// Environment variable prefix
    env_prefix: String,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self {
            config_path: Self::default_config_path(),
            env_prefix: "CIS".to_string(),
        }
    }
}

impl ConfigLoader {
    /// Create a new config loader with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config loader with a specific config file path
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: path.into(),
            env_prefix: "CIS".to_string(),
        }
    }

    /// Create a config loader with custom environment prefix
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            config_path: Self::default_config_path(),
            env_prefix: prefix.into(),
        }
    }

    /// Get the default configuration file path
    fn default_config_path() -> PathBuf {
        // Check for CIS_CONFIG environment variable first
        if let Ok(config_path) = env::var("CIS_CONFIG") {
            return PathBuf::from(config_path);
        }

        // Try to find config in standard locations
        let possible_paths = [
            PathBuf::from("cis.toml"),
            PathBuf::from("config.toml"),
            dirs::config_dir()
                .map(|d| d.join("cis").join("config.toml"))
                .unwrap_or_else(|| PathBuf::from("/etc/cis/config.toml")),
            PathBuf::from("/etc/cis/config.toml"),
        ];

        for path in &possible_paths {
            if path.exists() {
                return path.clone();
            }
        }

        // Return the first path if none exist (will be created if needed)
        possible_paths[0].clone()
    }

    /// Load configuration with full hierarchy
    ///
    /// Merges configuration in the following order (later overrides earlier):
    /// 1. Default values
    /// 2. Configuration file (if exists)
    /// 3. Environment variables
    pub fn load(&self) -> Result<Config> {
        // 1. Start with defaults
        let mut config = Config::default();

        // 2. Merge configuration file if it exists
        if self.config_path.exists() {
            let file_config = self.load_from_file()?;
            config = self.merge_file_config(config, file_config);
        }

        // 3. Merge environment variables (highest priority)
        config = self.merge_env_config(config)?;

        // 4. Validate the final configuration
        config.validate().map_err(|e| {
            CisError::configuration(format!(
                "Configuration validation failed: {}",
                e
            ))
        })?;

        Ok(config)
    }

    /// Load configuration from file
    fn load_from_file(&self) -> Result<FileConfig> {
        let content = std::fs::read_to_string(&self.config_path).map_err(|e| {
            CisError::configuration(format!(
                "Failed to read config file '{}': {}",
                self.config_path.display(),
                e
            ))
        })?;

        let config: FileConfig = toml::from_str(&content).map_err(|e| {
            CisError::configuration(format!(
                "Failed to parse config file '{}': {}",
                self.config_path.display(),
                e
            ))
        })?;

        Ok(config)
    }

    /// Merge file configuration into default configuration
    fn merge_file_config(&self, mut base: Config, file: FileConfig) -> Config {
        // Merge network config
        if let Some(network) = file.network {
            if let Some(port) = network.tcp_port {
                base.network.tcp_port = port;
            }
            if let Some(port) = network.udp_port {
                base.network.udp_port = port;
            }
            if let Some(port) = network.http_port {
                base.network.http_port = port;
            }
            if let Some(port) = network.websocket_port {
                base.network.websocket_port = port;
            }
            if let Some(addr) = network.bind_address {
                base.network.bind_address = addr;
            }
            if let Some(timeout) = network.connection_timeout_secs {
                base.network.connection_timeout = std::time::Duration::from_secs(timeout);
            }
            if let Some(timeout) = network.request_timeout_secs {
                base.network.request_timeout = std::time::Duration::from_secs(timeout);
            }
            if let Some(interval) = network.keep_alive_secs {
                base.network.keep_alive = std::time::Duration::from_secs(interval);
            }
            if let Some(max) = network.max_connections {
                base.network.max_connections = max;
            }
            if let Some(rate) = network.rate_limit {
                base.network.rate_limit = rate;
            }
            if let Some(tls) = network.tls {
                if let Some(enabled) = tls.enabled {
                    base.network.tls.enabled = enabled;
                }
                if let Some(path) = tls.cert_path {
                    base.network.tls.cert_path = Some(path);
                }
                if let Some(path) = tls.key_path {
                    base.network.tls.key_path = Some(path);
                }
                if let Some(path) = tls.ca_path {
                    base.network.tls.ca_path = Some(path);
                }
                if let Some(require) = tls.require_client_cert {
                    base.network.tls.require_client_cert = require;
                }
                if let Some(version) = tls.min_version {
                    base.network.tls.min_version = version;
                }
                if let Some(suites) = tls.cipher_suites {
                    base.network.tls.cipher_suites = suites;
                }
            }
        }

        // Merge storage config
        if let Some(storage) = file.storage {
            if let Some(dir) = storage.data_dir {
                base.storage.data_dir = dir;
            }
            if let Some(dir) = storage.backup_dir {
                base.storage.backup_dir = dir;
            }
            if let Some(dir) = storage.temp_dir {
                base.storage.temp_dir = dir;
            }
            if let Some(max) = storage.max_connections {
                base.storage.max_connections = max;
            }
            if let Some(timeout) = storage.connection_timeout_secs {
                base.storage.connection_timeout = std::time::Duration::from_secs(timeout);
            }
            if let Some(timeout) = storage.busy_timeout_ms {
                base.storage.busy_timeout = std::time::Duration::from_millis(timeout);
            }
            if let Some(interval) = storage.wal_checkpoint_interval_secs {
                base.storage.wal_checkpoint_interval = std::time::Duration::from_secs(interval);
            }
            if let Some(size) = storage.cache_size_pages {
                base.storage.cache_size_pages = size;
            }
            if let Some(enabled) = storage.wal_enabled {
                base.storage.wal_enabled = enabled;
            }
            if let Some(enabled) = storage.foreign_keys_enabled {
                base.storage.foreign_keys_enabled = enabled;
            }
            if let Some(mode) = storage.synchronous {
                base.storage.synchronous = mode;
            }
            if let Some(mode) = storage.journal_mode {
                base.storage.journal_mode = mode;
            }
        }

        // Merge security config
        if let Some(security) = file.security {
            if let Some(whitelist) = security.command_whitelist {
                base.security.command_whitelist = whitelist;
            }
            if let Some(size) = security.max_request_size {
                base.security.max_request_size = size;
            }
            if let Some(limit) = security.rate_limit {
                base.security.rate_limit = limit;
            }
            if let Some(burst) = security.rate_limit_burst {
                base.security.rate_limit_burst = burst;
            }
            if let Some(hours) = security.token_expiration_hours {
                base.security.token_expiration = std::time::Duration::from_secs(hours * 3600);
            }
            if let Some(minutes) = security.session_timeout_minutes {
                base.security.session_timeout = std::time::Duration::from_secs(minutes * 60);
            }
            if let Some(len) = security.min_password_length {
                base.security.min_password_length = len;
            }
            if let Some(require) = security.require_strong_password {
                base.security.require_strong_password = require;
            }
            if let Some(attempts) = security.max_login_attempts {
                base.security.max_login_attempts = attempts;
            }
            if let Some(minutes) = security.lockout_duration_minutes {
                base.security.lockout_duration = std::time::Duration::from_secs(minutes * 60);
            }
            if let Some(enabled) = security.audit_logging_enabled {
                base.security.audit_logging_enabled = enabled;
            }
            if let Some(days) = security.audit_retention_days {
                base.security.audit_retention_days = days;
            }
            if let Some(origins) = security.allowed_origins {
                base.security.allowed_origins = origins;
            }
            if let Some(require) = security.require_https {
                base.security.require_https = require;
            }
        }

        // Merge WASM config
        if let Some(wasm) = file.wasm {
            if let Some(memory) = wasm.max_memory {
                base.wasm.max_memory = memory;
            }
            if let Some(secs) = wasm.max_execution_time_secs {
                base.wasm.max_execution_time = std::time::Duration::from_secs(secs);
            }
            if let Some(size) = wasm.stack_size {
                base.wasm.stack_size = size;
            }
            if let Some(size) = wasm.table_size {
                base.wasm.table_size = size;
            }
            if let Some(limit) = wasm.fuel_limit {
                base.wasm.fuel_limit = limit;
            }
            if let Some(secs) = wasm.compilation_timeout_secs {
                base.wasm.compilation_timeout = std::time::Duration::from_secs(secs);
            }
            if let Some(limit) = wasm.memory_pages_limit {
                base.wasm.memory_pages_limit = limit;
            }
            if let Some(size) = wasm.instance_pool_size {
                base.wasm.instance_pool_size = size;
            }
            if let Some(syscalls) = wasm.allowed_syscalls {
                base.wasm.allowed_syscalls = syscalls;
            }
            if let Some(functions) = wasm.allowed_host_functions {
                base.wasm.allowed_host_functions = functions;
            }
            if let Some(enabled) = wasm.simd_enabled {
                base.wasm.simd_enabled = enabled;
            }
            if let Some(enabled) = wasm.threads_enabled {
                base.wasm.threads_enabled = enabled;
            }
            if let Some(enabled) = wasm.module_caching_enabled {
                base.wasm.module_caching_enabled = enabled;
            }
        }

        // Merge P2P config
        if let Some(p2p) = file.p2p {
            if let Some(enabled) = p2p.enabled {
                base.p2p.enabled = enabled;
            }
            if let Some(nodes) = p2p.bootstrap_nodes {
                base.p2p.bootstrap_nodes = nodes;
            }
            if let Some(peers) = p2p.static_peers {
                base.p2p.static_peers = peers;
            }
            if let Some(secs) = p2p.discovery_interval_secs {
                base.p2p.discovery_interval = std::time::Duration::from_secs(secs);
            }
            if let Some(secs) = p2p.connection_timeout_secs {
                base.p2p.connection_timeout = std::time::Duration::from_secs(secs);
            }
            if let Some(max) = p2p.max_peers {
                base.p2p.max_peers = max;
            }
            if let Some(min) = p2p.min_peers {
                base.p2p.min_peers = min;
            }
        }

        base
    }

    /// Merge environment variable configuration
    fn merge_env_config(&self, mut config: Config) -> Result<Config> {
        let prefix = &self.env_prefix;

        // Network environment variables
        if let Ok(val) = env::var(format!("{}_NETWORK_TCP_PORT", prefix)) {
            config.network.tcp_port = parse_port(&val, "NETWORK_TCP_PORT")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_UDP_PORT", prefix)) {
            config.network.udp_port = parse_port(&val, "NETWORK_UDP_PORT")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_HTTP_PORT", prefix)) {
            config.network.http_port = parse_port(&val, "NETWORK_HTTP_PORT")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_WEBSOCKET_PORT", prefix)) {
            config.network.websocket_port = parse_port(&val, "NETWORK_WEBSOCKET_PORT")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_BIND_ADDRESS", prefix)) {
            config.network.bind_address = val;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_MAX_CONNECTIONS", prefix)) {
            config.network.max_connections = parse_u32(&val, "NETWORK_MAX_CONNECTIONS")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_RATE_LIMIT", prefix)) {
            config.network.rate_limit = parse_u32(&val, "NETWORK_RATE_LIMIT")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_TLS_ENABLED", prefix)) {
            config.network.tls.enabled = parse_bool(&val, "NETWORK_TLS_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_TLS_CERT_PATH", prefix)) {
            config.network.tls.cert_path = Some(PathBuf::from(val));
        }
        if let Ok(val) = env::var(format!("{}_NETWORK_TLS_KEY_PATH", prefix)) {
            config.network.tls.key_path = Some(PathBuf::from(val));
        }

        // Storage environment variables
        if let Ok(val) = env::var(format!("{}_STORAGE_DATA_DIR", prefix)) {
            config.storage.data_dir = PathBuf::from(val);
        }
        if let Ok(val) = env::var(format!("{}_STORAGE_BACKUP_DIR", prefix)) {
            config.storage.backup_dir = PathBuf::from(val);
        }
        if let Ok(val) = env::var(format!("{}_STORAGE_MAX_CONNECTIONS", prefix)) {
            config.storage.max_connections = parse_u32(&val, "STORAGE_MAX_CONNECTIONS")?;
        }
        if let Ok(val) = env::var(format!("{}_STORAGE_WAL_ENABLED", prefix)) {
            config.storage.wal_enabled = parse_bool(&val, "STORAGE_WAL_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_STORAGE_ENCRYPTION_ENABLED", prefix)) {
            config.storage.encryption.enabled = parse_bool(&val, "STORAGE_ENCRYPTION_ENABLED")?;
        }

        // Security environment variables
        if let Ok(val) = env::var(format!("{}_SECURITY_MAX_REQUEST_SIZE", prefix)) {
            config.security.max_request_size = parse_usize(&val, "SECURITY_MAX_REQUEST_SIZE")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_RATE_LIMIT", prefix)) {
            config.security.rate_limit = parse_u32(&val, "SECURITY_RATE_LIMIT")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_TOKEN_EXPIRATION_HOURS", prefix)) {
            let hours = parse_u64(&val, "SECURITY_TOKEN_EXPIRATION_HOURS")?;
            config.security.token_expiration = std::time::Duration::from_secs(hours * 3600);
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_SESSION_TIMEOUT_MINUTES", prefix)) {
            let mins = parse_u64(&val, "SECURITY_SESSION_TIMEOUT_MINUTES")?;
            config.security.session_timeout = std::time::Duration::from_secs(mins * 60);
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_MIN_PASSWORD_LENGTH", prefix)) {
            config.security.min_password_length = parse_usize(&val, "SECURITY_MIN_PASSWORD_LENGTH")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_REQUIRE_STRONG_PASSWORD", prefix)) {
            config.security.require_strong_password = parse_bool(&val, "SECURITY_REQUIRE_STRONG_PASSWORD")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_MAX_LOGIN_ATTEMPTS", prefix)) {
            config.security.max_login_attempts = parse_u32(&val, "SECURITY_MAX_LOGIN_ATTEMPTS")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_AUDIT_LOGGING_ENABLED", prefix)) {
            config.security.audit_logging_enabled = parse_bool(&val, "SECURITY_AUDIT_LOGGING_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_SECURITY_REQUIRE_HTTPS", prefix)) {
            config.security.require_https = parse_bool(&val, "SECURITY_REQUIRE_HTTPS")?;
        }

        // WASM environment variables
        if let Ok(val) = env::var(format!("{}_WASM_MAX_MEMORY", prefix)) {
            config.wasm.max_memory = parse_usize(&val, "WASM_MAX_MEMORY")?;
        }
        if let Ok(val) = env::var(format!("{}_WASM_MAX_EXECUTION_TIME_SECS", prefix)) {
            let secs = parse_u64(&val, "WASM_MAX_EXECUTION_TIME_SECS")?;
            config.wasm.max_execution_time = std::time::Duration::from_secs(secs);
        }
        if let Ok(val) = env::var(format!("{}_WASM_STACK_SIZE", prefix)) {
            config.wasm.stack_size = parse_usize(&val, "WASM_STACK_SIZE")?;
        }
        if let Ok(val) = env::var(format!("{}_WASM_FUEL_LIMIT", prefix)) {
            config.wasm.fuel_limit = parse_u64(&val, "WASM_FUEL_LIMIT")?;
        }
        if let Ok(val) = env::var(format!("{}_WASM_SIMD_ENABLED", prefix)) {
            config.wasm.simd_enabled = parse_bool(&val, "WASM_SIMD_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_WASM_THREADS_ENABLED", prefix)) {
            config.wasm.threads_enabled = parse_bool(&val, "WASM_THREADS_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_WASM_MODULE_CACHING_ENABLED", prefix)) {
            config.wasm.module_caching_enabled = parse_bool(&val, "WASM_MODULE_CACHING_ENABLED")?;
        }

        // P2P environment variables
        if let Ok(val) = env::var(format!("{}_P2P_ENABLED", prefix)) {
            config.p2p.enabled = parse_bool(&val, "P2P_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_BOOTSTRAP_NODES", prefix)) {
            config.p2p.bootstrap_nodes = val.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(val) = env::var(format!("{}_P2P_MAX_PEERS", prefix)) {
            config.p2p.max_peers = parse_u32(&val, "P2P_MAX_PEERS")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_MIN_PEERS", prefix)) {
            config.p2p.min_peers = parse_u32(&val, "P2P_MIN_PEERS")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_DISCOVERY_INTERVAL_SECS", prefix)) {
            let secs = parse_u64(&val, "P2P_DISCOVERY_INTERVAL_SECS")?;
            config.p2p.discovery_interval = std::time::Duration::from_secs(secs);
        }
        if let Ok(val) = env::var(format!("{}_P2P_MDNS_ENABLED", prefix)) {
            config.p2p.mdns_enabled = parse_bool(&val, "P2P_MDNS_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_DHT_ENABLED", prefix)) {
            config.p2p.dht_enabled = parse_bool(&val, "P2P_DHT_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_RELAY_ENABLED", prefix)) {
            config.p2p.relay_enabled = parse_bool(&val, "P2P_RELAY_ENABLED")?;
        }
        if let Ok(val) = env::var(format!("{}_P2P_PROTOCOL_VERSION", prefix)) {
            config.p2p.protocol_version = val;
        }
        if let Ok(val) = env::var(format!("{}_P2P_NETWORK_ID", prefix)) {
            config.p2p.network_id = val;
        }

        Ok(config)
    }

    /// Get the configuration file path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Create a default configuration file template
    pub fn create_template(&self) -> Result<String> {
        let template = r#"# CIS Configuration File
# Generated template - modify as needed

[network]
tcp_port = 6767
udp_port = 7677
http_port = 8080
websocket_port = 6768
bind_address = "0.0.0.0"
connection_timeout_secs = 30
request_timeout_secs = 60
keep_alive_secs = 30
max_connections = 1000
rate_limit = 1000

[network.tls]
enabled = false
cert_path = "/etc/cis/cert.pem"
key_path = "/etc/cis/key.pem"
min_version = "1.3"

[storage]
data_dir = "/var/lib/cis"
backup_dir = "/var/lib/cis/backups"
temp_dir = "/tmp/cis"
max_connections = 100
connection_timeout_secs = 30
busy_timeout_ms = 5000
wal_checkpoint_interval_secs = 300
cache_size_pages = 10000
wal_enabled = true
foreign_keys_enabled = true
synchronous = "NORMAL"
journal_mode = "WAL"

[storage.encryption]
enabled = false
key_derivation = "argon2id"

[security]
max_request_size = 10485760  # 10MB
rate_limit = 100
rate_limit_burst = 10
token_expiration_hours = 24
session_timeout_minutes = 30
min_password_length = 8
require_strong_password = true
max_login_attempts = 5
lockout_duration_minutes = 15
audit_logging_enabled = true
audit_retention_days = 90
require_https = false

[wasm]
max_memory = 536870912  # 512MB
max_execution_time_secs = 30
stack_size = 1048576    # 1MB
table_size = 10000
fuel_limit = 10000000000
compilation_timeout_secs = 60
memory_pages_limit = 8192
instance_pool_size = 10
simd_enabled = true
threads_enabled = false
bulk_memory_enabled = true
reference_types_enabled = true
multi_value_enabled = true
module_caching_enabled = true
strict_validation = true
max_module_size = 104857600  # 100MB
max_globals = 10000
max_functions = 100000
max_data_segments = 100000

[p2p]
enabled = false
bootstrap_nodes = []
static_peers = []
discovery_interval_secs = 60
connection_timeout_secs = 30
dial_timeout_secs = 10
keep_alive_interval_secs = 15
max_peers = 50
min_peers = 3
mdns_enabled = true
dht_enabled = true
relay_enabled = true
hole_punching_enabled = true
protocol_version = "cis/1.1.4"
network_id = "cis-mainnet"

[p2p.dht]
enabled = true
replication_factor = 3
record_ttl_secs = 7200

[p2p.gossip]
enabled = true
interval_secs = 30
fanout = 6

[p2p.nat]
upnp_enabled = true
stun_enabled = true
port_mapping_enabled = true

[p2p.quic]
enabled = true
max_streams = 100
"#;

        Ok(template.to_string())
    }
}

/// Parse a port number from string
fn parse_port(s: &str, name: &str) -> Result<u16> {
    s.parse::<u16>().map_err(|e| {
        CisError::configuration(format!(
            "Invalid {} '{}': must be a valid port number (1-65535). Error: {}",
            name, s, e
        ))
    })
}

/// Parse a u32 from string
fn parse_u32(s: &str, name: &str) -> Result<u32> {
    s.parse::<u32>().map_err(|e| {
        CisError::configuration(format!(
            "Invalid {} '{}': must be a valid number. Error: {}",
            name, s, e
        ))
    })
}

/// Parse a u64 from string
fn parse_u64(s: &str, name: &str) -> Result<u64> {
    s.parse::<u64>().map_err(|e| {
        CisError::configuration(format!(
            "Invalid {} '{}': must be a valid number. Error: {}",
            name, s, e
        ))
    })
}

/// Parse a usize from string
fn parse_usize(s: &str, name: &str) -> Result<usize> {
    s.parse::<usize>().map_err(|e| {
        CisError::configuration(format!(
            "Invalid {} '{}': must be a valid number. Error: {}",
            name, s, e
        ))
    })
}

/// Parse a boolean from string
fn parse_bool(s: &str, name: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(CisError::configuration(format!(
            "Invalid {} '{}': must be 'true' or 'false'",
            name, s
        ))),
    }
}

/// Configuration structure for file-based config
/// Uses Option for all fields to allow partial configuration
#[derive(Debug, Clone, Deserialize)]
struct FileConfig {
    #[serde(default)]
    pub network: Option<FileNetworkConfig>,
    #[serde(default)]
    pub storage: Option<FileStorageConfig>,
    #[serde(default)]
    pub security: Option<FileSecurityConfig>,
    #[serde(default)]
    pub wasm: Option<FileWasmConfig>,
    #[serde(default)]
    pub p2p: Option<FileP2PConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileNetworkConfig {
    pub tcp_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub http_port: Option<u16>,
    pub websocket_port: Option<u16>,
    pub bind_address: Option<String>,
    pub connection_timeout_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub keep_alive_secs: Option<u64>,
    pub max_connections: Option<u32>,
    pub rate_limit: Option<u32>,
    pub tls: Option<FileTlsConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileTlsConfig {
    pub enabled: Option<bool>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
    pub ca_path: Option<PathBuf>,
    pub require_client_cert: Option<bool>,
    pub min_version: Option<String>,
    pub cipher_suites: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileStorageConfig {
    pub data_dir: Option<PathBuf>,
    pub backup_dir: Option<PathBuf>,
    pub temp_dir: Option<PathBuf>,
    pub max_connections: Option<u32>,
    pub connection_timeout_secs: Option<u64>,
    pub busy_timeout_ms: Option<u64>,
    pub wal_checkpoint_interval_secs: Option<u64>,
    pub cache_size_pages: Option<i32>,
    pub wal_enabled: Option<bool>,
    pub foreign_keys_enabled: Option<bool>,
    pub synchronous: Option<String>,
    pub journal_mode: Option<String>,
    pub encryption: Option<FileEncryptionConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileEncryptionConfig {
    pub enabled: Option<bool>,
    pub key_derivation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileSecurityConfig {
    pub command_whitelist: Option<Vec<String>>,
    pub max_request_size: Option<usize>,
    pub rate_limit: Option<u32>,
    pub rate_limit_burst: Option<u32>,
    pub token_expiration_hours: Option<u64>,
    pub session_timeout_minutes: Option<u64>,
    pub min_password_length: Option<usize>,
    pub require_strong_password: Option<bool>,
    pub max_login_attempts: Option<u32>,
    pub lockout_duration_minutes: Option<u64>,
    pub audit_logging_enabled: Option<bool>,
    pub audit_retention_days: Option<u32>,
    pub allowed_origins: Option<Vec<String>>,
    pub require_https: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileWasmConfig {
    pub max_memory: Option<usize>,
    pub max_execution_time_secs: Option<u64>,
    pub stack_size: Option<usize>,
    pub table_size: Option<u32>,
    pub fuel_limit: Option<u64>,
    pub compilation_timeout_secs: Option<u64>,
    pub memory_pages_limit: Option<u32>,
    pub instance_pool_size: Option<usize>,
    pub allowed_syscalls: Option<Vec<String>>,
    pub allowed_host_functions: Option<Vec<String>>,
    pub simd_enabled: Option<bool>,
    pub threads_enabled: Option<bool>,
    pub module_caching_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct FileP2PConfig {
    pub enabled: Option<bool>,
    pub bootstrap_nodes: Option<Vec<String>>,
    pub static_peers: Option<Vec<String>>,
    pub discovery_interval_secs: Option<u64>,
    pub connection_timeout_secs: Option<u64>,
    pub max_peers: Option<u32>,
    pub min_peers: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_loader_new() {
        let loader = ConfigLoader::new();
        assert_eq!(loader.env_prefix, "CIS");
        assert!(loader.config_path.file_name().is_some());
    }

    #[test]
    fn test_config_loader_with_path() {
        let loader = ConfigLoader::with_path("/custom/config.toml");
        assert_eq!(loader.config_path, PathBuf::from("/custom/config.toml"));
    }

    #[test]
    fn test_config_loader_with_prefix() {
        let loader = ConfigLoader::with_prefix("MYAPP");
        assert_eq!(loader.env_prefix, "MYAPP");
    }

    #[test]
    fn test_load_from_file_success() {
        let toml_content = r#"
            [network]
            tcp_port = 9000
            bind_address = "127.0.0.1"
        "#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        
        let loader = ConfigLoader::with_path(temp_file.path());
        let config = loader.load().unwrap();
        
        assert_eq!(config.network.tcp_port, 9000);
        assert_eq!(config.network.bind_address, "127.0.0.1");
        // Other values should be defaults
        assert_eq!(config.network.udp_port, 7677);
    }

    #[test]
    fn test_load_from_nonexistent_file() {
        let loader = ConfigLoader::with_path("/nonexistent/path/config.toml");
        // Should use defaults without error
        let config = loader.load().unwrap();
        assert_eq!(config.network.tcp_port, 6767);
    }

    #[test]
    fn test_load_with_env_override() {
        // Set environment variable
        env::set_var("CIS_NETWORK_TCP_PORT", "8888");
        env::set_var("CIS_NETWORK_BIND_ADDRESS", "192.168.1.1");
        
        let loader = ConfigLoader::new();
        let config = loader.load().unwrap();
        
        assert_eq!(config.network.tcp_port, 8888);
        assert_eq!(config.network.bind_address, "192.168.1.1");
        
        // Clean up
        env::remove_var("CIS_NETWORK_TCP_PORT");
        env::remove_var("CIS_NETWORK_BIND_ADDRESS");
    }

    #[test]
    fn test_env_priority_over_file() {
        // Create temp config file
        let toml_content = r#"
            [network]
            tcp_port = 7000
        "#;
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        
        // Set environment variable
        env::set_var("CIS_NETWORK_TCP_PORT", "9000");
        
        let loader = ConfigLoader::with_path(temp_file.path());
        let config = loader.load().unwrap();
        
        // Environment should override file
        assert_eq!(config.network.tcp_port, 9000);
        
        // Clean up
        env::remove_var("CIS_NETWORK_TCP_PORT");
    }

    #[test]
    fn test_parse_port_valid() {
        assert_eq!(parse_port("8080", "TEST").unwrap(), 8080);
        assert_eq!(parse_port("1", "TEST").unwrap(), 1);
        assert_eq!(parse_port("65535", "TEST").unwrap(), 65535);
    }

    #[test]
    fn test_parse_port_invalid() {
        assert!(parse_port("abc", "TEST").is_err());
        assert!(parse_port("", "TEST").is_err());
        assert!(parse_port("70000", "TEST").is_err()); // Too large for u16
    }

    #[test]
    fn test_parse_bool_valid() {
        assert_eq!(parse_bool("true", "TEST").unwrap(), true);
        assert_eq!(parse_bool("TRUE", "TEST").unwrap(), true);
        assert_eq!(parse_bool("1", "TEST").unwrap(), true);
        assert_eq!(parse_bool("yes", "TEST").unwrap(), true);
        assert_eq!(parse_bool("on", "TEST").unwrap(), true);
        assert_eq!(parse_bool("false", "TEST").unwrap(), false);
        assert_eq!(parse_bool("FALSE", "TEST").unwrap(), false);
        assert_eq!(parse_bool("0", "TEST").unwrap(), false);
        assert_eq!(parse_bool("no", "TEST").unwrap(), false);
        assert_eq!(parse_bool("off", "TEST").unwrap(), false);
    }

    #[test]
    fn test_parse_bool_invalid() {
        assert!(parse_bool("maybe", "TEST").is_err());
        assert!(parse_bool("", "TEST").is_err());
        assert!(parse_bool("2", "TEST").is_err());
    }

    #[test]
    fn test_parse_numeric_types() {
        assert_eq!(parse_u32("100", "TEST").unwrap(), 100);
        assert_eq!(parse_u64("18446744073709551615", "TEST").unwrap(), u64::MAX);
        assert_eq!(parse_usize("1000", "TEST").unwrap(), 1000);
        
        assert!(parse_u32("abc", "TEST").is_err());
        assert!(parse_u64("abc", "TEST").is_err());
        assert!(parse_usize("abc", "TEST").is_err());
    }

    #[test]
    fn test_create_template() {
        let loader = ConfigLoader::new();
        let template = loader.create_template().unwrap();
        
        assert!(template.contains("[network]"));
        assert!(template.contains("[storage]"));
        assert!(template.contains("[security]"));
        assert!(template.contains("[wasm]"));
        assert!(template.contains("[p2p]"));
        assert!(template.contains("tcp_port = 6767"));
    }

    #[test]
    fn test_all_network_env_vars() {
        // Set all network-related env vars
        env::set_var("CIS_NETWORK_TCP_PORT", "1111");
        env::set_var("CIS_NETWORK_UDP_PORT", "2222");
        env::set_var("CIS_NETWORK_HTTP_PORT", "3333");
        env::set_var("CIS_NETWORK_WEBSOCKET_PORT", "4444");
        env::set_var("CIS_NETWORK_BIND_ADDRESS", "10.0.0.1");
        env::set_var("CIS_NETWORK_MAX_CONNECTIONS", "500");
        env::set_var("CIS_NETWORK_RATE_LIMIT", "200");
        env::set_var("CIS_NETWORK_TLS_ENABLED", "true");
        env::set_var("CIS_NETWORK_TLS_CERT_PATH", "/path/to/cert");
        env::set_var("CIS_NETWORK_TLS_KEY_PATH", "/path/to/key");
        
        let loader = ConfigLoader::new();
        let config = loader.load().unwrap();
        
        assert_eq!(config.network.tcp_port, 1111);
        assert_eq!(config.network.udp_port, 2222);
        assert_eq!(config.network.http_port, 3333);
        assert_eq!(config.network.websocket_port, 4444);
        assert_eq!(config.network.bind_address, "10.0.0.1");
        assert_eq!(config.network.max_connections, 500);
        assert_eq!(config.network.rate_limit, 200);
        assert!(config.network.tls.enabled);
        assert_eq!(config.network.tls.cert_path, Some(PathBuf::from("/path/to/cert")));
        assert_eq!(config.network.tls.key_path, Some(PathBuf::from("/path/to/key")));
        
        // Cleanup
        for var in [
            "CIS_NETWORK_TCP_PORT",
            "CIS_NETWORK_UDP_PORT",
            "CIS_NETWORK_HTTP_PORT",
            "CIS_NETWORK_WEBSOCKET_PORT",
            "CIS_NETWORK_BIND_ADDRESS",
            "CIS_NETWORK_MAX_CONNECTIONS",
            "CIS_NETWORK_RATE_LIMIT",
            "CIS_NETWORK_TLS_ENABLED",
            "CIS_NETWORK_TLS_CERT_PATH",
            "CIS_NETWORK_TLS_KEY_PATH",
        ] {
            env::remove_var(var);
        }
    }

    #[test]
    fn test_p2p_bootstrap_nodes_env() {
        env::set_var("CIS_P2P_BOOTSTRAP_NODES", "/ip4/1.2.3.4/tcp/7677,/ip4/5.6.7.8/tcp/7677");
        
        let loader = ConfigLoader::new();
        let config = loader.load().unwrap();
        
        assert_eq!(config.p2p.bootstrap_nodes.len(), 2);
        assert!(config.p2p.bootstrap_nodes.contains(&"/ip4/1.2.3.4/tcp/7677".to_string()));
        assert!(config.p2p.bootstrap_nodes.contains(&"/ip4/5.6.7.8/tcp/7677".to_string()));
        
        env::remove_var("CIS_P2P_BOOTSTRAP_NODES");
    }

    #[test]
    fn test_load_with_validation_error() {
        env::set_var("CIS_NETWORK_TCP_PORT", "80"); // Invalid (below 1024)
        
        let loader = ConfigLoader::new();
        let result = loader.load();
        
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Validation error") || err_msg.contains("validation"));
        
        env::remove_var("CIS_NETWORK_TCP_PORT");
    }
}
