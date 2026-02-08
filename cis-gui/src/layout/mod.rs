//! # Element-style Three-Panel Layout
//!
//! Three-panel layout inspired by Element chat interface:
//! - Sidebar: Navigation (Home, DAGs, Chat, Settings)
//! - Session List: DAG runs / Sessions
//! - Content Area: DAG visualization, timeline, composer

pub mod composer;
pub mod content_area;

use egui::{Context, Ui, SidePanel, CentralPanel, Frame, Margin, Vec2, Color32, RichText, Stroke, CornerRadius};
use tracing::info;

use crate::theme::*;

pub use composer::{Composer, QuickAction};
pub use content_area::{render_content, ContentResponse};

/// Main view types (like Element's room types)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainView {
    /// Home dashboard
    Home,
    /// DAG sessions view
    Dags,
    /// Chat with AI agent
    Chat,
    /// Settings
    Settings,
}

impl MainView {
    pub fn icon(&self) -> &'static str {
        match self {
            MainView::Home => "🏠",
            MainView::Dags => "📊",
            MainView::Chat => "💬",
            MainView::Settings => "⚙️",
        }
    }
    
    pub fn label(&self) -> &'static str {
        match self {
            MainView::Home => "Home",
            MainView::Dags => "DAGs",
            MainView::Chat => "Chat",
            MainView::Settings => "Settings",
        }
    }
    
    pub fn shortcut(&self) -> &'static str {
        match self {
            MainView::Home => "⌘1",
            MainView::Dags => "⌘2",
            MainView::Chat => "⌘3",
            MainView::Settings => "⌘4",
        }
    }
}

/// Layout manager for three-panel view
pub struct ThreePanelLayout {
    /// Current main view
    pub current_view: MainView,
    /// Sidebar width
    sidebar_width: f32,
    /// Session list width
    session_list_width: f32,
    /// Sidebar expanded state
    sidebar_expanded: bool,
    /// Selected session ID
    selected_session: Option<String>,
    /// Show composer
    show_composer: bool,
}

impl Default for ThreePanelLayout {
    fn default() -> Self {
        Self {
            current_view: MainView::Dags,
            sidebar_width: 72.0,
            session_list_width: 280.0,
            sidebar_expanded: true,
            selected_session: None,
            show_composer: true,
        }
    }
}

impl ThreePanelLayout {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Switch to a different view
    pub fn switch_view(&mut self, view: MainView) {
        info!("Switching to view: {:?}", view);
        self.current_view = view;
    }
    
    /// Handle keyboard shortcuts
    pub fn handle_shortcuts(&mut self, ctx: &Context) {
        ctx.input(|i| {
            // 1: Home
            if i.key_pressed(egui::Key::Num1) {
                self.switch_view(MainView::Home);
            }
            // 2: DAGs
            if i.key_pressed(egui::Key::Num2) {
                self.switch_view(MainView::Dags);
            }
            // 3: Chat
            if i.key_pressed(egui::Key::Num3) {
                self.switch_view(MainView::Chat);
            }
            // 4: Settings
            if i.key_pressed(egui::Key::Num4) {
                self.switch_view(MainView::Settings);
            }
        });
    }
    
    /// Render the three-panel layout
    pub fn render<F>(&mut self, ctx: &Context, content_fn: F)
    where
        F: FnOnce(&mut Ui, &MainView, &mut Option<String>),
    {
        self.handle_shortcuts(ctx);
        
        // Left sidebar (navigation)
        self.render_sidebar(ctx);
        
        // Middle panel (session list) - only show in DAGs and Chat views
        if matches!(self.current_view, MainView::Dags | MainView::Chat) {
            self.render_session_list(ctx);
        }
        
        // Right content area
        CentralPanel::default()
            .frame(Frame::default()
                .fill(MAIN_BG)
                .inner_margin(Margin::same(0)))
            .show(ctx, |ui| {
                content_fn(ui, &self.current_view, &mut self.selected_session);
            });
    }
    
