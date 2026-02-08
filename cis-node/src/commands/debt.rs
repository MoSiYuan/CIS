//! # Debt Management Commands
//!
//! Commands for managing technical debts accumulated during DAG execution:
//! - `cis debt list` - List accumulated technical debts
//! - `cis debt resolve <task-id>` - Resolve a specific debt
//! - `cis debt summary` - Show debt statistics

use anyhow::Result;
use chrono::{DateTime, Utc};
use cis_core::scheduler::{DagRunStatus, DagScheduler};
use cis_core::storage::Paths;
use cis_core::types::FailureType;
use clap::Subcommand;
use std::collections::HashMap;

/// Debt management commands
#[derive(Debug, Subcommand)]
pub enum DebtCommands {
    /// List accumulated technical debts
    List {
        /// Filter by DAG run ID
        #[arg(short, long)]
        run_id: Option<String>,
        /// Show all debts including resolved ones
        #[arg(short, long)]
        all: bool,
    },

    /// Resolve a specific debt
    Resolve {
        /// Task ID whose debt to resolve
        task_id: String,
        /// DAG run ID (uses active run if not specified)
        #[arg(short = 'r', long)]
        run_id: Option<String>,
        /// Resume downstream tasks after resolving
        #[arg(short = 'c', long, default_value = "true")]
        resume: bool,
    },

    /// Show debt summary statistics
    Summary,
}

/// Handle debt commands
pub async fn handle(cmd: DebtCommands) -> Result<()> {
    match cmd {
        DebtCommands::List { run_id, all } => {
            list_debts(run_id.as_deref(), all).await?;
        }
        DebtCommands::Resolve {
            task_id,
            run_id,
            resume,
        } => {
            resolve_debt(&task_id, run_id.as_deref(), resume).await?;
        }
        DebtCommands::Summary => {
            debt_summary().await?;
        }
    }

    Ok(())
}

/// List accumulated technical debts
pub async fn list_debts(run_id: Option<&str>, all: bool) -> Result<()> {
    let scheduler = load_scheduler().await?;

    let debts = if let Some(rid) = run_id {
        // List debts for specific run
        if let Some(run) = scheduler.get_run(rid) {
            if all {
                run.debts.clone()
            } else {
                run.debts.iter().filter(|d| !d.resolved).cloned().collect()
            }
        } else {
            println!("DAG run not found: {}", rid);
            return Ok(());
        }
    } else {
        // List debts from all runs
        let mut all_debts = Vec::new();
        for run in scheduler.run_ids() {
            if let Some(r) = scheduler.get_run(run) {
                let run_debts: Vec<_> = if all {
                    r.debts.clone()
                } else {
                    r.debts.iter().filter(|d| !d.resolved).cloned().collect()
                };
                all_debts.extend(run_debts);
            }
        }
        all_debts
    };

    if debts.is_empty() {
        if all {
            println!("No debts found.");
        } else {
            println!("No unresolved debts. Use --all to see resolved debts.");
        }
        return Ok(());
    }

    // Print header
    println!("Technical Debts:");
    println!();
    println!(
        "{:<12} {:<20} {:<12} {:<20} {:<10}",
        "Task ID", "DAG Run", "Type", "Created", "Status"
    );
    println!("{}", "-".repeat(80));

    // Print debts
    for debt in debts {
        let created = format_datetime(debt.created_at);
        let status = if debt.resolved { "Resolved" } else { "Pending" };
        let debt_type = format_failure_type(debt.failure_type);

        println!(
            "{:<12} {:<20} {:<12} {:<20} {}",
            truncate(&debt.task_id, 12),
            truncate(&debt.dag_run_id, 20),
            debt_type,
            created,
            status
        );

        if !debt.error_message.is_empty() {
            println!("  Error: {}", truncate(&debt.error_message, 70));
        }
    }

    Ok(())
}

/// Resolve a specific debt
pub async fn resolve_debt(task_id: &str, run_id: Option<&str>, resume: bool) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };

    // Check if run exists
    if scheduler.get_run(&target_run_id).is_none() {
        println!("DAG run not found: {}", target_run_id);
        return Ok(());
    }

    // Resolve the debt in the DAG
    match scheduler.resolve_run_debt(&target_run_id, task_id, resume) {
        Ok(new_ready) => {
            println!("✓ Debt resolved for task {}", task_id);

            if resume {
                println!("  Downstream tasks resumed");
                if !new_ready.is_empty() {
                    println!("  Newly ready tasks: {}", new_ready.join(", "));
                }
            } else {
                println!("  Task marked as failed, downstream tasks remain skipped");
            }

            // Show updated run status
            if let Some(run) = scheduler.get_run(&target_run_id) {
                println!("  Run status: {:?}", run.status);
            }
        }
        Err(e) => {
            println!("Failed to resolve debt: {}", e);
        }
    }

    Ok(())
}

