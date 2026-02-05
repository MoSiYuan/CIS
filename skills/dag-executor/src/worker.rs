//! Worker 管理模块
//!
//! 管理 Worker 进程的生命周期

use std::collections::HashMap;

use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use cis_core::scheduler::DagScope;

/// Worker 信息
#[derive(Debug)]
pub struct WorkerInfo {
    /// Worker ID
    pub worker_id: String,
    /// 作用域
    pub scope: DagScope,
    /// 进程句柄
    process: Child,
    /// 启动时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Matrix Room ID
    pub room_id: String,
    /// 活跃任务数
    pub active_tasks: usize,
}

impl WorkerInfo {
    pub fn new(worker_id: String, scope: DagScope, process: Child, room_id: String) -> Self {
        Self {
            worker_id,
            scope,
            process,
            started_at: chrono::Utc::now(),
            room_id,
            active_tasks: 0,
        }
    }

    /// 检查进程是否仍在运行
    pub async fn is_alive(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                debug!("Worker {} exited with status: {:?}", self.worker_id, status);
                false
            }
            Err(e) => {
                error!("Failed to check worker {} status: {}", self.worker_id, e);
                false
            }
        }
    }

    /// 终止 Worker 进程
    pub async fn kill(&mut self) -> std::io::Result<()> {
        match self.process.kill().await {
            Ok(_) => {
                info!("Worker {} terminated", self.worker_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to kill worker {}: {}", self.worker_id, e);
                Err(e)
            }
        }
    }
}

/// Worker 管理器
pub struct WorkerManager {
    /// Worker 映射: worker_id -> WorkerInfo
    workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 检查 Worker 是否存在且存活，返回 room_id
    pub async fn check_and_get_room(&self, worker_id: &str) -> Option<String> {
        let mut workers = self.workers.lock().await;
        
        if let Some(info) = workers.get_mut(worker_id) {
            if info.is_alive().await {
                return Some(info.room_id.clone());
            } else {
                // Worker 已死亡，移除
                workers.remove(worker_id);
            }
        }
        
        None
    }

    /// 获取 Worker 信息
    pub async fn get_worker_info(&self, worker_id: &str) -> Option<WorkerSummary> {
        let mut workers = self.workers.lock().await;
        
        if let Some(info) = workers.get_mut(worker_id) {
            Some(WorkerSummary {
                worker_id: worker_id.to_string(),
                scope: format!("{:?}", info.scope),
                is_alive: info.is_alive().await,
                started_at: info.started_at,
                active_tasks: info.active_tasks,
                room_id: info.room_id.clone(),
            })
        } else {
            None
        }
    }

    /// 添加 Worker
    pub async fn add_worker(
        &self,
        worker_id: String,
        scope: DagScope,
        process: Child,
        room_id: String,
    ) {
        let mut workers = self.workers.lock().await;
        let info = WorkerInfo::new(worker_id.clone(), scope, process, room_id);
        workers.insert(worker_id, info);
    }

    /// 移除 Worker
    pub async fn remove_worker(&self, worker_id: &str) {
        let mut workers = self.workers.lock().await;
        workers.remove(worker_id);
    }

    /// 增加任务计数
    pub async fn increment_tasks(&self, worker_id: &str) {
        let mut workers = self.workers.lock().await;
        if let Some(info) = workers.get_mut(worker_id) {
            info.active_tasks += 1;
        }
    }

    /// 减少任务计数
    pub async fn decrement_tasks(&self, worker_id: &str) {
        let mut workers = self.workers.lock().await;
        if let Some(info) = workers.get_mut(worker_id) {
            if info.active_tasks > 0 {
                info.active_tasks -= 1;
            }
        }
    }

    /// 停止所有 Worker
    pub async fn stop_all(&self) {
        let mut workers = self.workers.lock().await;
        
        for (worker_id, info) in workers.iter_mut() {
            if let Err(e) = info.kill().await {
                error!("Failed to stop worker {}: {}", worker_id, e);
            }
        }
        
        workers.clear();
        info!("All workers stopped");
    }

    /// 列出所有 Worker
    pub async fn list_workers(&self) -> Vec<WorkerSummary> {
        let mut workers = self.workers.lock().await;
        let mut summaries = Vec::new();
        
        for (id, info) in workers.iter_mut() {
            summaries.push(WorkerSummary {
                worker_id: id.clone(),
                scope: format!("{:?}", info.scope),
                is_alive: info.is_alive().await,
                started_at: info.started_at,
                active_tasks: info.active_tasks,
                room_id: info.room_id.clone(),
            });
        }
        
        summaries
    }

    /// 获取统计信息
    pub async fn stats(&self) -> WorkerStats {
        let workers = self.workers.lock().await;
        WorkerStats {
            total: workers.len(),
            active: workers.values().filter(|_| true).count(),
        }
    }
}

/// Worker 摘要
#[derive(Debug, Clone)]
pub struct WorkerSummary {
    pub worker_id: String,
    pub scope: String,
    pub is_alive: bool,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub active_tasks: usize,
    pub room_id: String,
}

/// Worker 统计
#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub total: usize,
    pub active: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_manager_new() {
        let manager = WorkerManager::new();
        let stats = manager.stats().await;
        assert_eq!(stats.total, 0);
        assert_eq!(stats.active, 0);
    }
}
