//! # CIS Node CLI
//!
//! Command-line interface for CIS (Cluster of Independent Systems).
#![allow(dead_code)]

use clap::{Parser, Subcommand, ValueEnum};
use tracing::{error, info};

mod commands;
use cis_node::TelemetryAction;
use cis_core::storage::paths::Paths;

/// CLI structure
#[derive(Parser, Debug)]
#[command(name = "cis")]
#[command(about = "CIS - Cluster of Independent Systems")]
#[command(version = "0.1.0")]
struct Cli {
    /// Output JSON format (AI-Native mode)
    #[arg(long, global = true, help = "Output in JSON format for AI integration")]
    json: bool,
    
    #[command(subcommand)]
    command: Commands,
}

/// Main commands
#[derive(Subcommand, Debug)]
enum Commands {
    /// IM (Instant Messaging) operations
    Im {
        #[command(subcommand)]
        action: ImSubcommand,
    },

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
        #[command(subcommand)]
        action: Option<AgentSubcommand>,
        
        /// The prompt to send to the agent (positional, 用于向后兼容)
        prompt: Vec<String>,
        /// Enable interactive chat mode
        #[arg(long, short)]
        chat: bool,
        /// List available agents
        #[arg(long)]
        list: bool,
        /// Session ID for conversation context
        #[arg(short, long)]
        session: Option<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
    },
    
    /// Check environment
    Doctor {
        /// Run quick fixes
        #[arg(long)]
        fix: bool,
    },
    
    /// Show CIS status and paths
    Status {
        /// Show detailed path information
        #[arg(long)]
        paths: bool,
    },
    
    /// Peer management (legacy)
    Peer {
        #[command(subcommand)]
        action: PeerAction,
    },
    
    /// P2P network management
    P2p {
        #[command(subcommand)]
        action: commands::p2p::P2pAction,
    },
    
    /// Node management (static peer discovery)
    Node {
        #[command(subcommand)]
        action: commands::node::NodeAction,
    },
    
    /// Network access control
    Network {
        #[command(subcommand)]
        action: commands::network::NetworkCommands,
    },
    
    /// Telemetry and request logging
    Telemetry {
        #[command(subcommand)]
        action: TelemetryAction,
    },
    
    /// Task level management (four-tier decision mechanism)
    TaskLevel {
        #[command(subcommand)]
        action: commands::task_level::TaskLevelCommands,
    },
    
    /// Technical debt management
    Debt {
        #[command(subcommand)]
        action: commands::debt::DebtCommands,
    },
    
    /// DAG execution management
    Dag {
        #[command(subcommand)]
        action: commands::dag::DagCommands,
    },
    
    /// GLM API service management
    Glm {
        #[command(subcommand)]
        action: commands::glm::GlmCommands,
    },
    
    /// DAG worker process
    Worker {
        #[command(subcommand)]
        action: commands::worker::WorkerCommands,
    },
    
    /// System management (directories, migration, cleanup)
    System {
        #[command(subcommand)]
        action: commands::system::SystemCommands,
    },
    
    /// CLI Schema self-description for AI integration
    Schema {
        /// Output format (json, yaml)
        #[arg(long, short, default_value = "json")]
        format: String,
        /// Show command compositions (pipeline patterns)
        #[arg(long)]
        compositions: bool,
    },
    
    /// Generate shell completion scripts
    Completion {
        /// Shell type (bash, zsh, fish, powershell)
        shell: ShellType,
    },
}

/// Shell types for completion
#[derive(ValueEnum, Debug, Clone, Copy)]
enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

/// Agent subcommands
#[derive(Subcommand, Debug)]
enum AgentSubcommand {
    /// Execute a prompt (默认)
    Prompt {
        /// The prompt text
        prompt: Vec<String>,
    },
    
    /// Interactive chat mode
    Chat,
    
    /// List available agents
    List,
    
