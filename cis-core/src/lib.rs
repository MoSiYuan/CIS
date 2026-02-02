//! # CIS Core Library
//!
//! Core library for CIS (Cluster of Independent Systems).
//!
//! This library provides the foundational components for building
//! hardware-bound, sovereign distributed computing systems.
//!
//! ## Architecture
//!
//! - **Types**: Core data structures and domain types
//! - **Sandbox**: Security and path validation
//! - **Scheduler**: DAG-based task scheduling
//! - **Memory**: Local sovereign memory storage (planned)
//! - **Executor**: Task execution engine (planned)
//! - **P2P**: Peer-to-peer communication (planned)
//! - **Identity**: Hardware-bound DID management (planned)
//!
//! ## Philosophy
//!
//! CIS follows first principles and Occam's razor:
//! - Hardware-binding for identity
//! - Local memory sovereignty
//! - Zero-token inter-node communication
//! - Pure P2P architecture (no central coordinator)

pub mod error;
pub mod types;

// Core modules (Phase 1 - Direct Inherit)
pub mod sandbox;
pub mod scheduler;

// Planned modules
// pub mod memory;     // Phase 2
// pub mod executor;   // Phase 2
// pub mod p2p;        // Phase 4
// pub mod identity;   // Phase 4
// pub mod sync;       // Phase 4

pub use error::{CisError, Result};