    /// Render sidebar navigation
    fn render_sidebar(&mut self, ctx: &Context) {
        SidePanel::left("sidebar")
            .exact_width(self.sidebar_width)
            .resizable(false)
            .frame(Frame::default()
                .fill(SIDEBAR_BG)
                .inner_margin(Margin::same(8)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    
                    // App icon/logo
                    ui.label(RichText::new("◈").size(32.0).color(ACCENT_PRIMARY));
                    ui.add_space(16.0);
                    
                    // Navigation buttons
                    let nav_items = [
                        MainView::Home,
                        MainView::Dags,
                        MainView::Chat,
                    ];
                    
                    for view in nav_items {
                        let is_selected = self.current_view == view;
                        
                        let button = if is_selected {
                            egui::Button::new(RichText::new(view.icon()).size(24.0))
                                .fill(ACCENT_PRIMARY.gamma_multiply(0.2))
                                .stroke(Stroke::new(2.0, ACCENT_PRIMARY))
                                .corner_radius(12)
                        } else {
                            egui::Button::new(RichText::new(view.icon()).size(24.0))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .corner_radius(12)
                        };
                        
                        if ui.add_sized(Vec2::new(48.0, 48.0), button).clicked() {
                            self.switch_view(view);
                        }
                        
                        ui.add_space(4.0);
                    }
                    
                    // Spacer to push settings to bottom
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        
                        // Settings button
                        let is_selected = self.current_view == MainView::Settings;
                        let button = if is_selected {
                            egui::Button::new(RichText::new(MainView::Settings.icon()).size(24.0))
                                .fill(ACCENT_PRIMARY.gamma_multiply(0.2))
                                .stroke(Stroke::new(2.0, ACCENT_PRIMARY))
                                .corner_radius(12)
                        } else {
                            egui::Button::new(RichText::new(MainView::Settings.icon()).size(24.0))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .corner_radius(12)
                        };
                        
                        if ui.add_sized(Vec2::new(48.0, 48.0), button).clicked() {
                            self.switch_view(MainView::Settings);
                        }
                    });
                });
            });
    }
    
    /// Render session list panel
    fn render_session_list(&mut self, ctx: &Context) {
        let panel_width = if matches!(self.current_view, MainView::Dags | MainView::Chat) {
            self.session_list_width
        } else {
            0.0
        };
        
        if panel_width > 0.0 {
            SidePanel::left("session_list")
                .exact_width(panel_width)
                .resizable(true)
                .min_width(200.0)
                .max_width(400.0)
                .frame(Frame::default()
                    .fill(SESSION_LIST_BG)
                    .inner_margin(Margin::same(12)))
                .show(ctx, |ui| {
                    match self.current_view {
                        MainView::Dags => {
                            self.render_dag_session_list(ui);
                        }
                        MainView::Chat => {
                            self.render_chat_session_list(ui);
                        }
                        _ => {}
                    }
                });
        }
    }
    
    /// Render DAG session list
    fn render_dag_session_list(&mut self, ui: &mut Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.heading(RichText::new("DAG Sessions").size(16.0).color(TEXT_PRIMARY));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕").clicked() {
                    // New DAG run
                }
            });
        });
        
        ui.add_space(8.0);
        
        // Search/filter
        ui.add(
            egui::TextEdit::singleline(&mut String::new())
                .hint_text("🔍 Search sessions...")
                .desired_width(ui.available_width())
        );
        
        ui.add_space(12.0);
        
        // Filter tabs
        ui.horizontal(|ui| {
            let filters = ["All", "Running", "Completed", "Failed"];
            for (i, filter) in filters.iter().enumerate() {
                let selected = i == 0;
                let text = if selected {
                    RichText::new(*filter).color(ACCENT_PRIMARY).strong()
                } else {
                    RichText::new(*filter).color(TEXT_SECONDARY)
                };
                if ui.selectable_label(selected, text).clicked() {
                    // Change filter
                }
            }
        });
        
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        
        // Session list
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Demo sessions
                let sessions = vec![
                    ("dag-001", "Build & Test", "running", "2m ago", true),
                    ("dag-002", "Deploy Prod", "completed", "2h ago", false),
                    ("dag-003", "Code Review", "failed", "Yesterday", false),
                    ("dag-004", "Benchmark", "completed", "2d ago", false),
                ];
                
                for (id, name, status, time, is_live) in sessions {
                    let is_selected = self.selected_session.as_ref() == Some(&id.to_string());
                    
                    let response = ui.add(SessionItem {
                        name: name.to_string(),
                        status: status.to_string(),
                        time: time.to_string(),
                        is_live,
                        is_selected,
                    });
                    
                    if response.clicked() {
                        self.selected_session = Some(id.to_string());
                    }
                    
                    ui.add_space(4.0);
                }
            });
    }
    
    /// Render Chat session list
    fn render_chat_session_list(&mut self, ui: &mut Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Conversations").size(16.0).color(TEXT_PRIMARY));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("➕").clicked() {
                    // New conversation
                }
            });
        });
        
        ui.add_space(8.0);
        
        // Search
        ui.add(
            egui::TextEdit::singleline(&mut String::new())
                .hint_text("🔍 Search conversations...")
                .desired_width(ui.available_width())
        );
        
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);
        
        // Conversation list
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Demo conversations
                let conversations = vec![
                    ("chat-001", "Current Session", "Active now", true),
                    ("chat-002", "Yesterday", "Yesterday", false),
                    ("chat-003", "Code Review Help", "2 days ago", false),
                ];
                
                for (id, name, time, is_live) in conversations {
                    let is_selected = self.selected_session.as_ref() == Some(&id.to_string());
                    
                    let response = ui.add(ChatSessionItem {
                        name: name.to_string(),
                        time: time.to_string(),
                        is_live,
                        is_selected,
                    });
                    
                    if response.clicked() {
                        self.selected_session = Some(id.to_string());
                    }
                    
                    ui.add_space(4.0);
                }
            });
    }
}

