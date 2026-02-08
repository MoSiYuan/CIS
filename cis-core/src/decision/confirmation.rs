//! # Confirmation Manager for Confirmed Level
//!
//! 实现 Confirmed 级别的用户确认功能

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tracing::{info, warn};
use uuid::Uuid;

/// 确认请求
#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    /// 请求 ID
    pub id: String,
    /// 任务 ID
    pub task_id: String,
    /// DAG 运行 ID
    pub run_id: String,
    /// 请求时间
    pub requested_at: Instant,
    /// 超时时间（秒）
    pub timeout_secs: u16,
    /// 当前状态
    pub status: ConfirmationStatus,
    /// 请求来源（CLI, GUI, Matrix 等）
    pub source: String,
}

impl ConfirmationRequest {
    /// 创建新的确认请求
    pub fn new(task_id: &str, run_id: &str, timeout_secs: u16) -> Self {
        Self {
            id: format!("conf-{}", Uuid::new_v4().to_string().split('-').next().unwrap()),
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            requested_at: Instant::now(),
            timeout_secs,
            status: ConfirmationStatus::Pending,
            source: "system".to_string(),
        }
    }

    /// 设置来源
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// 检查是否已超时
    pub fn is_expired(&self) -> bool {
        let elapsed = self.requested_at.elapsed().as_secs() as u16;
        elapsed >= self.timeout_secs
    }

    /// 获取剩余时间（秒）
    pub fn remaining_secs(&self) -> u16 {
        let elapsed = self.requested_at.elapsed().as_secs() as u16;
        if elapsed >= self.timeout_secs {
            0
        } else {
            self.timeout_secs - elapsed
        }
    }
}

/// 确认状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationStatus {
    /// 等待确认
    Pending,
    /// 已确认
    Confirmed,
    /// 已拒绝
    Rejected,
    /// 已超时
    Expired,
}

/// 确认响应
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationResponse {
    /// 确认执行
    Confirmed,
    /// 拒绝执行
    Rejected,
}

/// 确认管理器
pub struct ConfirmationManager {
    /// 待处理的请求
    requests: RwLock<HashMap<String, ConfirmationRequest>>,
    /// 响应通道
    responses: Arc<Mutex<HashMap<String, ConfirmationResponse>>>,
    /// 默认超时时间
    default_timeout: u16,
}

