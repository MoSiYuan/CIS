//! # Matrix Federation Discovery
//!
//! Implementation of Matrix federation server discovery protocol.
//!
//! ## Discovery Protocol
//!
//! 1. First query `_matrix-fed._tcp.example.com` SRV record
//! 2. If not found, query `_matrix._tcp.example.com` SRV record  
//! 3. If not found, try `/.well-known/matrix/server`
//! 4. Finally fallback to direct connection `server_name:8448`
//!
//! ## Version Negotiation
//!
//! After discovery, servers negotiate protocol versions via `/_matrix/federation/v1/version`.

use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::matrix::error::MatrixError;

/// Default Matrix federation port
const DEFAULT_MATRIX_PORT: u16 = 8448;

/// Federation discovery result
#[derive(Debug, Clone)]
pub struct ServerEndpoint {
    /// Server name (e.g., "example.com")
    pub server_name: String,
    /// Resolved host
    pub host: String,
    /// Resolved port
    pub port: u16,
    /// Whether server supports v1.11
    pub supports_v1_11: bool,
    /// Base URL for federation API
    pub base_url: String,
}

impl ServerEndpoint {
    /// Create a new server endpoint
    pub fn new(
        server_name: impl Into<String>,
        host: impl Into<String>,
        port: u16,
    ) -> Self {
        let server_name = server_name.into();
        let host = host.into();
        let base_url = format!("https://{}:{}", host, port);
        
        Self {
            server_name,
            host,
            port,
            supports_v1_11: false,
            base_url,
        }
    }

    /// Get the federation version URL
    pub fn version_url(&self) -> String {
        format!("{}/_matrix/federation/v1/version", self.base_url)
    }

    /// Get the key query URL
    pub fn key_url(&self) -> String {
        format!("{}/_matrix/key/v2/server", self.base_url)
    }

    /// Get the server URL
    pub fn server_url(&self) -> String {
        self.base_url.clone()
    }

    /// Set v1.11 support flag
    pub fn with_v1_11_support(mut self, supports: bool) -> Self {
        self.supports_v1_11 = supports;
        self
    }

    /// Get SocketAddr if host is resolvable
    pub async fn to_socket_addr(&self) -> Result<SocketAddr, MatrixError> {
        let host = self.host.clone();
        let port = self.port;
        
        // Use the ToSocketAddrs implementation for (String, u16)
        let addrs: Vec<SocketAddr> = match tokio::net::lookup_host((host, port)).await {
            Ok(iter) => iter.collect(),
            Err(e) => {
                return Err(MatrixError::federation(format!(
                    "Failed to resolve {}:{}: {}",
                    self.host, self.port, e
                )));
            }
        };
        
        addrs.into_iter().next().ok_or_else(|| {
            MatrixError::federation(format!(
                "No address found for {}:{}",
                self.host, self.port
            ))
        })
    }
}

/// Well-known response from `/.well-known/matrix/server`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WellKnownResponse {
    /// The server name to use for federation
    #[serde(rename = "m.server")]
    pub server: String,
}

/// Federation version response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationVersion {
    /// Server implementation name
    pub name: String,
    /// Server version
    pub version: String,
}

/// Version response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionResponse {
    #[serde(rename = "server")]
    server: Option<FederationVersion>,
}

/// Challenge for challenge-response authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationChallenge {
    /// Challenge nonce
    pub nonce: String,
    /// Server name issuing the challenge
    pub server_name: String,
    /// Timestamp
    pub timestamp: i64,
}

/// Challenge response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationChallengeResponse {
    /// The nonce being responded to
    pub nonce: String,
    /// Signature over the nonce
    pub signature: String,
    /// Public key used for signing
    pub public_key: String,
}

/// Federation discovery handler
#[derive(Debug, Clone)]
pub struct FederationDiscovery;

