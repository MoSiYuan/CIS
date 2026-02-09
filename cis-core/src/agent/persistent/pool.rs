//! # Agent Pool
//!
//! 管理 PersistentAgent 实例的池，支持动态创建、复用和生命周期管理。
//!
//! 主要特性：
//! - 多 Runtime 支持（Claude, OpenCode, Kimi, Aider）
//! - Agent 复用和池化
//! - 自动健康检查和清理
//! - 优雅关闭管理

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::agent::persistent::{
    AgentConfig, AgentInfo, AgentRuntime, AgentStatus, PersistentAgent, RuntimeType,
    TaskRequest, TaskResult,
};
use crate::error::{CisError, Result};

/// Pool 配置
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// 最大 Agent 数
    pub max_agents: usize,
    /// 默认超时时间
    pub default_timeout: Duration,
    /// 健康检查间隔
    pub health_check_interval: Duration,
    /// 自动清理空闲 Agent
    pub auto_cleanup: bool,
    /// 空闲超时时间
    pub idle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_agents: 10,
            default_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
            auto_cleanup: true,
            idle_timeout: Duration::from_secs(600),
        }
    }
}

/// Agent 获取配置
#[derive(Debug, Clone)]
pub struct AgentAcquireConfig {
    /// 指定 Runtime 类型
    pub runtime_type: RuntimeType,
    /// 复用已有 Agent ID
    pub reuse_agent_id: Option<String>,
    /// Agent 配置（创建新 Agent 时用）
    pub agent_config: Option<AgentConfig>,
    /// 超时时间
    pub timeout: Option<Duration>,
}

impl AgentAcquireConfig {
    /// 创建新的获取配置
    pub fn new(runtime_type: RuntimeType) -> Self {
        Self {
            runtime_type,
            reuse_agent_id: None,
            agent_config: None,
            timeout: None,
        }
    }

    /// 指定复用的 Agent ID
    pub fn with_reuse_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.reuse_agent_id = Some(agent_id.into());
        self
    }

    /// 设置 Agent 配置
    pub fn with_agent_config(mut self, config: AgentConfig) -> Self {
        self.agent_config = Some(config);
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Agent 句柄
///
/// 提供对池中 Agent 的安全访问。当句柄被释放时，
/// Agent 可以选择保留在池中或立即关闭。
#[derive(Clone)]
pub struct AgentHandle {
    agent_id: String,
    agents: Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>>,
    agent_info: Arc<RwLock<HashMap<String, AgentInfo>>>,
}

impl AgentHandle {
    /// 创建新的 Agent 句柄
    fn new(
        agent_id: String,
        agents: Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>>,
        agent_info: Arc<RwLock<HashMap<String, AgentInfo>>>,
    ) -> Self {
        Self {
            agent_id,
            agents,
            agent_info,
        }
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// 执行任务
    pub async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        let agents = self.agents.read().await;
        let agent = agents
            .get(&self.agent_id)
            .ok_or_else(|| CisError::not_found(format!("Agent {} not found in pool", self.agent_id)))?;

        // 更新 Agent 信息为 Busy 状态
        {
            let mut info = self.agent_info.write().await;
            if let Some(agent_info) = info.get_mut(&self.agent_id) {
                agent_info.status = AgentStatus::Busy;
                agent_info.current_task = Some(task.task_id.clone());
            }
        }

        let result = agent.execute(task).await;

        // 更新 Agent 信息为 Idle 状态
        {
            let mut info = self.agent_info.write().await;
            if let Some(agent_info) = info.get_mut(&self.agent_id) {
                agent_info.status = AgentStatus::Idle;
                agent_info.current_task = None;
                agent_info.last_active_at = Utc::now();
                if result.is_ok() {
                    agent_info.total_tasks += 1;
                }
            }
        }

        result
    }

    /// 获取 Agent 状态
    pub async fn status(&self) -> Result<AgentStatus> {
        let agents = self.agents.read().await;
        let agent = agents
            .get(&self.agent_id)
            .ok_or_else(|| CisError::not_found(format!("Agent {} not found in pool", self.agent_id)))?;

        Ok(agent.status().await)
    }

    /// Attach 到 Agent（进入交互式模式）
    pub async fn attach(&self) -> Result<()> {
        let agents = self.agents.read().await;
        let agent = agents
            .get(&self.agent_id)
            .ok_or_else(|| CisError::not_found(format!("Agent {} not found in pool", self.agent_id)))?;

        agent.attach().await
    }
}

impl std::fmt::Debug for AgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("agent_id", &self.agent_id)
            .finish()
    }
}

