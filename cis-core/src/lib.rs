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
//! - **Storage**: Cross-platform storage with data isolation and hot-swappable skills
//! - **Skill**: Hot-swappable skill management
//! - **Agent**: LLM Agent abstraction with bidirectional integration
//! - **Project**: Project-level configuration and local skills
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
//!
//! ## Bidirectional Agent Integration
//!
//! CIS serves as infrastructure with memory as data. The integration is bidirectional:
//!
//! ```
//! CIS → Agent: CIS calls external LLM Agent through AgentProvider
//! Agent → CIS: External Agent calls CIS through CLI/API
//! ```

pub mod error;
pub mod types;

// Core modules
pub mod sandbox;
pub mod scheduler;

// Memory module - Private/Public memory with encryption
pub mod memory;

// Storage module - Cross-platform storage with core/skill data isolation
pub mod storage;

// Skill module - Hot-swappable skill management
pub mod skill;

// AI module - AI Provider implementations
pub mod ai;

// Agent module - LLM Agent abstraction with bidirectional integration
pub mod agent;

// Project module - Project-level configuration
pub mod project;

// Wizard module - Initialization and onboarding
pub mod wizard;

// WASM module - WASM Runtime for sandboxed skills
#[cfg(feature = "wasm")]
pub mod wasm;

// Matrix protocol integration
pub mod matrix;

// Planned modules
// pub mod p2p;
// pub mod identity;

pub use error::{CisError, Result};