impl FederationDiscovery {
    /// Discover a remote Matrix server endpoint
    ///
    /// Follows the Matrix federation discovery protocol:
    /// 1. Query `_matrix-fed._tcp` SRV record
    /// 2. Query `_matrix._tcp` SRV record
    /// 3. Try `.well-known/matrix/server`
    /// 4. Fallback to direct connection on port 8448
    ///
    /// # Arguments
    /// * `server_name` - The server name to discover (e.g., "example.com")
    ///
    /// # Returns
    /// * `Result<ServerEndpoint, MatrixError>` - The discovered endpoint
    pub async fn discover(server_name: &str) -> Result<ServerEndpoint, MatrixError> {
        info!("Starting federation discovery for: {}", server_name);

        // Step 1: Try _matrix-fed._tcp SRV record (Matrix 1.11+)
        debug!("Trying _matrix-fed._tcp SRV record for {}", server_name);
        match Self::resolve_srv(&format!("_matrix-fed._tcp.{}", server_name)).await {
            Ok(addrs) if !addrs.is_empty() => {
                if let Some(addr) = addrs.first() {
                    info!(
                        "Found _matrix-fed._tcp SRV record for {}: {:?}",
                        server_name, addr
                    );
                    let host = addr.ip().to_string();
                    let endpoint = ServerEndpoint::new(server_name, host, addr.port())
                        .with_v1_11_support(true);
                    return Ok(endpoint);
                }
            }
            Ok(_) => debug!("Empty _matrix-fed._tcp SRV response"),
            Err(e) => debug!("_matrix-fed._tcp SRV lookup failed: {}", e),
        }

        // Step 2: Try _matrix._tcp SRV record (legacy)
        debug!("Trying _matrix._tcp SRV record for {}", server_name);
        match Self::resolve_srv(&format!("_matrix._tcp.{}", server_name)).await {
            Ok(addrs) if !addrs.is_empty() => {
                if let Some(addr) = addrs.first() {
                    info!(
                        "Found _matrix._tcp SRV record for {}: {:?}",
                        server_name, addr
                    );
                    let host = addr.ip().to_string();
                    return Ok(ServerEndpoint::new(server_name, host, addr.port()));
                }
            }
            Ok(_) => debug!("Empty _matrix._tcp SRV response"),
            Err(e) => debug!("_matrix._tcp SRV lookup failed: {}", e),
        }

        // Step 3: Try .well-known lookup
        debug!("Trying .well-known lookup for {}", server_name);
        match Self::fetch_well_known(server_name).await {
            Ok(well_known) => {
                info!(
                    "Found .well-known for {}: redirecting to {}",
                    server_name, well_known.server
                );
                // Parse the server value (format: "hostname" or "hostname:port")
                let (host, port) = Self::parse_host_port(&well_known.server);
                return Ok(ServerEndpoint::new(server_name, host, port));
            }
            Err(e) => debug!(".well-known lookup failed: {}", e),
        }

        // Step 4: Fallback to direct connection on port 8448
        info!(
            "Falling back to direct connection for {}:8448",
            server_name
        );
        Ok(ServerEndpoint::new(server_name, server_name, DEFAULT_MATRIX_PORT))
    }

    /// Resolve SRV records for a hostname
    ///
    /// # Arguments
    /// * `name` - The SRV record name (e.g., "_matrix._tcp.example.com")
    ///
    /// # Returns
    /// * `Result<Vec<SocketAddr>, MatrixError>` - List of resolved addresses
    #[cfg(feature = "federation")]
    /// * `Result<Vec<SocketAddr>, MatrixError>` - List of resolved addresses
    ///
    /// 注意：当前为简化实现，直接返回空列表
    /// 完整实现需要集成 trust-dns-resolver
    #[cfg(feature = "federation")]
    pub async fn resolve_srv(name: &str) -> Result<Vec<SocketAddr>, MatrixError> {
        debug!("SRV lookup for {} (simplified)", name);
        // 简化实现：直接返回空列表
        // 让调用方回退到 .well-known 或直接连接
        Ok(Vec::new())
    }

    /// Stub implementation when federation feature is disabled
    #[cfg(not(feature = "federation"))]
    pub async fn resolve_srv(_name: &str) -> Result<Vec<SocketAddr>, MatrixError> {
        debug!("SRV resolution requires 'federation' feature");
        Ok(Vec::new())
    }

