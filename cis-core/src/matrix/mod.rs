//! # CIS Matrix Integration Module
//!
//! Matrix protocol integration for CIS, enabling Element client connections
//! and inter-node federation.
//!
//! ## Port 分工
//!
//! - **6767**: 人机交互端口（对外暴露）
//!   - Matrix 客户端访问（Element 等）
//!   - 智能体 bearer 鉴权 API 访问
//!   
//! - **7676**: 节点间交互端口（集群内部）
//!   - 节点间 Matrix 同步
//!   - 跨节点 DAG 分发
//!   - Room 通信等
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  CIS Matrix Module                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │   Server     │  │   Nucleus    │  │  Federation  │       │
//! │  │  (6767)      │  │   (Core)     │  │  (7676/6768) │       │
//! │  └──────────────┘  └──────┬───────┘  └──────────────┘       │
//! │                           │                                 │
//! │         ┌─────────────────┼─────────────────┐               │
//! │         │                 │                 │               │
//! │    ┌────┴────┐      ┌────┴────┐      ┌────┴────┐           │
//! │    │  Store  │      │  Sync   │      │   WS    │           │
//! │    │         │      │  Queue  │      │ Tunnel  │           │
//! │    └─────────┘      └─────────┘      └─────────┘           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Components
//!
//! - **Server**: HTTP server handling Matrix Client-Server API (port 6767) - 人机交互
//! - **Federation**: Inter-node communication (port 7676) - BMI (Between Machine Interface)
//! - **WebSocket**: WebSocket federation (port 6768) - Low latency alternative to HTTP
//! - **Nucleus**: Core federation logic with sync queue and reconnection
//! - **Store**: Event storage and retrieval with SQLite
//! - **Sync**: Optimized sync queue with priority and batching
//! - **Error**: Matrix-specific error types
//!
//! ## Phase 0 Scope
//!
//! - Basic server running on port 6767
//! - GET /_matrix/client/versions (discovery)
//! - POST /_matrix/client/v3/login (simplified)
//!
//! ## Phase 2 Scope (Federation)
//!
//! - Simplified federation on port 7676
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
//! ## Phase 4 Scope (Complete Federation)
//!
//! - **FederationManager**: Centralized connection management with reconnection
//! - **SyncQueue**: Priority-based sync queue with batching
//! - **RoomStateSync**: Automatic room state synchronization
//! - **EventBroadcast**: Improved event federation broadcast
//! - **DeadLetterQueue**: Failed event handling

pub mod anchor;
pub mod bridge;
pub mod broadcast;
pub mod e2ee;
pub mod element_detect;
pub mod error;
pub mod events;
pub mod federation;
pub mod nucleus;
pub mod presence;
pub mod receipts;
pub mod server;
pub mod server_manager;
pub mod store;
pub mod store_social;
pub mod sync;
pub mod typing;

// Routes are internal, not exposed directly
mod routes;

// WebSocket federation module
pub mod websocket;

// Federation implementation (separate file to avoid mod.rs conflict)
mod federation_impl;

// Cloud Anchor module for NAT traversal and peer discovery
pub mod cloud;

// Re-export anchor types
pub use anchor::{CloudAnchor, PeerEndpoint};

// Re-export broadcast types
pub use broadcast::{EventBroadcaster, BroadcastResult};

// Re-export bridge types
pub use bridge::{MatrixBridge, SkillResult, SkillTask};

// Re-export error types
pub use error::{MatrixError, MatrixResult};

// Re-export nucleus types (core)
pub use nucleus::{
    HandlerId, MatrixEvent, MatrixNucleus, MatrixRoom, RoomId, RoomManager,
    RoomOptions as NucleusRoomOptions, RoomState, EventId, UserId,
};

// Re-export server types
pub use server::MatrixServer;

// Re-export store types
pub use store::{MatrixStore, RoomOptions, MatrixMessage, MatrixRoom as StoreMatrixRoom};
pub use store_social::{MatrixSocialStore, UserRecord, DeviceRecord, TokenInfo, UserProfile};

// Re-export sync types
pub use sync::{
    BatchOperation, SyncConsumer, SyncMetrics, SyncPriority, SyncQueue,
    SyncQueueConfig, SyncResult, SyncStatus, SyncTask,
};

// Re-export federation types from submodules
pub use federation::{
    client::{FederationClient, FederationClientError, FederationClientResult},
    discovery::PeerDiscovery,
    server::{FederationServer, FederationServerBuilder},
    types::{
        CisMatrixEvent, EventReceiveResponse, FederationConfig, FederationConfig as FedConfig,
        PeerInfo, RoomInfo, ServerKeyResponse, VerifyKey,
        FEDERATION_PORT, FEDERATION_API_VERSION,
    },
};

// Re-export FederationManager from federation_impl
pub use federation_impl::{
    ConnectionState, ConnectionStats,
    FederationConnection, FederationManager, FederationManagerConfig,
};

// Re-export websocket types
pub use websocket::{
    build_ws_url, AckMessage, AuthMessage, ConnectOptions, ErrorCode, ErrorMessage,
    EventMessage, HandshakeMessage, PingMessage, PongMessage, SyncRequest, SyncResponse,
    WebSocketClient, WebSocketClientBuilder, WebSocketServer, WebSocketServerBuilder,
    WsClientError, WsMessage, WsServerConfig, WsServerError,
    Tunnel, TunnelError, TunnelManager, TunnelState, TunnelStats,
    CONNECTION_TIMEOUT, DEFAULT_WS_PORT, HEARTBEAT_INTERVAL, PROTOCOL_VERSION, WS_PATH,
};

// Re-export noise protocol types
pub use websocket::{
    NoiseError, NoiseHandshake, NoiseTransport,
    noise::keys as noise_keys,
};

// Re-export Cloud Anchor types
pub use cloud::{
    CloudAnchorClient, CloudAnchorConfig, CloudAnchorError, CloudAnchorResult,
    DiscoveredPeer, HeartbeatRequest, HeartbeatResponse, HolePunchInfo, HolePunchRequest, 
    HolePunchResponse, NatType, NodeCapabilities, NodeRegistration, PunchCoordination, 
    RegistrationResponse, RelayMessage, QuotaInfo,
};
