//! # P2P Configuration
//!
//! Peer-to-peer networking configuration including discovery, transport, and protocol settings.

use serde::{Deserialize, Serialize};

use super::{validation_error, validate_port, ValidateConfig};
use crate::error::Result;

/// Default discovery interval in seconds
pub const DEFAULT_DISCOVERY_INTERVAL_SECS: u64 = 60;

/// Default connection timeout in seconds
pub const DEFAULT_CONNECTION_TIMEOUT_SECS: u64 = 30;

/// Default dial timeout in seconds
pub const DEFAULT_DIAL_TIMEOUT_SECS: u64 = 10;

/// Default keep-alive interval in seconds
pub const DEFAULT_KEEP_ALIVE_INTERVAL_SECS: u64 = 15;

/// Default max peers
pub const DEFAULT_MAX_PEERS: u32 = 50;

/// Default min peers for full connectivity
pub const DEFAULT_MIN_PEERS: u32 = 3;

/// Default bootstrap timeout in seconds
pub const DEFAULT_BOOTSTRAP_TIMEOUT_SECS: u64 = 60;

/// Default replication factor for DHT
pub const DEFAULT_DHT_REPLICATION_FACTOR: u8 = 3;

/// Default DHT record TTL in seconds
pub const DEFAULT_DHT_RECORD_TTL_SECS: u64 = 7200;

/// Default gossip interval in seconds
pub const DEFAULT_GOSSIP_INTERVAL_SECS: u64 = 30;

/// Default max message size (10 MB)
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

/// P2P network configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2PConfig {
    /// Enable P2P networking
    #[serde(default = "default_p2p_enabled")]
    pub enabled: bool,

    /// Bootstrap nodes (multiaddresses)
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,

    /// Static peers (permanent connections)
    #[serde(default)]
    pub static_peers: Vec<String>,

    /// Discovery interval
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval: std::time::Duration,

    /// Connection timeout
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: std::time::Duration,

    /// Dial timeout for new connections
    #[serde(default = "default_dial_timeout")]
    pub dial_timeout: std::time::Duration,

    /// Keep-alive interval
    #[serde(default = "default_keep_alive_interval")]
    pub keep_alive_interval: std::time::Duration,

    /// Maximum number of peers
    #[serde(default = "default_max_peers")]
    pub max_peers: u32,

    /// Minimum number of peers to maintain
    #[serde(default = "default_min_peers")]
    pub min_peers: u32,

    /// Bootstrap timeout
    #[serde(default = "default_bootstrap_timeout")]
    pub bootstrap_timeout: std::time::Duration,

    /// Listen addresses (in addition to default)
    #[serde(default)]
    pub listen_addresses: Vec<String>,

    /// External addresses (advertised to peers)
    #[serde(default)]
    pub external_addresses: Vec<String>,

    /// Enable mDNS discovery
    #[serde(default = "default_mdns_enabled")]
    pub mdns_enabled: bool,

    /// Enable DHT routing
    #[serde(default = "default_dht_enabled")]
    pub dht_enabled: bool,

    /// DHT configuration
    #[serde(default)]
    pub dht: DhtConfig,

    /// Gossip configuration
    #[serde(default)]
    pub gossip: GossipConfig,

    /// NAT traversal configuration
    #[serde(default)]
    pub nat: NatConfig,

    /// QUIC transport configuration
    #[serde(default)]
    pub quic: QuicConfig,

    /// Maximum message size
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Enable relay transport for NAT traversal
    #[serde(default = "default_relay_enabled")]
    pub relay_enabled: bool,

    /// Relay configuration
    #[serde(default)]
    pub relay: RelayConfig,

    /// Enable hole punching
    #[serde(default = "default_hole_punching_enabled")]
    pub hole_punching_enabled: bool,

    /// Protocol version (must match between peers)
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// Network ID (isolate different networks)
    #[serde(default = "default_network_id")]
    pub network_id: String,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            enabled: default_p2p_enabled(),
            bootstrap_nodes: Vec::new(),
            static_peers: Vec::new(),
            discovery_interval: default_discovery_interval(),
            connection_timeout: default_connection_timeout(),
            dial_timeout: default_dial_timeout(),
            keep_alive_interval: default_keep_alive_interval(),
            max_peers: default_max_peers(),
            min_peers: default_min_peers(),
            bootstrap_timeout: default_bootstrap_timeout(),
            listen_addresses: Vec::new(),
            external_addresses: Vec::new(),
            mdns_enabled: default_mdns_enabled(),
            dht_enabled: default_dht_enabled(),
            dht: DhtConfig::default(),
            gossip: GossipConfig::default(),
            nat: NatConfig::default(),
            quic: QuicConfig::default(),
            max_message_size: default_max_message_size(),
            relay_enabled: default_relay_enabled(),
            relay: RelayConfig::default(),
            hole_punching_enabled: default_hole_punching_enabled(),
            protocol_version: default_protocol_version(),
            network_id: default_network_id(),
        }
    }
}

