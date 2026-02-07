//! # CLI AI-Native Module
//!
//! 提供 AI-Native 的 CLI 支持，包括：
//! - 结构化 JSON 输出
//! - 标准化退出码
//! - 命令 Schema 自描述
//! - 管道和组合支持

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod output;
pub mod schema;

pub use output::{OutputFormat, OutputManager, TableOutput, TreeOutput};
pub use schema::{CommandSchema, CommandRegistry, ParameterSchema, TypeSchema};

/// 标准化退出码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// 成功
    Success = 0,

    /// 一般错误（通用）
    GeneralError = 1,

    /// 需要用户确认
    NeedConfirmation = 2,

    /// 配置缺失
    ConfigMissing = 3,

    /// 网络错误
    NetworkError = 4,

    /// 权限错误
    PermissionDenied = 5,

    /// 资源不可用
    ResourceUnavailable = 6,

    /// 超时
    Timeout = 7,

    /// 部分成功（批量操作）
    PartialSuccess = 8,

    /// 取消（用户中断）
    Cancelled = 130, // SIGINT
}

impl ExitCode {
    /// 获取退出码的整数表示
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// 从整数创建退出码
    pub fn from_i32(code: i32) -> Self {
        match code {
            0 => ExitCode::Success,
            1 => ExitCode::GeneralError,
            2 => ExitCode::NeedConfirmation,
            3 => ExitCode::ConfigMissing,
            4 => ExitCode::NetworkError,
            5 => ExitCode::PermissionDenied,
            6 => ExitCode::ResourceUnavailable,
            7 => ExitCode::Timeout,
            8 => ExitCode::PartialSuccess,
            130 => ExitCode::Cancelled,
            _ => ExitCode::GeneralError,
        }
    }

    /// 获取人类可读的描述
    pub fn description(&self) -> &'static str {
        match self {
            ExitCode::Success => "Success",
            ExitCode::GeneralError => "General error",
            ExitCode::NeedConfirmation => "User confirmation required",
            ExitCode::ConfigMissing => "Configuration missing",
            ExitCode::NetworkError => "Network error",
            ExitCode::PermissionDenied => "Permission denied",
            ExitCode::ResourceUnavailable => "Resource unavailable",
            ExitCode::Timeout => "Operation timed out",
            ExitCode::PartialSuccess => "Partial success",
            ExitCode::Cancelled => "Operation cancelled",
        }
    }

    /// 获取 AI 处理建议
    pub fn ai_suggestion(&self) -> &'static str {
        match self {
            ExitCode::Success => "Continue to next step",
            ExitCode::GeneralError => "Check stderr for details, retry or report",
            ExitCode::NeedConfirmation => "Request user input or use --force flag",
            ExitCode::ConfigMissing => "Run 'cis init' to initialize configuration",
            ExitCode::NetworkError => "Wait and retry, check network connectivity",
            ExitCode::PermissionDenied => "Request elevated privileges or adjust permissions",
            ExitCode::ResourceUnavailable => "Check dependent services are running",
            ExitCode::Timeout => "Increase timeout or retry with smaller batch",
            ExitCode::PartialSuccess => "Check failed items in output, retry failed only",
            ExitCode::Cancelled => "Operation was cancelled by user",
        }
    }

    /// 是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, ExitCode::Success)
    }

    /// 是否可以自动修复
    pub fn is_auto_fixable(&self) -> bool {
        matches!(
            self,
            ExitCode::ConfigMissing | ExitCode::NetworkError | ExitCode::Timeout
        )
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.as_i32(), self.description())
    }
}

/// CLI 结果包装器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliResult<T> {
    /// 状态
    pub status: String,
    /// 退出码
    pub code: i32,
    /// 数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CliError>,
    /// 元数据
    pub meta: MetaData,
}

/// CLI 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliError {
    /// 错误类型
    pub error_type: String,
    /// 错误消息
    pub message: String,
    /// 建议修复方法
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// 是否可自动修复
    pub auto_fixable: bool,
    /// 修复命令（如果可自动修复）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_command: Option<String>,
}

/// 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaData {
    /// 命令
    pub command: String,
    /// 时间戳
    pub timestamp: String,
    /// 执行时长（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// 版本
    pub version: String,
}

impl MetaData {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: None,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

impl<T: Serialize> CliResult<T> {
    /// 创建成功结果
    pub fn success(data: T, command: impl Into<String>) -> Self {
        Self {
            status: "success".to_string(),
            code: ExitCode::Success.as_i32(),
            data: Some(data),
            error: None,
            meta: MetaData::new(command),
        }
    }

