//! # CIS CLI Integration Tests
//!
//! Comprehensive integration tests for CIS CLI commands including:
//! - Task commands (list, create, update, delete, show)
//! - Session commands (list, create, delete, acquire, release, cleanup)
//! - Engine commands (scan, report, list-engines)
//! - Migrate commands (run, verify, rollback)
//!
//! Tests use temporary directories and clean up artifacts after completion.

use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test directory for temporary artifacts
const TEST_TEMP_DIR: &str = "cis_test_temp";

/// =============================================================================
/// Test Helper Functions
/// =============================================================================

/// Get the project root directory
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf()
}

/// Get the cis-node directory
fn cis_node_dir() -> PathBuf {
    project_root().join("cis-node")
}

/// Create a temporary test directory
fn create_temp_dir(test_name: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir()
        .join(TEST_TEMP_DIR)
        .join(test_name);

    // Clean up if exists from previous failed test
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    fs::create_dir_all(&temp_dir)
        .expect("Failed to create temp directory");

    temp_dir
}

/// Clean up temporary test directory
fn cleanup_temp_dir(dir: &Path) {
    if dir.exists() {
        fs::remove_dir_all(dir).ok();
    }
}

/// Create a mock CI environment variables
fn set_ci_env_vars() {
    std::env::set_var("CI", "true");
    std::env::set_var("GITHUB_ACTIONS", "true");
    std::env::set_var("GITHUB_REF", "refs/heads/main");
    std::env::set_var("GITHUB_SHA", "abc123");
}

/// Clear CI environment variables
fn clear_ci_env_vars() {
    std::env::remove_var("CI");
    std::env::remove_var("GITHUB_ACTIONS");
    std::env::remove_var("GITHUB_REF");
    std::env::remove_var("GITHUB_SHA");
}

/// Create a sample TOML task file for migration tests
fn create_sample_task_toml(dir: &Path) -> PathBuf {
    let toml_path = dir.join("test_tasks.toml");
    let content = r#"
# Test Tasks Definition

[[task]]
id = "TEST-001"
name = "Test Refactoring Task"
type = "refactoring"
priority = "P0"
effort_person_days = 3

prompt = """
# Test Task

## Objective
Refactor test code

## Steps
1. Review code
2. Make changes
3. Test
"""

dependencies = []

[[task]]
id = "TEST-002"
name = "Test Documentation Task"
type = "documentation"
priority = "P1"
effort_person_days = 1

prompt = """
# Documentation Task

## Objective
Write documentation

## Steps
1. Read code
2. Write docs
"""

dependencies = ["TEST-001"]
"#;

    fs::write(&toml_path, content)
        .expect("Failed to write test TOML file");

    toml_path
}

/// Create a sample game directory structure for engine scan tests
fn create_sample_game_dir(dir: &Path) -> PathBuf {
    let game_dir = dir.join("test_game");

    // Create common game directories
    fs::create_dir_all(game_dir.join("Content")).ok();
    fs::create_dir_all(game_dir.join("Source")).ok();
    fs::create_dir_all(game_dir.join("Scripts")).ok();
    fs::create_dir_all(game_dir.join("Assets")).ok();

    // Add sample files
    fs::write(game_dir.join("Content/Game.ini"), "[Game]\nName=TestGame").ok();
    fs::write(game_dir.join("Source/Game.cpp"), "// Game source code").ok();

    game_dir
}

/// Get current Unix timestamp
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

/// =============================================================================
/// Test Assertions
/// =============================================================================

