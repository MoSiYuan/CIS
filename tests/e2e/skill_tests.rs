//! # E2E Tests for `cis skill` Command
//!
//! These tests verify the skill management functionality:
//! - Listing skills
//! - Loading/unloading skills
//! - Activating/deactivating skills
//! - Installing skills
//! - Semantic skill invocation (do command)

use crate::e2e::helpers::TestEnv;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

/// Setup: Initialize CIS and return test environment
fn setup_with_init() -> TestEnv {
    let env = TestEnv::new().unwrap();
    env.init_cis().expect("Failed to initialize CIS");
    env
}

/// Test: `cis skill --help` shows available subcommands
#[test]
fn test_skill_help_shows_subcommands() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("load"))
        .stdout(predicate::str::contains("unload"))
        .stdout(predicate::str::contains("activate"))
        .stdout(predicate::str::contains("deactivate"))
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("do"));
}

/// Test: `cis skill list` shows registered skills
#[test]
fn test_skill_list_shows_skills() {
    let env = setup_with_init();

    env.cis_cmd().arg("skill").arg("list").assert().success();
    // Output may be empty or contain skills depending on setup
}

/// Test: `cis skill list` when no skills are registered
#[test]
fn test_skill_list_empty_when_no_skills() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No skills").or(predicate::str::is_empty()));
}

/// Test: `cis skill info <skill>` shows skill information
#[test]
#[ignore = "Requires a skill to be installed"]
fn test_skill_info_shows_details() {
    let env = setup_with_init();

    // This test requires a skill to be installed first
    env.cis_cmd()
        .arg("skill")
        .arg("info")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("Name"))
        .stdout(predicate::str::contains("Version"));
}

/// Test: `cis skill info` for non-existent skill
#[test]
fn test_skill_info_nonexistent_skill() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("info")
        .arg("nonexistent-skill-12345")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Error")));
}

/// Test: `cis skill load <skill>` loads a skill
#[test]
#[ignore = "Requires a skill to be installed"]
fn test_skill_load_activates_skill() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("loaded").or(predicate::str::contains("✅")));
}

/// Test: `cis skill load --activate <skill>` loads and activates
#[test]
#[ignore = "Requires a skill to be installed"]
fn test_skill_load_with_activate() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("init-wizard")
        .arg("--activate")
        .assert()
        .success()
        .stdout(predicate::str::contains("activated").or(predicate::str::contains("✅")));
}

/// Test: `cis skill unload <skill>` unloads a skill
#[test]
#[ignore = "Requires a skill to be loaded"]
fn test_skill_unload() {
    let env = setup_with_init();

    // First load the skill
    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("init-wizard")
        .assert()
        .success();

    // Then unload it
    env.cis_cmd()
        .arg("skill")
        .arg("unload")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("unloaded").or(predicate::str::contains("✅")));
}

/// Test: `cis skill activate <skill>` activates a loaded skill
#[test]
#[ignore = "Requires a skill to be loaded"]
fn test_skill_activate() {
    let env = setup_with_init();

    // Load without activate
    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("init-wizard")
        .assert()
        .success();

    // Activate
    env.cis_cmd()
        .arg("skill")
        .arg("activate")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("activated").or(predicate::str::contains("✅")));
}

/// Test: `cis skill deactivate <skill>` deactivates a skill
#[test]
#[ignore = "Requires a skill to be activated"]
fn test_skill_deactivate() {
    let env = setup_with_init();

    // Load and activate
    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("--activate")
        .arg("init-wizard")
        .assert()
        .success();

    // Deactivate
    env.cis_cmd()
        .arg("skill")
        .arg("deactivate")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("deactivated").or(predicate::str::contains("✅")));
}

/// Test: `cis skill do <description>` performs semantic skill invocation
#[test]
#[ignore = "Requires vector database and skills to be set up"]
fn test_skill_do_semantic_invocation() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("do")
        .arg("initialize a new project")
        .assert()
        .success();
}

/// Test: `cis skill do --candidates` shows candidate skills
#[test]
#[ignore = "Requires vector database and skills to be set up"]
fn test_skill_do_with_candidates() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("do")
        .arg("initialize a new project")
        .arg("--candidates")
        .assert()
        .success()
        .stdout(predicate::str::contains("candidate").or(predicate::str::contains("候选")));
}

