//! # Decision Panel
//!
//! Four-tier decision mechanism UI component for CIS-DAG.
//!
//! Decision levels:
//! - Mechanical: Auto-execute, retry on failure (silent background)
//! - Recommended: Countdown notification bar, can intervene
//! - Confirmed: Modal dialog, must manually confirm
//! - Arbitrated: Freeze DAG, open arbitration workspace

use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Vec2, Window};

use crate::theme::*;
use cis_core::types::TaskLevel;

/// Decision action returned by the UI
#[derive(Debug, Clone)]
pub enum DecisionAction {
    /// Auto-proceed (Mechanical level)
    AutoProceed,
    /// User chose to proceed
    Proceed,
    /// User chose to skip this task
    Skip,
    /// User chose to abort the DAG
    Abort,
    /// User modified the task
    Modify { changes: TaskChanges },
    /// Arbitration: mark as resolved
    MarkResolved,
    /// Arbitration: request assistance
    RequestAssistance,
    /// Arbitration: rollback to previous task
    Rollback,
    /// Arbitration: view details
    ViewDetails,
}

/// Task modification changes
#[derive(Debug, Clone, Default)]
pub struct TaskChanges {
    pub new_command: Option<String>,
    pub new_timeout: Option<u64>,
    pub new_params: Vec<(String, String)>,
}

/// Pending decision information
#[derive(Debug, Clone)]
pub struct PendingDecision {
    pub task_id: String,
    pub task_name: String,
    pub level: TaskLevel,
    pub description: String,
    pub risk_info: Option<String>,
    pub conflict_files: Vec<String>,
}

