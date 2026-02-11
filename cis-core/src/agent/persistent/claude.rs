//! # Claude Persistent Agent
//!
//! 基于 PTY 会话实现的 Claude Code 持久化 Agent。
//!
//! ## 功能特性
//! - 通过 SessionManager 管理 PTY 会话
//! - 支持 attach/detach 交互式会话
//! - 完整的生命周期管理（启动、停止、连接、恢复）
//! - 通过事件订阅机制等待任务完成
//!
//! ## 使用示例
//! ```rust,no_run
//! use cis_core::agent::persistent::claude::{ClaudePersistentAgent, ClaudeRuntime};
//! use cis_core::agent::persistent::{AgentConfig, AgentRuntime};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 启动新的 Claude Agent
//! let config = AgentConfig::new("my-claude-agent", std::path::PathBuf::from("/work"));
//! let runtime = ClaudeRuntime::new();
//! let agent = runtime.create_agent(config).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::agent::persistent::{
    AgentConfig, AgentInfo, AgentRuntime, AgentStatus, PersistentAgent, RuntimeType, TaskRequest,
    TaskResult,
};
use crate::agent::cluster::{SessionManager, SessionId};
use crate::agent::cluster::events::{SessionEvent, SessionState};
use crate::agent::AgentType;
use crate::error::{CisError, Result};

/// Claude 持久化 Agent 实现
///
/// 通过 PTY 与 Claude Code 进程通信，支持持久化运行和前后台切换。
pub struct ClaudePersistentAgent {
    /// Session ID (同时也是 Agent ID)
    session_id: SessionId,
    /// SessionManager 引用
    session_manager: &'static SessionManager,
    /// Agent 状态
    state: Arc<RwLock<ClaudeAgentState>>,
}

/// Claude Agent 内部状态
#[derive(Debug, Clone)]
struct ClaudeAgentState {
    /// 当前状态
    status: AgentStatus,
    /// 当前任务 ID（如果有）
    current_task: Option<String>,
    /// 已完成任务数
    completed_tasks: u64,
    /// 总输出字符数
    total_output_chars: u64,
}

/// Claude Agent 统计信息
#[derive(Debug, Clone)]
pub struct ClaudeAgentStats {
    /// 已完成任务数
    pub completed_tasks: u64,
    /// 总输出字符数
    pub total_output_chars: u64,
    /// 当前任务 ID（如果有）
    pub current_task: Option<String>,
}

impl ClaudePersistentAgent {
    /// 创建新的持久化 Claude Agent
    ///
    /// # Arguments
    /// * `manager` - SessionManager 引用
    /// * `config` - Agent 配置
    ///
    /// # Returns
    /// 新创建的 ClaudePersistentAgent 实例
    pub async fn start(
        manager: &'static SessionManager,
        config: AgentConfig,
    ) -> Result<Self> {
        let agent_name = config.name.clone();
        let work_dir = config.work_dir.clone();
        let system_prompt = config.system_prompt.clone().unwrap_or_default();
        
        info!("Starting Claude persistent agent: {}", agent_name);

        // 生成唯一的 session ID（使用 persistent 作为 run_id 前缀）
        let unique_id = format!("{}-{}", agent_name, uuid::Uuid::new_v4().to_string()[..8].to_string());
        let session_id = SessionId::new("persistent", &unique_id);

        // 检查 claude 命令是否可用
        match which::which("claude") {
            Ok(path) => debug!("Found claude at: {:?}", path),
            Err(_) => {
                return Err(CisError::execution(
                    "claude command not found. Please install Claude Code first.",
                ));
            }
        }

        // 构建初始提示词（包含系统提示词）
        let initial_prompt = if system_prompt.is_empty() {
            "You are Claude, a helpful AI assistant.".to_string()
        } else {
            system_prompt
        };

        // 创建 session（这会启动 claude 进程）
        manager.create_session(
            "persistent",
            &unique_id,
            AgentType::Claude,
            &initial_prompt,
            &work_dir,
            "", // no upstream context
        ).await?;

        // 获取 session 并设置为持久化模式
        if let Some(session_arc) = manager.get_session(&session_id).await {
            let mut session = session_arc.write().await;
            session.set_persistent(true);
            // 设置空闲超时为 1 小时
            session.set_max_idle_secs(3600);
            info!("Claude session {} created with persistent mode enabled", session_id.short());
        }

        info!("Claude persistent agent started: {}", agent_name);

        Ok(Self {
            session_id,
            session_manager: manager,
            state: Arc::new(RwLock::new(ClaudeAgentState {
                status: AgentStatus::Running,
                current_task: None,
                completed_tasks: 0,
                total_output_chars: 0,
            })),
        })
    }

