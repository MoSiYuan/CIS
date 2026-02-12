//! # CIS Error Handling Module
//!
//! This module provides comprehensive error handling for CIS.
//!
//! ## Organization
//!
//! - [`unified`] - New unified error system with rich context
//! - [`legacy`] - Legacy error types (kept for backward compatibility)
//! - [`macros`] - Error handling convenience macros
//!
//! ## Migration Path
//!
//! New code should use the unified error system:
//!
//! ```rust
//! use cis_core::error::{CisError, Result};
//! use cis_core::error::unified::{ErrorCategory, ErrorSeverity};
//!
//! fn my_function() -> Result<()> {
//!     Err(CisError::memory_not_found("key"))
//!         .with_context("operation", "get")
//!         .with_severity(ErrorSeverity::Warning)
//! }
//! ```
//!
//! Legacy code can continue using the old error types:
//!
//! ```rust
//! use cis_core::error::legacy::{CisError as LegacyError, Result as LegacyResult};
//! ```

pub mod unified;
pub mod legacy;
pub mod macros;
pub mod adapter;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// Re-export unified error types as the default
pub use unified::{
    CisError,
    Result,
    ErrorCategory,
    ErrorSeverity,
    ErrorContext,
    Recoverability,
};

// Re-export legacy types for backward compatibility
pub use legacy::{
    CisError as LegacyCisError,
    Result as LegacyResult,
};

// Re-export adapter utilities
pub use adapter::{IntoUnifiedError, IntoLegacyError, ResultAdapter};

// Re-export all macros
pub use macros::*;
