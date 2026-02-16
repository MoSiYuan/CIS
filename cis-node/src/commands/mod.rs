//! # CLI Commands
//!
//! Command implementations for the CIS CLI.

pub mod agent;
pub mod config_cmd;
pub mod dag;
pub mod debt;
pub mod decision;
pub mod doctor;
pub mod glm;
pub mod im;
pub mod init;
pub mod matrix;
pub mod memory;
pub mod memory_conflicts;  // 🔥 Memory Conflicts CLI (P1.7.0)
pub mod neighbor;
pub mod network;
pub mod pair;
pub mod node;
pub mod peer;
pub mod project;
#[cfg(feature = "p2p")]
pub mod p2p;
pub mod schema;
pub mod session;
pub mod skill;
pub mod system;
pub mod task;
pub mod task_level;
pub mod telemetry;
pub mod update;
pub mod unified;
pub mod worker;

