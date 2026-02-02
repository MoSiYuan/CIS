//! # CIS Node CLI
//!
//! Command-line interface for CIS (Cluster of Independent Systems).

use clap::{Parser, Subcommand, ValueEnum};
use tracing::{error, info};

mod commands;
use cis_node::TelemetryAction;

/// CLI structure
#[derive(Parser, Debug)]
#[command(name = "cis")]
#[command(about = "CIS - Cluster of Independent Systems")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Main commands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize CIS environment
    Init {
        /// Initialize project instead of global
        #[arg(long, short)]
        project: bool,
        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,
        /// Non-interactive mode (use defaults)
        #[arg(long)]
        non_interactive: bool,
        /// Skip environment checks
        #[arg(long)]
        skip_checks: bool,
        /// Preferred AI provider (claude|kimi|aider)
        #[arg(long)]
        provider: Option<String>,
    },
    
    /// Manage skills
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    
    /// Memory operations
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    
    /// Task management
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    
    /// Interact with AI agent
    Agent {
        /// The prompt to send to the agent
        prompt: Vec<String>,
        /// Enable interactive chat mode
        #[arg(long, short)]
        chat: bool,
        /// List available agents
        #[arg(long)]
        list: bool,
    },
    
    /// Check environment
    Doctor {
        /// Run quick fixes
        #[arg(long)]
        fix: bool,
    },
    
    /// Show CIS status
    Status,
    
    /// Peer management
    Peer {
        #[command(subcommand)]
        action: PeerAction,
    },
    
    /// Telemetry and request logging
    Telemetry {
        #[command(subcommand)]
        action: TelemetryAction,
    },
}

/// Skill subcommands
#[derive(Subcommand, Debug)]
enum SkillAction {
    /// List all skills
    List,
    
    /// Load a skill
    Load {
        /// Skill name
        name: String,
        /// Auto-activate after loading
        #[arg(long)]
        activate: bool,
    },
    
    /// Unload a skill
    Unload {
        /// Skill name
        name: String,
    },
    
    /// Activate a skill
    Activate {
        /// Skill name
        name: String,
    },
    
    /// Deactivate a skill
    Deactivate {
        /// Skill name
        name: String,
    },
    
    /// Show skill info
    Info {
        /// Skill name
        name: String,
    },
    
    /// Call a skill method
    Call {
        /// Skill name
        name: String,
        /// Method to call
        #[arg(long, short)]
        method: String,
        /// Arguments as JSON
        #[arg(long)]
        args: Option<String>,
    },
    
    /// Install a skill from path
    Install {
        /// Path to skill
        path: String,
    },
    
    /// Remove a skill
    Remove {
        /// Skill name
        name: String,
    },
    
    /// Execute skill by natural language (semantic invocation)
    Do {
        /// Natural language description
        description: String,
        /// Project path
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
        /// Show candidate skill list
        #[arg(short, long)]
        candidates: bool,
    },
}

/// Memory subcommands
#[derive(Subcommand, Debug)]
enum MemoryAction {
    /// Get a memory value
    Get {
        /// Memory key
        key: String,
    },
    
    /// Set a memory value
    Set {
        /// Memory key
        key: String,
        /// Memory value
        value: String,
        /// Memory domain (public/private)
        #[arg(long, value_enum, default_value = "public")]
        domain: MemoryDomain,
        /// Memory category
        #[arg(long, value_enum, default_value = "context")]
        category: MemoryCategory,
    },
    
    /// Delete a memory entry
    Delete {
        /// Memory key
        key: String,
    },
    
    /// Search memory entries (keyword-based)
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(long)]
        limit: Option<usize>,
    },
    
    /// Semantic search memory entries (vector-based)
    VectorSearch {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "5")]
        limit: usize,
        /// Similarity threshold
        #[arg(short, long)]
        threshold: Option<f32>,
        /// Category filter
        #[arg(short, long)]
        category: Option<String>,
    },
    
    /// List memory keys
    List {
        /// Key prefix filter
        #[arg(long)]
        prefix: Option<String>,
        /// Domain filter
        #[arg(long, value_enum)]
        domain: Option<MemoryDomain>,
    },
    
    /// Export public memory
    Export {
        /// Export since timestamp
        #[arg(long)]
        since: Option<i64>,
        /// Output file
        #[arg(long, short)]
        output: Option<String>,
    },
}

/// Peer subcommands
#[derive(Subcommand, Debug)]
enum PeerAction {
    /// Add a peer node
    Add {
        /// Node ID
        node_id: String,
        /// DID
        did: String,
        /// WebSocket endpoint
        endpoint: String,
    },
    
