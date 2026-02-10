//! # CIS Matrix Federation (BMI)
//!
//! Between Machine Interface (BMI) for CIS node-to-node communication.
//!
//! ## Overview
//!
//! This module implements a simplified federation protocol (Scheme B) for CIS nodes
//! to communicate with each other. It does not implement the full Matrix Federation
//! protocol but uses a simplified HTTP API with Matrix event format.
//!
//! ## Port
//!
//! 节点间交互端口（集群内部）: **7676**
//! 用于节点间 Matrix 同步、DAG 分发、room 通信等
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  Federation Module                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │ Federation   │  │   HTTP       │  │   WebSocket  │       │
//! │  │   Manager    │  │   Client     │  │   Client     │       │
//! │  │              │  │              │  │              │       │
//! │  └──────┬───────┘  └──────────────┘  └──────────────┘       │
//! │         │                                                   │
//! │    ┌────┴────┐                                              │
//! │    │  Peer   │                                              │
//! │    │Discovery│                                              │
//! │    └─────────┘                                              │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Components
//!
//! - **FederationManager**: Centralized connection management with reconnection logic
//! - **FederationServer**: HTTP server handling incoming federation requests
//! - **FederationClient**: HTTP client for sending events to other nodes
//! - **PeerDiscovery**: Manages known peers and discovery
//! - **Types**: Event formats and configuration
//!
//! ## Features
//!
//! - ✅ Simplified HTTP API (not full Matrix Federation)
//! - ✅ Matrix-compatible event format
//! - ✅ Manual peer configuration
//! - ✅ Event forwarding to multiple peers
//! - ✅ Optional mDNS discovery via `MatrixDiscovery`
//! - ✅ Optional mTLS support
//! - ✅ Connection pooling and retries
//! - ✅ Automatic reconnection with exponential backoff
//! - ✅ DID-based authentication
//! - ✅ Room state synchronization
//!
//! ## Example
//!
//! ```ignore
//! use cis_core::matrix::federation::{
//!     FederationManager, FederationManagerConfig, PeerDiscovery, PeerInfo
//! };
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Configure this server
//! let config = FederationManagerConfig {
//!     use_websocket: true,
//!     auto_reconnect: true,
//!     reconnect_base_delay: 2,
//!     max_reconnect_attempts: 10,
//!     connection_timeout: 30,
//!     heartbeat_interval: 5,
//!     verify_dids: true,
//! };
//!
//! // Configure known peers
//! let discovery = PeerDiscovery::new(vec![
//!     PeerInfo::new("living.local", "living.local")
//!         .with_trusted(true),
//! ]);
//!
//! // Create or open store
//! let store = Arc::new(cis_core::matrix::MatrixStore::open_in_memory()?);
//! let did = Arc::new(cis_core::identity::DIDManager::generate("kitchen")?);
//!
//! // Create event channel
//! let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
//!
//! // Create and start federation manager
//! let federation = FederationManager::with_config(
//!     did,
//!     discovery,
//!     store,
//!     event_tx,
//!     config,
//! )?;
//!
//! federation.start().await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod discovery;
pub mod server;
pub mod types;

#[cfg(feature = "p2p")]
use crate::p2p::{MdnsService, DiscoveredNode as MdnsDiscoveredNode};
#[cfg(feature = "p2p")]
use std::time::Duration;

/// Matrix 局域网发现服务
///
/// 使用 mDNS 发现本地网络中的 Matrix homeserver。
/// 服务类型: `_matrix._tcp.local`
#[cfg(feature = "p2p")]
pub struct MatrixDiscovery {
    mdns: MdnsService,
}

#[cfg(feature = "p2p")]
impl MatrixDiscovery {
    /// 创建新的 Matrix 发现服务
    ///
    /// # Arguments
    /// * `node_id` - 本节点唯一标识
    /// * `port` - 服务端口
    /// * `did` - 去中心化身份标识
    pub fn new(
        node_id: &str,
        port: u16,
        did: &str,
    ) -> anyhow::Result<Self> {
        let metadata = std::collections::HashMap::new();
        let mdns = MdnsService::new(node_id, port, did, metadata)?;
        Ok(Self { mdns })
    }

    /// 发现本地网络中的 Matrix homeserver
    ///
    /// 搜索 `_matrix._tcp.local` 服务类型的节点。
    /// 默认超时时间为 10 秒。
    ///
    /// # Returns
    /// 发现的节点列表
    pub fn discover_local_homeservers(&self) -> anyhow::Result<Vec<MdnsDiscoveredNode>> {
        let service_type = "_matrix._tcp.local";
        let timeout = Duration::from_secs(10);
        self.mdns.discover_with_type(service_type, timeout)
    }

    /// 使用自定义超时发现本地 homeserver
    ///
    /// # Arguments
    /// * `timeout` - 发现超时时间
    pub fn discover_with_timeout(
        &self,
        timeout: Duration,
    ) -> anyhow::Result<Vec<MdnsDiscoveredNode>> {
        let service_type = "_matrix._tcp.local";
        self.mdns.discover_with_type(service_type, timeout)
    }

    /// 停止发现服务
    pub fn shutdown(self) -> anyhow::Result<()> {
        self.mdns.shutdown()
    }
}

// Re-export submodules
pub use client::{FederationClient, FederationClientError, FederationClientResult};
pub use discovery::PeerDiscovery;
pub use server::{FederationServer, FederationServerBuilder};
pub use types::{
    CisMatrixEvent, DiscoveredNode, DiscoverySource, EventReceiveResponse, 
    FederationConfig, PeerInfo, ServerKeyResponse, VerifyKey, 
    FEDERATION_PORT, FEDERATION_API_VERSION,
};
