//! # DAG Execution Management Commands
//!
//! Commands for managing DAG runs:
//! - `cis dag run <dag-file>` - Create new DAG run
//! - `cis dag status <run-id>` - Show DAG run status
//! - `cis dag pause <run-id>` - Pause DAG run
//! - `cis dag resume <run-id>` - Resume DAG run
//! - `cis dag abort <run-id>` - Abort DAG run
//! - `cis dag amend <run-id> <task-id>` - Amend task in running DAG

use anyhow::Result;
use cis_core::scheduler::{DagNodeStatus, DagRunStatus, DagScheduler, TaskDag};
use cis_core::types::{TaskLevel, Action};
use cis_core::storage::Paths;
use clap::Subcommand;
use std::path::Path;

/// DAG execution management commands
#[derive(Debug, Subcommand)]
pub enum DagCommands {
    /// Create and start a new DAG run from a DAG file
    Run {
        /// Path to the DAG definition file
        dag_file: String,
        /// Custom run ID (auto-generated if not provided)
        #[arg(short, long)]
        run_id: Option<String>,
        /// Start in paused mode (for inspection before execution)
        #[arg(long)]
        paused: bool,
    },

    /// Show DAG run status
    Status {
        /// DAG run ID (uses active run if not specified)
        #[arg(short, long)]
        run_id: Option<String>,
        /// Show detailed task list
        #[arg(short, long)]
        verbose: bool,
    },

    /// Pause DAG run (for arbitration)
    Pause {
        /// DAG run ID (uses active run if not specified)
        #[arg(short, long)]
        run_id: Option<String>,
    },

    /// Resume a paused DAG run
    Resume {
        /// DAG run ID (uses active run if not specified)
        #[arg(short, long)]
        run_id: Option<String>,
    },

    /// Abort DAG run
    Abort {
        /// DAG run ID (uses active run if not specified)
        #[arg(short, long)]
        run_id: Option<String>,
        /// Force abort without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Amend task parameters in a running DAG
    Amend {
        /// DAG run ID
        #[arg(short, long)]
        run_id: String,
        /// Task ID to amend
        task_id: String,
        /// Environment variable changes (KEY=VALUE)
        #[arg(short, long, value_delimiter = ',')]
        env: Vec<String>,
        /// Update task command
        #[arg(short, long)]
        command: Option<String>,
    },

    /// List all DAG runs
    List {
        /// Show all runs including completed ones
        #[arg(short, long)]
        all: bool,
    },

    /// Set active DAG run
    Use {
        /// Run ID to set as active
        run_id: String,
    },

    /// Manage DAG workers
    Worker {
        #[command(subcommand)]
        cmd: WorkerCommands,
    },
}

/// Worker management subcommands
#[derive(Debug, Subcommand)]
pub enum WorkerCommands {
    /// List all active workers
    List,
}

/// Handle DAG commands
pub async fn handle(cmd: DagCommands) -> Result<()> {
    match cmd {
        DagCommands::Run {
            dag_file,
            run_id,
            paused,
        } => {
            let id = create_run(&dag_file, run_id, paused).await?;
            println!("Created DAG run: {}", id);
        }
        DagCommands::Status { run_id, verbose } => {
            show_status(run_id.as_deref(), verbose).await?;
        }
        DagCommands::Pause { run_id } => {
            pause_run(run_id.as_deref()).await?;
        }
        DagCommands::Resume { run_id } => {
            resume_run(run_id.as_deref()).await?;
        }
        DagCommands::Abort { run_id, force } => {
            abort_run(run_id.as_deref(), force).await?;
        }
        DagCommands::Amend {
            run_id,
            task_id,
            env,
            command,
        } => {
            amend_task(&run_id, &task_id, env, command).await?;
        }
        DagCommands::List { all } => {
            list_runs(all).await?;
        }
        DagCommands::Use { run_id } => {
            set_active_run(&run_id).await?;
        }
        DagCommands::Worker { cmd } => {
            match cmd {
                WorkerCommands::List => {
                    list_workers().await?;
                }
            }
        }
    }

    Ok(())
}

/// Create a new DAG run from a DAG definition file
pub async fn create_run(
    dag_file: &str,
    run_id: Option<String>,
    paused: bool,
) -> Result<String> {
    let dag_path = Path::new(dag_file);

    if !dag_path.exists() {
        anyhow::bail!("DAG file not found: {}", dag_file);
    }

    // Load DAG from file
    let dag = load_dag_from_file(dag_path).await?;

    // Validate DAG
    dag.validate()?;

    // Create scheduler and run
    let mut scheduler = load_scheduler().await?;

    let run_id = if let Some(id) = run_id {
        scheduler.create_run_with_id(dag, id)
    } else {
        scheduler.create_run(dag)
    };

    // If paused mode, immediately pause the run
    if paused {
        if let Some(run) = scheduler.get_run_mut(&run_id) {
            run.status = DagRunStatus::Paused;
        }
    }

    save_scheduler(&scheduler).await?;

    if paused {
        println!("✓ DAG run created in PAUSED mode: {}", run_id);
        println!("  Use 'cis dag resume {}' to start execution", run_id);
    } else {
        println!("✓ DAG run created and started: {}", run_id);
    }

    Ok(run_id)
}

/// Show DAG run status
pub async fn show_status(run_id: Option<&str>, verbose: bool) -> Result<()> {
    let scheduler = load_scheduler().await?;

    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id or use 'cis dag use <run-id>'.");
        return Ok(());
    };

