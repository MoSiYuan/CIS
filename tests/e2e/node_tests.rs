//! # E2E Tests for `cis node` Command
//!
//! These tests verify the node management functionality:
//! - Listing nodes
//! - Binding/connecting to nodes
//! - Pinging nodes
//! - Inspecting node details
//! - Disconnecting from nodes

use crate::e2e::helpers::TestEnv;
use assert_cmd::Command;
use predicates::prelude::*;

/// Setup: Initialize CIS and return test environment
fn setup_with_init() -> TestEnv {
    let env = TestEnv::new().unwrap();
    env.init_cis().expect("Failed to initialize CIS");
    env
}

/// Test: `cis node --help` shows available subcommands
#[test]
fn test_node_help_shows_subcommands() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("list").or(predicate::str::contains("ls")))
        .stdout(predicate::str::contains("bind"))
        .stdout(predicate::str::contains("inspect"))
        .stdout(predicate::str::contains("disconnect"))
        .stdout(predicate::str::contains("ping"))
        .stdout(predicate::str::contains("sync"))
        .stdout(predicate::str::contains("blacklist"));
}

/// Test: `cis node list` (or `cis node ls`) shows nodes
#[test]
fn test_node_list_shows_nodes() {
    let env = setup_with_init();

    env.cis_cmd().arg("node").arg("list").assert().success();
    // Output may be empty if no nodes are bound
}

/// Test: `cis node ls` alias works
#[test]
fn test_node_ls_alias() {
    let env = setup_with_init();

    env.cis_cmd().arg("node").arg("ls").assert().success();
}

/// Test: `cis node list --all` shows all nodes including offline
#[test]
fn test_node_list_all() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("list")
        .arg("--all")
        .assert()
        .success();
}

/// Test: `cis node list --quiet` only shows IDs
#[test]
fn test_node_list_quiet() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("list")
        .arg("--quiet")
        .assert()
        .success();
}

/// Test: `cis node list --format json` outputs JSON
#[test]
fn test_node_list_format_json() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("list")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("[").or(predicate::str::contains("{")));
}

/// Test: `cis node list --format wide` shows extended info
#[test]
fn test_node_list_format_wide() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("list")
        .arg("--format")
        .arg("wide")
        .assert()
        .success();
}

/// Test: `cis node bind <endpoint>` connects to a node
#[test]
#[ignore = "Requires a running CIS node to bind to"]
fn test_node_bind() {
    let env = setup_with_init();

    // This test requires a running CIS node
    // For local testing, you might start a test node or use a mock endpoint
    let endpoint = "ws://localhost:7677";

    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg(endpoint)
        .assert()
        .success()
        .stdout(predicate::str::contains("bound").or(predicate::str::contains("✅")));
}

/// Test: `cis node bind` with DID option
#[test]
#[ignore = "Requires a running CIS node"]
fn test_node_bind_with_did() {
    let env = setup_with_init();

    let endpoint = "ws://localhost:7677";
    let did = "did:cis:test:123";

    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg(endpoint)
        .arg("--did")
        .arg(did)
        .assert()
        .success();
}

/// Test: `cis node bind` with trust level
#[test]
#[ignore = "Requires a running CIS node"]
fn test_node_bind_with_trust() {
    let env = setup_with_init();

    let endpoint = "ws://localhost:7677";

    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg(endpoint)
        .arg("--trust")
        .arg("full")
        .assert()
        .success();
}

/// Test: `cis node bind` with auto-sync
#[test]
#[ignore = "Requires a running CIS node"]
fn test_node_bind_with_auto_sync() {
    let env = setup_with_init();

    let endpoint = "ws://localhost:7677";

    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg(endpoint)
        .arg("--auto-sync")
        .assert()
        .success();
}

/// Test: `cis node inspect <node-id>` shows node details
#[test]
#[ignore = "Requires a bound node"]
fn test_node_inspect() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("inspect")
        .arg(node_id)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("ID")
                .or(predicate::str::contains("DID"))
                .or(predicate::str::contains("Status")),
        );
}

/// Test: `cis node inspect --format` with template
#[test]
#[ignore = "Requires a bound node"]
fn test_node_inspect_format() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("inspect")
        .arg(node_id)
        .arg("--format")
        .arg("{{.ID}}")
        .assert()
        .success();
}

/// Test: `cis node ping <node-id>` checks connectivity
#[test]
#[ignore = "Requires a bound node"]
fn test_node_ping() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("ping")
        .arg(node_id)
        .assert()
        .success()
        .stdout(predicate::str::contains("OK").or(predicate::str::contains("successful")));
}

