//! Skill Event Loop Management
//!
//! 管理活跃 Skill 实例的事件循环和生命周期。

use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use super::super::{Event, Skill, SkillContext, SkillConfig, types::SkillInfo};
use crate::storage::db::SkillDb;

/// Skill 事件循环命令
#[derive(Debug)]
pub enum SkillEventCommand {
    /// 处理事件
    HandleEvent(Event),
    /// 停止事件循环
    Stop,
}

/// 活跃的 Skill 实例
pub struct ActiveSkill {
    /// Skill 元数据
    pub _info: SkillInfo,
    /// Skill 数据库
    pub db: SkillDb,
    /// 配置
    pub config: SkillConfig,
    /// Skill 实例（用于事件处理）
    pub skill: Arc<dyn Skill>,
    /// 事件发送通道
    pub event_sender: Option<mpsc::UnboundedSender<SkillEventCommand>>,
    /// 停止信号
    pub shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ActiveSkill {
    /// 检查 Skill 是否处于活跃状态
    pub fn is_active(&self) -> bool {
        self.event_sender.is_some()
    }

    /// 安全关闭 Skill 事件循环
    pub async fn shutdown(&mut self) {
        // 发送停止命令到事件循环
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SkillEventCommand::Stop);
        }

        // 发送 shutdown 信号（备用）
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        // 清除通道引用
        self.event_sender = None;

        // 小延迟确保事件循环有时间处理停止命令
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}

impl Drop for ActiveSkill {
    fn drop(&mut self) {
        // 尝试同步关闭（仅清理资源，不阻塞）
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SkillEventCommand::Stop);
        }
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}