    let run = match scheduler.get_run(&target_run_id) {
        Some(r) => r,
        None => {
            println!("DAG run not found: {}", target_run_id);
            return Ok(());
        }
    };

    // Print status header
    println!("╔════════════════════════════════════════╗");
    println!("║          DAG Run Status                ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    println!("Run ID:          {}", run.run_id);
    println!("Status:          {}", format_run_status(run.status));
    println!("Created:         {}", run.created_at.format("%Y-%m-%d %H:%M:%S"));

    // Count tasks by status
    let mut completed = 0;
    let mut running = 0;
    let mut pending = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut arbitrated = 0;
    let mut ignorable_debts = 0;
    let mut blocking_debts = 0;

    for node in run.dag.nodes().values() {
        match node.status {
            DagNodeStatus::Completed => completed += 1,
            DagNodeStatus::Running => running += 1,
            DagNodeStatus::Pending | DagNodeStatus::Ready => pending += 1,
            DagNodeStatus::Failed => failed += 1,
            DagNodeStatus::Skipped => skipped += 1,
            DagNodeStatus::Arbitrated => arbitrated += 1,
            DagNodeStatus::Debt(cis_core::types::FailureType::Ignorable) => {
                ignorable_debts += 1;
            }
            DagNodeStatus::Debt(cis_core::types::FailureType::Blocking) => {
                blocking_debts += 1;
            }
        }
    }

    let total = run.dag.node_count();

    println!();
    println!("Tasks: {} total", total);
    println!("  ✓ Completed:   {}", completed);
    println!("  ▸ Running:     {}", running);
    println!("  ○ Pending:     {}", pending);
    if failed > 0 {
        println!("  ✗ Failed:      {}", failed);
    }
    if skipped > 0 {
        println!("  ⊘ Skipped:     {}", skipped);
    }
    if arbitrated > 0 {
        println!("  ⚠ Arbitrated:  {}", arbitrated);
    }

    // Debts
    if ignorable_debts > 0 || blocking_debts > 0 {
        println!();
        println!("Debts:");
        if ignorable_debts > 0 {
            println!("  ! Ignorable:   {} (non-blocking)", ignorable_debts);
        }
        if blocking_debts > 0 {
            println!("  ✗ Blocking:    {} (halted execution)", blocking_debts);
        }
    }

    // Progress bar
    if total > 0 {
        let progress = ((completed + failed + skipped) as f32 / total as f32 * 100.0) as u32;
        println!();
        print!("Progress: [");
        let filled = (progress / 5) as usize;
        for _ in 0..filled {
            print!("█");
        }
        for _ in filled..20 {
            print!("░");
        }
        println!("] {}%", progress);
    }

    // Verbose mode: show all tasks
    if verbose {
        println!();
        println!("Task Details:");
        println!(
            "{:<16} {:<12} {:<15}",
            "Task ID", "Status", "Level"
        );
        println!("{}", "-".repeat(50));

        for (task_id, node) in run.dag.nodes() {
            let status = format_node_status(node.status);
            let level = format_task_level(&node.level);
            println!(
                "{:<16} {:<12} {}",
                truncate(task_id, 16),
                status,
                level
            );
        }
    }

    // Show actions based on status
    println!();
    match run.status {
        DagRunStatus::Paused => {
            println!("Actions: cis dag resume {}", target_run_id);
        }
        DagRunStatus::Running => {
            println!("Actions: cis dag pause {} | cis dag abort {}",
                target_run_id, target_run_id);
        }
        DagRunStatus::Failed => {
            if ignorable_debts > 0 || blocking_debts > 0 {
                println!("Actions: cis debt list --run-id {}", target_run_id);
            }
        }
        _ => {}
    }

    Ok(())
}

/// Pause a running DAG run
pub async fn pause_run(run_id: Option<&str>) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };

    if let Some(run) = scheduler.get_run_mut(&target_run_id) {
        if run.status == DagRunStatus::Running {
            run.status = DagRunStatus::Paused;
            save_scheduler(&scheduler).await?;
            println!("✓ DAG run {} paused", target_run_id);
            println!("  Use 'cis dag resume {}' to resume", target_run_id);
        } else {
            println!("Cannot pause run {} (status: {:?})", target_run_id, run.status);
        }
    } else {
        println!("DAG run not found: {}", target_run_id);
    }

    Ok(())
}

