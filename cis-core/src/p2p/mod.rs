//! P2P 网络模块
//!
//! 提供节点发现、连接管理和数据同步功能。
//!
//! ## Phase 3 Migration Note
//!
//! This module is kept for backward compatibility. The P2P functionality has been migrated
//! to cis-common/cis-p2p crate. New code should use:
//!
//! ```rust
//! use cis_p2p::*;  // Recommended
//! ```
//!
//! This module re-exports from cis_p2p for backward compatibility.

pub use cis_p2p::*;
