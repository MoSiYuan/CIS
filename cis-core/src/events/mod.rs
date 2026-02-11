//! # 事件模块
//!
//! 定义 CIS 系统中使用的所有领域事件。
//!
//! ## 设计原则
//!
//! - **显式通信**: 每个事件都有明确的发送者和接收者
//! - **可追溯**: 所有事件包含时间戳和事件 ID
//! - **可序列化**: 支持 JSON 序列化用于持久化和网络传输
//! - **类型安全**: 使用强类型事件，避免字符串魔法值
//!
//! ## 事件类型
//!
//! | 事件 | 类型 | 说明 |
//! |------|------|------|
//! | `RoomMessageEvent` | 房间消息 | Matrix 房间收到消息 |
//! | `SkillExecuteEvent` | Skill 执行请求 | 请求执行 Skill |
//! | `SkillCompletedEvent` | Skill 执行完成 | Skill 执行结果 |
//! | `AgentOnlineEvent` | Agent 上线 | 节点 Agent 上线通知 |
//! | `FederationTaskEvent` | 联邦任务 | 跨节点任务分发 |

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

pub mod domain;

pub use domain::*;

/// 事件包装器 - 用于序列化和传输
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventWrapper {
    /// 房间消息事件
    RoomMessage(RoomMessageEvent),
    /// Skill 执行请求事件
    SkillExecute(SkillExecuteEvent),
    /// Skill 执行完成事件
    SkillCompleted(SkillCompletedEvent),
    /// Agent 上线事件
    AgentOnline(AgentOnlineEvent),
    /// 联邦任务事件
    FederationTask(FederationTaskEvent),
}

impl EventWrapper {
    /// 获取事件类型字符串
    pub fn event_type(&self) -> &'static str {
        match self {
            EventWrapper::RoomMessage(_) => "room.message",
            EventWrapper::SkillExecute(_) => "skill.execute",
            EventWrapper::SkillCompleted(_) => "skill.completed",
            EventWrapper::AgentOnline(_) => "agent.online",
            EventWrapper::FederationTask(_) => "federation.task",
        }
    }

    /// 获取事件 ID
    pub fn event_id(&self) -> &str {
        match self {
            EventWrapper::RoomMessage(e) => &e.event_id,
            EventWrapper::SkillExecute(e) => &e.event_id,
            EventWrapper::SkillCompleted(e) => &e.event_id,
            EventWrapper::AgentOnline(e) => &e.event_id,
            EventWrapper::FederationTask(e) => &e.event_id,
        }
    }

    /// 获取事件时间戳
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            EventWrapper::RoomMessage(e) => e.timestamp,
            EventWrapper::SkillExecute(e) => e.timestamp,
            EventWrapper::SkillCompleted(e) => e.timestamp,
            EventWrapper::AgentOnline(e) => e.timestamp,
            EventWrapper::FederationTask(e) => e.timestamp,
        }
    }
}

/// 消息内容类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "msgtype")]
pub enum MessageContent {
    /// 文本消息
    #[serde(rename = "m.text")]
    Text { body: String },
    /// 图片消息
    #[serde(rename = "m.image")]
    Image { url: String, info: Option<ImageInfo> },
    /// 文件消息
    #[serde(rename = "m.file")]
    File { url: String, filename: String, info: Option<FileInfo> },
    /// 代码消息
    #[serde(rename = "m.code")]
    Code { body: String, language: Option<String> },
}

/// 图片信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub mimetype: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub mimetype: Option<String>,
    pub size: Option<u64>,
}

/// 执行上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// 请求者 ID
    pub requester_id: String,
    /// 房间 ID（如果是从房间触发的）
    pub room_id: Option<String>,
    /// 消息 ID（如果是从消息触发的）
    pub message_id: Option<String>,
    /// 会话 ID
    pub session_id: Option<String>,
    /// 额外参数
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl ExecutionContext {
    /// 从房间消息创建执行上下文
    pub fn from_room(room_id: impl Into<String>, sender: impl Into<String>) -> Self {
        Self {
            requester_id: sender.into(),
            room_id: Some(room_id.into()),
            message_id: None,
            session_id: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// 设置消息 ID
    pub fn with_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    /// 设置会话 ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 输出数据
    pub output: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 执行日志
    pub logs: Vec<String>,
}

impl ExecutionResult {
    /// 创建成功结果
    pub fn success(output: impl Into<serde_json::Value>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            duration_ms: 0,
            logs: Vec::new(),
        }
    }

    /// 创建失败结果
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            duration_ms: 0,
            logs: Vec::new(),
        }
    }

    /// 设置执行耗时
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// 添加日志
    pub fn with_logs(mut self, logs: Vec<String>) -> Self {
        self.logs = logs;
        self
    }
}

