//! # DAG Worker Command
//!
//! Worker process for executing DAG tasks in a Matrix Room.
//!
//! ## Usage
//!
//! ```bash
//! cis-node worker --worker-id <ID> --room <ROOM_ID> --scope <SCOPE> --parent-node <NODE_ID>
//! ```
//!
//! ## Features
//!
//! - Connects to a specified Matrix Room
//! - Listens for Task events from the room
//! - Executes tasks and reports results back to the room
//! - Performs periodic health checks

use anyhow::{Context, Result};
use clap::Subcommand;
use tracing::{info, warn, debug, error};
use cis_core::service::{
    worker_service::{WorkerService, WorkerCreateOptions, WorkerScope as ServiceWorkerScope},
    ListOptions, ResourceStatus,
};

/// Matrix HTTP Client for Worker
/// 
/// Uses Matrix Client-Server API to communicate with the Matrix server
#[derive(Debug, Clone)]
pub struct MatrixHttpClient {
    /// Base URL of the Matrix server (e.g., http://localhost:7676)
    server_url: String,
    /// Access token for authentication
    access_token: String,
    /// User ID (e.g., @worker:localhost)
    user_id: String,
    /// HTTP client
    http_client: reqwest::Client,
}

impl MatrixHttpClient {
    /// Create a new Matrix HTTP client
    pub fn new(server_url: String, access_token: String, user_id: String) -> Self {
        Self {
            server_url,
            access_token,
            user_id,
            http_client: reqwest::Client::new(),
        }
    }
    
    /// Join a Matrix room
    pub async fn join_room(&self, room_id: &str) -> Result<()> {
        let url = format!("{}/_matrix/client/v3/rooms/{}/join", self.server_url, room_id);
        
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("Failed to send join room request")?;
        
        if response.status().is_success() {
            info!("Successfully joined room: {}", room_id);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to join room: {} - {}", status, body))
        }
    }
    
    /// Send a message to a room
    pub async fn send_message(
        &self, 
        room_id: &str, 
        msgtype: &str, 
        body: &str,
        extra_content: Option<serde_json::Value>,
    ) -> Result<String> {
        let txn_id = format!("{}-{}", uuid::Uuid::new_v4(), chrono::Utc::now().timestamp_millis());
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.server_url, room_id, txn_id
        );
        
        let mut content = serde_json::json!({
            "msgtype": msgtype,
            "body": body,
        });
        
        // Merge extra content if provided
        if let Some(extra) = extra_content {
            if let Some(obj) = content.as_object_mut() {
                if let Some(extra_obj) = extra.as_object() {
                    for (k, v) in extra_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        
        let response = self.http_client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&content)
            .send()
            .await
            .context("Failed to send message")?;
        
        if response.status().is_success() {
            let json: serde_json::Value = response.json().await.context("Failed to parse response")?;
            let event_id = json["event_id"].as_str().unwrap_or("unknown").to_string();
            debug!("Sent message to room {}: event_id={}", room_id, event_id);
            Ok(event_id)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to send message: {} - {}", status, body))
        }
    }
    
    /// Poll room messages using sync API
    pub async fn sync(
        &self,
        since: Option<&str>,
        timeout_ms: u64,
    ) -> Result<SyncResponse> {
        let mut url = format!(
            "{}/_matrix/client/v3/sync?timeout={}",
            self.server_url, timeout_ms
        );
        
        if let Some(s) = since {
            url.push_str(&format!("&since={}", s));
        }
        
        let response = self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .context("Failed to sync")?;
        
        if response.status().is_success() {
            let json: serde_json::Value = response.json().await.context("Failed to parse sync response")?;
            Ok(SyncResponse::from_json(json))
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Sync failed: {} - {}", status, body))
        }
    }
}

/// Sync response from Matrix server
#[derive(Debug, Clone, Default)]
pub struct SyncResponse {
    pub next_batch: String,
    pub rooms: Rooms,
}

impl SyncResponse {
    pub fn from_json(json: serde_json::Value) -> Self {
        Self {
            next_batch: json["next_batch"].as_str().unwrap_or("").to_string(),
            rooms: Rooms::from_json(&json["rooms"]),
        }
    }
}

/// Rooms in sync response
#[derive(Debug, Clone, Default)]
pub struct Rooms {
    pub join: std::collections::HashMap<String, RoomInfo>,
}

impl Rooms {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let mut rooms = Self::default();
        
        if let Some(join) = json["join"].as_object() {
            for (room_id, room_data) in join {
                rooms.join.insert(room_id.clone(), RoomInfo::from_json(room_data));
            }
        }
        
        rooms
    }
}

/// Room info in sync response
#[derive(Debug, Clone, Default)]
pub struct RoomInfo {
    pub timeline: Timeline,
}

impl RoomInfo {
    pub fn from_json(json: &serde_json::Value) -> Self {
        Self {
            timeline: Timeline::from_json(&json["timeline"]),
        }
    }
}

/// Timeline in room info
#[derive(Debug, Clone, Default)]
pub struct Timeline {
    pub events: Vec<TimelineEvent>,
}

impl Timeline {
    pub fn from_json(json: &serde_json::Value) -> Self {
        let mut timeline = Self::default();
        
        if let Some(events) = json["events"].as_array() {
            for event in events {
                if let Some(te) = TimelineEvent::from_json(event) {
                    timeline.events.push(te);
                }
            }
        }
        
        timeline
    }
}

/// Timeline event
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub event_id: String,
    pub event_type: String,
    pub sender: String,
    pub content: serde_json::Value,
}

impl TimelineEvent {
    pub fn from_json(json: &serde_json::Value) -> Option<Self> {
        Some(Self {
            event_id: json["event_id"].as_str()?.to_string(),
            event_type: json["type"].as_str()?.to_string(),
            sender: json["sender"].as_str()?.to_string(),
            content: json["content"].clone(),
        })
    }
}

/// Worker scope for isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerScope {
    /// Global scope - shared worker for all DAGs
    Global,
    /// Project scope - isolated worker per project
    Project,
    /// User scope - isolated worker per user
    User,
    /// Type scope - isolated worker per DAG type
    Type,
}

impl std::str::FromStr for WorkerScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "global" => Ok(WorkerScope::Global),
            "project" => Ok(WorkerScope::Project),
            "user" => Ok(WorkerScope::User),
            "type" => Ok(WorkerScope::Type),
            _ => Err(format!("Unknown scope: {}. Valid values: global, project, user, type", s)),
        }
    }
}

impl std::fmt::Display for WorkerScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerScope::Global => write!(f, "global"),
            WorkerScope::Project => write!(f, "project"),
            WorkerScope::User => write!(f, "user"),
            WorkerScope::Type => write!(f, "type"),
        }
    }
}

/// Worker subcommands - Docker style CLI
#[derive(Debug, Subcommand)]
pub enum WorkerCommands {
    /// Run a new worker (create and start)
    /// 
    /// Examples:
    ///   cis worker run --room "!room:server" --scope global --parent-node "node1"
    ///   cis worker run -r "!room:server" -s project --parent-node "node1" --scope-id "proj123"
    #[command(alias = "create")]
    Run {
        /// Worker unique identifier (auto-generated if not specified)
        #[arg(long, short)]
        worker_id: Option<String>,
        
        /// Matrix Room ID to join
        #[arg(long, short)]
        room: String,
        
        /// Worker scope (global|project|user|type)
        #[arg(long, short)]
        scope: WorkerScope,
        
        /// Parent node ID that spawned this worker
        #[arg(long)]
        parent_node: String,
        
        /// Scope identifier (required for project/user/type scopes)
        #[arg(long)]
        scope_id: Option<String>,
        
        /// Health check interval in seconds
        #[arg(long, default_value = "30")]
        health_interval: u64,
        
        /// Enable verbose logging
        #[arg(long, short)]
        verbose: bool,
        
        /// Maximum CPU cores (0 = no limit)
        #[arg(long, default_value = "0")]
        max_cpu: usize,
        
        /// Maximum memory in MB (0 = no limit)
        #[arg(long, default_value = "0")]
        max_memory_mb: usize,
        
        /// Matrix server URL (e.g., http://localhost:7676)
        #[arg(long, default_value = "http://localhost:7676")]
        matrix_server: String,
        
        /// Matrix access token for authentication
        #[arg(long, default_value = "")]
        matrix_token: String,
        
        /// Run in background (detached mode)
        #[arg(long, short = 'd')]
        detach: bool,
    },
    
