//! GLM API 管理面板
//!
//! 提供 GUI 界面管理 GLM API 服务和待确认任务

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::theme::*;

/// GLM 面板状态
pub struct GlmPanel {
    /// 是否打开
    open: bool,
    /// 服务地址
    server_url: String,
    /// DID（与 CIS 其他节点间认证格式一致）
    /// 格式: did:cis:{node_id}:{pub_key_short}
    did: String,
    /// 待确认任务列表
    pending_dags: Vec<PendingDagInfo>,
    /// 选中的 DAG
    selected_dag: Option<String>,
    /// 状态消息
    status_message: Option<(String, bool)>, // (message, is_error)
    /// 刷新任务
    refresh_trigger: std::time::Instant,
}

/// 待确认 DAG 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDagInfo {
    pub dag_id: String,
    pub description: String,
    pub task_count: usize,
    pub created_at: String,
    pub expires_at: String,
    pub requested_by: String,
}

/// GLM 面板响应
#[derive(Debug)]
pub enum GlmPanelResponse {
    /// 确认 DAG
    ConfirmDag(String),
    /// 拒绝 DAG
    RejectDag(String),
    /// 刷新列表
    Refresh,
    /// 关闭面板
    Close,
}

impl GlmPanel {
    pub fn new() -> Self {
        Self {
            open: false,
            server_url: "http://127.0.0.1:6767".to_string(),
            // 默认使用示例 DID，与 CIS 其他节点间认证格式一致
            did: "did:cis:glm-cloud:abc123".to_string(),
            pending_dags: vec![],
            selected_dag: None,
            status_message: None,
            refresh_trigger: std::time::Instant::now(),
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.refresh_trigger = std::time::Instant::now();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.refresh_trigger = std::time::Instant::now();
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 设置待确认 DAG 列表
    pub fn set_pending_dags(&mut self, dags: Vec<PendingDagInfo>) {
        self.pending_dags = dags;
    }

    /// 设置状态消息
    pub fn set_status(&mut self, message: String, is_error: bool) {
        self.status_message = Some((message, is_error));
    }

    /// 渲染面板
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<GlmPanelResponse> {
        if !self.open {
            return None;
        }

        let mut response = None;

        egui::Window::new("🔮 GLM API 管理")
            .default_size([500.0, 400.0])
            .resizable(true)
            .collapsible(true)
            .frame(
                egui::Frame::default()
                    .fill(MAIN_BG)
                    .stroke(egui::Stroke::new(1.0, BORDER_COLOR))
                    .corner_radius(8.0)
                    .inner_margin(16.0)
            )
            .show(ctx, |ui| {
                // 服务配置区域
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("服务配置")
                            .strong()
                            .color(ACCENT_BLUE)
                            .size(14.0)
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("地址:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.server_url)
                                .desired_width(200.0)
                                .text_color(TERMINAL_FG)
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("DID:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.did)
                                .desired_width(350.0)
                                .text_color(TERMINAL_FG)
                        );
                    });
                    ui.label(
                        egui::RichText::new("格式: did:cis:{node_id}:{pub_key_short}")
                            .color(MUTED_TEXT)
                            .size(11.0)
                    );
                });

                ui.add_space(16.0);

                // 状态消息
                if let Some((msg, is_error)) = &self.status_message {
                    let color = if *is_error { ACCENT_RED } else { ACCENT_GREEN };
                    ui.label(egui::RichText::new(msg).color(color).size(12.0));
                    ui.add_space(8.0);
                }

                // 待确认 DAG 列表
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("待确认任务")
                            .strong()
                            .color(ACCENT_BLUE)
                            .size(14.0)
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🔄 刷新").clicked() {
                            response = Some(GlmPanelResponse::Refresh);
                        }
                    });
                });

                ui.add_space(8.0);

                // DAG 列表
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        if self.pending_dags.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    egui::RichText::new("暂无待确认任务")
                                        .color(MUTED_TEXT)
                                        .size(14.0)
                                );
                            });
                        } else {
                            for dag in &self.pending_dags {
                                let is_selected = self.selected_dag.as_ref() == Some(&dag.dag_id);
                                let bg_color = if is_selected {
                                    ACCENT_BLUE.gamma_multiply(0.2)
                                } else {
                                    MAIN_BG
                                };

                                egui::Frame::default()
                                    .fill(bg_color)
                                    .stroke(egui::Stroke::new(1.0, BORDER_COLOR))
                                    .corner_radius(4.0)
                                    .inner_margin(12.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // DAG ID 和描述
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&dag.dag_id)
                                                        .strong()
                                                        .color(ACCENT_BLUE)
                                                        .size(13.0)
                                                );
                                                ui.label(
                                                    egui::RichText::new(&dag.description)
                                                        .color(TERMINAL_FG)
                                                        .size(12.0)
                                                );
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{} 个任务 · 过期: {}",
                                                        dag.task_count,
                                                        dag.expires_at
                                                    ))
                                                        .color(MUTED_TEXT)
                                                        .size(11.0)
                                                );
                                            });

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // 拒绝按钮
                                                    if ui.button(
                                                        egui::RichText::new("❌")
                                                            .color(ACCENT_RED)
                                                    ).clicked() {
                                                        response = Some(GlmPanelResponse::RejectDag(
                                                            dag.dag_id.clone()
                                                        ));
                                                    }

                                                    // 确认按钮
                                                    if ui.button(
                                                        egui::RichText::new("✅")
                                                            .color(ACCENT_GREEN)
                                                    ).clicked() {
                                                        response = Some(GlmPanelResponse::ConfirmDag(
                                                            dag.dag_id.clone()
                                                        ));
                                                    }
                                                }
                                            );
                                        });

                                        // 点击选择
                                        if ui.interact(
                                            ui.min_rect(),
                                            ui.id().with(&dag.dag_id),
                                            egui::Sense::click()
                                        ).clicked() {
                                            self.selected_dag = Some(dag.dag_id.clone());
                                        }
                                    });

                                ui.add_space(8.0);
                            }
                        }
                    });

                ui.add_space(16.0);

                // 底部按钮
                ui.horizontal(|ui| {
                    if ui.button("📋 查看详细").clicked() {
                        if let Some(dag_id) = &self.selected_dag {
                            // TODO: 打开详细视图
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("关闭").clicked() {
                            response = Some(GlmPanelResponse::Close);
                        }
                    });
                });
            });

        response
    }

    /// 获取服务器 URL
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// 获取 DID
    pub fn did(&self) -> &str {
        &self.did
    }

    /// 模拟加载演示数据
    pub fn load_demo_data(&mut self) {
        self.pending_dags = vec![
            PendingDagInfo {
                dag_id: "backup_daily".to_string(),
                description: "每日凌晨3点备份文档到NAS".to_string(),
                task_count: 2,
                created_at: "2026-02-04T10:00:00Z".to_string(),
                expires_at: "2026-02-04T10:05:00Z".to_string(),
                requested_by: "glm_cloud_user".to_string(),
            },
            PendingDagInfo {
                dag_id: "cleanup_logs".to_string(),
                description: "清理30天前的日志文件".to_string(),
                task_count: 1,
                created_at: "2026-02-04T09:30:00Z".to_string(),
                expires_at: "2026-02-04T09:35:00Z".to_string(),
                requested_by: "glm_cloud_user".to_string(),
            },
        ];
    }
}

impl Default for GlmPanel {
    fn default() -> Self {
        Self::new()
    }
}
