//! # CIS Four-Tier Decision Mechanism
//!
//! 四级决策机制实现：
//! - Mechanical: 自动执行
//! - Recommended: 倒计时后自动执行默认动作
//! - Confirmed: 等待用户确认
//! - Arbitrated: 多方仲裁投票

pub mod arbitration;
pub mod config;
pub mod countdown;
pub mod confirmation;

pub use arbitration::{ArbitrationManager, ArbitrationVote, Vote, VoteStatus, VoteStats, VoteResult};
pub use config::DecisionConfig;
pub use confirmation::{ConfirmationManager, ConfirmationRequest, ConfirmationResponse};
pub use countdown::CountdownTimer;

use crate::types::{Action, Task, TaskLevel};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// 决策引擎 - 处理四级决策逻辑
pub struct DecisionEngine {
    config: DecisionConfig,
    arbitration_manager: Arc<Mutex<ArbitrationManager>>,
    confirmation_manager: Arc<Mutex<ConfirmationManager>>,
}

impl DecisionEngine {
    /// 创建新的决策引擎
    pub fn new() -> Self {
        Self::with_config(DecisionConfig::load())
    }

    /// 使用指定配置创建决策引擎
    pub fn with_config(config: DecisionConfig) -> Self {
        Self {
            config: config.clone(),
            arbitration_manager: Arc::new(Mutex::new(ArbitrationManager::new(
                config.timeout_arbitrated,
            ))),
            confirmation_manager: Arc::new(Mutex::new(ConfirmationManager::new(
                config.timeout_confirmed,
            ))),
        }
    }

    /// 处理任务决策
    ///
    /// 根据 TaskLevel 执行相应的决策逻辑：
    /// - Mechanical: 直接返回 Allow
    /// - Recommended: 启动倒计时
    /// - Confirmed: 等待用户确认
    /// - Arbitrated: 启动仲裁流程
    pub async fn process_decision(&self, task: &Task, run_id: &str) -> DecisionResult {
        match &task.level {
            TaskLevel::Mechanical { .. } => DecisionResult::Allow,
            TaskLevel::Recommended {
                default_action,
                timeout_secs,
            } => {
                self.handle_recommended(task, run_id, *timeout_secs, *default_action)
                    .await
            }
            TaskLevel::Confirmed => self.handle_confirmed(task, run_id).await,
            TaskLevel::Arbitrated { stakeholders } => {
                self.handle_arbitrated(task, run_id, stakeholders.clone()).await
            }
        }
    }

    /// 处理 Recommended 级别 - 倒计时
    async fn handle_recommended(
        &self,
        task: &Task,
        _run_id: &str,
        timeout_secs: u16,
        default_action: Action,
    ) -> DecisionResult {
        info!(
            "Task '{}' (Recommended): Starting countdown for {} seconds",
            task.id, timeout_secs
        );

        let timer = CountdownTimer::new(timeout_secs, default_action);
        
        // 显示倒计时
        timer.run_with_display(&task.id).await;

        match default_action {
            Action::Execute => {
                info!("Task '{}': Countdown complete, executing", task.id);
                DecisionResult::Allow
            }
            Action::Skip => {
                warn!("Task '{}': Countdown complete, skipping", task.id);
                DecisionResult::Skip
            }
            Action::Abort => {
                warn!("Task '{}': Countdown complete, aborting", task.id);
                DecisionResult::Abort
            }
        }
    }

    /// 处理 Confirmed 级别 - 用户确认
    async fn handle_confirmed(&self, task: &Task, run_id: &str) -> DecisionResult {
        info!("Task '{}' (Confirmed): Waiting for user confirmation", task.id);

        let manager = self.confirmation_manager.clone();
        let mut mgr = manager.lock().await;

        let request = ConfirmationRequest::new(&task.id, run_id, self.config.timeout_confirmed);
        let request_id = request.id.clone();

        mgr.add_request(request);
        drop(mgr); // 释放锁

        // 等待确认结果
        let response = ConfirmationManager::wait_for_response(manager, &request_id).await;

        match response {
            Some(ConfirmationResponse::Confirmed) => {
                info!("Task '{}': User confirmed", task.id);
                DecisionResult::Allow
            }
            Some(ConfirmationResponse::Rejected) => {
                warn!("Task '{}': User rejected", task.id);
                DecisionResult::Abort
            }
            None => {
                warn!("Task '{}': Confirmation timeout, aborting", task.id);
                DecisionResult::Abort
            }
        }
    }

    /// 处理 Arbitrated 级别 - 仲裁投票
    async fn handle_arbitrated(
        &self,
        task: &Task,
        run_id: &str,
        stakeholders: Vec<String>,
    ) -> DecisionResult {
        info!(
            "Task '{}' (Arbitrated): Starting arbitration with {:?}",
            task.id, stakeholders
        );

        let manager = self.arbitration_manager.clone();
        let mut mgr = manager.lock().await;

        let vote = ArbitrationVote::new(&task.id, run_id, stakeholders, self.config.timeout_arbitrated);
        let vote_id = vote.id.clone();

        mgr.start_vote(vote);
        drop(mgr); // 释放锁

        // 等待仲裁结果
        let result = ArbitrationManager::wait_for_result(manager, &vote_id).await;

        match result {
            Some(VoteResult::Approved) => {
                info!("Task '{}': Arbitration approved", task.id);
                DecisionResult::Allow
            }
            Some(VoteResult::Rejected) => {
                warn!("Task '{}': Arbitration rejected", task.id);
                DecisionResult::Abort
            }
            Some(VoteResult::Timeout) | None => {
                warn!("Task '{}': Arbitration timeout, aborting", task.id);
                DecisionResult::Abort
            }
        }
    }

    /// 获取仲裁管理器（用于 CLI 交互）
    pub fn arbitration_manager(&self) -> Arc<Mutex<ArbitrationManager>> {
        self.arbitration_manager.clone()
    }

    /// 获取确认管理器（用于 CLI 交互）
    pub fn confirmation_manager(&self) -> Arc<Mutex<ConfirmationManager>> {
        self.confirmation_manager.clone()
    }

    /// 获取配置
    pub fn config(&self) -> &DecisionConfig {
        &self.config
    }
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 决策结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionResult {
    /// 允许执行
    Allow,
    /// 跳过任务
    Skip,
    /// 中止执行
    Abort,
    /// 需要等待（用于异步决策）
    Pending(String),
}

impl DecisionResult {
    /// 是否允许执行
    pub fn is_allowed(&self) -> bool {
        matches!(self, DecisionResult::Allow)
    }

    /// 是否中止
    pub fn is_abort(&self) -> bool {
        matches!(self, DecisionResult::Abort)
    }

    /// 是否跳过
    pub fn is_skip(&self) -> bool {
        matches!(self, DecisionResult::Skip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_result() {
        assert!(DecisionResult::Allow.is_allowed());
        assert!(!DecisionResult::Allow.is_abort());
        
        assert!(!DecisionResult::Abort.is_allowed());
        assert!(DecisionResult::Abort.is_abort());
        
        assert!(!DecisionResult::Skip.is_allowed());
        assert!(DecisionResult::Skip.is_skip());
    }
}