/// Test: `cis node ping --count` specifies attempts
#[test]
#[ignore = "Requires a bound node"]
fn test_node_ping_with_count() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("ping")
        .arg(node_id)
        .arg("--count")
        .arg("5")
        .assert()
        .success();
}

/// Test: `cis node ping` for non-existent node fails
#[test]
fn test_node_ping_nonexistent() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("ping")
        .arg("nonexistent-node-12345")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("Error"))
                .or(predicate::str::contains("No such node")),
        );
}

/// Test: `cis node disconnect <node-id>` removes a node
#[test]
#[ignore = "Requires a bound node"]
fn test_node_disconnect() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("disconnect")
        .arg(node_id)
        .arg("--force")
        .assert()
        .success()
        .stdout(predicate::str::contains("Disconnected").or(predicate::str::contains("✓")));
}

/// Test: `cis node disconnect` multiple nodes
#[test]
#[ignore = "Requires bound nodes"]
fn test_node_disconnect_multiple() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("disconnect")
        .arg("node1")
        .arg("node2")
        .arg("node3")
        .arg("--force")
        .assert()
        .success();
}

/// Test: `cis node blacklist <node-id>` blocks a node
#[test]
#[ignore = "Requires a bound node"]
fn test_node_blacklist() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("blacklist")
        .arg(node_id)
        .assert()
        .success()
        .stdout(predicate::str::contains("blacklisted").or(predicate::str::contains("✅")));
}

/// Test: `cis node blacklist` with reason
#[test]
#[ignore = "Requires a bound node"]
fn test_node_blacklist_with_reason() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("blacklist")
        .arg(node_id)
        .arg("--reason")
        .arg("Testing blacklist functionality")
        .assert()
        .success();
}

/// Test: `cis node unblacklist <node-id>` removes from blacklist
#[test]
#[ignore = "Requires a blacklisted node"]
fn test_node_unblacklist() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("unblacklist")
        .arg(node_id)
        .assert()
        .success()
        .stdout(predicate::str::contains("unblacklisted").or(predicate::str::contains("✅")));
}

/// Test: `cis node sync <node-id>` syncs data
#[test]
#[ignore = "Requires a bound node"]
fn test_node_sync() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("sync")
        .arg(node_id)
        .assert()
        .success()
        .stdout(predicate::str::contains("completed").or(predicate::str::contains("✅")));
}

/// Test: `cis node sync --full` performs full sync
#[test]
#[ignore = "Requires a bound node"]
fn test_node_sync_full() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("sync")
        .arg(node_id)
        .arg("--full")
        .assert()
        .success();
}

/// Test: `cis node prune` removes offline nodes
#[test]
#[ignore = "May require specific node state"]
fn test_node_prune() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("prune")
        .arg("--max-offline-days")
        .arg("7")
        .arg("--force")
        .assert()
        .success();
}

/// Test: `cis node stats` shows resource usage
#[test]
#[ignore = "Requires bound nodes"]
fn test_node_stats() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("CPU").or(predicate::str::contains("Memory")));
}

/// Test: `cis node stats <node-id>` for specific node
#[test]
#[ignore = "Requires a bound node"]
fn test_node_stats_specific() {
    let env = setup_with_init();

    let node_id = "test-node-id";

    env.cis_cmd()
        .arg("node")
        .arg("stats")
        .arg(node_id)
        .assert()
        .success();
}

/// Test: Node operations require CIS initialization
#[test]
fn test_node_operations_require_init() {
    let env = TestEnv::new().unwrap();
    // Note: We intentionally don't call init_cis()

    let output = env.cis_cmd().arg("node").arg("list").output().unwrap();

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

/// Test: `cis node list` with filter
#[test]
#[ignore = "Requires nodes with specific properties"]
fn test_node_list_with_filter() {
    let env = setup_with_init();

    env.cis_cmd()
        .arg("node")
        .arg("list")
        .arg("--filter")
        .arg("status=online")
        .assert()
        .success();
}

/// Test: Node bind fails with invalid endpoint
#[test]
fn test_node_bind_invalid_endpoint() {
    let env = setup_with_init();

    // Invalid endpoint format
    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg("not-a-valid-endpoint")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Error")
                .or(predicate::str::contains("invalid"))
                .or(predicate::str::contains("failed")),
        );
}

/// Test: Node bind to unreachable endpoint
#[test]
#[ignore = "May hang or timeout"]
fn test_node_bind_unreachable() {
    let env = setup_with_init();

    // Unreachable endpoint
    env.cis_cmd()
        .arg("node")
        .arg("bind")
        .arg("ws://192.0.2.1:9999") // TEST-NET-1, should be unreachable
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .failure();
}
