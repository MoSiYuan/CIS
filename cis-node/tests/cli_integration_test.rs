//! # CLI Integration Tests
//!
//! CIS Node CLI 集成测试

use std::process::Command;
use std::path::PathBuf;

/// 获取项目根目录
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Failed to get parent directory")
        .to_path_buf()
}

/// 获取 cis-node 目录
fn cis_node_dir() -> PathBuf {
    project_root().join("cis-node")
}

/// 测试 CLI skill do 命令帮助
#[test]
fn test_cli_skill_do_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "skill", "do", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // 检查输出是否包含帮助信息
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("description") || combined.contains("DESCRIPTION") || 
        combined.contains("自然语言") || combined.contains("自然語言"),
        "Help should mention description argument or natural language. Output: {}", combined
    );
}

/// 测试 CLI memory search 命令帮助
#[test]
fn test_cli_memory_search_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "memory", "vector-search", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // 检查输出是否包含帮助信息
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("query") || combined.contains("QUERY") || 
        combined.contains("search") || combined.contains("Search"),
        "Help should mention query or search. Output: {}", combined
    );
}

/// 测试 CLI 主帮助
#[test]
fn test_cli_main_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 检查主要子命令是否存在
    assert!(combined.contains("skill") || combined.contains("Skill"), 
            "Help should mention 'skill' command");
    assert!(combined.contains("memory") || combined.contains("Memory"), 
            "Help should mention 'memory' command");
}

/// 测试 CLI skill 子命令帮助
#[test]
fn test_cli_skill_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "skill", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 检查子命令
    assert!(combined.contains("list") || combined.contains("List"), 
            "Should mention 'list' subcommand");
    assert!(combined.contains("do") || combined.contains("Do"), 
            "Should mention 'do' subcommand");
}

/// 测试 CLI memory 子命令帮助
#[test]
fn test_cli_memory_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "memory", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 检查子命令
    assert!(combined.contains("get") || combined.contains("Get"), 
            "Should mention 'get' subcommand");
    assert!(combined.contains("set") || combined.contains("Set"), 
            "Should mention 'set' subcommand");
    assert!(combined.contains("search") || combined.contains("Search"), 
            "Should mention 'search' subcommand");
}

/// 测试 CLI version 命令
#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 应该包含版本信息
    assert!(combined.contains("0.1.0") || combined.contains("version"), 
            "Should show version info");
}

/// 测试 CLI status 命令（可能需要 CIS 初始化）
#[test]
#[ignore = "Requires CIS initialization, run manually"]
fn test_cli_status() {
    let output = Command::new("cargo")
        .args(["run", "--", "status"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    // 即使失败也应该有输出
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    println!("stdout: {}", stdout);
    println!("stderr: {}", stderr);
    
    // 检查输出是否包含状态相关信息
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("CIS") || combined.contains("cis") || combined.contains("Status"),
        "Should show CIS status"
    );
}

/// 测试 CLI doctor 命令帮助
#[test]
fn test_cli_doctor_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "doctor", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 应该包含帮助信息
    assert!(
        combined.contains("fix") || combined.contains("check") || combined.contains("help"),
        "Should show doctor help"
    );
}

/// 测试 CLI task 子命令帮助
#[test]
fn test_cli_task_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "task", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 检查子命令
    assert!(combined.contains("list") || combined.contains("List"), 
            "Should mention 'list' subcommand");
    assert!(combined.contains("create") || combined.contains("Create"), 
            "Should mention 'create' subcommand");
}

/// 测试 CLI peer 子命令帮助
#[test]
fn test_cli_peer_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "peer", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 检查子命令
    assert!(combined.contains("list") || combined.contains("List"), 
            "Should mention 'list' subcommand");
    assert!(combined.contains("add") || combined.contains("Add"), 
            "Should mention 'add' subcommand");
}

/// 测试 CLI init 命令帮助
#[test]
fn test_cli_init_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "init", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 应该包含项目相关的帮助信息
    assert!(
        combined.contains("project") || combined.contains("force") || combined.contains("help"),
        "Should show init help with project option"
    );
}

/// 测试 CLI agent 命令帮助
#[test]
fn test_cli_agent_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "agent", "--help"])
        .current_dir(cis_node_dir())
        .output()
        .expect("Failed to execute command");
    
    assert!(output.status.success(), "Command should exit successfully");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    
    // 应该包含帮助信息
    assert!(
        combined.contains("chat") || combined.contains("list") || combined.contains("prompt"),
        "Should show agent help"
    );
}
