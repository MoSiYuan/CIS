//! # Decision Commands for Four-Tier Decision Mechanism
//!
//! CLI commands for managing four-tier decisions:
//! - `cis decision confirm <dag-id>` - Confirm a Confirmed-level task
//! - `cis decision reject <dag-id>` - Reject a Confirmed-level task
//! - `cis decision status` - Show pending decisions
//! - `cis decision vote <vote-id> --approve/--reject` - Vote on arbitration

use anyhow::Result;
use clap::Subcommand;
use std::sync::Arc;
use tokio::sync::Mutex;

use cis_core::decision::{
    ArbitrationManager, ConfirmationManager, ConfirmationResponse, Vote, VoteStatus, VoteStats,
};
use cis_core::storage::Paths;

/// Decision management commands
#[derive(Debug, Subcommand)]
pub enum DecisionCommands {
    /// Confirm a pending task (Confirmed level)
    Confirm {
        /// Request ID or Task ID
        request_id: String,
    },

    /// Reject a pending task (Confirmed level)
    Reject {
        /// Request ID or Task ID
        request_id: String,
    },

    /// Show pending decisions
    Status {
        /// Show all decisions including completed
        #[arg(short, long)]
        all: bool,
        /// Filter by DAG run ID
        #[arg(short, long)]
        run_id: Option<String>,
    },

    /// Vote on an arbitration (Arbitrated level)
    Vote {
        /// Vote ID
        vote_id: String,
        /// Approve the task
        #[arg(long)]
        approve: bool,
        /// Reject the task
        #[arg(long)]
        reject: bool,
        /// Stakeholder identity
        #[arg(short, long)]
        stakeholder: String,
    },

    /// List active arbitrations
    Arbitrations {
        /// Show completed arbitrations
        #[arg(short, long)]
        all: bool,
    },

    /// Initialize decision configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },
}

/// Handle decision commands
pub async fn handle(cmd: DecisionCommands) -> Result<()> {
    match cmd {
        DecisionCommands::Confirm { request_id } => {
            confirm_request(&request_id).await?;
        }
        DecisionCommands::Reject { request_id } => {
            reject_request(&request_id).await?;
        }
        DecisionCommands::Status { all, run_id } => {
            show_status(all, run_id.as_deref()).await?;
        }
        DecisionCommands::Vote {
            vote_id,
            approve,
            reject,
            stakeholder,
        } => {
            if !approve && !reject {
                println!("Error: Must specify --approve or --reject");
                return Ok(());
            }
            let vote = if approve { Vote::Approve } else { Vote::Reject };
            cast_vote(&vote_id, &stakeholder, vote).await?;
        }
        DecisionCommands::Arbitrations { all } => {
            list_arbitrations(all).await?;
        }
        DecisionCommands::Init { force } => {
            init_config(force).await?;
        }
    }

    Ok(())
}

/// Confirm a pending request
async fn confirm_request(request_id: &str) -> Result<()> {
    let manager = create_confirmation_manager().await?;
    let mgr = manager.lock().await;

    // 尝试直接确认
    if mgr.confirm(request_id).await {
        println!("✓ Confirmed request: {}", request_id);
        return Ok(());
    }

    // 如果没有找到，尝试查找匹配的请求
    let pending = mgr.get_pending().await;
    let matching: Vec<_> = pending
        .iter()
        .filter(|r| r.task_id == request_id || r.id.starts_with(request_id))
        .collect();

    if matching.is_empty() {
        println!("No pending confirmation found for: {}", request_id);
        return Ok(());
    }

    if matching.len() == 1 {
        let req = &matching[0];
        if mgr.confirm(&req.id).await {
            println!("✓ Confirmed task: {} ({})", req.task_id, req.id);
        }
    } else {
        println!("Multiple matching requests found:");
        for req in matching {
            println!("  {} - Task: {} (Run: {})", req.id, req.task_id, req.run_id);
        }
        println!("Please use the full request ID to confirm.");
    }

    Ok(())
}