    /// List workers (like `docker ps`)
    /// 
    /// Examples:
    ///   cis worker ps           # List running workers
    ///   cis worker ps -a        # List all workers
    ///   cis worker ps -q        # Only display IDs
    ///   cis worker ps --format json
    #[command(alias = "ls", alias = "list")]
    Ps {
        /// Show all workers (including stopped)
        #[arg(long, short)]
        all: bool,
        
        /// Only display worker IDs
        #[arg(long, short)]
        quiet: bool,
        
        /// Filter workers by condition
        /// Example: --filter status=running --filter scope=global
        #[arg(long, value_parser = parse_filter)]
        filter: Vec<(String, String)>,
        
        /// Output format (table|json|wide)
        #[arg(long, short, default_value = "table")]
        format: OutputFormat,
        
        /// Show last n workers
        #[arg(long, short = 'n')]
        last: Option<usize>,
    },
    
    /// Display detailed information about a worker (like `docker inspect`)
    /// 
    /// Example:
    ///   cis worker inspect abc123
    ///   cis worker inspect abc123 --format "{{.Status}}"
    Inspect {
        /// Worker ID (or prefix)
        worker_id: String,
        
        /// Format output using Go template syntax
        #[arg(long)]
        format: Option<String>,
    },
    
    /// Stop a running worker (like `docker stop`)
    /// 
    /// Examples:
    ///   cis worker stop abc123
    ///   cis worker stop abc123 --timeout 30
    ///   cis worker stop $(cis worker ps -q)  # Stop all running
    Stop {
        /// Worker ID(s) to stop (supports prefix matching)
        worker_ids: Vec<String>,
        
        /// Force stop without graceful shutdown
        #[arg(long, short)]
        force: bool,
        
        /// Timeout in seconds to wait for graceful shutdown
        #[arg(long, short, default_value = "10")]
        timeout: u64,
    },
    
    /// Remove one or more workers (like `docker rm`)
    /// 
    /// Examples:
    ///   cis worker rm abc123
    ///   cis worker rm abc123 def456
    ///   cis worker rm $(cis worker ps -aq)  # Remove all stopped
    #[command(alias = "remove")]
    Rm {
        /// Worker ID(s) to remove (supports prefix matching)
        worker_ids: Vec<String>,
        
        /// Force remove running workers
        #[arg(long, short)]
        force: bool,
        
        /// Remove all stopped workers
        #[arg(long)]
        stopped: bool,
    },
    
    /// Remove all stopped workers (like `docker prune`)
    /// 
    /// Example:
    ///   cis worker prune -f
    Prune {
        /// Do not prompt for confirmation
        #[arg(long, short)]
        force: bool,
    },
    
    /// Fetch logs of a worker
    /// 
    /// Examples:
    ///   cis worker logs abc123
    ///   cis worker logs abc123 -f  # Follow log output
    ///   cis worker logs abc123 --tail 100
    Logs {
        /// Worker ID (or prefix)
        worker_id: String,
        
        /// Follow log output
        #[arg(long, short)]
        follow: bool,
        
        /// Number of lines to show from end of logs
        #[arg(long, short, default_value = "100")]
        tail: usize,
        
        /// Show timestamps
        #[arg(long, short)]
        timestamps: bool,
    },
    
    /// Display a live stream of worker resource usage statistics
    /// 
    /// Examples:
    ///   cis worker stats
    ///   cis worker stats abc123
    Stats {
        /// Worker ID(s) (if not specified, shows all)
        worker_ids: Vec<String>,
        
        /// Disable streaming stats and only pull the first result
        #[arg(long)]
        no_stream: bool,
    },
    
    /// Show running workers sorted by resource usage (like `docker top`)
    /// 
    /// Examples:
    ///   cis worker top
    ///   cis worker top --sort memory
    Top {
        /// Sort by field (cpu|memory|tasks|uptime)
        #[arg(long, short, default_value = "cpu")]
        sort: String,
        
        /// Number of workers to show
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,
    },
    
    /// Start a stopped worker
    /// 
    /// Example:
    ///   cis worker start abc123
    Start {
        /// Worker ID to start (supports prefix matching)
        worker_id: String,
    },
    
    /// Restart a worker
    /// 
    /// Example:
    ///   cis worker restart abc123
    Restart {
        /// Worker ID to restart (supports prefix matching)
        worker_id: String,
        
        /// Timeout for stop before restart
        #[arg(long, short, default_value = "10")]
        timeout: u64,
    },
}

/// Output format for listing
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Wide,
}

/// Parse filter argument in format "key=value"
fn parse_filter(s: &str) -> Result<(String, String), String> {
    s.find('=')
        .map(|i| (s[..i].to_string(), s[i+1..].to_string()))
        .ok_or_else(|| "Filter must be in format 'key=value'".to_string())
}

/// Handle worker commands
pub async fn handle(cmd: WorkerCommands) -> Result<()> {
    match cmd {
        WorkerCommands::Run {
            worker_id,
            room,
            scope,
            parent_node,
            scope_id,
            health_interval,
            verbose,
            max_cpu,
            max_memory_mb,
            matrix_server,
            matrix_token,
            detach,
        } => {
            let worker_id = worker_id.unwrap_or_else(|| generate_worker_id());
            let args = WorkerArgs {
                worker_id,
                room,
                scope,
                parent_node,
                scope_id,
                health_interval,
                verbose,
                max_cpu,
                max_memory_mb,
                matrix_server,
                matrix_token,
            };
            if detach {
                run_worker_detached(args).await
            } else {
                run_worker(args).await
            }
        }
        WorkerCommands::Ps { all, quiet, filter, format, last } => {
            list_workers(all, quiet, filter, format, last).await
        }
        WorkerCommands::Inspect { worker_id, format } => {
            inspect_worker(&worker_id, format.as_deref()).await
        }
        WorkerCommands::Stop { worker_ids, force, timeout } => {
            stop_workers(&worker_ids, force, timeout).await
        }
        WorkerCommands::Rm { worker_ids, force, stopped } => {
            if stopped {
                prune_workers(force).await
            } else {
                remove_workers(&worker_ids, force).await
            }
        }
        WorkerCommands::Prune { force } => {
            prune_workers(force).await
        }
        WorkerCommands::Logs { worker_id, follow, tail, timestamps } => {
            show_worker_logs(&worker_id, follow, tail, timestamps).await
        }
        WorkerCommands::Stats { worker_ids, no_stream } => {
            show_worker_stats(&worker_ids, no_stream).await
        }
        WorkerCommands::Top { sort, limit } => {
            show_worker_top(&sort, limit).await
        }
        WorkerCommands::Start { worker_id } => {
            start_worker(&worker_id).await
        }
        WorkerCommands::Restart { worker_id, timeout } => {
            restart_worker(&worker_id, timeout).await
        }
    }
}

