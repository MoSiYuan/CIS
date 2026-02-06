//! DAG Executor Skill
//!
//! 基于作用域隔离的 DAG 执行器 Skill
//!
//! 功能：
//! - 接收 DAG 执行请求（通过 Matrix Event 或 CIS Event）
//! - 根据 DagScope 创建/复用 Worker 进程
//! - 通过 Matrix Room 向 Worker 分发 Task
//!
//! Worker 隔离策略：
//! - Global: 共享 worker-global
//! - Project: 每 project 独立 worker-project-{id}
//! - User: 每 user 独立 worker-user-{id}
//! - Type: 每 type 独立 worker-type-{type}

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use cis_core::scheduler::DagSpec;
use cis_core::skill::{Event, Skill, SkillConfig, SkillContext};
use cis_core::matrix::nucleus::{MatrixNucleus, RoomOptions, RoomId};
use ruma::events::room::message::RoomMessageEventContent;

pub mod error;
pub mod process_lock;
pub mod worker;

use error::DagExecutorError;
use worker::WorkerManager;
use process_lock::{ProcessLock, OrphanDetector};

/// Task 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（秒）
    pub retry_delay_secs: u64,
    /// 是否启用指数退避
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_secs: 5,
            exponential_backoff: true,
        }
    }
}

/// DAG 执行器 Skill
pub struct DagExecutorSkill {
    /// Skill 名称
    name: String,
    /// Worker 管理器
    worker_manager: WorkerManager,
    /// Matrix Nucleus（用于发送 Room 事件）
    nucleus: Mutex<Option<Arc<MatrixNucleus>>>,
    /// 节点 ID
    node_id: String,
    /// Worker 二进制路径
    worker_binary: String,
    /// 重试配置
    retry_config: RetryConfig,
}

impl DagExecutorSkill {
    /// 创建新的 DAG 执行器 Skill
    pub fn new(node_id: String, worker_binary: String) -> Self {
        Self {
            name: "dag-executor".to_string(),
            worker_manager: WorkerManager::new(),
            nucleus: Mutex::new(None),
            node_id,
            worker_binary,
            retry_config: RetryConfig::default(),
        }
    }
    
    /// 创建带自定义重试配置的 DAG 执行器
    pub fn with_retry_config(
        node_id: String, 
        worker_binary: String, 
        retry_config: RetryConfig
    ) -> Self {
        Self {
            name: "dag-executor".to_string(),
            worker_manager: WorkerManager::new(),
            nucleus: Mutex::new(None),
            node_id,
            worker_binary,
            retry_config,
        }
    }

    /// 执行 DAG
    async fn execute_dag(&self, spec: DagSpec) -> Result<String, DagExecutorError> {
        info!("Executing DAG {} with scope {:?}", spec.dag_id, spec.scope);

        let worker_id = spec.worker_id();
        let run_id = format!("dag-run-{}-{}", spec.dag_id, uuid::Uuid::new_v4());

        // 1. 确保 Worker 存在
        let room_id = self.ensure_worker(&worker_id, &spec.scope).await?;

        // 2. 分发每个 Task 到 Worker
        for task in &spec.tasks {
            self.dispatch_task(&worker_id, &room_id, &run_id, task).await?;
        }

        info!("DAG {} dispatched to worker {} (run_id: {})", spec.dag_id, worker_id, run_id);
        Ok(run_id)
    }

    /// 确保 Worker 存在
    async fn ensure_worker(
        &self,
        worker_id: &str,
        scope: &cis_core::scheduler::DagScope,
    ) -> Result<String, DagExecutorError> {
        // 检查现有 Worker
        if let Some(room_id) = self.worker_manager.check_and_get_room(worker_id).await {
            debug!("Reusing existing worker {}", worker_id);
            return Ok(room_id);
        }

        // 创建新 Worker
        self.spawn_worker(worker_id, scope).await
    }

