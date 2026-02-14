//! # Multi-Agent DAG Executor
//!
//! 支持多 Agent 的 DAG 执行器，每个 Task 可以指定不同的 Agent。
//!
//! 主要特性：
//! - 多 Runtime 支持（Claude, OpenCode, Kimi, Aider）
//! - Agent 复用和池化管理
//! - 上游上下文自动注入
//! - 并发任务执行
//! - 超时和错误处理

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::agent::persistent::{
    AgentAcquireConfig, AgentHandle, AgentPool, RuntimeType as AgentRuntimeType,
    TaskRequest, TaskResult,
};
use crate::agent::cluster::context::ContextStore;
use crate::error::{CisError, Result};
use crate::scheduler::{DagNode, DagNodeStatus, DagScheduler, RuntimeType, TaskDag};

/// 转换 scheduler::RuntimeType 到 persistent::RuntimeType
fn to_persistent_runtime_type(rt: RuntimeType) -> AgentRuntimeType {
    match rt {
        RuntimeType::Claude => AgentRuntimeType::Claude,
        RuntimeType::Kimi => AgentRuntimeType::Kimi,
        RuntimeType::Aider => AgentRuntimeType::Aider,
        RuntimeType::OpenCode => AgentRuntimeType::OpenCode,
        RuntimeType::Default => AgentRuntimeType::Claude, // 默认使用 Claude
    }
}

/// 转换 persistent::RuntimeType 到 scheduler::RuntimeType
fn to_scheduler_runtime_type(rt: AgentRuntimeType) -> RuntimeType {
    match rt {
        AgentRuntimeType::Claude => RuntimeType::Claude,
        AgentRuntimeType::Kimi => RuntimeType::Kimi,
        AgentRuntimeType::Aider => RuntimeType::Aider,
        AgentRuntimeType::OpenCode => RuntimeType::OpenCode,
    }
}

/// Scheduling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingMode {
    /// Event-driven scheduling (default, recommended)
    EventDriven,
    /// Legacy polling-based scheduling (fallback)
    Polling,
}

impl Default for SchedulingMode {
    fn default() -> Self {
        Self::EventDriven
    }
}

/// 多 Agent DAG 执行器配置
#[derive(Debug, Clone)]
pub struct MultiAgentExecutorConfig {
    /// Scheduling mode
    pub scheduling_mode: SchedulingMode,
    /// 默认 Agent Runtime
    pub default_runtime: AgentRuntimeType,
    /// 是否自动清理完成的 Agent
    pub auto_cleanup: bool,
    /// 任务超时时间
    pub task_timeout: Duration,
    /// 是否启用上游上下文注入
    pub enable_context_injection: bool,
    /// 最大并发任务数
    pub max_concurrent_tasks: usize,
}

impl Default for MultiAgentExecutorConfig {
    fn default() -> Self {
        Self {
            scheduling_mode: SchedulingMode::EventDriven,
            default_runtime: AgentRuntimeType::Claude,
            auto_cleanup: true,
            task_timeout: Duration::from_secs(300),
            enable_context_injection: true,
            max_concurrent_tasks: 4,
        }
    }
}

impl MultiAgentExecutorConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置调度模式
    pub fn with_scheduling_mode(mut self, mode: SchedulingMode) -> Self {
        self.scheduling_mode = mode;
        self
    }

    /// 设置默认 Runtime
    pub fn with_default_runtime(mut self, runtime: AgentRuntimeType) -> Self {
        self.default_runtime = runtime;
        self
    }

    /// 设置自动清理
    pub fn with_auto_cleanup(mut self, auto_cleanup: bool) -> Self {
        self.auto_cleanup = auto_cleanup;
        self
    }

    /// 设置任务超时
    pub fn with_task_timeout(mut self, timeout: Duration) -> Self {
        self.task_timeout = timeout;
        self
    }

    /// 设置上下文注入
    pub fn with_context_injection(mut self, enable: bool) -> Self {
        self.enable_context_injection = enable;
        self
    }

    /// 设置最大并发数
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent_tasks = max;
        self
    }
}