/// Agent Pool
///
/// 管理多个 PersistentAgent 实例的池，支持：
/// - 多种 Runtime 类型注册
/// - Agent 复用和生命周期管理
/// - 自动健康检查
/// - 空闲清理
pub struct AgentPool {
    /// Runtime 注册表（使用 Arc 以便共享）
    runtimes: RwLock<HashMap<RuntimeType, Arc<dyn AgentRuntime>>>,
    /// 持久化 Agent 实例表
    agents: Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>>,
    /// Agent 元信息表
    agent_info: Arc<RwLock<HashMap<String, AgentInfo>>>,
    /// 配置
    config: PoolConfig,
    /// 健康检查任务句柄
    health_check_handle: RwLock<Option<JoinHandle<()>>>,
    /// 关闭信号发送端
    shutdown_tx: RwLock<Option<tokio::sync::mpsc::Sender<()>>>,
}

impl std::fmt::Debug for AgentPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentPool")
            .field("config", &self.config)
            .field("agents", &self.agents.blocking_read().len())
            .field("agent_info", &self.agent_info.blocking_read().len())
            .finish_non_exhaustive()
    }
}

impl Clone for AgentPool {
    fn clone(&self) -> Self {
        Self {
            runtimes: RwLock::new(HashMap::new()), // 新实例需要重新注册 runtime
            agents: Arc::clone(&self.agents),
            agent_info: Arc::clone(&self.agent_info),
            config: self.config.clone(),
            health_check_handle: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
        }
    }
}

impl AgentPool {
    /// 创建新的 Pool
    pub fn new(config: PoolConfig) -> Self {
        Self {
            runtimes: RwLock::new(HashMap::new()),
            agents: Arc::new(RwLock::new(HashMap::new())),
            agent_info: Arc::new(RwLock::new(HashMap::new())),
            config,
            health_check_handle: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
        }
    }

    /// 注册 Runtime
    pub async fn register_runtime(&self, runtime: Arc<dyn AgentRuntime>) -> Result<()> {
        let runtime_type = runtime.runtime_type();
        info!("Registering runtime: {:?}", runtime_type);

        let mut runtimes = self.runtimes.write().await;
        if runtimes.contains_key(&runtime_type) {
            return Err(CisError::already_exists(format!(
                "Runtime {:?} already registered",
                runtime_type
            )));
        }

        runtimes.insert(runtime_type, runtime);
        debug!("Runtime {:?} registered successfully", runtime_type);

        Ok(())
    }