    /// Remove a peer node
    Remove {
        /// Node ID
        node_id: String,
    },
    
    /// List all peers
    List {
        /// Show offline peers
        #[arg(long)]
        all: bool,
    },
    
    /// Show peer details
    Info {
        /// Node ID
        node_id: String,
    },
    
    /// Set trust level for a peer
    Trust {
        /// Node ID
        node_id: String,
        /// Trust level (block, read, write)
        level: String,
    },
    
    /// Ping a peer
    Ping {
        /// Node ID
        node_id: String,
    },
    
    /// Show sync queue status
    Sync,
}

/// Task subcommands
#[derive(Subcommand, Debug)]
enum TaskAction {
    /// List tasks
    List {
        /// Filter by status
        #[arg(long, value_enum)]
        status: Option<TaskStatus>,
    },
    
    /// Show task details
    Show {
        /// Task ID
        id: String,
    },
    
    /// Create a new task
    Create {
        /// Task title
        #[arg(long, short)]
        title: String,
        /// Task description
        #[arg(long, short)]
        description: Option<String>,
        /// Task group
        #[arg(long, short)]
        group: Option<String>,
        /// Task priority
        #[arg(long, short, value_enum)]
        priority: Option<TaskPriority>,
        /// Completion criteria
        #[arg(long)]
        criteria: Option<String>,
    },
    
    /// Update task status
    Update {
        /// Task ID
        id: String,
        /// New status
        #[arg(long, short, value_enum)]
        status: TaskStatus,
    },
    
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
    
    /// Execute tasks using DAG scheduler
    Execute,
}

/// Memory domain enum
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum MemoryDomain {
    Public,
    Private,
}

impl From<MemoryDomain> for cis_core::types::MemoryDomain {
    fn from(domain: MemoryDomain) -> Self {
        match domain {
            MemoryDomain::Public => cis_core::types::MemoryDomain::Public,
            MemoryDomain::Private => cis_core::types::MemoryDomain::Private,
        }
    }
}

/// Memory category enum
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum MemoryCategory {
    Execution,
    Result,
    Error,
    Context,
    Skill,
}

impl From<MemoryCategory> for cis_core::types::MemoryCategory {
    fn from(category: MemoryCategory) -> Self {
        match category {
            MemoryCategory::Execution => cis_core::types::MemoryCategory::Execution,
            MemoryCategory::Result => cis_core::types::MemoryCategory::Result,
            MemoryCategory::Error => cis_core::types::MemoryCategory::Error,
            MemoryCategory::Context => cis_core::types::MemoryCategory::Context,
            MemoryCategory::Skill => cis_core::types::MemoryCategory::Skill,
        }
    }
}

/// Task status enum
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

impl From<TaskStatus> for cis_core::types::TaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Pending => cis_core::types::TaskStatus::Pending,
            TaskStatus::Running => cis_core::types::TaskStatus::Running,
            TaskStatus::Completed => cis_core::types::TaskStatus::Completed,
            TaskStatus::Failed => cis_core::types::TaskStatus::Failed,
            TaskStatus::Blocked => cis_core::types::TaskStatus::Blocked,
            TaskStatus::Cancelled => cis_core::types::TaskStatus::Cancelled,
        }
    }
}

