//! # CIS End-to-End (E2E) Test Suite
//!
//! This module contains end-to-end tests for the CIS CLI.
//! These tests execute the actual binary and verify behavior through
//! command-line interface interactions.
//!
//! ## Test Organization
//!
//! - `init_tests.rs` - Tests for `cis init` command
//! - `skill_tests.rs` - Tests for `cis skill` command
//! - `dag_tests.rs` - Tests for `cis dag` command
//! - `node_tests.rs` - Tests for `cis node` command
//! - `helpers.rs` - Test utilities and helper functions

pub mod dag_tests;
pub mod helpers;
pub mod init_tests;
pub mod node_tests;
pub mod skill_tests;

/// E2E test configuration constants
pub mod config {
    /// Default timeout for CLI commands (in seconds)
    pub const DEFAULT_TIMEOUT: u64 = 30;

    /// Long timeout for complex operations (in seconds)
    pub const LONG_TIMEOUT: u64 = 120;

    /// Test data directory prefix
    pub const TEST_DATA_PREFIX: &str = "cis_e2e_test_";
}
