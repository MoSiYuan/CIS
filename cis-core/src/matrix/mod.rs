//! # CIS Matrix Integration Module
//!
//! Matrix protocol integration for CIS, enabling Element client connections
//! and inter-node federation.
//!
//! ## Architecture
//!
//! - **Server**: HTTP server handling Matrix Client-Server API (port 7676)
//! - **Federation**: Inter-node communication (port 6767) - BMI (Between Machine Interface)
//! - **WebSocket**: WebSocket federation (port 6768) - Low latency alternative to HTTP
//! - **Routes**: API endpoints (discovery, login, sync, etc.)
//! - **Store**: Event storage and retrieval
//! - **Error**: Matrix-specific error types
//!
//! ## Phase 0 Scope
//!
//! - Basic server running on port 7676
//! - GET /_matrix/client/versions (discovery)
//! - POST /_matrix/client/v3/login (simplified)
//!
//! ## Phase 2 Scope (Federation)
//!
//! - Simplified federation on port 6767
//! - GET /_matrix/key/v2/server (Matrix spec)
//! - POST /_cis/v1/event/receive (CIS custom)
//! - Manual peer configuration
//! - Event forwarding
//!
//! ## Phase 3 Scope (WebSocket Federation)
//!
//! - WebSocket federation on port 6768
//! - Persistent connections with heartbeat
//! - Real-time event push
//! - Noise protocol handshake
//! - DID authentication
//!
//! ## Future Phases
//!
//! - Full sync API
//! - Room management
//! - Full Matrix Federation

pub mod bridge;
pub mod error;
pub mod federation;
pub mod server;
pub mod store;

// Routes are internal, not exposed directly
mod routes;

// WebSocket federation module (optional feature)
#[cfg(feature = "websocket")]
pub mod websocket;

pub use bridge::{MatrixBridge, SkillResult, SkillTask};
pub use error::{MatrixError, MatrixResult};
pub use server::MatrixServer;
pub use store::MatrixStore;
