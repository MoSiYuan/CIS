//! # Federation Types
//!
//! Data types for CIS Matrix Federation (BMI - Between Machine Interface).
//!
//! ## Port
//!
//! Default federation port: 6767

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// Default federation port
pub const FEDERATION_PORT: u16 = 6767;

/// Federation API version path
pub const FEDERATION_API_VERSION: &str = "/_cis/v1";

/// CIS Matrix Event format for inter-node communication
///
/// This is a simplified Matrix event format used for federation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CisMatrixEvent {
    /// Event ID (globally unique)
    pub event_id: String,
    
    /// Room ID
    pub room_id: String,
    
    /// Sender user ID
    pub sender: String,
    
    /// Event type (e.g., "m.room.message", "cis.task.request")
    pub event_type: String,
    
    /// Event content
    pub content: serde_json::Value,
    
    /// Origin server timestamp (milliseconds since epoch)
    pub origin_server_ts: i64,
    
    /// Unsigned data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<serde_json::Value>,
    
    /// State key for state events (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    
    /// Origin server name (set by receiving server)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    
    /// Signatures from origin server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
    
    /// Hash of the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<HashMap<String, String>>,
}

impl CisMatrixEvent {
    /// Create a new CIS Matrix event
    pub fn new(
        event_id: impl Into<String>,
        room_id: impl Into<String>,
        sender: impl Into<String>,
        event_type: impl Into<String>,
        content: serde_json::Value,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            room_id: room_id.into(),
            sender: sender.into(),
            event_type: event_type.into(),
            content,
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
            unsigned: None,
            state_key: None,
            origin: None,
            signatures: None,
            hashes: None,
        }
    }
    
    /// Set the origin server
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }
    
    /// Set unsigned data
    pub fn with_unsigned(mut self, unsigned: serde_json::Value) -> Self {
        self.unsigned = Some(unsigned);
        self
    }
    
    /// Set state key
    pub fn with_state_key(mut self, state_key: impl Into<String>) -> Self {
        self.state_key = Some(state_key.into());
        self
    }
}

/// Peer information for known CIS nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer server name (e.g., "kitchen.local", "living.local")
    pub server_name: String,
    
    /// Display name for this peer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    
    /// Hostname or IP address
    pub host: String,
    
    /// Port number (default: 6767)
    pub port: u16,
    
    /// Whether to use HTTPS
    pub use_https: bool,
    
    /// Public key for signature verification (base64 encoded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    
    /// Whether this peer is trusted
    pub trusted: bool,
    
    /// Last seen timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
    
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    
    /// Source of this peer: "mdns", "manual", "cloud_seed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    
    /// Expected DID for cloud seed nodes (verification key like LLM API key)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_did: Option<String>,
    
    /// Whether DID verification passed
    #[serde(default)]
    pub did_verified: bool,
    
    /// Connection type: "direct", "relay", "cloud"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<String>,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(server_name: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            display_name: None,
            host: host.into(),
            port: FEDERATION_PORT,
            use_https: false,
            public_key: None,
            trusted: false,
            last_seen: None,
            metadata: None,
            source: Some("manual".to_string()),
            expected_did: None,
            did_verified: false,
            connection_type: None,
        }
    }
    
    /// Create a cloud seed node (with DID verification)
    pub fn new_cloud_seed(
        server_name: impl Into<String>, 
        host: impl Into<String>,
        expected_did: impl Into<String>
    ) -> Self {
        Self {
            server_name: server_name.into(),
            display_name: None,
            host: host.into(),
            port: FEDERATION_PORT,
            use_https: false,
            public_key: None,
            trusted: true, // Cloud seeds are trusted by default
            last_seen: None,
            metadata: None,
            source: Some("cloud_seed".to_string()),
            expected_did: Some(expected_did.into()),
            did_verified: false,
            connection_type: None,
        }
    }
    
    /// Get display identifier
    /// Local: "Munin-macmini"
    /// Cloud: "seed.cis.dev@did:cis:seed:abc123"
    pub fn display_id(&self) -> String {
        match (&self.source, &self.expected_did) {
            (Some(s), Some(did)) if s == "cloud_seed" => {
                format!("{}@{}", self.host, did)
            }
            _ => self.display_name.clone().unwrap_or_else(|| self.server_name.clone()),
        }
    }
    
    /// Check if this is a cloud seed node
    pub fn is_cloud_seed(&self) -> bool {
        self.source.as_deref() == Some("cloud_seed")
    }
    
    /// Verify DID matches expected value
    pub fn verify_did(&self, actual_did: &str) -> bool {
        match &self.expected_did {
            Some(expected) => expected == actual_did,
            None => true, // No expected DID means no verification needed
        }
    }
    
    /// Set display name
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }
    
    /// Set port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    /// Enable HTTPS
    pub fn with_https(mut self, enabled: bool) -> Self {
        self.use_https = enabled;
        self
    }
    
    /// Set public key
    pub fn with_public_key(mut self, key: impl Into<String>) -> Self {
        self.public_key = Some(key.into());
        self
    }
    
    /// Set trusted status
    pub fn with_trusted(mut self, trusted: bool) -> Self {
        self.trusted = trusted;
        self
    }
    
    /// Set as cloud seed with expected DID
    pub fn with_expected_did(mut self, did: impl Into<String>) -> Self {
        self.expected_did = Some(did.into());
        self.source = Some("cloud_seed".to_string());
        self.trusted = true;
        self
    }
    
    /// Set source type
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
    
    /// Get the base URL for this peer
    pub fn base_url(&self) -> String {
        let scheme = if self.use_https { "https" } else { "http" };
        format!("{}://{}:{}", scheme, self.host, self.port)
    }
    
    /// Get the federation API URL for this peer
    pub fn federation_url(&self) -> String {
        format!("{}{}", self.base_url(), FEDERATION_API_VERSION)
    }
}