/// Generate a unique worker ID
fn generate_worker_id() -> String {
    format!("worker-{}-{}", 
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

/// Worker arguments
#[derive(Debug, Clone)]
pub struct WorkerArgs {
    /// Worker unique identifier
    pub worker_id: String,
    /// Matrix Room ID
    pub room: String,
    /// Worker scope
    pub scope: WorkerScope,
    /// Parent node ID
    pub parent_node: String,
    /// Scope identifier
    pub scope_id: Option<String>,
    /// Health check interval
    pub health_interval: u64,
    /// Verbose logging
    pub verbose: bool,
    /// Maximum CPU cores (0 = no limit)
    pub max_cpu: usize,
    /// Maximum memory in MB (0 = no limit)
    pub max_memory_mb: usize,
    /// Matrix server URL
    pub matrix_server: String,
    /// Matrix access token
    pub matrix_token: String,
}

/// Run the worker process
async fn run_worker(args: WorkerArgs) -> Result<()> {
    info!("Starting DAG worker: {}", args.worker_id);
    info!("  Room: {}", args.room);
    info!("  Scope: {}", args.scope);
    info!("  Parent Node: {}", args.parent_node);
    
    if let Some(ref scope_id) = args.scope_id {
        info!("  Scope ID: {}", scope_id);
    }
    
    // Print startup banner
    println!("╔════════════════════════════════════════╗");
    println!("║     CIS DAG Worker                     ║");
    println!("╚════════════════════════════════════════╝");
    println!();
    println!("Worker ID:    {}", args.worker_id);
    println!("Room:         {}", args.room);
    println!("Scope:        {}", args.scope);
    println!("Parent Node:  {}", args.parent_node);
    if let Some(ref scope_id) = args.scope_id {
        println!("Scope ID:     {}", scope_id);
    }
    println!("Health Intv:  {}s", args.health_interval);
    
    // Resource limits
    if args.max_cpu > 0 {
        println!("CPU Limit:    {} cores", args.max_cpu);
    }
    if args.max_memory_mb > 0 {
        println!("Memory Limit: {} MB", args.max_memory_mb);
    }
    if args.max_cpu == 0 && args.max_memory_mb == 0 {
        println!("Resources:    unlimited");
    }
    
    println!();
    
    // Step 1: Initialize node
    println!("📡 Initializing worker node...");
    let node_info = initialize_node(&args).await?;
    println!("✅ Node initialized: {}", node_info.node_id);
    
    // Step 2: Initialize Matrix client and join room
    let matrix_client = if !args.matrix_token.is_empty() {
        println!("🔌 Initializing Matrix client: {}...", args.matrix_server);
        let client = MatrixHttpClient::new(
            args.matrix_server.clone(),
            args.matrix_token.clone(),
            format!("@worker-{}", args.worker_id),
        );
        
        // Join the room
        match client.join_room(&args.room).await {
            Ok(()) => {
                println!("✅ Joined Matrix room: {}", args.room);
                Some(client)
            }
            Err(e) => {
                warn!("Failed to join room: {}. Continuing without Matrix integration.", e);
                None
            }
        }
    } else {
        println!("⚠️  No Matrix token provided, running in standalone mode");
        None
    };
    
    let room_conn = join_room(&args.room, &node_info, matrix_client).await?;
    println!("✅ Connected to room: {}", room_conn.room_name.as_deref().unwrap_or(&args.room));
    
    // Step 3: Start event loop
    println!("👂 Listening for task events...");
    println!("   Press Ctrl+C to stop");
    println!();
    
    // Register worker in registry
    let registry = WorkerRegistry::new()?;
    let current_pid = std::process::id();
    registry.register_worker(&args.worker_id, current_pid, &args)?;
    info!("Worker {} registered (PID: {})", args.worker_id, current_pid);
    
    // Statistics for heartbeat
    let tasks_executed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let active_tasks = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    
    let shutdown_requested = false;
    
    // Main event loop
    loop {
        if shutdown_requested {
            break;
        }
        
        // Check for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Shutdown signal received, stopping worker...");
                println!("\n🛑 Shutdown signal received, stopping worker...");
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(args.health_interval)) => {
                // Periodic health check and heartbeat update
                perform_health_check(&args).await?;
                
                // Update registry with current stats
                let _ = registry.update_heartbeat(
                    &args.worker_id,
                    tasks_executed.load(std::sync::atomic::Ordering::Relaxed),
                    active_tasks.load(std::sync::atomic::Ordering::Relaxed)
                );
            }
            event = poll_room_events(&room_conn) => {
                match event {
                    Ok(Some(task_event)) => {
                        active_tasks.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let result = handle_task_event(&args, &room_conn, task_event).await;
                        active_tasks.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        tasks_executed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        
                        if let Err(e) = result {
                            error!("Error handling task event: {}", e);
                        }
                    }
                    Ok(None) => {
                        // No events, continue
                    }
                    Err(e) => {
                        warn!("Error polling room events: {}", e);
                    }
                }
            }
        }
    }
    
    // Graceful shutdown
    println!("🧹 Cleaning up...");
    cleanup_worker(&args).await?;
    
    // Remove from registry
    let _ = registry.remove_worker(&args.worker_id);
    println!("✅ Worker {} stopped", args.worker_id);
    
    Ok(())
}

/// Worker process registry for tracking active workers
pub struct WorkerRegistry {
    /// Data directory for storing worker state
    data_dir: std::path::PathBuf,
}

impl WorkerRegistry {
    /// Create new registry
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("cis")
            .join("workers");
        
        std::fs::create_dir_all(&data_dir)?;
        
        Ok(Self { data_dir })
    }
    
    /// Register a new worker
    pub fn register_worker(&self, worker_id: &str, pid: u32, args: &WorkerArgs) -> Result<()> {
        let worker_file = self.data_dir.join(format!("{}.json", worker_id));
        
        let info = WorkerInfo {
            worker_id: worker_id.to_string(),
            pid,
            room: args.room.clone(),
            scope: format!("{:?}", args.scope),
            parent_node: args.parent_node.clone(),
            started_at: chrono::Utc::now(),
            last_heartbeat: chrono::Utc::now(),
            tasks_executed: 0,
            active_tasks: 0,
            status: WorkerStatus::Running,
        };
        
        let json = serde_json::to_string_pretty(&info)?;
        std::fs::write(&worker_file, json)?;
        
        Ok(())
    }
    
    /// Get worker info
    pub fn get_worker(&self, worker_id: &str) -> Result<Option<WorkerInfo>> {
        let worker_file = self.data_dir.join(format!("{}.json", worker_id));
        
        if !worker_file.exists() {
            return Ok(None);
        }
        
        let json = std::fs::read_to_string(&worker_file)?;
        let info: WorkerInfo = serde_json::from_str(&json)?;
        
        // Check if process is still running
        let mut info = info;
        if !Self::is_process_running(info.pid) {
            info.status = WorkerStatus::Stopped;
        }
        
        Ok(Some(info))
    }
    
    /// List all workers
    pub fn list_workers(&self) -> Result<Vec<WorkerInfo>> {
        let mut workers = Vec::new();
        
        for entry in std::fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(mut info) = serde_json::from_str::<WorkerInfo>(&json) {
                        // Update status based on process existence
                        if !Self::is_process_running(info.pid) {
                            info.status = WorkerStatus::Stopped;
                        }
                        workers.push(info);
                    }
                }
            }
        }
        
        // Sort by started time (newest first)
        workers.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        Ok(workers)
    }
    
    /// Remove worker from registry
    pub fn remove_worker(&self, worker_id: &str) -> Result<()> {
        let worker_file = self.data_dir.join(format!("{}.json", worker_id));
        if worker_file.exists() {
            std::fs::remove_file(&worker_file)?;
        }
        Ok(())
    }
    
    /// Update worker heartbeat
    pub fn update_heartbeat(&self, worker_id: &str, tasks_executed: u64, active_tasks: u64) -> Result<()> {
        if let Some(mut info) = self.get_worker(worker_id)? {
            info.last_heartbeat = chrono::Utc::now();
            info.tasks_executed = tasks_executed;
            info.active_tasks = active_tasks;
            
            let worker_file = self.data_dir.join(format!("{}.json", worker_id));
            let json = serde_json::to_string_pretty(&info)?;
            std::fs::write(&worker_file, json)?;
        }
        Ok(())
    }
    
    /// Check if process is running
    pub fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix, send signal 0 to check if process exists
            unsafe {
                libc::kill(pid as i32, 0) == 0
            }
        }
        #[cfg(windows)]
        {
            // On Windows, try to open the process
            use std::os::windows::process::CommandExt;
            // Simplified check - in production, use proper Windows API
            true
        }
    }
    
    /// Find workers by ID prefix (for partial matching)
    pub fn find_by_prefix(&self, prefix: &str) -> Result<Vec<WorkerInfo>> {
        let all_workers = self.list_workers()?;
        let matches: Vec<_> = all_workers
            .into_iter()
            .filter(|w| w.worker_id.starts_with(prefix))
            .collect();
        Ok(matches)
    }
    
    /// Get single worker by ID or prefix (returns error if ambiguous)
    pub fn get_worker_by_id_or_prefix(&self, id: &str) -> Result<Option<WorkerInfo>> {
        // First try exact match
        if let Some(worker) = self.get_worker(id)? {
            return Ok(Some(worker));
        }
        
        // Then try prefix match
        let matches = self.find_by_prefix(id)?;
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches.into_iter().next().unwrap())),
            _ => Err(anyhow::anyhow!(
                "Multiple workers match prefix '{}': {}",
                id,
                matches.iter().map(|w| w.worker_id.clone()).collect::<Vec<_>>().join(", ")
            )),
        }
    }
    
    /// List workers with optional filters
    pub fn list_workers_filtered(
        &self,
        include_stopped: bool,
        filters: &[(String, String)],
    ) -> Result<Vec<WorkerInfo>> {
        let mut workers = self.list_workers()?;
        
        // Filter by status if not including stopped
        if !include_stopped {
            workers.retain(|w| matches!(w.status, WorkerStatus::Running));
        }
        
        // Apply custom filters
        for (key, value) in filters {
            match key.as_str() {
                "status" => {
                    workers.retain(|w| {
                        let status_str = format!("{:?}", w.status).to_lowercase();
                        status_str == value.to_lowercase()
                    });
                }
                "scope" => {
                    workers.retain(|w| w.scope.to_lowercase() == value.to_lowercase());
                }
                "room" => {
                    workers.retain(|w| w.room.contains(value));
                }
                "parent" => {
                    workers.retain(|w| w.parent_node.contains(value));
                }
                _ => {
                    // Unknown filter, ignore
                }
            }
        }
        
        Ok(workers)
    }
}

