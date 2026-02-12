//! Error Testing Utilities
//!
//! This module provides utilities for testing error handling,
//! including error simulation and scenario generation.

use crate::error::{
    unified::{CisError, ErrorCategory, ErrorSeverity, Recoverability},
    Result,
};
use std::time::Duration;

/// Builder for creating test errors with various properties
pub struct ErrorBuilder {
    category: ErrorCategory,
    code: String,
    message: String,
    suggestion: Option<String>,
    severity: ErrorSeverity,
    recoverability: Recoverability,
    context: Vec<(String, String)>,
}

impl ErrorBuilder {
    /// Create a new error builder
    pub fn new(category: ErrorCategory, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            category,
            code: code.into(),
            message: message.into(),
            suggestion: None,
            severity: ErrorSeverity::Error,
            recoverability: Recoverability::NotRetryable,
            context: Vec::new(),
        }
    }

    /// Set the error category
    pub fn category(mut self, category: ErrorCategory) -> Self {
        self.category = category;
        self
    }

    /// Set the error code
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    /// Set the error message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the error suggestion
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Set the error severity
    pub fn severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the error recoverability
    pub fn recoverability(mut self, recoverability: Recoverability) -> Self {
        self.recoverability = recoverability;
        self
    }

    /// Add context key-value pair
    pub fn context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    /// Build the error
    pub fn build(self) -> CisError {
        let mut err = CisError::new(self.category, self.code, self.message)
            .with_severity(self.severity)
            .with_recoverability(self.recoverability);

        if let Some(suggestion) = self.suggestion {
            err = err.with_suggestion(suggestion);
        }

        for (key, value) in self.context {
            err = err.with_context(key, value);
        }

        err
    }
}

/// Scenario generator for testing error handling
pub struct ErrorScenarios;

impl ErrorScenarios {
    /// Create a memory not found error
    pub fn memory_not_found(key: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Memory, "001", format!("Memory key not found: {}", key.into()))
            .suggestion("Check if key exists or verify key spelling")
            .recoverability(Recoverability::Retryable)
            .build()
    }

    /// Create a connection failed error
    pub fn connection_failed(peer: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Network, "001", format!("Failed to connect to peer: {}", peer.into()))
            .suggestion("Check if peer is online and reachable")
            .recoverability(Recoverability::RetryableAfterDelay(Duration::from_secs(5)))
            .build()
    }

    /// Create a skill not found error
    pub fn skill_not_found(name: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Skill, "001", format!("Skill not found: {}", name.into()))
            .suggestion("Install the skill using 'cis skill install'")
            .build()
    }

    /// Create an invalid input error
    pub fn invalid_input(field: impl Into<String>, reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Validation, "001", format!("Invalid input: {}", reason.into()))
            .context("field", field)
            .suggestion("Check the input value and format")
            .build()
    }

    /// Create an authentication failed error
    pub fn authentication_failed(reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Security, "003", format!("Authentication failed: {}", reason.into()))
            .severity(ErrorSeverity::Critical)
            .suggestion("Verify credentials and DID configuration")
            .build()
    }

    /// Create a DAG execution failed error
    pub fn dag_execution_failed(task: impl Into<String>, reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Scheduler, "003", format!("DAG task execution failed: {}", reason.into()))
            .context("task", task)
            .suggestion("Check task logs for detailed error information")
            .build()
    }

    /// Create an agent timeout error
    pub fn agent_timeout(name: impl Into<String>, duration: Duration) -> CisError {
        ErrorBuilder::new(ErrorCategory::Agent, "004", format!("Agent operation timed out after {:?}", duration))
            .context("agent", name)
            .context("timeout", format!("{:?}", duration))
            .suggestion("Increase timeout or check if agent is stuck")
            .build()
    }

    /// Create an AI rate limited error
    pub fn ai_rate_limited(provider: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Ai, "002", format!("Rate limited by provider: {}", provider.into()))
            .context("provider", provider)
            .suggestion("Wait before making more requests or upgrade your plan")
            .recoverability(Recoverability::RetryableAfterDelay(Duration::from_secs(60)))
            .build()
    }

    /// Create a configuration not found error
    pub fn config_not_found(path: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Config, "001", format!("Configuration file not found: {}", path.into()))
            .suggestion("Run 'cis init' to create a default configuration")
            .build()
    }

    /// Create a database query failed error
    pub fn database_query_failed(sql: impl Into<String>, reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Database, "002", format!("Database query failed: {}", reason.into()))
            .context("sql", sql)
            .suggestion("Check SQL syntax and database schema")
            .build()
    }

    /// Create a file not found error
    pub fn file_not_found(path: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Io, "001", format!("File not found: {}", path.into()))
            .suggestion("Check if file path is correct")
            .build()
    }

    /// Create a lock timeout error
    pub fn lock_timeout(resource: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Concurrency, "001", format!("Lock timeout on resource: {}", resource.into()))
            .suggestion("The operation may be blocked by another operation")
            .recoverability(Recoverability::Retryable)
            .build()
    }

    /// Create a WASM execution failed error
    pub fn wasm_execution_failed(reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Wasm, "002", format!("WASM execution failed: {}", reason.into()))
            .suggestion("Check the skill code and resource limits")
            .build()
    }

    /// Create an internal error
    pub fn internal_error(reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Internal, "001", format!("Internal error: {}", reason.into()))
            .severity(ErrorSeverity::Critical)
            .suggestion("This is a bug, please report it to the CIS team")
            .recoverability(Recoverability::Fatal)
            .build()
    }

    /// Create a fatal error
    pub fn fatal(reason: impl Into<String>) -> CisError {
        ErrorBuilder::new(ErrorCategory::Internal, "000", format!("Fatal error: {}", reason.into()))
            .severity(ErrorSeverity::Fatal)
            .recoverability(Recoverability::Fatal)
            .suggestion("The system cannot continue and must restart")
            .build()
    }
}