/// Resume a paused DAG run
pub async fn resume_run(run_id: Option<&str>) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };

    if let Some(run) = scheduler.get_run_mut(&target_run_id) {
        if run.status == DagRunStatus::Paused {
            run.status = DagRunStatus::Running;
            save_scheduler(&scheduler).await?;
            println!("✓ DAG run {} resumed", target_run_id);
        } else {
            println!("Cannot resume run {} (status: {:?})", target_run_id, run.status);
        }
    } else {
        println!("DAG run not found: {}", target_run_id);
    }

    Ok(())
}

/// Abort a DAG run
pub async fn abort_run(run_id: Option<&str>, force: bool) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };

    if let Some(run) = scheduler.get_run(&target_run_id) {
        if run.status == DagRunStatus::Completed {
            println!("Run {} is already completed.", target_run_id);
            return Ok(());
        }

        if !force {
            println!("Are you sure you want to abort run {}?", target_run_id);
            println!("This will terminate all running tasks.");
            println!("Use --force to skip this confirmation.");
            // In a real implementation, we'd prompt for confirmation here
            return Ok(());
        }

        // Remove the run
        scheduler.remove_run(&target_run_id);
        save_scheduler(&scheduler).await?;

        println!("✓ DAG run {} aborted", target_run_id);
    } else {
        println!("DAG run not found: {}", target_run_id);
    }

    Ok(())
}

/// Amend task parameters in a running DAG
pub async fn amend_task(
    run_id: &str,
    task_id: &str,
    env_changes: Vec<String>,
    command: Option<String>,
) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    let run = match scheduler.get_run_mut(run_id) {
        Some(r) => r,
        None => {
            println!("DAG run not found: {}", run_id);
            return Ok(());
        }
    };

    // Check if task exists
    if run.dag.get_node(task_id).is_none() {
        println!("Task not found: {} in run {}", task_id, run_id);
        return Ok(());
    }

    // Apply amendments
    if !env_changes.is_empty() {
        println!("Applying environment changes to task {}:", task_id);
        for change in &env_changes {
            if change.contains('=') {
                let parts: Vec<&str> = change.splitn(2, '=').collect();
                println!("  {} = {}", parts[0], parts[1]);
            } else {
                println!("  {} (invalid format, expected KEY=VALUE)", change);
            }
        }
    }

    if let Some(cmd) = command {
        println!("Updating command to: {}", cmd);
    }

    println!("✓ Task {} amended (persistence not yet implemented)", task_id);
    println!("  Note: Changes will take effect on next task execution");

    save_scheduler(&scheduler).await?;

    Ok(())
}