/// 能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// 能力 ID
    pub id: String,
    /// 能力名称
    pub name: String,
    /// 能力类型
    pub capability_type: String,
    /// 参数 schema
    pub parameters: Option<serde_json::Value>,
}

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// 任务类型
    pub task_type: String,
    /// 任务参数
    pub parameters: serde_json::Value,
    /// 优先级
    pub priority: u8,
    /// 超时时间（秒）
    pub timeout_secs: u32,
}

/// 事件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// 发送者 ID
    pub sender_id: String,
    /// 接收者 ID（可选，为空表示广播）
    pub recipient_id: Option<String>,
    /// 源节点 ID
    pub source_node: Option<String>,
    /// 目标节点 ID
    pub target_node: Option<String>,
    /// 相关事件 ID（用于追踪事件链）
    pub correlation_id: Option<String>,
}

impl EventMetadata {
    /// 创建新的元数据
    pub fn new(sender_id: impl Into<String>) -> Self {
        Self {
            sender_id: sender_id.into(),
            recipient_id: None,
            source_node: None,
            target_node: None,
            correlation_id: None,
        }
    }

    /// 设置接收者
    pub fn with_recipient(mut self, recipient_id: impl Into<String>) -> Self {
        self.recipient_id = Some(recipient_id.into());
        self
    }

    /// 设置源节点
    pub fn with_source_node(mut self, node_id: impl Into<String>) -> Self {
        self.source_node = Some(node_id.into());
        self
    }

    /// 设置目标节点
    pub fn with_target_node(mut self, node_id: impl Into<String>) -> Self {
        self.target_node = Some(node_id.into());
        self
    }

    /// 设置关联 ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_wrapper_event_type() {
        let event = RoomMessageEvent {
            event_id: "test-1".to_string(),
            timestamp: Utc::now(),
            room_id: "room-1".to_string(),
            sender: "user-1".to_string(),
            content: MessageContent::Text { body: "Hello".to_string() },
            metadata: EventMetadata::new("matrix-bridge"),
        };
        
        let wrapper = EventWrapper::RoomMessage(event);
        assert_eq!(wrapper.event_type(), "room.message");
    }

    #[test]
    fn test_execution_context_builder() {
        let ctx = ExecutionContext::from_room("room-1", "user-1")
            .with_message_id("msg-1")
            .with_session_id("session-1");
        
        assert_eq!(ctx.requester_id, "user-1");
        assert_eq!(ctx.room_id, Some("room-1".to_string()));
        assert_eq!(ctx.message_id, Some("msg-1".to_string()));
        assert_eq!(ctx.session_id, Some("session-1".to_string()));
    }

    #[test]
    fn test_execution_result_builder() {
        let result = ExecutionResult::success(json!({"key": "value"}))
            .with_duration(100)
            .with_logs(vec!["log1".to_string(), "log2".to_string()]);
        
        assert!(result.success);
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.logs.len(), 2);
    }

    #[test]
    fn test_event_metadata_builder() {
        let meta = EventMetadata::new("sender-1")
            .with_recipient("recipient-1")
            .with_source_node("node-1")
            .with_target_node("node-2")
            .with_correlation_id("corr-1");
        
        assert_eq!(meta.sender_id, "sender-1");
        assert_eq!(meta.recipient_id, Some("recipient-1".to_string()));
        assert_eq!(meta.source_node, Some("node-1".to_string()));
        assert_eq!(meta.target_node, Some("node-2".to_string()));
        assert_eq!(meta.correlation_id, Some("corr-1".to_string()));
    }
}