/// Error assertion utilities for tests
pub struct ErrorAssertions;

impl ErrorAssertions {
    /// Assert that an error has the expected category
    pub fn assert_category(err: &CisError, expected: ErrorCategory) {
        assert_eq!(err.category, expected,
            "Expected error category {:?}, got {:?}\nError: {}",
            expected, err.category, err);
    }

    /// Assert that an error is retryable
    pub fn assert_retryable(err: &CisError) {
        assert!(err.is_retryable(),
            "Expected error to be retryable, but it's not\nError: {}", err);
    }

    /// Assert that an error is not retryable
    pub fn assert_not_retryable(err: &CisError) {
        assert!(!err.is_retryable(),
            "Expected error to not be retryable, but it is\nError: {}", err);
    }

    /// Assert that an error has the expected severity
    pub fn assert_severity(err: &CisError, expected: ErrorSeverity) {
        assert_eq!(err.severity, expected,
            "Expected severity {:?}, got {:?}\nError: {}",
            expected, err.severity, err);
    }

    /// Assert that an error contains specific context
    pub fn assert_has_context(err: &CisError, key: &str, value: &str) {
        let actual = err.context.get(key);
        assert!(actual.is_some(),
            "Expected context key '{}', but it's not present\nError: {}",
            key, err);
        assert!(actual.unwrap().contains(value),
            "Expected context value to contain '{}', got '{}'\nError: {}",
            value, actual.unwrap(), err);
    }

    /// Assert that an error has a suggestion
    pub fn assert_has_suggestion(err: &CisError) {
        assert!(err.suggestion.is_some(),
            "Expected error to have a suggestion, but it doesn't\nError: {}", err);
    }

    /// Assert error code matches expected
    pub fn assert_code(err: &CisError, expected: &str) {
        assert_eq!(err.code, expected,
            "Expected error code '{}', got '{}'\nError: {}",
            expected, err.code, err);
    }

    /// Assert full error code matches expected
    pub fn assert_full_code(err: &CisError, expected: &str) {
        assert_eq!(err.full_code(), expected,
            "Expected full error code '{}', got '{}'\nError: {}",
            expected, err.full_code(), err);
    }
}

/// Mock error source for testing
#[derive(Debug, thiserror::Error)]
pub enum MockError {
    #[error("Mock IO error")]
    Io,

    #[error("Mock network error")]
    Network,

    #[error("Mock timeout")]
    Timeout,