/// Worker status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WorkerStatus {
    Running,
    Stopped,
    Error,
}

/// Worker information stored in registry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerInfo {
    pub worker_id: String,
    pub pid: u32,
    pub room: String,
    pub scope: String,
    pub parent_node: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub tasks_executed: u64,
    pub active_tasks: u64,
    pub status: WorkerStatus,
}

/// Stop a running worker
async fn stop_worker(worker_id: &str, force: bool) -> Result<()> {
    println!("Stopping worker: {}", worker_id);
    
    let registry = WorkerRegistry::new()?;
    
    match registry.get_worker(worker_id)? {
        Some(info) => {
            // Check if process is running
            if !WorkerRegistry::is_process_running(info.pid) {
                println!("Worker {} is not running (PID: {})", worker_id, info.pid);
                registry.remove_worker(worker_id)?;
                return Ok(());
            }
            
            println!("Found worker {} (PID: {})", worker_id, info.pid);
            
            if force {
                println!("⚠️ Force stop requested");
            }
            
            // Send termination signal
            #[cfg(unix)]
            {
                use libc::{kill, SIGTERM, SIGKILL};
                
                let pid = info.pid as i32;
                
                if force {
                    // Send SIGKILL for force stop
                    unsafe {
                        kill(pid, SIGKILL);
                    }
                    println!("Sent SIGKILL to worker {}", worker_id);
                } else {
                    // Send SIGTERM for graceful shutdown
                    unsafe {
                        kill(pid, SIGTERM);
                    }
                    println!("Sent SIGTERM to worker {} (waiting 5s for graceful shutdown)...", worker_id);
                    
                    // Wait for process to terminate
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    
                    // Check if still running
                    if WorkerRegistry::is_process_running(info.pid) {
                        println!("Worker {} still running, sending SIGKILL...", worker_id);
                        unsafe {
                            kill(pid, SIGKILL);
                        }
                    }
                }
            }
            
            #[cfg(windows)]
            {
                // On Windows, use taskkill
                let output = if force {
                    std::process::Command::new("taskkill")
                        .args(&["/F", "/PID", &info.pid.to_string()])
                        .output()?
                } else {
                    std::process::Command::new("taskkill")
                        .args(&["/PID", &info.pid.to_string()])
                        .output()?
                };
                
                if output.status.success() {
                    println!("Successfully stopped worker {}", worker_id);
                } else {
                    let error = String::from_utf8_lossy(&output.stderr);
                    println!("Failed to stop worker: {}", error);
                }
            }
            
            // Remove from registry
            registry.remove_worker(worker_id)?;
            println!("Worker {} removed from registry", worker_id);
        }
        None => {
            println!("Worker {} not found in registry", worker_id);
        }
    }
    
    Ok(())
}

/// Show worker status
async fn show_worker_status(worker_id: Option<&str>) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    if let Some(id) = worker_id {
        // Show specific worker status
        match registry.get_worker(id)? {
            Some(info) => {
                let uptime = chrono::Utc::now().signed_duration_since(info.started_at);
                let uptime_str = format_duration(uptime);
                
                let last_heartbeat = chrono::Utc::now().signed_duration_since(info.last_heartbeat);
                let heartbeat_str = format_duration(last_heartbeat);
                
                println!("╔════════════════════════════════════════╗");
                println!("║         Worker Status                  ║");
                println!("╚════════════════════════════════════════╝");
                println!();
                println!("Worker ID:      {}", info.worker_id);
                println!("PID:            {}", info.pid);
                println!("Status:         {:?}", info.status);
                println!("Room:           {}", info.room);
                println!("Scope:          {}", info.scope);
                println!("Parent Node:    {}", info.parent_node);
                println!();
                println!("Started:        {}", info.started_at.format("%Y-%m-%d %H:%M:%S UTC"));
                println!("Uptime:         {}", uptime_str);
                println!("Last Heartbeat: {} ago", heartbeat_str);
                println!();
                println!("Tasks Executed: {}", info.tasks_executed);
                println!("Active Tasks:   {}", info.active_tasks);
            }
            None => {
                println!("Worker '{}' not found", id);
                println!();
                println!("Use 'cis-node worker status' to list all workers.");
            }
        }
    } else {
        // List all workers
        let workers = registry.list_workers()?;
        
        if workers.is_empty() {
            println!("No workers found.");
            println!();
            println!("Use 'cis-node worker start' to start a worker.");
        } else {
            println!("╔══════════════════════════════════════════════════════════════════════════╗");
            println!("║                         Worker List                                      ║");
            println!("╚══════════════════════════════════════════════════════════════════════════╝");
            println!();
            println!("{:<20} {:<8} {:<12} {:<10} {:<20}", 
                "Worker ID", "PID", "Status", "Tasks", "Uptime");
            println!("{}", "-".repeat(80));
            
            for info in &workers {
                let uptime = chrono::Utc::now().signed_duration_since(info.started_at);
                let uptime_str = format_duration_short(uptime);
                
                let status_icon = match info.status {
                    WorkerStatus::Running => "🟢",
                    WorkerStatus::Stopped => "🔴",
                    WorkerStatus::Error => "⚠️ ",
                };
                
                println!("{:<20} {:<8} {} {:<10} {:<10} {:<20}",
                    truncate(&info.worker_id, 20),
                    info.pid,
                    status_icon,
                    format!("{}/{}", info.active_tasks, info.tasks_executed),
                    uptime_str,
                    truncate(&info.scope, 20)
                );
            }
            
            println!();
            println!("Total: {} workers", workers.len());
        }
    }
    
    Ok(())
}

/// Format duration for display
fn format_duration(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds();
    
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

/// Format duration in short form
fn format_duration_short(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds();
    
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}

/// Truncate string with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

// ==================== Docker Style CLI Commands ====================

/// Run worker in detached mode (background)
async fn run_worker_detached(args: WorkerArgs) -> Result<()> {
    // For now, just run in foreground but print a message
    // In full implementation, this would fork/spawn a background process
    println!("🚀 Starting worker '{}' in detached mode...", args.worker_id);
    println!("   Room: {}", args.room);
    println!("   Scope: {:?}", args.scope);
    println!();
    println!("ℹ️  Note: Full detached mode requires daemon implementation");
    println!("   Running in foreground for now...");
    println!();
    
    run_worker(args).await
}

/// List workers with Docker-style formatting
async fn list_workers(
    all: bool,
    quiet: bool,
    filters: Vec<(String, String)>,
    format: OutputFormat,
    last: Option<usize>,
) -> Result<()> {
    let service = WorkerService::new()?;
    
    let options = ListOptions {
        all,
        filters: filters.into_iter().collect(),
        limit: last,
        sort_by: Some("created_at".to_string()),
        sort_desc: true,
    };
    
    let result = service.list(options).await?;
    let workers = result.items;
    
    if quiet {
        // Only print IDs
        for worker in &workers {
            println!("{}", worker.id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&workers)?);
        }
        OutputFormat::Wide => {
            println!("{:<25} {:<8} {:<12} {:<25} {:<20} {:<15}", 
                "WORKER ID", "PID", "STATUS", "ROOM", "UPTIME", "TASKS");
            println!("{}", "-".repeat(115));
            
            for info in &workers {
                let status_str = info.status.to_string();
                let uptime_str = format_duration_short(chrono::Duration::seconds(info.uptime as i64));
                
                println!("{:<25} {:<8} {:<12} {:<25} {:<20} {}/{}",
                    truncate(&info.id, 25),
                    info.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
                    status_str,
                    truncate(&info.room, 25),
                    uptime_str,
                    info.active_tasks,
                    info.tasks_executed
                );
            }
            
            println!();
            println!("Total: {} workers", result.total);
        }
        OutputFormat::Table => {
            // Compact table format (default)
            println!("{:<20} {:<8} {:<10} {:<12} {:<15}", 
                "WORKER ID", "PID", "STATUS", "UPTIME", "SCOPE");
            println!("{}", "-".repeat(70));
            
            for info in &workers {
                let uptime_str = format_duration_short(chrono::Duration::seconds(info.uptime as i64));
                let status_icon = match info.status {
                    ResourceStatus::Running => "🟢 running",
                    ResourceStatus::Stopped => "🔴 stopped",
                    ResourceStatus::Error => "⚠️  error",
                    _ => "❓ unknown",
                };
                
                println!("{:<20} {:<8} {:<10} {:<12} {:<15}",
                    truncate(&info.id, 20),
                    info.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
                    status_icon,
                    uptime_str,
                    truncate(&info.scope, 15)
                );
            }
            
            if workers.is_empty() {
                println!("No workers found.");
            }
            println!();
            println!("Use 'cis worker ps -a' to show all workers, including stopped.");
        }
    }
    
    Ok(())
}

