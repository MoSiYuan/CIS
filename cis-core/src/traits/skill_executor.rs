//! # SkillExecutor Trait
//!
//! Skill 执行器的抽象接口，定义 Skill 执行的基本操作。
//!
//! ## 设计原则
//!
//! - **资源隔离**: 每个 Skill 在独立的资源限制下运行
//! - **可观测性**: 提供执行状态和日志查询
//! - **可控性**: 支持取消和超时控制
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::traits::{SkillExecutor, Skill, ExecutionContext};
//! use std::sync::Arc;
//!
//! # async fn example(executor: Arc<dyn SkillExecutor>) -> anyhow::Result<()> {
//! // 列出可用 Skills
//! let skills = executor.list_skills().await?;
//! for skill in skills {
//!     println!("{}: {}", skill.name, skill.description);
//! }
//!
//! // 执行 Skill
//! let skill = Skill::new("code-review");
//! let context = ExecutionContext::new("dag-1", "task-1", "/tmp/work");
//! let result = executor.execute(&skill, context).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// 执行上下文
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 执行 ID
    pub execution_id: String,
    /// DAG Run ID
    pub dag_run_id: String,
    /// 任务 ID
    pub task_id: String,
    /// 工作目录
    pub work_dir: std::path::PathBuf,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 上游输出
    pub upstream_outputs: HashMap<String, String>,
    /// 执行超时（秒）
    pub timeout_secs: u64,
    /// 执行用户
    pub user: Option<String>,
    /// 触发方式
    pub trigger: String,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    ///
    /// # Arguments
    /// * `dag_run_id` - DAG Run ID
    /// * `task_id` - 任务 ID
    /// * `work_dir` - 工作目录
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::traits::ExecutionContext;
    ///
    /// let context = ExecutionContext::new("dag-run-1", "task-1", "/tmp/work");
    /// ```
    pub fn new(
        dag_run_id: impl Into<String>,
        task_id: impl Into<String>,
        work_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            execution_id: format!("exec_{}", uuid::Uuid::new_v4()),
            dag_run_id: dag_run_id.into(),
            task_id: task_id.into(),
            work_dir: work_dir.into(),
            env_vars: HashMap::new(),
            upstream_outputs: HashMap::new(),
            timeout_secs: 3600, // 默认 1 小时
            user: None,
            trigger: "manual".to_string(),
        }
    }

    /// 设置环境变量
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// 设置上游输出
    pub fn with_upstream_output(
        mut self,
        task_id: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        self.upstream_outputs.insert(task_id.into(), output.into());
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// 设置执行用户
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// 设置触发方式
    pub fn with_trigger(mut self, trigger: impl Into<String>) -> Self {
        self.trigger = trigger.into();
        self
    }
}

/// Skill 执行配置
#[derive(Debug, Clone)]
pub struct SkillExecutionConfig {
    /// 最大并发执行数
    pub max_concurrent: usize,
    /// 默认超时（秒）
    pub default_timeout_secs: u64,
    /// 是否启用沙箱
    pub enable_sandbox: bool,
    /// 资源限制
    pub resource_limits: ResourceLimits,
    /// 是否记录详细日志
    pub verbose_logging: bool,
    /// 保留日志天数
    pub log_retention_days: u32,
}

impl Default for SkillExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            default_timeout_secs: 3600,
            enable_sandbox: true,
            resource_limits: ResourceLimits::default(),
            verbose_logging: false,
            log_retention_days: 7,
        }
    }
}

/// 资源限制
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// 最大内存（MB）
    pub max_memory_mb: usize,
    /// 最大 CPU 使用率（百分比）
    pub max_cpu_percent: f64,
    /// 最大网络带宽（KB/s）
    pub max_network_kbps: usize,
    /// 最大磁盘使用（MB）
    pub max_disk_mb: usize,
    /// 最大打开文件数
    pub max_open_files: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,
            max_cpu_percent: 50.0,
            max_network_kbps: 1024,
            max_disk_mb: 100,
            max_open_files: 1024,
        }
    }
}

/// Skill 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 成功完成
    Success,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
    /// 超时
    Timeout,
    /// 排队中
    Queued,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Success => write!(f, "success"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
            ExecutionStatus::Timeout => write!(f, "timeout"),
            ExecutionStatus::Queued => write!(f, "queued"),
        }
    }
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 退出码
    pub exit_code: i32,
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 输出产物
    pub artifacts: Vec<String>,
    /// 执行时长（毫秒）
    pub duration_ms: u64,
    /// 资源使用统计
    pub resource_usage: Option<ResourceUsage>,
}

/// 资源使用统计
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// 内存使用峰值（MB）
    pub peak_memory_mb: usize,
    /// CPU 使用时间（毫秒）
    pub cpu_time_ms: u64,
    /// 磁盘读取（KB）
    pub disk_read_kb: u64,
    /// 磁盘写入（KB）
    pub disk_write_kb: u64,
    /// 网络发送（KB）
    pub network_sent_kb: u64,
    /// 网络接收（KB）
    pub network_recv_kb: u64,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            success: true,
            exit_code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
            artifacts: Vec::new(),
            duration_ms: 0,
            resource_usage: None,
        }
    }

    /// 创建失败结果
    pub fn failure(stderr: impl Into<String>) -> Self {
        Self {
            success: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: stderr.into(),
            artifacts: Vec::new(),
            duration_ms: 0,
            resource_usage: None,
        }
    }

    /// 添加产物
    pub fn with_artifact(mut self, artifact: impl Into<String>) -> Self {
        self.artifacts.push(artifact.into());
        self
    }

    /// 设置执行时长
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }
}