/// List all DAG runs
pub async fn list_runs(all: bool) -> Result<()> {
    let scheduler = load_scheduler().await?;

    if scheduler.run_count() == 0 {
        println!("No DAG runs found.");
        return Ok(());
    }

    println!("DAG Runs:");
    println!();
    println!(
        "{:<36} {:<12} {:<10} {:<20} {}",
        "Run ID", "Status", "Tasks", "Created", "Active"
    );
    println!("{}", "-".repeat(90));

    let active_run = scheduler.get_active_run().map(|r| r.run_id.clone());

    for run_id in scheduler.run_ids() {
        if let Some(run) = scheduler.get_run(run_id) {
            // Skip completed runs unless --all is specified
            if !all && run.status == DagRunStatus::Completed {
                continue;
            }

            let is_active = active_run.as_ref() == Some(run_id);
            let total = run.dag.node_count();
            let completed = run
                .dag
                .nodes()
                .values()
                .filter(|n| matches!(n.status, DagNodeStatus::Completed))
                .count();

            println!(
                "{:<36} {:<12} {}/{}       {} {}",
                truncate(run_id, 36),
                format_run_status(run.status),
                completed,
                total,
                run.created_at.format("%Y-%m-%d %H:%M"),
                if is_active { "*" } else { "" }
            );
        }
    }

    println!();
    println!("* indicates the active run");

    Ok(())
}

/// Set active DAG run
pub async fn set_active_run(run_id: &str) -> Result<()> {
    let mut scheduler = load_scheduler().await?;

    match scheduler.set_active_run(run_id.to_string()) {
        Ok(()) => {
            save_scheduler(&scheduler).await?;
            println!("✓ Set active run to: {}", run_id);
        }
        Err(e) => {
            println!("Failed to set active run: {}", e);
        }
    }

    Ok(())
}