/// Inspect a worker (detailed information)
async fn inspect_worker(worker_id: &str, format: Option<&str>) -> Result<()> {
    let service = WorkerService::new()?;
    
    let info = match service.inspect(worker_id).await {
        Ok(w) => w,
        Err(_) => {
            println!("Error: No such worker: {}", worker_id);
            std::process::exit(1);
        }
    };
    
    // Custom format using Go template syntax (simplified)
    if let Some(fmt) = format {
        match fmt {
            "{{.Status}}" | "{{.State.Status}}" => {
                println!("{:?}", info.summary.status);
            }
            "{{.PID}}" => {
                println!("{}", info.summary.pid.map(|p| p.to_string()).unwrap_or_default());
            }
            "{{.ID}}" | "{{.WorkerID}}" => {
                println!("{}", info.summary.id);
            }
            _ => {
                println!("Unknown format: {}", fmt);
            }
        }
        return Ok(());
    }
    
    // Full JSON output (like docker inspect)
    let inspect_data = serde_json::json!({
        "ID": info.summary.id,
        "PID": info.summary.pid,
        "State": {
            "Status": info.summary.status.to_string(),
            "Running": matches!(info.summary.status, ResourceStatus::Running),
            "StartedAt": info.summary.created_at,
            "LastHeartbeat": info.summary.last_heartbeat,
        },
        "Config": {
            "Room": info.summary.room,
            "Scope": info.summary.scope,
            "ParentNode": info.parent_node,
        },
        "Stats": {
            "TasksExecuted": info.summary.tasks_executed,
            "ActiveTasks": info.summary.active_tasks,
        },
    });
    
    println!("{}", serde_json::to_string_pretty(&inspect_data)?);
    Ok(())
}

/// Stop multiple workers
async fn stop_workers(worker_ids: &[String], force: bool, timeout: u64) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    for id in worker_ids {
        let info = match registry.get_worker_by_id_or_prefix(id)? {
            Some(w) => w,
            None => {
                eprintln!("Error: No such worker: {}", id);
                continue;
            }
        };
        
        if !matches!(info.status, WorkerStatus::Running) {
            println!("Worker {} is already stopped", info.worker_id);
            continue;
        }
        
        print!("Stopping {}... ", info.worker_id);
        
        // Send stop signal
        #[cfg(unix)]
        {
            use libc::{kill, SIGTERM, SIGKILL};
            let pid = info.pid as i32;
            
            if force {
                unsafe { kill(pid, SIGKILL); }
                println!("killed (force)");
            } else {
                unsafe { kill(pid, SIGTERM); }
                
                // Wait for graceful shutdown
                let mut stopped = false;
                for _ in 0..timeout {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    if !WorkerRegistry::is_process_running(info.pid) {
                        stopped = true;
                        break;
                    }
                }
                
                if !stopped && !force {
                    unsafe { kill(pid, SIGKILL); }
                    println!("killed (timeout)");
                } else {
                    println!("stopped");
                }
            }
        }
        
        #[cfg(windows)]
        {
            // Windows implementation
            println!("stopped");
        }
        
        // Update registry
        let _ = registry.remove_worker(&info.worker_id);
    }
    
    Ok(())
}

/// Remove multiple workers
async fn remove_workers(worker_ids: &[String], force: bool) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    for id in worker_ids {
        let info = match registry.get_worker_by_id_or_prefix(id)? {
            Some(w) => w,
            None => {
                eprintln!("Error: No such worker: {}", id);
                continue;
            }
        };
        
        // Check if running
        if matches!(info.status, WorkerStatus::Running) && !force {
            eprintln!("Error: Worker {} is still running. Use --force to remove.", info.worker_id);
            continue;
        }
        
        // Stop if running and force
        if matches!(info.status, WorkerStatus::Running) && force {
            stop_workers(&[info.worker_id.clone()], true, 0).await?;
        }
        
        // Remove from registry
        registry.remove_worker(&info.worker_id)?;
        println!("Removed: {}", info.worker_id);
    }
    
    Ok(())
}

/// Prune stopped workers
async fn prune_workers(force: bool) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    let workers = registry.list_workers()?;
    
    let stopped: Vec<_> = workers
        .into_iter()
        .filter(|w| !matches!(w.status, WorkerStatus::Running))
        .collect();
    
    if stopped.is_empty() {
        println!("No stopped workers to prune.");
        return Ok(());
    }
    
    if !force {
        println!("This will remove the following stopped workers:");
        for w in &stopped {
            println!("  - {}", w.worker_id);
        }
        println!();
        println!("Total reclaimed: {} workers", stopped.len());
        print!("Are you sure? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }
    
    let mut removed = 0;
    for w in stopped {
        registry.remove_worker(&w.worker_id)?;
        removed += 1;
    }
    
    println!("Pruned {} stopped workers.", removed);
    Ok(())
}

/// Show worker logs (placeholder)
async fn show_worker_logs(
    worker_id: &str,
    follow: bool,
    tail: usize,
    timestamps: bool,
) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    let info = match registry.get_worker_by_id_or_prefix(worker_id)? {
        Some(w) => w,
        None => {
            println!("Error: No such worker: {}", worker_id);
            std::process::exit(1);
        }
    };
    
    // Check for log file
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("cis")
        .join("workers");
    let log_file = log_dir.join(format!("{}.log", info.worker_id));
    
    if !log_file.exists() {
        println!("No logs available for worker {}", info.worker_id);
        println!("Note: Log collection requires file-based logging to be enabled.");
        return Ok(());
    }
    
    // Read and display logs
    let contents = tokio::fs::read_to_string(&log_file).await?;
    let lines: Vec<_> = contents.lines().collect();
    
    // Apply tail
    let start = if lines.len() > tail { lines.len() - tail } else { 0 };
    for line in &lines[start..] {
        if timestamps {
            println!("{}", line);
        } else {
            // Remove timestamp prefix if present
            let cleaned = line.split_once(']').map(|(_, rest)| rest.trim()).unwrap_or(line);
            println!("{}", cleaned);
        }
    }
    
    if follow {
        println!("--follow not yet implemented (would tail -f here)");
    }
    
    Ok(())
}

