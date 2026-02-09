//! # Claude Persistent Agent 使用示例
//!
//! 这个示例展示了如何使用 ClaudePersistentAgent 来创建和管理持久化的 Claude Code Agent。
//!
//! ## 运行示例
//!
//! ```bash
//! cargo run --example claude_persistent_example
//! ```

use std::path::PathBuf;

use cis_core::agent::persistent::{
    claude::{ClaudePersistentAgent, ClaudeRuntime},
    AgentConfig, AgentRuntime, TaskRequest, PersistentAgent,
};
use cis_core::agent::cluster::SessionManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== Claude Persistent Agent Example ===\n");

    // 示例 1: 使用 Runtime 创建 Agent
    println!("1. Creating Claude Agent via Runtime...");
    let runtime = ClaudeRuntime::new();
    
    let config = AgentConfig::new(
        "example-claude-agent",
        PathBuf::from("/tmp/claude-example"),
    )
    .with_system_prompt("You are a helpful coding assistant.")
    .with_timeout(300);

    // 注意：这里会实际启动 claude 进程，需要 claude 命令已安装
    // 如果没有安装，会返回错误
    match runtime.create_agent(config).await {
        Ok(agent) => {
            println!("   ✓ Agent created: {}", agent.agent_id());
            println!("   ✓ Runtime type: {:?}", agent.runtime_type());

            // 检查状态
            let status = agent.status().await;
            println!("   ✓ Agent status: {:?}", status);

            // 执行任务
            println!("\n2. Executing a task...");
            let task = TaskRequest::new("example-task-1", "Write a hello world program in Rust")
                .with_context("language", "rust");

            match agent.execute(task).await {
                Ok(result) => {
                    println!("   ✓ Task completed in {}ms", result.duration_ms);
                    if let Some(output) = &result.output {
                        println!("   Output preview: {}...", &output[..output.len().min(100)]);
                    }
                }
                Err(e) => {
                    println!("   ✗ Task failed: {}", e);
                }
            }

            // 优雅关闭
            println!("\n3. Shutting down agent...");
            agent.shutdown().await?;
            println!("   ✓ Agent shutdown complete");
        }
        Err(e) => {
            println!("   ✗ Failed to create agent: {}", e);
            println!("   Note: Make sure 'claude' command is installed and available in PATH");
        }
    }

    // 示例 2: 直接创建 Agent（更底层的方式）
    println!("\n=== Direct Agent Creation Example ===\n");
    
    let session_manager = SessionManager::global();
    
    let config = AgentConfig::new(
        "direct-claude-agent",
        PathBuf::from("/tmp/claude-direct"),
    );

    match ClaudePersistentAgent::start(session_manager, config).await {
        Ok(agent) => {
            println!("   ✓ Agent created directly");
            
            // 连接到已有的 session
            let session_id = agent.agent_id().to_string();
            println!("   Session ID: {}", session_id);
            
            // 这里可以使用 attach_to_session 重新连接到已有的 session
            // let attached = ClaudePersistentAgent::attach_to_session(
            //     session_manager,
            //     session_id.parse()?
            // ).await?;
            
            agent.shutdown().await?;
        }
        Err(e) => {
            println!("   ✗ Failed to create agent: {}", e);
        }
    }

    // 示例 3: 列出所有 Claude Agents
    println!("\n=== List Agents Example ===\n");
    
    let agents = runtime.list_agents().await;
    println!("   Found {} Claude agent(s)", agents.len());
    for agent in agents {
        println!("   - {} ({:?}): {:?}", agent.id, agent.name, agent.status);
    }

    println!("\n=== Example Complete ===");
    Ok(())
}
