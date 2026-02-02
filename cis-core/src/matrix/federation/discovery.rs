//! # Peer Discovery
//!
//! Peer discovery mechanisms for CIS Matrix Federation.
//!
//! ## Discovery Methods
//!
//! 1. **Manual Configuration**: Static list of known peers
//! 2. **mDNS (Optional)**: Automatic discovery on local network
//!
//! ## Example
//!
//! ```no_run
//! use cis_core::matrix::federation::{PeerDiscovery, PeerInfo};
//!
//! # async fn example() {
//! // Manual discovery with known peers
//! let discovery = PeerDiscovery::new(vec![
//!     PeerInfo::new("kitchen.local", "kitchen.local"),
//!     PeerInfo::new("living.local", "living.local"),
//! ]);
//!
//! let peers = discovery.get_known_peers().await;
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};
use tracing::{debug, info};

use super::types::PeerInfo;

/// Peer discovery manager
#[derive(Debug, Clone)]
pub struct PeerDiscovery {
    /// Manually configured known peers
    known_peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    
    /// Whether mDNS discovery is enabled
    enable_mdns: bool,
    
    /// mDNS service name
    mdns_service_name: String,
    
    /// This server's name
    server_name: String,
}

impl PeerDiscovery {
    /// Create a new peer discovery manager
    ///
    /// # Arguments
    ///
    /// * `peers` - List of manually configured peers
    pub fn new(peers: Vec<PeerInfo>) -> Self {
        let mut peer_map = HashMap::new();
        for peer in peers {
            peer_map.insert(peer.server_name.clone(), peer);
        }
        
        Self {
            known_peers: Arc::new(Mutex::new(peer_map)),
            enable_mdns: false,
            mdns_service_name: "_cis._tcp".to_string(),
            server_name: "cis.local".to_string(),
        }
    }
    
    /// Create a new peer discovery manager with server name
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = server_name.into();
        self
    }
    
    /// Enable mDNS discovery
    ///
    /// Note: mDNS support is a placeholder for future implementation.
    /// Currently, this only sets a flag but doesn't actually start mDNS.
    pub fn with_mdns(mut self, enabled: bool) -> Self {
        self.enable_mdns = enabled;
        self
    }
    
    /// Get all known peers
    pub fn get_known_peers(&self) -> Vec<PeerInfo> {
        let peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.values().cloned().collect()
    }
    
    /// Get trusted peers only
    pub fn get_trusted_peers(&self) -> Vec<PeerInfo> {
        self.get_known_peers()
            .into_iter()
            .filter(|p| p.trusted)
            .collect()
    }
    
    /// Get a specific peer by server name
    pub fn get_peer(&self, server_name: &str) -> Option<PeerInfo> {
        let peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.get(server_name).cloned()
    }
    
    /// Add or update a peer
    pub fn add_peer(&self, peer: PeerInfo) {
        let mut peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.insert(peer.server_name.clone(), peer);
    }
    
    /// Remove a peer
    pub fn remove_peer(&self, server_name: &str) -> Option<PeerInfo> {
        let mut peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.remove(server_name)
    }
    
    /// Update peer last_seen timestamp
    pub fn mark_peer_seen(&self, server_name: &str) {
        let mut peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        
        if let Some(peer) = peers.get_mut(server_name) {
            peer.last_seen = Some(chrono::Utc::now().timestamp());
        }
    }
    
    /// Check if a peer exists
    pub fn has_peer(&self, server_name: &str) -> bool {
        let peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.contains_key(server_name)
    }
    
    /// Get peers for a room (simplified: returns all trusted peers)
    ///
    /// In a full implementation, this would look up room membership.
    pub fn get_peers_for_room(&self, _room_id: &str) -> Vec<PeerInfo> {
        self.get_trusted_peers()
    }
    
    /// Resolve a server name to peer info
    ///
    /// First checks known peers, then attempts DNS resolution if mDNS is enabled.
    pub async fn resolve_server(&self, server_name: &str) -> Option<PeerInfo> {
        // First check known peers
        if let Some(peer) = self.get_peer(server_name) {
            return Some(peer);
        }
        
        // If mDNS is enabled, try to discover
        if self.enable_mdns {
            // Placeholder for mDNS discovery
            // In a full implementation, this would query mDNS
            debug!("mDNS discovery not yet implemented for {}", server_name);
        }
        
        // Try to construct a default peer from server name
        // This assumes the server name is also a valid hostname
        Some(PeerInfo::new(server_name, server_name))
    }
    
    /// Start background discovery tasks
    ///
    /// This starts periodic tasks for:
    /// - mDNS discovery (if enabled)
    /// - Peer health checks
    pub async fn start_discovery(&self) {
        info!("Starting peer discovery for server: {}", self.server_name);
        
        let discovery = self.clone();
        
        // Spawn health check task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                discovery.check_peer_health().await;
            }
        });
        
        if self.enable_mdns {
            info!("mDNS discovery enabled (placeholder implementation)");
            // TODO: Implement actual mDNS discovery
            // This would use a library like `mdns` or `zeroconf`
        }
    }
    
    /// Check health of all known peers
    async fn check_peer_health(&self) {
        debug!("Checking peer health");
        
        let peers = self.get_known_peers();
        for peer in peers {
            // Simple connectivity check could be implemented here
            // For now, just log the peer status
            debug!(
                "Peer: {}, Last seen: {:?}, Trusted: {}",
                peer.server_name, peer.last_seen, peer.trusted
            );
        }
    }
    
    /// Get the number of known peers
    pub fn peer_count(&self) -> usize {
        let peers = self.known_peers.lock()
            .expect("Failed to lock known_peers");
        peers.len()
    }
    
    /// Create a peer discovery from a list of hostnames
    ///
    /// This is a convenience method for simple setups.
    pub fn from_hostnames(hostnames: Vec<String>) -> Self {
        let peers: Vec<PeerInfo> = hostnames
            .into_iter()
            .map(|host| PeerInfo::new(&host, &host))
            .collect();
        
        Self::new(peers)
    }
    
    /// Parse peers from a comma-separated string
    ///
    /// Format: "host1:port1,host2:port2" or just "host1,host2"
    pub fn parse_peers(peers_str: &str) -> Vec<PeerInfo> {
        peers_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                // Check if port is specified
                if let Some((host, port_str)) = s.split_once(':') {
                    if let Ok(port) = port_str.parse::<u16>() {
                        PeerInfo::new(host, host).with_port(port)
                    } else {
                        PeerInfo::new(s, s)
                    }
                } else {
                    PeerInfo::new(s, s)
                }
            })
            .collect()
    }
}