    /// 创建 Worker 进程
    async fn spawn_worker(
        &self,
        worker_id: &str,
        scope: &cis_core::scheduler::DagScope,
    ) -> Result<String, DagExecutorError> {
        info!("Spawning new worker {}", worker_id);

        // 生成 Worker 专用 Room ID
        let room_id = format!("!worker-{}:{}", worker_id, self.node_id);

        // 创建 Matrix Room（通过 Nucleus）
        let nucleus_guard = self.nucleus.lock().await;
        if let Some(nucleus) = nucleus_guard.as_ref() {
            let opts = RoomOptions::new(room_id.clone())
                .with_federate(false); // Worker Room 不需要联邦
            
            if let Err(e) = nucleus.create_room(opts).await {
                warn!("Failed to create worker room (may already exist): {}", e);
            }
        }
        drop(nucleus_guard);

        // 启动 Worker 进程
        let worker_binary = self.worker_binary.clone();
        let worker_args = vec![
            "worker".to_string(),
            "--worker-id".to_string(), worker_id.to_string(),
            "--room".to_string(), room_id.clone(),
            "--scope".to_string(), format!("{:?}", scope),
            "--parent-node".to_string(), self.node_id.clone(),
        ];

        let mut child = tokio::process::Command::new(&worker_binary)
            .args(&worker_args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| DagExecutorError::SpawnFailed(format!("Failed to spawn worker: {}", e)))?;

        // 等待 Worker 初始化
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 检查是否启动成功
        match child.try_wait() {
            Ok(None) => {
                // 进程仍在运行，启动成功
                self.worker_manager.add_worker(
                    worker_id.to_string(),
                    scope.clone(),
                    child,
                    room_id.clone(),
                ).await;

                info!("Worker {} started successfully (room: {})", worker_id, room_id);
                Ok(room_id)
            }
            Ok(Some(status)) => {
                Err(DagExecutorError::SpawnFailed(format!(
                    "Worker {} exited immediately with status: {:?}",
                    worker_id, status
                )))
            }
            Err(e) => {
                Err(DagExecutorError::SpawnFailed(format!(
                    "Failed to check worker status: {}", e
                )))
            }
        }
    }