/// Assert command exited successfully
fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{}: Command failed with exit code {:?}\nstdout: {}\nstderr: {}",
        context,
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Assert command failed with error
fn assert_failure(output: &std::process::Output, context: &str) {
    assert!(
        !output.status.success(),
        "{}: Command should have failed but succeeded\nstdout: {}\nstderr: {}",
        context,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Assert output contains expected text
fn assert_contains(output: &std::process::Output, text: &str, context: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    assert!(
        combined.contains(text),
        "{}: Expected output to contain '{}'\nActual output:\n{}",
        context,
        text,
        combined
    );
}

/// =============================================================================
/// Task Commands Tests
/// =============================================================================

#[test]
fn test_task_list_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "list", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task list --help");
    assert_contains(&output, "Filter by status", "task list help should mention status filter");
}

#[test]
fn test_task_create_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "create", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task create --help");
    assert_contains(&output, "task_id", "task create help should mention task_id");
    assert_contains(&output, "priority", "task create help should mention priority");
}

#[test]
fn test_task_update_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "update", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task update --help");
    assert_contains(&output, "status", "task update help should mention status");
}

#[test]
fn test_task_delete_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "delete", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task delete --help");
    assert_contains(&output, "task_id", "task delete help should mention task_id");
}

#[test]
fn test_task_show_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "show", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task show --help");
}

#[test]
fn test_task_group_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "task --help");
    assert_contains(&output, "list", "task help should list 'list' subcommand");
    assert_contains(&output, "create", "task help should list 'create' subcommand");
    assert_contains(&output, "update", "task help should list 'update' subcommand");
    assert_contains(&output, "delete", "task help should list 'delete' subcommand");
}

#[test]
#[ignore = "Requires CIS initialization and database"]
fn test_task_list_with_filters() {
    let temp_dir = create_temp_dir("task_list_filters");

    // Test with status filter
    let output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "list",
            "--status", "pending",
            "--priority", "P0",
            "--limit", "10"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute command");

    // Command should execute (may not find tasks)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("task list output:\nstdout: {}\nstderr: {}", stdout, stderr);

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires CIS initialization and database"]
fn test_task_create_and_delete() {
    let temp_dir = create_temp_dir("task_create_delete");

    // Create a task
    let create_output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "create",
            "TEST-INT-001",
            "Integration Test Task",
            "--type", "test",
            "--priority", "P1"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute task create");

    assert_success(&create_output, "task create");
    assert_contains(&create_output, "TEST-INT-001", "created task should show task ID");

    // Delete the task
    let delete_output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "delete",
            "TEST-INT-001"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute task delete");

    assert_success(&delete_output, "task delete");

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires CIS initialization and database"]
fn test_task_update_status() {
    let temp_dir = create_temp_dir("task_update_status");

    // First create a task
    let _ = Command::new("cargo")
        .args([
            "run", "--",
            "task", "create",
            "TEST-INT-002",
            "Update Test Task",
            "--type", "refactoring",
            "--priority", "P0"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to create task");

    // Update task status
    let update_output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "update",
            "TEST-INT-002",
            "--status", "running"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute task update");

    assert_success(&update_output, "task update");

    cleanup_temp_dir(&temp_dir);
}

/// =============================================================================
/// Session Commands Tests
/// =============================================================================

#[test]
fn test_session_list_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "list", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session list --help");
    assert_contains(&output, "agent-id", "session list help should mention agent-id filter");
}

#[test]
fn test_session_create_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "create", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session create --help");
    assert_contains(&output, "agent_type", "session create help should mention agent_type");
    assert_contains(&output, "capacity", "session create help should mention capacity");
    assert_contains(&output, "ttl", "session create help should mention TTL");
}

#[test]
fn test_session_acquire_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "acquire", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session acquire --help");
    assert_contains(&output, "min-capacity", "session acquire help should mention min-capacity");
}

#[test]
fn test_session_release_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "release", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session release --help");
    assert_contains(&output, "session-id", "session release help should mention session-id");
}

#[test]
fn test_session_cleanup_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "cleanup", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session cleanup --help");
    assert_contains(&output, "older-than", "session cleanup help should mention older-than");
}

