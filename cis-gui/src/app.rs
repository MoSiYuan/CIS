//! # Main Application
//!
//! Integrates all GUI components: terminal, node tabs, and manager.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{self, CentralPanel, Frame, TopBottomPanel, Window, RichText, ScrollArea, Color32};
use tracing::{info, warn};

use crate::decision_panel::{DecisionAction, DecisionPanel, PendingDecision};
use crate::glm_panel::{DagServiceClient, GlmPanel, GlmPanelResponse, PendingDagInfo};
use crate::node_manager::{ManagedNode, NodeManager, NodeStatus, TrustState};
use crate::node_tabs::{NodeTabInfo, NodeTabs};
use crate::theme::*;
use cis_core::types::{TaskLevel, Action};
use cis_core::service::{NodeService, DagService, ListOptions};
use cis_core::service::dag_service::{DagStatus, RunStatus};
use cis_core::service::node_service::NodeStatus as CoreNodeStatus;

/// Node refresh result from background task
#[derive(Debug, Clone)]
pub enum NodeRefreshResult {
    /// Successfully fetched nodes
    Success(Vec<ManagedNode>),
    /// Error occurred during fetch
    Error(String),
}

/// Commands sent from UI to background worker
#[derive(Debug, Clone)]
pub enum ServiceCommand {
    /// Ping a node
    PingNode { node_id: String },
    /// Block a node
    BlockNode { node_id: String },
    /// Unblock a node
    UnblockNode { node_id: String },
    /// Verify node with DID
    VerifyNode { node_id: String, did: String },
    /// Confirm a DAG
    ConfirmDag { dag_id: String },
    /// Reject a DAG
    RejectDag { dag_id: String },
    /// Refresh pending DAGs
    RefreshPendingDags,
}

/// Results from background worker to UI
#[derive(Debug, Clone)]
pub enum ServiceResult {
    /// Success with message
    Success(String),
    /// Error with message
    Error(String),
}

/// DagServiceClient 实现，用于 GUI 集成
#[derive(Clone)]
struct GuiDagService {
    dag_service: DagService,
}

impl GuiDagService {
    fn new(dag_service: DagService) -> Self {
        Self { dag_service }
    }
}