/// Task priority enum
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl From<TaskPriority> for cis_core::types::TaskPriority {
    fn from(priority: TaskPriority) -> Self {
        match priority {
            TaskPriority::Low => cis_core::types::TaskPriority::Low,
            TaskPriority::Medium => cis_core::types::TaskPriority::Medium,
            TaskPriority::High => cis_core::types::TaskPriority::High,
            TaskPriority::Urgent => cis_core::types::TaskPriority::Urgent,
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
        )
        .init();
    
    let cli = Cli::parse();
    
    info!("Running command: {:?}", cli.command);
    
    match run_command(cli.command).await {
        Ok(_) => {
            info!("Command completed successfully");
        }
        Err(e) => {
            error!("Command failed: {}", e);
            eprintln!("❌ Error: {}", e);
            std::process::exit(1);
        }
    }
}

async fn run_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Init { project, force, non_interactive, skip_checks, provider } => {
            let options = commands::init::InitOptions {
                project_mode: project,
                project_dir: None,
                skip_checks,
                force,
                preferred_provider: provider,
                non_interactive,
            };
            
            commands::init::init_with_options(options).await
        }
        
        Commands::Skill { action } => match action {
            SkillAction::List => commands::skill::list_skills(),
            SkillAction::Load { name, activate } => commands::skill::load_skill(&name, activate),
            SkillAction::Unload { name } => commands::skill::unload_skill(&name),
            SkillAction::Activate { name } => commands::skill::activate_skill(&name),
            SkillAction::Deactivate { name } => commands::skill::deactivate_skill(&name),
            SkillAction::Info { name } => commands::skill::skill_info(&name),
            SkillAction::Call { name, method, args } => {
                commands::skill::call_skill(&name, &method, args.as_deref())
            }
            SkillAction::Install { path } => commands::skill::install_skill(&path),
            SkillAction::Remove { name } => commands::skill::remove_skill(&name),
            SkillAction::Do { description, project, candidates } => {
                let args = commands::skill::SkillDoArgs {
                    description,
                    project,
                    candidates,
                };
                commands::skill::handle_skill_do(args).await
            }
        }
        
        Commands::Memory { action } => match action {
            MemoryAction::Get { key } => commands::memory::get_memory(&key),
            MemoryAction::Set { key, value, domain, category } => {
                commands::memory::set_memory(
                    &key,
                    &value,
                    domain.into(),
                    category.into(),
                )
            }
            MemoryAction::Delete { key } => commands::memory::delete_memory(&key),
            MemoryAction::Search { query, limit } => {
                commands::memory::search_memory(&query, limit).await
            }
            MemoryAction::VectorSearch { query, limit, threshold, category } => {
                let args = commands::memory::MemorySearchArgs {
                    query,
                    limit,
                    threshold,
                    category,
                };
                commands::memory::handle_memory_search(args).await
            }
            MemoryAction::List { prefix, domain } => {
                commands::memory::list_memory(
                    prefix.as_deref(),
                    domain.map(Into::into),
                )
            }
            MemoryAction::Export { since, output } => {
                commands::memory::export_memory(since, output.as_deref())
            }
        }
        
        Commands::Task { action } => match action {
            TaskAction::List { status } => {
                commands::task::list_tasks(status.map(Into::into))
            }
            TaskAction::Show { id } => commands::task::task_details(&id),
            TaskAction::Create { title, description, group, priority, criteria } => {
                commands::task::create_task(
                    &title,
                    description.as_deref(),
                    group.as_deref(),
                    priority.map(Into::into),
                    criteria.as_deref(),
                )
            }
            TaskAction::Update { id, status } => {
                commands::task::update_task_status(&id, status.into())
            }
            TaskAction::Delete { id } => commands::task::delete_task(&id),
            TaskAction::Execute => commands::task::execute_tasks(),
        }
        
        Commands::Agent { prompt, chat, list } => {
            if list {
                commands::agent::list_agents().await
            } else if chat {
                commands::agent::interactive_chat().await
            } else {
                let prompt = if prompt.is_empty() {
                    return Err(anyhow::anyhow!("Prompt is required. Use --chat for interactive mode or --list to see available agents."));
                } else {
                    prompt.join(" ")
                };
                commands::agent::execute_prompt(&prompt).await
            }
        }
        
        Commands::Doctor { fix } => {
            if fix {
                commands::doctor::quick_fix()
            } else {
                commands::doctor::doctor()
            }
        }
        
        Commands::Status => {
            show_status()
        }
        
        Commands::Peer { action } => match action {
            PeerAction::Add { node_id, did, endpoint } => {
                commands::peer::add_peer(&node_id, &did, &endpoint)
            }
            PeerAction::Remove { node_id } => commands::peer::remove_peer(&node_id),
            PeerAction::List { all } => commands::peer::list_peers(all),
            PeerAction::Info { node_id } => commands::peer::peer_info(&node_id),
            PeerAction::Trust { node_id, level } => commands::peer::set_trust(&node_id, &level),
            PeerAction::Ping { node_id } => commands::peer::ping_peer(&node_id).await,
            PeerAction::Sync => commands::peer::sync_status(),
        }
        
        Commands::Telemetry { action } => {
            commands::telemetry::handle_telemetry(action)
        }
    }
}

fn show_status() -> anyhow::Result<()> {
    use cis_core::storage::paths::Paths;
    
    println!("CIS Status\n");
    println!("{}", "-".repeat(40));
    
    // Check initialization
    let initialized = Paths::config_file().exists();
    if initialized {
        println!("✅ CIS initialized");
    } else {
        println!("❌ CIS not initialized");
        println!("   Run 'cis init' to initialize.");
    }
    
    // Data directory
    println!("\nData Directory: {}", Paths::data_dir().display());
    
    // Config file
    if initialized {
        println!("Config File:    {}", Paths::config_file().display());
    }
    
    // Project check
    let project_file = std::env::current_dir()?.join(".cis/project.toml");
    if project_file.exists() {
        println!("\n✅ CIS project initialized in current directory");
    }
    
    Ok(())
}