impl ValidateConfig for P2PConfig {
    fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Validate timeouts are not zero
        if self.discovery_interval.is_zero() {
            return Err(validation_error("discovery_interval cannot be zero"));
        }
        if self.connection_timeout.is_zero() {
            return Err(validation_error("connection_timeout cannot be zero"));
        }
        if self.dial_timeout.is_zero() {
            return Err(validation_error("dial_timeout cannot be zero"));
        }
        if self.keep_alive_interval.is_zero() {
            return Err(validation_error("keep_alive_interval cannot be zero"));
        }
        if self.bootstrap_timeout.is_zero() {
            return Err(validation_error("bootstrap_timeout cannot be zero"));
        }

        // Validate peer counts
        if self.max_peers == 0 {
            return Err(validation_error("max_peers cannot be zero"));
        }
        if self.max_peers > 1000 {
            return Err(validation_error("max_peers cannot exceed 1000"));
        }
        if self.min_peers > self.max_peers {
            return Err(validation_error(
                "min_peers cannot exceed max_peers",
            ));
        }

        // Validate max message size
        if self.max_message_size == 0 {
            return Err(validation_error("max_message_size cannot be zero"));
        }
        if self.max_message_size > 100 * 1024 * 1024 {
            // Max 100 MB
            return Err(validation_error(
                "max_message_size cannot exceed 100 MB",
            ));
        }

        // Validate protocol version is not empty
        if self.protocol_version.is_empty() {
            return Err(validation_error("protocol_version cannot be empty"));
        }

        // Validate network ID is not empty
        if self.network_id.is_empty() {
            return Err(validation_error("network_id cannot be empty"));
        }

        // Validate bootstrap nodes are valid multiaddresses
        for node in &self.bootstrap_nodes {
            if node.is_empty() {
                return Err(validation_error("bootstrap node address cannot be empty"));
            }
            // Basic multiaddress validation
            if !node.contains('/') {
                return Err(validation_error(format!(
                    "Invalid bootstrap node address (not a multiaddress): {}",
                    node
                )));
            }
        }

        // Validate sub-configs
        self.dht.validate()?;
        self.gossip.validate()?;
        self.nat.validate()?;
        self.quic.validate()?;
        self.relay.validate()?;

        Ok(())
    }
}

/// DHT (Distributed Hash Table) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DhtConfig {
    /// Enable DHT
    #[serde(default = "default_dht_config_enabled")]
    pub enabled: bool,

    /// Replication factor for DHT records
    #[serde(default = "default_replication_factor")]
    pub replication_factor: u8,

    /// Record TTL (time to live)
    #[serde(default = "default_record_ttl")]
    pub record_ttl: std::time::Duration,

    /// Query timeout
    #[serde(default = "default_dht_query_timeout")]
    pub query_timeout: std::time::Duration,

    /// Maximum stored records
    #[serde(default = "default_max_stored_records")]
    pub max_stored_records: u32,

    /// Provider record TTL
    #[serde(default = "default_provider_ttl")]
    pub provider_ttl: std::time::Duration,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            enabled: default_dht_config_enabled(),
            replication_factor: default_replication_factor(),
            record_ttl: default_record_ttl(),
            query_timeout: default_dht_query_timeout(),
            max_stored_records: default_max_stored_records(),
            provider_ttl: default_provider_ttl(),
        }
    }
}

impl ValidateConfig for DhtConfig {
    fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Validate replication factor
        if self.replication_factor == 0 {
            return Err(validation_error("replication_factor cannot be zero"));
        }
        if self.replication_factor > 20 {
            return Err(validation_error(
                "replication_factor cannot exceed 20",
            ));
        }

