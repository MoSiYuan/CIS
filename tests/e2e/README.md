# CIS End-to-End (E2E) Test Suite

This directory contains end-to-end tests for the CIS (Cluster of Independent Systems) CLI. These tests verify the complete functionality of the CLI by executing the actual binary and validating its behavior.

## Test Architecture

### Directory Structure

```
tests/e2e/
├── mod.rs           # Test framework public API and configuration
├── helpers.rs       # Test utilities and helper functions
├── init_tests.rs    # Tests for `cis init` command
├── skill_tests.rs   # Tests for `cis skill` command
├── dag_tests.rs     # Tests for `cis dag` command
└── node_tests.rs    # Tests for `cis node` command
```

### Test Framework Components

#### `TestEnv` (helpers.rs)

The `TestEnv` struct provides an isolated test environment for each test:

```rust
let env = TestEnv::new()?;  // Creates a temporary directory
env.init_cis()?;             // Initializes CIS in the temp directory
env.cis_cmd()                // Returns a Command configured for the test env
    .arg("status")
    .assert()
    .success();
```

Features:
- **Isolated directories**: Each test gets its own temporary directory
- **Automatic cleanup**: Temp directories are cleaned up after tests
- **Convenient helpers**: `file_exists()`, `read_file()`, `write_file()`

#### DAG File Helpers (helpers.rs)

Helper functions for creating test DAG files:

```rust
// Create a simple test DAG
create_test_dag_file(env.path(), "test.dag");

// Create a DAG with skill-based tasks
create_skill_dag_file(env.path(), "skill.dag");

// Create an invalid DAG for error testing
create_invalid_dag_file(env.path(), "invalid.dag");

// Create a DAG with circular dependencies
create_circular_dag_file(env.path(), "circular.dag");
```

## Running E2E Tests

### Run All E2E Tests

```bash
cargo test --test e2e
```

### Run Specific Test File

```bash
# Run only init tests
cargo test --test e2e init_tests

# Run only skill tests
cargo test --test e2e skill_tests
```

### Run Specific Test

```bash
# Run a single test
cargo test test_cis_init_creates_directory_structure

# Run tests matching a pattern
cargo test skill_list
```

### Run with Output

```bash
# Show test output for debugging
cargo test --test e2e -- --nocapture

# Show only failed test output
cargo test --test e2e -- --show-output
```

### Run with All Features

```bash
cargo test --test e2e --all-features
```

## Adding New Tests

### Basic Test Template

```rust
use crate::e2e::helpers::TestEnv;
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_my_new_feature() {
    // Setup: Create test environment
    let env = TestEnv::new().unwrap();
    
    // Initialize CIS (if needed)
    env.init_cis().expect("Failed to initialize CIS");
    
    // Execute command
    env.cis_cmd()
        .arg("my-command")
        .arg("--my-option")
        .assert()
        .success()
        .stdout(predicate::str::contains("expected output"));
    
    // Verify state
    assert!(env.file_exists("expected-file.txt"));
}
```

### Test with Custom DAG File

```rust
use crate::e2e::helpers::{TestEnv, create_test_dag_file};

#[test]
fn test_dag_with_custom_tasks() {
    let env = TestEnv::new().unwrap();
    env.init_cis().unwrap();
    
    // Create custom DAG
    let dag_path = create_test_dag_file(env.path(), "custom.dag");
    
    // Run the DAG
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();
}
```

### Test Error Conditions

```rust
#[test]
fn test_command_fails_gracefully() {
    let env = TestEnv::new().unwrap();
    env.init_cis().unwrap();
    
    env.cis_cmd()
        .arg("skill")
        .arg("info")
        .arg("nonexistent-skill")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
```

### Async Test Template

```rust
#[tokio::test]
async fn test_async_operation() {
    let env = TestEnv::new().unwrap();
    env.init_cis().unwrap();
    
    // For async operations, use Command directly
    let output = tokio::process::Command::new("cargo")
        .args(["run", "--", "dag", "run", "test.dag"])
        .current_dir(env.path())
        .output()
        .await
        .unwrap();
    
    assert!(output.status.success());
}
```

## Debugging Tips

### 1. View Test Output

```bash
cargo test test_name -- --nocapture
```

### 2. Preserve Temp Directories

Modify `TestEnv` to skip cleanup for debugging:

```rust
// In TestEnv::new(), use into_path() instead of keeping TempDir
pub fn new_debug() -> anyhow::Result<(Self, PathBuf)> {
    let temp_dir = tempdir()?.into_path(); // Consumes TempDir, won't auto-delete
    // ...
}
```

### 3. Run Binary Directly

```bash
# Build the binary
cargo build --bin cis-node

# Run directly
./target/debug/cis-node init --non-interactive
```

### 4. Use Environment Variables

```bash
# Set CIS data directory
export CIS_DATA_DIR=/tmp/cis-test

# Run command
./target/debug/cis-node status
```

### 5. Check Test Environment

Add debug output to tests:

```rust
#[test]
fn test_with_debug() {
    let env = TestEnv::new().unwrap();
    println!("Temp directory: {:?}", env.path());
    println!("Data directory: {:?}", env.data_dir);
    
    // ... rest of test
}
```

### 6. Use Assertions with Messages

```rust
assert!(
    env.file_exists("config.toml"),
    "Config file should exist. Temp dir: {:?}",
    env.path()
);
```

## Common Issues

### "CIS not initialized" Error

Make sure to call `env.init_cis()` before running commands that require initialization.

### Tests Hanging

Some tests (like `dag execute`) may hang if they wait for input. Use timeout:

```rust
env.cis_cmd()
    .arg("dag")
    .arg("execute")
    .timeout(Duration::from_secs(30))
    .assert();
```

### Permission Errors

On Unix systems, ensure the binary is executable:

```bash
chmod +x target/debug/cis-node
```

### Windows Path Issues

Use `PathBuf` and `Path` for cross-platform path handling:

```rust
let path = env.path().join("subdir").join("file.txt");
```

## Best Practices

1. **Always use TestEnv**: Don't create temporary directories manually
2. **Clean up resources**: Tests should not leave artifacts behind
3. **Use predicates**: Leverage `predicates` crate for flexible assertions
4. **Test both success and failure**: Include negative test cases
5. **Keep tests independent**: Each test should be able to run alone
6. **Use descriptive names**: Test names should describe what they verify
7. **Add timeout for long operations**: Prevent tests from hanging
8. **Ignore flaky tests**: Use `#[ignore]` for tests requiring external resources

## Dependencies

The E2E tests use these crates:

- `assert_cmd`: Command assertions for CLI testing
- `predicates`: Predicate assertions for flexible matching
- `tempfile`: Temporary directory management
- `anyhow`: Error handling

These are added to `[dev-dependencies]` in `Cargo.toml`.
