//! # Persistent Agent 模块
//!
//! 提供支持持久化运行的 AI Agent 抽象接口，用于 Agent Teams 功能。
//!
//! 主要特性：
//! - 持久化 Agent 生命周期管理
//! - 前后台切换（attach/detach）
//! - 统一的 Runtime 抽象
//! - 任务执行和状态监控
//! - Agent Pool 管理

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::Result;

pub mod claude;
pub mod opencode;
pub mod pool;

pub use claude::{ClaudeAgentStats, ClaudePersistentAgent, ClaudeRuntime};
pub use opencode::{OpenCodePersistentAgent, OpenCodeRuntime};
pub use pool::{AgentAcquireConfig, AgentHandle, AgentPool, PoolConfig};

/// Persistent Agent 统一接口
///
/// 所有持久化运行的 AI Agent（Claude, OpenCode, Kimi, Aider）实现此接口。
/// 支持前后台切换、状态监控和优雅关闭。
#[async_trait]
pub trait PersistentAgent: Send + Sync {
    /// 获取 Agent ID
    fn agent_id(&self) -> &str;

    /// 获取 Runtime 类型
    fn runtime_type(&self) -> RuntimeType;

    /// 执行任务
    ///
    /// # Arguments
    /// * `task` - 任务请求
    ///
    /// # Returns
    /// 任务执行结果
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult>;

    /// 获取 Agent 状态
    ///
    /// # Returns
    /// 当前 Agent 状态
    async fn status(&self) -> AgentStatus;

    /// 前台 attach（进入交互式模式）
    ///
    /// 将 Agent 切换到前台，用户可以直接与 Agent 交互。
    /// 此操作会阻塞直到用户主动 detach。
    async fn attach(&self) -> Result<()>;

    /// 后台 detach（返回后台运行）
    ///
    /// 将 Agent 切换到后台，Agent 继续运行但不再占用终端。
    async fn detach(&self) -> Result<()>;

    /// 优雅关闭
    ///
    /// 请求 Agent 优雅地关闭，保存状态并释放资源。
    async fn shutdown(&self) -> Result<()>;
}

/// Agent Runtime 统一接口
///
/// 负责管理和创建特定类型的 Persistent Agent。
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// 获取 Runtime 类型
    fn runtime_type(&self) -> RuntimeType;

    /// 创建新的 Agent
    ///
    /// # Arguments
    /// * `config` - Agent 配置
    ///
    /// # Returns
    /// 新创建的 Agent 实例
    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>>;

    /// 列出所有 Agent
    ///
    /// # Returns
    /// 所有 Agent 的信息列表
    async fn list_agents(&self) -> Vec<AgentInfo>;
}

/// Runtime 类型枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    /// Claude Code
    Claude,
    /// OpenCode
    OpenCode,
    /// Kimi Code
    Kimi,
    /// Aider
    Aider,
}

impl RuntimeType {
    /// 获取命令名称
    pub fn command_name(&self) -> &'static str {
        match self {
            RuntimeType::Claude => "claude",
            RuntimeType::OpenCode => "opencode",
            RuntimeType::Kimi => "kimi",
            RuntimeType::Aider => "aider",
        }
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            RuntimeType::Claude => "Claude Code",
            RuntimeType::OpenCode => "OpenCode",
            RuntimeType::Kimi => "Kimi Code",
            RuntimeType::Aider => "Aider",
        }
    }

    /// 是否支持 PTY 交互
    pub fn supports_pty(&self) -> bool {
        // 当前所有 Runtime 类型都支持 PTY 交互
        true
    }
}

impl std::fmt::Display for RuntimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for RuntimeType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(RuntimeType::Claude),
            "opencode" => Ok(RuntimeType::OpenCode),
            "kimi" => Ok(RuntimeType::Kimi),
            "aider" => Ok(RuntimeType::Aider),
            _ => Err(format!("Invalid runtime type: {}", s)),
        }
    }
}

/// Agent 状态枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// 运行中（后台）
    Running,
    /// 空闲等待
    Idle,
    /// 忙碌执行任务
    Busy,
    /// 错误状态
    Error,
    /// 已关闭
    Shutdown,
}