#[test]
fn test_session_group_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "session --help");
    assert_contains(&output, "list", "session help should list 'list' subcommand");
    assert_contains(&output, "create", "session help should list 'create' subcommand");
    assert_contains(&output, "acquire", "session help should list 'acquire' subcommand");
    assert_contains(&output, "release", "session help should list 'release' subcommand");
    assert_contains(&output, "cleanup", "session help should list 'cleanup' subcommand");
}

#[test]
#[ignore = "Requires CIS initialization and agent runtime"]
fn test_session_create_and_show() {
    let temp_dir = create_temp_dir("session_create_show");

    // Create a session
    let create_output = Command::new("cargo")
        .args([
            "run", "--",
            "session", "create",
            "claude",
            "claude",
            "--capacity", "100000",
            "--ttl", "60"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute session create");

    assert_success(&create_output, "session create");

    // Extract session ID from output (assuming it's shown in stdout)
    let stdout = String::from_utf8_lossy(&create_output.stdout);
    println!("Session create output: {}", stdout);

    // Show session details (would need to parse session ID)
    // For now, just verify create executed

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires CIS initialization and existing sessions"]
fn test_session_list_with_filters() {
    let temp_dir = create_temp_dir("session_list_filters");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "session", "list",
            "--agent-id", "1",
            "--status", "active"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute session list");

    // Command should execute (may not find sessions)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("session list output:\nstdout: {}\nstderr: {}", stdout, stderr);

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires CIS initialization and existing sessions"]
fn test_session_cleanup_old_sessions() {
    let temp_dir = create_temp_dir("session_cleanup");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "session", "cleanup",
            "--older-than", "7"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute session cleanup");

    // Should execute cleanup
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Session cleanup output: {}", stdout);

    cleanup_temp_dir(&temp_dir);
}

/// =============================================================================
/// Engine Commands Tests
/// =============================================================================

#[test]
fn test_engine_scan_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "scan", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "engine scan --help");
    assert_contains(&output, "directory", "engine scan help should mention directory");
    assert_contains(&output, "engine-type", "engine scan help should mention engine-type");
}

#[test]
fn test_engine_report_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "report", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "engine report --help");
    assert_contains(&output, "scan-result", "engine report help should mention scan-result");
    assert_contains(&output, "format", "engine report help should mention format");
}

#[test]
fn test_engine_list_engines() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "list-engines", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "engine list-engines --help");
}

#[test]
fn test_engine_group_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "engine --help");
    assert_contains(&output, "scan", "engine help should list 'scan' subcommand");
    assert_contains(&output, "report", "engine help should list 'report' subcommand");
    assert_contains(&output, "list-engines", "engine help should list 'list-engines' subcommand");
}

#[test]
#[ignore = "Requires actual engine code scanning implementation"]
fn test_engine_scan_directory() {
    let temp_dir = create_temp_dir("engine_scan");
    let game_dir = create_sample_game_dir(&temp_dir);

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "engine", "scan",
            game_dir.to_str().unwrap()
        ])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute engine scan");

    // Scan should execute
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Engine scan output:\nstdout: {}\nstderr: {}", stdout, stderr);

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires actual engine code scanning implementation"]
fn test_engine_scan_with_output() {
    let temp_dir = create_temp_dir("engine_scan_output");
    let game_dir = create_sample_game_dir(&temp_dir);
    let output_file = temp_dir.join("scan_results.json");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "engine", "scan",
            game_dir.to_str().unwrap(),
            "--output", output_file.to_str().unwrap()
        ])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute engine scan with output");

    // Check if output file was created
    if output.status.success() {
        assert!(
            output_file.exists(),
            "engine scan should create output file"
        );
    }

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires scan results file"]
fn test_engine_report_generation() {
    let temp_dir = create_temp_dir("engine_report");
    let scan_result = temp_dir.join("scan_results.json");

    // Create a mock scan result file
    let mock_scan = r#"{"engine":"unity","files":[]}"#;
    fs::write(&scan_result, mock_scan).ok();

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "engine", "report",
            scan_result.to_str().unwrap(),
            "--format", "markdown"
        ])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute engine report");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Engine report output: {}", stdout);

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires engine type listing implementation"]
fn test_engine_list_engines_output() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "list-engines"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute engine list-engines");

    // Should list supported engines
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Supported engines:\n{}", stdout);

    cleanup_temp_dir(&PathBuf::from("."));
}

