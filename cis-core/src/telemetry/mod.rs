//! 系统遥测和日志模块
//! 
//! RequestLogger 记录完整的请求处理链路，与私域记忆分离存储。
//! 
//! # 架构
//! 
//! ```
//! 用户输入 → IntentParser → SkillRouter → Skill执行 → 反馈报告
//!     │              │            │           │           │
//!     └──────────────┴────────────┴───────────┴───────────┘
//!                          │
//!                    RequestLogger
//!                          │
//!                  SQLite (telemetry.db)
//! ```
//! 
//! # 与私域记忆的区别
//! 
//! | 特性 | RequestLogger | 私域记忆 (VectorStorage) |
//! |------|--------------|------------------------|
//! | 目的 | 系统可观测性 | 用户数据存储 |
//! | 内容 | 请求链路追踪 | 语义化用户数据 |
//! | 存储 | SQLite结构化 | 向量+标量混合 |
//! | 保留 | 自动清理 | 长期保留 |
//! | 用途 | 调试/分析 | 召回/推理 |

use std::path::Path;

pub mod request_logger;

pub use request_logger::{
    LogQuery, RequestLog, RequestLogBuilder, RequestLogger, RequestMetrics, 
    RequestResult, RequestStage, SessionStats
};

/// 遥测配置
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// 日志存储路径
    pub log_path: std::path::PathBuf,
    /// 是否启用详细日志
    pub verbose: bool,
    /// 日志保留天数
    pub retention_days: u32,
    /// 采样率 (0.0-1.0)
    pub sample_rate: f32,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_path: std::path::PathBuf::from(".cis/telemetry.db"),
            verbose: true,
            retention_days: 30,
            sample_rate: 1.0,
        }
    }
}

impl TelemetryConfig {
    /// 从路径创建配置
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            log_path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }
    
    /// 设置采样率
    pub fn with_sample_rate(mut self, rate: f32) -> Self {
        self.sample_rate = rate.clamp(0.0, 1.0);
        self
    }
    
    /// 设置保留天数
    pub fn with_retention(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }
}
