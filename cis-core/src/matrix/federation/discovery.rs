//! # Peer Discovery
//!
//! Peer discovery mechanisms for CIS Matrix Federation.
//!
//! ## Discovery Methods
//!
//! 1. **Manual Configuration**: Static list of known peers
//! 2. **mDNS**: Automatic discovery on local network
//!
//! ## Example
//!
//! ```rust
//! use cis_core::matrix::federation::{PeerDiscovery, PeerInfo};
//!
//! // Manual discovery with known peers
//! let discovery = PeerDiscovery::new(vec![
//!     PeerInfo::new("kitchen.local", "kitchen.local"),
//!     PeerInfo::new("living.local", "living.local"),
//! ]);
//!
//! let peers = discovery.get_known_peers();
//! ```
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use super::types::{DiscoveredNode, DiscoverySource, PeerInfo};

/// mDNS service type for CIS Matrix federation
const MDNS_SERVICE_TYPE: &str = "_cis-matrix._tcp.local";

/// Default mDNS broadcast interval in seconds
const MDNS_BROADCAST_INTERVAL_SECS: u64 = 60;

/// Peer discovery manager
#[derive(Debug, Clone)]
pub struct PeerDiscovery {
    /// Manually configured known peers
    known_peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    
    /// Discovered nodes via mDNS
    discovered_nodes: Arc<Mutex<HashMap<String, DiscoveredNode>>>,
    
    /// Whether mDNS discovery is enabled
    enable_mdns: bool,
    
    /// mDNS service name
    #[allow(dead_code)]
    mdns_service_name: String,
    
    /// This server's name
    server_name: String,
    
    /// This node's ID
    node_id: String,
    
    /// This node's DID
    did: String,
    
    /// Federation port
    port: u16,
    
    /// Node version
    version: String,
    
