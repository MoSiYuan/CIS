//! # CIS GUI
//!
//! Egui-based GUI with Alacritty terminal for remote Agent sessions.
//!
//! ## Features
//!
//! - Terminal panel for local and remote Agent sessions
//! - Node tabs showing trust status (whitelist/verified/pending)
//! - Node manager for ACL configuration
//! - One-click remote Agent connection
#![allow(dead_code)]

use eframe::egui;
use tracing::info;

mod app;
mod app_element;
mod decision_panel;
mod glm_panel;
mod node_tabs;
mod node_manager;
mod terminal_panel;
mod remote_session;
mod theme;
mod layout;

use app_element::CisAppElement;

fn main() -> eframe::Result {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting CIS GUI");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "CIS - Cluster of Independent Systems",
        options,
        Box::new(|cc| Ok(Box::new(CisAppElement::new(cc)))),
    )
}
