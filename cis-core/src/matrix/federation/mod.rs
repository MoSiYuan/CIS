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
//! Default federation port: **6767**
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐      HTTP/6767      ┌─────────────────┐
//! │   CIS Node A    │ ◄──────────────────► │   CIS Node B    │
//! │  (kitchen.local)│   CisMatrixEvent     │  (living.local) │
//! └─────────────────┘                      └─────────────────┘
//!         │                                        │
//!         │    Endpoints:                          │
//!         │    - GET /_matrix/key/v2/server        │
//!         │    - POST /_cis/v1/event/receive       │
//!         │                                        │
//! ```
//!
//! ## Components
//!
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
//!
//! ## Example
//!
//! ```no_run
//! use cis_core::matrix::federation::{
//!     FederationServer, FederationConfig, PeerDiscovery, PeerInfo, CisMatrixEvent
//! };
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Configure this server
//! let config = FederationConfig::new("kitchen.local")
//!     .with_port(6767);
//!
//! // Configure known peers
//! let discovery = PeerDiscovery::new(vec![
//!     PeerInfo::new("living.local", "living.local")
//!         .with_trusted(true),
//! ]);
//!
//! // Create or open store
//! let store = Arc::new(cis_core::matrix::MatrixStore::open_in_memory()?);
//!
//! // Create and run server
//! let server = FederationServer::new(config, discovery, store);
//!
//! // In a real application, run in a separate task
//! // server.run().await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod discovery;
pub mod server;
pub mod types;

// Re-export main types
pub use client::{FederationClient, FederationClientError, FederationClientResult};
pub use discovery::PeerDiscovery;
pub use server::{FederationServer, FederationServerBuilder};
pub use types::{
    CisMatrixEvent, EventReceiveResponse, FederationConfig, PeerInfo,
    ServerKeyResponse, VerifyKey, FEDERATION_PORT, FEDERATION_API_VERSION,
};
