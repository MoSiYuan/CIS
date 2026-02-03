//! # Node Tabs
//!
//! Horizontal bar showing connected nodes with trust status indicators.

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use tracing::info;

use crate::theme::*;

/// Information about a node for display
#[derive(Debug, Clone)]
pub struct NodeTabInfo {
    pub id: String,
    pub name: String,
    pub did: Option<String>,
    pub is_local: bool,
    pub is_verified: bool,
    pub is_online: bool,
    pub has_session: bool,  // Has active remote session
}

impl NodeTabInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            did: None,
            is_local: false,
            is_verified: false,
            is_online: false,
            has_session: false,
        }
    }
    
    pub fn with_did(mut self, did: impl Into<String>) -> Self {
        self.did = Some(did.into());
        self
    }
    
    pub fn local(mut self) -> Self {
        self.is_local = true;
        self
    }
    
    pub fn verified(mut self) -> Self {
        self.is_verified = true;
        self
    }
    
    pub fn online(mut self) -> Self {
        self.is_online = true;
        self
    }
    
    pub fn with_session(mut self) -> Self {
        self.has_session = true;
        self
    }
}

/// Node tabs component
pub struct NodeTabs {
    nodes: Vec<NodeTabInfo>,
    active_node: String,
    show_manager: bool,
}

impl NodeTabs {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            active_node: String::new(),
            show_manager: false,
        }
    }
    
    pub fn with_nodes(nodes: Vec<NodeTabInfo>) -> Self {
        let active_node = nodes.first()
            .map(|n| n.id.clone())
            .unwrap_or_default();
        
        Self {
            nodes,
            active_node,
            show_manager: false,
        }
    }
    
    pub fn add_node(&mut self, node: NodeTabInfo) {
        if self.nodes.is_empty() {
            self.active_node = node.id.clone();
        }
        self.nodes.push(node);
    }
    
    pub fn remove_node(&mut self, id: &str) {
        self.nodes.retain(|n| n.id != id);
        if self.active_node == id && !self.nodes.is_empty() {
            self.active_node = self.nodes[0].id.clone();
        }
    }
    
    pub fn set_active(&mut self, id: impl Into<String>) {
        self.active_node = id.into();
    }
    
    pub fn active_node(&self) -> &str {
        &self.active_node
    }
    
    pub fn show_manager(&self) -> bool {
        self.show_manager
    }
    
    pub fn toggle_manager(&mut self) {
        self.show_manager = !self.show_manager;
    }
    
    pub fn ui(&mut self, ui: &mut egui::Ui) -> NodeTabsResponse {
        let mut response = NodeTabsResponse::default();
        
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            
            // Local session tab (always present)
            self.render_local_tab(ui, &mut response);
            
            ui.separator();
            
            // Node tabs
            let nodes = self.nodes.clone(); // Clone to avoid borrow issues
            for node in &nodes {
                let is_active = node.id == self.active_node;
                if self.render_node_tab(ui, node, is_active) {
                    self.active_node = node.id.clone();
                    response.node_selected = Some(node.id.clone());
                    info!("Selected node: {}", node.id);
                }
            }
            
            // Add/Manager button
            ui.separator();
            
            let manager_btn = egui::Button::new("☰")
                .min_size(Vec2::new(32.0, 32.0))
                .fill(PANEL_BG)
                .corner_radius(CornerRadius::same(6));
            
            if ui.add(manager_btn).clicked() {
                self.toggle_manager();
                response.manager_toggled = true;
            }
            
            // Tooltip
            if ui.ui_contains_pointer() {
                egui::show_tooltip(ui.ctx(), ui.layer_id(), "node_tabs_tooltip".into(), |ui| {
                    ui.label("Nodes: Click to switch sessions");
                    ui.label("☰: Open node manager");
                });
            }
        });
        
        response
    }
    
    fn render_local_tab(&self, ui: &mut egui::Ui, response: &mut NodeTabsResponse) -> bool {
        let is_active = self.active_node.is_empty() || self.active_node == "local";
        
        let (bg, text) = (VERIFIED_LOCAL_BG, VERIFIED_LOCAL_TEXT);
        let bg = if is_active { bg } else { bg.gamma_multiply(0.7) };
        
        let btn = egui::Button::new(
            RichText::new("● Local")
                .color(text)
                .size(13.0)
        )
        .fill(bg)
        .stroke(if is_active {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::NONE
        })
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(80.0, 32.0));
        
        let clicked = ui.add(btn).clicked();
        if clicked {
            response.node_selected = Some("local".to_string());
        }
        clicked
    }
    
    fn render_node_tab(&self, ui: &mut egui::Ui, node: &NodeTabInfo, is_active: bool) -> bool {
        let (bg, text) = if node.is_verified {
            if node.is_local {
                (VERIFIED_LOCAL_BG, VERIFIED_LOCAL_TEXT)
            } else {
                (VERIFIED_CLOUD_BG, VERIFIED_CLOUD_TEXT)
            }
        } else {
            (UNKNOWN_BG, UNKNOWN_TEXT)
        };
        
        let bg = if is_active { bg } else { bg.gamma_multiply(0.7) };
        
        // Status indicator
        let status_icon = if node.is_online { "●" } else { "○" };
        let _status_color = if node.is_online { STATUS_ONLINE } else { STATUS_OFFLINE };
        
        // Session indicator
        let session_icon = if node.has_session { " ⧉" } else { "" };
        
        let label = format!("{}{}{}", status_icon, node.name, session_icon);
        
        let btn = egui::Button::new(
            RichText::new(&label)
                .color(text)
                .size(13.0)
        )
        .fill(bg)
        .stroke(if is_active {
            Stroke::new(2.0, Color32::WHITE)
        } else {
            Stroke::NONE
        })
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(100.0, 32.0));
        
        let response = ui.add(btn);
        
        // Context menu
        response.context_menu(|ui| {
            ui.label(format!("DID: {}", node.did.as_deref().unwrap_or("Unknown")));
            ui.label(format!("Status: {}", if node.is_online { "Online" } else { "Offline" }));
            ui.label(format!("Verified: {}", if node.is_verified { "Yes" } else { "No" }));
            ui.separator();
            
            if ui.button("Connect Agent").clicked() {
                // TODO: Emit event
                ui.close_menu();
            }
            
            if ui.button("Disconnect").clicked() {
                // TODO: Emit event
                ui.close_menu();
            }
            
            if !node.is_verified && ui.button("Verify DID").clicked() {
                // TODO: Open verification dialog
                ui.close_menu();
            }
        });
        
        response.clicked()
    }
}

impl Default for NodeTabs {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from node tabs UI
#[derive(Debug, Default)]
pub struct NodeTabsResponse {
    pub node_selected: Option<String>,
    pub manager_toggled: bool,
}