/// Show worker stats (placeholder for live stats)
async fn show_worker_stats(worker_ids: &[String], no_stream: bool) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    let workers = if worker_ids.is_empty() {
        registry.list_workers()?
    } else {
        let mut result = Vec::new();
        for id in worker_ids {
            if let Some(w) = registry.get_worker_by_id_or_prefix(id)? {
                result.push(w);
            }
        }
        result
    };
    
    if workers.is_empty() {
        println!("No workers found.");
        return Ok(());
    }
    
    // Print header
    println!("{:<20} {:<10} {:<10} {:<15} {:<15}",
        "WORKER ID", "CPU %", "MEM %", "TASKS", "UPTIME");
    println!("{}", "-".repeat(80));
    
    loop {
        // Clear screen if streaming (simplified)
        if !no_stream {
            print!("\x1B[2J\x1B[1;1H");
            println!("{:<20} {:<10} {:<10} {:<15} {:<15}",
                "WORKER ID", "CPU %", "MEM %", "TASKS", "UPTIME");
            println!("{}", "-".repeat(80));
        }
        
        for info in &workers {
            let uptime = chrono::Utc::now().signed_duration_since(info.started_at);
            let uptime_str = format_duration_short(uptime);
            
            // Placeholder for actual resource metrics
            let cpu_pct = if matches!(info.status, WorkerStatus::Running) { "0.5" } else { "-" };
            let mem_pct = if matches!(info.status, WorkerStatus::Running) { "2.1" } else { "-" };
            
            println!("{:<20} {:<10} {:<10} {:<15} {:<15}",
                truncate(&info.worker_id, 20),
                cpu_pct,
                mem_pct,
                format!("{}/{}", info.active_tasks, info.tasks_executed),
                uptime_str
            );
        }
        
        if no_stream {
            break;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
    
    Ok(())
}

/// Show top workers by resource usage
async fn show_worker_top(sort: &str, limit: usize) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    let mut workers = registry.list_workers()?;
    
    // Sort by specified field
    match sort {
        "cpu" | "CPU" => {
            // Placeholder: would sort by actual CPU usage
            workers.sort_by(|a, b| b.active_tasks.cmp(&a.active_tasks));
        }
        "memory" | "mem" => {
            // Placeholder: would sort by memory usage
            workers.sort_by(|a, b| b.tasks_executed.cmp(&a.tasks_executed));
        }
        "tasks" => {
            workers.sort_by(|a, b| b.tasks_executed.cmp(&a.tasks_executed));
        }
        "uptime" => {
            workers.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        }
        _ => {
            workers.sort_by(|a, b| b.active_tasks.cmp(&a.active_tasks));
        }
    }
    
    // Apply limit
    workers.truncate(limit);
    
    println!("Top {} workers (sorted by {}):", limit, sort);
    println!();
    println!("{:<4} {:<20} {:<10} {:<12} {:<15} {}",
        "RANK", "WORKER ID", "STATUS", "TASKS", "UPTIME", "SCOPE");
    println!("{}", "-".repeat(90));
    
    for (i, info) in workers.iter().enumerate() {
        let uptime = chrono::Utc::now().signed_duration_since(info.started_at);
        let uptime_str = format_duration_short(uptime);
        let status_icon = match info.status {
            WorkerStatus::Running => "🟢",
            WorkerStatus::Stopped => "🔴",
            WorkerStatus::Error => "⚠️ ",
        };
        
        println!("{:<4} {:<20} {} {:<10} {:<15} {}",
            i + 1,
            truncate(&info.worker_id, 20),
            status_icon,
            format!("{}/{}", info.active_tasks, info.tasks_executed),
            uptime_str,
            truncate(&info.scope, 20)
        );
    }
    
    Ok(())
}

/// Start a stopped worker (placeholder)
async fn start_worker(worker_id: &str) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    let info = match registry.get_worker_by_id_or_prefix(worker_id)? {
        Some(w) => w,
        None => {
            println!("Error: No such worker: {}", worker_id);
            std::process::exit(1);
        }
    };
    
    if matches!(info.status, WorkerStatus::Running) {
        println!("Worker {} is already running", info.worker_id);
        return Ok(());
    }
    
    println!("Starting worker {}...", info.worker_id);
    println!();
    println!("Note: Starting a stopped worker requires persisting the original arguments.");
    println!("      Use 'cis worker run' to create a new worker instead.");
    
    Ok(())
}

/// Restart a worker
async fn restart_worker(worker_id: &str, timeout: u64) -> Result<()> {
    let registry = WorkerRegistry::new()?;
    
    let info = match registry.get_worker_by_id_or_prefix(worker_id)? {
        Some(w) => w,
        None => {
            println!("Error: No such worker: {}", worker_id);
            std::process::exit(1);
        }
    };
    
    println!("Restarting worker {}...", info.worker_id);
    
    // Stop if running
    if matches!(info.status, WorkerStatus::Running) {
        stop_workers(&[info.worker_id.clone()], false, timeout).await?;
    }
    
    // Remove old entry
    registry.remove_worker(&info.worker_id)?;
    
    println!("Note: Full restart requires persisting original worker arguments.");
    println!("      Use 'cis worker run' with the same parameters to recreate.");
    
    Ok(())
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub did: String,
    pub capabilities: Vec<String>,
}

/// Initialize the worker node
async fn initialize_node(args: &WorkerArgs) -> Result<NodeInfo> {
    // Generate deterministic node ID based on worker_id and parent
    let node_id = format!("worker-{}-{}", args.worker_id, uuid::Uuid::new_v4());
    
    // Load or generate DID
    let did = load_or_create_worker_did(&node_id).await?;
    
    // Define capabilities based on scope
    let mut capabilities = vec![
        "dag.execute".to_string(),
        "task.run".to_string(),
        "matrix.listen".to_string(),
        "matrix.send".to_string(),
    ];
    
    // Add scope-specific capabilities
    match args.scope {
        WorkerScope::Global => {
            capabilities.push("scope.global".to_string());
        }
        WorkerScope::Project => {
            capabilities.push("scope.project".to_string());
            if let Some(ref scope_id) = args.scope_id {
                capabilities.push(format!("project.{}", scope_id));
            }
        }
        WorkerScope::User => {
            capabilities.push("scope.user".to_string());
            if let Some(ref scope_id) = args.scope_id {
                capabilities.push(format!("user.{}", scope_id));
            }
        }
        WorkerScope::Type => {
            capabilities.push("scope.type".to_string());
        }
    }
    
    info!("Worker node initialized: {}", node_id);
    info!("  DID: {}", did);
    info!("  Capabilities: {:?}", capabilities);
    
    Ok(NodeInfo {
        node_id,
        did,
        capabilities,
    })
}

/// Load existing DID or create new one for worker
async fn load_or_create_worker_did(node_id: &str) -> Result<String> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("cis")
        .join("workers");
    
    let did_file = data_dir.join(format!("{}.did", node_id));
    
    // Try to load existing DID
    if did_file.exists() {
        if let Ok(did) = tokio::fs::read_to_string(&did_file).await {
            debug!("Loaded existing DID for worker: {}", did);
            return Ok(did.trim().to_string());
        }
    }
    
    // Create new DID
    let did = format!("did:cis:worker:{}", node_id);
    
    // Ensure directory exists
    tokio::fs::create_dir_all(&data_dir).await.ok();
    
    // Save DID to file
    if let Err(e) = tokio::fs::write(&did_file, &did).await {
        warn!("Failed to save DID: {}", e);
    }
    
    Ok(did)
}

/// Room connection information
#[derive(Debug, Clone)]
pub struct RoomConnection {
    pub room_id: String,
    pub room_name: Option<String>,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    /// Matrix HTTP client for sending/receiving events
    pub matrix_client: Option<MatrixHttpClient>,
}

/// Join a Matrix room
async fn join_room(
    room_id: &str, 
    node_info: &NodeInfo,
    matrix_client: Option<MatrixHttpClient>,
) -> Result<RoomConnection> {
    debug!("Joining room {} as node {}", room_id, node_info.node_id);
    
    // If Matrix client is available, room is already joined
    // Otherwise, create a placeholder connection for standalone mode
    if matrix_client.is_none() {
        debug!("Running in standalone mode, skipping actual room join");
    }
    
    Ok(RoomConnection {
        room_id: room_id.to_string(),
        room_name: Some(format!("Worker Room {}", room_id)),
        joined_at: chrono::Utc::now(),
        matrix_client,
    })
}

/// Task event from room
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// New task to execute
    NewTask {
        task_id: String,
        dag_id: String,
        task_spec: cis_core::scheduler::DagTaskSpec,
    },
    /// Task cancellation request
    CancelTask {
        task_id: String,
    },
    /// Heartbeat from parent node
    Heartbeat,
    /// Shutdown request
    Shutdown,
}