/// DAG session item widget
struct SessionItem {
    name: String,
    status: String,
    time: String,
    is_live: bool,
    is_selected: bool,
}

impl egui::Widget for SessionItem {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 64.0),
            egui::Sense::click(),
        );
        
        if ui.is_rect_visible(rect) {
            let visuals = if self.is_selected {
                ui.visuals().widgets.active
            } else if response.hovered() {
                ui.visuals().widgets.hovered
            } else {
                ui.visuals().widgets.inactive
            };
            
            // Background
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(8),
                if self.is_selected { ACCENT_PRIMARY.gamma_multiply(0.1) } else { visuals.bg_fill }
            );
            
            if self.is_selected {
                ui.painter().rect_stroke(
                    rect,
                    8,
                    Stroke::new(1.0, ACCENT_PRIMARY),
                    egui::StrokeKind::Inside,
                );
            }
            
            // Content
            let content_rect = rect.shrink(12.0);
            
            // Status indicator
            let status_color = match self.status.as_str() {
                "running" => STATUS_RUNNING,
                "completed" => STATUS_SUCCESS,
                "failed" => STATUS_ERROR,
                _ => STATUS_IDLE,
            };
            
            let status_pos = content_rect.left_center() + egui::vec2(6.0, 0.0);
            ui.painter().circle_filled(status_pos, 6.0, status_color);
            
            if self.is_live {
                // Pulsing effect for live sessions
                let pulse = (ui.input(|i| i.time) * 2.0).sin() as f32 * 0.5 + 0.5;
                ui.painter().circle_stroke(
                    status_pos,
                    8.0 + pulse * 2.0,
                    Stroke::new(1.0, status_color.gamma_multiply(0.5)),
                );
            }
            
            // Name and time
            let text_pos = content_rect.left_center() + egui::vec2(24.0, -8.0);
            ui.painter().text(
                text_pos,
                egui::Align2::LEFT_CENTER,
                &self.name,
                egui::FontId::proportional(14.0),
                TEXT_PRIMARY,
            );
            
            let subtext_pos = content_rect.left_center() + egui::vec2(24.0, 10.0);
            ui.painter().text(
                subtext_pos,
                egui::Align2::LEFT_CENTER,
                format!("{} • {}", self.status, self.time),
                egui::FontId::proportional(12.0),
                TEXT_SECONDARY,
            );
        }
        
        response
    }
}

/// Chat session item widget
struct ChatSessionItem {
    name: String,
    time: String,
    is_live: bool,
    is_selected: bool,
}

impl egui::Widget for ChatSessionItem {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 56.0),
            egui::Sense::click(),
        );
        
        if ui.is_rect_visible(rect) {
            let visuals = if self.is_selected {
                ui.visuals().widgets.active
            } else if response.hovered() {
                ui.visuals().widgets.hovered
            } else {
                ui.visuals().widgets.inactive
            };
            
            // Background
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(8),
                if self.is_selected { ACCENT_PRIMARY.gamma_multiply(0.1) } else { visuals.bg_fill }
            );
            
            // Content
            let content_rect = rect.shrink(12.0);
            
            // Avatar/icon
            let avatar_pos = content_rect.left_center();
            ui.painter().circle_filled(avatar_pos, 16.0, ACCENT_SECONDARY);
            ui.painter().text(
                avatar_pos,
                egui::Align2::CENTER_CENTER,
                "🤖",
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );
            
            // Name
            let name_pos = content_rect.left_center() + egui::vec2(28.0, -8.0);
            ui.painter().text(
                name_pos,
                egui::Align2::LEFT_CENTER,
                &self.name,
                egui::FontId::proportional(14.0),
                TEXT_PRIMARY,
            );
            
            // Time with live indicator
            let time_text = if self.is_live {
                "● ".to_string() + &self.time
            } else {
                self.time.clone()
            };
            
            let time_pos = content_rect.left_center() + egui::vec2(28.0, 10.0);
            ui.painter().text(
                time_pos,
                egui::Align2::LEFT_CENTER,
                &time_text,
                egui::FontId::proportional(12.0),
                if self.is_live { STATUS_RUNNING } else { TEXT_SECONDARY },
            );
        }
        
        response
    }
}
