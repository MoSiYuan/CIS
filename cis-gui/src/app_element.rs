//! # Main Application with Element-style Layout
//!
//! New app implementation using the three-panel Element-style layout.

use eframe::egui::{self, Context, Frame, TopBottomPanel, ViewportCommand};
use tracing::info;

use crate::layout::{ThreePanelLayout, MainView, Composer, render_content, ContentResponse};
use crate::theme::*;

/// Main CIS application with Element-style layout
pub struct CisAppElement {
    /// Three-panel layout manager
    layout: ThreePanelLayout,
    /// Message composer
    composer: Composer,
    /// Terminal history (for backward compatibility)
    terminal_history: Vec<String>,
    /// Pending message to handle
    pending_response: Option<ContentResponse>,
}

impl CisAppElement {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        info!("Initializing CIS GUI with Element-style layout");
        
        Self {
            layout: ThreePanelLayout::new(),
            composer: Composer::new(),
            terminal_history: vec![
                "CIS Agent Terminal v0.1.0".to_string(),
                "Type 'help' for available commands".to_string(),
                "".to_string(),
            ],
            pending_response: None,
        }
    }
    
    /// Process any pending response
    fn process_pending(&mut self) {
        if let Some(response) = self.pending_response.take() {
            self.handle_content_response(response);
        }
    }
    
    /// Handle content area response
    fn handle_content_response(&mut self, response: ContentResponse) {
        if response.message_sent {
            info!("Message sent: {}", response.message_text);
            self.terminal_history.push(format!("> {}", response.message_text));
        }
        
        if let Some(action) = response.quick_action {
            match action {
                crate::layout::QuickAction::AskAi => {
                    self.layout.switch_view(MainView::Chat);
                }
                crate::layout::QuickAction::NewDag => {
                    self.layout.switch_view(MainView::Dags);
                }
                _ => {}
            }
        }
    }
}

impl eframe::App for CisAppElement {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Process any pending actions first
        self.process_pending();
        
        // Top bar with app info
        TopBottomPanel::top("top_bar")
            .exact_height(40.0)
            .frame(Frame::default().fill(SIDEBAR_BG))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("CIS - Cluster of Independent Systems")
                            .strong()
                            .color(TEXT_PRIMARY)
                    );
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        
                        // Window controls
                        if ui.button("✕").clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        if ui.button("⬚").clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(true));
                        }
                        if ui.button("➖").clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
        
        // Collect response from content area
        let mut response = None;
        
        // Render three-panel layout
        self.layout.render(ctx, |ui, view, selected_session| {
            let resp = render_content(ui, view, selected_session, &mut self.composer);
            response = Some(resp);
        });
        
        // Store response for next frame
        if let Some(resp) = response {
            self.pending_response = Some(resp);
            ctx.request_repaint();
        }
    }
}