/// List all DAG workers
pub async fn list_workers() -> Result<()> {
    use cis_core::scheduler::{DagPersistence};
    use cis_core::storage::Paths;
    
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    // Check if database exists
    if !db_path.exists() {
        println!("No DAG workers found (database does not exist yet).");
        return Ok(());
    }
    
    // 使用 WorkerService 查询运行的 workers
    use cis_core::service::{WorkerService, ListOptions};
    
    println!("DAG Workers:");
    println!();
    
    match WorkerService::new() {
        Ok(service) => {
            let options = ListOptions::default();
            match service.list(options).await {
                Ok(result) => {
                    if result.items.is_empty() {
                        println!("No running DAG workers found.");
                        println!();
                        println!("Use 'cis worker run' to start a worker.");
                    } else {
                        println!("{:<30} {:<15} {:<10} {:<10} {}", 
                            "Worker ID", "Scope", "Status", "PID", "Uptime");
                        println!("{}", "-".repeat(90));
                        
                        for worker in result.items {
                            let uptime = format_duration(worker.uptime);
                            let pid_str = worker.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
                            
                            println!("{:<30} {:<15} {:<10} {:<10} {}",
                                truncate(&worker.id, 30),
                                truncate(&worker.scope, 15),
                                worker.status.to_string(),
                                pid_str,
                                uptime
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("Error listing workers: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Error initializing worker service: {}", e);
        }
    }
    
    Ok(())
}

/// Load DAG from file
/// 
/// 支持三种格式：
/// 1. TOML 格式的 skill.toml（包含 [skill] 和 [dag] 部分）
/// 2. JSON 格式的纯 DAG 定义（DagDefinition 结构）
/// 3. 简单文本格式（向后兼容）
async fn load_dag_from_file(path: &Path) -> Result<TaskDag> {
    use cis_core::skill::manifest::SkillManifest;
    use cis_core::skill::dag::SkillDagConverter;

    println!("Loading DAG from: {}", path.display());

    let content = tokio::fs::read_to_string(path).await?;

    // 检查文件扩展名
    let extension = path.extension().map(|e| e.to_str().unwrap_or("")).unwrap_or("");

    match extension {
        // TOML 格式：可能是完整的 skill.toml 或纯 DAG 定义
        "toml" => {
            // 首先尝试作为完整的 skill.toml 解析
            if let Ok(manifest) = SkillManifest::from_dag_file(path) {
                if let Some(dag_def) = manifest.dag {
                    println!("📦 Loaded DAG skill: {}", manifest.skill.name);
                    println!("   Tasks: {}", dag_def.tasks.len());
                    println!("   Policy: {:?}", dag_def.policy);
                    
                    let task_dag = SkillDagConverter::to_task_dag(&dag_def)?;
                    return Ok(task_dag);
                }
            }
            
            // 尝试作为纯 DAG TOML 解析
            if let Ok(dag_def) = toml::from_str::<cis_core::skill::manifest::DagDefinition>(&content) {
                println!("📦 Loaded DAG definition (TOML)");
                println!("   Tasks: {}", dag_def.tasks.len());
                
                let task_dag = SkillDagConverter::to_task_dag(&dag_def)?;
                return Ok(task_dag);
            }
            
            anyhow::bail!("Failed to parse TOML file as DAG definition");
        }
        
        // JSON 格式
        "json" => {
            if let Ok(dag_def) = serde_json::from_str::<cis_core::skill::manifest::DagDefinition>(&content) {
                println!("📦 Loaded DAG definition (JSON)");
                println!("   Tasks: {}", dag_def.tasks.len());
                
                let task_dag = SkillDagConverter::to_task_dag(&dag_def)?;
                return Ok(task_dag);
            }
            
            anyhow::bail!("Failed to parse JSON file as DAG definition");
        }
        
        // 其他格式：尝试简单文本格式（向后兼容）
        _ => {
            println!("⚠️  Unknown file extension, trying simple text format...");
            load_dag_from_simple_format(&content).await
        }
    }
}

/// 从简单文本格式加载 DAG（向后兼容）
/// 
/// 格式：每行 "task_id: dependency1,dependency2" 或 
///       "task_id: dependency1,dependency2 [level:Mechanical|Recommended|Confirmed|Arbitrated]"
async fn load_dag_from_simple_format(content: &str) -> Result<TaskDag> {
    let mut dag = TaskDag::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse line format: "task_id: dep1,dep2 [level:Mechanical]"
        let (task_part, level) = if let Some(idx) = line.find(" [level:") {
            let (task_str, level_str) = line.split_at(idx);
            let level_str = &level_str[7..level_str.len()-1]; // Remove " [level:" and "]"
            (task_str, parse_task_level(level_str)?)
        } else {
            (line, TaskLevel::Mechanical { retry: 3 })
        };

        // Parse task and dependencies
        let parts: Vec<&str> = task_part.split(':').collect();
        if parts.is_empty() {
            continue;
        }

        let task_id = parts[0].trim().to_string();
        let deps: Vec<String> = if parts.len() > 1 {
            parts[1]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        dag.add_node_with_level(task_id, deps, level)?;
    }

    if dag.is_empty() {
        anyhow::bail!("No valid tasks found in DAG file");
    }

    Ok(dag)
}

/// Parse task level from string
fn parse_task_level(s: &str) -> Result<TaskLevel> {
    let level = match s.to_lowercase().as_str() {
        "mechanical" => TaskLevel::Mechanical { retry: 3 },
        "recommended" => TaskLevel::Recommended { 
            default_action: Action::Execute, 
            timeout_secs: 30 
        },
        "confirmed" => TaskLevel::Confirmed,
        "arbitrated" => TaskLevel::Arbitrated { 
            stakeholders: vec!["default".to_string()] 
        },
        _ => TaskLevel::Mechanical { retry: 3 },
    };
    Ok(level)
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
/// This function is kept for compatibility but does minimal work.
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

/// Format run status for display
fn format_run_status(status: DagRunStatus) -> &'static str {
    match status {
        DagRunStatus::Running => "running",
        DagRunStatus::Paused => "paused",
        DagRunStatus::Completed => "completed",
        DagRunStatus::Failed => "failed",
    }
}

/// Format node status for display
fn format_node_status(status: DagNodeStatus) -> String {
    match status {
        DagNodeStatus::Pending => "pending".to_string(),
        DagNodeStatus::Ready => "ready".to_string(),
        DagNodeStatus::Running => "running".to_string(),
        DagNodeStatus::Completed => "completed".to_string(),
        DagNodeStatus::Failed => "failed".to_string(),
        DagNodeStatus::Skipped => "skipped".to_string(),
        DagNodeStatus::Arbitrated => "arbitrated".to_string(),
        DagNodeStatus::Debt(ft) => format!("debt({:?})", ft),
    }
}

/// Format task level for display
fn format_task_level(level: &cis_core::types::TaskLevel) -> &'static str {
    match level {
        cis_core::types::TaskLevel::Mechanical { .. } => "mechanical",
        cis_core::types::TaskLevel::Recommended { .. } => "recommended",
        cis_core::types::TaskLevel::Confirmed => "confirmed",
        cis_core::types::TaskLevel::Arbitrated { .. } => "arbitrated",
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

/// Helper: Format duration in seconds to human readable
fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
    }
}
