//! # Arbitration Manager for Arbitrated Level
//!
//! 实现 Arbitrated 级别的多方仲裁投票功能

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;

/// 仲裁投票
#[derive(Debug, Clone)]
pub struct ArbitrationVote {
    /// 投票 ID
    pub id: String,
    /// 任务 ID
    pub task_id: String,
    /// DAG 运行 ID
    pub run_id: String,
    /// 利益相关者列表
    pub stakeholders: Vec<String>,
    /// 投票记录
    pub votes: HashMap<String, Vote>,
    /// 创建时间
    pub created_at: Instant,
    /// 超时时间（秒）
    pub timeout_secs: u16,
    /// 当前状态
    pub status: VoteStatus,
    /// 投票阈值（同意比例，0.0-1.0）
    pub threshold: f32,
}

impl ArbitrationVote {
    /// 创建新的仲裁投票
    pub fn new(
        task_id: &str,
        run_id: &str,
        stakeholders: Vec<String>,
        timeout_secs: u16,
    ) -> Self {
        Self {
            id: format!("vote-{}", Uuid::new_v4().to_string().split('-').next().unwrap()),
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            stakeholders: stakeholders.clone(),
            votes: HashMap::new(),
            created_at: Instant::now(),
            timeout_secs,
            status: VoteStatus::Pending,
            threshold: 0.5, // 默认简单多数
        }
    }

    /// 设置投票阈值
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// 检查是否已超时
    pub fn is_expired(&self) -> bool {
        let elapsed = self.created_at.elapsed().as_secs() as u16;
        elapsed >= self.timeout_secs
    }

    /// 获取剩余时间（秒）
    pub fn remaining_secs(&self) -> u16 {
        let elapsed = self.created_at.elapsed().as_secs() as u16;
        if elapsed >= self.timeout_secs {
            0
        } else {
            self.timeout_secs - elapsed
        }
    }

    /// 记录投票
    pub fn cast_vote(&mut self, stakeholder: &str, vote: Vote) -> bool {
        // 检查是否是利益相关者
        if !self.stakeholders.contains(&stakeholder.to_string()) {
            warn!("Stakeholder '{}' not in list for vote '{}'", stakeholder, self.id);
            return false;
        }

        // 检查投票是否仍在进行
        if self.status != VoteStatus::Pending {
            warn!("Vote '{}' is not pending", self.id);
            return false;
        }

        // 记录投票
        self.votes.insert(stakeholder.to_string(), vote);
        info!(
            "Stakeholder '{}' voted {:?} for vote '{}'",
            stakeholder, vote, self.id
        );

        // 检查是否达到决策条件
        self.check_result();
        
        true
    }

    /// 检查投票结果
    fn check_result(&mut self) {
        if self.stakeholders.is_empty() {
            return;
        }

        let total = self.stakeholders.len();
        let approve_count = self.votes.values().filter(|v| **v == Vote::Approve).count();
        let reject_count = self.votes.values().filter(|v| **v == Vote::Reject).count();

        let approve_ratio = approve_count as f32 / total as f32;
        let reject_ratio = reject_count as f32 / total as f32;

        // 检查是否达到通过阈值
        if approve_ratio >= self.threshold {
            self.status = VoteStatus::Approved;
            info!("Vote '{}' approved with {:.0}% support", self.id, approve_ratio * 100.0);
        }
        // 检查是否达到拒绝阈值（超过一半人反对）
        else if reject_ratio > 0.5 {
            self.status = VoteStatus::Rejected;
            info!("Vote '{}' rejected with {:.0}% opposition", self.id, reject_ratio * 100.0);
        }
        // 所有人都投票了但还没结果
        else if self.votes.len() >= total {
            // 简单多数决定
            if approve_count > reject_count {
                self.status = VoteStatus::Approved;
            } else {
                self.status = VoteStatus::Rejected;
            }
        }
    }

    /// 获取当前投票统计
    pub fn get_stats(&self) -> VoteStats {
        let approve = self.votes.values().filter(|v| **v == Vote::Approve).count();
        let reject = self.votes.values().filter(|v| **v == Vote::Reject).count();
        let abstain = self.votes.values().filter(|v| **v == Vote::Abstain).count();
        let pending = self.stakeholders.len().saturating_sub(approve + reject + abstain);

        VoteStats {
            total: self.stakeholders.len(),
            approve,
            reject,
            abstain,
            pending,
        }
    }
}

/// 投票选项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    /// 同意
    Approve,
    /// 拒绝
    Reject,
    /// 弃权
    Abstain,
}

/// 投票状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteStatus {
    /// 等待投票
    Pending,
    /// 已通过
    Approved,
    /// 已拒绝
    Rejected,
    /// 已超时
    Expired,
}