    /// 创建错误结果
    pub fn error(
        code: ExitCode,
        error_type: impl Into<String>,
        message: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let error = CliError {
            error_type: error_type.into(),
            message: message.into(),
            suggestion: Some(code.ai_suggestion().to_string()),
            auto_fixable: code.is_auto_fixable(),
            fix_command: if code.is_auto_fixable() {
                Some(format!("cis fix --code {}", code.as_i32()))
            } else {
                None
            },
        };

        Self {
            status: "error".to_string(),
            code: code.as_i32(),
            data: None,
            error: Some(error),
            meta: MetaData::new(command),
        }
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
    }

    /// 打印到 stdout
    pub fn print(&self, format: OutputFormat) {
        match format {
            OutputFormat::Json | OutputFormat::NdJson => {
                println!("{}", self.to_json());
            }
            OutputFormat::Human => {
                if self.code == ExitCode::Success.as_i32() {
                    println!("✓ Success");
                } else {
                    println!("✗ Error (code: {})", self.code);
                    if let Some(ref error) = self.error {
                        println!("  Type: {}", error.error_type);
                        println!("  Message: {}", error.message);
                        if let Some(ref suggestion) = error.suggestion {
                            println!("  Suggestion: {}", suggestion);
                        }
                    }
                }
            }
            OutputFormat::Quiet => {
                // 静默模式仅输出错误信息
                if self.code != ExitCode::Success.as_i32() {
                    if let Some(ref error) = self.error {
                        eprintln!("Error: {}", error.message);
                    }
                }
            }
        }
    }
}

/// 流式输出事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum StreamEvent {
    /// 状态更新
    #[serde(rename = "status")]
    Status { state: String, progress: f32 },
    /// 输出数据
    #[serde(rename = "output")]
    Output { task_id: String, stdout: String, stderr: String },
    /// 完成事件
    #[serde(rename = "complete")]
    Complete { state: String, exit_code: i32 },
    /// 错误事件
    #[serde(rename = "error")]
    Error { message: String, code: i32 },
}

impl StreamEvent {
    /// 格式化为 SSE（Server-Sent Events）格式
    pub fn to_sse(&self) -> String {
        let data = serde_json::to_string(self).unwrap_or_default();
        format!("event: {}\ndata: {}\n\n", self.event_name(), data)
    }

    fn event_name(&self) -> &'static str {
        match self {
            StreamEvent::Status { .. } => "status",
            StreamEvent::Output { .. } => "output",
            StreamEvent::Complete { .. } => "complete",
            StreamEvent::Error { .. } => "error",
        }
    }
}

/// 执行上下文（用于保持命令间状态）
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 会话 ID
    pub session_id: Option<String>,
    /// 项目路径
    pub project_path: Option<std::path::PathBuf>,
    /// 环境变量
    pub env_vars: std::collections::HashMap<String, String>,
    /// 是否批处理模式
    pub batch_mode: bool,
    /// 输出格式
    pub output_format: OutputFormat,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            session_id: None,
            project_path: None,
            env_vars: std::collections::HashMap::new(),
            batch_mode: false,
            output_format: OutputFormat::Human,
        }
    }
}

impl ExecutionContext {
    /// 从环境检测
    pub fn detect() -> Self {
        let mut ctx = Self::default();

        // 检测批处理模式
        if std::env::var("CI").is_ok() || !atty::is(atty::Stream::Stdout) {
            ctx.batch_mode = true;
            ctx.output_format = OutputFormat::Json;
        }

        // 检测项目路径
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join(".cis").exists() {
                ctx.project_path = Some(cwd);
            }
        }

        ctx
    }

    /// 设置输出格式
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code() {
        assert_eq!(ExitCode::Success.as_i32(), 0);
        assert_eq!(ExitCode::ConfigMissing.as_i32(), 3);
        assert!(ExitCode::Success.is_success());
        assert!(!ExitCode::GeneralError.is_success());
    }

    #[test]
    fn test_exit_code_from_i32() {
        assert_eq!(ExitCode::from_i32(0), ExitCode::Success);
        assert_eq!(ExitCode::from_i32(3), ExitCode::ConfigMissing);
        assert_eq!(ExitCode::from_i32(999), ExitCode::GeneralError);
    }

    #[test]
    fn test_cli_result_success() {
        let result = CliResult::success("test data", "cis test");
        assert_eq!(result.status, "success");
        assert_eq!(result.code, 0);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_cli_result_error() {
        let result = CliResult::<()>::error(
            ExitCode::ConfigMissing,
            "ConfigMissing",
            "Config not found",
            "cis test",
        );
        assert_eq!(result.status, "error");
        assert_eq!(result.code, 3);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_stream_event() {
        let event = StreamEvent::Status {
            state: "running".to_string(),
            progress: 0.5,
        };
        let sse = event.to_sse();
        assert!(sse.contains("event: status"));
        assert!(sse.contains("running"));
    }
}
