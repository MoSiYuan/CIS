//! # Content Area Components
//!
//! Content area views for the three-panel layout:
//! - Home view: Dashboard with quick actions and recent activity
//! - DAGs view: DAG visualization and timeline
//! - Chat view: AI conversation with composer
//! - Settings view: Configuration panels

use egui::{Ui, Frame, Margin, RichText, Vec2, Stroke, ScrollArea, Layout};
use crate::theme::*;
use crate::layout::{MainView, composer::{Composer, QuickActions, QuickAction}};

/// Render content area based on current view
pub fn render_content(
    ui: &mut Ui,
    view: &MainView,
    selected_session: &mut Option<String>,
    composer: &mut Composer,
) -> ContentResponse {
    let mut response = ContentResponse::default();
    
    match view {
        MainView::Home => render_home_view(ui),
        MainView::Dags => {
            if let Some(ref session_id) = selected_session {
                render_dag_detail_view(ui, session_id);
            } else {
                render_dag_list_view(ui);
            }
            render_composer_area(ui, composer, &mut response);
        }
        MainView::Chat => {
            render_chat_view(ui, selected_session);
            render_composer_area(ui, composer, &mut response);
        }
        MainView::Settings => render_settings_view(ui),
    }
    
    response
}

/// Render home dashboard view
fn render_home_view(ui: &mut Ui) {
    ui.vertical(|ui| {
        // Header
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.heading(RichText::new("Home").size(24.0).color(TEXT_PRIMARY));
        });
        ui.add_space(16.0);
        
        ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                
                ui.vertical(|ui| {
                    // Quick Actions
                    render_dashboard_card(ui, "Quick Actions", |ui| {
                        let actions = vec![
                            ("📊", "New DAG", "Create a new workflow"),
                            ("🤖", "AI Assistant", "Ask the AI for help"),
                            ("📁", "Open Project", "Open a project folder"),
                            ("🔍", "Search Memory", "Search past memories"),
                        ];
                        
                        for (icon, title, desc) in actions {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).size(20.0));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(title).strong().color(TEXT_PRIMARY));
                                    ui.label(RichText::new(desc).small().color(TEXT_SECONDARY));
                                });
                            });
                            ui.add_space(12.0);
                        }
                    });
                    
                    ui.add_space(16.0);
                    
                    // Activity Timeline
                    render_dashboard_card(ui, "Activity Timeline", |ui| {
                        let activities = vec![
                            ("12:30", "🔄", "DAG 'Build' completed successfully"),
                            ("12:15", "💬", "AI suggested optimization in main.rs"),
                            ("11:45", "📝", "New memory: 'Architecture decision: use gRPC'"),
                            ("10:20", "⚠️", "DAG 'Test' failed - 3 tests need attention"),
                        ];
                        
                        for (time, icon, desc) in activities {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(time).monospace().color(TEXT_SECONDARY));
                                ui.label(RichText::new(icon).size(14.0));
                                ui.label(RichText::new(desc).color(TEXT_PRIMARY));
                            });
                            ui.add_space(8.0);
                        }
                    });
                });
                
                ui.add_space(16.0);
                
                ui.vertical(|ui| {
                    // Recent DAGs
                    render_dashboard_card(ui, "Recent DAGs", |ui| {
                        let dags = vec![
                            ("▶", "Build & Test", "2 hours ago", STATUS_RUNNING),
                            ("▶", "Deploy Prod", "Yesterday", STATUS_SUCCESS),
                            ("▶", "Code Review", "2 days ago", STATUS_IDLE),
                        ];
                        
                        for (icon, name, time, color) in dags {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).color(color));
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(name).color(TEXT_PRIMARY));
                                    ui.label(RichText::new(time).small().color(TEXT_SECONDARY));
                                });
                            });
                            ui.add_space(12.0);
                        }
                    });
                    
                    ui.add_space(16.0);
                    
                    // System Health
                    render_dashboard_card(ui, "System Health", |ui| {
                        let items = vec![
                            ("✅", "Vector Engine", "Ready"),
                            ("✅", "OpenCode Agent", "Connected"),
                            ("⚠️", "Active Sessions", "3 running"),
                        ];
                        
                        for (icon, name, status) in items {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon).size(14.0));
                                ui.label(RichText::new(name).color(TEXT_PRIMARY));
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(status).small().color(TEXT_SECONDARY));
                                });
                            });
                            ui.add_space(8.0);
                        }
                    });
                });
            });
        });
    });
}