/// 多 Agent DAG 执行器
#[derive(Clone)]
pub struct MultiAgentDagExecutor {
    /// DAG 调度器
    scheduler: Arc<RwLock<DagScheduler>>,
    /// Agent 池
    agent_pool: AgentPool,
    /// 上下文存储
    context_store: ContextStore,
    /// 执行配置
    config: MultiAgentExecutorConfig,
    /// 当前运行的 Agent 表（run_id -> agent_id -> AgentHandle）
    run_agents: Arc<RwLock<HashMap<String, HashMap<String, AgentHandle>>>>,
}

impl std::fmt::Debug for MultiAgentDagExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiAgentDagExecutor")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// 任务执行结果
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub exit_code: i32,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskExecutionResult {
    /// 从 TaskResult 转换
    fn from_task_result(task_id: String, result: TaskResult) -> Self {
        Self {
            task_id,
            success: result.success,
            output: result.output.unwrap_or_default(),
            exit_code: if result.success { 0 } else { 1 },
            metadata: result.metadata,
        }
    }
}

/// 多 Agent 执行报告
#[derive(Debug, Clone)]
pub struct MultiAgentExecutionReport {
    pub run_id: String,
    pub duration_secs: u64,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub final_status: String,
    pub task_outputs: HashMap<String, TaskExecutionResult>,
}

