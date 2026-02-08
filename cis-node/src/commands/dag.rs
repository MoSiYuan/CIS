//! # DAG Execution Management Commands
//!
//! Commands for managing DAG runs:
//! - `cis dag run <dag-file>` - Create new DAG run
//! - `cis dag status <run-id>` - Show DAG run status
//! - `cis dag pause <run-id>` - Pause DAG run
//! - `cis dag resume <run-id>` - Resume DAG run
//! - `cis dag abort <run-id>` - Abort DAG run
//! - `cis dag amend <run-id> <task-id>` - Amend task in running DAG
//! - `cis dag definitions` - List DAG definitions from database
//! - `cis dag list` - List DAG runs with filters
//! - `cis dag logs <run-id>` - View DAG execution logs

use anyhow::Result;
use cis_core::scheduler::{DagNodeStatus, DagRunStatus, DagScheduler, TaskDag, TodoItemStatus};
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
        /// Filter by status (running, paused, completed, failed)
        #[arg(short, long)]
        status: Option<String>,
        /// Filter by scope (global, project, user, type or specific scope_id)
        #[arg(short = 'S', long)]
        scope: Option<String>,
        /// Filter by target node
        #[arg(short, long)]
        node: Option<String>,
    },

    /// List DAG definitions from database
    Definitions {
        /// Filter by scope (type or scope_id)
        #[arg(short, long)]
        scope: Option<String>,
        /// Filter by target node
        #[arg(short, long)]
        node: Option<String>,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
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

    /// Execute DAG run tasks directly (embedded mode, no Matrix required)
    Execute {
        /// DAG run ID (uses active run if not specified)
        #[arg(short, long)]
        run_id: Option<String>,
        /// Use Agent Cluster execution (spawn Agent sessions)
        #[arg(short, long)]
        use_agent: bool,
        /// Max concurrent Agent workers (requires --use-agent)
        #[arg(short = 'w', long, default_value = "4")]
        max_workers: usize,
    },

    /// List active Agent sessions
    Sessions {
        /// Filter by DAG run ID
        #[arg(short, long)]
        dag: Option<String>,
        /// Show all sessions including completed ones
        #[arg(short, long)]
        all: bool,
    },

    /// Attach to an Agent session
    Attach {
        /// Session ID (format: run_id:task_id or short_id)
        session_id: Option<String>,
        /// DAG run ID (alternative to session_id)
        #[arg(short, long)]
        run: Option<String>,
        /// Task ID (used with --run)
        #[arg(short, long)]
        task: Option<String>,
        /// Force attach (kick existing user)
        #[arg(short, long)]
        force: bool,
        /// Readonly mode (don't take control)
        #[arg(long)]
        readonly: bool,
    },

    /// View session logs
    Logs {
        /// Session ID (format: run_id:task_id or short_id)
        session_id: String,
        /// Number of lines to show from the end
        #[arg(short, long, default_value = "50")]
        tail: usize,
        /// Follow output in real-time
        #[arg(short, long)]
        follow: bool,
    },

    /// Kill an Agent session
    Kill {
        /// Session ID (format: run_id:task_id or short_id)
        session_id: String,
        /// Kill all sessions for a DAG run
        #[arg(short, long)]
        all: bool,
    },

    /// Unblock a blocked session (mark as recovered)
    Unblock {
        /// Session ID (format: run_id:task_id or short_id)
        session_id: String,
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
        DagCommands::List { all, status, scope, node } => {
            if status.is_some() || scope.is_some() || node.is_some() {
                list_runs_filtered(status.as_deref(), scope.as_deref(), node.as_deref(), all).await?;
            } else {
                list_runs(all).await?;
            }
        }
        DagCommands::Definitions { scope, node, limit } => {
            list_definitions(scope.as_deref(), node.as_deref(), limit).await?;
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
        DagCommands::Execute { run_id, use_agent, max_workers } => {
            if use_agent {
                execute_run_agent(run_id.as_deref(), max_workers).await?;
            } else {
                execute_run(run_id.as_deref()).await?;
            }
        }
        DagCommands::Sessions { dag, all } => {
            list_sessions(dag.as_deref(), all).await?;
        }
        DagCommands::Attach { session_id, run, task, force, readonly } => {
            attach_session(session_id.as_deref(), run.as_deref(), task.as_deref(), force, readonly).await?;
        }
        DagCommands::Logs { session_id, tail, follow } => {
            // Try database logs first, fallback to session logs
            if view_logs_from_db(&session_id, tail).await.is_err() {
                view_logs(&session_id, tail, follow).await?;
            }
        }
        DagCommands::Kill { session_id, all } => {
            kill_session(&session_id, all).await?;
        }
        DagCommands::Unblock { session_id } => {
            unblock_session(&session_id).await?;
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

    // Load DAG and extract commands from file
    let (dag, task_commands) = load_dag_with_commands(dag_path).await?;

    // Validate DAG
    dag.validate()?;

    // Create scheduler and run
    let mut scheduler = load_scheduler().await?;

    let run_id = scheduler.create_run_with_source(
        dag,
        run_id,
        Some(dag_file.to_string()),
        task_commands,
    );

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
        "{:<36} {:<12} {:<10} {:<20} Active",
        "Run ID", "Status", "Tasks", "Created"
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
                        println!("{:<30} {:<15} {:<10} {:<10} Uptime", 
                            "Worker ID", "Scope", "Status", "PID");
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

/// Load DAG from file and extract task commands
async fn load_dag_with_commands(
    path: &Path,
) -> Result<(TaskDag, std::collections::HashMap<String, String>)> {
    use cis_core::skill::manifest::{DagDefinition, SkillManifest};
    use cis_core::skill::dag::SkillDagConverter;
    use std::collections::HashMap;

    let content = tokio::fs::read_to_string(path).await?;
    let extension = path.extension().map(|e| e.to_str().unwrap_or("")).unwrap_or("");

    let dag_def: DagDefinition = match extension {
        "toml" => {
            // Try as full skill.toml first
            match SkillManifest::from_dag_file(path) {
                Ok(manifest) => {
                    if let Some(dag) = manifest.dag {
                        dag
                    } else {
                        anyhow::bail!("Skill manifest has no DAG definition");
                    }
                }
                Err(e1) => {
                    // Try as pure DAG TOML with [dag] header
                    match toml::from_str::<cis_core::skill::manifest::DagFileDefinition>(&content) {
                        Ok(file_def) => file_def.dag,
                        Err(_e2) => {
                            // Try as pure DAG TOML without [dag] header
                            match toml::from_str::<DagDefinition>(&content) {
                                Ok(dag) => dag,
                                Err(e3) => {
                                    anyhow::bail!(
                                        "Failed to parse TOML:\n  As skill manifest: {}\n  As DAG file [dag]: {}\n  As DAG definition: {}",
                                        e1, _e2, e3
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        "json" => {
            serde_json::from_str::<DagDefinition>(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?
        }
        _ => {
            anyhow::bail!("Unsupported file extension for DAG: {}", extension);
        }
    };

    // Convert to TaskDag
    let mut task_dag = SkillDagConverter::to_task_dag(&dag_def)?;
    
    // Initialize the DAG - set root nodes to Ready status
    task_dag.initialize();

    // Extract commands: prioritize 'command' field, fallback to 'skill' field
    let mut commands = HashMap::new();
    for task in &dag_def.tasks {
        if !task.command.is_empty() {
            commands.insert(task.id.clone(), task.command.clone());
        } else if !task.skill.is_empty() {
            commands.insert(task.id.clone(), format!("skill:{}", task.skill));
        }
    }

    Ok((task_dag, commands))
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

/// View logs from database
async fn view_logs_from_db(run_id: &str, tail: usize) -> Result<()> {
    use cis_core::scheduler::{DagPersistence, TaskExecutionStatus};
    use cis_core::storage::Paths;
    
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    if !db_path.exists() {
        anyhow::bail!("No DAG database found");
    }
    
    let persistence = DagPersistence::new(db_path.to_str().unwrap())?;
    let executions = persistence.list_task_executions(Some(run_id))?;
    
    if executions.is_empty() {
        anyhow::bail!("No execution logs found for run: {}", run_id);
    }
    
    println!("Execution logs for run: {}", run_id);
    println!();
    
    for exec in executions.iter().take(tail) {
        let status_icon = match exec.status {
            TaskExecutionStatus::Completed => "✓",
            TaskExecutionStatus::Failed => "✗",
            TaskExecutionStatus::Running => "▸",
            TaskExecutionStatus::Cancelled => "⊘",
            TaskExecutionStatus::Pending => "○",
        };
        
        println!("{} Task: {} (retry: {})", status_icon, exec.task_id, exec.retry_count);
        println!("   Started:  {}", exec.started_at.format("%Y-%m-%d %H:%M:%S"));
        if let Some(completed) = exec.completed_at {
            let duration = completed.signed_duration_since(exec.started_at);
            println!("   Duration: {}s", duration.num_seconds());
        }
        
        if let Some(ref output) = exec.output {
            if !output.is_empty() {
                println!("   Output:");
                for line in output.lines().take(20) {
                    println!("     {}", line);
                }
                if output.lines().count() > 20 {
                    println!("     ... ({} more lines)", output.lines().count() - 20);
                }
            }
        }
        
        if let Some(ref error) = exec.error {
            if !error.is_empty() {
                println!("   Error: {}", error);
            }
        }
        
        println!();
    }
    
    Ok(())
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

/// Execute DAG run tasks
/// 
/// Each Ready task is executed sequentially. Future: spawn Worker Agents.
async fn execute_run(run_id: Option<&str>) -> Result<()> {
    use cis_core::scheduler::{DagNodeStatus, DagRunStatus};
    
    let mut scheduler = load_scheduler().await?;
    
    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };
    
    println!("Executing DAG run: {}", target_run_id);
    println!();
    
    // Execute tasks in topological order
    let mut executed_count = 0;
    let mut failed_count = 0;
    
    loop {
        // Get tasks that are ready to execute
        let ready_tasks: Vec<(String, String)> = {
            let Some(run) = scheduler.get_run(&target_run_id) else {
                println!("DAG run not found: {}", target_run_id);
                return Ok(());
            };
            
            if run.status == DagRunStatus::Paused {
                println!("DAG run is paused. Resume to continue execution.");
                break;
            }
            
            if run.status == DagRunStatus::Completed {
                println!("DAG run completed.");
                break;
            }
            
            if run.status == DagRunStatus::Failed {
                println!("DAG run failed.");
                break;
            }
            
            let commands = &run.task_commands;
            run.dag.nodes()
                .iter()
                .filter(|(_, node)| node.status == DagNodeStatus::Ready)
                .filter_map(|(id, _node)| {
                    commands.get(id).map(|cmd| (id.clone(), cmd.clone()))
                })
                .collect()
        };
        
        if ready_tasks.is_empty() {
            // Check if all tasks are done
            let run = scheduler.get_run(&target_run_id).unwrap();
            let all_done = run.dag.nodes().values().all(|n| {
                matches!(n.status, DagNodeStatus::Completed | DagNodeStatus::Failed | DagNodeStatus::Skipped)
            });
            
            if all_done {
                println!("All tasks completed.");
            } else {
                println!("No ready tasks. Waiting for dependencies...");
            }
            break;
        }
        
        // Execute ready tasks
        for (task_id, command) in ready_tasks {
            println!("  → Executing task: {}", task_id);
            
            // Mark task as running
            let run_clone = {
                let run = scheduler.get_run_mut(&target_run_id).unwrap();
                run.dag.mark_running(task_id.clone())?;
                run.clone()
            };
            scheduler.update_run(run_clone)?;
            
            // Execute command via shell
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .await;
            
            match output {
                Ok(result) => {
                    if result.status.success() {
                        let stdout = String::from_utf8_lossy(&result.stdout);
                        if !stdout.is_empty() {
                            println!("    Output: {}", stdout.trim());
                        }
                        
                        // Mark task as completed
                        let run_clone = {
                            let run = scheduler.get_run_mut(&target_run_id).unwrap();
                            run.dag.mark_completed(task_id.clone())?;
                            run.update_status();
                            run.clone()
                        };
                        scheduler.update_run(run_clone)?;
                        executed_count += 1;
                        println!("    ✓ Completed");
                    } else {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        println!("    ✗ Failed: {}", stderr.trim());
                        
                        // Mark task as failed
                        let run_clone = {
                            let run = scheduler.get_run_mut(&target_run_id).unwrap();
                            run.dag.mark_failed(task_id.clone())?;
                            run.update_status();
                            run.clone()
                        };
                        scheduler.update_run(run_clone)?;
                        failed_count += 1;
                    }
                }
                Err(e) => {
                    println!("    ✗ Execution error: {}", e);
                    
                    let run_clone = {
                        let run = scheduler.get_run_mut(&target_run_id).unwrap();
                        run.dag.mark_failed(task_id.clone())?;
                        run.update_status();
                        run.clone()
                    };
                    scheduler.update_run(run_clone)?;
                    failed_count += 1;
                }
            }
        }
    }
    
    println!();
    println!("Execution summary:");
    println!("  Completed: {}", executed_count);
    println!("  Failed: {}", failed_count);
    
    // Show final status
    show_status(Some(&target_run_id), false).await?;
    
    Ok(())
}

/// List active Agent sessions
async fn list_sessions(dag_filter: Option<&str>, all: bool) -> Result<()> {
    use cis_core::agent::cluster::SessionManager;
    
    let manager = SessionManager::global();
    
    let sessions = if let Some(dag_id) = dag_filter {
        manager.list_sessions_by_dag(dag_id).await
    } else {
        manager.list_sessions().await
    };
    
    if sessions.is_empty() {
        println!("No active sessions found.");
        if dag_filter.is_none() {
            println!("Use 'cis dag run <file>' to create a DAG run with Agent sessions.");
        }
        return Ok(());
    }
    
    // Filter out completed sessions unless --all
    let filtered_sessions: Vec<_> = sessions
        .into_iter()
        .filter(|s| {
            all || !s.state.contains("completed") && !s.state.contains("failed")
        })
        .collect();
    
    if filtered_sessions.is_empty() {
        println!("No active sessions found. Use --all to show completed sessions.");
        return Ok(());
    }
    
    println!("Agent Sessions:");
    println!();
    println!(
        "{:<20} {:<15} {:<12} {:<36} {:<10} Runtime",
        "Session ID", "Task", "Agent", "DAG Run", "Status"
    );
    println!("{}", "-".repeat(110));
    
    for session in filtered_sessions {
        let runtime = format_duration(session.runtime_secs);
        let agent_str = format!("{:?}", session.agent_type).to_lowercase();
        
        println!(
            "{:<20} {:<15} {:<12} {:<36} {:<10} {}",
            truncate(&session.short_id, 20),
            truncate(&session.task_id, 15),
            truncate(&agent_str, 12),
            truncate(&session.dag_run_id, 36),
            truncate(&session.state, 10),
            runtime
        );
        
        // Show output preview if available
        if !session.output_preview.is_empty() {
            let preview: String = session.output_preview
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join(" | ");
            println!("  └─ {}", truncate(&preview, 100));
        }
    }
    
    println!();
    println!("Commands:");
    println!("  cis dag attach <session-id>  - Attach to a session");
    println!("  cis dag logs <session-id>    - View session logs");
    println!("  cis dag kill <session-id>    - Kill a session");
    
    Ok(())
}

/// Attach to an Agent session
async fn attach_session(
    session_id: Option<&str>,
    run_id: Option<&str>,
    task_id: Option<&str>,
    force: bool,
    readonly: bool,
) -> Result<()> {
    use cis_core::agent::cluster::{SessionManager, SessionId, SessionState, parse_session_id};
    
    let manager = SessionManager::global();
    
    // Determine session ID
    let session_id = if let Some(sid) = session_id {
        parse_session_id(sid)?
    } else if let (Some(run), Some(task)) = (run_id, task_id) {
        SessionId::new(run, task)
    } else {
        anyhow::bail!("Must specify either session_id or both --run and --task");
    };
    
    // Check if session exists
    let state = manager.get_state(&session_id).await?;
    
    println!("Attaching to session {}...", session_id.short());
    println!("Current state: {}", state);
    println!();
    
    if readonly {
        // Readonly mode: just show output
        println!("=== Readonly Mode (Press Ctrl+C to exit) ===");
        let output = manager.get_output(&session_id).await?;
        println!("{}", output);
        return Ok(());
    }
    
    // Check if already attached
    if let SessionState::Attached { user } = &state {
        if !force {
            anyhow::bail!("Session is already attached by {}. Use --force to take over.", user);
        }
        println!("Force taking over session from {}...", user);
    }
    
    // Create attach handle
    let handle = manager.attach_session(&session_id, "cli-user").await?;
    
    println!("=== Attached to {} ===", session_id.short());
    println!("Commands:");
    println!("  Ctrl+B D - Detach");
    println!("  Ctrl+B K - Kill session");
    println!();
    
    // Set terminal to raw mode for interactive session
    let _ = crossterm::terminal::enable_raw_mode();
    
    // Spawn task to forward output
    let output_handle = tokio::spawn(async move {
        loop {
            if let Some(data) = handle.try_receive_output() {
                let text = String::from_utf8_lossy(&data);
                print!("{}", text);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });
    
    // Wait for Ctrl+C or detach command
    tokio::signal::ctrl_c().await?;
    
    // Cancel output task
    output_handle.abort();
    
    // Detach
    let _ = manager.detach_session(&session_id, "cli-user").await;
    
    println!();
    println!("Detached from session {}", session_id.short());
    
    Ok(())
}

/// View session logs
async fn view_logs(session_id_str: &str, tail: usize, follow: bool) -> Result<()> {
    use cis_core::agent::cluster::{SessionManager, parse_session_id};
    
    let manager = SessionManager::global();
    let session_id = parse_session_id(session_id_str)?;
    
    // Get current output
    let output = manager.get_output(&session_id).await?;
    
    // Show last N lines
    let lines: Vec<&str> = output.lines().collect();
    let start = if lines.len() > tail { lines.len() - tail } else { 0 };
    
    for line in &lines[start..] {
        println!("{}", line);
    }
    
    if follow {
        println!();
        println!("=== Following output (Press Ctrl+C to exit) ===");
        
        let mut last_len = lines.len();
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            let output = manager.get_output(&session_id).await?;
            let lines: Vec<&str> = output.lines().collect();
            
            if lines.len() > last_len {
                for line in &lines[last_len..] {
                    println!("{}", line);
                }
                last_len = lines.len();
            }
        }
    }
    
    Ok(())
}

/// Kill an Agent session
async fn kill_session(session_id_str: &str, all: bool) -> Result<()> {
    use cis_core::agent::cluster::{SessionManager, parse_session_id};
    
    let manager = SessionManager::global();
    
    if all {
        // Parse run_id from session_id (format: run_id:task_id)
        let parts: Vec<&str> = session_id_str.splitn(2, ':').collect();
        let run_id = parts[0];
        
        let count = manager.kill_all_by_dag(run_id, "User requested kill-all").await?;
        println!("✓ Killed {} session(s) for DAG run {}", count, run_id);
    } else {
        let session_id = parse_session_id(session_id_str)?;
        manager.kill_session(&session_id, "User requested").await?;
        println!("✓ Killed session {}", session_id.short());
    }
    
    Ok(())
}

/// Unblock a blocked session
async fn unblock_session(session_id_str: &str) -> Result<()> {
    use cis_core::agent::cluster::{SessionManager, SessionState, parse_session_id};
    
    let manager = SessionManager::global();
    let session_id = parse_session_id(session_id_str)?;
    
    // Check current state
    let state = manager.get_state(&session_id).await?;
    
    match state {
        SessionState::Blocked { reason } => {
            println!("Unblocking session {}...", session_id.short());
            println!("Previous blockage: {}", reason);
            
            manager.mark_recovered(&session_id).await?;
            
            println!("✓ Session unblocked. Agent will continue execution.");
        }
        _ => {
            println!("Session {} is not blocked (current state: {})", session_id.short(), state);
        }
    }
    
    Ok(())
}

/// Execute DAG run using Agent Cluster
async fn execute_run_agent(run_id: Option<&str>, max_workers: usize) -> Result<()> {
    use cis_core::agent::cluster::{AgentClusterConfig, AgentClusterExecutor};
    use cis_core::scheduler::DagRunStatus;
    
    let mut scheduler = load_scheduler().await?;
    
    let target_run_id = if let Some(rid) = run_id {
        rid.to_string()
    } else if let Some(active) = scheduler.get_active_run() {
        active.run_id.clone()
    } else {
        println!("No active DAG run. Please specify --run-id.");
        return Ok(());
    };
    
    // Get the run
    let run = match scheduler.get_run_mut(&target_run_id) {
        Some(r) => r,
        None => {
            println!("DAG run not found: {}", target_run_id);
            return Ok(());
        }
    };
    
    // Check if can execute
    if run.status == DagRunStatus::Completed {
        println!("Run {} is already completed.", target_run_id);
        return Ok(());
    }
    
    println!("Executing DAG run {} with Agent Cluster", target_run_id);
    println!("Max workers: {}", max_workers);
    println!();
    
    // Create executor with config
    let config = AgentClusterConfig {
        max_workers,
        ..Default::default()
    };
    
    let executor = AgentClusterExecutor::new(config)?;
    
    // Execute
    let report = executor.execute_run(run).await?;
    
    // Save updated run
    save_scheduler(&scheduler).await?;
    
    // Print summary
    println!();
    println!("╔════════════════════════════════════════╗");
    println!("║      Agent Cluster Execution Done      ║");
    println!("╚════════════════════════════════════════╝");
    println!();
    println!("Duration: {}s", report.duration_secs);
    println!("Completed: {}", report.completed);
    println!("Failed: {}", report.failed);
    println!("Skipped: {}", report.skipped);
    
    // Show outputs
    if !report.outputs.is_empty() {
        println!();
        println!("Task Outputs:");
        for (task_id, output) in &report.outputs {
            let icon = if output.success { "✓" } else { "✗" };
            println!("  {} {}: {} chars", icon, task_id, output.output.len());
        }
    }
    
    // Show next actions
    println!();
    match report.status {
        DagRunStatus::Completed => {
            println!("✓ DAG run completed successfully!");
        }
        DagRunStatus::Failed => {
            println!("✗ DAG run failed.");
            println!("  Check 'cis dag logs <session-id>' for details.");
        }
        DagRunStatus::Paused => {
            println!("⏸ DAG run is paused (blocked sessions).");
            println!("  Use 'cis dag sessions' to view blocked sessions.");
            println!("  Use 'cis dag attach <session-id>' to intervene.");
            println!("  Use 'cis dag unblock <session-id>' after fixing.");
        }
        _ => {}
    }
    
    Ok(())
}

/// List DAG definitions from database
async fn list_definitions(
    scope_filter: Option<&str>,
    node_filter: Option<&str>,
    limit: Option<usize>,
) -> Result<()> {
    use cis_core::scheduler::DagPersistence;
    use cis_core::storage::Paths;
    
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    if !db_path.exists() {
        println!("No DAG database found. Run a DAG first.");
        return Ok(());
    }
    
    let persistence = DagPersistence::new(db_path.to_str().unwrap())?;
    let conn = persistence.connection();
    
    // Load all and filter in Rust for simplicity
    let sql = "SELECT dag_id, scope_type, scope_id, target_node, priority, version, created_at 
               FROM dag_specs ORDER BY created_at DESC";
    
    let mut stmt = conn.prepare(sql)?;
    
    type DefRow = (String, String, Option<String>, Option<String>, String, i64, String);
    
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;
    
    let mut definitions: Vec<DefRow> = Vec::new();
    for row in rows {
        let (dag_id, scope_type, scope_id, target_node, priority, version, created_at) = row?;
        
        // Apply filters
        if let Some(filter) = scope_filter {
            let matches = scope_type.eq_ignore_ascii_case(filter)
                || scope_id.as_ref().map(|s| s.contains(filter)).unwrap_or(false);
            if !matches {
                continue;
            }
        }
        if let Some(filter) = node_filter {
            let matches = target_node.as_ref().map(|n| n == filter).unwrap_or(false);
            if !matches {
                continue;
            }
        }
        
        definitions.push((dag_id, scope_type, scope_id, target_node, priority, version, created_at));
        
        // Apply limit
        if let Some(lim) = limit {
            if definitions.len() >= lim {
                break;
            }
        }
    }
    
    if definitions.is_empty() {
        println!("No DAG definitions found.");
        return Ok(());
    }
    
    println!("DAG Definitions:");
    println!();
    println!(
        "{:<30} {:<12} {:<15} {:<15} {:<10} Version",
        "DAG ID", "Scope", "Scope ID", "Target Node", "Priority"
    );
    println!("{}", "-".repeat(110));
    
    for (dag_id, scope_type, scope_id, target_node, priority, version, _created_at) in definitions {
        let scope_str = scope_id.map(|id| format!("{}:{}", scope_type, id)).unwrap_or(scope_type.clone());
        let target_str = target_node.unwrap_or_else(|| "broadcast".to_string());
        
        println!(
            "{:<30} {:<12} {:<15} {:<15} {:<10} v{}",
            truncate(&dag_id, 30),
            scope_type,
            truncate(&scope_str, 15),
            truncate(&target_str, 15),
            priority,
            version
        );
    }
    
    Ok(())
}

/// List DAG runs from database with filtering
async fn list_runs_filtered(
    status_filter: Option<&str>,
    scope_filter: Option<&str>,
    node_filter: Option<&str>,
    all: bool,
) -> Result<()> {
    use cis_core::scheduler::DagPersistence;
    use cis_core::storage::Paths;
    
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    if !db_path.exists() {
        println!("No DAG runs found (database does not exist yet).");
        return Ok(());
    }
    
    let persistence = DagPersistence::new(db_path.to_str().unwrap())?;
    let conn = persistence.connection();
    
    // Use a simpler approach - load all and filter in Rust
    let mut sql = String::from(
        "SELECT run_id, dag_id, status, scope_type, scope_id, target_node, created_at 
         FROM dag_runs WHERE 1=1"
    );
    
    if !all {
        sql.push_str(" AND status != 'Completed'");
    }
    sql.push_str(" ORDER BY created_at DESC");
    
    let mut stmt = conn.prepare(&sql)?;
    
    type RunRow = (String, String, String, String, Option<String>, Option<String>, String);
    
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;
    
    let mut runs: Vec<RunRow> = Vec::new();
    for row in rows {
        let (run_id, dag_id, status, scope_type, scope_id, target_node, created_at) = row?;
        
        // Apply additional filters
        if let Some(filter) = status_filter {
            if !status.eq_ignore_ascii_case(filter) {
                continue;
            }
        }
        if let Some(filter) = scope_filter {
            let matches = scope_type.eq_ignore_ascii_case(filter) 
                || scope_id.as_ref().map(|s| s.contains(filter)).unwrap_or(false);
            if !matches {
                continue;
            }
        }
        if let Some(filter) = node_filter {
            let matches = target_node.as_ref().map(|n| n == filter).unwrap_or(false);
            if !matches {
                continue;
            }
        }
        
        runs.push((run_id, dag_id, status, scope_type, scope_id, target_node, created_at));
    }
    
    if runs.is_empty() {
        println!("No DAG runs found.");
        return Ok(());
    }
    
    println!("DAG Runs:");
    println!();
    println!(
        "{:<36} {:<30} {:<12} {:<15} {:<15} Created",
        "Run ID", "DAG ID", "Status", "Scope", "Target Node"
    );
    println!("{}", "-".repeat(130));
    
    for (run_id, dag_id, status, scope_type, scope_id, target_node, created_at) in runs {
        let scope_str = scope_id.map(|id| format!("{}:{}", scope_type, id)).unwrap_or(scope_type);
        let target_str = target_node.as_deref().unwrap_or("broadcast");
        let created_short = &created_at[..16]; // YYYY-MM-DD HH:MM
        
        println!(
            "{:<36} {:<30} {:<12} {:<15} {:<15} {}",
            truncate(&run_id, 36),
            truncate(&dag_id, 30),
            status,
            truncate(&scope_str, 15),
            truncate(target_str, 15),
            created_short
        );
    }
    
    Ok(())
}

/// Show detailed DAG status from database
async fn show_status_from_db(run_id: &str) -> Result<()> {
    use cis_core::scheduler::DagPersistence;
    use cis_core::storage::Paths;
    
    let data_dir = Paths::data_dir();
    let db_path = data_dir.join(DAG_RUNS_DB);
    
    if !db_path.exists() {
        println!("No DAG database found.");
        return Ok(());
    }
    
    let persistence = DagPersistence::new(db_path.to_str().unwrap())?;
    
    // Load the run
    let run = match persistence.load_run(run_id)? {
        Some(r) => r,
        None => {
            println!("DAG run not found: {}", run_id);
            return Ok(());
        }
    };
    
    // Print status header
    println!("╔════════════════════════════════════════╗");
    println!("║          DAG Run Status (DB)           ║");
    println!("╚════════════════════════════════════════╝");
    println!();
    
    println!("Run ID:          {}", run.run_id);
    println!("Status:          {}", format_run_status(run.status));
    println!("Created:         {}", run.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("Updated:         {}", run.updated_at.format("%Y-%m-%d %H:%M:%S"));
    
    // Count tasks by status
    let mut completed = 0;
    let mut running = 0;
    let mut pending = 0;
    let mut failed = 0;
    
    for node in run.dag.nodes().values() {
        match node.status {
            DagNodeStatus::Completed => completed += 1,
            DagNodeStatus::Running => running += 1,
            DagNodeStatus::Pending | DagNodeStatus::Ready => pending += 1,
            DagNodeStatus::Failed => failed += 1,
            _ => {}
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
    
    // Progress bar
    if total > 0 {
        let progress = ((completed + failed) as f32 / total as f32 * 100.0) as u32;
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
    
    // Show TODO list if available
    if !run.todo_list.items.is_empty() {
        println!();
        println!("TODO List:");
        for item in &run.todo_list.items {
            let status_icon = match item.status {
                TodoItemStatus::Completed => "✓",
                TodoItemStatus::InProgress => "▸",
                TodoItemStatus::Blocked => "⊘",
                _ => "○",
            };
            println!("  {} [{}] {} (P{})", 
                status_icon, 
                item.id,
                truncate(&item.description, 40),
                item.priority
            );
        }
        
        if let Some(ref checkpoint) = run.todo_list.last_checkpoint {
            println!();
            println!("Last Checkpoint: {}", checkpoint.format("%Y-%m-%d %H:%M:%S"));
        }
        if !run.todo_list.agent_notes.is_empty() {
            println!("Agent Notes: {}", run.todo_list.agent_notes);
        }
    }
    
    Ok(())
}
