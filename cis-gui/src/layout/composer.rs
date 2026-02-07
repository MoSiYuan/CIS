//! # Composer Component (Element-style)
//!
//! The composer is the message input area at the bottom of the content panel,
//! styled after Element's composer with support for:
//! - Multi-line text input
//! - Context attachments
//! - Agent mentions (@)
//! - Send button

use egui::{Ui, Vec2, Color32, RichText, Stroke, Margin, TextEdit, Frame};
use crate::theme::*;

/// Context attachment types
#[derive(Debug, Clone, PartialEq)]
pub enum ContextAttachment {
    /// Attached file
    File { name: String, path: String },
    /// Attached memory
    Memory { id: String, preview: String },
    /// Attached DAG result
    DagResult { dag_id: String, status: String },
    /// Attached code context
    Code { file: String, line: usize },
}

impl ContextAttachment {
    pub fn icon(&self) -> &'static str {
        match self {
            ContextAttachment::File { .. } => "📎",
            ContextAttachment::Memory { .. } => "📝",
            ContextAttachment::DagResult { .. } => "📊",
            ContextAttachment::Code { .. } => "💻",
        }
    }
    
    pub fn label(&self) -> String {
        match self {
            ContextAttachment::File { name, .. } => name.clone(),
            ContextAttachment::Memory { preview, .. } => preview.chars().take(20).collect(),
            ContextAttachment::DagResult { dag_id, .. } => dag_id.clone(),
            ContextAttachment::Code { file, line } => format!("{}:{}", file, line),
        }
    }
}

/// Composer state
#[derive(Debug, Default)]
pub struct Composer {
    /// Input text
    text: String,
    /// Attached contexts
    attachments: Vec<ContextAttachment>,
    /// Whether the composer is expanded (multi-line)
    expanded: bool,
    /// Show attachment menu
    show_attach_menu: bool,
    /// Mention query (when typing @)
    mention_query: Option<String>,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Clear the composer
    pub fn clear(&mut self) {
        self.text.clear();
        self.attachments.clear();
        self.expanded = false;
    }
    
    /// Get current text
    pub fn text(&self) -> &str {
        &self.text
    }
    
    /// Get attachments
    pub fn attachments(&self) -> &[ContextAttachment] {
        &self.attachments
    }
    
    /// Add an attachment
    pub fn attach(&mut self, attachment: ContextAttachment) {
        self.attachments.push(attachment);
    }
    
    /// Remove an attachment
    pub fn detach(&mut self, index: usize) {
        if index < self.attachments.len() {
            self.attachments.remove(index);
        }
    }
    