/// Test: `cis skill chain` discovers and executes skill chains
#[test]
#[ignore = "Requires vector database and skills to be set up"]
fn test_skill_chain() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("chain")
        .arg("initialize and configure project")
        .assert()
        .success();
}

/// Test: `cis skill chain --preview` shows chain without executing
#[test]
#[ignore = "Requires vector database and skills to be set up"]
fn test_skill_chain_preview() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("skill")
        .arg("chain")
        .arg("initialize and configure project")
        .arg("--preview")
        .assert()
        .success()
        .stdout(predicate::str::contains("preview").or(predicate::str::contains("Preview")));
}

/// Test: `cis skill call <skill> --method <method>` calls a skill method
#[test]
#[ignore = "Requires a loaded skill"]
fn test_skill_call_method() {
    let env = setup_with_init();

    // Load skill first
    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("--activate")
        .arg("init-wizard")
        .assert()
        .success();

    // Call method
    env.cis_cmd()
        .arg("skill")
        .arg("call")
        .arg("init-wizard")
        .arg("--method")
        .arg("help")
        .assert()
        .success();
}

/// Test: `cis skill call` with arguments
#[test]
#[ignore = "Requires a loaded skill"]
fn test_skill_call_with_args() {
    let env = setup_with_init();

    // Load skill first
    env.cis_cmd()
        .arg("skill")
        .arg("load")
        .arg("--activate")
        .arg("init-wizard")
        .assert()
        .success();

    // Call method with args
    env.cis_cmd()
        .arg("skill")
        .arg("call")
        .arg("init-wizard")
        .arg("--method")
        .arg("run")
        .arg("--args")
        .arg(r#"{"project": "test"}"#)
        .assert()
        .success();
}

/// Test: `cis skill remove <skill>` removes a skill
#[test]
#[ignore = "Requires a skill to be installed"]
fn test_skill_remove() {
    let env = setup_with_init();

    // First ensure skill is not loaded
    let _ = env
        .cis_cmd()
        .arg("skill")
        .arg("unload")
        .arg("init-wizard")
        .output();

    // Remove skill
    env.cis_cmd()
        .arg("skill")
        .arg("remove")
        .arg("init-wizard")
        .assert()
        .success()
        .stdout(predicate::str::contains("removed").or(predicate::str::contains("✅")));
}

/// Test: `cis skill install <path>` installs a skill from path
#[test]
#[ignore = "Requires a skill package to install"]
fn test_skill_install_from_path() {
    let env = setup_with_init();

    // Create a mock skill directory or use existing
    let skill_path = env.path().join("test-skill");
    fs::create_dir_all(&skill_path).unwrap();

    // Create a minimal skill manifest
    let manifest = r#"
[skill]
name = "test-skill"
version = "0.1.0"
description = "Test skill for E2E tests"
"#;
    fs::write(skill_path.join("skill.toml"), manifest).unwrap();

    env.cis_cmd()
        .arg("skill")
        .arg("install")
        .arg(skill_path.to_str().unwrap())
        .assert()
        .success();
}

/// Test: `cis skill install` with DAG file
#[test]
#[ignore = "Requires a valid DAG skill file"]
fn test_skill_install_dag_file() {
    let env = setup_with_init();

    // Create a DAG skill file
    let dag_content = r#"
[skill]
name = "test-dag-skill"
version = "0.1.0"
description = "Test DAG skill"

[[dag.tasks]]
id = "task1"
command = "echo 'test'"
"#;
    let dag_path = env.path().join("test-skill.toml");
    fs::write(&dag_path, dag_content).unwrap();

    env.cis_cmd()
        .arg("skill")
        .arg("install")
        .arg(dag_path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("installed").or(predicate::str::contains("✅")));
}

/// Test: Skill operations fail gracefully when CIS is not initialized
#[test]
fn test_skill_operations_fail_without_init() {
    let env = TestEnv::new().unwrap();
    // Note: We intentionally don't call init_cis()

    // Some commands might auto-initialize in release mode
    // This test verifies the behavior in development mode
    let output = env.cis_cmd().arg("skill").arg("list").output().unwrap();

    // In development mode, should fail with initialization message
    // In release mode, might auto-initialize
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Either success (auto-init) or failure with init message
    assert!(
        output.status.success()
            || stderr.contains("not initialized")
            || stderr.contains("尚未初始化")
            || stdout.contains("not initialized"),
        "Should either succeed or show initialization error"
    );
}
