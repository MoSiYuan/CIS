//! 通用类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Skill 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Skill 名称（唯一标识）
    pub name: String,
    /// 版本号 (semver)
    pub version: String,
    /// 描述
    pub description: String,
    /// 作者
    pub author: String,
    /// 权限声明
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// 订阅的事件类型
    #[serde(default)]
    pub subscriptions: Vec<String>,
    /// 配置 schema
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
}

/// 权限类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// 读取记忆
    MemoryRead,
    /// 写入记忆
    MemoryWrite,
    /// 调用 AI
    AiCall,
    /// 网络访问
    Network,
    /// 文件系统访问
    FileSystem,
    /// 执行命令
    Command,
    /// 自定义权限
    Custom(String),
}

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    /// Skill 初始化
    Init {
        config: serde_json::Value,
    },
    /// Skill 关闭
    Shutdown,
    /// 定时触发
    Tick,
    /// 记忆变更
    MemoryChange {
        key: String,
        value: Vec<u8>,
        operation: MemoryOp,
    },
    /// 自定义事件
    Custom {
        name: String,
        data: serde_json::Value,
    },
}

/// 记忆操作类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOp {
    Create,
    Update,
    Delete,
}

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

/// Skill 配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillConfig {
    #[serde(flatten)]
    pub values: HashMap<String, serde_json::Value>,
}

impl SkillConfig {
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.values.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    
    pub fn set<T: Serialize>(&mut self, key: impl Into<String>, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.values.insert(key.into(), v);
        }
    }
}

/// 日志级别
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// HTTP 方法
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

/// HTTP 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Vec<u8>,
}

/// HTTP 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// 技能状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    Registered,
    Initializing,
    Active,
    Paused,
    Error,
    Unloaded,
}

/// 调用上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeContext {
    /// 调用者 ID
    pub caller: String,
    /// 调用链 ID
    pub trace_id: String,
    /// 调用时间戳
    pub timestamp: u64,
    /// 额外上下文
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}
