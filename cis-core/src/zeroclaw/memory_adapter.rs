//! ZeroClaw Memory Adapter
//!
//! Implements the zeroclaw::Memory trait for CIS memory integration.

use async_trait::async_trait;

/// ZeroClaw-compatible memory adapter for CIS
pub struct CisMemoryAdapter {
    // Internal state
}

impl CisMemoryAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for CisMemoryAdapter {
    fn default() -> Self {
        Self::new()
    }
}
