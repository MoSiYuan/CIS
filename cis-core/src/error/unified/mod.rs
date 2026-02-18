//! # CIS Unified Error Handling System
//!
//! This module provides a comprehensive, type-safe error handling framework for CIS.
//!
//! ## Design Principles
//!
//! 1. **Type Safety**: Each error variant carries specific context, not just strings
//! 2. **Error Codes**: Machine-readable error codes for programmatic handling
//! 3. **Rich Context**: Errors include causes, suggestions, and source locations
//! 4. **Backward Compatibility**: Existing error types can be converted to unified errors
//! 5. **Recoverability**: Errors indicate whether they are retryable or fatal
//!
//! ## Error Code Format
//!
//! ```text
//! CIS-{CATEGORY}-{SPECIFIC}
//!
//! Examples:
//!   CIS-MEM-001: Memory not found
//!   CIS-NET-015: Connection timeout
//!   CIS-SCH-007: DAG execution failed
//! ```
//!
//! ## Usage Example
//!
//! ```rust
//! use cis_core::error::unified::{CisError, ErrorCategory, ErrorContext};
//! use cis_core::error::Result;
//!
//! fn get_memory(key: &str) -> Result<Vec<u8>> {
//!     if key.is_empty() {
//!         return Err(CisError::new(
//!             ErrorCategory::Memory,
//!             "CIS-MEM-001",
//!             "Memory key cannot be empty"
//!         )
//!         .with_context("key", key)
//!         .with_suggestion("Provide a non-empty memory key"));
//!     }
//!     // ...
//! }
//! ```

// Re-export legacy error types for backward compatibility
pub use super::legacy::{CisError as LegacyCisError, Result as LegacyResult};

// Module organization
mod types;
mod context;
mod convenience;
mod conversions;

// Public API re-exports
pub use types::{
    ErrorCategory,
    ErrorSeverity,
    Recoverability,
    CisError,
    Result,
    ERROR_CODE_PREFIX,
};

pub use context::ErrorContext;

// Convenience constructors are available as methods on CisError
// Trait implementations (Display, Error, From) are automatically included

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::Location;

    #[test]
    fn test_error_codes() {
        let err = CisError::memory_not_found("test-key");
        assert_eq!(err.full_code(), "CIS-MEM-001");
        assert_eq!(err.category, ErrorCategory::Memory);
    }

    #[test]
    fn test_error_display() {
        let err = CisError::memory_not_found("test-key")
            .with_context("attempt", "1")
            .with_context("operation", "get");
        let display = format!("{}", err);
        assert!(display.contains("CIS-MEM-001"));
        assert!(display.contains("test-key"));
        assert!(display.contains("Suggestion"));
    }

    #[test]
    fn test_retryable() {
        let err = CisError::connection_failed("peer1").retryable();
        assert!(err.is_retryable());
    }

    #[test]
    fn test_severity() {
        let err = CisError::internal_error("test").fatal();
        assert_eq!(err.severity, ErrorSeverity::Fatal);
    }

    #[test]
    fn test_legacy_conversion() {
        let legacy = LegacyCisError::not_found("test");
        let unified: CisError = legacy.into();
        assert_eq!(unified.category, ErrorCategory::NotFound);
    }

    #[test]
    fn test_io_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test.txt");
        let cis_err: CisError = io_err.into();
        assert_eq!(cis_err.category, ErrorCategory::Io);
        assert!(cis_err.to_string().contains("test.txt"));
    }

    #[test]
    fn test_with_location() {
        let loc = Location::caller();
        let err = CisError::invalid_input("test", "test error")
            .with_location(loc);
        assert!(err.context.to_string().contains("Location:"));
    }
}