/// Render DAG list view (when no session selected)
fn render_dag_list_view(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(100.0);
        ui.label(RichText::new("📊").size(64.0));
        ui.add_space(16.0);
        ui.heading(RichText::new("No DAG Selected").color(TEXT_PRIMARY));
        ui.add_space(8.0);
        ui.label(RichText::new("Select a DAG session from the list or create a new one").color(TEXT_SECONDARY));
        ui.add_space(16.0);
        if ui.button(RichText::new("➕ New DAG Run").strong()).clicked() {
            // Create new DAG
        }
    });
}

/// Render DAG detail view
fn render_dag_detail_view(ui: &mut Ui, session_id: &str) {
    ui.vertical(|ui| {
        // Header
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.heading(RichText::new(format!("DAG: {}", session_id)).color(TEXT_PRIMARY));
                ui.label(RichText::new("Running • 2m 30s").small().color(STATUS_RUNNING));
            });
            
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                if ui.button("⏹ Stop").clicked() {}
                if ui.button("⏸ Pause").clicked() {}
                if ui.button("👁 Attach").clicked() {}
            });
        });
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // DAG Visualization placeholder
        ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                
                // Demo DAG graph
                render_dag_visualization(ui);
            });
        });
    });
}

/// Render DAG visualization
fn render_dag_visualization(ui: &mut Ui) {
    Frame::default()
        .fill(SURFACE_BG)
        .corner_radius(12)
        .inner_margin(Margin::same(24))
        .show(ui, |ui| {
            ui.set_min_size(Vec2::new(600.0, 300.0));
            
            // Demo: Draw simple DAG nodes
            let nodes = vec![
                ("Compile", 100.0, 50.0, STATUS_SUCCESS),
                ("Test", 300.0, 50.0, STATUS_RUNNING),
                ("Package", 500.0, 50.0, STATUS_IDLE),
            ];
            
            let painter = ui.painter();
            
            // Draw connections
            for i in 0..nodes.len()-1 {
                let start = egui::pos2(nodes[i].1 + 60.0, nodes[i].2 + 20.0);
                let end = egui::pos2(nodes[i+1].1, nodes[i+1].2 + 20.0);
                painter.line_segment(
                    [start, end],
                    Stroke::new(2.0, BORDER_COLOR),
                );
            }
            
            // Draw nodes
            for (name, x, y, color) in nodes {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x, y),
                    Vec2::new(120.0, 40.0),
                );
                
                painter.rect_filled(rect, 8, SURFACE_BG);
                painter.rect_stroke(rect, 8, Stroke::new(2.0, color), egui::StrokeKind::Inside);
                
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    name,
                    egui::FontId::proportional(14.0),
                    TEXT_PRIMARY,
                );
            }
        });
}

/// Render chat view
fn render_chat_view(ui: &mut Ui, selected_session: &Option<String>) {
    ui.vertical(|ui| {
        // Header
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            if let Some(ref session) = selected_session {
                ui.heading(RichText::new(format!("Chat: {}", session)).color(TEXT_PRIMARY));
            } else {
                ui.heading(RichText::new("New Conversation").color(TEXT_PRIMARY));
            }
            
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                if ui.button("🗑 Clear").clicked() {}
            });
        });
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // Chat messages
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    
                    ui.vertical(|ui| {
                        // System message
                        render_system_message(ui, "Context: project=myapp, dag=123\nAgent: OpenCode (glm-4.7-free)");
                        
                        ui.add_space(16.0);
                        
                        // User message
                        render_user_message(ui, "Fix the failing test in auth module");
                        
                        ui.add_space(16.0);
                        
                        // Assistant message
                        render_assistant_message(ui, "I'll help fix the failing test. Let me analyze...\n\nFound issue: The test was checking for deprecated field. Updating...\n\n✅ Fixed! Changes applied:\n• Updated test_auth.rs\n• Removed deprecated field");
                    });
                });
            });
    });
}

/// Render system message
fn render_system_message(ui: &mut Ui, content: &str) {
    Frame::default()
        .fill(SURFACE_BG)
        .corner_radius(8)
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new("System").small().strong().color(TEXT_SECONDARY));
            ui.add_space(4.0);
            ui.label(RichText::new(content).small().color(TEXT_SECONDARY));
        });
}