impl PendingDecision {
    pub fn new(task_id: String, task_name: String, level: TaskLevel) -> Self {
        Self {
            task_id,
            task_name,
            level,
            description: String::new(),
            risk_info: None,
            conflict_files: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_risk(mut self, risk: impl Into<String>) -> Self {
        self.risk_info = Some(risk.into());
        self
    }

    pub fn with_conflicts(mut self, files: Vec<String>) -> Self {
        self.conflict_files = files;
        self
    }
}

/// Decision panel for four-tier decision mechanism
pub struct DecisionPanel {
    /// Current countdown for Recommended level (seconds remaining)
    pub countdown: Option<u16>,
    /// Whether showing confirmation modal for Confirmed level
    pub show_confirmation: bool,
    /// Current task needing decision
    pub pending_task: Option<PendingDecision>,
    /// Modification dialog state
    pub show_modify_dialog: bool,
    /// Arbitration workspace state
    pub show_arbitration_workspace: bool,
    /// Temporary storage for task modifications
    pub temp_changes: TaskChanges,
    /// Last update timestamp for countdown
    last_update: Option<std::time::Instant>,
}

impl DecisionPanel {
    pub fn new() -> Self {
        Self {
            countdown: None,
            show_confirmation: false,
            pending_task: None,
            show_modify_dialog: false,
            show_arbitration_workspace: false,
            temp_changes: TaskChanges::default(),
            last_update: None,
        }
    }

    /// Set a new pending decision
    pub fn set_pending_decision(&mut self, decision: PendingDecision) {
        match decision.level {
            TaskLevel::Mechanical { .. } => {
                // Mechanical: no UI, auto-proceed
                self.pending_task = Some(decision);
            }
            TaskLevel::Recommended { timeout_secs, .. } => {
                // Recommended: start countdown
                self.countdown = Some(timeout_secs);
                self.pending_task = Some(decision);
                self.last_update = Some(std::time::Instant::now());
            }
            TaskLevel::Confirmed => {
                // Confirmed: show modal
                self.show_confirmation = true;
                self.pending_task = Some(decision);
            }
            TaskLevel::Arbitrated { .. } => {
                // Arbitrated: freeze and show workspace
                self.show_arbitration_workspace = true;
                self.pending_task = Some(decision);
            }
        }
    }

    /// Clear current decision state
    pub fn clear(&mut self) {
        self.countdown = None;
        self.show_confirmation = false;
        self.show_modify_dialog = false;
        self.show_arbitration_workspace = false;
        self.pending_task = None;
        self.temp_changes = TaskChanges::default();
        self.last_update = None;
    }

    /// Check if there's a pending decision
    pub fn has_pending_decision(&self) -> bool {
        self.pending_task.is_some()
    }

    /// Render decision UI based on current state
    /// Returns Some(action) when user makes a decision
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<DecisionAction> {
        let mut action = None;

        // Update countdown if needed
        self.update_countdown();

        // Check for auto-proceed on countdown expiration
        if let Some(countdown) = self.countdown {
            if countdown == 0 && self.pending_task.is_some() {
                // Auto-proceed when countdown reaches 0
                action = Some(DecisionAction::AutoProceed);
                self.clear();
                return action;
            }
        }

        // Render based on decision level
        if let Some(ref pending) = self.pending_task {
            match pending.level {
                TaskLevel::Mechanical { .. } => {
                    // Mechanical: silent background execution
                    action = Some(DecisionAction::AutoProceed);
                    self.clear();
                }
                TaskLevel::Recommended { .. } => {
                    // Render notification bar
                    action = self.render_notification_bar(ctx);
                }
                TaskLevel::Confirmed => {
                    // Render modal dialog
                    action = self.render_confirmation_modal(ctx);
                }
                TaskLevel::Arbitrated { .. } => {
                    // Render arbitration workspace
                    action = self.render_arbitration_workspace(ctx);
                }
            }
        }

        // Render modification dialog if open
        if self.show_modify_dialog {
            if let Some(modify_action) = self.render_modify_dialog(ctx) {
                action = Some(modify_action);
            }
        }

        action
    }

    /// Update countdown timer
    fn update_countdown(&mut self) {
        if let (Some(countdown), Some(last_update)) = (self.countdown.as_mut(), self.last_update) {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_update).as_secs() as u16;

            if elapsed > 0 {
                *countdown = countdown.saturating_sub(elapsed);
                self.last_update = Some(now);
            }
        }
    }

    /// Render Recommended level notification bar
    fn render_notification_bar(&mut self, ctx: &egui::Context) -> Option<DecisionAction> {
        let mut action = None;

        let task_name = self.pending_task.as_ref()?.task_name.clone();
        let countdown = self.countdown.unwrap_or(0);

        // Top panel notification bar
        egui::TopBottomPanel::top("decision_notification")
            .exact_height(48.0)
            .frame(Frame::default().fill(STATUS_WARNING.gamma_multiply(0.3)))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(16.0);

                    // Lightning icon
                    ui.label(
                        RichText::new("⚡")
                            .size(18.0)
                            .color(STATUS_WARNING)
                    );

                    ui.add_space(8.0);

                    // Message
                    let msg = format!(
                        "即将执行: {} ({}s后自动执行)",
                        task_name, countdown
                    );
                    ui.label(
                        RichText::new(&msg)
                            .size(14.0)
                            .color(TEXT_PRIMARY)
                            .strong()
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);

                        // Execute now button
                        let execute_btn = egui::Button::new(
                            RichText::new("立即执行")
                                .color(Color32::WHITE)
                                .size(12.0)
                        )
                        .fill(STATUS_ONLINE)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(80.0, 28.0));

                        if ui.add(execute_btn).clicked() {
                            action = Some(DecisionAction::Proceed);
                        }

                        ui.add_space(8.0);

                        // Skip button
                        let skip_btn = egui::Button::new(
                            RichText::new("跳过")
                                .color(TEXT_PRIMARY)
                                .size(12.0)
                        )
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(60.0, 28.0));

                        if ui.add(skip_btn).clicked() {
                            action = Some(DecisionAction::Skip);
                        }

                        ui.add_space(8.0);