impl AgentStatus {
    /// 检查 Agent 是否可用（可以接收任务）
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Running | Self::Idle)
    }

    /// 检查 Agent 是否处于活动状态
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Shutdown)
    }

    /// 检查 Agent 是否处于错误状态
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Error => "error",
            Self::Shutdown => "shutdown",
        };
        write!(f, "{}", s)
    }
}

/// 任务请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// 任务 ID
    pub task_id: String,
    /// 任务描述/Prompt
    pub prompt: String,
    /// 工作目录
    pub work_dir: Option<PathBuf>,
    /// 关联的文件列表
    pub files: Vec<PathBuf>,
    /// 额外上下文
    pub context: HashMap<String, serde_json::Value>,
    /// 超时时间（秒）
    pub timeout_secs: Option<u64>,
}

impl TaskRequest {
    /// 创建新的任务请求
    pub fn new(task_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            prompt: prompt.into(),
            work_dir: None,
            files: Vec::new(),
            context: HashMap::new(),
            timeout_secs: None,
        }
    }

    /// 设置工作目录
    pub fn with_work_dir(mut self, dir: PathBuf) -> Self {
        self.work_dir = Some(dir);
        self
    }

    /// 添加文件
    pub fn with_file(mut self, file: PathBuf) -> Self {
        self.files.push(file);
        self
    }

    /// 添加多个文件
    pub fn with_files(mut self, files: Vec<PathBuf>) -> Self {
        self.files.extend(files);
        self
    }

    /// 添加上下文
    pub fn with_context(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.context.insert(key.into(), v);
        }
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }
}

/// 任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// 任务 ID
    pub task_id: String,
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub output: Option<String>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
    /// 完成时间
    pub completed_at: DateTime<Utc>,
    /// 额外元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskResult {
    /// 创建成功结果
    pub fn success(task_id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            success: true,
            output: Some(output.into()),
            error: None,
            duration_ms: 0,
            completed_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// 创建失败结果
    pub fn error(task_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            success: false,
            output: None,
            error: Some(error.into()),
            duration_ms: 0,
            completed_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// 设置执行时长
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), v);
        }
        self
    }
}

/// Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent 名称
    pub name: String,
    /// 使用的模型
    pub model: Option<String>,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 工作目录
    pub work_dir: PathBuf,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 最大并发任务数
    pub max_concurrent_tasks: usize,
    /// 默认超时时间（秒）
    pub default_timeout_secs: u64,
    /// 是否自动重启
    pub auto_restart: bool,
}

impl AgentConfig {
    /// 创建新的 Agent 配置
    pub fn new(name: impl Into<String>, work_dir: PathBuf) -> Self {
        Self {
            name: name.into(),
            model: None,
            system_prompt: None,
            work_dir,
            env_vars: HashMap::new(),
            max_concurrent_tasks: 1,
            default_timeout_secs: 300,
            auto_restart: false,
        }
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 添加环境变量
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// 设置最大并发任务数
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent_tasks = max;
        self
    }

    /// 设置默认超时时间
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.default_timeout_secs = secs;
        self
    }

    /// 启用自动重启
    pub fn with_auto_restart(mut self) -> Self {
        self.auto_restart = true;
        self
    }
}