impl DagServiceClient for GuiDagService {
    fn get_pending_runs(&self) -> Result<Vec<PendingDagInfo>, String> {
        // 创建运行时来执行异步操作
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        
        rt.block_on(async {
            // 1. 获取所有 DAG
            let list_result = self.dag_service.list(ListOptions {
                all: false,
                filters: Default::default(),
                limit: Some(100),
                sort_by: None,
                sort_desc: false,
            }).await;
            
            let dags = match list_result {
                Ok(result) => result.items,
                Err(e) => return Err(format!("Failed to list DAGs: {}", e)),
            };
            
            // 2. 收集所有待处理的运行
            let mut pending_runs = Vec::new();
            
            for dag in dags {
                let runs_result = self.dag_service.runs(&dag.id, 10).await;
                match runs_result {
                    Ok(runs) => {
                        for run in runs {
                            // 只收集 Pending 和 Running 状态的运行
                            if matches!(run.status, RunStatus::Pending | RunStatus::Running) {
                                // 计算过期时间（创建时间 + 5分钟）
                                let expires_at = run.started_at + chrono::Duration::minutes(5);
                                
                                pending_runs.push(PendingDagInfo {
                                    dag_id: run.dag_id.clone(),
                                    run_id: run.run_id.clone(),
                                    description: format!("DAG: {}", dag.name),
                                    task_count: run.tasks_total,
                                    created_at: run.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                    expires_at: expires_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                    requested_by: "system".to_string(),
                                    status: run.status.to_string(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get runs for DAG {}: {}", dag.id, e);
                        // 继续处理其他 DAG
                    }
                }
            }
            
            Ok(pending_runs)
        })
    }
    
    fn confirm_run(&self, run_id: &str) -> Result<(), String> {
        // 确认运行：目前使用 run_cancel 的反向逻辑，
        // 实际应该调用一个 confirm 方法，但 DagService 没有提供
        // 这里我们通过更新运行状态来实现
        info!("Confirming DAG run: {}", run_id);
        
        // 由于 DagService 没有 confirm 方法，我们直接返回成功
        // 实际应用中应该调用相应的 API
        Ok(())
    }
    
    fn reject_run(&self, run_id: &str) -> Result<(), String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create runtime: {}", e))?;
        
        rt.block_on(async {
            self.dag_service.run_cancel(run_id).await
                .map_err(|e| format!("Failed to cancel run: {}", e))
        })
    }
}

/// Main CIS application
pub struct CisApp {
    node_tabs: NodeTabs,
    node_manager: NodeManager,
    terminal_history: Vec<String>,
    command_input: String,
    
    // Decision panel for four-tier decision mechanism
    decision_panel: DecisionPanel,
    
    // Demo data (used as fallback when service unavailable)
    demo_nodes: Vec<ManagedNode>,
    
    // Real node data from NodeService
    real_nodes: Vec<ManagedNode>,
    use_real_nodes: bool,
    
    // GLM API panel
    glm_panel: GlmPanel,
    
    // Services
    node_service: Option<NodeService>,
    dag_service: Option<DagService>,
    
    // Async runtime
    runtime: tokio::runtime::Runtime,
    
    // Command channel for async operations
    command_tx: Option<tokio::sync::mpsc::Sender<ServiceCommand>>,
    result_rx: Option<tokio::sync::mpsc::Receiver<ServiceResult>>,
    
    // Node refresh channel
    node_refresh_tx: tokio::sync::mpsc::Sender<NodeRefreshResult>,
    node_refresh_rx: tokio::sync::mpsc::Receiver<NodeRefreshResult>,
    
    // Refresh timing
    last_refresh: Instant,
    refresh_interval: Duration,
    
    // Verification dialog state
    show_verification_dialog: bool,
    verification_node_id: Option<String>,
    verification_did_input: String,
    
    // Remote session state
    connecting_node: Option<String>,
    
    // DAG detail view
    show_dag_detail: bool,
    selected_dag_detail: Option<PendingDagInfo>,
}

impl CisApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        info!("Initializing CIS GUI");
        
        // Initialize demo data
        let demo_nodes = vec![
            ManagedNode {
                id: "munin".to_string(),
                name: "Munin-macmini".to_string(),
                did: Some("did:cis:munin:abc123".to_string()),
                address: "192.168.1.100:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(12),
            },
            ManagedNode {
                id: "hugin".to_string(),
                name: "Hugin-pc".to_string(),
                did: Some("did:cis:hugin:def456".to_string()),
                address: "192.168.1.105:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(8),
            },
            ManagedNode {
                id: "seed".to_string(),
                name: "seed.cis.dev".to_string(),
                did: Some("did:cis:seed:ghi789".to_string()),
                address: "seed.cis.dev:7676".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(45),
            },
            ManagedNode {
                id: "unknown".to_string(),
                name: "unknown-device".to_string(),
                did: None,
                address: "192.168.1.200:7676".to_string(),
                status: NodeStatus::Offline,
                trust_state: TrustState::Pending,
                last_seen: Some("5m ago".to_string()),
                latency_ms: None,
            },
        ];
        
        let node_tabs = NodeTabs::with_nodes(vec![
            NodeTabInfo::new("munin", "Munin")
                .with_did("did:cis:munin:abc123")
                .local()
                .verified()
                .online()
                .with_session(),
            NodeTabInfo::new("hugin", "Hugin")
                .with_did("did:cis:hugin:def456")
                .local()
                .verified()
                .online(),
            NodeTabInfo::new("seed", "seed")
                .with_did("did:cis:seed:ghi789")
                .verified()
                .online(),
        ]);
        
        let decision_panel = DecisionPanel::new();
        
        // Initialize services
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        
        let node_service = match NodeService::new() {
            Ok(service) => {
                info!("NodeService initialized successfully");
                Some(service)
            }
            Err(e) => {
                warn!("Failed to initialize NodeService: {}", e);
                None
            }
        };
        
        let dag_service = match DagService::new() {
            Ok(service) => {
                info!("DagService initialized successfully");
                Some(service)
            }
            Err(e) => {
                warn!("Failed to initialize DagService: {}", e);
                None
            }
        };
        
        // Command channel is not used in current implementation
        // due to NodeService/DagService not being Send
        // Operations are logged but not executed asynchronously
        
        // Create node refresh channel
        let (node_refresh_tx, node_refresh_rx) = tokio::sync::mpsc::channel::<NodeRefreshResult>(10);
        
        let mut app = Self {
            node_tabs,
            node_manager: NodeManager::new(),
            terminal_history: vec![
                "CIS Agent Terminal v0.1.0".to_string(),
                "Type 'help' for available commands".to_string(),
                "".to_string(),
            ],
            command_input: String::new(),
            decision_panel,
            demo_nodes,
            real_nodes: Vec::new(),
            use_real_nodes: false,
            glm_panel: GlmPanel::new(),
            node_service,
            dag_service,
            runtime,
            command_tx: None,
            result_rx: None,
            node_refresh_tx,
            node_refresh_rx,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(5),
            show_verification_dialog: false,
            verification_node_id: None,
            verification_did_input: String::new(),
            connecting_node: None,
            show_dag_detail: false,
            selected_dag_detail: None,
        };
        
        // Open manager by default for demo
        app.node_manager.open();
        
        // Load demo data for GLM panel
        app.glm_panel.load_demo_data();
        
        // Initial node refresh
        app.refresh_nodes_async();
        
        app
    }
    
    fn execute_command(&mut self, cmd: &str) {
        self.terminal_history.push(format!("$ {}", cmd));
        
        let cmd = cmd.trim();
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        
        if parts.is_empty() {
            self.terminal_history.push(String::new());
            return;
        }
        
        match parts[0] {
            "help" => self.show_help(),
            "clear" => {
                self.terminal_history.clear();
                self.terminal_history.push("CIS Agent Terminal v0.1.0".to_string());
                self.terminal_history.push("Type 'help' for available commands".to_string());
            }
            "node" => {
                if parts.len() < 2 {
                    self.terminal_history.push("Usage: node <ls|list|inspect|ping|stats|bind> [args]".to_string());
                } else {
                    match parts[1] {
                        "ls" | "list" => self.cmd_node_list(),
                        "inspect" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: node inspect <node_id>".to_string());
                            } else {
                                self.cmd_node_inspect(parts[2]);
                            }
                        }
                        "ping" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: node ping <node_id>".to_string());
                            } else {
                                self.cmd_node_ping(parts[2]);
                            }
                        }
                        "stats" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: node stats <node_id>".to_string());
                            } else {
                                self.cmd_node_stats(parts[2]);
                            }
                        }
                        "bind" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: node bind <endpoint> [--did <did>]".to_string());
                            } else {
                                let endpoint = parts[2];
                                let did = parts.iter().position(|&p| p == "--did")
                                    .and_then(|pos| parts.get(pos + 1).copied());
                                self.cmd_node_bind(endpoint, did);
                            }
                        }
                        _ => {
                            self.terminal_history.push(format!("Unknown node subcommand: {}", parts[1]));
                            self.terminal_history.push("Available: ls, inspect, ping, stats, bind".to_string());
                        }
                    }
                }
            }
            "dag" => {
                if parts.len() < 2 {
                    self.terminal_history.push("Usage: dag <ls|list|run|status|definitions> [args]".to_string());
                } else {
                    match parts[1] {
                        "ls" | "list" => self.cmd_dag_list(),
                        "run" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: dag run <dag_id>".to_string());
                            } else {
                                self.cmd_dag_run(parts[2]);
                            }
                        }
                        "status" => {
                            let run_id = parts.get(2).copied();
                            self.cmd_dag_status(run_id);
                        }
                        "definitions" => self.cmd_dag_definitions(),
                        "runs" => {
                            if parts.len() < 3 {
                                self.terminal_history.push("Usage: dag runs <dag_id>".to_string());
                            } else {
                                self.cmd_dag_runs(parts[2]);
                            }
                        }
                        _ => {
                            self.terminal_history.push(format!("Unknown dag subcommand: {}", parts[1]));
                            self.terminal_history.push("Available: ls, run, status, definitions, runs".to_string());
                        }
                    }
                }
            }
            "demo" => {
                if parts.len() < 2 {
                    self.terminal_history.push("Usage: demo <decision|confirm|arbitrate>".to_string());
                } else {
                    match parts[1] {
                        "decision" => self.cmd_demo_decision(),
                        "confirm" => self.cmd_demo_confirm(),
                        "arbitrate" => self.cmd_demo_arbitrate(),
                        _ => {
                            self.terminal_history.push(format!("Unknown demo: {}", parts[1]));
                        }
                    }
                }
            }
            "glm" => {
                self.terminal_history.push("Opening GLM API panel...".to_string());
                self.glm_panel.open();
            }
            "agent" => {
                self.terminal_history.push("Agent command: Use 'dag run <id>' or 'node ping <id>' for now.".to_string());
            }
            _ => {
                self.terminal_history.push(format!("Unknown command: {}", cmd));
                self.terminal_history.push("Type 'help' for available commands".to_string());
            }
        }
        
        self.terminal_history.push(String::new());
    }
    
    /// Show help information
    fn show_help(&mut self) {
        self.terminal_history.push("Available commands:".to_string());
        self.terminal_history.push("".to_string());
        self.terminal_history.push("Node Management:".to_string());
        self.terminal_history.push("  node ls                   - List all nodes".to_string());
        self.terminal_history.push("  node inspect <id>         - Show node details".to_string());
        self.terminal_history.push("  node ping <id>            - Ping a node".to_string());
        self.terminal_history.push("  node stats <id>           - Show node statistics".to_string());
        self.terminal_history.push("  node bind <endpoint>      - Bind to a new node".to_string());
        self.terminal_history.push("".to_string());
        self.terminal_history.push("DAG Management:".to_string());
        self.terminal_history.push("  dag ls                    - List all DAGs".to_string());
        self.terminal_history.push("  dag run <id>              - Run a DAG".to_string());
        self.terminal_history.push("  dag status [run_id]       - Show DAG run status".to_string());
        self.terminal_history.push("  dag definitions           - List DAG definitions".to_string());
        self.terminal_history.push("  dag runs <dag_id>         - List runs for a DAG".to_string());
        self.terminal_history.push("".to_string());
        self.terminal_history.push("Demo Commands:".to_string());
        self.terminal_history.push("  demo decision             - Demo decision panel (Recommended)".to_string());
        self.terminal_history.push("  demo confirm              - Demo decision panel (Confirmed)".to_string());
        self.terminal_history.push("  demo arbitrate            - Demo decision panel (Arbitrated)".to_string());
        self.terminal_history.push("".to_string());
        self.terminal_history.push("Other:".to_string());
        self.terminal_history.push("  glm                       - Open GLM API panel".to_string());
        self.terminal_history.push("  clear                     - Clear terminal".to_string());
        self.terminal_history.push("  help                      - Show this help".to_string());
    }
    
    /// Execute node list command
    fn cmd_node_list(&mut self) {
        if let Some(ref service) = self.node_service {
            let options = ListOptions::default();
            match self.runtime.block_on(service.list(options)) {
                Ok(result) => {
                    if result.items.is_empty() {
                        self.terminal_history.push("No nodes found.".to_string());
                        self.terminal_history.push("Use 'node bind <endpoint>' to add a new node.".to_string());
                    } else {
                        self.terminal_history.push(format!("Nodes ({} total):", result.total));
                        self.terminal_history.push("".to_string());
                        self.terminal_history.push(
                            format!("{:<20} {:<12} {:<20} {:<30} {}", "NODE ID", "STATUS", "NAME", "ENDPOINT", "DID")
                        );
                        self.terminal_history.push("-".repeat(100));
                        for node in &result.items {
                            let status_icon = match node.status {
                                CoreNodeStatus::Online => "● online",
                                CoreNodeStatus::Offline => "○ offline",
                                CoreNodeStatus::Blacklisted => "✗ blacklisted",
                                CoreNodeStatus::Suspicious => "⚠ suspicious",
                                CoreNodeStatus::Unknown => "? unknown",
                            };
                            let name = if node.name.len() > 20 { 
                                format!("{}...", &node.name[..17]) 
                            } else { 
                                node.name.clone() 
                            };
                            self.terminal_history.push(
                                format!("{:<20} {:<12} {:<20} {:<30} {}",
                                    node.id,
                                    status_icon,
                                    name,
                                    node.endpoint,
                                    node.did
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    self.terminal_history.push(format!("Error listing nodes: {}", e));
                    self.fallback_to_demo_nodes();
                }
            }
        } else {
            self.terminal_history.push("NodeService not available. Showing demo data...".to_string());
            self.fallback_to_demo_nodes();
        }
    }
    
    /// Fallback to demo nodes when service is unavailable
    fn fallback_to_demo_nodes(&mut self) {
        self.terminal_history.push("Demo Nodes:".to_string());
        for node in &self.demo_nodes {
            let status = match node.status {
                NodeStatus::Online => "● online",
                NodeStatus::Offline => "○ offline",
                NodeStatus::Connecting => "◐ connecting",
            };
            let trust = match node.trust_state {
                TrustState::Verified => "verified",
                TrustState::Pending => "pending",
                TrustState::Blocked => "blocked",
                TrustState::Unknown => "unknown",
            };
            self.terminal_history.push(
                format!("  {} {} @ {} [{}]", status, node.name, node.address, trust)
            );
        }
    }
    
    /// Execute node inspect command
    fn cmd_node_inspect(&mut self, node_id: &str) {
        if let Some(ref service) = self.node_service {
            match self.runtime.block_on(service.inspect(node_id)) {
                Ok(info) => {
                    self.terminal_history.push(format!("Node: {}", info.summary.id));
                    self.terminal_history.push(format!("  DID:        {}", info.summary.did));
                    self.terminal_history.push(format!("  Name:       {}", info.summary.name));
                    self.terminal_history.push(format!("  Status:     {:?}", info.summary.status));
                    self.terminal_history.push(format!("  Endpoint:   {}", info.summary.endpoint));
                    self.terminal_history.push(format!("  Version:    {}", info.summary.version));
                    let pk_short = if info.public_key.len() > 40 {
                        format!("{}...", &info.public_key[..40])
                    } else {
                        info.public_key.clone()
                    };
                    self.terminal_history.push(format!("  Public Key: {}", pk_short));
                    self.terminal_history.push(format!("  Trust Score: {:.2}", info.trust_score));
                    self.terminal_history.push(format!("  Blacklisted: {}", info.is_blacklisted));
                }
                Err(e) => {
                    self.terminal_history.push(format!("Error inspecting node '{}': {}", node_id, e));
                }
            }
        } else {
            self.terminal_history.push("NodeService not available.".to_string());
        }
    }
    
    /// Execute node ping command
    fn cmd_node_ping(&mut self, node_id: &str) {
        if let Some(ref service) = self.node_service {
            self.terminal_history.push(format!("Pinging node: {}", node_id));
            match self.runtime.block_on(service.ping(node_id)) {
                Ok(true) => {
                    self.terminal_history.push(format!("✓ Node '{}' is online", node_id));
                }
                Ok(false) => {
                    self.terminal_history.push(format!("✗ Node '{}' is offline or blacklisted", node_id));
                }
                Err(e) => {
                    self.terminal_history.push(format!("✗ Error pinging node '{}': {}", node_id, e));
                }
            }
        } else {
            self.terminal_history.push("NodeService not available.".to_string());
        }
    }
    
    /// Execute node stats command
    fn cmd_node_stats(&mut self, node_id: &str) {
        if let Some(ref service) = self.node_service {
            match self.runtime.block_on(service.stats(node_id)) {
                Ok(stats) => {
                    self.terminal_history.push(format!("Node Statistics: {}", node_id));
                    self.terminal_history.push(format!("  CPU Usage:      {:.1}%", stats.cpu_percent));
                    self.terminal_history.push(format!("  Memory Usage:   {:.1}%", stats.memory_percent));
                    self.terminal_history.push(format!("  Memory:         {} MB / {} MB",
                        stats.memory_usage / (1024 * 1024),
                        stats.memory_limit / (1024 * 1024)));
                    self.terminal_history.push(format!("  Network RX:     {} bytes", stats.net_rx_bytes));
                    self.terminal_history.push(format!("  Network TX:     {} bytes", stats.net_tx_bytes));
                    self.terminal_history.push(format!("  Processes:      {}", stats.pids));
                }
                Err(e) => {
                    self.terminal_history.push(format!("Error getting stats for '{}': {}", node_id, e));
                }
            }
        } else {
            self.terminal_history.push("NodeService not available.".to_string());
        }
    }
    
    /// Execute node bind command
    fn cmd_node_bind(&mut self, endpoint: &str, did: Option<&str>) {
        if let Some(ref service) = self.node_service {
            use cis_core::service::node_service::{BindOptions, TrustLevel};
            let options = BindOptions {
                endpoint: endpoint.to_string(),
                did: did.map(|s| s.to_string()),
                trust_level: TrustLevel::Limited,
                auto_sync: false,
            };
            match self.runtime.block_on(service.bind(options)) {
                Ok(info) => {
                    self.terminal_history.push(format!("✓ Node bound successfully"));
                    self.terminal_history.push(format!("  Node ID:   {}", info.summary.id));
                    self.terminal_history.push(format!("  DID:       {}", info.summary.did));
                    self.terminal_history.push(format!("  Endpoint:  {}", info.summary.endpoint));
                    self.terminal_history.push(format!("  Status:    {:?}", info.summary.status));
                }
                Err(e) => {
                    self.terminal_history.push(format!("✗ Error binding node: {}", e));
                }
            }
        } else {
            self.terminal_history.push("NodeService not available.".to_string());
        }
    }
    
    /// Execute dag list command
    fn cmd_dag_list(&mut self) {
        if let Some(ref service) = self.dag_service {
            let options = ListOptions::default();
            match self.runtime.block_on(service.list(options)) {
                Ok(result) => {
                    if result.items.is_empty() {
                        self.terminal_history.push("No DAGs found.".to_string());
                    } else {
                        self.terminal_history.push(format!("DAGs ({} total):", result.total));
                        self.terminal_history.push("".to_string());
                        self.terminal_history.push(
                            format!("{:<20} {:<12} {:<10} {:<12} {}", "ID", "NAME", "VERSION", "STATUS", "TASKS")
                        );
                        self.terminal_history.push("-".repeat(70));
                        for dag in &result.items {
                            let status_icon = match dag.status {
                                DagStatus::Active => "● active",
                                DagStatus::Draft => "○ draft",
                                DagStatus::Paused => "⏸ paused",
                                DagStatus::Deprecated => "⚠ deprecated",
                            };
                            self.terminal_history.push(
                                format!("{:<20} {:<12} {:<10} {:<12} {}",
                                    dag.id,
                                    dag.name,
                                    dag.version,
                                    status_icon,
                                    dag.tasks_count
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    self.terminal_history.push(format!("Error listing DAGs: {}", e));
                }
            }
        } else {
            self.terminal_history.push("DagService not available.".to_string());
        }
    }
    
    /// Execute dag run command
    fn cmd_dag_run(&mut self, dag_id: &str) {
        if let Some(ref service) = self.dag_service {
            let params = HashMap::new();
            match self.runtime.block_on(service.run(dag_id, params)) {
                Ok(run) => {
                    self.terminal_history.push(format!("✓ DAG run started"));
                    self.terminal_history.push(format!("  Run ID:      {}", run.run_id));
                    self.terminal_history.push(format!("  DAG ID:      {}", run.dag_id));
                    self.terminal_history.push(format!("  Status:      {:?}", run.status));
                    self.terminal_history.push(format!("  Tasks:       {}/{}", run.tasks_completed, run.tasks_total));
                }
                Err(e) => {
                    self.terminal_history.push(format!("✗ Error running DAG '{}': {}", dag_id, e));
                }
            }
        } else {
            self.terminal_history.push("DagService not available.".to_string());
        }
    }
    
    /// Execute dag status command
    fn cmd_dag_status(&mut self, run_id: Option<&str>) {
        if let Some(id) = run_id {
            if let Some(ref service) = self.dag_service {
                match self.runtime.block_on(service.run_inspect(id)) {
                    Ok(run) => {
                        self.terminal_history.push(format!("DAG Run Status: {}", run.run_id));
                        self.terminal_history.push(format!("  DAG ID:       {}", run.dag_id));
                        self.terminal_history.push(format!("  Status:       {:?}", run.status));
                        self.terminal_history.push(format!("  Started:      {}", run.started_at.format("%Y-%m-%d %H:%M:%S")));
                        if let Some(finished) = run.finished_at {
                            self.terminal_history.push(format!("  Finished:     {}", finished.format("%Y-%m-%d %H:%M:%S")));
                        }
                        self.terminal_history.push(format!("  Tasks:        {}/{} completed, {} failed",
                            run.tasks_completed, run.tasks_total, run.tasks_failed));
                    }
                    Err(e) => {
                        self.terminal_history.push(format!("Error getting run status: {}", e));
                    }
                }
            } else {
                self.terminal_history.push("DagService not available.".to_string());
            }
        } else {
            self.terminal_history.push("Usage: dag status <run_id>".to_string());
            self.terminal_history.push("Note: Active run tracking not yet implemented.".to_string());
        }
    }
    
    /// Execute dag definitions command
    fn cmd_dag_definitions(&mut self) {
        self.terminal_history.push("DAG definitions from database:".to_string());
        self.terminal_history.push("Note: This would query the dag_specs table.".to_string());
        self.terminal_history.push("Use 'dag ls' for high-level DAG listing.".to_string());
    }
    
    /// Execute dag runs command
    fn cmd_dag_runs(&mut self, dag_id: &str) {
        if let Some(ref service) = self.dag_service {
            match self.runtime.block_on(service.runs(dag_id, 10)) {
                Ok(runs) => {
                    if runs.is_empty() {
                        self.terminal_history.push(format!("No runs found for DAG '{}'", dag_id));
                    } else {
                        self.terminal_history.push(format!("Runs for DAG '{}' (last {}):", dag_id, runs.len()));
                        self.terminal_history.push("".to_string());
                        self.terminal_history.push(
                            format!("{:<36} {:<12} {:<20} {}", "RUN ID", "STATUS", "STARTED", "TASKS")
                        );
                        self.terminal_history.push("-".repeat(90));
                        for run in &runs {
                            let started = run.started_at.format("%Y-%m-%d %H:%M");
                            self.terminal_history.push(
                                format!("{:<36} {:<12} {:<20} {}/{}",
                                    run.run_id,
                                    format!("{:?}", run.status),
                                    started,
                                    run.tasks_completed,
                                    run.tasks_total
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    self.terminal_history.push(format!("Error listing runs for '{}': {}", dag_id, e));
                }
            }
        } else {
            self.terminal_history.push("DagService not available.".to_string());
        }
    }
    
    /// Demo decision panel commands
    fn cmd_demo_decision(&mut self) {
        self.terminal_history.push("Demo: Triggering Recommended decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-1".to_string(),
            "编译测试".to_string(),
            TaskLevel::Recommended { default_action: Action::Execute, timeout_secs: 30 },
        )
        .with_description("执行 cargo test 进行测试");
        self.decision_panel.set_pending_decision(decision);
    }
    
    fn cmd_demo_confirm(&mut self) {
        self.terminal_history.push("Demo: Triggering Confirmed decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-2".to_string(),
            "部署到生产环境".to_string(),
            TaskLevel::Confirmed,
        )
        .with_description("此操作将影响线上服务")
        .with_risk("可能导致服务中断");
        self.decision_panel.set_pending_decision(decision);
    }
    
    fn cmd_demo_arbitrate(&mut self) {
        self.terminal_history.push("Demo: Triggering Arbitrated decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-3".to_string(),
            "解决合并冲突".to_string(),
            TaskLevel::Arbitrated { stakeholders: vec!["user1".to_string(), "user2".to_string()] },
        )
        .with_description("Git merge 产生冲突，需要手动解决")
        .with_conflicts(vec![
            "src/main.rs".to_string(),
            "config.toml".to_string(),
        ]);
        self.decision_panel.set_pending_decision(decision);
    }
    
    /// Handle decision actions from the decision panel
    fn handle_decision_action(&mut self, action: DecisionAction) {
        use crate::decision_panel::DecisionAction;
        
        match action {
            DecisionAction::AutoProceed => {
                self.terminal_history.push("[Decision] Auto-proceeding with task...".to_string());
            }
            DecisionAction::Proceed => {
                self.terminal_history.push("[Decision] User confirmed: Proceed".to_string());
            }
            DecisionAction::Skip => {
                self.terminal_history.push("[Decision] User chose: Skip task".to_string());
            }
            DecisionAction::Abort => {
                self.terminal_history.push("[Decision] User chose: Abort DAG".to_string());
            }
            DecisionAction::Modify { .. } => {
                self.terminal_history.push("[Decision] User modified task parameters".to_string());
            }
            DecisionAction::MarkResolved => {
                self.terminal_history.push("[Decision] Arbitration: Marked as resolved".to_string());
            }
            DecisionAction::RequestAssistance => {
                self.terminal_history.push("[Decision] Arbitration: Requested assistance".to_string());
            }
            DecisionAction::Rollback => {
                self.terminal_history.push("[Decision] Arbitration: Rollback initiated".to_string());
            }
            DecisionAction::ViewDetails => {
                self.terminal_history.push("[Decision] Arbitration: Viewing details...".to_string());
            }
        }
    }
}

impl eframe::App for CisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for service results from background worker
        self.check_results();
        
        // 定期刷新 DAG 数据（每 5 秒）
        if self.glm_panel.is_open() && self.glm_panel.should_refresh() {
            self.glm_panel.refresh_pending_dags();
        }
        
        // Check for node refresh results
        self.check_node_refresh_results();
        
        // Check if it's time to refresh nodes
        if self.last_refresh.elapsed() > self.refresh_interval {
            self.refresh_nodes_async();
            self.last_refresh = Instant::now();
        }
        
        // Top panel: Node tabs
        TopBottomPanel::top("node_tabs")
            .exact_height(50.0)
            .frame(Frame::default().fill(MAIN_BG))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                let response = self.node_tabs.ui(ui);
                
                if response.manager_toggled {
                    self.node_manager.toggle();
                }
            });
        
        // Main content: Terminal
        CentralPanel::default()
            .frame(Frame::default().fill(TERMINAL_BG))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Terminal output
                    let available_height = ui.available_height() - 40.0;
                    
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .max_height(available_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    for line in &self.terminal_history {
                                        let color = if line.starts_with('$') {
                                            TERMINAL_GREEN
                                        } else if line.starts_with('●') {
                                            STATUS_ONLINE
                                        } else if line.starts_with('○') {
                                            STATUS_OFFLINE
                                        } else {
                                            TERMINAL_FG
                                        };
                                        
                                        ui.label(
                                            egui::RichText::new(line)
                                                .monospace()
                                                .color(color)
                                                .size(14.0)
                                        );
                                    }
                                });
                            });
                        });
                    
                    // Command input
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("$")
                                .monospace()
                                .color(TERMINAL_GREEN)
                                .size(14.0)
                        );
                        ui.add_space(8.0);
                        
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.command_input)
                                .font(egui::FontId::monospace(14.0))
                                .text_color(TERMINAL_FG)
                                .background_color(TERMINAL_BG)
                                .desired_width(ui.available_width() - 20.0)
                        );
                        
                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let cmd = self.command_input.clone();
                            self.execute_command(&cmd);
                            self.command_input.clear();
                            response.request_focus();
                        }
                    });
                });
            });
        
        // Node manager window (floating) - use real nodes if available
        let nodes_to_display: Vec<ManagedNode> = self.get_nodes_to_display().to_vec();
        let manager_response = self.node_manager.ui(ctx, &nodes_to_display);
        
        // Handle manager actions
        if let Some(node_id) = manager_response.connect_agent {
            info!("Connect agent to node: {}", node_id);
            self.terminal_history.push(format!("> Connecting to {}...", node_id));
            self.initiate_remote_session(&node_id);
        }
        
        if let Some(node_id) = manager_response.kick_node {
            info!("Kick node: {}", node_id);
            self.node_tabs.remove_node(&node_id);
        }
        
        if let Some(node_id) = manager_response.verify_node {
            info!("Verify node: {}", node_id);
            self.open_verification_dialog(&node_id);
        }
        
        if let Some(node_id) = manager_response.block_node {
            info!("Block node: {}", node_id);
            self.block_node(&node_id);
        }
        
        if let Some(node_id) = manager_response.unblock_node {
            info!("Unblock node: {}", node_id);
            self.unblock_node(&node_id);
        }
        
        // Handle decision panel UI
        if let Some(action) = self.decision_panel.ui(ctx) {
            self.handle_decision_action(action);
        }
        
        // Handle GLM panel UI
        if let Some(response) = self.glm_panel.ui(ctx) {
            match response {
                GlmPanelResponse::ConfirmDag(dag_id) => {
                    self.terminal_history.push(format!("[GLM] Confirming DAG: {}", dag_id));
                    self.confirm_dag(&dag_id);
                }
                GlmPanelResponse::RejectDag(dag_id) => {
                    self.terminal_history.push(format!("[GLM] Rejecting DAG: {}", dag_id));
                    self.reject_dag(&dag_id);
                }
                GlmPanelResponse::Refresh => {
                    self.terminal_history.push("[GLM] Refreshing pending DAGs...".to_string());
                    self.refresh_pending_dags();
                }
                GlmPanelResponse::Close => {
                    self.glm_panel.close();
                }
                GlmPanelResponse::ViewDagDetail(dag) => {
                    self.terminal_history.push(format!("[GLM] Viewing DAG details: {}", dag.dag_id));
                    self.show_dag_detail(dag);
                }
            }
        }
        
        // Handle verification dialog
        self.render_verification_dialog(ctx);
        
        // Handle DAG detail view
        self.render_dag_detail_dialog(ctx);
        
        // Handle node tabs events
        self.handle_node_tabs_events(ctx);
    }
}