    /// Render the composer
    pub fn ui(&mut self, ui: &mut Ui) -> ComposerResponse {
        let mut response = ComposerResponse::default();
        
        Frame::default()
            .fill(COMPOSER_BG)
            .stroke(Stroke::new(1.0, BORDER_COLOR))
            .corner_radius(12)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Attachment chips
                    if !self.attachments.is_empty() {
                        let attachments: Vec<_> = self.attachments.iter().cloned().collect();
                        ui.horizontal_wrapped(|ui| {
                            for (i, attachment) in attachments.iter().enumerate() {
                                render_attachment_chip(ui, i, attachment);
                                ui.add_space(4.0);
                            }
                        });
                        ui.add_space(8.0);
                    }
                    
                    // Input area
                    ui.horizontal(|ui| {
                        // Attach button
                        ui.menu_button(
                            RichText::new("📎").size(18.0),
                            |ui| {
                                if ui.button("📁 File").clicked() {
                                    self.attachments.push(ContextAttachment::File {
                                        name: "example.rs".to_string(),
                                        path: "/path/to/file".to_string(),
                                    });
                                }
                                if ui.button("📝 Memory").clicked() {
                                    self.attachments.push(ContextAttachment::Memory {
                                        id: "mem-001".to_string(),
                                        preview: "Architecture decision...".to_string(),
                                    });
                                }
                                if ui.button("📊 DAG Result").clicked() {
                                    self.attachments.push(ContextAttachment::DagResult {
                                        dag_id: "dag-001".to_string(),
                                        status: "completed".to_string(),
                                    });
                                }
                                if ui.button("💻 Code").clicked() {
                                    self.attachments.push(ContextAttachment::Code {
                                        file: "main.rs".to_string(),
                                        line: 42,
                                    });
                                }
                            }
                        );
                        
                        ui.add_space(8.0);
                        
                        // Text input
                        let min_height = if self.expanded { 80.0 } else { 40.0 };
                        let available_width = ui.available_width() - 60.0; // Space for send button
                        
                        let text_edit = TextEdit::multiline(&mut self.text)
                            .hint_text("Type a message or command... Use @ to mention")
                            .desired_width(available_width)
                            .min_size(Vec2::new(available_width, min_height))
                            .text_color(TEXT_PRIMARY)
                            .background_color(COMPOSER_BG);
                        
                        let text_response = ui.add(text_edit);
                        
                        // Check for @ mention
                        if self.text.ends_with('@') {
                            self.mention_query = Some(String::new());
                        } else if let Some(ref mut query) = self.mention_query {
                            if let Some(last_at) = self.text.rfind('@') {
                                *query = self.text[last_at + 1..].to_string();
                            }
                        }
                        
                        // Send button
                        ui.add_space(8.0);
                        let send_button = ui.add_sized(
                            Vec2::new(40.0, 36.0),
                            egui::Button::new(RichText::new("⬆").size(18.0).strong())
                                .fill(if self.text.is_empty() { Color32::TRANSPARENT } else { ACCENT_PRIMARY })
                                .stroke(Stroke::new(1.0, if self.text.is_empty() { BORDER_COLOR } else { ACCENT_PRIMARY }))
                                .corner_radius(8)
                        );
                        
                        if send_button.clicked() && !self.text.is_empty() {
                            response.send_clicked = true;
                        }
                        
                        // Enter to send, Shift+Enter for new line
                        if text_response.lost_focus() 
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !ui.input(|i| i.modifiers.shift) {
                            if !self.text.is_empty() {
                                response.send_clicked = true;
                            }
                        }
                    });
                    
                    // Mention suggestions
                    if let Some(query) = self.mention_query.clone() {
                        ui.add_space(4.0);
                        let mut mention_selected = None;
                        Frame::default()
                            .fill(SURFACE_BG)
                            .stroke(Stroke::new(1.0, BORDER_COLOR))
                            .corner_radius(8)
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Mention:").small().color(TEXT_SECONDARY));
                                ui.horizontal(|ui| {
                                    let agents = vec![
                                        ("@agent", "Default Agent"),
                                        ("@claude", "Claude"),
                                        ("@opencode", "OpenCode"),
                                        ("@kimi", "Kimi"),
                                    ];
                                    
                                    for (mention, description) in agents {
                                        if query.is_empty() || mention.contains(&query) {
                                            if ui.button(
                                                RichText::new(format!("{} - {}", mention, description))
                                                    .small()
                                            ).clicked() {
                                                mention_selected = Some(mention.to_string());
                                            }
                                        }
                                    }
                                });
                            });
                        
                        if let Some(mention) = mention_selected {
                            if let Some(last_at) = self.text.rfind('@') {
                                self.text = format!("{}{} ", &self.text[..last_at], mention);
                            }
                            self.mention_query = None;
                        }
                    }
                });
            });
        
        if response.send_clicked {
            response.text = self.text.clone();
            response.attachments = self.attachments.clone();
            self.clear();
        }
        
        response
    }
}

/// Composer response
#[derive(Debug, Default)]
pub struct ComposerResponse {
    /// Send button was clicked
    pub send_clicked: bool,
    /// Text content
    pub text: String,
    /// Attachments
    pub attachments: Vec<ContextAttachment>,
}

impl ComposerResponse {
    pub fn sent(&self) -> bool {
        self.send_clicked
    }
}

/// Render an attachment chip
pub fn render_attachment_chip(ui: &mut Ui, _index: usize, attachment: &ContextAttachment) {
    Frame::default()
        .fill(ACCENT_PRIMARY.gamma_multiply(0.1))
        .stroke(Stroke::new(1.0, ACCENT_PRIMARY))
        .corner_radius(16)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(attachment.icon()).size(14.0));
                ui.label(RichText::new(attachment.label()).size(12.0).color(TEXT_PRIMARY));
                
                // Remove button
                if ui.button(RichText::new("✕").size(10.0)).clicked() {
                    // Mark for removal (handled in parent)
                }
            });
        });
}

/// Quick actions bar above composer
pub struct QuickActions;

impl QuickActions {
    pub fn render(ui: &mut Ui) -> Option<QuickAction> {
        let mut action = None;
        
        ui.horizontal(|ui| {
            let actions = vec![
                ("🤖", "AI", QuickAction::AskAi),
                ("📊", "New DAG", QuickAction::NewDag),
                ("📝", "Memory", QuickAction::OpenMemory),
                ("🔍", "Search", QuickAction::Search),
            ];
            
            for (icon, label, act) in actions {
                if ui.button(
                    RichText::new(format!("{} {}", icon, label))
                        .small()
                        .color(TEXT_SECONDARY)
                ).clicked() {
                    action = Some(act);
                }
                ui.add_space(8.0);
            }
        });
        
        action
    }
}

/// Quick action types
#[derive(Debug, Clone)]
pub enum QuickAction {
    AskAi,
    NewDag,
    OpenMemory,
    Search,
}
