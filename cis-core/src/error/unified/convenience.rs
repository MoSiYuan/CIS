//! # Convenience Error Constructors
//!
//! This module provides convenience constructors for common error types.

use super::types::{CisError, ErrorCategory, ErrorSeverity, Recoverability};

impl CisError {
    // ========================================================================
    // Memory Errors
    // ========================================================================

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

    // ========================================================================
    // Network Errors
    // ========================================================================

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

    // ========================================================================
    // Scheduler Errors
    // ========================================================================

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

    // ========================================================================
    // Agent Errors
    // ========================================================================

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

    // ========================================================================
    // Skill Errors
    // ========================================================================

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

    // ========================================================================
    // AI Errors
    // ========================================================================

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

    // ========================================================================
    // Configuration Errors
    // ========================================================================

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

    // ========================================================================
    // Database Errors
    // ========================================================================

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

    // ========================================================================
    // I/O Errors
    // ========================================================================

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

    // ========================================================================
    // Validation Errors
    // ========================================================================

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
        .with_suggestion("Verify the DID format and ensure it matches did:cis:node:xxx")
    }

    /// Identity/DID error (for backward compatibility with legacy CisError::identity)
    pub fn identity(msg: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Identity,
            "001",
            format!("Identity error: {}", msg.into()),
        )
        .with_severity(ErrorSeverity::Error)
        .with_suggestion("Check the DID configuration and key storage")
    }
        )
        .with_context("did", did)
        .with_suggestion("Ensure the DID follows the did:peer format")
    }

    // ========================================================================
    // Concurrency Errors
    // ========================================================================

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

    // ========================================================================
    // WASM Errors
    // ========================================================================

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

    /// Generic WASM error (for backward compatibility)
    pub fn wasm(msg: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Wasm,
            "000",
            format!("WASM error: {}", msg.into()),
        )
        .with_severity(ErrorSeverity::Error)
    }

    // ========================================================================
    // Internal Errors
    // ========================================================================

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
