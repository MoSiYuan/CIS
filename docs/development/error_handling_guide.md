# CIS Error Handling Guide

> **Version**: 1.1.6
> **Last Updated**: 2026-02-12
> **Target Audience**: CIS Developers

---

## Table of Contents

- [Overview](#overview)
- [The Unified Error System](#the-unified-error-system)
- [Error Categories](#error-categories)
- [Best Practices](#best-practices)
- [Common Scenarios](#common-scenarios)
- [Migration Guide](#migration-guide)
- [Testing Errors](#testing-errors)

---

## Overview

CIS v1.1.6 introduces a unified error handling system that provides:

- **Type Safety**: Strongly-typed errors with specific context
- **Machine-Readable Codes**: Structured error codes (e.g., `CIS-MEM-001`)
- **Rich Context**: Errors include causes, suggestions, and source locations
- **Backward Compatibility**: Legacy error types automatically convert to unified errors
- **Recoverability**: Errors indicate whether they are retryable or fatal

### Why a New Error System?

The previous error handling in CIS had several issues:

1. **Inconsistent Error Types**: Each module defined its own error types
2. **Loss of Context**: Generic string messages lost important information
3. **No Retry Information**: Callers couldn't tell if errors were retryable
4. **Poor User Experience**: Errors lacked helpful suggestions for recovery

The unified system addresses all these issues while maintaining backward compatibility.

---

## The Unified Error System

### Core Components

```rust
use cis_core::error::{
    CisError,        // Main error type
    Result,          // Result<T, CisError>
    ErrorCategory,    // Error classification
    ErrorSeverity,    // Error severity level
    ErrorContext,     // Additional context
    Recoverability,   // Recovery information
};
```

### Error Structure

```rust
pub struct CisError {
    pub category: ErrorCategory,      // Error classification
    pub code: String,                 // Error code (e.g., "001")
    pub message: String,               // Human-readable message
    pub suggestion: Option<String>,      // Recovery suggestion
    pub severity: ErrorSeverity,        // Severity level
    pub recoverability: Recoverability,  // Recovery information
    pub context: ErrorContext,          // Additional context
    pub source: Option<Box<dyn StdError + Send + Sync>>,  // Underlying error
}
```

### Error Codes

Error codes follow the format: `CIS-{CATEGORY}-{SPECIFIC}`

| Code | Category | Example |
|-------|----------|----------|
| `CIS-MEM-001` | Memory | Memory key not found |
| `CIS-NET-002` | Network | Connection timeout |
| `CIS-SCH-003` | Scheduler | DAG execution failed |
| `CIS-AGT-001` | Agent | Agent not found |
| `CIS-SKL-002` | Skill | Skill load failed |
| `CIS-AI-001` | AI | AI request failed |

---

## Error Categories

### 1. Memory Errors (`ErrorCategory::Memory`)

For operations related to memory storage and retrieval.

```rust
use cis_core::error::CisError;

// Memory not found
Err(CisError::memory_not_found("user/preferences"))

// Memory set failed
Err(CisError::memory_set_failed("user/preferences", "database locked"))

// Encryption failed
Err(CisError::memory_encryption_failed("key derivation failed"))

// Decryption failed
Err(CisError::memory_decryption_failed("invalid key"))
```

### 2. Network Errors (`ErrorCategory::Network`)

For P2P and network communication issues.

```rust
// Connection failed
Err(CisError::connection_failed("peer:12D3KooW..."))

// Connection timeout
Err(CisError::connection_timeout("peer:12D3KooW..."))

// Authentication failed
Err(CisError::authentication_failed("invalid DID signature"))

// ACL denied
Err(CisError::acl_denied("peer:12D3KooW...", "not in whitelist"))
```

### 3. Scheduler Errors (`ErrorCategory::Scheduler`)

For DAG scheduling and execution issues.

```rust
// DAG not found
Err(CisError::dag_not_found("code-review-workflow"))

// DAG validation error
Err(CisError::dag_validation_error("circular dependency detected"))

// DAG execution failed
Err(CisError::dag_execution_failed("task-1", "skill not found"))

// Dependency not met
Err(CisError::dependency_not_met("task-2", "task-1"))
```

### 4. Agent Errors (`ErrorCategory::Agent`)

For Agent lifecycle and execution issues.

```rust
// Agent not found
Err(CisError::agent_not_found("my-agent"))

// Agent startup failed
Err(CisError::agent_startup_failed("my-agent", "port in use"))

// Agent shutdown failed
Err(CisError::agent_shutdown_failed("my-agent", "still running tasks"))

// Agent timeout
Err(CisError::agent_timeout("my-agent", Duration::from_secs(300)))
```

### 5. Skill Errors (`ErrorCategory::Skill`)

For skill loading and execution issues.

```rust
// Skill not found
Err(CisError::skill_not_found("custom-linter"))

// Skill load failed
Err(CisError::skill_load_failed("custom-linter", "invalid manifest"))

// Skill execution failed
Err(CisError::skill_execution_failed("custom-linter", "runtime error"))

// Permission denied
Err(CisError::skill_permission_denied("custom-linter", "filesystem"))
```

### 6. AI Errors (`ErrorCategory::Ai`)

For AI provider and embedding issues.

```rust
// AI request failed
Err(CisError::ai_request_failed("claude", "rate limit exceeded"))

// Rate limited
Err(CisError::ai_rate_limited("claude"))

// Embedding failed
Err(CisError::embedding_failed("model not available"))
```

---

## Best Practices

### 1. Always Return Structured Errors

```rust
// ❌ BAD: Return string errors
fn get_memory(key: &str) -> std::result::Result<Vec<u8>, String> {
    Err(format!("Key not found: {}", key))
}

// ✅ GOOD: Return CisError
fn get_memory(key: &str) -> Result<Vec<u8>> {
    Err(CisError::memory_not_found(key))
}
```

### 2. Add Context to Errors

```rust
use cis_core::error::{CisError, Result};

fn process_data(id: &str, data: &[u8]) -> Result<()> {
    // Add context about what we're processing
    let result = dangerous_operation(data)
        .map_err(|e| {
            CisError::internal_error(e.to_string())
                .with_context("operation", "dangerous_operation")
                .with_context("data_id", id)
                .with_context("data_length", data.len())
        })?;

    Ok(())
}
```

### 3. Provide Suggestions

```rust
fn connect_to_peer(peer_id: &str) -> Result<()> {
    let peer = lookup_peer(peer_id)
        .map_err(|_| {
            CisError::not_found(peer_id)
                .with_suggestion("Check if the peer is online and try adding it with 'cis peer add'")
        })?;

    // ...
}
```

### 4. Use the Right Severity

```rust
use cis_core::error::{CisError, ErrorSeverity};

// Non-critical: operation can be retried
CisError::connection_failed("peer-1")
    .with_severity(ErrorSeverity::Warning)

// Critical: system may be unstable
CisError::memory_encryption_failed("key derivation failed")
    .with_severity(ErrorSeverity::Critical)

// Fatal: system cannot continue
CisError::internal_error("corrupted state")
    .with_severity(ErrorSeverity::Fatal)
```

### 5. Mark Recoverable Errors

```rust
use std::time::Duration;
use cis_core::error::{CisError, Recoverability};

// Retryable immediately
CisError::connection_failed("peer-1")
    .retryable()

// Retryable after delay
CisError::connection_timeout("peer-1")
    .retryable_after(Duration::from_secs(5))

// Not retryable (needs user action)
CisError::skill_not_found("my-skill")
    .with_recoverability(Recoverability::NotRetryable)
```

### 6. Use Convenience Macros

```rust
use cis_core::error::{CisError, Result};
use cis_core::error::{bail, ensure, ensure_range};

// Early return
fn validate_input(value: i32) -> Result<()> {
    if value < 0 {
        bail!(CisError::invalid_input("value", "must be non-negative"));
    }
    Ok(())
}

// Conditional check
fn divide(a: i32, b: i32) -> Result<i32> {
    ensure!(b != 0, CisError::invalid_input("b", "division by zero"));
    Ok(a / b)
}

// Range check
fn set_volume(level: u8) -> Result<()> {
    ensure_range!(level, 0..=100, CisError::invalid_input("level", "must be 0-100"));
    Ok(())
}
```

---

## Common Scenarios

### Scenario 1: File I/O Error

```rust
use std::fs;
use cis_core::error::{CisError, Result};
use cis_core::error::macros::wrap_err;

fn read_config(path: &str) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|e| {
            wrap_err!(e, "Failed to read configuration file",
                "path" => path,
                "hint" => "Check if file exists and is readable"
            )
        })
}
```

### Scenario 2: Database Error

```rust
use cis_core::error::{CisError, Result};

fn get_user(id: &str) -> Result<User> {
    db.query("SELECT * FROM users WHERE id = ?", [id])
        .map_err(|e| {
            CisError::database_query_failed("SELECT ...", e.to_string())
                .with_context("query_type", "select_user")
                .with_context("user_id", id)
        })?
        .get(0)
        .ok_or_else(|| CisError::not_found(format!("User {}", id)))
}
```

### Scenario 3: Network Retry Logic

```rust
use std::time::Duration;
use cis_core::error::{CisError, Result};
use cis_core::error::macros::retry;

async fn connect_with_retry(peer_id: &str) -> Result<()> {
    retry!(3, Duration::from_millis(100), {
        attempt_connect(peer_id).await
    })
}
```

### Scenario 4: Error Conversion from Dependencies

```rust
use cis_core::error::{CisError, Result};

fn parse_config(toml_str: &str) -> Result<Config> {
    toml::from_str(toml_str)
        .map_err(|e| {
            CisError::config_parse_error("config.toml", e.to_string())
                .with_source(e)
        })
}
```

### Scenario 5: Validation Errors

```rust
use cis_core::error::{CisError, Result};
use cis_core::error::macros::precondition;

fn create_user(username: &str, email: &str) -> Result<User> {
    // Precondition checks
    precondition!(!username.is_empty(), "username cannot be empty");
    precondition!(!email.is_empty(), "email cannot be empty");
    precondition!(
        email.contains('@'),
        "invalid email format",
        "email" => email
    );

    // ... create user
}
```

---

## Migration Guide

### Migrating from Legacy Error Types

If you have code using the old `CisError`, you have several options:

#### Option 1: Update to Unified Errors (Recommended)

```rust
// Before
use cis_core::error::{CisError, Result};

fn get_memory(key: &str) -> Result<Vec<u8>> {
    if key.is_empty() {
        return Err(CisError::invalid_input("Key cannot be empty"));
    }
    // ...
}

// After
use cis_core::error::{CisError, Result};

fn get_memory(key: &str) -> Result<Vec<u8>> {
    ensure!(!key.is_empty(), CisError::invalid_input("key", "cannot be empty"));
    // ...
}
```

#### Option 2: Use Adapter (Temporary)

```rust
use cis_core::error::{
    legacy::{CisError as LegacyError, Result as LegacyResult},
    adapter::ResultAdapter,
};

fn my_function() -> LegacyResult<i32> {
    // Legacy code...
    Ok(42)
}

// Convert to unified
fn unified_wrapper() -> Result<i32> {
    my_function().into_unified()
}
```

#### Option 3: Keep Using Legacy Errors (Supported)

The legacy error type is still available:

```rust
use cis_core::error::legacy::{CisError as LegacyError, Result as LegacyResult};

fn my_function() -> LegacyResult<()> {
    // Code using legacy errors continues to work
}
```

### Automatic Conversion

The unified error type automatically converts from legacy errors:

```rust
use cis_core::error::legacy::{CisError as LegacyError};
use cis_core::error::CisError as UnifiedError;

fn example(legacy: LegacyError) -> UnifiedError {
    // Automatic conversion via From trait
    let unified: UnifiedError = legacy.into();
    unified
}
```

---

## Testing Errors

### Unit Testing Error Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cis_core::error::{CisError, Result};

    #[test]
    fn test_memory_not_found() {
        let err = CisError::memory_not_found("test-key");
        assert_eq!(err.category, ErrorCategory::Memory);
        assert!(err.to_string().contains("test-key"));
        assert!(err.is_retryable());  // Can retry if key was added later
    }

    #[test]
    fn test_error_context() {
        let err = CisError::invalid_input("age", "must be positive")
            .with_context("provided_value", "-5");

        assert!(err.to_string().contains("provided_value"));
        assert!(err.to_string().contains("-5"));
    }

    #[test]
    fn test_error_severity() {
        let err = CisError::internal_error("bug").fatal();
        assert_eq!(err.severity, ErrorSeverity::Fatal);
    }
}
```

### Testing Error Paths

```rust
#[test]
fn test_error_path() -> Result<()> {
    let result = dangerous_operation();

    // Verify it fails with expected error
    assert!(result.is_err());
    let err = result.unwrap_err();

    // Check error category
    assert_eq!(err.category, ErrorCategory::Validation);

    // Check error code
    assert_eq!(err.full_code(), "CIS-VAL-001");

    Ok(())
}
```

---

## Error Handling Checklist

When implementing new functions, use this checklist:

- [ ] Return `Result<T>` instead of bare values
- [ ] Use specific error constructors (e.g., `CisError::memory_not_found()`)
- [ ] Add relevant context with `.with_context()`
- [ ] Provide helpful suggestions with `.with_suggestion()`
- [ ] Set appropriate severity level
- [ ] Mark recoverable errors appropriately
- [ ] Include source error if wrapping another error
- [ ] Write tests for error cases
- [ ] Document error conditions in function docs

---

## Common Patterns

### Pattern 1: Early Validation

```rust
fn validate_request(req: &Request) -> Result<()> {
    ensure!(!req.id.is_empty(), CisError::invalid_input("id", "required"));
    ensure!(req.size > 0, CisError::invalid_input("size", "must be positive"));
    ensure_range!(req.level, 0..=100, CisError::invalid_input("level", "out of range"));
    Ok(())
}
```

### Pattern 2: Resource Cleanup

```rust
use std::fs::File;

fn process_file(path: &str) -> Result<String> {
    let mut file = File::open(path)
        .map_err(|e| CisError::file_not_found(path).with_source(e))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| {
            CisError::io(e.to_string())
                .with_context("operation", "read_file")
                .with_context("path", path)
                .with_source(e)
        })?;

    Ok(content)
}
```

### Pattern 3: Error Aggregation

```rust
fn validate_batch(items: &[Item]) -> Result<()> {
    let mut errors = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        if let Err(e) = validate_item(item) {
            errors.push((idx, e));
        }
    }

    if !errors.is_empty() {
        return Err(CisError::invalid_input("items", format!("{} validation errors", errors.len()))
            .with_context("errors", format!("{:?}", errors)));
    }

    Ok(())
}
```

---

## References

- [Unified Error API](../api/error.md)
- [Error Macros Guide](../macros/error_macros.md)
- [CIS Architecture Guide](../architecture/overview.md)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-12
**Maintainer**: CIS Development Team
