//! # Worker 主程序
//!
//! 独立的 Agent 进程（类似 Claude Code），通过 Matrix Room 接收任务并执行。
//!
//! ## 启动方式
//! ```bash
//! cis worker run \
//!   --id worker-project-proj-a \
//!   --scope project:proj-a \
//!   --room "!worker-project-proj-a:node1" \
//!   --parent-node node1
//! ```
//!
//! ## 执行循环
//! 1. 连接 Matrix Room
//! 2. 监听 Room 消息（DAG 任务）
//! 3. 执行任务（shell/skill）
//! 4. 上报结果到 Room

use std::sync::Arc;

use clap::Parser;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use cis_core::scheduler::{DagRun, TaskDag, DagNodeStatus, DagRunStatus};
use cis_core::matrix::events::{DagExecuteEvent, NodeClaimFilter, parse_dag_event};

/// Worker CLI 参数
#[derive(Parser, Debug)]
#[command(name = "cis-worker")]
#[command(about = "CIS DAG Worker - Agent process for task execution")]
pub struct WorkerArgs {
    /// Worker ID
    #[arg(long)]
    pub id: String,
    
    /// Scope (format: type:id, e.g., project:proj-a)
    #[arg(long)]
    pub scope: String,
    
    /// Matrix Room ID
    #[arg(long)]
    pub room: String,
    
    /// Parent node ID
    #[arg(long)]
    pub parent_node: String,
    
    /// Data directory
    #[arg(long, default_value = "~/.cis/worker")]
    pub data_dir: String,
    
    /// Max concurrent tasks
    #[arg(long, default_value = "4")]
    pub max_workers: usize,
}

/// Worker Agent 主结构
pub struct WorkerAgent {
    /// Worker ID
    worker_id: String,
    /// Scope
    scope: String,
    /// Room ID
    room_id: String,
    /// Parent node
    parent_node: String,
    /// 当前运行的 DAG Runs
    active_runs: Arc<Mutex<Vec<DagRun>>>,
    /// 配置
    config: WorkerConfig,
}

/// Worker 配置
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub max_concurrent_tasks: usize,
    pub data_dir: String,
}