    #[error("Mock validation error: {0}")]
    Validation(String),

    #[error("Mock not found: {0}")]
    NotFound(String),
}

/// Test helper to simulate fallible operations
pub struct FallibleOperation<T> {
    result: Result<T, MockError>,
    attempt: u32,
}

impl<T> FallibleOperation<T> {
    /// Create a new fallible operation
    pub fn new(result: Result<T, MockError>) -> Self {
        Self {
            result,
            attempt: 0,
        }
    }

    /// Set the attempt number for context
    pub fn with_attempt(mut self, attempt: u32) -> Self {
        self.attempt = attempt;
        self
    }

    /// Execute the operation
    pub fn execute(self) -> Result<T> {
        self.result.map_err(|e| {
            CisError::internal_error(e.to_string())
                .with_context("mock_error", format!("{:?}", e))
                .with_context("attempt", self.attempt.to_string())
        })
    }
}

/// Create a fallible operation that always fails
pub fn always_fail<T>() -> FallibleOperation<T> {
    FallibleOperation::new(Err(MockError::Internal("Always fails".into()))
}

/// Create a fallible operation that always succeeds
pub fn always_succeed<T>(value: T) -> FallibleOperation<T> {
    FallibleOperation::new(Ok(value))
}

/// Create a fallible operation that fails n times then succeeds
pub fn fail_n_times<T>(fail_count: u32, success_value: T) -> impl FnMut() -> FallibleOperation<T> {
    let mut attempts = 0;
    move || {
        attempts += 1;
        if attempts <= fail_count {
            FallibleOperation::new(Err(MockError::Timeout))
                .with_attempt(attempts)
        } else {
            FallibleOperation::new(Ok(success_value))
                .with_attempt(attempts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_builder() {
        let err = ErrorBuilder::new(ErrorCategory::Memory, "001", "Test error")
            .suggestion("Try again")
            .severity(ErrorSeverity::Warning)
            .context("key", "test-key")
            .build();

        assert_eq!(err.category, ErrorCategory::Memory);
        assert_eq!(err.code, "001");
        assert!(err.suggestion.is_some());
        assert_eq!(err.severity, ErrorSeverity::Warning);
        assert!(err.context.get("key").is_some());
    }

    #[test]
    fn test_scenarios_memory_not_found() {
        let err = ErrorScenarios::memory_not_found("test-key");
        ErrorAssertions::assert_category(&err, ErrorCategory::Memory);
        ErrorAssertions::assert_code(&err, "001");
        ErrorAssertions::assert_retryable(&err);
        ErrorAssertions::assert_has_suggestion(&err);
    }

    #[test]
    fn test_scenarios_connection_failed() {
        let err = ErrorScenarios::connection_failed("peer-1");
        ErrorAssertions::assert_category(&err, ErrorCategory::Network);
        ErrorAssertions::assert_retryable(&err);
        ErrorAssertions::assert_has_suggestion(&err);
    }

    #[test]
    fn test_scenarios_skill_not_found() {
        let err = ErrorScenarios::skill_not_found("my-skill");
        ErrorAssertions::assert_category(&err, ErrorCategory::Skill);
        ErrorAssertions::assert_code(&err, "001");
        ErrorAssertions::assert_not_retryable(&err);
    }

    #[test]
    fn test_assertions() {
        let err = ErrorScenarios::invalid_input("test", "bad value")
            .context("provided", "abc");

        ErrorAssertions::assert_category(&err, ErrorCategory::Validation);
        ErrorAssertions::assert_has_context(&err, "provided", "abc");
        ErrorAssertions::assert_has_suggestion(&err);
    }

    #[test]
    fn test_fallible_operation() {
        let op = FallibleOperation::new(Ok(42));
        assert!(op.execute().is_ok());

        let op = FallibleOperation::new(Err(MockError::NotFound("test".into())));
        assert!(op.execute().is_err());
    }

    #[test]
    fn test_fail_n_times() {
        let mut fail_twice = fail_n_times(2, "success");

        // First two attempts fail
        assert!(fail_twice().execute().is_err());
        assert!(fail_twice().execute().is_err());

        // Third attempt succeeds
        assert!(fail_twice().execute().is_ok());
    }
}
