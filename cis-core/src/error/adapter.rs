//! Error adapter for backward compatibility
//!
//! This module provides adapter traits and implementations
//! for converting between legacy and unified error types.

use crate::error::{
    legacy::{CisError as LegacyError, Result as LegacyResult},
    unified::{CisError, ErrorCategory},
    Result,
};

/// Trait for converting legacy errors to unified errors
pub trait IntoUnifiedError {
    fn into_unified(self) -> CisError;
}

impl IntoUnifiedError for LegacyError {
    fn into_unified(self) -> CisError {
        CisError::from(self)
    }
}

/// Extension trait to convert LegacyResult to unified Result
pub trait ResultAdapter<T> {
    fn into_unified(self) -> Result<T>;
}

impl<T> ResultAdapter<T> for LegacyResult<T> {
    fn into_unified(self) -> Result<T> {
        self.map_err(|e| e.into_unified())
    }
}

/// Extension trait for converting unified errors to legacy
pub trait IntoLegacyError {
    fn into_legacy(self) -> LegacyError;
}

impl IntoLegacyError for CisError {
    fn into_legacy(self) -> LegacyError {
        match self.category {
            ErrorCategory::Memory => LegacyError::Memory(self.to_string()),
            ErrorCategory::Network => LegacyError::P2P(self.to_string()),
            ErrorCategory::Scheduler => LegacyError::Scheduler(self.to_string()),
            ErrorCategory::Agent => LegacyError::Execution(self.to_string()),
            ErrorCategory::Skill => LegacyError::Skill(self.to_string()),
            ErrorCategory::Ai => LegacyError::Ai(self.to_string()),
            ErrorCategory::Config => LegacyError::Configuration(self.to_string()),
            ErrorCategory::Database => LegacyError::Storage(self.to_string()),
            ErrorCategory::Io => {
                // Extract the IO error message
                let msg = self.to_string();
                LegacyError::Other(msg)
            }
            ErrorCategory::Serialization => LegacyError::Serialization(
                serde_json::Error::msg(self.to_string()).into()
            ),
            ErrorCategory::Validation => LegacyError::InvalidInput(self.to_string()),
            ErrorCategory::NotFound => LegacyError::NotFound(self.to_string()),
            ErrorCategory::Security => LegacyError::Encryption(self.to_string()),
            _ => LegacyError::Other(self.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_to_unified_conversion() {
        let legacy = LegacyError::not_found("test");
        let unified = legacy.into_unified();
        assert_eq!(unified.category, ErrorCategory::NotFound);
    }

    #[test]
    fn test_unified_to_legacy_conversion() {
        let unified = CisError::memory_not_found("test");
        let legacy = unified.into_legacy();
        match legacy {
            LegacyError::Memory(_) => {},
            _ => panic!("Expected Memory error"),
        }
    }

    #[test]
    fn test_result_adapter() {
        let legacy_result: LegacyResult<i32> = Ok(42);
        let unified_result: Result<i32> = legacy_result.into_unified();
        assert!(unified_result.is_ok());
        assert_eq!(unified_result.unwrap(), 42);
    }
}