/// 执行信息
#[derive(Debug, Clone)]
pub struct ExecutionInfo {
    /// 执行 ID
    pub execution_id: String,
    /// Skill 名称
    pub skill_name: String,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 开始时间（Unix 毫秒）
    pub started_at: Option<u64>,
    /// 结束时间（Unix 毫秒）
    pub ended_at: Option<u64>,
    /// 退出码
    pub exit_code: Option<i32>,
    /// 错误信息
    pub error_message: Option<String>,
    /// 进度百分比 (0-100)
    pub progress_percent: u8,
    /// 当前步骤
    pub current_step: Option<String>,
}

/// Skill 元数据
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    /// Skill 名称（唯一标识）
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 输入参数定义（JSON Schema）
    pub input_schema: Option<String>,
    /// 输出定义（JSON Schema）
    pub output_schema: Option<String>,
    /// 默认资源限制
    pub default_resources: ResourceLimits,
    /// 是否启用
    pub enabled: bool,
    /// 创建时间
    pub created_at: u64,
    /// 更新时间
    pub updated_at: u64,
}

/// Skill 定义（简化版）
#[derive(Debug, Clone)]
pub struct Skill {
    /// Skill 名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 参数
    pub parameters: HashMap<String, String>,
}

impl Skill {
    /// 创建新的 Skill
    ///
    /// # Arguments
    /// * `name` - Skill 名称
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::traits::Skill;
    ///
    /// let skill = Skill::new("code-review")
    ///     .with_description("Review code for issues")
    ///     .with_version("1.0.0");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            version: "1.0.0".to_string(),
            parameters: HashMap::new(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// 添加参数
    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.insert(key.into(), value.into());
        self
    }
}

/// Skill 执行器抽象接口
///
/// 定义 Skill 执行的基本操作，包括执行、状态查询和取消。
///
/// ## 实现要求
///
/// - 所有方法必须是线程安全的 (Send + Sync)
/// - 所有异步方法必须返回 Result 类型
/// - 实现应该提供资源隔离和超时控制
///
/// ## 使用示例
///
/// ```rust,no_run
/// use cis_core::traits::{SkillExecutor, Skill, ExecutionContext};
/// use std::sync::Arc;
///
/// # async fn example(executor: Arc<dyn SkillExecutor>) -> anyhow::Result<()> {
/// // 列出可用 Skills
/// let skills = executor.list_skills().await?;
///
/// // 执行 Skill
/// let skill = Skill::new("test-skill");
/// let context = ExecutionContext::new("dag-1", "task-1", "/tmp");
/// let result = executor.execute(&skill, context).await?;
///
/// if result.success {
    ///     println!("Output: {}", result.stdout);
/// } else {
    ///     eprintln!("Error: {}", result.stderr);