    /// 获取或创建 Agent（核心方法）
    pub async fn acquire(&self, config: AgentAcquireConfig) -> Result<AgentHandle> {
        let _timeout = config.timeout.unwrap_or(self.config.default_timeout);
        let runtime_type = config.runtime_type;

        // 如果指定了复用 Agent ID，尝试获取已有 Agent
        if let Some(ref agent_id) = config.reuse_agent_id {
            let agents = self.agents.read().await;
            if agents.contains_key(agent_id) {
                debug!("Reusing existing agent: {}", agent_id);
                return Ok(AgentHandle::new(
                    agent_id.clone(),
                    self.agents.clone(),
                    self.agent_info.clone(),
                ));
            }
            warn!(
                "Requested agent {} not found, will create new one",
                agent_id
            );
        }

        // 检查 Agent 数量限制
        {
            let agents = self.agents.read().await;
            if agents.len() >= self.config.max_agents {
                return Err(CisError::execution(format!(
                    "Agent pool limit reached ({}/{})",
                    agents.len(),
                    self.config.max_agents
                )));
            }
        }

        // 获取 Runtime
        let runtime = {
            let runtimes = self.runtimes.read().await;
            runtimes
                .get(&runtime_type)
                .ok_or_else(|| {
                    CisError::invalid_input(format!("Runtime {:?} not registered", runtime_type))
                })?
                .clone()
        };

        // 创建新 Agent
        let agent_config = config.agent_config.unwrap_or_else(|| {
            AgentConfig::new(
                format!("agent-{}", uuid::Uuid::new_v4()),
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            )
        });

        let agent = runtime.create_agent(agent_config.clone()).await?;
        let agent_id = agent.agent_id().to_string();

        info!("Created new agent: {} (runtime: {:?})", agent_id, runtime_type);

        // 创建 Agent 信息
        let agent_info = AgentInfo::new(
            &agent_id,
            &agent_config.name,
            agent_config.work_dir.clone(),
        )
        .with_runtime_type(runtime_type)
        .with_status(AgentStatus::Idle);

        // 存储 Agent
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), agent);
        }

        // 存储 Agent 信息
        {
            let mut info = self.agent_info.write().await;
            info.insert(agent_id.clone(), agent_info);
        }

        Ok(AgentHandle::new(
            agent_id,
            self.agents.clone(),
            self.agent_info.clone(),
        ))
    }

    /// 释放 Agent
    ///
    /// # Arguments
    /// * `handle` - Agent 句柄
    /// * `keep` - 是否保留在池中（true=保留，false=立即关闭）
    pub async fn release(&self, handle: AgentHandle, keep: bool) -> Result<()> {
        let agent_id = handle.agent_id().to_string();
        debug!("Releasing agent: {} (keep={})", agent_id, keep);

        if !keep {
            // 从池中移除并关闭 Agent
            let mut agents = self.agents.write().await;
            let mut info = self.agent_info.write().await;

            if let Some(agent) = agents.remove(&agent_id) {
                info!("Shutting down agent: {}", agent_id);
                if let Err(e) = agent.shutdown().await {
                    warn!("Failed to shutdown agent {}: {}", agent_id, e);
                }
            }

            info.remove(&agent_id);
            debug!("Agent {} removed from pool", agent_id);
        } else {
            // 更新最后活动时间
            let mut info = self.agent_info.write().await;
            if let Some(agent_info) = info.get_mut(&agent_id) {
                agent_info.last_active_at = Utc::now();
                agent_info.status = AgentStatus::Idle;
            }
            debug!("Agent {} kept in pool", agent_id);
        }

        Ok(())
    }

    /// 获取指定 Agent
    pub async fn get(&self, agent_id: &str) -> Result<Option<AgentHandle>> {
        let agents = self.agents.read().await;

        if agents.contains_key(agent_id) {
            Ok(Some(AgentHandle::new(
                agent_id.to_string(),
                self.agents.clone(),
                self.agent_info.clone(),
            )))
        } else {
            Ok(None)
        }
    }

    /// 列出所有 Agent
    pub async fn list(&self) -> Vec<AgentInfo> {
        let info = self.agent_info.read().await;
        info.values().cloned().collect()
    }

    /// 强制终止 Agent
    pub async fn kill(&self, agent_id: &str) -> Result<()> {
        info!("Killing agent: {}", agent_id);

        let mut agents = self.agents.write().await;
        let mut info = self.agent_info.write().await;

        if let Some(agent) = agents.remove(agent_id) {
            if let Err(e) = agent.shutdown().await {
                warn!("Error during agent {} shutdown: {}", agent_id, e);
            }
            info.remove(agent_id);
            info!("Agent {} killed", agent_id);
            Ok(())
        } else {
            Err(CisError::not_found(format!("Agent {} not found", agent_id)))
        }
    }

    /// 启动健康检查
    pub async fn start_health_check(&self) {
        let agents = self.agents.clone();
        let agent_info = self.agent_info.clone();
        let interval = self.config.health_check_interval;
        let auto_cleanup = self.config.auto_cleanup;
        let idle_timeout = self.config.idle_timeout;

        // 创建关闭信号通道
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let handle = tokio::spawn(async move {
            info!("Agent pool health check started (interval: {:?})", interval);
            let mut tick_interval = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = tick_interval.tick() => {
                        health_check_tick(
                            &agents,
                            &agent_info,
                            auto_cleanup,
                            idle_timeout,
                        ).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Health check task shutting down");
                        break;
                    }
                }
            }
        });

        *self.health_check_handle.write().await = Some(handle);
    }

    /// 停止健康检查
    pub async fn stop_health_check(&self) {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(()).await;
        }

        if let Some(handle) = self.health_check_handle.write().await.take() {
            let _ = handle.await;
        }
    }

    /// 优雅关闭所有 Agent
    pub async fn shutdown_all(&self) -> Result<()> {
        info!("Shutting down all agents in pool...");

        // 停止健康检查
        self.stop_health_check().await;

        let mut agents = self.agents.write().await;
        let mut info = self.agent_info.write().await;

        let agent_ids: Vec<String> = agents.keys().cloned().collect();
        let mut errors = Vec::new();

        for agent_id in agent_ids {
            if let Some(agent) = agents.remove(&agent_id) {
                info!("Shutting down agent: {}", agent_id);
                if let Err(e) = agent.shutdown().await {
                    warn!("Failed to shutdown agent {}: {}", agent_id, e);
                    errors.push(format!("{}: {}", agent_id, e));
                }
            }
        }

        info.clear();

        if errors.is_empty() {
            info!("All agents shutdown successfully");
            Ok(())
        } else {
            Err(CisError::execution(format!(
                "Some agents failed to shutdown: {}",
                errors.join(", ")
            )))
        }
    }

    /// 获取当前 Agent 数量
    pub async fn agent_count(&self) -> usize {
        self.agents.read().await.len()
    }

    /// 获取配置
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }
}