    /// Node capabilities
    capabilities: Vec<String>,
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
            discovered_nodes: Arc::new(Mutex::new(HashMap::new())),
            enable_mdns: false,
            mdns_service_name: MDNS_SERVICE_TYPE.to_string(),
            server_name: "cis.local".to_string(),
            node_id: uuid::Uuid::new_v4().to_string(),
            did: String::new(),
            port: super::FEDERATION_PORT,
            version: env!("CARGO_PKG_VERSION").to_string(),
            capabilities: vec!["federation".to_string(), "matrix".to_string()],
        }
    }
    
    /// Create a new peer discovery manager with server name
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = server_name.into();
        self
    }
    
    /// Set node ID
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = node_id.into();
        self
    }
    
    /// Set DID
    pub fn with_did(mut self, did: impl Into<String>) -> Self {
        self.did = did.into();
        self
    }
    
    /// Set port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    /// Set version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
    
    /// Set capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }
    
    /// Enable mDNS discovery
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
        
        // Check discovered nodes
        {
            let discovered = self.discovered_nodes.lock()
                .expect("Failed to lock discovered_nodes");
            for node in discovered.values() {
                if node.server_name.as_deref() == Some(server_name) {
                    let peer = PeerInfo::new(server_name, node.address.ip().to_string())
                        .with_port(node.address.port())
                        .with_trusted(false);
                    return Some(peer);
                }
            }
        }
        
        // If mDNS is enabled, try to discover
        #[cfg(feature = "p2p")]
        if self.enable_mdns {
            debug!("mDNS discovery enabled, trying to discover {}", server_name);
            if let Ok(nodes) = self.discover_mdns_matrix().await {
                for node in nodes {
                    if node.server_name.as_deref() == Some(server_name) {
                        let peer = PeerInfo::new(server_name, node.address.ip().to_string())
                            .with_port(node.address.port())
                            .with_trusted(false);
                        return Some(peer);
                    }
                }
            }
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
        
        #[cfg(feature = "p2p")]
        if self.enable_mdns {
            info!("mDNS discovery enabled, starting mDNS services");
            
            // Start mDNS broadcaster
            let broadcaster = self.clone();
            tokio::spawn(async move {
                if let Err(e) = broadcaster.start_mdns_broadcast().await {
                    error!("mDNS broadcast failed: {}", e);
                }
            });
            
            // Start mDNS listener
            let listener = self.clone();
            tokio::spawn(async move {
                if let Err(e) = listener.start_mdns_listener().await {
                    error!("mDNS listener failed: {}", e);
                }
            });
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
    
    /// Get discovered nodes via mDNS
    pub fn get_discovered_nodes(&self) -> Vec<DiscoveredNode> {
        let nodes = self.discovered_nodes.lock()
            .expect("Failed to lock discovered_nodes");
        nodes.values().cloned().collect()
    }
    
    /// Get discovered nodes filtered by source
    pub fn get_discovered_nodes_by_source(&self, source: DiscoverySource) -> Vec<DiscoveredNode> {
        self.get_discovered_nodes()
            .into_iter()
            .filter(|n| n.source == source)
            .collect()
    }
    
    /// Discover Matrix nodes via mDNS
    ///
    /// This performs an active mDNS browse operation and returns discovered nodes.
    #[cfg(feature = "p2p")]
    pub async fn discover_mdns_matrix(&self) -> anyhow::Result<Vec<DiscoveredNode>> {
        if !self.enable_mdns {
            return Ok(vec![]);
        }
        
        debug!("Starting mDNS discovery for service: {}", MDNS_SERVICE_TYPE);
        
        // Create mDNS daemon
        let mdns = mdns_sd::ServiceDaemon::new()
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS daemon: {}", e))?;
        
        // Browse for services
        let receiver = mdns.browse(MDNS_SERVICE_TYPE)
            .map_err(|e| anyhow::anyhow!("Failed to browse mDNS: {}", e))?;
        
        let mut discovered = Vec::new();
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();
        
        // Collect discovered services with timeout
        while let Ok(event) = receiver.recv_timeout(timeout - start.elapsed().min(timeout)) {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(info) => {
                    if let Some(node) = Self::parse_mdns_service_info(&info, &self.node_id) {
                        debug!("Discovered node via mDNS: {} at {}", node.node_id, node.address);
                        discovered.push(node);
                    }
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                    debug!("Service removed: {}", fullname);
                }
                _ => {}
            }
            
            if start.elapsed() >= timeout {
                break;
            }
        }
        
        // Shutdown mDNS daemon
        if let Err(e) = mdns.shutdown() {
            warn!("Failed to shutdown mDNS daemon: {}", e);
        }
        
        info!("mDNS discovery completed, found {} nodes", discovered.len());
        Ok(discovered)
    }
    
    /// Start mDNS broadcast service
    ///
    /// This registers the local node as a CIS Matrix service and keeps the registration alive.
    #[cfg(feature = "p2p")]
    async fn start_mdns_broadcast(&self) -> anyhow::Result<()> {
        info!("Starting mDNS broadcast for service: {}", MDNS_SERVICE_TYPE);
        
        let node_id = self.node_id.clone();
        let did = self.did.clone();
        let server_name = self.server_name.clone();
        let port = self.port;
        let version = self.version.clone();
        let capabilities = self.capabilities.clone();
        
        // Create mDNS daemon
        let mdns = mdns_sd::ServiceDaemon::new()
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS daemon: {}", e))?;
        
        // Build service name
        let instance_name = format!("cis-matrix-{}", node_id);
        let host_name = format!("{}.local", server_name);
        
        // Build TXT properties
        let mut properties = HashMap::new();
        properties.insert("node_id".to_string(), node_id.clone());
        properties.insert("did".to_string(), did.clone());
        properties.insert("version".to_string(), version);
        properties.insert("capabilities".to_string(), capabilities.join(","));
        
        // Get local IP addresses
        let ip_addrs = Self::get_local_ip_addresses().await;
        if ip_addrs.is_empty() {
            return Err(anyhow::anyhow!("No local IP addresses found"));
        }
        
        debug!("Registering mDNS service with IPs: {:?}", ip_addrs);
        
        // Create service info
        let service_info = mdns_sd::ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance_name,
            &host_name,
            &ip_addrs[..],
            port,
            properties,
        ).map_err(|e| anyhow::anyhow!("Failed to create service info: {}", e))?;
        
        // Register service
        mdns.register(service_info)
            .map_err(|e| anyhow::anyhow!("Failed to register mDNS service: {}", e))?;
        
        info!("mDNS service registered: {} on port {}", instance_name, port);
        
        // Keep the service alive with periodic TTL updates
        let mut interval = interval(Duration::from_secs(MDNS_BROADCAST_INTERVAL_SECS));
        
        loop {
            interval.tick().await;
            debug!("mDNS broadcast heartbeat for {}", instance_name);
        }
    }
    
    /// Start mDNS listener for discovering other nodes
    ///
    /// This continuously listens for mDNS service announcements.
    #[cfg(feature = "p2p")]
    async fn start_mdns_listener(&self) -> anyhow::Result<()> {
        info!("Starting mDNS listener for service: {}", MDNS_SERVICE_TYPE);
        
        let discovered_nodes = Arc::clone(&self.discovered_nodes);
        let local_node_id = self.node_id.clone();
        
        // Create mDNS daemon
        let mdns = mdns_sd::ServiceDaemon::new()
            .map_err(|e| anyhow::anyhow!("Failed to create mDNS daemon: {}", e))?;
        
        // Start browsing
        let receiver = mdns.browse(MDNS_SERVICE_TYPE)
            .map_err(|e| anyhow::anyhow!("Failed to browse mDNS: {}", e))?;
        
        // Process incoming events
        while let Ok(event) = receiver.recv() {
            match event {
                mdns_sd::ServiceEvent::ServiceResolved(info) => {
                    if let Some(node) = Self::parse_mdns_service_info(&info, &local_node_id) {
                        // Skip self
                        if node.node_id == local_node_id {
                            continue;
                        }
                        
                        info!("Discovered peer via mDNS: {} at {} (DID: {})", 
                            node.node_id, node.address, node.did);
                        
                        // Update discovered nodes
                        let mut nodes = discovered_nodes.lock()
                            .expect("Failed to lock discovered_nodes");
                        nodes.insert(node.node_id.clone(), node);
                    }
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                    debug!("Service removed: {}", fullname);
                    
                    // Remove from discovered nodes
                    let mut nodes = discovered_nodes.lock()
                        .expect("Failed to lock discovered_nodes");
                    nodes.retain(|_, node| !fullname.contains(&node.node_id));
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// Parse mDNS service info into DiscoveredNode
    #[cfg(feature = "p2p")]
    fn parse_mdns_service_info(info: &mdns_sd::ServiceInfo, local_node_id: &str) -> Option<DiscoveredNode> {
        let properties = info.get_properties();
        
        // Extract required properties
        let node_id = properties.get("node_id")?.to_string();
        
        // Skip if this is ourselves
        if node_id == local_node_id {
            return None;
        }
        
        let did = properties.get("did")?.to_string();
        let version = properties.get("version").map(|v| v.to_string());
        let capabilities = properties.get("capabilities")
            .map(|c| c.to_string().split(',').map(|s| s.to_string()).collect());
        
        // Get addresses and port
        let addresses = info.get_addresses();
        let port = info.get_port();
        
        if addresses.is_empty() {
            warn!("No addresses found for mDNS service: {}", info.get_fullname());
            return None;
        }
        
        // Use the first address
        let ip = addresses.iter().next()?;
        let address = SocketAddr::new(*ip, port);
        
        // Build server name from instance name
        let instance_name = info.get_fullname();
        let server_name = if instance_name.starts_with("cis-matrix-") {
            Some(format!("{}.local", &instance_name[11..instance_name.find('.').unwrap_or(instance_name.len())]))
        } else {
            None
        };
        
        Some(DiscoveredNode {
            node_id,
            did,
            address,
            source: DiscoverySource::Mdns,
            server_name,
            version,
            capabilities,
            last_seen: chrono::Utc::now().timestamp(),
        })
    }
    
    /// Get local IP addresses
    async fn get_local_ip_addresses() -> Vec<std::net::IpAddr> {
        let mut addresses = Vec::new();
        
        match tokio::net::lookup_host("localhost:0").await {
            Ok(addrs) => {
                for addr in addrs {
                    let ip = addr.ip();
                    if !ip.is_loopback() && !addresses.contains(&ip) {
                        addresses.push(ip);
                    }
                }
            }
            Err(e) => {
                debug!("Failed to lookup localhost: {}", e);
            }
        }
        
        // If no addresses found, try to get interface addresses
        if addresses.is_empty() {
            // Fallback: use common local addresses
            addresses.push(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
        }
        
        addresses
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
    use crate::matrix::federation::FEDERATION_PORT;

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
    
    #[test]
    fn test_discovered_node_creation() {
        let address = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)), 6767);
        let node = DiscoveredNode::new(
            "test-node-123",
            "did:cis:test-node-123",
            address,
            DiscoverySource::Mdns,
        )
        .with_server_name("test.local")
        .with_version("1.0.0")
        .with_capabilities(vec!["federation".to_string(), "matrix".to_string()]);
        
        assert_eq!(node.node_id, "test-node-123");
        assert_eq!(node.did, "did:cis:test-node-123");
        assert_eq!(node.address, address);
        assert_eq!(node.source, DiscoverySource::Mdns);
        assert_eq!(node.server_name, Some("test.local".to_string()));
        assert_eq!(node.version, Some("1.0.0".to_string()));
        assert_eq!(node.capabilities, Some(vec!["federation".to_string(), "matrix".to_string()]));
    }
    
    #[test]
    fn test_discovery_source_variants() {
        assert_eq!(DiscoverySource::Mdns, DiscoverySource::Mdns);
        assert_ne!(DiscoverySource::Mdns, DiscoverySource::Manual);
        assert_ne!(DiscoverySource::Dht, DiscoverySource::Seed);
    }
    
    #[test]
    fn test_peer_discovery_with_builder_methods() {
        let discovery = PeerDiscovery::new(vec![])
            .with_server_name("test.local")
            .with_node_id("node-123")
            .with_did("did:cis:node-123")
            .with_port(6768)
            .with_version("1.0.0")
            .with_capabilities(vec!["federation".to_string()])
            .with_mdns(true);
        
        assert!(discovery.enable_mdns);
        assert_eq!(discovery.port, 6768);
        assert_eq!(discovery.node_id, "node-123");
        assert_eq!(discovery.did, "did:cis:node-123");
    }
}
