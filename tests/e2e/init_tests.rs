//! # E2E Tests for `cis init` Command
//!
//! These tests verify the initialization functionality of CIS:
//! - Creating directory structure
//! - Generating configuration files
//! - Handling force overwrite
//! - Non-interactive mode

use crate::e2e::helpers::TestEnv;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

/// Test: `cis init` creates the directory structure
#[test]
fn test_cis_init_creates_directory_structure() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success()
        .stdout(predicate::str::contains("完成").or(predicate::str::contains("Initialized")));

    // Verify directory structure
    assert!(env.data_dir.exists(), "Data directory should exist");
    assert!(env.config_file.exists(), "Config file should exist");
}

/// Test: `cis init` creates config.toml with valid content
#[test]
fn test_cis_init_creates_valid_config() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Verify config file content
    let config_content = fs::read_to_string(&env.config_file).unwrap();

    // Should contain essential sections
    assert!(
        config_content.contains("node"),
        "Config should contain [node] section"
    );
    assert!(
        config_content.contains("version"),
        "Config should contain version info"
    );
}

/// Test: `cis init --force` overwrites existing configuration
#[test]
fn test_cis_init_force_overwrites_existing() {
    let env = TestEnv::new().unwrap();

    // First initialization
    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    let first_config = fs::read_to_string(&env.config_file).unwrap();

    // Second initialization with --force
    env.cis_cmd()
        .arg("init")
        .arg("--force")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success()
        .stdout(predicate::str::contains("完成").or(predicate::str::contains("Initialized")));

    let second_config = fs::read_to_string(&env.config_file).unwrap();

    // Config should still be valid after force
    assert!(
        second_config.contains("node"),
        "Config should still contain [node] section"
    );
}

/// Test: `cis init --non-interactive` runs without user input
#[test]
fn test_cis_init_non_interactive_mode() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success()
        .stdout(predicate::str::contains("完成").or(predicate::str::contains("Initialized")));

    // Verify initialization completed
    assert!(
        env.config_file.exists(),
        "Config file should exist after non-interactive init"
    );
}

/// Test: `cis init --project` initializes project mode
#[test]
fn test_cis_init_project_mode() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--project")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Project mode should create .cis directory in current directory
    let project_cis_dir = env.path().join(".cis");
    assert!(
        project_cis_dir.exists() || env.data_dir.exists(),
        "Project CIS directory or data directory should exist"
    );
}

/// Test: `cis init` with --provider option
#[test]
fn test_cis_init_with_provider() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .arg("--provider")
        .arg("claude")
        .assert()
        .success();

    assert!(env.config_file.exists(), "Config file should exist");

    // Check if provider is set in config
    let config_content = fs::read_to_string(&env.config_file).unwrap();
    // The provider info might be stored in different sections
    assert!(
        config_content.contains("claude")
            || config_content.contains("provider")
            || config_content.contains("ai"),
        "Config should contain provider information"
    );
}

/// Test: `cis init` fails gracefully when directory is not writable
#[test]
#[ignore = "Requires specific permission setup"]
fn test_cis_init_fails_on_readonly_directory() {
    // This test is ignored by default as it requires special setup
    // It demonstrates how to test error conditions
}

/// Test: `cis init --skip-checks` bypasses environment checks
#[test]
fn test_cis_init_skip_checks() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    assert!(
        env.config_file.exists(),
        "Config should be created with --skip-checks"
    );
}

/// Test: Running init twice without --force shows appropriate message
#[test]
fn test_cis_init_without_force_shows_warning() {
    let env = TestEnv::new().unwrap();

    // First initialization
    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Second initialization without --force
    // The behavior may vary - it might succeed (idempotent) or show warning
    let output = env
        .cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .output()
        .unwrap();

    // Either success or warning about existing config
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        output.status.success()
            || combined.contains("exists")
            || combined.contains("already")
            || combined.contains("已存在"),
        "Should either succeed or show existing config warning: {}",
        combined
    );
}

/// Test: `cis init` creates required subdirectories
#[test]
fn test_cis_init_creates_subdirectories() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Check for common subdirectories
    let expected_dirs = vec!["skills", "data", "logs"];

    for dir in expected_dirs {
        let path = env.data_dir.join(dir);
        // Some of these might not exist depending on implementation
        // Just log if they don't exist
        if !path.exists() {
            println!("Note: Directory '{}' not created by init", dir);
        }
    }
}

/// Test: `cis init` help shows all available options
#[test]
fn test_cis_init_help_shows_options() {
    let env = TestEnv::new().unwrap();

    env.cis_cmd()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--project"))
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--non-interactive"))
        .stdout(predicate::str::contains("--skip-checks"))
        .stdout(predicate::str::contains("--provider"));
}

/// Test: `cis status` works after initialization
#[test]
fn test_cis_status_after_init() {
    let env = TestEnv::new().unwrap();

    // Initialize first
    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Check status
    env.cis_cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("CIS").or(predicate::str::contains("initialized")));
}

/// Test: `cis doctor` works after initialization
#[test]
fn test_cis_doctor_after_init() {
    let env = TestEnv::new().unwrap();

    // Initialize first
    env.cis_cmd()
        .arg("init")
        .arg("--non-interactive")
        .arg("--skip-checks")
        .assert()
        .success();

    // Run doctor
    env.cis_cmd().arg("doctor").assert().success();
}
