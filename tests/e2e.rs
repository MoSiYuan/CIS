//! # CIS End-to-End (E2E) Test Suite
//!
//! This is the main entry point for E2E tests. It includes all test modules
//! from the `e2e/` directory.
//!
//! ## Running Tests
//!
//! Run all E2E tests:
//! ```bash
//! cargo test --test e2e
//! ```
//!
//! Run with output:
//! ```bash
//! cargo test --test e2e -- --nocapture
//! ```
//!
//! Run specific test:
//! ```bash
//! cargo test test_cis_init_creates_directory_structure
//! ```

mod e2e;

// Re-export all test modules for running
pub use e2e::*;