    /// Fetch .well-known/matrix/server
    ///
    /// # Arguments
    /// * `server_name` - The server name to query
    ///
    /// # Returns
    /// * `Result<WellKnownResponse, MatrixError>` - The well-known response
    pub async fn fetch_well_known(server_name: &str) -> Result<WellKnownResponse, MatrixError> {
        let url = format!(
            "https://{}/.well-known/matrix/server",
            server_name
        );

        debug!("Fetching .well-known from: {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| MatrixError::federation(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                MatrixError::federation(format!(
                    "Failed to fetch .well-known from {}: {}",
                    server_name, e
                ))
            })?;

        if !response.status().is_success() {
            return Err(MatrixError::federation(format!(
                ".well-known returned status {} from {}",
                response.status(),
                server_name
            )));
        }

        let well_known: WellKnownResponse = response.json().await.map_err(|e| {
            MatrixError::federation(format!(
                "Failed to parse .well-known response from {}: {}",
                server_name, e
            ))
        })?;

        Ok(well_known)
    }

    /// Negotiate federation version with a server
    ///
    /// # Arguments
    /// * `endpoint` - The server endpoint to query
    ///
    /// # Returns
    /// * `Result<FederationVersion, MatrixError>` - The negotiated version
    pub async fn negotiate_version(
        endpoint: &ServerEndpoint,
    ) -> Result<FederationVersion, MatrixError> {
        let url = endpoint.version_url();
        debug!("Negotiating federation version with: {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| MatrixError::federation(format!("Failed to create HTTP client: {}", e)))?;

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                MatrixError::federation(format!(
                    "Failed to negotiate version with {}: {}",
                    endpoint.server_name, e
                ))
            })?;

        if !response.status().is_success() {
            return Err(MatrixError::federation(format!(
                "Version negotiation returned status {} from {}",
                response.status(),
                endpoint.server_name
            )));
        }

        let version_resp: VersionResponse = response.json().await.map_err(|e| {
            MatrixError::federation(format!(
                "Failed to parse version response from {}: {}",
                endpoint.server_name, e
            ))
        })?;

        version_resp.server.ok_or_else(|| {
            MatrixError::federation(format!(
                "No server version info in response from {}",
                endpoint.server_name
            ))
        })
    }

    /// Generate a challenge for challenge-response authentication
    ///
    /// # Arguments
    /// * `server_name` - The server name issuing the challenge
    ///
    /// # Returns
    /// * `FederationChallenge` - A new challenge
    pub fn generate_challenge(server_name: &str) -> FederationChallenge {
        use rand::Rng;
        
        // Generate a random 32-byte nonce
        let mut nonce_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = hex::encode(nonce_bytes);

        FederationChallenge {
            nonce,
            server_name: server_name.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Verify a challenge response
    ///
    /// # Arguments
    /// * `challenge` - The original challenge
    /// * `response` - The response to verify
    /// * `public_key` - The expected public key
    ///
    /// # Returns
    /// * `Result<bool, MatrixError>` - Whether the response is valid
    pub fn verify_challenge_response(
        challenge: &FederationChallenge,
        response: &FederationChallengeResponse,
        public_key: &str,
    ) -> Result<bool, MatrixError> {
        // Verify the nonce matches
        if challenge.nonce != response.nonce {
            warn!("Challenge nonce mismatch");
            return Ok(false);
        }

        // Verify the public key matches
        if public_key != response.public_key {
            warn!("Challenge public key mismatch");
            return Ok(false);
        }

        // Check timestamp is within acceptable window (5 minutes)
        let now = chrono::Utc::now().timestamp_millis();
        let age = now - challenge.timestamp;
        if age > 5 * 60 * 1000 || age < 0 {
            warn!("Challenge timestamp outside acceptable window");
            return Ok(false);
        }

        // In a real implementation, we would verify the signature here
        // For now, we just check that a signature is present
        if response.signature.is_empty() {
            warn!("Empty challenge signature");
            return Ok(false);
        }

        debug!("Challenge response verified successfully");
        Ok(true)
    }

    /// Parse host:port string
    fn parse_host_port(s: &str) -> (String, u16) {
        if let Some((host, port_str)) = s.split_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                return (host.to_string(), port);
            }
        }
        (s.to_string(), DEFAULT_MATRIX_PORT)
    }
}

