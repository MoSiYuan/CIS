//! # Main Application
//!
//! Integrates all GUI components: terminal, node tabs, and manager.

use eframe::egui::{self, CentralPanel, Frame, TopBottomPanel};
use tracing::info;

use crate::node_manager::{ManagedNode, NodeManager, NodeStatus, TrustState};
use crate::node_tabs::{NodeTabInfo, NodeTabs};
use crate::theme::*;

/// Main CIS application
pub struct CisApp {
    node_tabs: NodeTabs,
    node_manager: NodeManager,
    terminal_history: Vec<String>,
    command_input: String,
    
    // Demo data (will be replaced with real data)
    demo_nodes: Vec<ManagedNode>,
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
        
        let mut app = Self {
            node_tabs,
            node_manager: NodeManager::new(),
            terminal_history: vec![
                "CIS Agent Terminal v0.1.0".to_string(),
                "Type 'help' for available commands".to_string(),
                "".to_string(),
            ],
            command_input: String::new(),
            demo_nodes,
        };
        
        // Open manager by default for demo
        app.node_manager.open();
        
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
}

impl eframe::App for CisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            // TODO: Initiate remote session
        }
        
        if let Some(node_id) = manager_response.kick_node {
            info!("Kick node: {}", node_id);
            self.node_tabs.remove_node(&node_id);
        }
        
        if let Some(node_id) = manager_response.verify_node {
            info!("Verify node: {}", node_id);
            // TODO: Open verification dialog
        }
    }
}