    /// 连接到已有的 Session
    ///
    /// # Arguments
    /// * `manager` - SessionManager 引用
    /// * `session_id` - 要连接的 Session ID
    ///
    /// # Returns
    /// 新创建的 ClaudePersistentAgent 实例
    pub async fn attach_to_session(
        manager: &'static SessionManager,
        session_id: SessionId,
    ) -> Result<Self> {
        info!("Attaching to existing Claude session: {}", session_id.short());

        // 验证 session 存在
        let session_arc = manager.get_session(&session_id).await
            .ok_or_else(|| CisError::not_found(format!("Session {} not found", session_id)))?;

        // 验证 session 是 Claude 类型
        {
            let session = session_arc.read().await;
            if session.agent_type != AgentType::Claude {
                return Err(CisError::invalid_input(format!(
                    "Session {} is not Claude type (is {:?})",
                    session_id, session.agent_type
                )));
            }
            
            if !session.is_persistent() {
                warn!("Session {} is not in persistent mode", session_id.short());
            }
        }

        info!("Successfully attached to Claude session: {}", session_id.short());

        Ok(Self {
            session_id,
            session_manager: manager,
            state: Arc::new(RwLock::new(ClaudeAgentState {
                status: AgentStatus::Running,
                current_task: None,
                completed_tasks: 0,
                total_output_chars: 0,
            })),
        })
    }

    /// 获取统计信息
    pub async fn stats(&self) -> ClaudeAgentStats {
        let state = self.state.read().await;
        ClaudeAgentStats {
            completed_tasks: state.completed_tasks,
            total_output_chars: state.total_output_chars,
            current_task: state.current_task.clone(),
        }
    }

