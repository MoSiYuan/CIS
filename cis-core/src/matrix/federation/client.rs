//! # Federation HTTP Client
//!
//! HTTP client for sending events to other CIS nodes.
//!
//! ## Features
//!
//! - Event forwarding to multiple peers
//! - TLS/mTLS support
//! - Connection pooling
//! - Retry logic
//!
//! ## Example
//!
//! ```no_run
//! use cis_core::matrix::federation::{FederationClient, PeerInfo, CisMatrixEvent};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = FederationClient::new()?;
//!
//! let peer = PeerInfo::new("kitchen.local", "kitchen.local");
//! let event = CisMatrixEvent::new(
//!     "$event123",
//!     "!room:cis.local",
//!     "@alice:cis.local",
//!     "m.room.message",
//!     serde_json::json!({ "body": "Hello" }),
//! );
//!
//! client.send_event(&peer, &event).await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode};
use tracing::{debug, error, info, warn};

use super::types::{CisMatrixEvent, EventReceiveResponse, PeerInfo, FEDERATION_API_VERSION};

/// HTTP client for federation
#[derive(Debug, Clone)]
pub struct FederationClient {
    /// Underlying HTTP client
    client: Client,
    
    /// Request timeout
    timeout: Duration,
    
    /// Maximum retries for failed requests
    max_retries: u32,
}

/// Result type for federation client operations
pub type FederationClientResult<T> = Result<T, FederationClientError>;

/// Errors that can occur during federation client operations
#[derive(Debug, thiserror::Error)]
pub enum FederationClientError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    /// Peer rejected the event
    #[error("Peer rejected event: {0}")]
    Rejected(String),
    
    /// Invalid response from peer
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    /// Timeout
    #[error("Request timeout")]
    Timeout,
    
    /// Max retries exceeded
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
    
    /// TLS configuration error
    #[error("TLS error: {0}")]
    TlsError(String),
}

impl FederationClient {
    /// Create a new federation client with default configuration
    pub fn new() -> FederationClientResult<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(FederationClientError::HttpError)?;
        