    /// Execute with conversation context
    Context {
        /// The prompt to send
        prompt: String,
        /// Session ID for conversation context
        #[arg(short, long)]
        session: Option<String>,
        /// Project path
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
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
    
    /// Discover and execute skill chains
    Chain {
        /// Natural language description of the task
        description: String,
        /// Preview mode - only show the chain without executing
        #[arg(long)]
        preview: bool,
        /// Show detailed matching information
        #[arg(short, long)]
        verbose: bool,
        /// Project path
        #[arg(short, long)]
        project: Option<std::path::PathBuf>,
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
        /// Output format (plain, json, table)
        #[arg(short, long, value_enum, default_value = "plain")]
        format: OutputFormat,
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

/// IM subcommands
#[derive(Subcommand, Debug)]
enum ImSubcommand {
    /// Send a message
    Send(commands::im::SendArgs),
    /// List sessions
    List(commands::im::ListArgs),
    /// View message history
    History(commands::im::HistoryArgs),
    /// Search messages
    Search(commands::im::SearchArgs),
    /// Create a new session
    Create(commands::im::CreateArgs),
    /// Mark messages as read
    Read(commands::im::ReadArgs),
    /// Get session info
    Info(commands::im::InfoArgs),
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

/// Output format enum for search results
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputFormat {
    Plain,
    Json,
    Table,
}

impl From<OutputFormat> for commands::memory::OutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Plain => commands::memory::OutputFormat::Plain,
            OutputFormat::Json => commands::memory::OutputFormat::Json,
            OutputFormat::Table => commands::memory::OutputFormat::Table,
        }
    }
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
    
    // 首次运行检测（排除 init, doctor, status, help）
    if let Err(e) = check_first_run(&cli.command).await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    
    match run_command(cli.command, cli.json).await {
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

/// 检查是否首次运行
/// 
/// 对于需要初始化的命令，如果 CIS 未初始化：
/// - Release 模式：自动创建默认配置
/// - 开发模式：提示用户初始化
async fn check_first_run(command: &Commands) -> anyhow::Result<()> {
    use cis_core::storage::paths::{Paths, RunMode};
    
    // Release 模式下，Status 命令也需要触发自动初始化
    let is_release = Paths::run_mode() == RunMode::Release;
    
    // 不需要初始化的命令白名单
    let needs_init = if is_release {
        // Release 模式：只有 init 和 doctor 不需要初始化
        !matches!(command, Commands::Init { .. } | Commands::Doctor { .. })
    } else {
        // 开发模式：init, doctor, status 不需要初始化
        !matches!(command, 
            Commands::Init { .. } | 
            Commands::Doctor { .. } | 
            Commands::Status { .. }
        )
    };
    
    if needs_init && !Paths::config_file().exists() {
        // Release 模式下自动初始化
        if Paths::run_mode() == RunMode::Release {
            eprintln!("📦 Release 模式：自动初始化 CIS...");
            
            // 创建默认配置
            let config = create_default_config().await?;
            
            // 确保目录存在
            if let Some(parent) = Paths::config_file().parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            // 写入配置
            std::fs::write(Paths::config_file(), config)?;
            
            // 创建数据目录
            Paths::ensure_dirs()?;
            
            eprintln!("✅ CIS 自动初始化完成");
            eprintln!("   配置: {}", Paths::config_file().display());
            eprintln!("   数据: {}", Paths::data_dir().display());
            eprintln!();
            
            return Ok(());
        }
        
        // 开发模式：提示用户初始化
        eprintln!("⚠️  CIS 尚未初始化");
        eprintln!();
        
        // 显示路径信息
        Paths::print_info();
        
        eprintln!();
        eprintln!("💡 请先初始化 CIS:");
        eprintln!("   cis init           # 交互式初始化");
        eprintln!("   cis init --help    # 查看初始化选项");
        eprintln!();
        eprintln!("   或使用快速初始化:");
        eprintln!("   cis init --non-interactive --provider claude");
        eprintln!();
        
        // 检查 Git 项目
        if let Some(git_root) = Paths::git_root() {
            eprintln!("📁 检测到 Git 项目: {}", git_root.display());
            eprintln!("   初始化数据将存储在: {}", git_root.join(".cis").display());
            eprintln!();
        }
        
        return Err(anyhow::anyhow!("CIS not initialized"));
    }
    
    Ok(())
}

/// 创建默认配置（用于 Release 模式自动初始化）
async fn create_default_config() -> anyhow::Result<String> {
    use cis_core::wizard::ConfigGenerator;
    use cis_core::storage::paths::Paths;
    
    // 生成节点密钥
    let node_key = generate_node_key();
    let key_path = Paths::data_dir().join("node.key");
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&key_path, &node_key)?;
    
    // 设置权限（Unix only）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&key_path)?.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(&key_path, permissions)?;
    }
    
    let generator = ConfigGenerator::new();
    let mut config = generator.generate_global_config(None)?;
    
    // 添加节点密钥到配置
    let key_hex = hex::encode(&node_key);
    config.push_str(&format!(r#"
[node]
key = "{}"
"#, key_hex));
    
    // 添加 P2P 默认配置
    config.push_str(r#"

[p2p]
enabled = true
listen_port = 7677
enable_dht = true
enable_nat_traversal = true

[p2p.bootstrap]
nodes = []
"#);
    
    Ok(config)
}

/// 生成节点密钥
fn generate_node_key() -> Vec<u8> {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key.to_vec()
}

async fn run_command(command: Commands, json_output: bool) -> anyhow::Result<()> {
    // Store json_output flag for use in subcommands
    let _json_mode = json_output;
    match command {
        Commands::Im { action } => {
            let args = commands::im::ImArgs { action: match action {
                ImSubcommand::Send(args) => commands::im::ImAction::Send(args),
                ImSubcommand::List(args) => commands::im::ImAction::List(args),
                ImSubcommand::History(args) => commands::im::ImAction::History(args),
                ImSubcommand::Search(args) => commands::im::ImAction::Search(args),
                ImSubcommand::Create(args) => commands::im::ImAction::Create(args),
                ImSubcommand::Read(args) => commands::im::ImAction::Read(args),
                ImSubcommand::Info(args) => commands::im::ImAction::Info(args),
            }};
            commands::im::handle_im(args).await
        }
        
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
            SkillAction::Load { name, activate } => commands::skill::load_skill(&name, activate).await,
            SkillAction::Unload { name } => commands::skill::unload_skill(&name).await,
            SkillAction::Activate { name } => commands::skill::activate_skill(&name).await,
            SkillAction::Deactivate { name } => commands::skill::deactivate_skill(&name).await,
            SkillAction::Info { name } => commands::skill::skill_info(&name),
            SkillAction::Call { name, method, args } => {
                commands::skill::call_skill(&name, &method, args.as_deref()).await
            }
            SkillAction::Install { path } => commands::skill::install_skill(&path),
            SkillAction::Remove { name } => commands::skill::remove_skill(&name).await,
            SkillAction::Do { description, project, candidates } => {
                let args = commands::skill::SkillDoArgs {
                    description,
                    project,
                    candidates,
                };
                commands::skill::handle_skill_do(args).await
            }
            SkillAction::Chain { description, preview, verbose, project } => {
                let args = commands::skill::SkillChainArgs {
                    description,
                    preview,
                    verbose,
                    project,
                };
                commands::skill::handle_skill_chain(args).await
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
            MemoryAction::VectorSearch { query, limit, threshold, category, format } => {
                let args = commands::memory::MemorySearchArgs {
                    query,
                    limit,
                    threshold,
                    category,
                    format: format.into(),
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
            TaskAction::Execute => commands::task::execute_tasks().await,
        }
        
        Commands::Agent { action, prompt, chat, list, session, project } => {
            // 如果指定了子命令，使用子命令
            if let Some(action) = action {
                match action {
                    AgentSubcommand::Prompt { prompt: p } => {
                        let prompt = p.join(" ");
                        if prompt.is_empty() {
                            return Err(anyhow::anyhow!("Prompt is required"));
                        }
                        commands::agent::execute_prompt(&prompt).await
                    }
                    AgentSubcommand::Chat => {
                        commands::agent::interactive_chat().await
                    }
                    AgentSubcommand::List => {
                        commands::agent::list_agents().await
                    }
                    AgentSubcommand::Context { prompt, session, project } => {
                        let args = commands::agent::AgentContextArgs {
                            prompt,
                            session,
                            project,
                        };
                        commands::agent::handle_agent_context(args).await
                    }
                }
            } else {
                // 向后兼容：使用 flags
                if list {
                    commands::agent::list_agents().await
                } else if chat {
                    commands::agent::interactive_chat().await
                } else if session.is_some() || project.is_some() {
                    let prompt = if prompt.is_empty() {
                        return Err(anyhow::anyhow!("Prompt is required. Use --chat for interactive mode or --list to see available agents."));
                    } else {
                        prompt.join(" ")
                    };
                    let args = commands::agent::AgentContextArgs {
                        prompt,
                        session,
                        project,
                    };
                    commands::agent::handle_agent_context(args).await
                } else {
                    let prompt = if prompt.is_empty() {
                        return Err(anyhow::anyhow!("Prompt is required. Use --chat for interactive mode or --list to see available agents."));
                    } else {
                        prompt.join(" ")
                    };
                    commands::agent::execute_prompt(&prompt).await
                }
            }
        }
        
        Commands::Doctor { fix } => {
            if fix {
                commands::doctor::quick_fix()
            } else {
                commands::doctor::doctor()
            }
        }
        
        Commands::Status { paths } => {
            if paths {
                Paths::print_info();
            } else {
                show_status();
            }
            Ok(())
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
        
        Commands::P2p { action } => {
            let args = commands::p2p::P2pArgs { action };
            commands::p2p::handle_p2p(args).await
        }
        
        Commands::Node { action } => {
            commands::node::handle(action).await
        }
        
        Commands::Network { action } => {
            commands::network::handle(action).await
        }
        
        Commands::Telemetry { action } => {
            commands::telemetry::handle_telemetry(action)
        }
        
        Commands::TaskLevel { action } => {
            commands::task_level::handle(action).await
        }
        
        Commands::Debt { action } => {
            commands::debt::handle(action).await
        }
        
        Commands::Dag { action } => {
            commands::dag::handle(action).await
        }
        
        Commands::Glm { action } => {
            commands::glm::execute(action).await
        }
        
        Commands::Worker { action } => {
            commands::worker::handle(action).await
        }
        
        Commands::System { action } => {
            commands::system::handle(action).await
        }
        
        Commands::Schema { format, compositions } => {
            commands::schema::handle(format, compositions).await
        }
        
        Commands::Completion { shell } => {
            generate_completion(shell);
            Ok(())
        }
    }
}

/// Generate shell completion script
fn generate_completion(shell: ShellType) {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};
    
    let mut cmd = Cli::command();
    let shell_type = match shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
        ShellType::PowerShell => Shell::PowerShell,
        ShellType::Elvish => Shell::Elvish,
    };
    
    let bin_name = "cis";
    generate(shell_type, &mut cmd, bin_name, &mut std::io::stdout());
}

fn show_status() {
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
    let project_file = std::env::current_dir()
        .ok()
        .and_then(|d| Some(d.join(".cis/project.toml")));
    
    if let Some(project_file) = project_file {
        if project_file.exists() {
            println!("\n✅ CIS project initialized in current directory");
        }
    }
    
    // Run mode
    println!("\nRun Mode: {}", match Paths::run_mode() {
        cis_core::storage::paths::RunMode::Release => "Release (Portable)",
        cis_core::storage::paths::RunMode::Development => "Development",
    });
}