    /// 分发 Task 到 Worker（带重试）
    async fn dispatch_task(
        &self,
        worker_id: &str,
        room_id: &str,
        run_id: &str,
        task: &cis_core::scheduler::DagTaskSpec,
    ) -> Result<(), DagExecutorError> {
        let mut last_error = None;
        let max_retries = self.retry_config.max_retries;
        
        for attempt in 0..=max_retries {
            match self.try_dispatch_task(worker_id, room_id, run_id, task).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!("Task {} dispatched successfully after {} retries", task.id, attempt);
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = if self.retry_config.exponential_backoff {
                            self.retry_config.retry_delay_secs * (2_u64.pow(attempt))
                        } else {
                            self.retry_config.retry_delay_secs
                        };
                        warn!(
                            "Task {} dispatch failed (attempt {}/{}), retrying in {}s...",
                            task.id,
                            attempt + 1,
                            max_retries + 1,
                            delay
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            DagExecutorError::DispatchFailed("Max retries exceeded".to_string())
        }))
    }
    
    /// 尝试分发 Task（单次，不重试）
    async fn try_dispatch_task(
        &self,
        worker_id: &str,
        room_id: &str,
        run_id: &str,
        task: &cis_core::scheduler::DagTaskSpec,
    ) -> Result<(), DagExecutorError> {
        // 构建事件内容 JSON
        let task_event = serde_json::json!({
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

        // 通过 Matrix Room 发送事件
        let nucleus_guard = self.nucleus.lock().await;
        if let Some(nucleus) = nucleus_guard.as_ref() {
            // 解析 Room ID
            let room_id_parsed = RoomId::parse(room_id)
                .map_err(|e| DagExecutorError::MatrixRoom(format!("Invalid room ID: {}", e)))?;
            
            // 创建 RoomMessageEventContent
            let content = RoomMessageEventContent::text_plain(task_event.to_string());
            
            // 发送事件
            match nucleus.send_event(&room_id_parsed, content).await {
                Ok(event_id) => {
                    info!("Task {} -> room {} (event_id: {})", task.id, room_id, event_id);
                }
                Err(e) => {
                    warn!("Failed to send task {} to room {}: {}", task.id, room_id, e);
                    return Err(DagExecutorError::MatrixRoom(format!(
                        "Failed to send event: {}", e
                    )));
                }
            }
        } else {
            // Nucleus 未初始化，仅记录日志（用于测试场景）
            info!("Task {} -> room {} (content: {}) - Nucleus not available, logged only", 
                task.id, room_id, task_event);
        }

        // 更新 Worker 活跃任务计数
        self.worker_manager.increment_tasks(worker_id).await;

        Ok(())
    }

    /// 获取 DAG 运行状态
    async fn get_run_status(&self, run_id: &str) -> Option<RunStatus> {
        self.worker_manager.get_run_status(run_id).await
    }
}

#[async_trait]
impl Skill for DagExecutorSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "DAG Executor - Scope-based worker isolation for DAG execution"
    }

    fn room_id(&self) -> Option<String> {
        Some(format!("!{}:{}", self.name, self.node_id))
    }

    async fn init(&mut self, _config: SkillConfig) -> cis_core::error::Result<()> {
        info!("DAG Executor Skill initialized");
        Ok(())
    }

    async fn init_room(&self, nucleus: Arc<MatrixNucleus>) -> cis_core::error::Result<()> {
        // 保存 Nucleus 引用
        let mut nucleus_guard = self.nucleus.lock().await;
        *nucleus_guard = Some(nucleus);
        drop(nucleus_guard);
        
        info!("DAG Executor Skill room initialized with MatrixNucleus");
        Ok(())
    }

    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> cis_core::error::Result<()> {
        match event {
            Event::Custom { name, data } => {
                match name.as_str() {
                    "dag:execute" => {
                        // 解析 DAG 规格
                        let spec: DagSpec = serde_json::from_value(data)
                            .map_err(|e| cis_core::error::CisError::skill(format!("Invalid DAG spec: {}", e)))?;

                        // 执行 DAG
                        match self.execute_dag(spec).await {
                            Ok(run_id) => {
                                ctx.log_info(&format!("DAG executed, run_id: {}", run_id));
                            }
                            Err(e) => {
                                ctx.log_error(&format!("DAG execution failed: {}", e));
                                return Err(cis_core::error::CisError::skill(e.to_string()));
                            }
                        }
                    }
                    "dag:execute:http" => {
                        // HTTP API 触发
                        // data 格式: { "dag_spec": {...} }
                        if let Some(dag_spec) = data.get("dag_spec") {
                            let spec: DagSpec = serde_json::from_value(dag_spec.clone())
                                .map_err(|e| cis_core::error::CisError::skill(format!("Invalid DAG spec: {}", e)))?;

                            match self.execute_dag(spec).await {
                                Ok(run_id) => {
                                    ctx.log_info(&format!("HTTP triggered DAG executed, run_id: {}", run_id));
                                }
                                Err(e) => {
                                    ctx.log_error(&format!("HTTP triggered DAG execution failed: {}", e));
                                    return Err(cis_core::error::CisError::skill(e.to_string()));
                                }
                            }
                        }
                    }
                    "dag:status" => {
                        // 查询 DAG 状态
                        if let Some(run_id) = data.get("run_id").and_then(|v| v.as_str()) {
                            let status = self.get_run_status(run_id).await;
                            ctx.log_info(&format!("DAG status for {}: {:?}", run_id, status));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn shutdown(&self) -> cis_core::error::Result<()> {
        info!("Shutting down DAG Executor Skill");
        self.worker_manager.stop_all().await;
        Ok(())
    }
}

/// DAG 执行事件（用于 Matrix Room 接收）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecuteEvent {
    pub dag_id: String,
    pub tasks: Vec<cis_core::scheduler::DagTaskSpec>,
    #[serde(default)]
    pub target_node: Option<String>,
    #[serde(default)]
    pub scope: cis_core::scheduler::DagScope,
}

/// DAG 运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStatus {
    pub run_id: String,
    pub worker_id: String,
    pub status: String,
    pub task_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub started_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_executor_skill_new() {
        let skill = DagExecutorSkill::new(
            "test-node".to_string(),
            "/usr/local/bin/cis-node".to_string(),
        );
        
        assert_eq!(skill.name(), "dag-executor");
        assert_eq!(skill.version(), "0.1.0");
    }
}