impl WorkerAgent {
    /// 创建新的 Worker Agent
    pub fn new(args: WorkerArgs) -> Self {
        let config = WorkerConfig {
            max_concurrent_tasks: args.max_workers,
            data_dir: shellexpand::tilde(&args.data_dir).to_string(),
        };
        
        Self {
            worker_id: args.id,
            scope: args.scope,
            room_id: args.room,
            parent_node: args.parent_node,
            active_runs: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }
    
    /// 启动 Worker 执行循环（Task 5.1）
    pub async fn run(&self) -> anyhow::Result<()> {
        info!(
            "Worker {} started (scope: {}, room: {})",
            self.worker_id, self.scope, self.room_id
        );
        
        // 1. 初始化 - 确保数据目录存在
        tokio::fs::create_dir_all(&self.config.data_dir).await?;
        
        // 2. 创建节点认领过滤器
        let claim_filter = NodeClaimFilter::new(
            self.parent_node.clone(),
            true, // 接受广播任务
        );
        
        // 3. 主执行循环
        info!("Worker {} entering main execution loop", self.worker_id);
        
        loop {
            // 模拟从 Room 接收消息
            // 实际实现中，这里应该通过 Matrix Client 接收消息
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            
            // 检查是否应该退出（父进程死亡检测）
            if !self.is_parent_alive().await {
                warn!("Parent process died, worker {} shutting down", self.worker_id);
                break;
            }
            
            // 处理待执行任务
            self.process_pending_tasks(&claim_filter).await?;
            
            // 清理已完成的 runs
            self.cleanup_finished_runs().await;
        }
        
        info!("Worker {} stopped", self.worker_id);
        Ok(())
    }
    
    /// 检查父进程是否存活
    async fn is_parent_alive(&self) -> bool {
        // 简化实现：检查父进程 PID
        // 实际应该通过 ProcessLock 或信号检测
        true
    }
    
    /// 处理待执行任务（Task 5.1）
    async fn process_pending_tasks(
        &self,
        claim_filter: &NodeClaimFilter,
    ) -> anyhow::Result<()> {
        // 这里应该实际从 Matrix Room 拉取消息
        // 简化实现：从文件或内存队列获取
        
        // 模拟：检查是否有新任务
        let pending_tasks = self.fetch_pending_tasks().await;
        
        for task_msg in pending_tasks {
            // 解析 DAG 事件
            if let Some(event) = parse_dag_event(&task_msg) {
                // 节点认领过滤（Task 4.3）
                let (should_execute, reason) = claim_filter.should_execute(&event);
                
                if should_execute {
                    info!("Claiming DAG {}: {}", event.content.dag_id, reason);
                    self.execute_dag(event).await?;
                } else {
                    debug!("Ignoring DAG {}: {}", event.content.dag_id, reason);
                }
            }
        }
        
        Ok(())
    }
    
    /// 从 Room 获取待处理任务
    async fn fetch_pending_tasks(&self) -> Vec<String> {
        // 实际实现：通过 Matrix Client 拉取 Room 消息
        // 简化返回空
        vec![]
    }
    
    /// 执行 DAG（Task 5.1）
    async fn execute_dag(&self, event: DagExecuteEvent) -> anyhow::Result<()> {
        let content = event.content;
        
        info!("Starting execution of DAG {}", content.dag_id);
        
        // 1. 构建 TaskDag
        let mut task_dag = TaskDag::new();
        for task in &content.tasks {
            task_dag.add_node(task.id.clone(), task.depends_on.clone())?;
        }
        task_dag.initialize();
        
        // 2. 创建 DagRun
        let mut dag_run = DagRun::new(task_dag);
        dag_run.init_todo_from_tasks();
        
        let run_id = dag_run.run_id.clone();
        
        // 3. 添加到活跃 runs
        {
            let mut runs = self.active_runs.lock().await;
            runs.push(dag_run);
        }
        
        // 4. 启动执行循环（Task 5.1）
        let active_runs = self.active_runs.clone();
        let worker_id = self.worker_id.clone();
        
        tokio::spawn(async move {
            if let Err(e) = run_execution_loop(&run_id, active_runs, &worker_id).await {
                error!("Execution loop failed for {}: {}", run_id, e);
            }
        });
        
        info!("DAG {} execution started with run_id: {}", content.dag_id, run_id);
        Ok(())
    }
    
    /// 清理已完成的 runs
    async fn cleanup_finished_runs(&self) {
        let mut runs = self.active_runs.lock().await;
        let before_count = runs.len();
        
        runs.retain(|run| {
            !matches!(run.status, DagRunStatus::Completed | DagRunStatus::Failed)
        });
        
        let after_count = runs.len();
        if before_count != after_count {
            debug!("Cleaned up {} finished runs", before_count - after_count);
        }
    }
}

/// 执行循环 - 实际执行任务（Task 5.1）
async fn run_execution_loop(
    run_id: &str,
    active_runs: Arc<Mutex<Vec<DagRun>>>,
    worker_id: &str,
) -> anyhow::Result<()> {
    info!("[{}] Execution loop started for run {}", worker_id, run_id);
    
    loop {
        // 获取待执行的任务
        let ready_tasks = {
            let runs = active_runs.lock().await;
            let run = runs.iter().find(|r| r.run_id == run_id);
            
            if let Some(run) = run {
                if run.status == DagRunStatus::Failed {
                    break;
                }
                if run.dag.get_ready_tasks().is_empty() 
                    && run.dag.nodes().values().all(|n| 
                        matches!(n.status, DagNodeStatus::Completed | DagNodeStatus::Skipped | DagNodeStatus::Failed)
                    ) {
                    // 所有任务完成
                    break;
                }
                run.dag.get_ready_tasks()
            } else {
                break;
            }
        };
        
        // 执行就绪的任务
        for task_id in ready_tasks {
            // 标记为运行中
            {
                let mut runs = active_runs.lock().await;
                if let Some(run) = runs.iter_mut().find(|r| r.run_id == run_id) {
                    if let Err(e) = run.dag.mark_running(task_id.clone()) {
                        warn!("Failed to mark task {} running: {:?}", task_id, e);
                        continue;
                    }
                }
            }
            
            // 执行任务（Task 5.1）
            info!("[{}] Executing task: {}", worker_id, task_id);
            
            // 实际执行（shell 命令或 skill 调用）
            let result = execute_task(&task_id).await;
            
            // 更新状态
            {
                let mut runs = active_runs.lock().await;
                if let Some(run) = runs.iter_mut().find(|r| r.run_id == run_id) {
                    match result {
                        Ok(_) => {
                            if let Err(e) = run.dag.mark_completed(task_id) {
                                warn!("Failed to mark task {} completed: {:?}", task_id, e);
                            }
                        }
                        Err(_) => {
                            // 失败处理（Task 5.3 - 重试逻辑）
                            handle_task_failure(run, &task_id).await;
                        }
                    }
                    run.update_status();
                }
            }
        }
        
        // 避免忙等
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    info!("[{}] Execution loop finished for run {}", worker_id, run_id);
    Ok(())
}

/// 执行任务
async fn execute_task(task_id: &str) -> anyhow::Result<String> {
    // 简化实现：模拟任务执行
    // 实际应该：
    // 1. 解析任务命令
    // 2. 执行 shell 命令或调用 skill
    // 3. 捕获输出
    
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // 模拟成功
    Ok(format!("Task {} completed", task_id))
}

/// 任务执行上下文（包含重试信息）
#[derive(Debug, Clone, Default)]
pub struct TaskExecutionContext {
    /// 当前重试次数
    pub retry_count: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔（秒）
    pub retry_delay_secs: u64,
    /// 是否启用指数退避
    pub exponential_backoff: bool,
}

impl TaskExecutionContext {
    pub fn new(max_retries: u32, retry_delay_secs: u64, exponential_backoff: bool) -> Self {
        Self {
            retry_count: 0,
            max_retries,
            retry_delay_secs,
            exponential_backoff,
        }
    }

    /// 计算下一次重试的延迟时间
    pub fn next_retry_delay(&self) -> tokio::time::Duration {
        let base_delay = self.retry_delay_secs;
        let delay = if self.exponential_backoff {
            base_delay * (2_u64.pow(self.retry_count))
        } else {
            base_delay
        };
        // 最大延迟 5 分钟
        tokio::time::Duration::from_secs(delay.min(300))
    }

    /// 是否可以重试
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// 增加重试计数
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// 处理任务失败（Task 5.3）
async fn handle_task_failure(
    run: &mut DagRun, 
    task_id: &str,
    context: &mut TaskExecutionContext,
) {
    if context.can_retry() {
        context.increment_retry();
        let delay = context.next_retry_delay();
        
        warn!(
            "Task {} failed, will retry in {:?} ({}/{})", 
            task_id, delay, context.retry_count, context.max_retries
        );
        
        // 等待后重试
        tokio::time::sleep(delay).await;
        
        // 重置任务状态到 Pending
        if let Err(e) = run.dag.reset_node(task_id) {
            warn!("Failed to reset task {} for retry: {:?}", task_id, e);
        }
    } else {
        // 超过重试次数，标记为失败
        error!("Task {} failed after {} retries, giving up", task_id, context.max_retries);
        if let Err(e) = run.dag.mark_failed(task_id.to_string()) {
            warn!("Failed to mark task {} as failed: {:?}", task_id, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_agent_creation() {
        let args = WorkerArgs {
            id: "test-worker".to_string(),
            scope: "project:test".to_string(),
            room: "!test:node1".to_string(),
            parent_node: "node1".to_string(),
            data_dir: "/tmp/cis-test".to_string(),
            max_workers: 2,
        };
        
        let agent = WorkerAgent::new(args);
        assert_eq!(agent.worker_id, "test-worker");
        assert_eq!(agent.scope, "project:test");
    }
}