/// Agent 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent ID
    pub id: String,
    /// Agent 名称
    pub name: String,
    /// Runtime 类型
    pub runtime_type: RuntimeType,
    /// 当前状态
    pub status: AgentStatus,
    /// 当前任务 ID（如果有）
    pub current_task: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后活动时间
    pub last_active_at: DateTime<Utc>,
    /// 已执行任务数
    pub total_tasks: u64,
    /// 工作目录
    pub work_dir: PathBuf,
    /// 额外元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentInfo {
    /// 创建新的 Agent 信息
    pub fn new(id: impl Into<String>, name: impl Into<String>, work_dir: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            runtime_type: RuntimeType::Claude,
            status: AgentStatus::Idle,
            current_task: None,
            created_at: now,
            last_active_at: now,
            total_tasks: 0,
            work_dir,
            metadata: HashMap::new(),
        }
    }

    /// 设置 Runtime 类型
    pub fn with_runtime_type(mut self, rt: RuntimeType) -> Self {
        self.runtime_type = rt;
        self
    }

    /// 设置状态
    pub fn with_status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    /// 设置当前任务
    pub fn with_current_task(mut self, task_id: impl Into<String>) -> Self {
        self.current_task = Some(task_id.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), v);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_runtime_type() {
        assert_eq!(RuntimeType::Claude.command_name(), "claude");
        assert_eq!(RuntimeType::Kimi.display_name(), "Kimi Code");
        assert!(RuntimeType::OpenCode.supports_pty());

        // 测试字符串解析
        assert_eq!(RuntimeType::from_str("claude").unwrap(), RuntimeType::Claude);
        assert_eq!(RuntimeType::from_str("Kimi").unwrap(), RuntimeType::Kimi);
        assert!(RuntimeType::from_str("unknown").is_err());
    }

    #[test]
    fn test_agent_status() {
        assert!(AgentStatus::Idle.is_available());
        assert!(AgentStatus::Running.is_available());
        assert!(!AgentStatus::Busy.is_available());
        assert!(!AgentStatus::Error.is_available());
        assert!(!AgentStatus::Shutdown.is_available());

        assert!(AgentStatus::Running.is_active());
        assert!(AgentStatus::Error.is_active());
        assert!(!AgentStatus::Shutdown.is_active());

        assert!(AgentStatus::Error.is_error());
        assert!(!AgentStatus::Running.is_error());
    }

    #[test]
    fn test_task_request_builder() {
        let req = TaskRequest::new("task-1", "Hello world")
            .with_work_dir(PathBuf::from("/tmp"))
            .with_file(PathBuf::from("test.txt"))
            .with_timeout(60);

        assert_eq!(req.task_id, "task-1");
        assert_eq!(req.prompt, "Hello world");
        assert_eq!(req.work_dir, Some(PathBuf::from("/tmp")));
        assert_eq!(req.files.len(), 1);
        assert_eq!(req.timeout_secs, Some(60));
    }

    #[test]
    fn test_task_result_builder() {
        let result = TaskResult::success("task-1", "Done")
            .with_duration(1000)
            .with_metadata("key", "value");

        assert!(result.success);
        assert_eq!(result.output, Some("Done".to_string()));
        assert_eq!(result.duration_ms, 1000);
        assert!(result.metadata.contains_key("key"));
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new("test-agent", PathBuf::from("/work"))
            .with_model("claude-3-sonnet")
            .with_system_prompt("You are a helpful assistant")
            .with_env("API_KEY", "secret")
            .with_max_concurrent(2)
            .with_timeout(600)
            .with_auto_restart();

        assert_eq!(config.name, "test-agent");
        assert_eq!(config.model, Some("claude-3-sonnet".to_string()));
        assert_eq!(config.system_prompt, Some("You are a helpful assistant".to_string()));
        assert_eq!(config.env_vars.get("API_KEY"), Some(&"secret".to_string()));
        assert_eq!(config.max_concurrent_tasks, 2);
        assert_eq!(config.default_timeout_secs, 600);
        assert!(config.auto_restart);
    }

    #[test]
    fn test_agent_info_builder() {
        let info = AgentInfo::new("agent-1", "My Agent", PathBuf::from("/work"))
            .with_runtime_type(RuntimeType::OpenCode)
            .with_status(AgentStatus::Busy)
            .with_current_task("task-1");

        assert_eq!(info.id, "agent-1");
        assert_eq!(info.name, "My Agent");
        assert_eq!(info.runtime_type, RuntimeType::OpenCode);
        assert_eq!(info.status, AgentStatus::Busy);
        assert_eq!(info.current_task, Some("task-1".to_string()));
    }

    #[test]
    fn test_serialization() {
        // 测试序列化/反序列化
        let req = TaskRequest::new("task-1", "test");
        let json = serde_json::to_string(&req).unwrap();
        let decoded: TaskRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.task_id, decoded.task_id);

        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }
}