    /// 等待任务完成
    ///
    /// 通过订阅 session 事件来等待任务完成或失败
    async fn wait_for_task_completion(&self, timeout: Duration) -> Result<String> {
        // 订阅 session 事件
        let mut event_rx = self.session_manager.subscribe_events();
        
        let start = Instant::now();
        let session_id = self.session_id.clone();
        
        // 获取初始输出作为基准
        let initial_output = self.session_manager.get_output(&session_id).await.unwrap_or_default();
        let mut last_output = initial_output.clone();
        
        // 用于检测任务完成的静默期（当输出停止变化一段时间后认为任务完成）
        let mut last_change = Instant::now();
        let silence_timeout = Duration::from_secs(5); // 5秒静默认为任务完成
        
        while start.elapsed() < timeout {
            // 检查事件
            while let Ok(event) = event_rx.try_recv() {
                match &event {
                    SessionEvent::Completed { session_id: sid, output, .. } => {
                        if sid == &session_id {
                            return Ok(output.clone());
                        }
                    }
                    SessionEvent::Failed { session_id: sid, error, .. } => {
                        if sid == &session_id {
                            return Err(CisError::execution(format!("Task failed: {}", error)));
                        }
                    }
                    SessionEvent::Killed { session_id: sid, reason, .. } => {
                        if sid == &session_id {
                            return Err(CisError::execution(format!("Session killed: {}", reason)));
                        }
                    }
                    _ => {}
                }
            }
            
            // 检查输出是否有变化
            if let Ok(current_output) = self.session_manager.get_output(&session_id).await {
                if current_output != last_output {
                    last_output = current_output;
                    last_change = Instant::now();
                } else if last_change.elapsed() > silence_timeout {
                    // 静默期已过，认为任务完成
                    // 返回新增的输出内容
                    if last_output.len() > initial_output.len() {
                        let new_output = &last_output[initial_output.len()..];
                        return Ok(new_output.trim().to_string());
                    }
                    return Ok("Task completed (no new output)".to_string());
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Err(CisError::execution(format!(
            "Task execution timeout after {:?}",
            timeout
        )))
    }

    /// 构造完整的提示词
    fn build_full_prompt(&self, task: &TaskRequest) -> String {
        let mut parts = Vec::new();
        
        // 添加上下文信息
        if !task.context.is_empty() {
            parts.push("Context:".to_string());
            for (key, value) in &task.context {
                if let Ok(v) = serde_json::to_string(value) {
                    parts.push(format!("  {}: {}", key, v));
                }
            }
            parts.push(String::new());
        }
        
        // 添加关联文件
        if !task.files.is_empty() {
            parts.push("Files:".to_string());
            for file in &task.files {
                parts.push(format!("  - {}", file.display()));
            }
            parts.push(String::new());
        }
        
        // 添加主要提示
        parts.push("Task:".to_string());
        parts.push(task.prompt.clone());
        
        parts.join("\n")
    }
}

#[async_trait]
impl PersistentAgent for ClaudePersistentAgent {
    fn agent_id(&self) -> &str {
        // 返回 session_id 的字符串表示
        &self.session_id.dag_run_id
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Claude
    }

    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        let start_time = Instant::now();
        let task_id = task.task_id.clone();
        
        info!("Executing task {} on Claude agent {}", task_id, self.session_id.short());

        // 更新状态为忙碌
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Busy;
            state.current_task = Some(task_id.clone());
        }

        // 确保 session 处于可接受任务的状态
        let session_state = self.session_manager.get_state(&self.session_id).await?;
        if !matches!(session_state, SessionState::Idle | SessionState::RunningDetached) {
            // 尝试恢复 session
            if let Some(session_arc) = self.session_manager.get_session(&self.session_id).await {
                let session = session_arc.write().await;
                if session_state == SessionState::Paused {
                    if let Err(e) = session.resume().await {
                        warn!("Failed to resume session {}: {}", self.session_id.short(), e);
                    }
                }
            }
        }

        // 构造完整的提示
        let full_prompt = self.build_full_prompt(&task);
        debug!("Sending prompt to Claude session {}:\n{}", self.session_id.short(), full_prompt);

        // 通过 SessionManager 发送输入到 PTY
        self.session_manager.send_input(&self.session_id, full_prompt.as_bytes()).await?;

        // 等待任务完成
        let timeout = Duration::from_secs(task.timeout_secs.unwrap_or(300));
        let output = match self.wait_for_task_completion(timeout).await {
            Ok(out) => out,
            Err(e) => {
                // 更新状态为错误
                {
                    let mut state = self.state.write().await;
                    state.status = AgentStatus::Error;
                    state.current_task = None;
                }
                return Err(e);
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // 更新统计
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Idle;
            state.current_task = None;
            state.completed_tasks += 1;
            state.total_output_chars += output.len() as u64;
        }

        // 将 session 标记为 idle（保持持久化）
        if let Some(session_arc) = self.session_manager.get_session(&self.session_id).await {
            let session = session_arc.read().await;
            session.mark_idle().await;
        }

        info!(
            "Task {} completed in {}ms on Claude agent {}",
            task_id, duration_ms, self.session_id.short()
        );

        Ok(TaskResult {
            task_id: task_id.clone(),
            success: true,
            output: Some(output),
            error: None,
            duration_ms,
            completed_at: Utc::now(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("session_id".to_string(), json!(self.session_id.to_string()));
                meta.insert("agent_type".to_string(), json!("claude"));
                meta.insert("runtime".to_string(), json!("claude"));
                meta
            },
        })
    }

    async fn status(&self) -> AgentStatus {
        // 首先检查 session 是否存在
        let session_exists = self.session_manager.get_session(&self.session_id).await.is_some();
        if !session_exists {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Shutdown;
            return AgentStatus::Shutdown;
        }

        // 获取 session 状态
        match self.session_manager.get_state(&self.session_id).await {
            Ok(session_state) => {
                let mut state = self.state.write().await;
                state.status = match session_state {
                    SessionState::Idle => AgentStatus::Idle,
                    SessionState::RunningDetached => AgentStatus::Running,
                    SessionState::Attached { .. } => AgentStatus::Busy,
                    SessionState::Blocked { .. } => AgentStatus::Error,
                    SessionState::Spawning => AgentStatus::Running,
                    SessionState::Paused => AgentStatus::Running,
                    SessionState::Completed { .. } => AgentStatus::Idle,
                    SessionState::Failed { .. } => AgentStatus::Error,
                    SessionState::Killed => AgentStatus::Shutdown,
                };
                state.status.clone()
            }
            Err(_) => {
                let mut state = self.state.write().await;
                state.status = AgentStatus::Error;
                AgentStatus::Error
            }
        }
    }

    async fn attach(&self) -> Result<()> {
        info!("Attaching to Claude session: {}", self.session_id.short());

        // 获取当前用户（从环境变量或使用默认值）
        let user = std::env::var("USER").unwrap_or_else(|_| "cli".to_string());
        
        // 使用 SessionManager 的 attach_session 功能
        let handle = self.session_manager.attach_session(&self.session_id, &user).await?;
        
        // 在后台持续读取输出直到用户 detach
        // 注意：实际的 attach 交互是通过 handle 进行的
        // 这里我们等待一段时间，实际的交互由调用者处理
        
        // attach 会阻塞直到用户 detach，但这里我们只是设置状态
        // 实际的 attach 交互应该由 CLI/GUI 层处理
        
        info!("Attached to Claude session: {}", self.session_id.short());
        
        // 保持 handle 存活
        drop(handle);
        
        Ok(())
    }

    async fn detach(&self) -> Result<()> {
        info!("Detaching from Claude session: {}", self.session_id.short());

        // 获取当前用户
        let user = std::env::var("USER").unwrap_or_else(|_| "cli".to_string());
        
        // 使用 SessionManager 的 detach_session 功能
        self.session_manager.detach_session(&self.session_id, &user).await?;

        info!("Detached from Claude session: {}", self.session_id.short());
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Claude agent: {}", self.session_id.short());

        // 更新状态
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Shutdown;
        }

        // 使用 SessionManager 终止 session
        self.session_manager
            .kill_session(&self.session_id, "Shutdown requested")
            .await?;

        info!("Claude agent {} shutdown complete", self.session_id.short());
        Ok(())
    }
}

impl Drop for ClaudePersistentAgent {
    fn drop(&mut self) {
        // 尝试在 drop 时清理资源
        // 注意：这里不能使用 async，只能尝试同步关闭
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let session_manager = self.session_manager;
            let session_id = self.session_id.clone();
            rt.spawn(async move {
                let _ = session_manager.kill_session(&session_id, "Agent dropped").await;
            });
        }
    }
}