        // Validate timeouts are not zero
        if self.record_ttl.is_zero() {
            return Err(validation_error("record_ttl cannot be zero"));
        }
        if self.query_timeout.is_zero() {
            return Err(validation_error("query_timeout cannot be zero"));
        }
        if self.provider_ttl.is_zero() {
            return Err(validation_error("provider_ttl cannot be zero"));
        }

        // Validate max stored records
        if self.max_stored_records == 0 {
            return Err(validation_error("max_stored_records cannot be zero"));
        }

        Ok(())
    }
}

/// Gossip protocol configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GossipConfig {
    /// Enable gossip protocol
    #[serde(default = "default_gossip_enabled")]
    pub enabled: bool,

    /// Gossip interval
    #[serde(default = "default_gossip_interval")]
    pub interval: std::time::Duration,

    /// Maximum gossip message size
    #[serde(default = "default_gossip_max_message_size")]
    pub max_message_size: usize,

    /// Gossip fanout (number of peers to gossip to)
    #[serde(default = "default_gossip_fanout")]
    pub fanout: u8,

    /// History length (number of recent messages to remember)
    #[serde(default = "default_gossip_history_length")]
    pub history_length: u32,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            enabled: default_gossip_enabled(),
            interval: default_gossip_interval(),
            max_message_size: default_gossip_max_message_size(),
            fanout: default_gossip_fanout(),
            history_length: default_gossip_history_length(),
        }
    }
}

impl ValidateConfig for GossipConfig {
    fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Validate interval
        if self.interval.is_zero() {
            return Err(validation_error("gossip interval cannot be zero"));
        }

        // Validate max message size
        if self.max_message_size == 0 {
            return Err(validation_error("gossip max_message_size cannot be zero"));
        }
        if self.max_message_size > 10 * 1024 * 1024 {
            // Max 10 MB for gossip
            return Err(validation_error(
                "gossip max_message_size cannot exceed 10 MB",
            ));
        }

        // Validate fanout
        if self.fanout == 0 {
            return Err(validation_error("gossip fanout cannot be zero"));
        }
        if self.fanout > 100 {
            return Err(validation_error("gossip fanout cannot exceed 100"));
        }

        // Validate history length
        if self.history_length == 0 {
            return Err(validation_error("gossip history_length cannot be zero"));
        }
        if self.history_length > 10000 {
            return Err(validation_error(
                "gossip history_length cannot exceed 10000",
            ));
        }

        Ok(())
    }
}

/// NAT traversal configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NatConfig {
    /// Enable UPnP for automatic port mapping
    #[serde(default = "default_upnp_enabled")]
    pub upnp_enabled: bool,

    /// Enable STUN for discovering external address
    #[serde(default = "default_stun_enabled")]
    pub stun_enabled: bool,

    /// STUN servers
    #[serde(default = "default_stun_servers")]
    pub stun_servers: Vec<String>,

    /// Enable port mapping reservation
    #[serde(default = "default_port_mapping_enabled")]
    pub port_mapping_enabled: bool,

    /// Port mapping lease duration
    #[serde(default = "default_port_mapping_lease")]
    pub port_mapping_lease: std::time::Duration,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            upnp_enabled: default_upnp_enabled(),
            stun_enabled: default_stun_enabled(),
            stun_servers: default_stun_servers(),
            port_mapping_enabled: default_port_mapping_enabled(),
            port_mapping_lease: default_port_mapping_lease(),
        }
    }
}

impl ValidateConfig for NatConfig {
    fn validate(&self) -> Result<()> {
        if self.port_mapping_enabled && self.port_mapping_lease.is_zero() {
            return Err(validation_error(
                "port_mapping_lease cannot be zero when port_mapping is enabled",
            ));
        }

        // Validate STUN servers
        for server in &self.stun_servers {
            if server.is_empty() {
                return Err(validation_error("STUN server address cannot be empty"));
            }
        }

        Ok(())
    }
}