/// Show debt summary statistics
pub async fn debt_summary() -> Result<()> {
    let scheduler = load_scheduler().await?;

    let mut total_debts = 0;
    let mut unresolved_ignorable = 0;
    let mut unresolved_blocking = 0;
    let mut resolved_count = 0;
    let mut run_stats: HashMap<String, (usize, usize)> = HashMap::new(); // (total, unresolved)

    for run_id in scheduler.run_ids() {
        if let Some(run) = scheduler.get_run(run_id) {
            let total = run.debts.len();
            let unresolved = run.debts.iter().filter(|d| !d.resolved).count();

            total_debts += total;
            resolved_count += run.debts.iter().filter(|d| d.resolved).count();

            for debt in &run.debts {
                if !debt.resolved {
                    match debt.failure_type {
                        FailureType::Ignorable => unresolved_ignorable += 1,
                        FailureType::Blocking => unresolved_blocking += 1,
                    }
                }
            }

            run_stats.insert(run_id.clone(), (total, unresolved));
        }
    }

    // Print summary
    println!("╔════════════════════════════════════════╗");
    println!("║       Technical Debt Summary           ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    println!("Total Debts: {}", total_debts);
    println!("  Resolved: {}", resolved_count);
    println!(
        "  Unresolved: {} (Ignorable: {}, Blocking: {})",
        unresolved_ignorable + unresolved_blocking,
        unresolved_ignorable,
        unresolved_blocking
    );

    if !run_stats.is_empty() {
        println!();
        println!("By DAG Run:");
        println!("{:<36} {:<10} Unresolved", "Run ID", "Total");
        println!("{}", "-".repeat(60));

        for (run_id, (total, unresolved)) in run_stats {
            let run_status = if let Some(run) = scheduler.get_run(&run_id) {
                format_run_status(run.status)
            } else {
                "Unknown".to_string()
            };

            println!(
                "{:<36} {:<10} {}  [{}]",
                truncate(&run_id, 36),
                total,
                unresolved,
                run_status
            );
        }
    }

    // Show explanation
    println!();
    println!("Legend:");
    println!("  Ignorable - Task failed but downstream can continue");
    println!("  Blocking  - Task failed and blocked downstream tasks");
    println!();
    println!("Use 'cis debt resolve <task-id>' to resolve a debt.");

    Ok(())
}

/// DAG 运行数据库文件名
const DAG_RUNS_DB: &str = "dag_runs.db";

/// Load the DAG scheduler from persistent storage
async fn load_scheduler() -> Result<DagScheduler> {
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    // Ensure data directory exists
    tokio::fs::create_dir_all(&data_dir).await?;
    
    // Load with persistence
    match DagScheduler::with_persistence(db_path.to_str().unwrap()) {
        Ok(scheduler) => Ok(scheduler),
        Err(e) => {
            eprintln!("Warning: Failed to load scheduler from persistence: {}", e);
            eprintln!("Falling back to in-memory scheduler");
            Ok(DagScheduler::new())
        }
    }
}

/// Save the DAG scheduler to persistent storage
/// Note: With persistence enabled, runs are saved automatically on modification.
async fn save_scheduler(scheduler: &DagScheduler) -> Result<()> {
    // Persistence is handled automatically when runs are modified
    // We just verify the persistence layer is accessible
    if scheduler.persistence().is_none() {
        // No persistence configured, save to a temporary location
        let data_dir = Paths::data_dir();
        let db_path = data_dir.join(DAG_RUNS_DB);
        
        tokio::fs::create_dir_all(&data_dir).await?;
        
        // Create a new scheduler with persistence and copy runs
        let mut persistent_scheduler = DagScheduler::with_persistence(db_path.to_str().unwrap())?;
        
        // Copy all runs from the in-memory scheduler
        for run_id in scheduler.run_ids() {
            if let Some(run) = scheduler.get_run(run_id) {
                persistent_scheduler.update_run(run.clone())?;
            }
        }
    }
    Ok(())
}

/// Format DateTime for display
fn format_datetime(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// Format failure type for display
fn format_failure_type(ft: FailureType) -> &'static str {
    match ft {
        FailureType::Ignorable => "Ignorable",
        FailureType::Blocking => "Blocking",
    }
}

/// Format run status for display
fn format_run_status(status: DagRunStatus) -> String {
    match status {
        DagRunStatus::Running => "Running".to_string(),
        DagRunStatus::Paused => "Paused".to_string(),
        DagRunStatus::Completed => "Completed".to_string(),
        DagRunStatus::Failed => "Failed".to_string(),
    }
}

/// Helper: Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