impl ConfirmationManager {
    /// 创建新的确认管理器
    pub fn new(default_timeout: u16) -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            responses: Arc::new(Mutex::new(HashMap::new())),
            default_timeout,
        }
    }

    /// 添加确认请求
    pub async fn add_request(&self, request: ConfirmationRequest) {
        let mut requests = self.requests.write().await;
        info!(
            "Adding confirmation request for task '{}' (timeout: {}s)",
            request.task_id, request.timeout_secs
        );
        requests.insert(request.id.clone(), request);
    }

    /// 获取待处理的请求
    pub async fn get_pending(&self) -> Vec<ConfirmationRequest> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.status == ConfirmationStatus::Pending)
            .cloned()
            .collect()
    }

    /// 按运行 ID 获取待处理请求
    pub async fn get_pending_by_run(&self, run_id: &str) -> Vec<ConfirmationRequest> {
        let requests = self.requests.read().await;
        requests
            .values()
            .filter(|r| r.status == ConfirmationStatus::Pending && r.run_id == run_id)
            .cloned()
            .collect()
    }

    /// 确认请求
    pub async fn confirm(&self, request_id: &str) -> bool {
        let mut requests = self.requests.write().await;
        
        if let Some(request) = requests.get_mut(request_id) {
            if request.status == ConfirmationStatus::Pending {
                request.status = ConfirmationStatus::Confirmed;
                
                // 设置响应
                let mut responses = self.responses.lock().await;
                responses.insert(request_id.to_string(), ConfirmationResponse::Confirmed);
                
                info!("Confirmation request '{}' confirmed", request_id);
                return true;
            }
        }
        
        false
    }

    /// 拒绝请求
    pub async fn reject(&self, request_id: &str) -> bool {
        let mut requests = self.requests.write().await;
        
        if let Some(request) = requests.get_mut(request_id) {
            if request.status == ConfirmationStatus::Pending {
                request.status = ConfirmationStatus::Rejected;
                
                // 设置响应
                let mut responses = self.responses.lock().await;
                responses.insert(request_id.to_string(), ConfirmationResponse::Rejected);
                
                info!("Confirmation request '{}' rejected", request_id);
                return true;
            }
        }
        
        false
    }

    /// 获取请求状态
    pub async fn get_status(&self, request_id: &str) -> Option<ConfirmationStatus> {
        let requests = self.requests.read().await;
        requests.get(request_id).map(|r| r.status.clone())
    }

    /// 等待响应
    ///
    /// 异步等待用户响应，超时返回 None
    pub async fn wait_for_response(
        manager: Arc<Mutex<Self>>,
        request_id: &str,
    ) -> Option<ConfirmationResponse> {
        let timeout_secs = {
            let mgr = manager.lock().await;
            
            // 先检查是否已有响应
            if let Ok(responses) = mgr.responses.try_lock() {
                if let Some(response) = responses.get(request_id) {
                    return Some(response.clone());
                }
            }
            
            // 获取请求的超时时间
            let timeout = if let Ok(requests) = mgr.requests.try_read() {
                requests.get(request_id).map(|r| r.timeout_secs)?
            } else {
                return None;
            };
            timeout
        };

        // 轮询等待响应
        let poll_interval = Duration::from_millis(100);
        let timeout = Duration::from_secs(timeout_secs as u64);
        let start = Instant::now();

        loop {
            // 检查响应
            {
                let mgr = manager.lock().await;
                if let Ok(responses) = mgr.responses.try_lock() {
                    if let Some(response) = responses.get(request_id) {
                        return Some(response.clone());
                    }
                };
            }

            // 检查超时
            if start.elapsed() >= timeout {
                // 标记为超时
                let mgr = manager.lock().await;
                let mut requests = mgr.requests.write().await;
                if let Some(request) = requests.get_mut(request_id) {
                    request.status = ConfirmationStatus::Expired;
                }
                warn!("Confirmation request '{}' timed out", request_id);
                return None;
            }

            sleep(poll_interval).await;
        }
    }

    /// 清理已完成的请求
    pub async fn cleanup(&self) -> usize {
        let mut requests = self.requests.write().await;
        let mut responses = self.responses.lock().await;
        
        let to_remove: Vec<String> = requests
            .iter()
            .filter(|(_, r)| {
                r.status != ConfirmationStatus::Pending || r.is_expired()
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            requests.remove(&id);
            responses.remove(&id);
        }

        count
    }

    /// 获取所有请求（包括已完成的）
    pub async fn get_all_requests(&self) -> Vec<ConfirmationRequest> {
        let requests = self.requests.read().await;
        requests.values().cloned().collect()
    }

    /// 按 ID 获取请求
    pub async fn get_request(&self, request_id: &str) -> Option<ConfirmationRequest> {
        let requests = self.requests.read().await;
        requests.get(request_id).cloned()
    }

    /// 取消请求
    pub async fn cancel(&self, request_id: &str) -> bool {
        let mut requests = self.requests.write().await;
        
        if let Some(request) = requests.get_mut(request_id) {
            if request.status == ConfirmationStatus::Pending {
                request.status = ConfirmationStatus::Expired;
                warn!("Confirmation request '{}' cancelled", request_id);
                return true;
            }
        }
        
        false
    }
}

impl Default for ConfirmationManager {
    fn default() -> Self {
        Self::new(300) // 默认 5 分钟
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_confirmation_request() {
        let request = ConfirmationRequest::new("task-1", "run-1", 60);
        assert_eq!(request.task_id, "task-1");
        assert_eq!(request.run_id, "run-1");
        assert_eq!(request.timeout_secs, 60);
        assert_eq!(request.status, ConfirmationStatus::Pending);
        assert!(!request.is_expired());
        assert!(request.remaining_secs() > 0);
    }

    #[tokio::test]
    async fn test_confirmation_manager() {
        let manager = ConfirmationManager::new(60);
        
        let request = ConfirmationRequest::new("task-1", "run-1", 60);
        let request_id = request.id.clone();
        
        manager.add_request(request).await;
        
        let pending = manager.get_pending().await;
        assert_eq!(pending.len(), 1);
        
        // 确认请求
        let confirmed = manager.confirm(&request_id).await;
        assert!(confirmed);
        
        let status = manager.get_status(&request_id).await;
        assert_eq!(status, Some(ConfirmationStatus::Confirmed));
        
        // 再次确认应该失败
        let confirmed_again = manager.confirm(&request_id).await;
        assert!(!confirmed_again);
    }

    #[tokio::test]
    async fn test_confirmation_reject() {
        let manager = ConfirmationManager::new(60);
        
        let request = ConfirmationRequest::new("task-1", "run-1", 60);
        let request_id = request.id.clone();
        
        manager.add_request(request).await;
        
        // 拒绝请求
        let rejected = manager.reject(&request_id).await;
        assert!(rejected);
        
        let status = manager.get_status(&request_id).await;
        assert_eq!(status, Some(ConfirmationStatus::Rejected));
        
        // 确认已拒绝的请求应该失败
        let confirmed = manager.confirm(&request_id).await;
        assert!(!confirmed);
    }
}
