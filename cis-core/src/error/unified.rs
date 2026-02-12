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
//! use cis_core::error::unified::{CisError, CisErrorKind, ErrorContext};
//! use cis_core::error::Result;
//!
//! fn get_memory(key: &str) -> Result<Vec<u8>> {
//!     if key.is_empty() {
//!         return Err(CisError::new(
//!             CisErrorKind::Memory,
//!             "CIS-MEM-001",
//!             "Memory key cannot be empty"
//!         )
//!         .with_context("key", key)
//!         .with_suggestion("Provide a non-empty memory key"));
//!     }
//!     // ...
//! }
//! ```

use std::fmt;
use std::error::Error as StdError;
use std::backtrace::Backtrace;
use std::panic::Location;

// Re-export common types
pub use super::{CisError as LegacyCisError, Result as LegacyResult};

/// Result type alias for the unified error system
pub type Result<T, E = CisError> = std::result::Result<T, E>;

/// Error code prefix for CIS errors
pub const ERROR_CODE_PREFIX: &str = "CIS";

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
// Error Context
// ============================================================================

/// Additional context for an error
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Key-value pairs providing context
    pub context: Vec<(String, String)>,

    /// Source code location where error was created
    pub location: Option<&'static Location<'static>>,

    /// Backtrace if available
    pub backtrace: Option<Backtrace>,
}

impl ErrorContext {
    /// Create a new empty error context
    pub fn new() -> Self {
        Self {
            context: Vec::new(),
            location: None,
            backtrace: std::backtrace::Backtrace::capture().into(),
        }
    }

    /// Add a key-value pair to the context
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    /// Set the source location
    pub fn with_location(mut self, loc: &'static Location<'static>) -> Self {
        self.location = Some(loc);
        self
    }

    /// Get a context value by key
    pub fn get(&self, key: &str) -> Option<&String> {
        self.context.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.context.is_empty() {
            return Ok(());
        }

        writeln!(f, "\nContext:")?;
        for (key, value) in &self.context {
            writeln!(f, "  {}: {}", key, value)?;
        }

        if let Some(loc) = self.location {
            writeln!(f, "Location: {}:{}:{}", loc.file(), loc.line(), loc.column())?;
        }

        Ok(())
    }
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
    pub source: Option<Box<dyn StdError + Send + Sync>>,
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
    pub fn with_source(mut self, source: impl Into<Box<dyn StdError + Send + Sync>>) -> Self {
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

    // ========================================================================
    // Convenience Constructors for Common Errors
    // ========================================================================

    // ----- Memory Errors -----
    pub fn memory_not_found(key: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Memory,
            "001",
            format!("Memory key not found: {}", key.into()),
        )
        .with_suggestion("Check if the key exists or verify the key spelling")
        .retryable()
    }

