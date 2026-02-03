//! # Cloud Anchor 云端服务客户端
//!
//! 提供 NAT 穿透协助和节点发现功能，当 mDNS 和直接连接都失败时使用。
//!
//! ## 功能
//!
//! - **节点注册**: 节点上线时向 Cloud Anchor 注册，定期心跳保活
//! - **节点发现**: 查询特定节点或获取推荐的可连接节点列表
//! - **NAT 穿透协助**: 作为信令服务器协调 hole punching
//! - **消息中继**: 当直接连接失败时中继消息 (TURN-like)
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   Cloud Anchor Client                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │  Registration │  │  Discovery   │  │   Relay      │       │
//! │  │              │  │              │  │  (TURN-like) │       │
//! │  └──────────────┘  └──────────────┘  └──────────────┘       │
//! │                                                              │
//! │  ┌──────────────┐  ┌──────────────┐                          │
//! │  │  Heartbeat   │  │Hole Punching │                          │
//! │  │   Manager    │  │ Coordination │                          │
//! │  └──────────────┘  └──────────────┘                          │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod types;

// Re-export main types
pub use client::CloudAnchorClient;
pub use config::CloudAnchorConfig;
pub use error::{CloudAnchorError, CloudAnchorResult};
pub use types::{
    DiscoveredPeer, HeartbeatRequest, HeartbeatResponse, HolePunchInfo, HolePunchRequest, 
    HolePunchResponse, NatType, NodeCapabilities, NodeRegistration, PunchCoordination, 
    RegistrationResponse, RelayMessage, QuotaInfo,
};
