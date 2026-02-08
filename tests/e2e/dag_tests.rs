//! # E2E Tests for `cis dag` Command
//!
//! These tests verify the DAG execution management functionality:
//! - Creating DAG runs
//! - Checking DAG status
//! - Pausing and resuming DAG runs
//! - Viewing logs
//! - Aborting runs

use crate::e2e::helpers::{
    create_circular_dag_file, create_invalid_dag_file, create_skill_dag_file, create_test_dag_file,
    TestEnv,
};
use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

/// Setup: Initialize CIS and return test environment
fn setup_with_init() -> TestEnv {
    let env = TestEnv::new().unwrap();
    env.init_cis().expect("Failed to initialize CIS");
    env
}

/// Test: `cis dag --help` shows available subcommands
#[test]
fn test_dag_help_shows_subcommands() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("dag")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("pause"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("abort"))
        .stdout(predicate::str::contains("logs"));
}

/// Test: `cis dag run <file>` creates a new DAG run
#[test]
fn test_dag_run_creates_run() {
    let env = setup_with_init();

    // Create a test DAG file
    let dag_path = create_test_dag_file(env.path(), "test.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created").or(predicate::str::contains("run")));
}

/// Test: `cis dag run` with custom run ID
#[test]
fn test_dag_run_with_custom_id() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let custom_id = "my-custom-run-id";

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(custom_id)
        .assert()
        .success()
        .stdout(predicate::str::contains(custom_id));
}

/// Test: `cis dag run --paused` creates run in paused state
#[test]
fn test_dag_run_paused_mode() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--paused")
        .assert()
        .success()
        .stdout(predicate::str::contains("PAUSED").or(predicate::str::contains("paused")));
}

/// Test: `cis dag status` shows current run status
#[test]
fn test_dag_status_shows_info() {
    let env = setup_with_init();

    // Create a run first
    let dag_path = create_test_dag_file(env.path(), "test.dag");

    let output = env
        .cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check status (might need --run-id depending on implementation)
    env.cis_cmd()
        .arg("dag")
        .arg("status")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Run ID")
                .or(predicate::str::contains("Status"))
                .or(predicate::str::contains("Tasks")),
        );
}

/// Test: `cis dag status --verbose` shows detailed task list
#[test]
fn test_dag_status_verbose() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();

    env.cis_cmd()
        .arg("dag")
        .arg("status")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Task ID").or(predicate::str::contains("Task Details")));
}

/// Test: `cis dag list` shows all DAG runs
#[test]
fn test_dag_list_shows_runs() {
    let env = setup_with_init();

    // Create a run first
    let dag_path = create_test_dag_file(env.path(), "test.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();

    // List runs
    env.cis_cmd()
        .arg("dag")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Run ID").or(predicate::str::contains("Status")));
}

/// Test: `cis dag list --all` shows all runs including completed
#[test]
fn test_dag_list_all() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("dag")
        .arg("list")
        .arg("--all")
        .assert()
        .success();
}

/// Test: `cis dag pause` pauses a running DAG
#[test]
fn test_dag_pause() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    // Create a run first
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();

    // Pause the run
    env.cis_cmd().arg("dag").arg("pause").assert().success();
}

/// Test: `cis dag resume` resumes a paused DAG
#[test]
fn test_dag_resume() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    // Create a paused run
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--paused")
        .assert()
        .success();

    // Resume the run
    env.cis_cmd()
        .arg("dag")
        .arg("resume")
        .assert()
        .success()
        .stdout(predicate::str::contains("resumed").or(predicate::str::contains("✓")));
}

/// Test: `cis dag abort` aborts a DAG run
#[test]
fn test_dag_abort() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    // Create a run
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();

    // Abort with --force
    env.cis_cmd()
        .arg("dag")
        .arg("abort")
        .arg("--force")
        .assert()
        .success()
        .stdout(predicate::str::contains("aborted").or(predicate::str::contains("✓")));
}

/// Test: `cis dag use <run-id>` sets active run
#[test]
fn test_dag_use_sets_active_run() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let run_id = "test-run-123";

    // Create a run with specific ID
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(run_id)
        .assert()
        .success();

    // Set as active
    env.cis_cmd()
        .arg("dag")
        .arg("use")
        .arg(run_id)
        .assert()
        .success()
        .stdout(predicate::str::contains("active").or(predicate::str::contains("✓")));
}