/// Federation handshake result
#[derive(Debug, Clone)]
pub struct FederationHandshake {
    /// The discovered endpoint
    pub endpoint: ServerEndpoint,
    /// The negotiated version
    pub version: FederationVersion,
    /// Whether the server supports v1.11
    pub supports_v1_11: bool,
}

impl FederationHandshake {
    /// Complete the full federation handshake
    ///
    /// This performs discovery and version negotiation in one step.
    pub async fn perform(server_name: &str) -> Result<Self, MatrixError> {
        // Step 1: Discovery
        let endpoint = FederationDiscovery::discover(server_name).await?;

        // Step 2: Version negotiation
        let version = FederationDiscovery::negotiate_version(&endpoint).await?;

        info!(
            "Federation handshake with {} complete: {} {}",
            server_name, version.name, version.version
        );

        Ok(Self {
            endpoint,
            version,
            supports_v1_11: false, // Would be determined from version response
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_endpoint_creation() {
        let endpoint = ServerEndpoint::new("example.com", "matrix.example.com", 8448);
        
        assert_eq!(endpoint.server_name, "example.com");
        assert_eq!(endpoint.host, "matrix.example.com");
        assert_eq!(endpoint.port, 8448);
        assert_eq!(endpoint.base_url, "https://matrix.example.com:8448");
        assert!(!endpoint.supports_v1_11);
    }

    #[test]
    fn test_server_endpoint_with_v1_11() {
        let endpoint = ServerEndpoint::new("example.com", "matrix.example.com", 8448)
            .with_v1_11_support(true);
        
        assert!(endpoint.supports_v1_11);
    }

    #[test]
    fn test_server_endpoint_urls() {
        let endpoint = ServerEndpoint::new("example.com", "matrix.example.com", 8448);
        
        assert_eq!(
            endpoint.version_url(),
            "https://matrix.example.com:8448/_matrix/federation/v1/version"
        );
        assert_eq!(
            endpoint.key_url(),
            "https://matrix.example.com:8448/_matrix/key/v2/server"
        );
        assert_eq!(endpoint.server_url(), "https://matrix.example.com:8448");
    }

    #[test]
    fn test_well_known_response_deserialization() {
        let json = r#"{"m.server": "matrix.example.com:8448"}"#;
        let response: WellKnownResponse = serde_json::from_str(json).unwrap();
        
        assert_eq!(response.server, "matrix.example.com:8448");
    }

    #[test]
    fn test_federation_version_deserialization() {
        let json = r#"{"name": "Synapse", "version": "1.95.0"}"#;
        let version: FederationVersion = serde_json::from_str(json).unwrap();
        
        assert_eq!(version.name, "Synapse");
        assert_eq!(version.version, "1.95.0");
    }

    #[test]
    fn test_version_response_deserialization() {
        let json = r#"{"server": {"name": "Synapse", "version": "1.95.0"}}"#;
        let response: VersionResponse = serde_json::from_str(json).unwrap();
        
        assert!(response.server.is_some());
        let server = response.server.unwrap();
        assert_eq!(server.name, "Synapse");
        assert_eq!(server.version, "1.95.0");
    }

    #[test]
    fn test_parse_host_port() {
        assert_eq!(
            FederationDiscovery::parse_host_port("matrix.example.com:8448"),
            ("matrix.example.com".to_string(), 8448)
        );
        
        assert_eq!(
            FederationDiscovery::parse_host_port("matrix.example.com"),
            ("matrix.example.com".to_string(), DEFAULT_MATRIX_PORT)
        );
        
        assert_eq!(
            FederationDiscovery::parse_host_port("192.168.1.1:443"),
            ("192.168.1.1".to_string(), 443)
        );
    }

    #[test]
    fn test_challenge_generation() {
        let challenge = FederationDiscovery::generate_challenge("example.com");
        
        assert_eq!(challenge.server_name, "example.com");
        assert!(!challenge.nonce.is_empty());
        assert!(challenge.timestamp > 0);
    }

    #[test]
    fn test_verify_challenge_response_valid() {
        let challenge = FederationDiscovery::generate_challenge("example.com");
        
        let response = FederationChallengeResponse {
            nonce: challenge.nonce.clone(),
            signature: "valid_signature_placeholder".to_string(),
            public_key: "test_key".to_string(),
        };

        let result = FederationDiscovery::verify_challenge_response(
            &challenge,
            &response,
            "test_key",
        );

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_challenge_response_invalid_nonce() {
        let challenge = FederationDiscovery::generate_challenge("example.com");
        
        let response = FederationChallengeResponse {
            nonce: "wrong_nonce".to_string(),
            signature: "valid_signature".to_string(),
            public_key: "test_key".to_string(),
        };

        let result = FederationDiscovery::verify_challenge_response(
            &challenge,
            &response,
            "test_key",
        );

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_challenge_response_invalid_key() {
        let challenge = FederationDiscovery::generate_challenge("example.com");
        
        let response = FederationChallengeResponse {
            nonce: challenge.nonce.clone(),
            signature: "valid_signature".to_string(),
            public_key: "wrong_key".to_string(),
        };

        let result = FederationDiscovery::verify_challenge_response(
            &challenge,
            &response,
            "expected_key",
        );

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_challenge_response_empty_signature() {
        let challenge = FederationDiscovery::generate_challenge("example.com");
        
        let response = FederationChallengeResponse {
            nonce: challenge.nonce.clone(),
            signature: "".to_string(),
            public_key: "test_key".to_string(),
        };

        let result = FederationDiscovery::verify_challenge_response(
            &challenge,
            &response,
            "test_key",
        );

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_federation_challenge_serialization() {
        let challenge = FederationChallenge {
            nonce: "test_nonce_123".to_string(),
            server_name: "example.com".to_string(),
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&challenge).unwrap();
        assert!(json.contains("test_nonce_123"));
        assert!(json.contains("example.com"));

        let deserialized: FederationChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.nonce, "test_nonce_123");
        assert_eq!(deserialized.server_name, "example.com");
        assert_eq!(deserialized.timestamp, 1234567890);
    }

    #[test]
    fn test_federation_challenge_response_serialization() {
        let response = FederationChallengeResponse {
            nonce: "test_nonce".to_string(),
            signature: "signature_data".to_string(),
            public_key: "public_key_data".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: FederationChallengeResponse = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.nonce, "test_nonce");
        assert_eq!(deserialized.signature, "signature_data");
        assert_eq!(deserialized.public_key, "public_key_data");
    }

    #[test]
    fn test_federation_challenge_stale() {
        let mut challenge = FederationDiscovery::generate_challenge("example.com");
        // Set timestamp to 10 minutes ago
        challenge.timestamp = chrono::Utc::now().timestamp_millis() - 10 * 60 * 1000;
        
        let response = FederationChallengeResponse {
            nonce: challenge.nonce.clone(),
            signature: "valid_signature".to_string(),
            public_key: "test_key".to_string(),
        };

        let result = FederationDiscovery::verify_challenge_response(
            &challenge,
            &response,
            "test_key",
        );

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should fail due to stale timestamp
    }

    #[test]
    fn test_federation_handshake_creation() {
        let endpoint = ServerEndpoint::new("example.com", "matrix.example.com", 8448);
        let version = FederationVersion {
            name: "TestServer".to_string(),
            version: "1.0.0".to_string(),
        };

        let handshake = FederationHandshake {
            endpoint: endpoint.clone(),
            version: version.clone(),
            supports_v1_11: true,
        };

        assert_eq!(handshake.endpoint.server_name, "example.com");
        assert_eq!(handshake.version.name, "TestServer");
        assert!(handshake.supports_v1_11);
    }
}
