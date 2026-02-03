//! # WebSocket Federation Layer
//!
//! WebSocket-based federation for CIS Matrix (BMI - Between Machine Interface).
//!
//! ## Overview
//!
//! This module provides a WebSocket-based alternative to HTTP federation,
//! offering:
//!
//! - **Low latency**: Persistent connections eliminate HTTP handshake overhead
//! - **Server push**: Real-time event forwarding without polling
//! - **Bidirectional**: Both nodes can initiate messages
//! - **Efficient**: Binary Protobuf support (planned)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    WebSocket Federation                      │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │   Server     │  │   Client     │  │   Tunnel     │       │
//! │  │  (server.rs) │  │  (client.rs) │  │  (tunnel.rs) │       │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
//! │         │                 │                 │               │
//! │         └─────────────────┴─────────────────┘               │
//! │                           │                                 │
//! │                    ┌──────┴──────┐                         │
//! │                    │   Protocol  │                         │
//! │                    │ (protocol.rs)│                         │
//! │                    └─────────────┘                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Port
//!
//! Default WebSocket federation port: **6768**
//!
//! ## Usage
//!
//! ### Server
//!
//! ```no_run
//! use cis_core::websocket::{WebSocketServer, WsServerConfig, TunnelManager};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = WsServerConfig::new("kitchen.local")
//!     .with_port(6768);
//!
//! let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);
//! let tunnel_manager = Arc::new(TunnelManager::with_event_channel(event_tx));
//!
//! let did_manager = Arc::new(cis_core::identity::DIDManager::generate("kitchen")?);
//! let mut server = WebSocketServer::new(
//!     config,
//!     tunnel_manager,
//!     store,
//!     "did:cis:kitchen",
//!     did_manager,
//! );
//!
//! server.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Client
//!
//! ```no_run
//! use cis_core::websocket::WebSocketClient;
//! use cis_core::matrix::federation::PeerInfo;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = WebSocketClient::new("my-node", "did:cis:my-node");
//!
//! let peer = PeerInfo::new("living.local", "living.local")
//!     .with_port(6768);
//!
//! let tunnel = client.connect(&peer, tunnel_manager).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Protocol
//!
//! Messages are JSON-encoded (Protobuf planned for Phase 2):
//!
//! 1. **Handshake**: Noise protocol key exchange
//! 2. **Auth**: DID-based authentication
//! 3. **Event**: Matrix event forwarding
//! 4. **Heartbeat**: Ping/pong every 5 seconds
//! 5. **Ack**: Message acknowledgments
//!
//! ## Migration from HTTP
//!
//! The WebSocket federation is designed to coexist with HTTP federation:
//! - HTTP remains for initial discovery and key exchange
//! - WebSocket is preferred for active communication
//! - Fallback to HTTP if WebSocket connection fails

pub mod client;
pub mod hole_punching;
pub mod noise;
pub mod protocol;
pub mod server;
pub mod tunnel;

// Re-export main types
pub use client::{
    ConnectOptions, WebSocketClient, WebSocketClientBuilder, WsClientError,
};
pub use hole_punching::{
    create_punch_socket, simultaneous_punch, HolePunchConfig, HolePunchManager,
    HolePunchState, InMemorySignalingClient, PunchResult, SignalingClient,
};
pub use crate::p2p::nat::HolePunchResult;
pub use noise::{
    keys as noise_keys, NoiseError, NoiseHandshake, NoiseTransport,
};
pub use protocol::{
    build_ws_url, AckMessage, AuthMessage, ErrorCode, ErrorMessage, EventMessage,
    HandshakeMessage, PingMessage, PongMessage, SyncRequest, SyncResponse, WsMessage,
    DEFAULT_WS_PORT, PROTOCOL_VERSION, WS_PATH,
};
pub use server::{
    WebSocketServer, WebSocketServerBuilder, WsServerConfig, WsServerError,
};
pub use tunnel::{
    Tunnel, TunnelError, TunnelManager, TunnelState, TunnelStats,
    CONNECTION_TIMEOUT, HEARTBEAT_INTERVAL,
};