/// 健康检查单次执行
async fn health_check_tick(
    agents: &Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>>,
    agent_info: &Arc<RwLock<HashMap<String, AgentInfo>>>,
    auto_cleanup: bool,
    idle_timeout: Duration,
) {
    debug!("Running health check...");

    let agents_guard = agents.read().await;
    let info_guard = agent_info.read().await;

    let mut to_remove = Vec::new();
    let mut to_update = Vec::new();
    let now = Utc::now();

    for (agent_id, agent) in agents_guard.iter() {
        if let Some(info) = info_guard.get(agent_id) {
            let status = agent.status().await;

            match status {
                AgentStatus::Error | AgentStatus::Shutdown => {
                    warn!(
                        "Agent {} is in {:?} state, will be removed",
                        agent_id, status
                    );
                    to_remove.push(agent_id.clone());
                }
                AgentStatus::Idle => {
                    // 检查空闲超时
                    if auto_cleanup {
                        let idle_duration = now.signed_duration_since(info.last_active_at);
                        if idle_duration.num_seconds() > idle_timeout.as_secs() as i64 {
                            info!(
                                "Agent {} idle timeout ({}s), will be removed",
                                agent_id,
                                idle_duration.num_seconds()
                            );
                            to_remove.push(agent_id.clone());
                        }
                    }
                }
                _ => {
                    // 其他状态正常，更新状态
                    to_update.push((agent_id.clone(), status));
                }
            }
        }
    }

    drop(agents_guard);
    drop(info_guard);

    // 更新 Agent 状态
    {
        let mut info = agent_info.write().await;
        for (agent_id, status) in to_update {
            if let Some(agent_info) = info.get_mut(&agent_id) {
                agent_info.status = status;
            }
        }
    }

    // 清理需要移除的 Agent
    if auto_cleanup && !to_remove.is_empty() {
        let mut agents = agents.write().await;
        let mut info = agent_info.write().await;

        for agent_id in to_remove {
            if let Some(agent) = agents.remove(&agent_id) {
                info!("Auto-removing agent: {}", agent_id);
                if let Err(e) = agent.shutdown().await {
                    warn!("Failed to shutdown agent {} during cleanup: {}", agent_id, e);
                }
                info.remove(&agent_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::PathBuf;

    /// Mock PersistentAgent for testing
    struct MockAgent {
        agent_id: String,
        runtime_type: RuntimeType,
        status: RwLock<AgentStatus>,
    }

    impl MockAgent {
        fn new(agent_id: impl Into<String>, runtime_type: RuntimeType) -> Self {
            Self {
                agent_id: agent_id.into(),
                runtime_type,
                status: RwLock::new(AgentStatus::Idle),
            }
        }
    }

    #[async_trait]
    impl PersistentAgent for MockAgent {
        fn agent_id(&self) -> &str {
            &self.agent_id
        }

        fn runtime_type(&self) -> RuntimeType {
            self.runtime_type
        }

        async fn execute(&self, _task: TaskRequest) -> Result<TaskResult> {
            let mut status = self.status.write().await;
            *status = AgentStatus::Busy;
            
            // Simulate work
            tokio::time::sleep(Duration::from_millis(10)).await;
            
            *status = AgentStatus::Idle;
            
            Ok(TaskResult::success("task-1", "done"))
        }

        async fn status(&self) -> AgentStatus {
            *self.status.read().await
        }

        async fn attach(&self) -> Result<()> {
            Ok(())
        }

        async fn detach(&self) -> Result<()> {
            Ok(())
        }

        async fn shutdown(&self) -> Result<()> {
            let mut status = self.status.write().await;
            *status = AgentStatus::Shutdown;
            Ok(())
        }
    }

    /// Mock AgentRuntime for testing
    struct MockRuntime {
        runtime_type: RuntimeType,
    }

    impl MockRuntime {
        fn new(runtime_type: RuntimeType) -> Self {
            Self { runtime_type }
        }
    }

    #[async_trait]
    impl AgentRuntime for MockRuntime {
        fn runtime_type(&self) -> RuntimeType {
            self.runtime_type
        }

        async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>> {
            let agent = MockAgent::new(
                format!("{}-{}", self.runtime_type.command_name(), config.name),
                self.runtime_type,
            );
            Ok(Box::new(agent))
        }

        async fn list_agents(&self) -> Vec<AgentInfo> {
            Vec::new()
        }
    }

    fn create_test_config() -> PoolConfig {
        PoolConfig {
            max_agents: 5,
            default_timeout: Duration::from_secs(60),
            health_check_interval: Duration::from_millis(100),
            auto_cleanup: false,
            idle_timeout: Duration::from_secs(1),
        }
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let config = create_test_config();
        let pool = AgentPool::new(config);

        assert_eq!(pool.agent_count().await, 0);
        assert_eq!(pool.config().max_agents, 5);
    }

    #[tokio::test]
    async fn test_register_runtime() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));

        pool.register_runtime(runtime).await.unwrap();

        // 重复注册应该失败
        let runtime2 = Arc::new(MockRuntime::new(RuntimeType::Claude));
        let result = pool.register_runtime(runtime2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_acquire_and_release() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        // Acquire agent
        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();

        assert_eq!(pool.agent_count().await, 1);

        // Release with keep=true
        pool.release(handle.clone(), true).await.unwrap();
        assert_eq!(pool.agent_count().await, 1);

        // Release with keep=false
        pool.release(handle, false).await.unwrap();
        assert_eq!(pool.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_agent() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();
        let agent_id = handle.agent_id().to_string();

        // Get existing agent
        let found = pool.get(&agent_id).await.unwrap();
        assert!(found.is_some());

        // Get non-existent agent
        let not_found = pool.get("non-existent").await.unwrap();
        assert!(not_found.is_none());

        // Clean up
        pool.release(handle, false).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_agents() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let list = pool.list().await;
        assert!(list.is_empty());

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();

        let list = pool.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, handle.agent_id());

        pool.release(handle, false).await.unwrap();
    }

    #[tokio::test]
    async fn test_kill_agent() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();
        let agent_id = handle.agent_id().to_string();

        assert_eq!(pool.agent_count().await, 1);

        // Kill the agent
        pool.kill(&agent_id).await.unwrap();
        assert_eq!(pool.agent_count().await, 0);

        // Killing non-existent agent should fail
        let result = pool.kill(&agent_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_acquire_without_runtime() {
        let pool = AgentPool::new(create_test_config());

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let result = pool.acquire(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pool_limit() {
        let config = PoolConfig {
            max_agents: 2,
            ..create_test_config()
        };
        let pool = AgentPool::new(config);
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        // Acquire max agents
        let handle1 = pool
            .acquire(AgentAcquireConfig::new(RuntimeType::Claude))
            .await
            .unwrap();
        let handle2 = pool
            .acquire(AgentAcquireConfig::new(RuntimeType::Claude))
            .await
            .unwrap();

        assert_eq!(pool.agent_count().await, 2);

        // Try to acquire one more (should fail)
        let result = pool.acquire(AgentAcquireConfig::new(RuntimeType::Claude)).await;
        assert!(result.is_err());

        // Clean up
        pool.release(handle1, false).await.unwrap();
        pool.release(handle2, false).await.unwrap();
    }



    #[tokio::test]
    async fn test_reuse_agent() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();
        let agent_id = handle.agent_id().to_string();

        // Release with keep=true
        pool.release(handle, true).await.unwrap();

        // Reuse the same agent
        let reuse_config = AgentAcquireConfig::new(RuntimeType::Claude)
            .with_reuse_agent_id(&agent_id);
        let handle2 = pool.acquire(reuse_config).await.unwrap();

        assert_eq!(handle2.agent_id(), agent_id);
        assert_eq!(pool.agent_count().await, 1);

        pool.release(handle2, false).await.unwrap();
    }

    #[tokio::test]
    async fn test_shutdown_all() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        // Create multiple agents
        let handle1 = pool
            .acquire(AgentAcquireConfig::new(RuntimeType::Claude))
            .await
            .unwrap();
        let handle2 = pool
            .acquire(AgentAcquireConfig::new(RuntimeType::Claude))
            .await
            .unwrap();

        // Release them to pool
        pool.release(handle1, true).await.unwrap();
        pool.release(handle2, true).await.unwrap();

        assert_eq!(pool.agent_count().await, 2);

        // Shutdown all
        pool.shutdown_all().await.unwrap();
        assert_eq!(pool.agent_count().await, 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        // This test verifies that health check task can be started and stopped
        // For a full test of cleanup behavior, integration tests would be more reliable
        let config = PoolConfig {
            auto_cleanup: false, // Disable auto cleanup for this test
            health_check_interval: Duration::from_millis(10),
            ..create_test_config()
        };
        let pool = AgentPool::new(config);
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        // Start health check
        pool.start_health_check().await;

        // Create an agent
        let handle = pool
            .acquire(AgentAcquireConfig::new(RuntimeType::Claude))
            .await
            .unwrap();
        pool.release(handle, true).await.unwrap();

        // Verify agent exists
        assert_eq!(pool.agent_count().await, 1);

        // Wait a bit for health check to run
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Agent should still exist (auto_cleanup is disabled)
        assert_eq!(pool.agent_count().await, 1);

        // Stop health check
        pool.stop_health_check().await;

        // Clean up
        pool.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_health_check_cleanup() {
        // Test health check cleanup logic directly
        use chrono::Duration as ChronoDuration;

        let agents: Arc<RwLock<HashMap<String, Box<dyn PersistentAgent>>>> = 
            Arc::new(RwLock::new(HashMap::new()));
        let agent_info: Arc<RwLock<HashMap<String, AgentInfo>>> = 
            Arc::new(RwLock::new(HashMap::new()));

        // Create a mock agent that will be marked as idle for a long time
        let mock_agent = MockAgent::new("old-agent", RuntimeType::Claude);
        let old_time = Utc::now() - ChronoDuration::seconds(1000);
        
        {
            let mut agents_guard = agents.write().await;
            agents_guard.insert("old-agent".to_string(), Box::new(mock_agent));
        }

        {
            let mut info_guard = agent_info.write().await;
            let mut info = AgentInfo::new("old-agent", "test", PathBuf::from("/tmp"));
            info.status = AgentStatus::Idle;
            info.last_active_at = old_time;
            info_guard.insert("old-agent".to_string(), info);
        }

        // Run health check tick with auto_cleanup enabled
        health_check_tick(
            &agents,
            &agent_info,
            true,
            Duration::from_secs(60),
        ).await;

        // Agent should be removed
        let agents_guard = agents.read().await;
        assert!(!agents_guard.contains_key("old-agent"));
    }

    #[tokio::test]
    async fn test_concurrent_acquire() {
        let pool = Arc::new(AgentPool::new(PoolConfig {
            max_agents: 10,
            ..create_test_config()
        }));
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let mut handles = Vec::new();

        // Spawn multiple concurrent acquires
        for _ in 0..5 {
            let pool = pool.clone();
            let handle = tokio::spawn(async move {
                let config = AgentAcquireConfig::new(RuntimeType::Claude);
                pool.acquire(config).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        // All should succeed
        for result in &results {
            assert!(result.is_ok(), "Failed to acquire agent: {:?}", result);
        }

        assert_eq!(pool.agent_count().await, 5);

        // Clean up
        for result in results {
            if let Ok(agent_handle) = result {
                pool.release(agent_handle, false).await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn test_agent_handle_execute() {
        let pool = AgentPool::new(create_test_config());
        let runtime = Arc::new(MockRuntime::new(RuntimeType::Claude));
        pool.register_runtime(runtime).await.unwrap();

        let config = AgentAcquireConfig::new(RuntimeType::Claude);
        let handle = pool.acquire(config).await.unwrap();

        // Execute a task
        let task = TaskRequest::new("task-1", "Test task");
        let result = handle.execute(task).await.unwrap();

        assert!(result.success);
        assert_eq!(result.task_id, "task-1");

        // Check status
        let status = handle.status().await.unwrap();
        assert_eq!(status, AgentStatus::Idle);

        pool.release(handle, false).await.unwrap();
    }
}