/// =============================================================================
/// Migrate Commands Tests
/// =============================================================================

#[test]
fn test_migrate_run_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "migrate", "run", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "migrate run --help");
    assert_contains(&output, "source", "migrate run help should mention source");
    assert_contains(&output, "verify", "migrate run help should mention verify flag");
}

#[test]
fn test_migrate_verify_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "migrate", "verify", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "migrate verify --help");
    assert_contains(&output, "database", "migrate verify help should mention database");
}

#[test]
fn test_migrate_rollback_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "migrate", "rollback", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "migrate rollback --help");
    assert_contains(&output, "before", "migrate rollback help should mention before timestamp");
}

#[test]
fn test_migrate_group_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "migrate", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_success(&output, "migrate --help");
    assert_contains(&output, "run", "migrate help should list 'run' subcommand");
    assert_contains(&output, "verify", "migrate help should list 'verify' subcommand");
    assert_contains(&output, "rollback", "migrate help should list 'rollback' subcommand");
}

#[test]
#[ignore = "Requires database and migration implementation"]
fn test_migrate_run_from_toml() {
    let temp_dir = create_temp_dir("migrate_run_toml");
    let toml_file = create_sample_task_toml(&temp_dir);
    let db_file = temp_dir.join("test_tasks.db");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "migrate", "run",
            toml_file.to_str().unwrap(),
            "--database", db_file.to_str().unwrap()
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute migrate run");

    assert_success(&output, "migrate run from TOML");

    // Check if database was created
    if output.status.success() {
        assert!(
            db_file.exists(),
            "migrate should create database file"
        );
    }

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires database and migration implementation"]
fn test_migrate_run_with_verify() {
    let temp_dir = create_temp_dir("migrate_run_verify");
    let toml_file = create_sample_task_toml(&temp_dir);
    let db_file = temp_dir.join("test_tasks_verify.db");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "migrate", "run",
            toml_file.to_str().unwrap(),
            "--database", db_file.to_str().unwrap(),
            "--verify"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute migrate run with verify");

    assert_success(&output, "migrate run with verify");
    assert_contains(&output, "verify", "should mention verification");

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires database and migration implementation"]
fn test_migrate_rollback_with_timestamp() {
    let temp_dir = create_temp_dir("migrate_rollback");
    let db_file = temp_dir.join("test_tasks_rollback.db");
    let before_timestamp = current_timestamp();

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "migrate", "rollback",
            "--before", &before_timestamp.to_string(),
            "--database", db_file.to_str().unwrap()
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute migrate rollback");

    // May fail if no migrations exist, but should handle gracefully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Migrate rollback output:\nstdout: {}\nstderr: {}", stdout, stderr);

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires database and migration implementation"]
fn test_migrate_rollback_requires_before() {
    let temp_dir = create_temp_dir("migrate_rollback_requires_before");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "migrate", "rollback"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute migrate rollback");

    assert_failure(&output, "migrate rollback without --before should fail");
    assert_contains(&output, "--before", "should mention required --before parameter");

    cleanup_temp_dir(&temp_dir);
}

#[test]
#[ignore = "Requires database and migration implementation"]
fn test_migrate_verify_database() {
    let temp_dir = create_temp_dir("migrate_verify_db");
    let db_file = temp_dir.join("test_tasks_verify.db");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "migrate", "verify",
            "--database", db_file.to_str().unwrap()
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute migrate verify");

    // Should execute verify (may report no migrations)
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Migrate verify output: {}", stdout);

    cleanup_temp_dir(&temp_dir);
}

/// =============================================================================
/// Environment Variable Tests
/// =============================================================================

#[test]
fn test_ci_environment_detection() {
    set_ci_env_vars();

    let temp_dir = create_temp_dir("ci_env_test");

    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .env("CI", "true")
        .output()
        .expect("Failed to execute command");

    // Version should work in CI environment
    assert_success(&output, "version in CI environment");

    clear_ci_env_vars();
    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_custom_data_directory() {
    let temp_dir = create_temp_dir("custom_data_dir");

    let output = Command::new("cargo")
        .args(["run", "--", "task", "list"])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute command");

    // Should use custom data directory
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Custom data dir output:\nstdout: {}\nstderr: {}", stdout, stderr);

    clear_ci_env_vars();
    cleanup_temp_dir(&temp_dir);
}

/// =============================================================================
/// Error Handling Tests
/// =============================================================================

#[test]
fn test_task_create_missing_required_args() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "create"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "task create without args should fail");
    assert_contains(&output, "required", "should mention required arguments");
}

#[test]
fn test_session_create_missing_required_args() {
    let output = Command::new("cargo")
        .args(["run", "--", "session", "create"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "session create without args should fail");
    assert_contains(&output, "required", "should mention required arguments");
}

#[test]
fn test_engine_scan_missing_directory() {
    let output = Command::new("cargo")
        .args(["run", "--", "engine", "scan"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "engine scan without directory should fail");
    assert_contains(&output, "required", "should mention required arguments");
}

#[test]
fn test_migrate_run_missing_source() {
    let output = Command::new("cargo")
        .args(["run", "--", "migrate", "run"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "migrate run without source should fail");
    assert_contains(&output, "required", "should mention required arguments");
}

#[test]
fn test_invalid_task_type() {
    let temp_dir = create_temp_dir("invalid_task_type");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "create",
            "TEST-INVALID",
            "Invalid Type Test",
            "--type", "invalid_type",
            "--priority", "P0"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "task create with invalid type should fail");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_invalid_task_priority() {
    let temp_dir = create_temp_dir("invalid_task_priority");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "task", "create",
            "TEST-INVALID",
            "Invalid Priority Test",
            "--type", "refactoring",
            "--priority", "INVALID"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "task create with invalid priority should fail");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_invalid_session_status() {
    let temp_dir = create_temp_dir("invalid_session_status");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "session", "list",
            "--status", "invalid_status"
        ])
        .current_dir(cis_node_dir())
        .env("CIS_DATA_DIR", temp_dir.as_path())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "session list with invalid status should fail");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_invalid_engine_type() {
    let temp_dir = create_temp_dir("invalid_engine_type");

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "engine", "scan",
            temp_dir.to_str().unwrap(),
            "--engine-type", "invalid_engine"
        ])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "engine scan with invalid type should fail");

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn test_invalid_report_format() {
    let temp_dir = create_temp_dir("invalid_report_format");
    let scan_result = temp_dir.join("scan.json");
    fs::write(&scan_result, "{}").ok();

    let output = Command::new("cargo")
        .args([
            "run", "--",
            "engine", "report",
            scan_result.to_str().unwrap(),
            "--format", "invalid_format"
        ])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");

    assert_failure(&output, "engine report with invalid format should fail");

    cleanup_temp_dir(&temp_dir);
}

/// =============================================================================
/// Integration Test Cleanup
/// =============================================================================

#[test]
fn test_cleanup_test_artifacts() {
    // This test verifies cleanup logic
    let temp_dir = create_temp_dir("cleanup_test");

    // Create some test artifacts
    let test_file = temp_dir.join("test.txt");
    fs::write(&test_file, "test content").ok();

    assert!(test_file.exists(), "test file should exist");

    // Cleanup
    cleanup_temp_dir(&temp_dir);

    assert!(!temp_dir.exists(), "temp directory should be cleaned up");
}
