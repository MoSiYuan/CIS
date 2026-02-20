//! # Storage Module
//!
//! 跨平台存储管理，支持数据隔离和热插拔。
//!
//! ## Phase 3 Migration Note
//!
//! This module is kept for backward compatibility. The storage functionality has been migrated
//! to cis-common/cis-storage crate. New code should use:
//!
//! ```rust
//! use cis_storage::*;  // Recommended
//! ```
//!
//! This module re-exports from cis_storage for backward compatibility.

pub use cis_storage::*;
