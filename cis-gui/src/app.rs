//! # Main Application
//!
//! Integrates all GUI components: terminal, node tabs, and manager.

use eframe::egui::{self, CentralPanel, Frame, TopBottomPanel, Window, RichText, ScrollArea, Color32};
use tracing::{info, warn};

use crate::decision_panel::{DecisionAction, DecisionPanel, PendingDecision};
use crate::glm_panel::{GlmPanel, GlmPanelResponse, PendingDagInfo};
use crate::node_manager::{ManagedNode, NodeManager, NodeStatus, TrustState};
use crate::node_tabs::{NodeTabInfo, NodeTabs};
use crate::theme::*;
use cis_core::types::{TaskLevel, Action};
use cis_core::service::{NodeService, DagService};

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

/// Main CIS application
pub struct CisApp {
    node_tabs: NodeTabs,
    node_manager: NodeManager,
    terminal_history: Vec<String>,
    command_input: String,
    
    // Decision panel for four-tier decision mechanism
    decision_panel: DecisionPanel,
    
    // Demo data (will be replaced with real data)
    demo_nodes: Vec<ManagedNode>,
    
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
                address: "192.168.1.100:6767".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(12),
            },
            ManagedNode {
                id: "hugin".to_string(),
                name: "Hugin-pc".to_string(),
                did: Some("did:cis:hugin:def456".to_string()),
                address: "192.168.1.105:6767".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(8),
            },
            ManagedNode {
                id: "seed".to_string(),
                name: "seed.cis.dev".to_string(),
                did: Some("did:cis:seed:ghi789".to_string()),
                address: "seed.cis.dev:6767".to_string(),
                status: NodeStatus::Online,
                trust_state: TrustState::Verified,
                last_seen: Some("Now".to_string()),
                latency_ms: Some(45),
            },
            ManagedNode {
                id: "unknown".to_string(),
                name: "unknown-device".to_string(),
                did: None,
                address: "192.168.1.200:6767".to_string(),
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
            glm_panel: GlmPanel::new(),
            node_service,
            dag_service,
            runtime,
            command_tx: None,
            result_rx: None,
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
        
        app
    }
    
    fn execute_command(&mut self, cmd: &str) {
        self.terminal_history.push(format!("$ {}", cmd));
        
        match cmd.trim() {
            "help" => {
                self.terminal_history.push("Available commands:".to_string());
                self.terminal_history.push("  help          - Show this help".to_string());
                self.terminal_history.push("  node list     - List nodes".to_string());
                self.terminal_history.push("  agent         - Call agent".to_string());
                self.terminal_history.push("  clear         - Clear terminal".to_string());
                self.terminal_history.push("  demo decision - Demo decision panel".to_string());
            }
            "node list" => {
                self.terminal_history.push("Nodes:".to_string());
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
            "demo decision" => {
                self.terminal_history.push("Demo: Triggering Recommended decision level...".to_string());
                // Demo: trigger a Recommended level decision
                let decision = PendingDecision::new(
                    "task-demo-1".to_string(),
                    "编译测试".to_string(),
                    TaskLevel::Recommended { default_action: Action::Execute, timeout_secs: 30 },
                )
                .with_description("执行 cargo test 进行测试");
                self.decision_panel.set_pending_decision(decision);
            }
            "demo confirm" => {
                self.terminal_history.push("Demo: Triggering Confirmed decision level...".to_string());
                // Demo: trigger a Confirmed level decision
                let decision = PendingDecision::new(
                    "task-demo-2".to_string(),
                    "部署到生产环境".to_string(),
                    TaskLevel::Confirmed,
                )
                .with_description("此操作将影响线上服务")
                .with_risk("可能导致服务中断");
                self.decision_panel.set_pending_decision(decision);
            }
            "demo arbitrate" => {
                self.terminal_history.push("Demo: Triggering Arbitrated decision level...".to_string());
                // Demo: trigger an Arbitrated level decision
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
            "glm" => {
                self.terminal_history.push("Opening GLM API panel...".to_string());
                self.glm_panel.open();
            }
            "clear" => {
                self.terminal_history.clear();
            }
            "" => {}
            _ => {
                self.terminal_history.push(format!("Unknown command: {}", cmd));
                self.terminal_history.push("Type 'help' for available commands".to_string());
            }
        }
        
        self.terminal_history.push(String::new());
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
        
        // Node manager window (floating)
        let manager_response = self.node_manager.ui(ctx, &self.demo_nodes);
        
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
    fn confirm_dag(&mut self, dag_id: &str) {
        info!("Confirming DAG: {}", dag_id);
        self.glm_panel.set_status(format!("DAG {} confirmed", dag_id), false);
        // Note: Async service calls disabled - DagService is not Send
    }
    
    /// Reject a DAG
    fn reject_dag(&mut self, dag_id: &str) {
        info!("Rejecting DAG: {}", dag_id);
        self.glm_panel.set_status(format!("DAG {} rejected", dag_id), false);
        // Note: Async service calls disabled - DagService is not Send
    }
    
    /// Refresh pending DAGs from API
    fn refresh_pending_dags(&mut self) {
        info!("Refreshing pending DAGs");
        // Load demo data as fallback
        self.glm_panel.load_demo_data();
        self.glm_panel.set_status("Refreshed".to_string(), false);
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
    fn handle_node_tabs_events(&mut self, ctx: &egui::Context) {
        // Handle events from node tabs context menu
        // This is called every frame to process any pending events
    }
}
