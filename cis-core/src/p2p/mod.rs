//! P2P 网络模块
//!
//! 提供节点发现、连接管理和数据同步功能。

pub mod crdt;
pub mod discovery;
pub mod gossip;
pub mod peer;
pub mod sync;
pub mod transport;
pub mod dht;
pub mod nat;
pub mod mdns_service;
pub mod network;

pub use crdt::{LWWRegister, GCounter, PNCounter, ORSet, VectorClock};
pub use discovery::{DiscoveryService, PeerDiscoveryInfo};
pub use gossip::GossipProtocol;
pub use peer::{PeerManager, PeerInfo};
pub use sync::{MemorySyncManager, SyncMemoryEntry, SyncRequest, SyncResponse};
pub use transport::{QuicTransport, Connection, ConnectionInfo};
pub use dht::{DhtService, DhtConfig, DhtStats, RoutingTableEntry};
pub use nat::{NatTraversal, NatType, HolePunchCoordinator, HolePunchResult, TraversalMethod, TraversalResult, DEFAULT_STUN_SERVERS, DEFAULT_TURN_SERVERS};
pub use mdns_service::{MdnsService, DiscoveredNode};
pub use network::{P2PNetwork, P2PConfig, NetworkStatus};
