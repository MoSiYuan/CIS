//! Agent Teams 真实集成测试
//!
//! 这些测试验证与真实 AI 工具的集成：
//! - Claude Code CLI (通过 PTY)
//! - OpenCode CLI (通过 HTTP serve)
//!
//! 运行方式:
//! ```bash
//! # 运行所有集成测试（需要安装 claude 和 opencode）
//! cargo test --release -p cis-core --test agent_teams_integration_test -- --ignored
//!
//! # 只运行 Claude 测试
//! cargo test --release -p cis-core --test agent_teams_integration_test claude -- --ignored
//!
//! # 只运行 OpenCode 测试
//! cargo test --release -p cis-core --test agent_teams_integration_test opencode -- --ignored
//! ```

use cis_core::agent::persistent::{
    AgentConfig, AgentPool, AgentAcquireConfig, PoolConfig, AgentRuntime, 
    RuntimeType, TaskRequest, PersistentAgent,
};
use cis_core::agent::persistent::claude::ClaudeRuntime;
use cis_core::agent::persistent::opencode::OpenCodePersistentAgent;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use std::sync::Arc;

/// 检查 Claude CLI 是否可用
async fn check_claude_available() -> bool {
    match tokio::process::Command::new("claude")
        .arg("--version")
        .output()
        .await
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// 检查 OpenCode CLI 是否可用
async fn check_opencode_available() -> bool {
    match tokio::process::Command::new("opencode")
        .arg("--version")
        .output()
        .await
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// 获取临时工作目录
fn get_test_work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cis-agent-test-{}", name));
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// 清理测试目录
fn cleanup_test_dir(dir: &PathBuf) {
    std::fs::remove_dir_all(dir).ok();
}

// =============================================================================
// Claude 真实集成测试
// =============================================================================

#[tokio::test]
#[ignore = "Requires claude CLI to be installed"]
async fn test_claude_real_agent_lifecycle() {
    // 检查工具可用性
    if !check_claude_available().await {
        eprintln!("⚠️  Claude CLI not available, skipping test");
        return;
    }

    let work_dir = get_test_work_dir("claude-lifecycle");
    let config = AgentConfig::new("test-claude-real", work_dir.clone());

    let runtime = ClaudeRuntime::new();
    
    println!("🚀 Creating Claude agent...");
    let agent = runtime
        .create_agent(config)
        .await
        .expect("Failed to create Claude agent");

    // 验证初始状态
    let status = agent.status().await;
    assert!(status.is_available(), "Agent should be available after creation");
    println!("✅ Agent created and available");

    // 执行简单任务
    println!("📝 Executing task...");
    let task = TaskRequest::new("test-1", "Echo 'Hello from Claude test'")
        .with_context("system_prompt", "You are a test assistant. Respond briefly.");

    let result = timeout(Duration::from_secs(60), agent.execute(task))
        .await
        .expect("Task timed out")
        .expect("Task execution failed");

    assert!(result.success, "Task should succeed");
    assert!(result.output.is_some(), "Task should have output");
    println!("✅ Task completed: {:?}", result.output);

    // 执行多个任务验证持久化
    println!("📝 Executing second task...");
    let task2 = TaskRequest::new("test-2", "What is 2+2? Answer with just the number.");
    
    let result2 = timeout(Duration::from_secs(60), agent.execute(task2))
        .await
        .expect("Task 2 timed out")
        .expect("Task 2 execution failed");
    
    assert!(result2.success);
    println!("✅ Second task completed");

    // 关闭 agent
    println!("🔌 Shutting down agent...");
    agent.shutdown().await.expect("Failed to shutdown agent");

    let status = agent.status().await;
    assert!(!status.is_available(), "Agent should not be available after shutdown");
    println!("✅ Agent shutdown complete");

    // 清理
    cleanup_test_dir(&work_dir);
}

#[tokio::test]
#[ignore = "Requires claude CLI to be installed"]
async fn test_claude_real_task_with_file() {
    if !check_claude_available().await {
        eprintln!("⚠️  Claude CLI not available, skipping test");
        return;
    }

    let work_dir = get_test_work_dir("claude-file");
    
    // 创建一个测试文件
    let test_file = work_dir.join("test.txt");
    std::fs::write(&test_file, "Hello from test file!").unwrap();

    let config = AgentConfig::new("test-claude-file", work_dir.clone());
    let runtime = ClaudeRuntime::new();

    println!("🚀 Creating Claude agent for file test...");
    let agent = runtime
        .create_agent(config)
        .await
        .expect("Failed to create agent");

    // 执行涉及文件的任务
    println!("📝 Executing file-related task...");
    let task = TaskRequest::new(
        "file-task",
        "Read the file test.txt and echo its contents",
    );

    let result = timeout(Duration::from_secs(60), agent.execute(task))
        .await
        .expect("Task timed out")
        .expect("Task failed");

    assert!(result.success);
    let output = result.output.unwrap_or_default();
    assert!(
        output.contains("Hello from test file") || output.contains("test file"),
        "Output should contain file content hint: {}",
        output
    );
    println!("✅ File task completed: {}", output);

    agent.shutdown().await.ok();
    cleanup_test_dir(&work_dir);
}

// =============================================================================
// OpenCode 真实集成测试
// =============================================================================

#[tokio::test]
#[ignore = "Requires opencode CLI to be installed"]
async fn test_opencode_real_agent_lifecycle() {
    // 检查工具可用性
    if !check_opencode_available().await {
        eprintln!("⚠️  OpenCode CLI not available, skipping test");
        return;
    }

    let work_dir = get_test_work_dir("opencode-lifecycle");
    let config = AgentConfig::new("test-opencode-real", work_dir.clone());

    println!("🚀 Starting OpenCode agent...");
    let agent = timeout(
        Duration::from_secs(30),
        OpenCodePersistentAgent::start(config)
    )
    .await
    .expect("Agent startup timed out")
    .expect("Failed to start OpenCode agent");

    // 验证状态
    let status = agent.status().await;
    assert!(status.is_available(), "Agent should be available");
    assert!(agent.is_local().await, "Agent should be local");
    println!("✅ OpenCode agent started and available");

    // 执行简单任务
    println!("📝 Executing task...");
    let task = TaskRequest::new("test-1", "Say 'Hello from OpenCode test'")
        .with_context("system_prompt", "You are a helpful assistant.");

    let result = timeout(Duration::from_secs(60), agent.execute(task))
        .await
        .expect("Task timed out")
        .expect("Task execution failed");

    assert!(result.success, "Task should succeed");
    assert!(result.output.is_some(), "Task should have output");
    println!("✅ Task completed: {:?}", result.output);

    // 执行代码相关任务
    println!("📝 Executing code task...");
    let code_task = TaskRequest::new(
        "code-task",
        "Write a Python one-liner that prints 'Hello World'",
    );

    let result2 = timeout(Duration::from_secs(60), agent.execute(code_task))
        .await
        .expect("Code task timed out")
        .expect("Code task failed");

    assert!(result2.success);
    let output = result2.output.unwrap_or_default();
    assert!(
        output.contains("print") || output.contains("Hello"),
        "Output should contain code: {}",
        output
    );
    println!("✅ Code task completed: {}", output);

    // 检查统计
    let (total, _last_activity) = agent.stats().await;
    assert_eq!(total, 2, "Should have 2 total tasks");
    println!("✅ Stats verified: {} tasks", total);

    // 关闭
    println!("🔌 Shutting down agent...");
    agent.shutdown().await.expect("Failed to shutdown");

    let status = agent.status().await;
    assert!(!status.is_available(), "Agent should be unavailable after shutdown");
    println!("✅ Shutdown complete");

    cleanup_test_dir(&work_dir);
}

#[tokio::test]
#[ignore = "Requires opencode CLI to be installed"]
async fn test_opencode_real_multiple_tasks() {
    if !check_opencode_available().await {
        eprintln!("⚠️  OpenCode CLI not available, skipping test");
        return;
    }

    let work_dir = get_test_work_dir("opencode-multi");
    let config = AgentConfig::new("test-opencode-multi", work_dir.clone());

    println!("🚀 Starting OpenCode agent for multi-task test...");
    let agent = OpenCodePersistentAgent::start(config)
        .await
        .expect("Failed to start agent");

    // 执行多个任务
    let tasks = vec![
        ("task-1", "What is the capital of France?"),
        ("task-2", "What is 15 * 7?"),
        ("task-3", "List three primary colors"),
    ];

    for (id, prompt) in tasks {
        println!("📝 Executing {}...", id);
        let task = TaskRequest::new(id, prompt);
        
        let result = timeout(Duration::from_secs(60), agent.execute(task))
            .await
            .unwrap_or_else(|_| panic!("{} timed out", id))
            .unwrap_or_else(|e| panic!("{} failed: {:?}", id, e));

        assert!(result.success, "{} should succeed", id);
        println!("✅ {} completed", id);
    }

    // 验证统计
    let (total, _last_activity) = agent.stats().await;
    assert_eq!(total, 3, "Should have 3 tasks");

    agent.shutdown().await.ok();
    cleanup_test_dir(&work_dir);
}

// =============================================================================
// Agent Pool 真实集成测试
// =============================================================================

#[tokio::test]
#[ignore = "Requires claude or opencode CLI to be installed"]
async fn test_agent_pool_real_operations() {
    let claude_available = check_claude_available().await;
    let opencode_available = check_opencode_available().await;

    if !claude_available && !opencode_available {
        eprintln!("⚠️  No AI tools available, skipping test");
        return;
    }

    println!("📋 Tool availability: Claude={}, OpenCode={}", claude_available, opencode_available);

    let work_dir = get_test_work_dir("pool-real");
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    // 注册可用的运行时
    if claude_available {
        pool.register_runtime(Arc::new(ClaudeRuntime::new())).await.expect("Failed to register runtime");
        println!("✅ Registered Claude runtime");
    }

    // 获取 agent
    println!("🔍 Acquiring agent from pool...");
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(AgentConfig::new("pool-test", work_dir.clone()));
    
    let handle = timeout(
        Duration::from_secs(30),
        pool.acquire(config)
    )
    .await
    .expect("Acquire timed out")
    .expect("Failed to acquire agent");

    let agent_id = handle.agent_id().to_string();
    println!("✅ Acquired agent: {}", agent_id);

    // 执行任务
    println!("📝 Executing task through pool agent...");
    let task = TaskRequest::new("pool-task", "Say 'Hello from Agent Pool'");
    
    let result = timeout(Duration::from_secs(60), handle.execute(task))
        .await
        .expect("Task timed out")
        .expect("Task failed");

    assert!(result.success, "Pool task should succeed");
    println!("✅ Pool task completed");

    // 释放回 pool (keep=true 表示保留)
    println!("♻️  Releasing agent back to pool...");
    pool.release(handle, true).await.expect("Failed to release agent");
    println!("✅ Agent released");

    // 验证 pool 中有可用 agent
    let agents = pool.list().await;
    assert!(!agents.is_empty(), "Pool should have agents");
    println!("✅ Pool has {} agents", agents.len());

    // 复用 agent
    println!("🔄 Reusing agent from pool...");
    let config2 = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_reuse_agent_id(agent_id.clone());
    
    let reused = timeout(
        Duration::from_secs(10),
        pool.acquire(config2)
    )
    .await
    .expect("Reuse acquire timed out")
    .expect("Failed to reuse agent");

    assert_eq!(reused.agent_id(), agent_id, "Should reuse same agent");
    println!("✅ Reused same agent: {}", reused.agent_id());

    // 第二个任务
    let task2 = TaskRequest::new("pool-task-2", "What is 2+2?");
    let result2 = timeout(Duration::from_secs(60), reused.execute(task2))
        .await
        .expect("Task 2 timed out")
        .expect("Task 2 failed");

    assert!(result2.success);
    println!("✅ Second task completed");

    // 清理
    pool.release(reused, false).await.ok(); // keep=false 表示关闭 agent
    pool.shutdown_all().await.expect("Failed to shutdown pool");
    cleanup_test_dir(&work_dir);
    println!("✅ Pool shutdown complete");
}

// =============================================================================
// 混合运行时测试
// =============================================================================

#[tokio::test]
#[ignore = "Requires both claude and opencode CLI to be installed"]
async fn test_mixed_runtime_real() {
    let claude_available = check_claude_available().await;
    let opencode_available = check_opencode_available().await;

    if !claude_available || !opencode_available {
        eprintln!("⚠️  Both tools required for this test, skipping");
        return;
    }

    println!("🚀 Testing mixed runtimes with real tools...");

    let work_dir = get_test_work_dir("mixed-real");
    let pool = AgentPool::new(PoolConfig {
        max_agents: 10,
        ..Default::default()
    });

    // 注册两个运行时
    pool.register_runtime(Arc::new(ClaudeRuntime::new())).await.expect("Failed to register Claude");
    pool.register_runtime(Arc::new(cis_core::agent::persistent::opencode::OpenCodeRuntime)).await.expect("Failed to register OpenCode");
    println!("✅ Registered both runtimes");

    // 获取 Claude agent
    let claude_dir = work_dir.join("claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    
    println!("🔍 Acquiring Claude agent...");
    let claude_config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(AgentConfig::new("claude-mixed", claude_dir));
    let claude_handle = pool.acquire(claude_config).await.expect("Failed to acquire Claude");
    
    let claude_id = claude_handle.agent_id().to_string();
    println!("✅ Got Claude agent: {}", claude_id);

    // 获取 OpenCode agent
    let opencode_dir = work_dir.join("opencode");
    std::fs::create_dir_all(&opencode_dir).unwrap();
    
    println!("🔍 Acquiring OpenCode agent...");
    let opencode_config = AgentAcquireConfig::new(RuntimeType::OpenCode)
        .with_agent_config(AgentConfig::new("opencode-mixed", opencode_dir));
    let opencode_handle = pool.acquire(opencode_config).await.expect("Failed to acquire OpenCode");
    
    let opencode_id = opencode_handle.agent_id().to_string();
    println!("✅ Got OpenCode agent: {}", opencode_id);

    // 并行执行不同运行时的任务
    println!("📝 Executing parallel tasks...");
    
    let claude_task = TaskRequest::new("claude-task", "Write a haiku about coding");
    let opencode_task = TaskRequest::new("opencode-task", "List three programming languages");

    let (claude_result, opencode_result) = tokio::join!(
        claude_handle.execute(claude_task),
        opencode_handle.execute(opencode_task)
    );

    let claude_result = claude_result.expect("Claude task failed");
    let opencode_result = opencode_result.expect("OpenCode task failed");

    assert!(claude_result.success, "Claude task should succeed");
    assert!(opencode_result.success, "OpenCode task should succeed");

    println!("✅ Claude output: {:?}", claude_result.output);
    println!("✅ OpenCode output: {:?}", opencode_result.output);

    // 清理
    pool.release(claude_handle, false).await.ok();
    pool.release(opencode_handle, false).await.ok();
    pool.shutdown_all().await.ok();
    cleanup_test_dir(&work_dir);

    println!("✅ Mixed runtime test complete");
}

// 为 AgentAcquireConfig 添加辅助方法
trait AgentAcquireConfigExt {
    fn with_agent_config(self, config: AgentConfig) -> Self;
    fn with_reuse_agent_id(self, id: String) -> Self;
}

impl AgentAcquireConfigExt for AgentAcquireConfig {
    fn with_agent_config(mut self, config: AgentConfig) -> Self {
        self.agent_config = Some(config);
        self
    }
    
    fn with_reuse_agent_id(mut self, id: String) -> Self {
        self.reuse_agent_id = Some(id);
        self
    }
}