/// Server key response for `/_matrix/key/v2/server`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeyResponse {
    /// Server name
    pub server_name: String,
    
    /// Valid until timestamp
    pub valid_until_ts: i64,
    
    /// Verify keys
    pub verify_keys: HashMap<String, VerifyKey>,
    
    /// Old verify keys (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_verify_keys: Option<HashMap<String, OldVerifyKey>>,
    
    /// Signatures
    pub signatures: HashMap<String, HashMap<String, String>>,
    
    /// TLS fingerprints (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_fingerprints: Option<Vec<TlsFingerprint>>,
}

/// Verify key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyKey {
    /// Base64-encoded public key
    pub key: String,
}

/// Old verify key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OldVerifyKey {
    /// Base64-encoded public key
    pub key: String,
    
    /// Expired timestamp
    pub expired_ts: i64,
}

/// TLS fingerprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsFingerprint {
    /// Hash algorithm
    pub sha256: String,
}

/// Event receive response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReceiveResponse {
    /// Whether the event was accepted
    pub accepted: bool,
    
    /// Optional error message if not accepted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// Event ID that was processed
    pub event_id: String,
}

impl EventReceiveResponse {
    /// Create a successful response
    pub fn success(event_id: impl Into<String>) -> Self {
        Self {
            accepted: true,
            error: None,
            event_id: event_id.into(),
        }
    }
    
    /// Create an error response
    pub fn error(event_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            accepted: false,
            error: Some(error.into()),
            event_id: event_id.into(),
        }
    }
}

/// Federation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationConfig {
    /// Server name (must be unique in the cluster)
    pub server_name: String,
    
    /// Port to listen on (default: 6767)
    pub port: u16,
    
    /// Bind address (default: "0.0.0.0")
    pub bind_address: String,
    
    /// Whether to enable HTTPS
    pub use_https: bool,
    
    /// Path to TLS certificate (if using HTTPS)
    pub tls_cert_path: Option<String>,
    
    /// Path to TLS key (if using HTTPS)
    pub tls_key_path: Option<String>,
    
    /// Whether to use mTLS (mutual TLS)
    pub use_mtls: bool,
    
    /// Path to CA certificate for mTLS
    pub ca_cert_path: Option<String>,
    
    /// List of known peers (manual configuration)
    pub known_peers: Vec<PeerInfo>,
    
    /// Whether to enable mDNS discovery
    pub enable_mdns: bool,
    
    /// Whether to verify peer signatures
    pub verify_signatures: bool,
    
    /// Public key for this server (base64 encoded)
    pub public_key: Option<String>,
    
    /// Private key for this server (base64 encoded) - should be stored securely
    pub private_key: Option<String>,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            server_name: "cis.local".to_string(),
            port: FEDERATION_PORT,
            bind_address: "0.0.0.0".to_string(),
            use_https: false,
            tls_cert_path: None,
            tls_key_path: None,
            use_mtls: false,
            ca_cert_path: None,
            known_peers: Vec::new(),
            enable_mdns: false,
            verify_signatures: false,
            public_key: None,
            private_key: None,
        }
    }
}