/// Reject a pending request
async fn reject_request(request_id: &str) -> Result<()> {
    let manager = create_confirmation_manager().await?;
    let mgr = manager.lock().await;

    // 尝试直接拒绝
    if mgr.reject(request_id).await {
        println!("✗ Rejected request: {}", request_id);
        return Ok(());
    }

    // 如果没有找到，尝试查找匹配的请求
    let pending = mgr.get_pending().await;
    let matching: Vec<_> = pending
        .iter()
        .filter(|r| r.task_id == request_id || r.id.starts_with(request_id))
        .collect();

    if matching.is_empty() {
        println!("No pending confirmation found for: {}", request_id);
        return Ok(());
    }

    if matching.len() == 1 {
        let req = &matching[0];
        if mgr.reject(&req.id).await {
            println!("✗ Rejected task: {} ({})", req.task_id, req.id);
        }
    } else {
        println!("Multiple matching requests found:");
        for req in matching {
            println!("  {} - Task: {} (Run: {})", req.id, req.task_id, req.run_id);
        }
        println!("Please use the full request ID to reject.");
    }

    Ok(())
}

/// Show pending decisions status
async fn show_status(all: bool, run_id_filter: Option<&str>) -> Result<()> {
    println!("╔════════════════════════════════════════╗");
    println!("║        Pending Decisions               ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    // 显示确认请求
    let confirmation_manager = create_confirmation_manager().await?;
    let mgr = confirmation_manager.lock().await;
    let pending_confirmations = if let Some(run_id) = run_id_filter {
        mgr.get_pending_by_run(run_id).await
    } else {
        mgr.get_pending().await
    };
    drop(mgr);

    if !pending_confirmations.is_empty() {
        println!("Confirmed Level (Waiting for User Confirmation):");
        println!();
        println!(
            "{:<20} {:<20} {:<15} {}",
            "Request ID", "Task ID", "Run ID", "Remaining"
        );
        println!("{}", "-".repeat(80));

        for req in &pending_confirmations {
            let remaining = req.remaining_secs();
            let time_str = format!("{}s", remaining);
            println!(
                "{:<20} {:<20} {:<15} {}",
                truncate(&req.id, 20),
                truncate(&req.task_id, 20),
                truncate(&req.run_id, 15),
                time_str
            );
        }
        println!();
        println!("Commands:");
        println!("  cis decision confirm <request-id>  - Confirm and continue");
        println!("  cis decision reject <request-id>   - Reject and abort");
        println!();
    } else {
        println!("No pending confirmations.");
        println!();
    }

    // 显示仲裁投票
    let arbitration_manager = create_arbitration_manager().await?;
    let mgr = arbitration_manager.lock().await;
    let active_votes = mgr.get_active_votes().await;
    drop(mgr);

    if !active_votes.is_empty() {
        println!("Arbitrated Level (Waiting for Votes):");
        println!();
        println!(
            "{:<20} {:<20} {:<15} {:<15} {}",
            "Vote ID", "Task ID", "Stakeholders", "Votes Cast", "Remaining"
        );
        println!("{}", "-".repeat(90));

        for vote in &active_votes {
            let stats = vote.get_stats();
            let remaining = vote.remaining_secs();
            println!(
                "{:<20} {:<20} {:<15} {:<15} {}s",
                truncate(&vote.id, 20),
                truncate(&vote.task_id, 20),
                stats.total,
                stats.approve + stats.reject + stats.abstain,
                remaining
            );
        }
        println!();
        println!("Commands:");
        println!("  cis decision vote <vote-id> --stakeholder <name> --approve/--reject");
        println!();
    } else {
        println!("No active arbitrations.");
    }

    Ok(())
}

/// Cast a vote on arbitration
async fn cast_vote(vote_id: &str, stakeholder: &str, vote: Vote) -> Result<()> {
    let manager = create_arbitration_manager().await?;
    let mgr = manager.lock().await;

    // 获取投票信息
    let vote_info = mgr.get_vote(vote_id).await;
    drop(mgr);

    match vote_info {
        Some(v) => {
            let mgr = manager.lock().await;
            if mgr.cast_vote(vote_id, stakeholder, vote).await {
                let stats = mgr.get_stats(vote_id).await.unwrap_or(VoteStats {
                    total: 0,
                    approve: 0,
                    reject: 0,
                    abstain: 0,
                    pending: 0,
                });
                println!(
                    "✓ Vote recorded for task: {} ({:?})",
                    v.task_id, vote
                );
                println!(
                    "  Progress: {}/{} voted ({} approve, {} reject)",
                    stats.approve + stats.reject + stats.abstain,
                    stats.total,
                    stats.approve,
                    stats.reject
                );
            } else {
                println!("✗ Failed to record vote. Make sure you are a valid stakeholder.");
            }
        }
        None => {
            println!("Vote not found: {}", vote_id);
        }
    }

    Ok(())
}

/// List active arbitrations
async fn list_arbitrations(all: bool) -> Result<()> {
    let manager = create_arbitration_manager().await?;
    let mgr = manager.lock().await;

    let votes = if all {
        // 获取所有投票
        vec![] // 简化实现
    } else {
        mgr.get_active_votes().await
    };

    if votes.is_empty() {
        println!("No active arbitrations found.");
        return Ok(());
    }

    println!("Active Arbitrations:");
    println!();
    println!(
        "{:<20} {:<20} {:<15} {:<12} {}",
        "Vote ID", "Task ID", "Run ID", "Status", "Progress"
    );
    println!("{}", "-".repeat(80));

    for vote in votes {
        let stats = vote.get_stats();
        let progress = format!(
            "{}/{} ({:.0}% approve)",
            stats.approve + stats.reject + stats.abstain,
            stats.total,
            stats.approve_ratio() * 100.0
        );

        let status = match vote.status {
            VoteStatus::Pending => "pending",
            VoteStatus::Approved => "approved",
            VoteStatus::Rejected => "rejected",
            VoteStatus::Expired => "expired",
        };

        println!(
            "{:<20} {:<20} {:<15} {:<12} {}",
            truncate(&vote.id, 20),
            truncate(&vote.task_id, 20),
            truncate(&vote.run_id, 15),
            status,
            progress
        );
    }

    Ok(())
}

/// Initialize decision configuration
async fn init_config(force: bool) -> Result<()> {
    use cis_core::decision::config::{generate_default_config, DecisionConfig};

    let config_path = DecisionConfig::config_path();

    match config_path {
        Some(path) => {
            if path.exists() && !force {
                println!("Configuration already exists at: {}", path.display());
                println!("Use --force to overwrite.");
                return Ok(());
            }

            // 确保目录存在
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // 写入默认配置
            let config_content = generate_default_config();
            std::fs::write(&path, config_content)?;

            println!("✓ Decision configuration initialized");
            println!("  Path: {}", path.display());
            println!();
            println!("You can customize the following settings:");
            println!("  - timeout_recommended: Countdown seconds for Recommended level");
            println!("  - timeout_confirmed: Timeout for user confirmation");
            println!("  - timeout_arbitrated: Timeout for arbitration voting");
            println!("  - arbitration_threshold: Vote threshold for approval (0.0-1.0)");
        }
        None => {
            println!("Could not determine config directory.");
        }
    }

    Ok(())
}

/// Create confirmation manager
async fn create_confirmation_manager() -> Result<Arc<Mutex<ConfirmationManager>>> {
    use cis_core::decision::DecisionConfig;
    let config = DecisionConfig::load();
    Ok(Arc::new(Mutex::new(ConfirmationManager::new(
        config.timeout_confirmed,
    ))))
}

/// Create arbitration manager
async fn create_arbitration_manager() -> Result<Arc<Mutex<ArbitrationManager>>> {
    use cis_core::decision::DecisionConfig;
    let config = DecisionConfig::load();
    Ok(Arc::new(Mutex::new(ArbitrationManager::new(
        config.timeout_arbitrated,
    ))))
}

/// Helper: Truncate string
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("test", 4), "test");
    }
}