/// 投票结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteResult {
    /// 通过
    Approved,
    /// 拒绝
    Rejected,
    /// 超时
    Timeout,
}

/// 投票统计
#[derive(Debug, Clone)]
pub struct VoteStats {
    /// 总人数
    pub total: usize,
    /// 同意人数
    pub approve: usize,
    /// 拒绝人数
    pub reject: usize,
    /// 弃权人数
    pub abstain: usize,
    /// 待投票人数
    pub pending: usize,
}

impl VoteStats {
    /// 获取同意比例
    pub fn approve_ratio(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.approve as f32 / self.total as f32
        }
    }

    /// 获取拒绝比例
    pub fn reject_ratio(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.reject as f32 / self.total as f32
        }
    }

    /// 是否已决出结果
    pub fn is_decided(&self) -> bool {
        self.approve + self.reject + self.abstain >= self.total
    }
}

/// 仲裁管理器
pub struct ArbitrationManager {
    /// 进行中的投票
    votes: RwLock<HashMap<String, ArbitrationVote>>,
    /// 默认超时时间
    default_timeout: u16,
    /// 默认阈值
    default_threshold: f32,
}

impl ArbitrationManager {
    /// 创建新的仲裁管理器
    pub fn new(default_timeout: u16) -> Self {
        Self {
            votes: RwLock::new(HashMap::new()),
            default_timeout,
            default_threshold: 0.5,
        }
    }

    /// 设置默认阈值
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.default_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// 开始新的投票
    pub async fn start_vote(&self, mut vote: ArbitrationVote) {
        vote.threshold = self.default_threshold;
        
        let mut votes = self.votes.write().await;
        info!(
            "Starting arbitration vote for task '{}' with {} stakeholders",
            vote.task_id,
            vote.stakeholders.len()
        );
        votes.insert(vote.id.clone(), vote);
    }

    /// 记录投票
    pub async fn cast_vote(&self, vote_id: &str, stakeholder: &str, vote: Vote) -> bool {
        let mut votes = self.votes.write().await;
        
        if let Some(arbitration) = votes.get_mut(vote_id) {
            arbitration.cast_vote(stakeholder, vote)
        } else {
            warn!("Vote '{}' not found", vote_id);
            false
        }
    }

    /// 获取投票状态
    pub async fn get_status(&self, vote_id: &str) -> Option<VoteStatus> {
        let votes = self.votes.read().await;
        votes.get(vote_id).map(|v| v.status.clone())
    }

    /// 获取投票统计
    pub async fn get_stats(&self, vote_id: &str) -> Option<VoteStats> {
        let votes = self.votes.read().await;
        votes.get(vote_id).map(|v| v.get_stats())
    }

    /// 等待投票结果
    ///
    /// 异步等待投票完成或超时
    pub async fn wait_for_result(
        manager: Arc<Mutex<Self>>,
        vote_id: &str,
    ) -> Option<VoteResult> {
        let timeout_secs = {
            let mgr = manager.lock().await;
            
            let timeout = if let Ok(votes) = mgr.votes.try_read() {
                votes.get(vote_id).map(|v| v.timeout_secs)?
            } else {
                return None;
            };
            timeout
        };

        // 轮询等待结果
        let poll_interval = Duration::from_millis(500);
        let timeout = Duration::from_secs(timeout_secs as u64);
        let start = Instant::now();

        loop {
            // 检查投票状态
            {
                let mgr = manager.lock().await;
                let votes = mgr.votes.read().await;
                
                if let Some(vote) = votes.get(vote_id) {
                    match vote.status {
                        VoteStatus::Approved => return Some(VoteResult::Approved),
                        VoteStatus::Rejected => return Some(VoteResult::Rejected),
                        VoteStatus::Expired => return Some(VoteResult::Timeout),
                        VoteStatus::Pending => {
                            // 检查是否超时
                            if vote.is_expired() {
                                drop(votes);
                                let mut votes = mgr.votes.write().await;
                                if let Some(v) = votes.get_mut(vote_id) {
                                    v.status = VoteStatus::Expired;
                                }
                                return Some(VoteResult::Timeout);
                            }
                        }
                    }
                } else {
                    return None;
                }
            }

            // 检查总超时
            if start.elapsed() >= timeout {
                let mgr = manager.lock().await;
                let mut votes = mgr.votes.write().await;
                if let Some(vote) = votes.get_mut(vote_id) {
                    vote.status = VoteStatus::Expired;
                }
                warn!("Vote '{}' timed out", vote_id);
                return Some(VoteResult::Timeout);
            }

            sleep(poll_interval).await;
        }
    }