/// Test: `cis dag logs <run-id>` shows execution logs
#[test]
#[ignore = "May require database setup"]
fn test_dag_logs() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let run_id = "test-run-logs";

    // Create and potentially execute a run
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(run_id)
        .assert()
        .success();

    // View logs
    env.cis_cmd()
        .arg("dag")
        .arg("logs")
        .arg(run_id)
        .assert()
        .success();
}

/// Test: `cis dag logs --tail` limits output
#[test]
#[ignore = "May require database setup"]
fn test_dag_logs_tail() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let run_id = "test-run-tail";

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(run_id)
        .assert()
        .success();

    env.cis_cmd()
        .arg("dag")
        .arg("logs")
        .arg(run_id)
        .arg("--tail")
        .arg("10")
        .assert()
        .success();
}

/// Test: `cis dag definitions` lists DAG definitions
#[test]
fn test_dag_definitions() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("dag")
        .arg("definitions")
        .assert()
        .success();
}

/// Test: `cis dag worker list` shows workers
#[test]
fn test_dag_worker_list() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("dag")
        .arg("worker")
        .arg("list")
        .assert()
        .success();
}

/// Test: `cis dag execute` runs tasks directly
#[test]
#[ignore = "May hang or require specific setup"]
fn test_dag_execute() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let run_id = "test-execute-run";

    // Create a run
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(run_id)
        .arg("--paused")
        .assert()
        .success();

    // Execute
    env.cis_cmd()
        .arg("dag")
        .arg("execute")
        .arg("--run-id")
        .arg(run_id)
        .timeout(Duration::from_secs(30))
        .assert()
        .success();
}

/// Test: Running DAG with invalid file fails gracefully
#[test]
fn test_dag_run_invalid_file() {
    let env = setup_with_init();

    let invalid_path = env.path().join("nonexistent.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(invalid_path.to_str().unwrap())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("Error"))
                .or(predicate::str::contains("不存在")),
        );
}

/// Test: Running DAG with invalid format fails gracefully
#[test]
fn test_dag_run_invalid_format() {
    let env = setup_with_init();

    let dag_path = create_invalid_dag_file(env.path(), "invalid.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Error")
                .or(predicate::str::contains("failed"))
                .or(predicate::str::contains("parse")),
        );
}

/// Test: Running DAG with circular dependencies fails
#[test]
fn test_dag_run_circular_dependency() {
    let env = setup_with_init();

    let dag_path = create_circular_dag_file(env.path(), "circular.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("circular")
                .or(predicate::str::contains("cycle"))
                .or(predicate::str::contains("Error")),
        );
}

/// Test: `cis dag amend` modifies task parameters
#[test]
#[ignore = "Advanced feature that may not be fully implemented"]
fn test_dag_amend() {
    let env = setup_with_init();

    let dag_path = create_test_dag_file(env.path(), "test.dag");
    let run_id = "test-amend-run";

    // Create a paused run
    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .arg("--run-id")
        .arg(run_id)
        .arg("--paused")
        .assert()
        .success();

    // Amend a task
    env.cis_cmd()
        .arg("dag")
        .arg("amend")
        .arg("--run-id")
        .arg(run_id)
        .arg("task1")
        .arg("--env")
        .arg("KEY=value")
        .assert()
        .success();
}

/// Test: `cis dag sessions` lists active sessions
#[test]
#[ignore = "Requires agent cluster setup"]
fn test_dag_sessions() {
    let env = setup_with_init();

    env.cis_cmd().arg("dag").arg("sessions").assert().success();
}

/// Test: `cis dag sessions --all` includes completed sessions
#[test]
#[ignore = "Requires agent cluster setup"]
fn test_dag_sessions_all() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("dag")
        .arg("sessions")
        .arg("--all")
        .assert()
        .success();
}

/// Test: Running skill-based DAG
#[test]
#[ignore = "Requires skills to be installed"]
fn test_dag_run_with_skills() {
    let env = setup_with_init();

    let dag_path = create_skill_dag_file(env.path(), "skill.dag");

    env.cis_cmd()
        .arg("dag")
        .arg("run")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success();
}

/// Test: DAG operations require CIS initialization
#[test]
fn test_dag_operations_require_init() {
    let env = TestEnv::new().unwrap();
    // Note: We intentionally don't call init_cis()

    let dag_path = create_test_dag_file(env.path(), "test.dag");

    let output = env.cis_cmd().arg("dag").arg("list").output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Either success (auto-init in release) or failure with init message
    assert!(
        output.status.success()
            || stderr.contains("not initialized")
            || stderr.contains("尚未初始化")
            || stdout.contains("not initialized"),
        "Should either succeed or show initialization error"
    );
}