    pub fn memory_set_failed(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Memory,
            "002",
            format!("Failed to set memory: {}", reason.into()),
        )
        .with_context("key", key)
        .retryable()
    }

    pub fn memory_encryption_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "003",
            format!("Memory encryption failed: {}", reason.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Check if the encryption key is valid")
    }

    pub fn memory_decryption_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "004",
            format!("Memory decryption failed: {}", reason.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("The data may be corrupted or the key may be invalid")
    }

    // ----- Network Errors -----
    pub fn connection_failed(peer: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Network,
            "001",
            format!("Failed to connect to peer: {}", peer.into()),
        )
        .with_suggestion("Check if the peer is online and reachable")
        .retryable_after(std::time::Duration::from_secs(5))
    }

    pub fn connection_timeout(peer: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Network,
            "002",
            format!("Connection timeout to peer: {}", peer.into()),
        )
        .with_suggestion("The peer may be overloaded or the network may be slow")
        .retryable_after(std::time::Duration::from_secs(10))
    }

    pub fn authentication_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "003",
            format!("Authentication failed: {}", reason.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Verify credentials and DID configuration")
    }

    pub fn acl_denied(peer: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "004",
            format!("ACL denied for peer: {}", reason.into()),
        )
        .with_context("peer", peer)
        .with_suggestion("Add the peer to the whitelist or check ACL rules")
    }

    // ----- Scheduler Errors -----
    pub fn dag_not_found(name: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Scheduler,
            "001",
            format!("DAG not found: {}", name.into()),
        )
        .with_suggestion("Check if the DAG name is correct or install the skill")
    }

    pub fn dag_validation_error(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Validation,
            "002",
            format!("DAG validation failed: {}", reason.into()),
        )
        .with_suggestion("Check the DAG definition for circular dependencies or invalid tasks")
    }

    pub fn dag_execution_failed(task: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Scheduler,
            "003",
            format!("DAG task execution failed: {}", reason.into()),
        )
        .with_context("task", task)
        .with_suggestion("Check task logs for detailed error information")
    }

    pub fn dependency_not_met(task: impl Into<String>, dep: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Scheduler,
            "004",
            format!("Dependency not met: task {} depends on {}", task.into(), dep.into()),
        )
        .with_suggestion("Ensure the dependency task completes successfully")
    }

    // ----- Agent Errors -----
    pub fn agent_not_found(name: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Agent,
            "001",
            format!("Agent not found: {}", name.into()),
        )
        .with_suggestion("Check if the agent is running or create a new agent")
    }

    pub fn agent_startup_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Agent,
            "002",
            format!("Agent startup failed: {}", reason.into()),
        )
        .with_context("agent", name)
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Check agent logs and configuration")
    }

    pub fn agent_shutdown_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Agent,
            "003",
            format!("Agent shutdown failed: {}", reason.into()),
        )
        .with_context("agent", name)
        .with_suggestion("Force kill the agent if necessary")
    }

    pub fn agent_timeout(name: impl Into<String>, duration: std::time::Duration) -> Self {
        Self::new(
            ErrorCategory::Agent,
            "004",
            format!("Agent operation timed out after {:?}", duration),
        )
        .with_context("agent", name)
        .with_suggestion("Increase timeout or check if the agent is stuck")
    }

    // ----- Skill Errors -----
    pub fn skill_not_found(name: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Skill,
            "001",
            format!("Skill not found: {}", name.into()),
        )
        .with_suggestion("Install the skill using 'cis skill install'")
    }

    pub fn skill_load_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Skill,
            "002",
            format!("Skill load failed: {}", reason.into()),
        )
        .with_context("skill", name)
        .with_suggestion("Check the skill configuration and dependencies")
    }

    pub fn skill_execution_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Skill,
            "003",
            format!("Skill execution failed: {}", reason.into()),
        )
        .with_context("skill", name)
        .with_suggestion("Check skill logs and input parameters")
    }

    pub fn skill_permission_denied(name: impl Into<String>, permission: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "004",
            format!("Permission denied for skill: {}", permission.into()),
        )
        .with_context("skill", name)
        .with_suggestion("Update skill permissions in the skill manifest")
    }

    // ----- AI Errors -----
    pub fn ai_request_failed(provider: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Ai,
            "001",
            format!("AI request failed: {}", reason.into()),
        )
        .with_context("provider", provider)
        .with_suggestion("Check API key, quota, and network connectivity")
        .retryable_after(std::time::Duration::from_secs(5))
    }

    pub fn ai_rate_limited(provider: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Ai,
            "002",
            format!("Rate limited by provider: {}", provider.into()),
        )
        .with_suggestion("Wait before making more requests or upgrade your plan")
        .retryable_after(std::time::Duration::from_secs(60))
    }

    pub fn embedding_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Ai,
            "003",
            format!("Embedding generation failed: {}", reason.into()),
        )
        .with_suggestion("Check if the embedding model is available")
    }

    // ----- Configuration Errors -----
    pub fn config_not_found(path: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Config,
            "001",
            format!("Configuration file not found: {}", path.into()),
        )
        .with_suggestion("Run 'cis init' to create a default configuration")
    }

    pub fn config_parse_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Config,
            "002",
            format!("Configuration parse error: {}", reason.into()),
        )
        .with_context("file", path)
        .with_suggestion("Check the configuration file syntax")
    }

    pub fn config_validation_error(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Validation,
            "003",
            format!("Configuration validation failed: {}", reason.into()),
        )
        .with_context("field", field)
        .with_suggestion("Fix the configuration field value")
    }

    // ----- Database Errors -----
    pub fn database_not_found(path: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Database,
            "001",
            format!("Database not found: {}", path.into()),
        )
        .with_suggestion("Run 'cis init' to initialize the database")
    }

    pub fn database_query_failed(sql: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Database,
            "002",
            format!("Database query failed: {}", reason.into()),
        )
        .with_context("sql", sql)
        .with_suggestion("Check the SQL syntax and database schema")
    }

    pub fn database_corrupted(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Database,
            "003",
            format!("Database corrupted: {}", reason.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Restore from backup or reinitialize the database")
    }

    // ----- I/O Errors -----
    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Io,
            "001",
            format!("File not found: {}", path.into()),
        )
        .with_suggestion("Check if the file path is correct")
    }

    pub fn permission_denied(path: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Io,
            "002",
            format!("Permission denied: {}", path.into()),
        )
        .with_suggestion("Check file permissions and ownership")
    }

    pub fn disk_full(path: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Io,
            "003",
            format!("Disk full: {}", path.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Free up disk space")
    }

    // ----- Validation Errors -----
    pub fn invalid_input(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Validation,
            "001",
            format!("Invalid input: {}", reason.into()),
        )
        .with_context("field", field)
        .with_suggestion("Check the input value and format")
    }

    pub fn invalid_did(did: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Validation,
            "002",
            format!("Invalid DID: {}", reason.into()),
        )
        .with_context("did", did)
        .with_suggestion("Ensure the DID follows the did:peer format")
    }

    // ----- Concurrency Errors -----
    pub fn lock_timeout(resource: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Concurrency,
            "001",
            format!("Lock timeout on resource: {}", resource.into()),
        )
        .with_suggestion("The operation may be blocked by another operation")
        .retryable()
    }

    pub fn deadlock_detected(resources: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Concurrency,
            "002",
            format!("Potential deadlock detected: {}", resources.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("Retry the operation or restart the application")
    }

    // ----- WASM Errors -----
    pub fn wasm_compilation_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Wasm,
            "001",
            format!("WASM compilation failed: {}", reason.into()),
        )
        .with_suggestion("Check the WASM module bytecode and dependencies")
    }

    pub fn wasm_execution_failed(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Wasm,
            "002",
            format!("WASM execution failed: {}", reason.into()),
        )
        .with_suggestion("Check the skill code and resource limits")
    }

    pub fn wasm_fuel_exhausted() -> Self {
        Self::new(
            ErrorCategory::Wasm,
            "003",
            "WASM fuel exhausted (computation limit exceeded)",
        )
        .with_suggestion("Increase the fuel limit or optimize the skill code")
    }

    pub fn wasm_syscall_blocked(syscall: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Security,
            "004",
            format!("WASM syscall blocked: {}", syscall.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("The skill is trying to perform an unauthorized operation")
    }

    // ----- Internal Errors -----
    pub fn internal_error(reason: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Internal,
            "001",
            format!("Internal error: {}", reason.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("This is a bug, please report it to the CIS team")
    }

    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Internal,
            "002",
            format!("Feature not implemented: {}", feature.into()),
        )
        .with_suggestion("This feature is planned but not yet available")
    }

    pub fn unexpected_state(state: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Internal,
            "003",
            format!("Unexpected state: {}", state.into()),
        )
        .with_severity(ErrorSeverity::Critical)
        .with_suggestion("This may indicate a bug or corrupted state")
    }
}

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

impl StdError for CisError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|err| err.as_ref() as &(dyn StdError + 'static))
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
                Some("Check if file path is correct".into()),
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
                Some("Check if the service is running".into()),
            ),
            std::io::ErrorKind::ConnectionReset => (
                format!("Connection reset: {}", err),
                Some("The connection was closed by the peer".into()),
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
            LegacyCisError::Serialization(err) => Self::from(err),
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
