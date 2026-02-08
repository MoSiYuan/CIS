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
//! - ✅ Optional mDNS discovery (placeholder)
//! - ✅ Optional mTLS support
//! - ✅ Connection pooling and retries
//! - ✅ Automatic reconnection with exponential backoff
//! - ✅ DID-based authentication
//! - ✅ Room state synchronization
//!
//! ## Example
//!
//! ```no_run
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

// Re-export submodules
pub use client::{FederationClient, FederationClientError, FederationClientResult};
pub use discovery::PeerDiscovery;
pub use server::{FederationServer, FederationServerBuilder};
pub use types::{
    CisMatrixEvent, DiscoveredNode, DiscoverySource, EventReceiveResponse, 
    FederationConfig, PeerInfo, ServerKeyResponse, VerifyKey, 
    FEDERATION_PORT, FEDERATION_API_VERSION,
};