                        // Modify button
                        let modify_btn = egui::Button::new(
                            RichText::new("修改")
                                .color(TEXT_PRIMARY)
                                .size(12.0)
                        )
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(60.0, 28.0));

                        if ui.add(modify_btn).clicked() {
                            self.show_modify_dialog = true;
                        }
                    });
                });
            });

        if action.is_some() {
            self.clear();
        }

        action
    }

    /// Render Confirmed level modal dialog
    fn render_confirmation_modal(&mut self, ctx: &egui::Context) -> Option<DecisionAction> {
        let mut action = None;

        if !self.show_confirmation {
            return None;
        }

        let pending = self.pending_task.as_ref()?;
        let task_name = pending.task_name.clone();
        let description = pending.description.clone();
        let risk_info = pending.risk_info.clone();

        Window::new("⚠️ 需要确认")
            .collapsible(false)
            .resizable(false)
            .fixed_size([400.0, 200.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::default()
                    .fill(PANEL_BG)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(20))
                    .shadow(egui::epaint::Shadow {
                        color: Color32::BLACK.gamma_multiply(0.5),
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                    })
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    // Warning icon
                    ui.label(
                        RichText::new("⚠️")
                            .size(32.0)
                            .color(STATUS_WARNING)
                    );

                    ui.add_space(16.0);

                    // Task name
                    ui.label(
                        RichText::new(&task_name)
                            .size(16.0)
                            .color(TEXT_PRIMARY)
                            .strong()
                    );

                    ui.add_space(8.0);

                    // Description
                    if !description.is_empty() {
                        ui.label(
                            RichText::new(&description)
                                .size(13.0)
                                .color(TEXT_SECONDARY)
                        );
                    }

                    // Risk warning
                    if let Some(ref risk) = risk_info {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!("风险: {}", risk))
                                .size(12.0)
                                .color(STATUS_ERROR)
                        );
                    }

                    ui.add_space(24.0);

                    // Action buttons
                    ui.horizontal(|ui| {
                        // Cancel button
                        let cancel_btn = egui::Button::new(
                            RichText::new("取消")
                                .color(TEXT_PRIMARY)
                                .size(13.0)
                        )
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(100.0, 36.0));

                        if ui.add(cancel_btn).clicked() {
                            action = Some(DecisionAction::Abort);
                        }

                        ui.add_space(16.0);

                        // Confirm button
                        let confirm_btn = egui::Button::new(
                            RichText::new("确认执行")
                                .color(Color32::WHITE)
                                .size(13.0)
                        )
                        .fill(STATUS_WARNING)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(100.0, 36.0));

                        if ui.add(confirm_btn).clicked() {
                            action = Some(DecisionAction::Proceed);
                        }
                    });
                });
            });

        if action.is_some() {
            self.clear();
        }

        action
    }

    /// Render Arbitrated level full-screen workspace
    fn render_arbitration_workspace(&mut self, ctx: &egui::Context) -> Option<DecisionAction> {
        let mut action = None;

        if !self.show_arbitration_workspace {
            return None;
        }

        // Clone data to avoid borrow issues
        let task_name = self.pending_task.as_ref()?.task_name.clone();
        let description = self.pending_task.as_ref()?.description.clone();
        let conflict_files = self.pending_task.as_ref()?.conflict_files.clone();

        // Full screen overlay
        egui::CentralPanel::default()
            .frame(Frame::default().fill(MAIN_BG.gamma_multiply(0.95)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Header
                    ui.add_space(40.0);

                    ui.label(
                        RichText::new("🔒 DAG 已暂停 - 等待仲裁")
                            .size(24.0)
                            .color(STATUS_ERROR)
                            .strong()
                    );

                    ui.add_space(32.0);

                    // Task info card
                    Frame::default()
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(24))
                        .show(ui, |ui| {
                            ui.set_min_width(500.0);

                            ui.label(
                                RichText::new(format!("任务: {}", &task_name))
                                    .size(16.0)
                                    .color(TEXT_PRIMARY)
                                    .strong()
                            );

                            ui.add_space(12.0);

                            if !description.is_empty() {
                                ui.label(
                                    RichText::new(&description)
                                        .size(14.0)
                                        .color(TEXT_SECONDARY)
                                );
                            }

                            // Conflict files
                            if !conflict_files.is_empty() {
                                ui.add_space(16.0);

                                ui.label(
                                    RichText::new("冲突文件:")
                                        .size(13.0)
                                        .color(TEXT_PRIMARY)
                                );

                                for file in &conflict_files {
                                    ui.horizontal(|ui| {
                                        ui.add_space(16.0);
                                        ui.label(
                                            RichText::new(format!("• {}", file))
                                                .size(12.0)
                                                .color(TERMINAL_YELLOW)
                                                .monospace()
                                        );
                                    });
                                }
                            }
                        });

                    ui.add_space(32.0);

                    // Action buttons
                    ui.horizontal(|ui| {
                        // View details button
                        let details_btn = egui::Button::new(
                            RichText::new("查看详情")
                                .color(TEXT_PRIMARY)
                                .size(13.0)
                        )
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(100.0, 40.0));

                        if ui.add(details_btn).clicked() {
                            action = Some(DecisionAction::ViewDetails);
                        }

                        ui.add_space(12.0);

                        // Request assistance button
                        let assist_btn = egui::Button::new(
                            RichText::new("请求协助")
                                .color(TEXT_PRIMARY)
                                .size(13.0)
                        )
                        .fill(PANEL_BG)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(100.0, 40.0));

                        if ui.add(assist_btn).clicked() {
                            action = Some(DecisionAction::RequestAssistance);
                        }

                        ui.add_space(12.0);

                        // Mark resolved button
                        let resolved_btn = egui::Button::new(
                            RichText::new("标记已解决")
                                .color(Color32::WHITE)
                                .size(13.0)
                        )
                        .fill(STATUS_ONLINE)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(120.0, 40.0));

                        if ui.add(resolved_btn).clicked() {
                            action = Some(DecisionAction::MarkResolved);
                        }

                        ui.add_space(12.0);

                        // Rollback button
                        let rollback_btn = egui::Button::new(
                            RichText::new("回滚到上一个任务")
                                .color(Color32::WHITE)
                                .size(13.0)
                        )
                        .fill(STATUS_ERROR)
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(140.0, 40.0));

                        if ui.add(rollback_btn).clicked() {
                            action = Some(DecisionAction::Rollback);
                        }
                    });
                });
            });

        if action.is_some() {
            self.clear();
        }

        action
    }

    /// Render modification dialog
    fn render_modify_dialog(&mut self, ctx: &egui::Context) -> Option<DecisionAction> {
        let mut action = None;

        Window::new("修改任务")
            .collapsible(false)
            .resizable(false)
            .fixed_size([350.0, 200.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::default()
                    .fill(PANEL_BG)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::same(20))
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("修改任务参数")
                            .size(14.0)
                            .color(TEXT_PRIMARY)
                            .strong()
                    );

                    ui.add_space(16.0);

                    // New command input (placeholder)
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("命令:")
                                .size(12.0)
                                .color(TEXT_SECONDARY)
                        );
                        let mut cmd = self.temp_changes.new_command.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut cmd).changed() {
                            self.temp_changes.new_command = Some(cmd);
                        }
                    });

                    ui.add_space(16.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        // Cancel
                        if ui.button("取消").clicked() {
                            self.show_modify_dialog = false;
                            self.temp_changes = TaskChanges::default();
                        }

                        ui.add_space(8.0);

                        // Save
                        let save_btn = egui::Button::new(
                            RichText::new("保存修改")
                                .color(Color32::WHITE)
                        )
                        .fill(VERIFIED_LOCAL_BG);

                        if ui.add(save_btn).clicked() {
                            action = Some(DecisionAction::Modify {
                                changes: self.temp_changes.clone(),
                            });
                            self.show_modify_dialog = false;
                            self.clear();
                        }
                    });
                });
            });

        action
    }
}

impl Default for DecisionPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_decision_builder() {
        let decision = PendingDecision::new(
            "task-1".to_string(),
            "测试任务".to_string(),
            TaskLevel::Confirmed,
        )
        .with_description("这是一个测试任务")
        .with_risk("高风险操作");

        assert_eq!(decision.task_id, "task-1");
        assert_eq!(decision.task_name, "测试任务");
        assert_eq!(decision.description, "这是一个测试任务");
        assert_eq!(decision.risk_info, Some("高风险操作".to_string()));
    }

    #[test]
    fn test_decision_panel_state() {
        let mut panel = DecisionPanel::new();
        assert!(!panel.has_pending_decision());

        let decision = PendingDecision::new(
            "task-1".to_string(),
            "测试".to_string(),
            TaskLevel::Confirmed,
        );

        panel.set_pending_decision(decision);
        assert!(panel.has_pending_decision());
        assert!(panel.show_confirmation);

        panel.clear();
        assert!(!panel.has_pending_decision());
        assert!(!panel.show_confirmation);
    }

    #[test]
    fn test_task_changes_default() {
        let changes = TaskChanges::default();
        assert!(changes.new_command.is_none());
        assert!(changes.new_timeout.is_none());
        assert!(changes.new_params.is_empty());
    }
}
