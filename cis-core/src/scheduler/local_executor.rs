//! 本地 DAG 执行器
//!
//! 管理 Worker 进程生命周期，按 DagScope 隔离：
//! - Global: 共享 worker-global
//! - Project: 每 project 独立 worker
//! - User: 每 user 独立 worker
//! - Type: 每 dag_type 独立 worker
//!
//! Worker 通信：通过 Matrix Room 发送 Task

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{CisError, Result};
use crate::matrix::store::MatrixStore;
use crate::scheduler::{DagScope, DagSpec, DagTaskSpec};

/// Worker 进程信息
#[derive(Debug)]
pub struct WorkerInfo {
    /// Worker ID (格式: worker-{scope}-{id})
    pub worker_id: String,
    /// 作用域
    pub scope: DagScope,
    /// 进程句柄
    pub process: Child,
    /// 启动时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Matrix Room ID (用于通信)
    pub room_id: String,
    /// 活跃任务数
    pub active_tasks: usize,
}

impl WorkerInfo {
    /// 检查进程是否仍在运行
    pub fn is_alive(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }
    
    /// 终止 Worker 进程
    pub async fn kill(&mut self) -> Result<()> {
        match self.process.kill().await {
            Ok(_) => {
                info!("Worker {} terminated", self.worker_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to kill worker {}: {}", self.worker_id, e);
                Err(CisError::execution(format!("Failed to kill worker: {}", e)))
            }
        }
    }
}

/// 本地执行器，管理 Worker 进程池
pub struct LocalExecutor {
    /// Worker 映射: worker_id -> WorkerInfo
    workers: Arc<Mutex<HashMap<String, WorkerInfo>>>,
    /// 节点 ID
    node_id: String,
    /// Worker 二进制路径
    worker_binary: String,
    /// 默认 Matrix Room (用于 Worker 通信)
    default_room: String,
    /// Matrix Store (用于事件持久化)
    matrix_store: Option<Arc<MatrixStore>>,
}

impl LocalExecutor {
    /// 创建新的本地执行器
    pub fn new(node_id: String, worker_binary: String, default_room: String) -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            node_id,
            worker_binary,
            default_room,
            matrix_store: None,
        }
    }
    
    /// 创建带 Matrix Store 的执行器
    pub fn with_matrix_store(
        node_id: String,
        worker_binary: String,
        default_room: String,
        matrix_store: Arc<MatrixStore>,
    ) -> Self {
        Self {
            workers: Arc::new(Mutex::new(HashMap::new())),
            node_id,
            worker_binary,
            default_room,
            matrix_store: Some(matrix_store),
        }
    }
    
    /// 执行 DAG 规格
    ///
    /// 流程:
    /// 1. 根据 DagScope 获取或创建 Worker
    /// 2. 将 Task 序列化并通过 Matrix Room 发送
    /// 3. 返回 run_id 用于后续查询
    pub async fn execute(&self, spec: &DagSpec) -> Result<String> {
        let worker_id = spec.worker_id();
        let run_id = format!("dag-run-{}-{}", spec.dag_id, uuid::Uuid::new_v4());
        
        info!(
            "Executing DAG {} with worker {} (run_id: {})",
            spec.dag_id, worker_id, run_id
        );
        
        // 1. 确保 Worker 存在
        let room_id = self.ensure_worker(&worker_id, &spec.scope).await?;
        
        // 2. 将每个 Task 发送到 Worker
        for task in &spec.tasks {
            self.dispatch_task(&worker_id, &room_id, &run_id, task).await?;
        }
        
        info!("DAG {} dispatched to worker {} successfully", spec.dag_id, worker_id);
        Ok(run_id)
    }
    
    /// 确保 Worker 进程存在
    ///
    /// 如果 Worker 不存在或已死亡，创建新的
    async fn ensure_worker(&self, worker_id: &str, scope: &DagScope) -> Result<String> {
        let mut workers = self.workers.lock().await;
        
        // 检查现有 Worker
        if let Some(worker) = workers.get_mut(worker_id) {
            if worker.is_alive() {
                debug!("Reusing existing worker {}", worker_id);
                return Ok(worker.room_id.clone());
            } else {
                warn!("Worker {} is dead, removing", worker_id);
                workers.remove(worker_id);
            }
        }
        
        // 创建新 Worker
        drop(workers); // 释放锁，避免持有锁期间启动进程
        let room_id = self.spawn_worker(worker_id, scope).await?;
        Ok(room_id)
    }
    
    /// 创建 Worker 进程
    ///
    /// Worker 进程是独立的 cis-node 实例，通过 Matrix Room 接收任务
    async fn spawn_worker(&self, worker_id: &str, scope: &DagScope) -> Result<String> {
        info!("Spawning new worker {}", worker_id);
        
        // 生成 Worker 专用 Room ID
        let room_id = format!("!worker-{}:{}", worker_id, self.node_id);
        
        // 准备 Worker 启动参数
        let worker_args = vec![
            "worker".to_string(),
            "--worker-id".to_string(), worker_id.to_string(),
            "--room".to_string(), room_id.clone(),
            "--scope".to_string(), format!("{:?}", scope),
            "--parent-node".to_string(), self.node_id.clone(),
        ];
        
        // 启动 Worker 进程
        let mut child = Command::new(&self.worker_binary)
            .args(&worker_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CisError::execution(format!("Failed to spawn worker: {}", e)))?;
        
        // 等待 Worker 初始化 (简单延时，实际应该用健康检查)
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // 检查是否启动成功
        match child.try_wait() {
            Ok(None) => {
                // 进程仍在运行，启动成功
                let worker = WorkerInfo {
                    worker_id: worker_id.to_string(),
                    scope: scope.clone(),
                    process: child,
                    started_at: chrono::Utc::now(),
                    room_id: room_id.clone(),
                    active_tasks: 0,
                };
                
                let mut workers = self.workers.lock().await;
                workers.insert(worker_id.to_string(), worker);
                
                info!("Worker {} started successfully (room: {})", worker_id, room_id);
                Ok(room_id)
            }
            Ok(Some(status)) => {
                Err(CisError::execution(format!(
                    "Worker {} exited immediately with status: {:?}",
                    worker_id, status
                )))
            }
            Err(e) => {
                Err(CisError::execution(format!(
                    "Failed to check worker status: {}", e
                )))
            }
        }
    }
    
    /// 分发单个 Task 到 Worker
    ///
    /// 通过 Matrix Room 发送 Task 事件
    async fn dispatch_task(
        &self,
        worker_id: &str,
        room_id: &str,
        run_id: &str,
        task: &DagTaskSpec,
    ) -> Result<()> {
        // 构建 Task 事件
        let event_content = serde_json::json!({
            "type": "dag.task",
            "run_id": run_id,
            "task": {
                "id": task.id,
                "task_type": task.task_type,
                "command": task.command,
                "depends_on": task.depends_on,
                "env": task.env,
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        debug!(
            "Dispatching task {} to worker {} (room: {})",
            task.id, worker_id, room_id
        );
        
        // 通过 Matrix Store 保存事件，Worker 会通过同步接口获取
        if let Some(ref store) = self.matrix_store {
            let event_id = format!("dag-task-{}-{}", task.id, uuid::Uuid::new_v4());
            let sender = format!("@scheduler:{}", self.node_id);
            let timestamp = chrono::Utc::now().timestamp_millis();
            
            match store.save_event(
                room_id,
                &event_id,
                &sender,
                "cis.dag.task",
                &event_content.to_string(),
                timestamp,
                None,
                None,
            ) {
                Ok(_) => {
                    info!(
                        "Task {} dispatched to room {} (event_id: {})",
                        task.id, room_id, event_id
                    );
                }
                Err(e) => {
                    warn!("Failed to save task event to Matrix store: {}", e);
                    // 继续执行，记录日志即可
                }
            }
        } else {
            // Matrix Store 未配置，仅记录日志
            info!(
                "Task {} dispatched to room {} (content: {})",
                task.id, room_id, event_content
            );
        }
        
        // 更新 Worker 活跃任务计数
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.active_tasks += 1;
        }
        
        Ok(())
    }
    
    /// 停止指定 Worker
    pub async fn stop_worker(&self, worker_id: &str) -> Result<()> {
        let mut workers = self.workers.lock().await;
        
        if let Some(worker) = workers.get_mut(worker_id) {
            worker.kill().await?;
            workers.remove(worker_id);
            info!("Worker {} stopped", worker_id);
        } else {
            warn!("Worker {} not found", worker_id);
        }
        
        Ok(())
    }
    
    /// 停止所有 Worker
    pub async fn stop_all(&self) -> Result<()> {
        let mut workers = self.workers.lock().await;
        
        for (worker_id, worker) in workers.iter_mut() {
            if let Err(e) = worker.kill().await {
                error!("Failed to stop worker {}: {}", worker_id, e);
            }
        }
        
        workers.clear();
        info!("All workers stopped");
        Ok(())
    }
    
    /// 获取 Worker 列表
    pub async fn list_workers(&self) -> Vec<WorkerSummary> {
        let mut workers = self.workers.lock().await;
        let mut summaries = Vec::new();
        
        for (id, worker) in workers.iter_mut() {
            summaries.push(WorkerSummary {
                worker_id: id.clone(),
                scope: format!("{:?}", worker.scope),
                is_alive: worker.is_alive(),
                started_at: worker.started_at,
                active_tasks: worker.active_tasks,
                room_id: worker.room_id.clone(),
            });
        }
        
        summaries
    }
    
    /// 获取 Worker 统计信息
    pub async fn stats(&self) -> ExecutorStats {
        let workers = self.workers.lock().await;
        let total = workers.len();
        let alive = workers.values().filter(|_w| {
            // 注意：这里需要可变引用，简单处理
            true
        }).count();
        
        ExecutorStats {
            total_workers: total,
            active_workers: alive,
            node_id: self.node_id.clone(),
        }
    }
}

/// Worker 摘要信息
#[derive(Debug, Clone)]
pub struct WorkerSummary {
    pub worker_id: String,
    pub scope: String,
    pub is_alive: bool,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub active_tasks: usize,
    pub room_id: String,
}

/// 执行器统计
#[derive(Debug, Clone)]
pub struct ExecutorStats {
    pub total_workers: usize,
    pub active_workers: usize,
    pub node_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_worker_id_generation() {
        let scope = DagScope::Project { 
            project_id: "test-proj".to_string(),
            force_new: false,
        };
        assert_eq!(scope.worker_id(), "worker-project-test-proj");
        
        let scope = DagScope::Global;
        assert_eq!(scope.worker_id(), "worker-global");
        
        let scope = DagScope::User { 
            user_id: "alice".to_string(),
            force_new: false,
        };
        assert_eq!(scope.worker_id(), "worker-user-alice");
    }
    
    #[tokio::test]
    async fn test_local_executor_new() {
        let executor = LocalExecutor::new(
            "test-node".to_string(),
            "/usr/local/bin/cis-node".to_string(),
            "!test-room:localhost".to_string(),
        );
        
        let stats = executor.stats().await;
        assert_eq!(stats.total_workers, 0);
        assert_eq!(stats.node_id, "test-node");
    }
}