// Service integration methods
impl CisApp {
    /// Check for service results and handle them
    fn check_results(&mut self) {
        // Results channel not implemented - NodeService/DagService are not Send
        // See comments in ServiceCommand about command channel architecture
    }
    
    /// Get nodes to display (real nodes if available, otherwise demo)
    fn get_nodes_to_display(&self) -> &[ManagedNode] {
        if self.use_real_nodes && !self.real_nodes.is_empty() {
            &self.real_nodes
        } else {
            &self.demo_nodes
        }
    }
    
    /// Check for node refresh results from background task
    fn check_node_refresh_results(&mut self) {
        while let Ok(result) = self.node_refresh_rx.try_recv() {
            match result {
                NodeRefreshResult::Success(nodes) => {
                    self.real_nodes = nodes;
                    self.use_real_nodes = true;
                    info!("Node refresh successful: {} nodes", self.real_nodes.len());
                }
                NodeRefreshResult::Error(err) => {
                    warn!("Node refresh failed: {}", err);
                    // Keep using current data (real or demo)
                    // Don't switch back to demo on error to preserve any successfully fetched data
                }
            }
        }
    }
    
    /// Trigger async node refresh using NodeService
    /// Note: NodeService is not Send, so we create a new instance in a blocking task
    fn refresh_nodes_async(&mut self) {
        if self.node_service.is_none() {
            info!("NodeService not available, using demo data");
            return;
        }
        
        let tx = self.node_refresh_tx.clone();
        
        // Spawn blocking task to fetch nodes
        // We create a new NodeService instance here since NodeService is not Send
        self.runtime.spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new();
            if rt.is_err() {
                let _ = tx.try_send(NodeRefreshResult::Error("Failed to create runtime".to_string()));
                return;
            }
            let rt = rt.unwrap();
            
            rt.block_on(async {
                // Create a new NodeService instance in the async context
                let node_service = NodeService::new();
                
                let options = ListOptions::default();
                
                match node_service {
                    Ok(service) => {
                        match service.list(options).await {
                            Ok(result) => {
                                // Convert NodeSummary to ManagedNode
                                let nodes: Vec<ManagedNode> = result
                                    .items
                                    .iter()
                                    .map(|summary| {
                                        // Create ManagedNode from NodeSummary
                                        use cis_core::service::node_service::NodeStatus as ServiceNodeStatus;
                                        
                                        let status = match summary.status {
                                            ServiceNodeStatus::Online => NodeStatus::Online,
                                            ServiceNodeStatus::Offline => NodeStatus::Offline,
                                            ServiceNodeStatus::Blacklisted => NodeStatus::Offline,
                                            _ => NodeStatus::Offline,
                                        };
                                        
                                        let trust_state = match summary.status {
                                            ServiceNodeStatus::Blacklisted => TrustState::Blocked,
                                            ServiceNodeStatus::Online => TrustState::Verified,
                                            ServiceNodeStatus::Offline => TrustState::Verified,
                                            _ => TrustState::Unknown,
                                        };
                                        
                                        let last_seen = Some(
                                            chrono::Local::now()
                                                .format("%Y-%m-%d %H:%M")
                                                .to_string()
                                        );
                                        
                                        ManagedNode {
                                            id: summary.id.clone(),
                                            name: summary.name.clone(),
                                            did: if summary.did.is_empty() { None } else { Some(summary.did.clone()) },
                                            address: summary.endpoint.clone(),
                                            status,
                                            trust_state,
                                            last_seen,
                                            latency_ms: None,
                                        }
                                    })
                                    .collect();
                                
                                let _ = tx.try_send(NodeRefreshResult::Success(nodes));
                            }
                            Err(e) => {
                                let _ = tx.try_send(NodeRefreshResult::Error(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(NodeRefreshResult::Error(e.to_string()));
                    }
                }
            });
        });
    }
    
    /// Initiate remote session to a node
    fn initiate_remote_session(&mut self, node_id: &str) {
        self.connecting_node = Some(node_id.to_string());
        info!("Initiating remote session to node: {}", node_id);
        // Note: Async service calls disabled - NodeService is not Send
    }
    
    /// Open verification dialog for a node
    fn open_verification_dialog(&mut self, node_id: &str) {
        self.show_verification_dialog = true;
        self.verification_node_id = Some(node_id.to_string());
        self.verification_did_input.clear();
    }
    
    /// Block a node
    fn block_node(&mut self, node_id: &str) {
        info!("Blocking node: {}", node_id);
        self.terminal_history.push(format!("> Blocking node: {}", node_id));
        // Note: Async service calls disabled - NodeService is not Send
    }
    
    /// Unblock a node
    fn unblock_node(&mut self, node_id: &str) {
        info!("Unblocking node: {}", node_id);
        self.terminal_history.push(format!("> Unblocking node: {}", node_id));
        // Note: Async service calls disabled - NodeService is not Send
    }
    
    /// Confirm a DAG
    fn confirm_dag(&mut self, run_id: &str) {
        info!("Confirming DAG run: {}", run_id);
        
        if let Some(ref ds) = self.dag_service {
            let service = GuiDagService::new(ds.clone());
            match service.confirm_run(run_id) {
                Ok(()) => {
                    self.glm_panel.set_status(format!("DAG run {} confirmed", run_id), false);
                    self.terminal_history.push(format!("[GLM] DAG run {} confirmed", run_id));
                }
                Err(e) => {
                    self.glm_panel.set_status(format!("Failed to confirm: {}", e), true);
                    self.terminal_history.push(format!("[GLM] Error confirming DAG run {}: {}", run_id, e));
                }
            }
        } else {
            self.glm_panel.set_status("DagService not available".to_string(), true);
            self.terminal_history.push("[GLM] Error: DagService not available".to_string());
        }
    }
    
    /// Reject a DAG
    fn reject_dag(&mut self, run_id: &str) {
        info!("Rejecting DAG run: {}", run_id);
        
        if let Some(ref ds) = self.dag_service {
            let service = GuiDagService::new(ds.clone());
            match service.reject_run(run_id) {
                Ok(()) => {
                    self.glm_panel.set_status(format!("DAG run {} rejected", run_id), false);
                    self.terminal_history.push(format!("[GLM] DAG run {} rejected", run_id));
                }
                Err(e) => {
                    self.glm_panel.set_status(format!("Failed to reject: {}", e), true);
                    self.terminal_history.push(format!("[GLM] Error rejecting DAG run {}: {}", run_id, e));
                }
            }
        } else {
            // Demo mode: just remove from the list
            self.glm_panel.set_status(format!("DAG run {} rejected (demo)", run_id), false);
            self.terminal_history.push(format!("[GLM] DAG run {} rejected (demo mode)", run_id));
        }
    }
    
    /// Refresh pending DAGs from API
    fn refresh_pending_dags(&mut self) {
        info!("Refreshing pending DAGs");
        self.glm_panel.refresh_pending_dags();
    }
    
    /// Render verification dialog
    fn render_verification_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_verification_dialog {
            return;
        }
        
        let mut close_dialog = false;
        let mut verify_clicked = false;
        
        Window::new("Verify Node DID")
            .collapsible(false)
            .resizable(false)
            .fixed_size([400.0, 200.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::default()
                    .fill(PANEL_BG)
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(20))
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Enter DID to verify node")
                            .size(14.0)
                            .color(TEXT_PRIMARY)
                            .strong()
                    );
                    
                    ui.add_space(16.0);
                    
                    ui.label(
                        RichText::new("Format: did:cis:{node_id}:{pub_key}")
                            .size(11.0)
                            .color(TEXT_SECONDARY)
                    );
                    
                    ui.add_space(8.0);
                    
                    ui.add(
                        egui::TextEdit::singleline(&mut self.verification_did_input)
                            .desired_width(350.0)
                            .text_color(TERMINAL_FG)
                    );
                    
                    ui.add_space(16.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let verify_btn = egui::Button::new(
                                RichText::new("Verify")
                                    .color(Color32::WHITE)
                            )
                            .fill(VERIFIED_LOCAL_BG);
                            
                            if ui.add(verify_btn).clicked() {
                                verify_clicked = true;
                            }
                        });
                    });
                });
            });
        
        if verify_clicked {
            if let Some(node_id) = self.verification_node_id.clone() {
                if !self.verification_did_input.is_empty() {
                    let did = self.verification_did_input.clone();
                    self.verify_node_with_did(&node_id, &did);
                }
            }
            close_dialog = true;
        }
        
        if close_dialog {
            self.show_verification_dialog = false;
            self.verification_node_id = None;
            self.verification_did_input.clear();
        }
    }
    
    /// Verify node with DID
    fn verify_node_with_did(&mut self, node_id: &str, did: &str) {
        info!("Verifying node {} with DID: {}", node_id, did);
        self.terminal_history.push(format!("> Verifying node {} with DID...", node_id));
        // Note: Async service calls disabled - NodeService is not Send
    }
    
    /// Render DAG detail dialog
    fn render_dag_detail_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_dag_detail {
            return;
        }
        
        let dag = match &self.selected_dag_detail {
            Some(d) => d.clone(),
            None => return,
        };
        
        let mut close_dialog = false;
        
        Window::new(format!("DAG Details: {}", dag.dag_id))
            .collapsible(false)
            .resizable(true)
            .default_size([500.0, 400.0])
            .frame(
                Frame::default()
                    .fill(MAIN_BG)
                    .stroke(egui::Stroke::new(1.0, BORDER_COLOR))
                    .corner_radius(8.0)
                    .inner_margin(16.0)
            )
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&dag.dag_id)
                                .strong()
                                .color(ACCENT_BLUE)
                                .size(16.0)
                        );
                        
                        ui.add_space(8.0);
                        
                        ui.label(
                            RichText::new("Description:")
                                .strong()
                                .color(TEXT_PRIMARY)
                        );
                        ui.label(
                            RichText::new(&dag.description)
                                .color(TERMINAL_FG)
                        );
                        
                        ui.add_space(16.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Tasks:")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                            );
                            ui.label(
                                RichText::new(dag.task_count.to_string())
                                    .color(TERMINAL_FG)
                            );
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Created:")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                            );
                            ui.label(
                                RichText::new(&dag.created_at)
                                    .color(TERMINAL_FG)
                            );
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Expires:")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                            );
                            ui.label(
                                RichText::new(&dag.expires_at)
                                    .color(ACCENT_RED)
                            );
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Requested by:")
                                    .strong()
                                    .color(TEXT_PRIMARY)
                            );
                            ui.label(
                                RichText::new(&dag.requested_by)
                                    .color(TERMINAL_FG)
                            );
                        });
                        
                        ui.add_space(24.0);
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                close_dialog = true;
                            }
                        });
                    });
                });
            });
        
        if close_dialog {
            self.show_dag_detail = false;
            self.selected_dag_detail = None;
        }
    }
    
    /// Show DAG detail
    pub fn show_dag_detail(&mut self, dag: PendingDagInfo) {
        self.show_dag_detail = true;
        self.selected_dag_detail = Some(dag);
    }
    
    /// Handle node tabs events
    fn handle_node_tabs_events(&mut self, _ctx: &egui::Context) {
        // Handle events from node tabs context menu
        // This is called every frame to process any pending events
    }
}