        Ok(Self {
            client,
            timeout: Duration::from_secs(30),
            max_retries: 3,
        })
    }
    
    /// Create a new federation client with mTLS
    ///
    /// # Arguments
    ///
    /// * `cert_path` - Path to client certificate
    /// * `key_path` - Path to client private key
    /// * `ca_path` - Path to CA certificate (optional)
    pub fn with_mtls(
        cert_path: &str,
        key_path: &str,
        ca_path: Option<&str>,
    ) -> FederationClientResult<Self> {
        // Load identity (client cert + key)
        let identity = Self::load_identity(cert_path, key_path)
            .map_err(|e| FederationClientError::TlsError(e.to_string()))?;
        
        let mut builder = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .identity(identity);
        
        // Add CA certificate if provided
        if let Some(ca) = ca_path {
            let cert = Self::load_ca_cert(ca)
                .map_err(|e| FederationClientError::TlsError(e.to_string()))?;
            builder = builder.add_root_certificate(cert);
        }
        
        let client = builder
            .build()
            .map_err(FederationClientError::HttpError)?;
        
        Ok(Self {
            client,
            timeout: Duration::from_secs(30),
            max_retries: 3,
        })
    }
    
    /// Load TLS identity from certificate and key files
    fn load_identity(cert_path: &str, key_path: &str) -> Result<reqwest::Identity, Box<dyn std::error::Error>> {
        let cert = std::fs::read(cert_path)?;
        let key = std::fs::read(key_path)?;
        
        // Combine cert and key into PKCS#12 format
        // For simplicity, we assume PEM format
        let identity = reqwest::Identity::from_pem(&[cert, key].concat())?;
        Ok(identity)
    }
    
    /// Load CA certificate
    fn load_ca_cert(ca_path: &str) -> Result<reqwest::Certificate, Box<dyn std::error::Error>> {
        let cert = std::fs::read(ca_path)?;
        let cert = reqwest::Certificate::from_pem(&cert)?;
        Ok(cert)
    }
    
    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// Set maximum retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
    
    /// Send an event to a peer
    ///
    /// # Arguments
    ///
    /// * `peer` - The peer to send to
    /// * `event` - The event to send
    pub async fn send_event(
        &self,
        peer: &PeerInfo,
        event: &CisMatrixEvent,
    ) -> FederationClientResult<EventReceiveResponse> {
        let url = format!("{}{}/event/receive", peer.base_url(), FEDERATION_API_VERSION);
        
        debug!("Sending event {} to {}", event.event_id, peer.server_name);
        
        let response = self.client
            .post(&url)
            .json(event)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send event to {}: {}", peer.server_name, e);
                FederationClientError::HttpError(e)
            })?;
        
        let status = response.status();
        
        if status == StatusCode::OK || status == StatusCode::ACCEPTED {
            let result: EventReceiveResponse = response.json().await
                .map_err(|e| FederationClientError::InvalidResponse(e.to_string()))?;
            
            if result.accepted {
                info!("Event {} accepted by {}", event.event_id, peer.server_name);
            } else {
                warn!(
                    "Event {} rejected by {}: {:?}",
                    event.event_id, peer.server_name, result.error
                );
            }
            
            Ok(result)
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(FederationClientError::Rejected(format!(
                "HTTP {}: {}",
                status, text
            )))
        }
    }
    
    /// Send an event with retry logic
    ///
    /// Retries on transient failures (network errors, timeouts).
    pub async fn send_event_with_retry(
        &self,
        peer: &PeerInfo,
        event: &CisMatrixEvent,
    ) -> FederationClientResult<EventReceiveResponse> {
        let mut last_error = None;
        
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                debug!(
                    "Retrying send to {} (attempt {}/{})",
                    peer.server_name, attempt, self.max_retries
                );
                
                // Exponential backoff
                let delay = Duration::from_millis(100 * (1 << attempt));
                tokio::time::sleep(delay).await;
            }
            
            match self.send_event(peer, event).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Don't retry on client errors (4xx)
                    if let FederationClientError::Rejected(ref msg) = e {
                        if msg.contains("400") || msg.contains("401") || msg.contains("403") || msg.contains("404") {
                            return Err(e);
                        }
                    }
                    
                    last_error = Some(e);
                }
            }
        }
        
        Err(last_error.unwrap_or(FederationClientError::MaxRetriesExceeded))
    }
    
    /// Broadcast an event to multiple peers
    ///
    /// Returns a map of server names to results.
    pub async fn broadcast_event(
        &self,
        peers: &[PeerInfo],
        event: &CisMatrixEvent,
    ) -> HashMap<String, FederationClientResult<EventReceiveResponse>> {
        let mut results = HashMap::new();
        
        for peer in peers {
            let server_name = peer.server_name.clone();
            let result = self.send_event_with_retry(peer, event).await;
            results.insert(server_name, result);
        }
        
        results
    }
    
    /// Broadcast an event to multiple peers in parallel
    ///
    /// This is more efficient than sequential broadcast.
    pub async fn broadcast_event_parallel(
        &self,
        peers: &[PeerInfo],
        event: &CisMatrixEvent,
    ) -> HashMap<String, FederationClientResult<EventReceiveResponse>> {
        use futures::future::join_all;
        
        let futures: Vec<_> = peers
            .iter()
            .map(|peer| {
                let client = self.clone();
                let peer = peer.clone();
                let event = event.clone();
                async move {
                    let result = client.send_event_with_retry(&peer, &event).await;
                    (peer.server_name, result)
                }
            })
            .collect();
        
        join_all(futures).await.into_iter().collect()
    }
    
    /// Fetch server key from a peer
    ///
    /// This implements the `/_matrix/key/v2/server` endpoint.
    pub async fn fetch_server_key(
        &self,
        peer: &PeerInfo,
    ) -> FederationClientResult<serde_json::Value> {
        let url = format!("{}/_matrix/key/v2/server", peer.base_url());
        
        debug!("Fetching server key from {}", peer.server_name);
        
        let response = self.client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(FederationClientError::HttpError)?;
        
        if response.status().is_success() {
            let key = response.json().await
                .map_err(|e| FederationClientError::InvalidResponse(e.to_string()))?;
            Ok(key)
        } else {
            Err(FederationClientError::Rejected(format!(
                "HTTP {}",
                response.status()
            )))
        }
    }
    
    /// Check if a peer is reachable
    pub async fn health_check(&self, peer: &PeerInfo) -> bool {
        let url = format!("{}/_matrix/key/v2/server", peer.base_url());
        
        match self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

impl Default for FederationClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default client")
    }
}

// Import HashMap for broadcast methods
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests would need a mock server to run properly
    // For now, we just test the client creation

    #[test]
    fn test_client_creation() {
        let client = FederationClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_builder() {
        let client = FederationClient::new()
            .unwrap()
            .with_timeout(Duration::from_secs(60))
            .with_max_retries(5);
        
        assert_eq!(client.timeout, Duration::from_secs(60));
        assert_eq!(client.max_retries, 5);
    }

    #[tokio::test]
    async fn test_health_check_unreachable() {
        // This should fail because there's no server
        let client = FederationClient::new().unwrap();
        let peer = PeerInfo::new("invalid.local", "127.0.0.1")
            .with_port(59999); // Unlikely to be used
        
        let healthy = client.health_check(&peer).await;
        assert!(!healthy);
    }
}
