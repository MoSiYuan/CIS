//! # Node Manager
//!
//! Window for managing network ACL, showing all nodes and their trust status.

use eframe::egui::{self, Color32, Frame, ScrollArea, Window};

use crate::theme::*;

/// Filter for node list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeFilter {
    All,
    Connected,
    Verified,
    Pending,
    Disconnected,
    Blocked,
}

impl NodeFilter {
    fn as_str(&self) -> &'static str {
        match self {
            NodeFilter::All => "All",
            NodeFilter::Connected => "Connected",
            NodeFilter::Verified => "Verified",
            NodeFilter::Pending => "Pending",
            NodeFilter::Disconnected => "Disconnected",
            NodeFilter::Blocked => "Blocked",
        }
    }
}

/// Node information for manager
#[derive(Debug, Clone)]
pub struct ManagedNode {
    pub id: String,
    pub name: String,
    pub did: Option<String>,
    pub address: String,
    pub status: NodeStatus,
    pub trust_state: TrustState,
    pub last_seen: Option<String>,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
    Connecting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    Unknown,
    Pending,
    Verified,
    Blocked,
}

/// Node manager window
pub struct NodeManager {
    open: bool,
    filter: NodeFilter,
    selected_node: Option<String>,
    new_did_input: String,
    new_reason_input: String,
}

impl NodeManager {
    pub fn new() -> Self {
        Self {
            open: false,
            filter: NodeFilter::All,
            selected_node: None,
            new_did_input: String::new(),
            new_reason_input: String::new(),
        }
    }
    
    pub fn is_open(&self) -> bool {
        self.open
    }
    
    pub fn open(&mut self) {
        self.open = true;
    }
    
    pub fn close(&mut self) {
        self.open = false;
    }
    
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
    
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }
    
    pub fn ui(&mut self, ctx: &egui::Context, nodes: &[ManagedNode]) -> NodeManagerResponse {
        let mut response = NodeManagerResponse::default();
        
        if !self.open {
            return response;
        }
        
        Window::new("Node Manager")
            .collapsible(false)
            .resizable(true)
            .default_size([800.0, 500.0])
            .show(ctx, |ui| {
                // Filter tabs
                ui.horizontal(|ui| {
                    for filter in [
                        NodeFilter::All,
                        NodeFilter::Connected,
                        NodeFilter::Verified,
                        NodeFilter::Pending,
                        NodeFilter::Disconnected,
                        NodeFilter::Blocked,
                    ] {
                        let selected = self.filter == filter;
                        let text = format!("{} (0)", filter.as_str()); // TODO: Count
                        
                        let btn = egui::Button::new(text)
                            .fill(if selected { VERIFIED_LOCAL_BG } else { PANEL_BG })
                            .corner_radius(egui::CornerRadius::same(4));
                        
                        if ui.add(btn).clicked() {
                            self.filter = filter;
                        }
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add Node").clicked() {
                            response.add_node_clicked = true;
                        }
                    });
                });
                
                ui.separator();
                
                // Node list
                let filtered_nodes: Vec<_> = nodes.iter()
                    .filter(|n| self.matches_filter(n))
                    .collect();
                
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for node in filtered_nodes {
                            self.render_node_row(ui, node, &mut response);
                        }
                    });
                
                // Bottom actions
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Total: {} nodes", nodes.len()));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.close();
                        }
                        if ui.button("Sync ACL").clicked() {
                            response.sync_acl_clicked = true;
                        }
                    });
                });
            });
        
        response
    }
    
    fn matches_filter(&self, node: &ManagedNode) -> bool {
        match self.filter {
            NodeFilter::All => true,
            NodeFilter::Connected => matches!(node.status, NodeStatus::Online),
            NodeFilter::Verified => matches!(node.trust_state, TrustState::Verified),
            NodeFilter::Pending => matches!(node.trust_state, TrustState::Pending),
            NodeFilter::Disconnected => matches!(node.status, NodeStatus::Offline),
            NodeFilter::Blocked => matches!(node.trust_state, TrustState::Blocked),
        }
    }
    
    fn render_node_row(&mut self, ui: &mut egui::Ui, node: &ManagedNode, response: &mut NodeManagerResponse) {
        let is_selected = self.selected_node.as_ref() == Some(&node.id);
        
        Frame::default()
            .fill(if is_selected { PANEL_BG.gamma_multiply(1.2) } else { PANEL_BG })
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Status indicator
                    let (status_icon, status_color) = match node.status {
                        NodeStatus::Online => ("●", STATUS_ONLINE),
                        NodeStatus::Offline => ("○", STATUS_OFFLINE),
                        NodeStatus::Connecting => ("◐", STATUS_WARNING),
                    };
                    ui.colored_label(status_color, status_icon);
                    
                    ui.add_space(8.0);
                    
                    // Trust state badge
                    let (trust_text, trust_bg) = match node.trust_state {
                        TrustState::Verified => ("✓ Verified", VERIFIED_LOCAL_BG),
                        TrustState::Pending => ("◐ Pending", PENDING_BG),
                        TrustState::Blocked => ("✗ Blocked", BLOCKED_BG),
                        TrustState::Unknown => ("? Unknown", UNKNOWN_BG),
                    };
                    
                    ui.label(
                        egui::RichText::new(trust_text)
                            .background_color(trust_bg)
                            .color(if matches!(node.trust_state, TrustState::Blocked | TrustState::Unknown) {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            })
                            .size(11.0)
                    );
                    
                    ui.add_space(8.0);
                    
                    // Node info
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&node.name).strong());
                        ui.label(egui::RichText::new(&node.address).size(11.0).color(TEXT_SECONDARY));
                        if let Some(ref did) = node.did {
                            ui.label(egui::RichText::new(format!("DID: {}", did))
                                .size(10.0)
                                .color(TEXT_SECONDARY));
                        }
                    });
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Action buttons
                        match node.trust_state {
                            TrustState::Verified => {
                                if ui.button("Agent").clicked() {
                                    response.connect_agent = Some(node.id.clone());
                                }
                                if ui.button("Kick").clicked() {
                                    response.kick_node = Some(node.id.clone());
                                }
                            }
                            TrustState::Pending => {
                                if ui.button("Verify").clicked() {
                                    response.verify_node = Some(node.id.clone());
                                }
                                if ui.button("Block").clicked() {
                                    response.block_node = Some(node.id.clone());
                                }
                            }
                            TrustState::Blocked => {
                                if ui.button("Unblock").clicked() {
                                    response.unblock_node = Some(node.id.clone());
                                }
                            }
                            TrustState::Unknown => {
                                if ui.button("Add to Whitelist").clicked() {
                                    response.allow_node = Some(node.id.clone());
                                }
                                if ui.button("Block").clicked() {
                                    response.block_node = Some(node.id.clone());
                                }
                            }
                        }
                    });
                });
            });
        
        ui.add_space(4.0);
    }
}

impl Default for NodeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from node manager
#[derive(Debug, Default)]
pub struct NodeManagerResponse {
    pub add_node_clicked: bool,
    pub sync_acl_clicked: bool,
    pub node_selected: Option<String>,
    pub connect_agent: Option<String>,
    pub kick_node: Option<String>,
    pub verify_node: Option<String>,
    pub block_node: Option<String>,
    pub unblock_node: Option<String>,
    pub allow_node: Option<String>,
}