/// }
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    /// 执行 Skill
    ///
    /// # Arguments
    /// * `skill` - Skill 定义
    /// * `context` - 执行上下文
    ///
    /// # Returns
    /// * `Ok(ExecutionResult)` - 执行完成
    /// * `Err(CisError::Skill(_))` - Skill 错误
    /// * `Err(CisError::Execution(_))` - 执行错误
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::{SkillExecutor, Skill, ExecutionContext};
    ///
    /// # async fn example(executor: &dyn SkillExecutor) -> anyhow::Result<()> {
    /// let skill = Skill::new("code-lint")
    ///     .with_description("Lint code files");
    ///
    /// let context = ExecutionContext::new("dag-1", "task-1", "/project")
    ///     .with_timeout(300)
    ///     .with_env_var("LANGUAGE", "rust");
    ///
    /// match executor.execute(&skill, context).await {
    ///     Ok(result) => {
    ///         if result.success {
    ///             println!("Lint passed!");
    ///         } else {
    ///             eprintln!("Lint failed: {}", result.stderr);
    ///         }
    ///     }
    ///     Err(e) => eprintln!("Execution error: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn execute(&self, skill: &Skill, context: ExecutionContext) -> Result<ExecutionResult>;

    /// 列出所有可用的 Skills
    ///
    /// # Returns
    /// * `Ok(Vec<SkillMetadata>)` - Skill 元数据列表
    /// * `Err(CisError::Skill(_))` - 查询失败
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::SkillExecutor;
    ///
    /// # async fn example(executor: &dyn SkillExecutor) -> anyhow::Result<()> {
    /// let skills = executor.list_skills().await?;
    /// for skill in skills {
    ///     println!("{} v{}: {}", 
    ///         skill.name, 
    ///         skill.version,
    ///         skill.description
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn list_skills(&self) -> Result<Vec<SkillMetadata>>;

    /// 获取特定 Skill 的元数据
    ///
    /// # Arguments
    /// * `skill_name` - Skill 名称
    ///
    /// # Returns
    /// * `Ok(Some(SkillMetadata))` - Skill 存在
    /// * `Ok(None)` - Skill 不存在
    /// * `Err(CisError::Skill(_))` - 查询失败
    async fn get_skill_metadata(&self, skill_name: &str) -> Result<Option<SkillMetadata>>;

    /// 获取执行状态
    ///
    /// # Arguments
    /// * `execution_id` - 执行 ID
    ///
    /// # Returns
    /// * `Ok(Some(ExecutionInfo))` - 执行信息
    /// * `Ok(None)` - 执行不存在
    /// * `Err(CisError::Execution(_))` - 查询失败
    async fn get_status(&self, execution_id: &str) -> Result<Option<ExecutionInfo>>;

    /// 取消执行
    ///
    /// # Arguments
    /// * `execution_id` - 执行 ID
    ///
    /// # Returns
    /// * `Ok(())` - 取消成功
    /// * `Err(CisError::NotFound(_))` - 执行不存在
    /// * `Err(CisError::Execution(_))` - 取消失败（如已结束）
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::traits::SkillExecutor;
    ///
    /// # async fn example(executor: &dyn SkillExecutor) -> anyhow::Result<()> {
    /// match executor.cancel("exec-123").await {
    ///     Ok(()) => println!("Execution cancelled"),
    ///     Err(e) => eprintln!("Failed to cancel: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    async fn cancel(&self, execution_id: &str) -> Result<()>;

    /// 列出正在执行的 Skill
    ///
    /// # Returns
    /// * `Ok(Vec<ExecutionInfo>)` - 执行信息列表
    /// * `Err(CisError::Execution(_))` - 查询失败
    async fn list_running(&self) -> Result<Vec<ExecutionInfo>>;

    /// 列出最近的执行历史
    ///
    /// # Arguments
    /// * `limit` - 返回的最大数量
    ///
    /// # Returns
    /// * `Ok(Vec<ExecutionInfo>)` - 执行信息列表
    async fn list_history(&self, limit: usize) -> Result<Vec<ExecutionInfo>>;

    /// 获取执行日志
    ///
    /// # Arguments
    /// * `execution_id` - 执行 ID
    /// * `lines` - 返回的最大行数（从末尾开始）
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - 日志行列表
    /// * `Err(CisError::NotFound(_))` - 执行不存在
    async fn get_logs(&self, execution_id: &str, lines: usize) -> Result<Vec<String>>;

    /// 获取执行器配置
    ///
    /// # Returns
    /// * `Ok(SkillExecutionConfig)` - 当前配置
    fn get_config(&self) -> Result<SkillExecutionConfig>;

    /// 更新执行器配置
    ///
    /// # Arguments
    /// * `config` - 新配置
    ///
    /// # Returns
    /// * `Ok(())` - 更新成功
    /// * `Err(CisError::Configuration(_))` - 配置无效
    async fn update_config(&self, config: SkillExecutionConfig) -> Result<()>;
}

/// SkillExecutor 的 Arc 包装类型
pub type SkillExecutorRef = Arc<dyn SkillExecutor>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_builder() {
        let ctx = ExecutionContext::new("dag-1", "task-1", "/tmp")
            .with_env_var("KEY", "value")
            .with_timeout(600)
            .with_user("admin")
            .with_trigger("schedule");

        assert_eq!(ctx.dag_run_id, "dag-1");
        assert_eq!(ctx.timeout_secs, 600);
        assert_eq!(ctx.user, Some("admin".to_string()));
        assert_eq!(ctx.trigger, "schedule");
        assert_eq!(ctx.env_vars.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_execution_result_builder() {
        let result = ExecutionResult::success("output")
            .with_artifact("file.txt")
            .with_duration(5000);

        assert!(result.success);
        assert_eq!(result.stdout, "output");
        assert_eq!(result.artifacts, vec!["file.txt"]);
        assert_eq!(result.duration_ms, 5000);
    }

    #[test]
    fn test_skill_builder() {
        let skill = Skill::new("test-skill")
            .with_description("Test description")
            .with_version("2.0.0")
            .with_parameter("key", "value");

        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "Test description");
        assert_eq!(skill.version, "2.0.0");
        assert_eq!(skill.parameters.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_execution_status_display() {
        assert_eq!(format!("{}", ExecutionStatus::Pending), "pending");
        assert_eq!(format!("{}", ExecutionStatus::Running), "running");
        assert_eq!(format!("{}", ExecutionStatus::Success), "success");
        assert_eq!(format!("{}", ExecutionStatus::Failed), "failed");
        assert_eq!(format!("{}", ExecutionStatus::Cancelled), "cancelled");
        assert_eq!(format!("{}", ExecutionStatus::Timeout), "timeout");
        assert_eq!(format!("{}", ExecutionStatus::Queued), "queued");
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_mb, 512);
        assert_eq!(limits.max_cpu_percent, 50.0);
        assert_eq!(limits.max_open_files, 1024);
    }
}