/// QUIC transport configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QuicConfig {
    /// Enable QUIC transport
    #[serde(default = "default_quic_enabled")]
    pub enabled: bool,

    /// QUIC port (if different from main port)
    #[serde(default)]
    pub port: Option<u16>,

    /// Maximum concurrent streams
    #[serde(default = "default_quic_max_streams")]
    pub max_streams: u32,

    /// Initial stream flow control window
    #[serde(default = "default_quic_stream_window")]
    pub stream_window: u32,

    /// Initial connection flow control window
    #[serde(default = "default_quic_connection_window")]
    pub connection_window: u32,

    /// Maximum idle timeout
    #[serde(default = "default_quic_max_idle_timeout")]
    pub max_idle_timeout: std::time::Duration,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            enabled: default_quic_enabled(),
            port: None,
            max_streams: default_quic_max_streams(),
            stream_window: default_quic_stream_window(),
            connection_window: default_quic_connection_window(),
            max_idle_timeout: default_quic_max_idle_timeout(),
        }
    }
}

impl ValidateConfig for QuicConfig {
    fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Validate port if specified
        if let Some(port) = self.port {
            validate_port(port, "quic_port")?;
        }

        // Validate max streams
        if self.max_streams == 0 {
            return Err(validation_error("quic max_streams cannot be zero"));
        }
        if self.max_streams > 10000 {
            return Err(validation_error("quic max_streams cannot exceed 10000"));
        }

        // Validate window sizes
        if self.stream_window == 0 {
            return Err(validation_error("quic stream_window cannot be zero"));
        }
        if self.connection_window == 0 {
            return Err(validation_error("quic connection_window cannot be zero"));
        }

        // Validate timeout
        if self.max_idle_timeout.is_zero() {
            return Err(validation_error("quic max_idle_timeout cannot be zero"));
        }

        Ok(())
    }
}

/// Relay configuration for NAT traversal
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayConfig {
    /// Enable relay client
    #[serde(default = "default_relay_client_enabled")]
    pub client_enabled: bool,

    /// Enable relay server
    #[serde(default)]
    pub server_enabled: bool,

    /// Relay server listen addresses
    #[serde(default)]
    pub listen_addresses: Vec<String>,

    /// Maximum relayed connections (server mode)
    #[serde(default = "default_relay_max_connections")]
    pub max_connections: u32,

    /// Relay reservation duration
    #[serde(default = "default_relay_reservation_ttl")]
    pub reservation_ttl: std::time::Duration,

    /// Maximum relayed data per second per peer (bytes)
    #[serde(default = "default_relay_max_rate")]
    pub max_rate: u64,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            client_enabled: default_relay_client_enabled(),
            server_enabled: false,
            listen_addresses: Vec::new(),
            max_connections: default_relay_max_connections(),
            reservation_ttl: default_relay_reservation_ttl(),
            max_rate: default_relay_max_rate(),
        }
    }
}

impl ValidateConfig for RelayConfig {
    fn validate(&self) -> Result<()> {
        if self.server_enabled {
            if self.listen_addresses.is_empty() {
                return Err(validation_error(
                    "relay server listen_addresses cannot be empty when server is enabled",
                ));
            }
            if self.max_connections == 0 {
                return Err(validation_error(
                    "relay max_connections cannot be zero",
                ));
            }
            if self.reservation_ttl.is_zero() {
                return Err(validation_error(
                    "relay reservation_ttl cannot be zero",
                ));
            }
            if self.max_rate == 0 {
                return Err(validation_error("relay max_rate cannot be zero"));
            }
        }

        Ok(())
    }
}

// Default value functions
fn default_p2p_enabled() -> bool {
    false // Disabled by default
}

fn default_discovery_interval() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_DISCOVERY_INTERVAL_SECS)
}

fn default_connection_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_CONNECTION_TIMEOUT_SECS)
}

fn default_dial_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_DIAL_TIMEOUT_SECS)
}

fn default_keep_alive_interval() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_KEEP_ALIVE_INTERVAL_SECS)
}

fn default_max_peers() -> u32 {
    DEFAULT_MAX_PEERS
}

fn default_min_peers() -> u32 {
    DEFAULT_MIN_PEERS
}

fn default_bootstrap_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_BOOTSTRAP_TIMEOUT_SECS)
}

fn default_mdns_enabled() -> bool {
    true
}

fn default_dht_enabled() -> bool {
    true
}

fn default_dht_config_enabled() -> bool {
    true
}

fn default_replication_factor() -> u8 {
    DEFAULT_DHT_REPLICATION_FACTOR
}

fn default_record_ttl() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_DHT_RECORD_TTL_SECS)
}

fn default_dht_query_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(30)
}

fn default_max_stored_records() -> u32 {
    10000
}