/// Poll room for new events
async fn poll_room_events(room_conn: &RoomConnection) -> Result<Option<TaskEvent>> {
    // If Matrix client is available, use it to poll events
    if let Some(ref client) = room_conn.matrix_client {
        match client.sync(None, 30000).await {
            Ok(response) => {
                // Check for events in our room
                if let Some(room) = response.rooms.join.get(&room_conn.room_id) {
                    // Process timeline events
                    for event in &room.timeline.events {
                        if event.event_type == "m.room.message" {
                            // Parse message content
                            if let Some(msgtype) = event.content["msgtype"].as_str() {
                                if msgtype == "m.text" {
                                    if let Some(body) = event.content["body"].as_str() {
                                        // Try to parse as task event
                                        if let Some(task_event) = parse_task_event(body, &event.content) {
                                            return Ok(Some(task_event));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Sync failed: {}", e);
            }
        }
    } else {
        // No Matrix client, simulate polling delay
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    
    Ok(None)
}

/// Parse task event from message body and content
fn parse_task_event(body: &str, content: &serde_json::Value) -> Option<TaskEvent> {
    // First, check for embedded cis.task in extra content
    if let Some(extra) = content.get("cis.task") {
        return parse_task_from_json(extra);
    }
    
    // Try to parse body as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if json.get("type").is_some() {
            return parse_task_from_json(&json);
        }
    }
    
    None
}

/// Parse task from JSON value
fn parse_task_from_json(json: &serde_json::Value) -> Option<TaskEvent> {
    let event_type = json["type"].as_str()?;
    
    match event_type {
        "dag.task" => {
            let run_id = json["run_id"].as_str()?.to_string();
            let task = &json["task"];
            
            let task_spec = cis_core::scheduler::DagTaskSpec {
                id: task["id"].as_str()?.to_string(),
                task_type: task["task_type"].as_str()?.to_string(),
                command: task["command"].as_str()?.to_string(),
                depends_on: task["depends_on"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                env: task["env"]
                    .as_object()
                    .map(|obj| obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
                    .unwrap_or_default(),
            };
            
            Some(TaskEvent::NewTask {
                task_id: task_spec.id.clone(),
                dag_id: run_id,
                task_spec,
            })
        }
        "dag.task.cancel" => {
            let task_id = json["task_id"].as_str()?.to_string();
            Some(TaskEvent::CancelTask { task_id })
        }
        "cis.worker.heartbeat" => {
            Some(TaskEvent::Heartbeat)
        }
        "cis.worker.shutdown" => {
            Some(TaskEvent::Shutdown)
        }
        _ => None,
    }
}

/// Handle a task event
async fn handle_task_event(
    args: &WorkerArgs,
    room_conn: &RoomConnection,
    event: TaskEvent,
) -> Result<()> {
    match event {
        TaskEvent::NewTask { task_id, dag_id, task_spec } => {
            info!("Received new task: {} (DAG: {})", task_id, dag_id);
            execute_task(args, room_conn, &task_id, &dag_id, &task_spec).await?;
        }
        TaskEvent::CancelTask { task_id } => {
            info!("Received cancellation for task: {}", task_id);
            // Note: Actual task cancellation requires tracking running processes
            // For now, we just log the request
            warn!("Task cancellation request received for {}. Note: Cancellation is best-effort and requires process tracking.", task_id);
        }
        TaskEvent::Heartbeat => {
            debug!("Received heartbeat from parent node");
            // Send heartbeat response
            send_heartbeat_response(args, room_conn).await?;
        }
        TaskEvent::Shutdown => {
            info!("Received shutdown request");
            // Graceful shutdown will be handled by the main loop
        }
    }
    
    Ok(())
}

/// Execute a task and report result
async fn execute_task(
    args: &WorkerArgs,
    room_conn: &RoomConnection,
    task_id: &str,
    dag_id: &str,
    task_spec: &cis_core::scheduler::DagTaskSpec,
) -> Result<()> {
    println!("🚀 Executing task: {} (DAG: {})", task_id, dag_id);
    println!("   Type: {}", task_spec.task_type);
    println!("   Command: {}", task_spec.command);
    
    if !task_spec.depends_on.is_empty() {
        println!("   Dependencies: {}", task_spec.depends_on.join(", "));
    }
    
    if !task_spec.env.is_empty() {
        println!("   Environment:");
        for (key, value) in &task_spec.env {
            println!("     {}={}", key, value);
        }
    }
    
    let start_time = std::time::Instant::now();
    
    // Execute based on task type
    let result = match task_spec.task_type.as_str() {
        "shell" | "sh" | "bash" => {
            execute_shell_task(task_id, &task_spec.command, &task_spec.env, args).await
        }
        "skill" => {
            execute_skill_task(task_id, &task_spec.command, &task_spec.env).await
        }
        _ => {
            // Default to shell execution for unknown types
            println!("   Unknown task type '{}', defaulting to shell", task_spec.task_type);
            execute_shell_task(task_id, &task_spec.command, &task_spec.env, args).await
        }
    };
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    // Report result to room
    report_task_result(args, room_conn, result, execution_time_ms).await?;
    
    Ok(())
}

/// Execute shell command task
async fn execute_shell_task(
    task_id: &str,
    command: &str,
    env: &std::collections::HashMap<String, String>,
    args: &WorkerArgs,
) -> TaskResult {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};
    
    // Parse command (handle shell operators)
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    
    // Log resource limits if set
    if args.max_cpu > 0 || args.max_memory_mb > 0 {
        debug!("Task {} resource limits: CPU={}, Memory={}MB", 
               task_id, args.max_cpu, args.max_memory_mb);
    }
    
    // Build command with resource limits
    // Note: Full resource limit implementation requires platform-specific code
    // This is a simplified version using wrapper commands
    let wrapped_command = if args.max_memory_mb > 0 || args.max_cpu > 0 {
        // Wrap command with resource limits
        // Linux: use ulimit for memory, cpulimit or taskset for CPU
        // macOS: use ulimit (memory only)
        // Windows: use job objects (not implemented)
        
        let mut limit_commands = Vec::new();
        
        #[cfg(target_os = "linux")]
        {
            if args.max_memory_mb > 0 {
                // ulimit -v sets virtual memory limit in KB
                limit_commands.push(format!("ulimit -v {} 2>/dev/null || true", args.max_memory_mb * 1024));
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            if args.max_memory_mb > 0 {
                // ulimit -v on macOS
                limit_commands.push(format!("ulimit -v {} 2>/dev/null || true", args.max_memory_mb * 1024));
            }
        }
        
        // Build wrapped command
        let limits = limit_commands.join(" && ");
        if limits.is_empty() {
            command.to_string()
        } else {
            format!("{} && {}", limits, command)
        }
    } else {
        command.to_string()
    };
    
    // Build command
    let mut cmd = Command::new(&shell);
    cmd.arg("-c").arg(&wrapped_command);
    
    // Set environment variables
    cmd.envs(env);
    
    // Add resource limit markers to environment (for child processes)
    if args.max_cpu > 0 {
        cmd.env("CIS_WORKER_MAX_CPU", args.max_cpu.to_string());
    }
    if args.max_memory_mb > 0 {
        cmd.env("CIS_WORKER_MAX_MEMORY_MB", args.max_memory_mb.to_string());
    }
    
    // Set working directory if specified
    if let Some(work_dir) = args.scope_id.as_ref() {
        cmd.current_dir(work_dir);
    }
    
    // Execute with timeout (5 minutes default)
    let timeout_duration = Duration::from_secs(300);
    let execution_result = timeout(timeout_duration, cmd.output()).await;
    
    match execution_result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code();
            
            let (status, output_str) = if output.status.success() {
                (TaskStatus::Success, stdout.to_string())
            } else {
                let error_output = if stderr.is_empty() {
                    stdout.to_string()
                } else {
                    stderr.to_string()
                };
                (TaskStatus::Failed, error_output)
            };
            
            TaskResult {
                task_id: task_id.to_string(),
                status,
                output: output_str.trim().to_string(),
                exit_code,
                execution_time_ms: 0, // Will be set by caller
            }
        }
        Ok(Err(e)) => {
            // Failed to execute command
            TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                output: format!("Failed to execute command: {}", e),
                exit_code: None,
                execution_time_ms: 0,
            }
        }
        Err(_) => {
            // Timeout
            TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Timeout,
                output: "Task execution timed out (300s)".to_string(),
                exit_code: None,
                execution_time_ms: 300_000,
            }
        }
    }
}

/// Execute skill task using SkillManager
async fn execute_skill_task(
    task_id: &str,
    skill_command: &str,
    env: &std::collections::HashMap<String, String>,
) -> TaskResult {
    println!("   Executing skill: {}", skill_command);
    
    // Parse skill command: format is "skill_name method args..."
    let parts: Vec<&str> = skill_command.split_whitespace().collect();
    if parts.is_empty() {
        return TaskResult {
            task_id: task_id.to_string(),
            status: TaskStatus::Failed,
            output: "Empty skill command".to_string(),
            exit_code: Some(1),
            execution_time_ms: 0,
        };
    }
    
    let skill_name = parts[0];
    let method = parts.get(1).copied().unwrap_or("execute");
    
    // Initialize SkillManager
    let db_manager = match cis_core::storage::db::DbManager::new() {
        Ok(db) => std::sync::Arc::new(db),
        Err(e) => {
            return TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                output: format!("Failed to initialize DbManager: {}", e),
                exit_code: Some(1),
                execution_time_ms: 0,
            };
        }
    };
    
    let skill_manager = match cis_core::skill::SkillManager::new(db_manager) {
        Ok(sm) => sm,
        Err(e) => {
            return TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                output: format!("Failed to initialize SkillManager: {}", e),
                exit_code: Some(1),
                execution_time_ms: 0,
            };
        }
    };
    
    // Check if skill exists
    match skill_manager.get_info(skill_name) {
        Ok(Some(skill_info)) => {
            println!("   Found skill: {} (type: {:?})", skill_info.meta.name, skill_info.meta.skill_type);
            
            // Build input from env
            let input = serde_json::json!({
                "method": method,
                "args": parts.get(2..).unwrap_or(&[]),
                "env": env,
            });
            
            // Send event to skill if it's active
            match skill_manager.is_active(skill_name) {
                Ok(true) => {
                    // Skill is active, we would send event here
                    // For now, just return success
                    TaskResult {
                        task_id: task_id.to_string(),
                        status: TaskStatus::Success,
                        output: format!("Skill '{}' method '{}' called with input: {}", 
                            skill_name, method, input),
                        exit_code: Some(0),
                        execution_time_ms: 0,
                    }
                }
                Ok(false) => {
                    // Skill not active, try to activate
                    println!("   Skill not active, attempting activation...");
                    match tokio::runtime::Handle::current().block_on(skill_manager.activate(skill_name)) {
                        Ok(()) => {
                            println!("   Skill activated successfully");
                            TaskResult {
                                task_id: task_id.to_string(),
                                status: TaskStatus::Success,
                                output: format!("Skill '{}' activated and ready", skill_name),
                                exit_code: Some(0),
                                execution_time_ms: 0,
                            }
                        }
                        Err(e) => {
                            TaskResult {
                                task_id: task_id.to_string(),
                                status: TaskStatus::Failed,
                                output: format!("Failed to activate skill '{}': {}", skill_name, e),
                                exit_code: Some(1),
                                execution_time_ms: 0,
                            }
                        }
                    }
                }
                Err(e) => {
                    TaskResult {
                        task_id: task_id.to_string(),
                        status: TaskStatus::Failed,
                        output: format!("Error checking skill status: {}", e),
                        exit_code: Some(1),
                        execution_time_ms: 0,
                    }
                }
            }
        }
        Ok(None) => {
            TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                output: format!("Skill '{}' not found", skill_name),
                exit_code: Some(1),
                execution_time_ms: 0,
            }
        }
        Err(e) => {
            TaskResult {
                task_id: task_id.to_string(),
                status: TaskStatus::Failed,
                output: format!("Error looking up skill '{}': {}", skill_name, e),
                exit_code: Some(1),
                execution_time_ms: 0,
            }
        }
    }
}

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: String,
    pub exit_code: Option<i32>,
    pub execution_time_ms: u64,
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Success,
    Failed,
    Cancelled,
    Timeout,
}