/// Claude Runtime 实现
///
/// 负责管理和创建 Claude Persistent Agent。
#[derive(Debug)]
pub struct ClaudeRuntime {
    session_manager: &'static SessionManager,
}

impl ClaudeRuntime {
    /// 创建新的 ClaudeRuntime
    pub fn new() -> Self {
        Self {
            session_manager: SessionManager::global(),
        }
    }

    /// 从 SessionManager 创建（用于依赖注入）
    pub fn with_manager(manager: &'static SessionManager) -> Self {
        Self { session_manager: manager }
    }
    
    /// 从 session 目录加载统计信息（异步）
    async fn load_session_stats(work_dir: &std::path::PathBuf) -> (Option<chrono::DateTime<Utc>>, u32) {
        let stats_file = work_dir.join(".session_stats.json");
        
        if let Ok(content) = tokio::fs::read_to_string(&stats_file).await {
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(&content) {
                let last_active = stats.get("last_active")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));
                
                let total_tasks = stats.get("total_tasks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                
                return (last_active, total_tasks);
            }
        }
        
        (None, 0)
    }
    
    /// 从 session 目录加载统计信息（同步版本）
    fn load_session_stats_sync(work_dir: &std::path::PathBuf) -> (Option<chrono::DateTime<Utc>>, u32) {
        let stats_file = work_dir.join(".session_stats.json");
        
        if let Ok(content) = std::fs::read_to_string(&stats_file) {
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(&content) {
                let last_active: Option<chrono::DateTime<Utc>> = stats.get("last_active")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));
                
                let total_tasks = stats.get("total_tasks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                
                return (last_active, total_tasks);
            }
        }
        
        (None, 0)
    }
}