fn default_provider_ttl() -> std::time::Duration {
    std::time::Duration::from_secs(7200)
}

fn default_gossip_enabled() -> bool {
    true
}

fn default_gossip_interval() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_GOSSIP_INTERVAL_SECS)
}

fn default_gossip_max_message_size() -> usize {
    1024 * 1024 // 1 MB
}

fn default_gossip_fanout() -> u8 {
    6
}

fn default_gossip_history_length() -> u32 {
    1000
}

fn default_upnp_enabled() -> bool {
    true
}

fn default_stun_enabled() -> bool {
    true
}

fn default_stun_servers() -> Vec<String> {
    vec![
        "stun.l.google.com:19302".to_string(),
        "stun1.l.google.com:19302".to_string(),
    ]
}

fn default_port_mapping_enabled() -> bool {
    true
}

fn default_port_mapping_lease() -> std::time::Duration {
    std::time::Duration::from_secs(3600)
}

fn default_quic_enabled() -> bool {
    true
}

fn default_quic_max_streams() -> u32 {
    100
}

fn default_quic_stream_window() -> u32 {
    1024 * 1024 // 1 MB
}

fn default_quic_connection_window() -> u32 {
    10 * 1024 * 1024 // 10 MB
}

fn default_quic_max_idle_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(60)
}

fn default_max_message_size() -> usize {
    DEFAULT_MAX_MESSAGE_SIZE
}

fn default_relay_enabled() -> bool {
    true
}

fn default_relay_client_enabled() -> bool {
    true
}

fn default_relay_max_connections() -> u32 {
    100
}

fn default_relay_reservation_ttl() -> std::time::Duration {
    std::time::Duration::from_secs(3600)
}

fn default_relay_max_rate() -> u64 {
    1024 * 1024 // 1 MB/s
}

fn default_hole_punching_enabled() -> bool {
    true
}

fn default_protocol_version() -> String {
    "cis/1.1.4".to_string()
}

fn default_network_id() -> String {
    "cis-mainnet".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2p_config_default() {
        let config = P2PConfig::default();
        assert!(!config.enabled);
        assert!(config.bootstrap_nodes.is_empty());
        assert_eq!(config.max_peers, 50);
        assert_eq!(config.min_peers, 3);
        assert!(config.mdns_enabled);
        assert!(config.dht_enabled);
        assert!(config.relay_enabled);
        assert!(config.hole_punching_enabled);
        assert_eq!(config.protocol_version, "cis/1.1.4");
        assert_eq!(config.network_id, "cis-mainnet");
    }

    #[test]
    fn test_p2p_config_validate_disabled() {
        let config = P2PConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_p2p_config_validate_enabled() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_p2p_config_validate_min_peers_exceeds_max() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.min_peers = 100;
        config.max_peers = 50;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("min_peers"));
    }

    #[test]
    fn test_p2p_config_validate_zero_max_peers() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.max_peers = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_peers"));
    }

    #[test]
    fn test_p2p_config_validate_max_peers_too_high() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.max_peers = 1001;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_peers"));
    }

    #[test]
    fn test_p2p_config_validate_invalid_bootstrap_node() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.bootstrap_nodes = vec!["invalid-address".to_string()];
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("multiaddress"));
    }

    #[test]
    fn test_p2p_config_validate_valid_bootstrap_node() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.bootstrap_nodes = vec!["/ip4/192.168.1.1/tcp/7677".to_string()];
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_p2p_config_validate_empty_protocol_version() {
        let mut config = P2PConfig::default();
        config.enabled = true;
        config.protocol_version = String::new();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("protocol_version"));
    }

    #[test]
    fn test_dht_config_validate() {
        let config = DhtConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_gossip_config_validate() {
        let config = GossipConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_quic_config_validate() {
        let config = QuicConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_p2p_config_serialize() {
        let config = P2PConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("bootstrap_nodes"));
        assert!(toml.contains("max_peers"));
    }

    #[test]
    fn test_p2p_config_deserialize() {
        let toml = r#"
            enabled = true
            max_peers = 100
            min_peers = 5
            protocol_version = "cis/1.0.0"
            network_id = "testnet"
        "#;
        let config: P2PConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.max_peers, 100);
        assert_eq!(config.min_peers, 5);
        assert_eq!(config.protocol_version, "cis/1.0.0");
        assert_eq!(config.network_id, "testnet");
    }
}