impl FederationConfig {
    /// Create a new federation config with the given server name
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            ..Default::default()
        }
    }
    
    /// Set port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    /// Add a known peer
    pub fn with_peer(mut self, peer: PeerInfo) -> Self {
        self.known_peers.push(peer);
        self
    }
    
    /// Enable mDNS discovery
    pub fn with_mdns(mut self, enabled: bool) -> Self {
        self.enable_mdns = enabled;
        self
    }
    
    /// Enable HTTPS
    pub fn with_https(mut self, enabled: bool) -> Self {
        self.use_https = enabled;
        self
    }
    
    /// Enable mTLS
    pub fn with_mtls(mut self, enabled: bool) -> Self {
        self.use_mtls = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cis_matrix_event_creation() {
        let event = CisMatrixEvent::new(
            "$event123",
            "!room123:cis.local",
            "@alice:cis.local",
            "m.room.message",
            serde_json::json!({ "body": "Hello", "msgtype": "m.text" }),
        );
        
        assert_eq!(event.event_id, "$event123");
        assert_eq!(event.room_id, "!room123:cis.local");
        assert_eq!(event.sender, "@alice:cis.local");
        assert_eq!(event.event_type, "m.room.message");
    }

    #[test]
    fn test_peer_info() {
        let peer = PeerInfo::new("kitchen.local", "kitchen.local")
            .with_display_name("Kitchen Node")
            .with_trusted(true);
        
        assert_eq!(peer.server_name, "kitchen.local");
        assert_eq!(peer.display_name, Some("Kitchen Node".to_string()));
        assert!(peer.trusted);
        assert_eq!(peer.base_url(), "http://kitchen.local:6767");
        assert_eq!(peer.federation_url(), "http://kitchen.local:6767/_cis/v1");
    }

    #[test]
    fn test_peer_info_https() {
        let peer = PeerInfo::new("secure.local", "secure.local")
            .with_https(true);
        
        assert_eq!(peer.base_url(), "https://secure.local:6767");
    }

    #[test]
    fn test_event_receive_response() {
        let success = EventReceiveResponse::success("$event123");
        assert!(success.accepted);
        assert_eq!(success.event_id, "$event123");
        assert!(success.error.is_none());
        
        let error = EventReceiveResponse::error("$event456", "Invalid signature");
        assert!(!error.accepted);
        assert_eq!(error.event_id, "$event456");
        assert_eq!(error.error, Some("Invalid signature".to_string()));
    }
}

/// Room information for federation queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    /// Room ID
    pub room_id: String,
    /// Room creator
    pub creator: String,
    /// Room name
    pub name: Option<String>,
    /// Room topic
    pub topic: Option<String>,
    /// Whether room is federated
    pub federate: bool,
    /// Creation timestamp
    pub created_at: i64,
}

impl RoomInfo {
    /// Create new room info
    pub fn new(
        room_id: impl Into<String>,
        creator: impl Into<String>,
    ) -> Self {
        Self {
            room_id: room_id.into(),
            creator: creator.into(),
            name: None,
            topic: None,
            federate: true,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Set room name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set room topic
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Set federate flag
    pub fn with_federate(mut self, federate: bool) -> Self {
        self.federate = federate;
        self
    }
}

/// Discovery source for a discovered node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscoverySource {
    /// Discovered via mDNS
    Mdns,
    /// Manually configured
    Manual,
    /// Discovered via DHT
    Dht,
    /// Discovered via seed node
    Seed,
}

/// Discovered node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredNode {
    /// Node ID (unique identifier)
    pub node_id: String,
    /// DID (Decentralized Identifier)
    pub did: String,
    /// Network address
    pub address: SocketAddr,
    /// Discovery source
    pub source: DiscoverySource,
    /// Server name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// Node version (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Capabilities (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    /// Last seen timestamp
    pub last_seen: i64,
}

impl DiscoveredNode {
    /// Create a new discovered node
    pub fn new(
        node_id: impl Into<String>,
        did: impl Into<String>,
        address: SocketAddr,
        source: DiscoverySource,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            did: did.into(),
            address,
            source,
            server_name: None,
            version: None,
            capabilities: None,
            last_seen: chrono::Utc::now().timestamp(),
        }
    }

    /// Set server name
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = Some(server_name.into());
        self
    }

    /// Set version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Update last_seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = chrono::Utc::now().timestamp();
    }
}