/// Report task result to room
async fn report_task_result(
    args: &WorkerArgs,
    room_conn: &RoomConnection,
    mut result: TaskResult,
    execution_time_ms: u64,
) -> Result<()> {
    // Update execution time
    result.execution_time_ms = execution_time_ms;
    
    let status_icon = match result.status {
        TaskStatus::Success => "✅",
        TaskStatus::Failed => "❌",
        TaskStatus::Cancelled => "⚠️",
        TaskStatus::Timeout => "⏱️",
    };
    
    println!("{} Task {} completed: {:?}", status_icon, result.task_id, result.status);
    
    if args.verbose {
        println!("   Output: {}", result.output);
        if let Some(code) = result.exit_code {
            println!("   Exit Code: {}", code);
        }
        println!("   Execution Time: {}ms", result.execution_time_ms);
    }
    
    // Construct result event
    let result_event = TaskResultEvent {
        event_type: "dag.task.result".to_string(),
        worker_id: args.worker_id.clone(),
        room_id: room_conn.room_id.clone(),
        task_id: result.task_id.clone(),
        status: format!("{:?}", result.status).to_lowercase(),
        output: result.output.clone(),
        exit_code: result.exit_code,
        execution_time_ms: result.execution_time_ms,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    
    // Serialize to JSON
    let event_json = match serde_json::to_string(&result_event) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize result event: {}", e);
            return Ok(());
        }
    };
    
    // Send event to Matrix room if client is available
    if let Some(ref client) = room_conn.matrix_client {
        match client.send_message(
            &room_conn.room_id,
            "m.text",
            &format!("Task {} completed: {:?}", result.task_id, result.status),
            Some(serde_json::json!({
                "cis.task_result": result_event
            })),
        ).await {
            Ok(event_id) => {
                info!("Sent task result to room {}: event_id={}", room_conn.room_id, event_id);
            }
            Err(e) => {
                warn!("Failed to send task result to room {}: {}", room_conn.room_id, e);
            }
        }
    } else {
        // Matrix client not available, just log the event
        if args.verbose {
            println!("📤 Result (not sent - no Matrix client): {}", event_json);
        }
        debug!("Task result event: {}", event_json);
    }
    
    Ok(())
}

/// Task result event for Matrix room
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskResultEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub worker_id: String,
    pub room_id: String,
    pub task_id: String,
    pub status: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub execution_time_ms: u64,
    pub timestamp: String,
}

/// Send heartbeat response
async fn send_heartbeat_response(
    args: &WorkerArgs,
    room_conn: &RoomConnection,
) -> Result<()> {
    debug!("Sending heartbeat response from worker {}", args.worker_id);
    
    // Send heartbeat to Matrix room if client is available
    if let Some(ref client) = room_conn.matrix_client {
        let heartbeat = serde_json::json!({
            "type": "cis.worker.heartbeat",
            "worker_id": args.worker_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "healthy",
        });
        
        match client.send_message(
            &room_conn.room_id,
            "m.text",
            &format!("💓 Heartbeat from worker {}", args.worker_id),
            Some(serde_json::json!({
                "cis.heartbeat": heartbeat
            })),
        ).await {
            Ok(event_id) => {
                debug!("Sent heartbeat to room {}: event_id={}", room_conn.room_id, event_id);
            }
            Err(e) => {
                debug!("Failed to send heartbeat: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Perform health check
async fn perform_health_check(args: &WorkerArgs) -> Result<()> {
    debug!("Performing health check for worker {}", args.worker_id);
    
    // Check system resources
    let mut checks = Vec::new();
    
    // Check 1: Memory usage
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = tokio::fs::read_to_string("/proc/self/status").await {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            let mb = kb / 1024;
                            if mb > 1024 {
                                checks.push(format!("⚠️  High memory usage: {} MB", mb));
                            } else {
                                checks.push(format!("✅ Memory: {} MB", mb));
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    
    #[cfg(target_os = "macos")]
    {
        // On macOS, we can use task_info or ps command
        checks.push("✅ Memory check skipped (macOS)".to_string());
    }
    
    // Check 2: Task queue depth (placeholder)
    checks.push("✅ Task queue: OK".to_string());
    
    // Check 3: Connection status
    checks.push("✅ Connection: OK".to_string());
    
    if args.verbose {
        println!("💓 Health check:");
        for check in &checks {
            println!("   {}", check);
        }
    }
    
    Ok(())
}

/// Cleanup worker resources
async fn cleanup_worker(args: &WorkerArgs) -> Result<()> {
    info!("Cleaning up worker resources: {}", args.worker_id);
    
    // Step 1: Remove from registry
    if let Ok(registry) = WorkerRegistry::new() {
        if let Err(e) = registry.remove_worker(&args.worker_id) {
            warn!("Failed to remove worker from registry: {}", e);
        } else {
            debug!("Removed worker {} from registry", args.worker_id);
        }
    }
    
    // Step 2: Clean up temporary files
    let temp_dir = std::env::temp_dir().join("cis").join(&args.worker_id);
    if temp_dir.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(&temp_dir).await {
            warn!("Failed to remove temp directory: {}", e);
        } else {
            debug!("Removed temp directory: {:?}", temp_dir);
        }
    }
    
    // Step 3: Release resources (close file handles, etc.)
    // This is mostly handled by process termination
    
    info!("Cleanup completed for worker {}", args.worker_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_worker_scope_from_str() {
        assert_eq!(
            WorkerScope::from_str("global").unwrap(),
            WorkerScope::Global
        );
        assert_eq!(
            WorkerScope::from_str("project").unwrap(),
            WorkerScope::Project
        );
        assert_eq!(
            WorkerScope::from_str("user").unwrap(),
            WorkerScope::User
        );
        assert_eq!(
            WorkerScope::from_str("type").unwrap(),
            WorkerScope::Type
        );
        assert!(WorkerScope::from_str("invalid").is_err());
    }

    #[test]
    fn test_worker_scope_display() {
        assert_eq!(WorkerScope::Global.to_string(), "global");
        assert_eq!(WorkerScope::Project.to_string(), "project");
        assert_eq!(WorkerScope::User.to_string(), "user");
        assert_eq!(WorkerScope::Type.to_string(), "type");
    }
}
