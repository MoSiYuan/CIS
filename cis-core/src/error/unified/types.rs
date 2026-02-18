//! # Error Type Definitions
//!
//! This module contains the core error type definitions for the unified error system.

use std::fmt;
use std::panic::Location;

use super::context::ErrorContext;

/// Error code prefix for CIS errors
pub const ERROR_CODE_PREFIX: &str = "CIS";

/// Result type alias for the unified error system
pub type Result<T, E = CisError> = std::result::Result<T, E>;

// ============================================================================
// Error Categories
// ============================================================================

/// Error category classifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCategory {
    /// Memory/storage operations
    Memory,

    /// Network and P2P operations
    Network,

    /// Scheduler and DAG execution
    Scheduler,

    /// Agent lifecycle and execution
    Agent,

    /// Skill loading and execution
    Skill,

    /// AI and embedding operations
    Ai,

    /// Security and encryption
    Security,

    /// Configuration and validation
    Config,

    /// Database operations
    Database,

    /// I/O operations
    Io,

    /// Serialization/deserialization
    Serialization,

    /// Matrix federation
    Matrix,

    /// Identity and DID operations
    Identity,

    /// WASM execution
    Wasm,

    /// Lock and concurrency
    Concurrency,

    /// Validation errors
    Validation,

    /// Not found errors
    NotFound,

    /// Internal/bug errors
    Internal,
}

impl ErrorCategory {
    /// Get the short code for this category
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCategory::Memory => "MEM",
            ErrorCategory::Network => "NET",
            ErrorCategory::Scheduler => "SCH",
            ErrorCategory::Agent => "AGT",
            ErrorCategory::Skill => "SKL",
            ErrorCategory::Ai => "AI",
            ErrorCategory::Security => "SEC",
            ErrorCategory::Config => "CFG",
            ErrorCategory::Database => "DB",
            ErrorCategory::Io => "IO",
            ErrorCategory::Serialization => "SER",
            ErrorCategory::Matrix => "MAT",
            ErrorCategory::Identity => "ID",
            ErrorCategory::Wasm => "WSM",
            ErrorCategory::Concurrency => "CON",
            ErrorCategory::Validation => "VAL",
            ErrorCategory::NotFound => "NFD",
            ErrorCategory::Internal => "INT",
        }
    }

    /// Get the display name for this category
    pub fn name(&self) -> &'static str {
        match self {
            ErrorCategory::Memory => "Memory",
            ErrorCategory::Network => "Network",
            ErrorCategory::Scheduler => "Scheduler",
            ErrorCategory::Agent => "Agent",
            ErrorCategory::Skill => "Skill",
            ErrorCategory::Ai => "AI",
            ErrorCategory::Security => "Security",
            ErrorCategory::Config => "Configuration",
            ErrorCategory::Database => "Database",
            ErrorCategory::Io => "I/O",
            ErrorCategory::Serialization => "Serialization",
            ErrorCategory::Matrix => "Matrix",
            ErrorCategory::Identity => "Identity",
            ErrorCategory::Wasm => "WASM",
            ErrorCategory::Concurrency => "Concurrency",
            ErrorCategory::Validation => "Validation",
            ErrorCategory::NotFound => "Not Found",
            ErrorCategory::Internal => "Internal",
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Error Severity
// ============================================================================

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    /// Warning - operation may continue
    Warning = 0,

    /// Info - informational message
    Info = 1,

    /// Error - operation failed but system is intact
    Error = 2,

    /// Critical - system may be unstable
    Critical = 3,

    /// Fatal - system cannot continue
    Fatal = 4,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
            ErrorSeverity::Fatal => write!(f, "FATAL"),
        }
    }
}

// ============================================================================
// Error Recoverability
// ============================================================================

/// Whether an error is recoverable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recoverability {
    /// Operation can be retried immediately
    Retryable,

    /// Operation can be retried after a delay
    RetryableAfterDelay(std::time::Duration),

    /// Operation is not retryable (user action needed)
    NotRetryable,

    /// Error is fatal (system cannot continue)
    Fatal,
}

// ============================================================================
// Unified Error Type
// ============================================================================

/// The unified CIS error type
#[derive(Debug)]
pub struct CisError {
    /// Error category
    pub category: ErrorCategory,

    /// Machine-readable error code (e.g., "CIS-MEM-001")
    pub code: String,

    /// Human-readable error message
    pub message: String,

    /// Suggested recovery action
    pub suggestion: Option<String>,

    /// Error severity
    pub severity: ErrorSeverity,

    /// Whether the error is recoverable
    pub recoverability: Recoverability,

    /// Additional context
    pub context: ErrorContext,

    /// Underlying error source
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl CisError {
    /// Create a new error with minimal information
    pub fn new(
        category: ErrorCategory,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let code = code.into();
        let message = message.into();

        Self {
            category,
            code,
            message,
            suggestion: None,
            severity: ErrorSeverity::Error,
            recoverability: Recoverability::NotRetryable,
            context: ErrorContext::new(),
            source: None,
        }
    }

    /// Set the error severity
    pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the recoverability
    pub fn with_recoverability(mut self, recoverability: Recoverability) -> Self {
        self.recoverability = recoverability;
        self
    }

    /// Add a suggestion for recovery
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add context key-value pair
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context = self.context.with(key, value);
        self
    }

    /// Set the source location
    pub fn with_location(mut self, loc: &'static Location<'static>) -> Self {
        self.context = self.context.with_location(loc);
        self
    }

    /// Set the underlying error source
    pub fn with_source(mut self, source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Mark as retryable
    pub fn retryable(self) -> Self {
        self.with_recoverability(Recoverability::Retryable)
    }

    /// Mark as retryable after delay
    pub fn retryable_after(self, delay: std::time::Duration) -> Self {
        self.with_recoverability(Recoverability::RetryableAfterDelay(delay))
    }

    /// Mark as fatal
    pub fn fatal(self) -> Self {
        self.with_severity(ErrorSeverity::Fatal)
            .with_recoverability(Recoverability::Fatal)
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.recoverability,
            Recoverability::Retryable | Recoverability::RetryableAfterDelay(_)
        )
    }

    /// Get the full error code with category
    pub fn full_code(&self) -> String {
        format!("{}-{}-{}", ERROR_CODE_PREFIX, self.category.code(), self.code)
    }
}
