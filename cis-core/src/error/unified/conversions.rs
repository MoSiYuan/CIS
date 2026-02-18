//! # Error Conversions
//!
//! This module provides implementations for converting from other error types.

use std::fmt;

use super::types::{CisError, ErrorCategory};

// Re-export legacy error for conversion
pub use super::super::legacy::CisError as LegacyCisError;

// ============================================================================
// Trait Implementations
// ============================================================================

impl fmt::Display for CisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} {}] {}",
            self.full_code(),
            self.severity,
            self.message
        )?;

        if let Some(suggestion) = &self.suggestion {
            write!(f, "\nSuggestion: {}", suggestion)?;
        }

        write!(f, "{}", self.context)?;

        if let Some(source) = &self.source {
            write!(f, "\nCaused by: {}", source)?;
        }

        Ok(())
    }
}

impl std::error::Error for CisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|err| err.as_ref() as &(dyn std::error::Error + 'static))
    }
}

// ============================================================================
// Conversions from Standard Library Types
// ============================================================================

impl From<std::io::Error> for CisError {
    fn from(err: std::io::Error) -> Self {
        let kind = err.kind();

        let (message, suggestion) = match kind {
            std::io::ErrorKind::NotFound => (
                format!("File not found: {}", err),
                Some("Check if the file path is correct".into()),
            ),
            std::io::ErrorKind::PermissionDenied => (
                format!("Permission denied: {}", err),
                Some("Check file permissions and ownership".into()),
            ),
            std::io::ErrorKind::ConnectionRefused => (
                format!("Connection refused: {}", err),
                Some("Check if service is running".into()),
            ),
            std::io::ErrorKind::ConnectionReset => (
                format!("Connection reset: {}", err),
                Some("The connection was closed by peer".into()),
            ),
            _ => (
                format!("I/O error: {}", err),
                None,
            ),
        };

        Self::new(ErrorCategory::Io, "000", message)
            .with_suggestion(
                suggestion.unwrap_or_else(|| "Check I/O operations".into()),
            )
            .with_source(err)
    }
}

// ============================================================================
// Conversion from Legacy Error Type
// ============================================================================

impl From<LegacyCisError> for CisError {
    fn from(err: LegacyCisError) -> Self {
        let message = err.to_string();

        match err {
            LegacyCisError::NotFound(_) => Self::new(
                ErrorCategory::Scheduler,
                "000",
                format!("Execution error: {}", message),
            ),
            LegacyCisError::P2P(_) => Self::new(
                ErrorCategory::Network,
                "000",
                format!("P2P error: {}", message),
            ),
            LegacyCisError::Identity(_) => Self::new(
                ErrorCategory::Identity,
                "000",
                format!("Identity error: {}", message),
            ),
            LegacyCisError::Database(err) => Self::from(err),
            LegacyCisError::Io(err) => Self::from(err),
            LegacyCisError::Serialization(err) => Self::new(
                ErrorCategory::Io,
                "000",
                format!("Serialization error: {}", err),
            ),
            LegacyCisError::NotFound(_) => Self::new(
                ErrorCategory::NotFound,
                "000",
                format!("Not found: {}", message),
            ),
            LegacyCisError::AlreadyExists(_) => Self::new(
                ErrorCategory::Validation,
                "000",
                format!("Already exists: {}", message),
            ),
            LegacyCisError::InvalidInput(_) => Self::new(
                ErrorCategory::Validation,
                "000",
                format!("Invalid input: {}", message),
            ),
            LegacyCisError::Configuration(_) => Self::new(
                ErrorCategory::Config,
                "000",
                format!("Configuration error: {}", message),
            ),
            LegacyCisError::Storage(_) => Self::new(
                ErrorCategory::Database,
                "000",
                format!("Storage error: {}", message),
            ),
            LegacyCisError::Skill(_) => Self::new(
                ErrorCategory::Skill,
                "000",
                format!("Skill error: {}", message),
            ),
            LegacyCisError::Vector(_) => Self::new(
                ErrorCategory::Ai,
                "000",
                format!("Vector error: {}", message),
            ),
            LegacyCisError::Conversation(_) => Self::new(
                ErrorCategory::Internal,
                "000",
                format!("Conversation error: {}", message),
            ),
            LegacyCisError::Intent(_) => Self::new(
                ErrorCategory::Internal,
                "000",
                format!("Intent error: {}", message),
            ),
            LegacyCisError::Telemetry(_) => Self::new(
                ErrorCategory::Internal,
                "000",
                format!("Telemetry error: {}", message),
            ),
            LegacyCisError::CloudAnchor(_) => Self::new(
                ErrorCategory::Network,
                "000",
                format!("Cloud Anchor error: {}", message),
            ),
            LegacyCisError::SkillNotFound(_) => Self::skill_not_found(message),
            LegacyCisError::Ai(_) => Self::new(
                ErrorCategory::Ai,
                "000",
                format!("AI error: {}", message),
            ),
            LegacyCisError::Wasm(_) => Self::new(
                ErrorCategory::Wasm,
                "000",
                format!("WASM error: {}", message),
            ),
            LegacyCisError::Matrix(_) => Self::new(
                ErrorCategory::Matrix,
                "000",
                format!("Matrix error: {}", message),
            ),
            LegacyCisError::Federation(_) => Self::new(
                ErrorCategory::Matrix,
                "000",
                format!("Federation error: {}", message),
            ),
            LegacyCisError::Encryption(_) => Self::new(
                ErrorCategory::Security,
                "000",
                format!("Encryption error: {}", message),
            ),
            LegacyCisError::Other(_) => Self::new(
                ErrorCategory::Internal,
                "000",
                format!("Other error: {}", message),
            ),
        }
    }
}