    /// 获取进行中的投票
    pub async fn get_active_votes(&self) -> Vec<ArbitrationVote> {
        let votes = self.votes.read().await;
        votes
            .values()
            .filter(|v| v.status == VoteStatus::Pending)
            .cloned()
            .collect()
    }

    /// 按运行 ID 获取投票
    pub async fn get_votes_by_run(&self, run_id: &str) -> Vec<ArbitrationVote> {
        let votes = self.votes.read().await;
        votes
            .values()
            .filter(|v| v.run_id == run_id)
            .cloned()
            .collect()
    }

    /// 获取指定投票
    pub async fn get_vote(&self, vote_id: &str) -> Option<ArbitrationVote> {
        let votes = self.votes.read().await;
        votes.get(vote_id).cloned()
    }

    /// 取消投票
    pub async fn cancel_vote(&self, vote_id: &str) -> bool {
        let mut votes = self.votes.write().await;
        
        if let Some(vote) = votes.get_mut(vote_id) {
            if vote.status == VoteStatus::Pending {
                vote.status = VoteStatus::Expired;
                warn!("Vote '{}' cancelled", vote_id);
                return true;
            }
        }
        
        false
    }

    /// 清理已完成的投票
    pub async fn cleanup(&self) -> usize {
        let mut votes = self.votes.write().await;
        
        let to_remove: Vec<String> = votes
            .iter()
            .filter(|(_, v)| {
                v.status != VoteStatus::Pending || v.is_expired()
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            votes.remove(&id);
        }

        count
    }
}

impl Default for ArbitrationManager {
    fn default() -> Self {
        Self::new(3600) // 默认 1 小时
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_arbitration_vote() {
        let mut vote = ArbitrationVote::new(
            "task-arb-vote",
            "run-arb-vote",
            vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
            3600,
        )
        .with_threshold(0.75); // 设置更高阈值，避免过早结束投票

        assert_eq!(vote.status, VoteStatus::Pending);
        assert!(!vote.is_expired());

        // 投票
        assert!(vote.cast_vote("alice", Vote::Approve));
        assert!(vote.cast_vote("bob", Vote::Approve));
        
        // 重复投票应该覆盖（此时投票仍在进行中）
        assert!(vote.cast_vote("alice", Vote::Reject));

        // 非利益相关者不能投票
        assert!(!vote.cast_vote("dave", Vote::Approve));
    }

    #[tokio::test]
    async fn test_voting_threshold() {
        let mut vote = ArbitrationVote::new(
            "task-threshold",
            "run-threshold",
            vec!["alice".to_string(), "bob".to_string()],
            3600,
        )
        .with_threshold(0.5);

        // 需要 50% 同意，即 1 票
        assert!(vote.cast_vote("alice", Vote::Approve));
        // 状态应该变为 Approved
        assert_eq!(vote.status, VoteStatus::Approved);
    }

    #[tokio::test]
    async fn test_voting_rejection() {
        let mut vote = ArbitrationVote::new(
            "task-rejection",
            "run-rejection",
            vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
            3600,
        );

        // 两票反对应该导致拒绝
        assert!(vote.cast_vote("alice", Vote::Reject));
        assert!(vote.cast_vote("bob", Vote::Reject));
        
        assert_eq!(vote.status, VoteStatus::Rejected);
    }

    #[tokio::test]
    async fn test_vote_stats() {
        let mut vote = ArbitrationVote::new(
            "task-stats",
            "run-stats",
            vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
            3600,
        );

        vote.cast_vote("alice", Vote::Approve);
        vote.cast_vote("bob", Vote::Reject);
        vote.cast_vote("charlie", Vote::Abstain);

        let stats = vote.get_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.approve, 1);
        assert_eq!(stats.reject, 1);
        assert_eq!(stats.abstain, 1);
        assert_eq!(stats.pending, 0);
        assert!(stats.is_decided());
    }

    #[tokio::test]
    async fn test_arbitration_manager() {
        let manager = ArbitrationManager::new(3600).with_threshold(0.75); // 设置更高阈值
        
        let vote = ArbitrationVote::new(
            "task-manager",
            "run-manager",
            vec!["alice".to_string(), "bob".to_string()],
            3600,
        );
        let vote_id = vote.id.clone();
        
        manager.start_vote(vote).await;
        
        let active = manager.get_active_votes().await;
        assert_eq!(active.len(), 1);
        
        // 投票（需要两人都投票才能达到 75% 阈值）
        assert!(manager.cast_vote(&vote_id, "alice", Vote::Approve).await);
        assert!(manager.cast_vote(&vote_id, "bob", Vote::Approve).await);
        
        // 检查状态 - 2/2 = 1.0 >= 0.75，应该通过
        let status = manager.get_status(&vote_id).await;
        assert_eq!(status, Some(VoteStatus::Approved));
    }
}