impl Default for PeerDiscovery {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_discovery_new() {
        let peers = vec![
            PeerInfo::new("kitchen.local", "kitchen.local"),
            PeerInfo::new("living.local", "living.local"),
        ];
        
        let discovery = PeerDiscovery::new(peers);
        
        assert_eq!(discovery.peer_count(), 2);
        assert!(discovery.has_peer("kitchen.local"));
        assert!(discovery.has_peer("living.local"));
    }

    #[test]
    fn test_add_and_remove_peer() {
        let discovery = PeerDiscovery::default();
        
        let peer = PeerInfo::new("test.local", "test.local");
        discovery.add_peer(peer.clone());
        
        assert_eq!(discovery.peer_count(), 1);
        assert!(discovery.has_peer("test.local"));
        
        let removed = discovery.remove_peer("test.local");
        assert!(removed.is_some());
        assert_eq!(discovery.peer_count(), 0);
    }

    #[test]
    fn test_trusted_peers() {
        let peers = vec![
            PeerInfo::new("trusted.local", "trusted.local").with_trusted(true),
            PeerInfo::new("untrusted.local", "untrusted.local").with_trusted(false),
        ];
        
        let discovery = PeerDiscovery::new(peers);
        let trusted = discovery.get_trusted_peers();
        
        assert_eq!(trusted.len(), 1);
        assert_eq!(trusted[0].server_name, "trusted.local");
    }

    #[test]
    fn test_parse_peers() {
        let peers = PeerDiscovery::parse_peers("host1,host2:6768,host3");
        
        assert_eq!(peers.len(), 3);
        assert_eq!(peers[0].host, "host1");
        assert_eq!(peers[0].port, FEDERATION_PORT);
        assert_eq!(peers[1].host, "host2");
        assert_eq!(peers[1].port, 6768);
    }

    #[test]
    fn test_from_hostnames() {
        let hostnames = vec![
            "kitchen.local".to_string(),
            "living.local".to_string(),
        ];
        
        let discovery = PeerDiscovery::from_hostnames(hostnames);
        
        assert_eq!(discovery.peer_count(), 2);
    }
}
