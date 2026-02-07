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
//! ```text
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

// Intent module - Natural language intent parsing
pub mod intent;

// AI module - AI Provider implementations
pub mod ai;

// Agent module - LLM Agent abstraction with bidirectional integration
pub mod agent;

// Project module - Project-level configuration
pub mod project;

// Wizard module - Initialization and onboarding
pub mod wizard;

// Init module - New initialization wizard with full flow
pub mod init;

// WASM module - WASM Runtime for sandboxed skills
#[cfg(feature = "wasm")]
pub mod wasm;

// Matrix protocol integration
pub mod matrix;

// Identity module - DID management
pub mod identity;

// Vector Intelligence module - Semantic search
pub mod vector;

// Conversation module - Session dialogue management
pub mod conversation;

// Telemetry module - Request logging and observability
pub mod telemetry;

// Task module - Task management and vector indexing
pub mod task;

// P2P module - Peer-to-peer networking
pub mod p2p;

// Network module - Access control and DID-based admission
pub mod network;

// GLM module - Cloud node API for GLM integration
pub mod glm;

// Service layer - Unified data access for CLI, GUI, API
pub mod service;

// CLI module - AI-Native CLI framework
pub mod cli;

pub use error::{CisError, Result};
pub use identity::DIDManager;