impl MultiAgentDagExecutor {
    /// 创建新的执行器
    pub fn new(
        scheduler: DagScheduler,
        agent_pool: AgentPool,
        config: MultiAgentExecutorConfig,
    ) -> Result<Self> {
        let context_store = ContextStore::default_store()?;

        Ok(Self {
            scheduler: Arc::new(RwLock::new(scheduler)),
            agent_pool,
            context_store,
            config,
            run_agents: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 创建新的执行器（简化版，使用默认调度器）
    pub fn with_pool(agent_pool: AgentPool, config: MultiAgentExecutorConfig) -> Result<Self> {
        let scheduler = DagScheduler::new();
        Self::new(scheduler, agent_pool, config)
    }

    /// 创建 DAG 运行
    pub async fn create_run(&self, dag: TaskDag) -> Result<String> {
        let mut scheduler = self.scheduler.write().await;
        let run_id = scheduler.create_run(dag);
        info!("Created DAG run: {}", run_id);
        Ok(run_id)
    }

    /// 创建 DAG 运行并指定任务命令映射
    pub async fn create_run_with_commands(
        &self,
        dag: TaskDag,
        task_commands: HashMap<String, String>,
    ) -> Result<String> {
        let mut scheduler = self.scheduler.write().await;
        let run_id = scheduler.create_run_with_source(dag, None, None, task_commands);
        info!("Created DAG run with commands: {}", run_id);
        Ok(run_id)
    }

    /// 执行 DAG 运行（核心方法）
    pub async fn execute(&self, run_id: &str) -> Result<MultiAgentExecutionReport> {
        let start_time = std::time::Instant::now();
        info!(
            "Starting MultiAgent execution for run {} (mode: {:?})",
            run_id, self.config.scheduling_mode
        );

        // 初始化 DAG
        {
            let mut scheduler = self.scheduler.write().await;
            let run = scheduler
                .get_run_mut(run_id)
                .ok_or_else(|| CisError::scheduler("Run not found"))?;
            run.dag.initialize();
            info!("Initialized DAG with {} tasks", run.dag.node_count());
        }

        // 选择调度模式
        match self.config.scheduling_mode {
            SchedulingMode::EventDriven => {
                self.execute_event_driven(run_id, start_time).await
            }
            SchedulingMode::Polling => {
                self.execute_polling(run_id, start_time).await
            }
        }
    }

    /// 事件驱动执行（新实现）
    async fn execute_event_driven(
        &self,
        run_id: &str,
        start_time: std::time::Instant,
    ) -> Result<MultiAgentExecutionReport> {
        use crate::scheduler::notify::{ReadyNotify, TaskCompletion};
        use tokio::select;

        let ready_notify = Arc::new(ReadyNotify::new());
        let (completion_tx, mut completion_rx) = tokio::sync::broadcast::channel(100);

        // 初始通知
        ready_notify.notify_ready();

        loop {
            // 检查运行状态
            let should_stop = {
                let scheduler = self.scheduler.read().await;
                let run = scheduler.get_run(run_id);
                if let Some(run) = run {
                    run.status == crate::scheduler::DagRunStatus::Failed
                        || run.status == crate::scheduler::DagRunStatus::Paused
                } else {
                    return Err(CisError::scheduler("Run not found"));
                }
            };

            if should_stop {
                warn!("Run {} stopped due to failure or pause", run_id);
                break;
            }

            // 获取就绪任务
            let ready_tasks = {
                let scheduler = self.scheduler.read().await;
                let run = scheduler.get_run(run_id);
                if let Some(run) = run {
                    run.dag.get_ready_tasks()
                } else {
                    return Err(CisError::scheduler("Run not found"));
                }
            };

            // 检查是否完成
            if ready_tasks.is_empty() {
                let is_completed = {
                    let scheduler = self.scheduler.read().await;
                    let run = scheduler.get_run(run_id);
                    if let Some(run) = run {
                        run.dag.nodes().values().all(|n| n.is_terminal())
                    } else {
                        return Err(CisError::scheduler("Run not found"));
                    }
                };

                if is_completed {
                    info!("Run {} completed", run_id);
                    break;
                }
            } else {
                // 有就绪任务，限制并发数
                let active_count = self.count_active_agents(run_id).await;
                let available_slots = self
                    .config
                    .max_concurrent_tasks
                    .saturating_sub(active_count);

                if available_slots > 0 {
                    let tasks_to_start: Vec<String> = ready_tasks
                        .into_iter()
                        .take(available_slots)
                        .collect();

                    info!(
                        "Starting {} tasks for run {} (active: {}, slots: {})",
                        tasks_to_start.len(),
                        run_id,
                        active_count,
                        available_slots
                    );

                    // 启动任务
                    for task_id in tasks_to_start {
                        // 标记为运行中
                        {
                            let mut scheduler = self.scheduler.write().await;
                            if let Some(run) = scheduler.get_run_mut(run_id) {
                                let _ = run.dag.mark_running(task_id.clone());
                            }
                        }

                        // 异步执行任务
                        let this = self.clone_ref();
                        let run_id = run_id.to_string();
                        let task_id = task_id.clone();
                        let ready_notify = ready_notify.clone();
                        let completion_tx = completion_tx.clone();

                        tokio::spawn(async move {
                            let result = this.execute_task(&run_id, &task_id).await;

                            match result {
                                Ok(task_result) => {
                                    // 更新结果
                                    let _ = this.update_task_result(&run_id, &task_id, task_result).await;

                                    // 发送完成通知
                                    let _ = completion_tx.send(TaskCompletion {
                                        run_id: run_id.clone(),
                                        task_id: task_id.clone(),
                                        success: true,
                                        output: String::new(),
                                        exit_code: 0,
                                        duration_ms: 0,
                                    });

                                    // 通知有任务可能就绪
                                    ready_notify.notify_ready();
                                }
                                Err(e) => {
                                    warn!("Task {} execution failed: {}", task_id, e);
                                    let _ = this.mark_task_failed(&run_id, &task_id, e.to_string()).await;

                                    // 即使失败也通知（可能触发跳过等逻辑）
                                    ready_notify.notify_ready();
                                }
                            }
                        });
                    }
                }
            }

            // 等待事件
            select! {
                _ = ready_notify.wait_for_ready() => {
                    // 有任务就绪，继续循环
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    // 超时，继续循环检查
                }
            }
        }

        // 清理 Agent
        if let Err(e) = self.cleanup_run(run_id).await {
            warn!("Failed to cleanup run {}: {}", run_id, e);
        }

        // 生成报告
        self.build_report(run_id, start_time.elapsed()).await
    }

    /// 轮询执行（原始实现）
    async fn execute_polling(
        &self,
        run_id: &str,
        start_time: std::time::Instant,
    ) -> Result<MultiAgentExecutionReport> {
        // 主执行循环（原始轮询逻辑）
        loop {
            // 检查运行状态
            let should_stop = {
                let scheduler = self.scheduler.read().await;
                if let Some(run) = scheduler.get_run(run_id) {
                    run.status == crate::scheduler::DagRunStatus::Failed
                        || run.status == crate::scheduler::DagRunStatus::Paused
                } else {
                    return Err(CisError::scheduler("Run not found"));
                }
            };

            if should_stop {
                warn!("Run {} stopped due to failure or pause", run_id);
                break;
            }

            // 获取就绪任务
            let ready_tasks = {
                let scheduler = self.scheduler.read().await;
                let run = scheduler
                    .get_run(run_id)
                    .ok_or_else(|| CisError::scheduler("Run not found"))?;
                run.dag.get_ready_tasks()
            };

            if ready_tasks.is_empty() {
                // 检查是否完成
                let is_completed = {
                    let scheduler = self.scheduler.read().await;
                    let run = scheduler
                        .get_run(run_id)
                        .ok_or_else(|| CisError::scheduler("Run not found"))?;
                    run.dag.nodes().values().all(|n| n.is_terminal())
                };

                if is_completed {
                    info!("Run {} completed", run_id);
                    break;
                }

                // 等待一段时间再检查
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // 限制并发数
            let active_count = self.count_active_agents(run_id).await;
            let available_slots = self
                .config
                .max_concurrent_tasks
                .saturating_sub(active_count);

            if available_slots == 0 {
                debug!(
                    "Max concurrent reached ({}/{}), waiting...",
                    active_count, self.config.max_concurrent_tasks
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let tasks_to_start: Vec<String> = ready_tasks
                .into_iter()
                .take(available_slots)
                .collect();

            info!(
                "Starting {} tasks for run {} (active: {}, slots: {})",
                tasks_to_start.len(),
                run_id,
                active_count,
                available_slots
            );

            // 顺序执行就绪任务（避免 Send 问题）
            for task_id in tasks_to_start {
                // 标记任务为运行中
                {
                    let mut scheduler = self.scheduler.write().await;
                    let run = scheduler
                        .get_run_mut(run_id)
                        .ok_or_else(|| CisError::scheduler("Run not found"))?;
                    if let Err(e) = run.dag.mark_running(task_id.clone()) {
                        warn!("Failed to mark task {} running: {}", task_id, e);
                        continue;
                    }
                }

                // 执行任务
                match self.execute_task(run_id, &task_id).await {
                    Ok(result) => {
                        // 更新任务结果
                        if let Err(e) = self.update_task_result(run_id, &task_id, result).await {
                            warn!("Failed to update task result for {}: {}", task_id, e);
                        }
                    }
                    Err(e) => {
                        // 任务执行出错
                        warn!("Task {} execution failed: {}", task_id, e);
                        if let Err(e) = self.mark_task_failed(run_id, &task_id, e.to_string()).await
                        {
                            warn!("Failed to mark task {} as failed: {}", task_id, e);
                        }
                    }
                }
            }
        }

        // 清理 Agent
        if let Err(e) = self.cleanup_run(run_id).await {
            warn!("Failed to cleanup run {}: {}", run_id, e);
        }

        // 生成报告
        let report = self.build_report(run_id, start_time.elapsed()).await?;

        info!(
            "Run {} execution finished: {} completed, {} failed, {} skipped in {}s",
            run_id,
            report.completed,
            report.failed,
            report.skipped,
            report.duration_secs
        );

        Ok(report)
    }

    /// 执行单个任务
    async fn execute_task(&self, run_id: &str, task_id: &str) -> Result<TaskExecutionResult> {
        debug!("Executing task {} for run {}", task_id, run_id);

        // 获取任务信息
        let (task, command) = {
            let scheduler = self.scheduler.read().await;
            let run = scheduler
                .get_run(run_id)
                .ok_or_else(|| CisError::scheduler("Run not found"))?;
            let task = run
                .dag
                .get_node(task_id)
                .ok_or_else(|| CisError::scheduler("Task not found"))?
                .clone();
            let command = run
                .task_commands
                .get(task_id)
                .cloned()
                .unwrap_or_else(|| format!("Execute task: {}", task_id));
            (task, command)
        };

        // 获取或创建 Agent
        let agent = self.get_or_create_agent(run_id, &task).await?;

        // 构建任务请求
        let prompt = self.build_task_prompt(&task, &command).await?;
        let context = if self.config.enable_context_injection {
            self.build_context(run_id, &task).await?
        } else {
            String::new()
        };

        let work_dir = std::env::current_dir().ok();

        let request = TaskRequest {
            task_id: task_id.to_string(),
            prompt,
            work_dir,
            files: Vec::new(),
            context: {
                let mut ctx = HashMap::new();
                if !context.is_empty() {
                    ctx.insert("upstream_context".to_string(), serde_json::json!(context));
                }
                ctx
            },
            timeout_secs: Some(self.config.task_timeout.as_secs()),
        };

        // 执行任务
        let result = tokio::time::timeout(self.config.task_timeout, agent.execute(request)).await;

        let task_result = match result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                // 释放 Agent（不保留）
                let _ = self.agent_pool.release(agent, false).await;
                return Err(CisError::execution(format!("Task execution failed: {}", e)));
            }
            Err(_) => {
                // 超时
                let _ = self.agent_pool.release(agent, false).await;
                return Err(CisError::execution("Task execution timeout".to_string()));
            }
        };

        // 释放 Agent（根据 keep_agent 配置）
        let keep = task.keep_agent;
        if let Err(e) = self.agent_pool.release(agent.clone(), keep).await {
            warn!("Failed to release agent: {}", e);
        }

        // 如果从池中移除了 Agent，也从 run_agents 中移除
        if !keep {
            let mut run_agents = self.run_agents.write().await;
            if let Some(agents) = run_agents.get_mut(run_id) {
                agents.remove(agent.agent_id());
            }
        }

        Ok(TaskExecutionResult::from_task_result(
            task_id.to_string(),
            task_result,
        ))
    }

    /// 获取或创建 Agent
    async fn get_or_create_agent(&self, run_id: &str, task: &DagNode) -> Result<AgentHandle> {
        // 1. 检查是否复用已有 Agent
        if let Some(ref reuse_agent_id) = task.reuse_agent {
            // 检查本 run 中是否已有此 Agent
            let run_agents = self.run_agents.read().await;
            if let Some(agents) = run_agents.get(run_id) {
                if let Some(agent) = agents.get(reuse_agent_id) {
                    debug!("Reusing agent {} from current run", reuse_agent_id);
                    return Ok(agent.clone());
                }
            }
            drop(run_agents);

            // 检查全局 Pool 中是否有此 Agent
            if let Some(agent) = self.agent_pool.get(reuse_agent_id).await? {
                debug!("Reusing agent {} from global pool", reuse_agent_id);
                // 添加到本 run 的 Agent 表
                let mut run_agents = self.run_agents.write().await;
                run_agents
                    .entry(run_id.to_string())
                    .or_default()
                    .insert(reuse_agent_id.clone(), agent.clone());
                return Ok(agent);
            }

            warn!(
                "Requested agent {} not found, will create new one",
                reuse_agent_id
            );
        }

        // 2. 从 Pool 获取新 Agent
        let runtime_type = task
            .agent_runtime
            .map(to_persistent_runtime_type)
            .unwrap_or(self.config.default_runtime);
        
        // 将 agent::AgentConfig 转换为 persistent::AgentConfig
        let persistent_agent_config = task.agent_config.as_ref().map(|cfg| {
            crate::agent::persistent::AgentConfig::new(
                format!("agent-{}", task.task_id),
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            )
            .with_model(cfg.model.clone().unwrap_or_default())
            .with_timeout(cfg.timeout_secs.unwrap_or(300))
        });
        
        let acquire_config = AgentAcquireConfig {
            runtime_type,
            reuse_agent_id: None,
            agent_config: persistent_agent_config,
            timeout: Some(self.config.task_timeout),
        };

        let agent = self.agent_pool.acquire(acquire_config).await?;
        let agent_id = agent.agent_id().to_string();

        info!(
            "Acquired new agent {} (runtime: {:?}) for run {}",
            agent_id, runtime_type, run_id
        );

        // 3. 记录到本 run 的 Agent 表
        {
            let mut run_agents = self.run_agents.write().await;
            run_agents
                .entry(run_id.to_string())
                .or_default()
                .insert(agent_id, agent.clone());
        }

        Ok(agent)
    }

    /// 构建任务提示
    async fn build_task_prompt(&self, _task: &DagNode, command: &str) -> Result<String> {
        // 可以使用 task 的 agent_config 中的 system_prompt 或直接使用 command
        Ok(command.to_string())
    }

    /// 构建上下文（上游依赖输出）
    async fn build_context(&self, run_id: &str, task: &DagNode) -> Result<String> {
        if task.dependencies.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::new();
        context.push_str(&format!("\n## Upstream Task Outputs for {}\n\n", task.task_id));

        for dep_id in &task.dependencies {
            // 获取依赖任务的输出
            match self.context_store.load(run_id, dep_id).await {
                Ok(output) => {
                    context.push_str(&format!("### Output from {}\n\n", dep_id));
                    context.push_str(&format_output(&output));
                    context.push_str("\n---\n\n");
                }
                Err(e) => {
                    warn!("Failed to load output for task {}: {}", dep_id, e);
                }
            }
        }

        Ok(context)
    }

    /// 更新任务结果
    async fn update_task_result(
        &self,
        run_id: &str,
        task_id: &str,
        result: TaskExecutionResult,
    ) -> Result<()> {
        // 保存到上下文存储
        self.context_store
            .save(run_id, task_id, &result.output, Some(result.exit_code))
            .await?;

        // 更新 DAG 状态
        let mut scheduler = self.scheduler.write().await;
        let run = scheduler
            .get_run_mut(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;

        if result.success {
            if let Err(e) = run.dag.mark_completed(task_id.to_string()) {
                warn!("Failed to mark task {} as completed: {}", task_id, e);
            } else {
                info!("Task {} completed successfully", task_id);
            }
        } else {
            if let Err(e) = run.dag.mark_failed(task_id.to_string()) {
                warn!("Failed to mark task {} as failed: {}", task_id, e);
            } else {
                warn!("Task {} failed", task_id);
            }
        }

        run.update_status();

        Ok(())
    }

    /// 标记任务失败
    async fn mark_task_failed(&self, run_id: &str, task_id: &str, error: String) -> Result<()> {
        self.context_store.save(run_id, task_id, &error, Some(1)).await?;

        let mut scheduler = self.scheduler.write().await;
        let run = scheduler
            .get_run_mut(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;
        if let Err(e) = run.dag.mark_failed(task_id.to_string()) {
            warn!("Failed to mark task {} as failed: {}", task_id, e);
        }
        run.update_status();

        warn!("Marked task {} as failed: {}", task_id, error);

        Ok(())
    }

    /// 清理运行的 Agent
    async fn cleanup_run(&self, run_id: &str) -> Result<()> {
        info!("Cleaning up agents for run {}", run_id);

        let mut run_agents = self.run_agents.write().await;

        if let Some(agents) = run_agents.remove(run_id) {
            for (agent_id, _handle) in agents {
                // 检查是否需要销毁
                // 如果 keep_agent 为 false，或者 auto_cleanup 为 true，则销毁
                if self.config.auto_cleanup {
                    if let Err(e) = self.agent_pool.kill(&agent_id).await {
                        warn!("Failed to kill agent {}: {}", agent_id, e);
                    } else {
                        debug!("Killed agent {}", agent_id);
                    }
                }
            }
        }

        // 清理上下文存储缓存
        self.context_store.clear_run_cache(run_id);

        Ok(())
    }

    /// 构建执行报告
    async fn build_report(
        &self,
        run_id: &str,
        duration: Duration,
    ) -> Result<MultiAgentExecutionReport> {
        let scheduler = self.scheduler.read().await;
        let run = scheduler
            .get_run(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;

        let mut completed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut task_outputs = HashMap::new();

        for (task_id, node) in run.dag.nodes() {
            match node.status {
                DagNodeStatus::Completed => {
                    completed += 1;
                    // 加载输出
                    if let Ok(output) = self.context_store.load(run_id, task_id).await {
                        task_outputs.insert(
                            task_id.clone(),
                            TaskExecutionResult {
                                task_id: task_id.clone(),
                                success: true,
                                output,
                                exit_code: 0,
                                metadata: HashMap::new(),
                            },
                        );
                    }
                }
                DagNodeStatus::Failed => {
                    failed += 1;
                    if let Ok(output) = self.context_store.load(run_id, task_id).await {
                        task_outputs.insert(
                            task_id.clone(),
                            TaskExecutionResult {
                                task_id: task_id.clone(),
                                success: false,
                                output,
                                exit_code: 1,
                                metadata: HashMap::new(),
                            },
                        );
                    }
                }
                DagNodeStatus::Skipped => {
                    skipped += 1;
                }
                _ => {}
            }
        }

        let final_status = if failed > 0 {
            "failed"
        } else if skipped > 0 && completed == 0 {
            "skipped"
        } else {
            "success"
        }
        .to_string();

        Ok(MultiAgentExecutionReport {
            run_id: run_id.to_string(),
            duration_secs: duration.as_secs(),
            completed,
            failed,
            skipped,
            final_status,
            task_outputs,
        })
    }

    /// 计算当前运行的 Agent 数量
    async fn count_active_agents(&self, run_id: &str) -> usize {
        let run_agents = self.run_agents.read().await;
        run_agents
            .get(run_id)
            .map(|agents| agents.len())
            .unwrap_or(0)
    }

    /// 获取运行状态
    pub async fn get_run_status(
        &self,
        run_id: &str,
    ) -> Result<Option<crate::scheduler::DagRunStatus>> {
        let scheduler = self.scheduler.read().await;
        Ok(scheduler.get_run(run_id).map(|run| run.status))
    }

    /// 获取运行统计信息
    pub async fn get_run_stats(&self, run_id: &str) -> Result<(usize, usize, usize)> {
        let scheduler = self.scheduler.read().await;
        let run = scheduler
            .get_run(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;

        let mut completed = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for node in run.dag.nodes().values() {
            match node.status {
                DagNodeStatus::Completed => completed += 1,
                DagNodeStatus::Failed => failed += 1,
                DagNodeStatus::Skipped => skipped += 1,
                _ => {}
            }
        }

        Ok((completed, failed, skipped))
    }

    /// 获取 Agent 池引用
    pub fn agent_pool(&self) -> &AgentPool {
        &self.agent_pool
    }

    /// 获取上下文存储
    pub fn context_store(&self) -> &ContextStore {
        &self.context_store
    }

    /// 获取配置
    pub fn config(&self) -> &MultiAgentExecutorConfig {
        &self.config
    }

    /// 取消运行
    pub async fn cancel_run(&self, run_id: &str) -> Result<()> {
        info!("Cancelling run {}", run_id);
        self.cleanup_run(run_id).await
    }

    /// Clone references for spawned tasks
    fn clone_ref(&self) -> Self {
        Self {
            scheduler: self.scheduler.clone(),
            agent_pool: self.agent_pool.clone(),
            context_store: self.context_store.clone(),
            config: self.config.clone(),
            run_agents: self.run_agents.clone(),
        }
    }
}

/// 格式化输出（截断如果太长）
fn format_output(output: &str) -> String {
    const MAX_LEN: usize = 10000;

    if output.len() > MAX_LEN {
        format!(
            "{}\n\n[... truncated, total length: {} characters ...]",
            &output[..MAX_LEN],
            output.len()
        )
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_agent_executor_config_default() {
        let config = MultiAgentExecutorConfig::default();
        assert_eq!(config.scheduling_mode, SchedulingMode::EventDriven);
        assert_eq!(config.default_runtime, AgentRuntimeType::Claude);
        assert!(config.auto_cleanup);
        assert_eq!(config.task_timeout, Duration::from_secs(300));
        assert!(config.enable_context_injection);
        assert_eq!(config.max_concurrent_tasks, 4);
    }

    #[test]
    fn test_multi_agent_executor_config_builder() {
        let config = MultiAgentExecutorConfig::new()
            .with_scheduling_mode(SchedulingMode::Polling)
            .with_default_runtime(AgentRuntimeType::Kimi)
            .with_auto_cleanup(false)
            .with_task_timeout(Duration::from_secs(600))
            .with_context_injection(false)
            .with_max_concurrent(8);

        assert_eq!(config.scheduling_mode, SchedulingMode::Polling);
        assert_eq!(config.default_runtime, AgentRuntimeType::Kimi);
        assert!(!config.auto_cleanup);
        assert_eq!(config.task_timeout, Duration::from_secs(600));
        assert!(!config.enable_context_injection);
        assert_eq!(config.max_concurrent_tasks, 8);
    }

    #[test]
    fn test_scheduling_mode_default() {
        let mode = SchedulingMode::default();
        assert_eq!(mode, SchedulingMode::EventDriven);
    }

    #[test]
    fn test_task_execution_result() {
        let result = TaskExecutionResult {
            task_id: "task-1".to_string(),
            success: true,
            output: "done".to_string(),
            exit_code: 0,
            metadata: HashMap::new(),
        };

        assert_eq!(result.task_id, "task-1");
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_format_output() {
        let short = "short output";
        assert_eq!(format_output(short), short);

        let long = "a".repeat(20000);
        let formatted = format_output(&long);
        assert!(formatted.contains("truncated"));
        assert!(formatted.len() < long.len());
    }

    #[test]
    fn test_runtime_type_conversion() {
        // Test scheduler -> persistent
        assert_eq!(
            to_persistent_runtime_type(RuntimeType::Claude),
            AgentRuntimeType::Claude
        );
        assert_eq!(
            to_persistent_runtime_type(RuntimeType::Kimi),
            AgentRuntimeType::Kimi
        );
        assert_eq!(
            to_persistent_runtime_type(RuntimeType::Default),
            AgentRuntimeType::Claude
        );

        // Test persistent -> scheduler
        assert_eq!(
            to_scheduler_runtime_type(AgentRuntimeType::Claude),
            RuntimeType::Claude
        );
        assert_eq!(
            to_scheduler_runtime_type(AgentRuntimeType::OpenCode),
            RuntimeType::OpenCode
        );
    }
}
