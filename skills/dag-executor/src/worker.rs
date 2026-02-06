//! Worker 管理模块
//!
//! 管理 Worker 进程的生命周期

use std::collections::HashMap;

use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use cis_core::scheduler::DagScope;
use crate::error::DagExecutorError;

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

/// Run 信息
#[derive(Debug, Clone)]
pub struct RunInfo {
    pub run_id: String,
    pub worker_id: String,
    pub status: RunStatus,
    pub task_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Run 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Worker 池配置
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// 最大 worker 数量
    pub max_workers: usize,
    /// Worker 空闲超时时间（秒）
    pub idle_timeout_secs: u64,
    /// 是否启用 LRU 淘汰
    pub enable_lru: bool,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            max_workers: 10,
            idle_timeout_secs: 300, // 5分钟
            enable_lru: true,
        }
    }
}

/// Worker 管理器
pub struct WorkerManager {
    /// Worker 映射: worker_id -> WorkerInfo
    workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
    /// Run 映射: run_id -> RunInfo
    runs: Arc<Mutex<HashMap<String, RunInfo>>>,
    /// 访问顺序（用于 LRU）
    access_order: Arc<Mutex<Vec<String>>>,
    /// 配置
    config: WorkerPoolConfig,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self::with_config(WorkerPoolConfig::default())
    }

    pub fn with_config(config: WorkerPoolConfig) -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            runs: Arc::new(Mutex::new(HashMap::new())),
            access_order: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }

    /// 获取或创建 Worker（Task 3.2）
    /// 
    /// 逻辑：
    /// 1. 检查现有 Worker 是否存活
    /// 2. 如果存活，更新访问时间并返回
    /// 3. 如果不存活或不存在，检查是否达到 max_workers
    /// 4. 如果达到限制，淘汰最久未使用的 Worker
    /// 5. 创建新 Worker
    pub async fn get_or_create_worker<F, Fut>(
        &self,
        worker_id: &str,
        scope: DagScope,
        spawn_fn: F,
    ) -> Result<String, DagExecutorError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(Child, String), DagExecutorError>>,
    {
        // 1. 检查现有 Worker
        if let Some(room_id) = self.check_and_get_room(worker_id).await {
            self.update_access_time(worker_id).await;
            return Ok(room_id);
        }

        // 2. 检查是否达到 max_workers
        let current_count = self.worker_count().await;
        if current_count >= self.config.max_workers && self.config.enable_lru {
            // 淘汰最久未使用的 Worker
            self.evict_oldest_worker().await?;
        }

        // 3. 创建新 Worker
        let (process, room_id) = spawn_fn().await?;
        
        // 4. 添加到管理器
        self.add_worker(worker_id.to_string(), scope, process, room_id.clone()).await;
        self.update_access_time(worker_id).await;

        Ok(room_id)
    }

    /// 更新 Worker 访问时间（LRU）
    async fn update_access_time(&self, worker_id: &str) {
        let mut order = self.access_order.lock().await;
        // 移除旧位置
        order.retain(|id| id != worker_id);
        // 添加到末尾（最新）
        order.push(worker_id.to_string());
    }

    /// 淘汰最久未使用的 Worker
    async fn evict_oldest_worker(&self) -> Result<(), DagExecutorError> {
        let oldest_id = {
            let order = self.access_order.lock().await;
            order.first().cloned()
        };

        if let Some(worker_id) = oldest_id {
            info!("Evicting oldest worker: {}", worker_id);
            self.stop_worker(&worker_id).await?;
        }

        Ok(())
    }

    /// 停止指定 Worker
    pub async fn stop_worker(&self, worker_id: &str) -> Result<(), DagExecutorError> {
        let mut workers = self.workers.lock().await;
        
        if let Some(info) = workers.get_mut(worker_id) {
            if let Err(e) = info.kill().await {
                error!("Failed to stop worker {}: {}", worker_id, e);
            }
        }
        
        workers.remove(worker_id);
        drop(workers);

        // 从访问顺序中移除
        let mut order = self.access_order.lock().await;
        order.retain(|id| id != worker_id);

        Ok(())
    }

    /// 清理已死亡的 Worker（Task 3.2）
    pub async fn cleanup_dead_workers(&self) -> Vec<String> {
        let mut workers = self.workers.lock().await;
        let mut dead_workers = Vec::new();
        let mut to_remove = Vec::new();

        // 检查每个 Worker 的存活状态
        for (worker_id, info) in workers.iter_mut() {
            if !info.is_alive().await {
                dead_workers.push(worker_id.clone());
                to_remove.push(worker_id.clone());
            }
        }

        // 移除死亡的 Worker
        for worker_id in &to_remove {
            workers.remove(worker_id);
        }

        drop(workers);

        // 更新访问顺序
        let mut order = self.access_order.lock().await;
        for worker_id in &dead_workers {
            order.retain(|id| id != worker_id);
        }

        dead_workers
    }

    /// 获取当前 Worker 数量
    pub async fn worker_count(&self) -> usize {
        self.workers.lock().await.len()
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

    /// 添加 Run
    pub async fn add_run(&self, run_id: String, worker_id: String, task_count: usize) {
        let mut runs = self.runs.lock().await;
        let info = RunInfo {
            run_id: run_id.clone(),
            worker_id,
            status: RunStatus::Running,
            task_count,
            completed_count: 0,
            failed_count: 0,
            started_at: chrono::Utc::now(),
        };
        runs.insert(run_id, info);
    }

    /// 获取 Run 状态
    pub async fn get_run_status(&self, run_id: &str) -> Option<crate::RunStatus> {
        let runs = self.runs.lock().await;
        runs.get(run_id).map(|info| crate::RunStatus {
            run_id: info.run_id.clone(),
            worker_id: info.worker_id.clone(),
            status: info.status.to_string(),
            task_count: info.task_count,
            completed_count: info.completed_count,
            failed_count: info.failed_count,
            started_at: info.started_at.to_rfc3339(),
        })
    }

    /// 更新 Run 状态
    pub async fn update_run_status(&self, run_id: &str, status: RunStatus) {
        let mut runs = self.runs.lock().await;
        if let Some(info) = runs.get_mut(run_id) {
            info.status = status;
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

/// 启动 Worker 进程（Task 3.1）
///
/// 命令: `cis worker run --id worker-project-proj-a --scope project:proj-a`
pub async fn spawn_worker(
    worker_id: String,
    scope: cis_core::scheduler::DagScope,
    worker_binary: String,
    node_id: String,
) -> Result<(tokio::process::Child, String), crate::error::DagExecutorError> {
    use crate::process_lock::ProcessLock;
    use tracing::info;

    // 1. 尝试获取文件锁
    let _lock = ProcessLock::try_acquire(&worker_id)
        .ok_or_else(|| crate::error::DagExecutorError::SpawnFailed(
            format!("Worker {} is already running or lock failed", worker_id)
        ))?;

    // 2. 生成 Worker 专用 Room ID
    let room_id = format!("!worker-{}:{}", worker_id, node_id);

    // 3. 构建启动参数
    let scope_str = match &scope {
        cis_core::scheduler::DagScope::Global => "global".to_string(),
        cis_core::scheduler::DagScope::Project { project_id, .. } => format!("project:{}", project_id),
        cis_core::scheduler::DagScope::User { user_id, .. } => format!("user:{}", user_id),
        cis_core::scheduler::DagScope::Type { dag_type, .. } => format!("type:{}", dag_type),
    };

    let worker_args = vec![
        "worker".to_string(),
        "run".to_string(),
        "--id".to_string(), worker_id.clone(),
        "--scope".to_string(), scope_str,
        "--room".to_string(), room_id.clone(),
        "--parent-node".to_string(), node_id,
    ];

    // 4. 启动 Worker 进程
    info!("Spawning worker {} with binary {}", worker_id, worker_binary);
    
    let child = tokio::process::Command::new(&worker_binary)
        .args(&worker_args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| crate::error::DagExecutorError::SpawnFailed(
            format!("Failed to spawn worker: {}", e)
        ))?;

    // 5. 启动孤儿进程检测
    let worker_id_for_orphan = worker_id.clone();
    let orphan_detector = crate::process_lock::OrphanDetector::new(
        worker_id.clone(),
        5, // 每 5 秒检查一次父进程
    );
    
    tokio::spawn(async move {
        orphan_detector.start(move || {
            // 父进程死亡时的处理
            tracing::warn!("Worker {} becoming orphan, initiating shutdown", worker_id_for_orphan);
            // 这里可以添加清理逻辑
            std::process::exit(1);
        }).await;
    });

    // 6. 等待 Worker 初始化
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 7. 检查是否启动成功
    let mut child = child;
    match child.try_wait() {
        Ok(None) => {
            info!("Worker {} started successfully (room: {})", worker_id, room_id);
            Ok((child, room_id))
        }
        Ok(Some(status)) => {
            Err(crate::error::DagExecutorError::SpawnFailed(
                format!("Worker exited immediately with status: {:?}", status)
            ))
        }
        Err(e) => {
            Err(crate::error::DagExecutorError::SpawnFailed(
                format!("Failed to check worker status: {}", e)
            ))
        }
    }
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