/// Render user message
fn render_user_message(ui: &mut Ui, content: &str) {
    ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
        Frame::default()
            .fill(ACCENT_PRIMARY.gamma_multiply(0.2))
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() * 0.7);
                ui.label(RichText::new(content).color(TEXT_PRIMARY));
            });
    });
}

/// Render assistant message
fn render_assistant_message(ui: &mut Ui, content: &str) {
    ui.horizontal(|ui| {
        // Avatar
        ui.label(RichText::new("🤖").size(24.0));
        
        Frame::default()
            .fill(SURFACE_BG)
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() * 0.7);
                ui.label(RichText::new(content).color(TEXT_PRIMARY));
            });
    });
}

/// Render composer area at bottom
fn render_composer_area(ui: &mut Ui, composer: &mut Composer, response: &mut ContentResponse) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    
    // Quick actions
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        if let Some(action) = QuickActions::render(ui) {
            response.quick_action = Some(action);
        }
    });
    
    ui.add_space(8.0);
    
    // Composer
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.vertical(|ui| {
            let composer_response = composer.ui(ui);
            if composer_response.sent() {
                response.message_sent = true;
                response.message_text = composer_response.text;
                response.message_attachments = composer_response.attachments;
            }
        });
        ui.add_space(8.0);
    });
}

/// Render settings view
fn render_settings_view(ui: &mut Ui) {
    ui.vertical(|ui| {
        // Header
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.heading(RichText::new("Settings").size(24.0).color(TEXT_PRIMARY));
        });
        ui.add_space(16.0);
        
        ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                
                // Left sidebar for settings categories
                ui.vertical(|ui| {
                    let categories = ["General", "AI Providers", "Vector Engine", "Memory", "DAG", "Network", "Appearance"];
                    for (i, cat) in categories.iter().enumerate() {
                        let selected = i == 1; // AI Providers selected
                        let text = if selected {
                            RichText::new(*cat).color(ACCENT_PRIMARY).strong()
                        } else {
                            RichText::new(*cat).color(TEXT_PRIMARY)
                        };
                        if ui.selectable_label(selected, text).clicked() {}
                        ui.add_space(4.0);
                    }
                });
                
                ui.add_space(32.0);
                ui.separator();
                ui.add_space(32.0);
                
                // Settings content
                ui.vertical(|ui| {
                    ui.heading(RichText::new("AI Providers").size(18.0).color(TEXT_PRIMARY));
                    ui.add_space(16.0);
                    
                    // OpenCode provider
                    render_setting_card(ui, "OpenCode (Default)", |ui| {
                        ui.label(RichText::new("Model: glm-4.7-free").color(TEXT_SECONDARY));
                        ui.label(RichText::new("Max Tokens: 4096").color(TEXT_SECONDARY));
                        ui.label(RichText::new("Temperature: 0.7").color(TEXT_SECONDARY));
                        ui.add_space(8.0);
                        if ui.button("Edit").clicked() {}
                    });
                    
                    ui.add_space(16.0);
                    
                    // Claude provider
                    render_setting_card(ui, "Claude CLI", |ui| {
                        ui.label(RichText::new("Model: claude-3-sonnet").color(TEXT_SECONDARY));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("Set as Default").clicked() {}
                            if ui.button("Edit").clicked() {}
                        });
                    });
                    
                    ui.add_space(16.0);
                    
                    if ui.button("➕ Add Provider").clicked() {}
                });
            });
        });
    });
}

/// Helper: Render dashboard card
fn render_dashboard_card<R>(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::default()
        .fill(SURFACE_BG)
        .corner_radius(12)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width(280.0);
            ui.heading(RichText::new(title).size(16.0).color(TEXT_PRIMARY));
            ui.add_space(12.0);
            content(ui)
        })
        .inner
}

/// Helper: Render setting card
fn render_setting_card<R>(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::default()
        .fill(SURFACE_BG)
        .stroke(Stroke::new(1.0, BORDER_COLOR))
        .corner_radius(8)
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_width(400.0);
            ui.heading(RichText::new(title).size(14.0).color(TEXT_PRIMARY));
            ui.add_space(8.0);
            content(ui)
        })
        .inner
}

/// Content area response
#[derive(Debug, Default)]
pub struct ContentResponse {
    /// Message was sent from composer
    pub message_sent: bool,
    /// Message text
    pub message_text: String,
    /// Message attachments
    pub message_attachments: Vec<crate::layout::composer::ContextAttachment>,
    /// Quick action triggered
    pub quick_action: Option<QuickAction>,
}
