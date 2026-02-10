//! Agent 命令真实实现
//!
//! 使用 ProcessDetector 显示真实的 Agent 状态

use anyhow::Result;
use clap::Subcommand;

use cis_core::agent::process_detector::{AgentProcessDetector, AgentType};

/// Agent 子命令
#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// 显示 Agent 状态
    Status {
        /// 指定 Agent 类型
        #[arg(long, short)]
        agent: Option<String>,
    },
}

/// 处理 Agent 命令
pub async fn handle_agent(cmd: AgentCommands) -> Result<()> {
    match cmd {
        AgentCommands::Status { agent } => show_status(agent.as_deref()).await,
    }
}

/// 显示 Agent 状态
async fn show_status(agent_filter: Option<&str>) -> Result<()> {
    println!("📊 Agent Status");
    println!("═══════════════\n");

    let agents = if let Some(filter) = agent_filter {
        // 显示特定 Agent
        match filter.to_lowercase().as_str() {
            "claude" => vec![(AgentType::Claude, AgentProcessDetector::detect(AgentType::Claude))],
            "opencode" => vec![(AgentType::OpenCode, AgentProcessDetector::detect(AgentType::OpenCode))],
            "kimi" => vec![(AgentType::Kimi, AgentProcessDetector::detect(AgentType::Kimi))],
            _ => {
                println!("❌ Unknown agent type: {}", filter);
                return Ok(());
            }
        }
    } else {
        // 显示所有 Agent
        vec![
            (AgentType::Claude, AgentProcessDetector::detect(AgentType::Claude)),
            (AgentType::OpenCode, AgentProcessDetector::detect(AgentType::OpenCode)),
            (AgentType::Kimi, AgentProcessDetector::detect(AgentType::Kimi)),
        ]
    };

    let mut total_running = 0;

    for (agent_type, processes) in agents {
        let display_name = agent_type.display_name();
        
        println!("{}:", display_name);
        
        if processes.is_empty() {
            println!("  🔴 Not running");
            println!("  💡 Start with: cis agent start {}\n", agent_type.process_name());
        } else {
            for proc in processes {
                total_running += 1;
                println!("  🟢 Running (PID: {})", proc.pid);
                println!("  📁 Working dir: {}", proc.working_dir.display());
                println!("  ⏱️  Started: {:?}", proc.start_time);
                
                if let Some(port) = proc.port {
                    println!("  🌐 Port: {}", port);
                }
                
                // 检查是否为僵尸进程
                if !AgentProcessDetector::is_running(proc.pid) {
                    println!("  ⚠️  Stale (process not found)");
                }
                
                println!();
            }
        }
    }

    if agent_filter.is_none() {
        println!("─────────────────────");
        println!("Total: {} agent(s) running", total_running);
    }

    Ok(())
}