impl Default for ClaudeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentRuntime for ClaudeRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Claude
    }

    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>> {
        let agent = ClaudePersistentAgent::start(self.session_manager, config).await?;
        Ok(Box::new(agent))
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        // 从 SessionManager 获取所有 Claude 类型的 session
        // 筛选出 persistent 标记的
        let summaries = self.session_manager.list_sessions().await;
        
        summaries
            .into_iter()
            .filter(|s| s.agent_type == AgentType::Claude)
            // 筛选出 run_id 为 "persistent" 的 session
            .filter(|s| s.dag_run_id == "persistent")
            .map(|s| {
                let status = match s.state.as_str() {
                    "idle" => AgentStatus::Idle,
                    "running" | "spawning" => AgentStatus::Running,
                    "attached" => AgentStatus::Busy,
                    "blocked" => AgentStatus::Error,
                    "completed" => AgentStatus::Idle,
                    "failed" | "killed" => AgentStatus::Error,
                    _ => AgentStatus::Running,
                };
                
                // 从 work_dir 推断统计信息
                let work_dir = std::env::temp_dir().join(&s.id);
                
                // 尝试从 session 文件加载统计信息（同步）
                let (last_active, total_tasks) = Self::load_session_stats_sync(&work_dir);
                
                AgentInfo {
                    id: s.id.clone(),
                    name: s.task_id.clone(), // 使用 task_id 作为名称
                    runtime_type: RuntimeType::Claude,
                    status,
                    current_task: None,
                    created_at: s.created_at,
                    last_active_at: last_active.unwrap_or(s.created_at),
                    total_tasks: total_tasks as u64,
                    work_dir,
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("short_id".to_string(), json!(s.short_id));
                        m
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use downcast_rs::Downcast;

    #[tokio::test]
    async fn test_claude_runtime_creation() {
        let runtime = ClaudeRuntime::new();
        assert_eq!(runtime.runtime_type(), RuntimeType::Claude);
    }

    #[tokio::test]
    async fn test_claude_runtime_list_agents() {
        let runtime = ClaudeRuntime::new();
        let agents = runtime.list_agents().await;
        // 初始时应该没有 agent
        assert!(agents.is_empty() || !agents.iter().any(|a| a.runtime_type == RuntimeType::Claude));
    }

    // 集成测试（需要 claude 命令）
    #[tokio::test]
    #[ignore = "Requires claude to be installed"]
    async fn test_claude_agent_lifecycle() {
        let runtime = ClaudeRuntime::new();
        let config = AgentConfig::new(
            "test-claude-agent",
            std::env::temp_dir().join("claude-test"),
        );

        // 创建 Agent
        let agent = runtime.create_agent(config).await.unwrap();
        assert_eq!(agent.runtime_type(), RuntimeType::Claude);

        // 检查状态
        let status = agent.status().await;
        assert!(status.is_available());

        // 执行任务
        let task = TaskRequest::new("test-task-1", "Say 'Hello, World!'");

        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
        assert!(result.output.is_some());

        // 检查统计（需要获取具体类型）
        let agent_ref = agent.as_any().downcast_ref::<ClaudePersistentAgent>().unwrap();
        let stats = agent_ref.stats().await;
        assert_eq!(stats.completed_tasks, 1);

        // 关闭 Agent
        let _: () = agent.shutdown().await.unwrap();

        // 验证状态
        let status: AgentStatus = agent.status().await;
        assert!(!status.is_available());
    }
}
