//! P2P 网络模块
//!
//! 提供节点发现、连接管理和数据同步功能。

pub mod connection_manager;
pub mod crdt;
pub mod discovery;
pub mod gossip;
pub mod peer;
pub mod sync;
pub mod transport;
pub mod transport_secure;
pub mod dht;
pub mod kademlia;
pub mod nat;
pub mod mdns_service;
pub mod network;
pub mod offline_queue;  // P1-9: 离线队列

#[cfg(test)]
mod connection_manager_tests;

#[cfg(test)]
mod transport_secure_tests;

pub use connection_manager::{ConnectionManager, ConnectionHandle, ConnectionState};
pub use peer::Message;
pub use offline_queue::{OfflineQueue, OfflineQueueConfig, QueuedMessage, QueueStats};  // P1-9

pub mod crypto {
    //! P2P 加密模块
    pub mod keys;
    pub mod noise;
}

pub use crdt::{LWWRegister, GCounter, PNCounter, ORSet, VectorClock};
pub use discovery::{DiscoveryService, PeerDiscoveryInfo};
pub use gossip::GossipProtocol;
pub use peer::{PeerManager, PeerInfo};
pub use sync::{MemorySyncManager, SyncMemoryEntry, SyncRequest, SyncResponse};
pub use transport::{QuicTransport, Connection, ConnectionInfo};
pub use transport_secure::{
    SecureP2PTransport, SecureConnection, SecureConnectionInfo, SecureTransportConfig,
};
pub use dht::{DhtService, DhtConfig, DhtStats, RoutingTableEntry};
pub use kademlia::{KademliaDht, KademliaConfig, NodeId, NodeInfo, RoutingTable};
pub use nat::{NatTraversal, NatType, HolePunchCoordinator, HolePunchResult, TraversalMethod, TraversalResult, DEFAULT_STUN_SERVERS, DEFAULT_TURN_SERVERS};
pub use mdns_service::{MdnsService, DiscoveredNode};
pub use network::{P2PNetwork, P2PConfig, NetworkStatus};
