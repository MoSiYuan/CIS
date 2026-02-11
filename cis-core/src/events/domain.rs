//! # 领域事件定义
//!
//! 定义 CIS 系统中的核心业务领域事件。
//!
//! ## 事件设计原则
//!
//! 1. **不变性**: 事件一旦创建不可修改
//! 2. **完整性**: 包含事件处理所需的全部信息
//! 3. **可追溯**: 每个事件都有唯一 ID 和时间戳
//! 4. **显式边界**: 明确标识发送者和接收者

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::events::{MessageContent, ExecutionContext, ExecutionResult, Capability, Task, EventMetadata};

/// 房间消息事件
/// 
/// 当 Matrix 房间收到新消息时触发
/// 
/// ## 路由
/// - **发布者**: Matrix Bridge
/// - **订阅者**: Skill Handler, Logger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessageEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 房间 ID
    pub room_id: String,
    /// 发送者 ID
    pub sender: String,
    /// 消息内容
    pub content: MessageContent,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl RoomMessageEvent {
    /// 创建新的房间消息事件
    pub fn new(
        room_id: impl Into<String>,
        sender: impl Into<String>,
        content: MessageContent,
    ) -> Self {
        let room_id = room_id.into();
        let sender = sender.into();
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            room_id: room_id.clone(),
            sender: sender.clone(),
            content,
            metadata: EventMetadata::new("matrix-bridge")
                .with_recipient("skill-handler"),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "room.message"
    }

    /// 获取文本内容（如果是文本消息）
    pub fn text_body(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text { body } => Some(body),
            _ => None,
        }
    }

    /// 是否是命令消息（以 ! 开头）
    pub fn is_command(&self) -> bool {
        self.text_body()
            .map(|body| body.trim().starts_with('!'))
            .unwrap_or(false)
    }

    /// 提取命令名称（不包括 ! 前缀）
    pub fn command_name(&self) -> Option<String> {
        self.text_body()
            .and_then(|body| {
                let trimmed = body.trim();
                if trimmed.starts_with('!') {
                    let without_prefix = &trimmed[1..];
                    without_prefix.split_whitespace().next()
                        .map(|s| s.to_lowercase())
                } else {
                    None
                }
            })
    }
}

/// Skill 执行请求事件
///
/// 当需要执行某个 Skill 时触发
///
/// ## 路由
/// - **发布者**: Skill Handler, Agent
/// - **订阅者**: Skill Executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecuteEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// Skill 名称
    pub skill_name: String,
    /// 执行方法
    pub method: String,
    /// 执行参数（JSON 序列化）
    pub params: serde_json::Value,
    /// 请求者 ID
    pub requester: String,
    /// 执行上下文
    pub context: ExecutionContext,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl SkillExecuteEvent {
    /// 创建新的 Skill 执行事件
    pub fn new(
        skill_name: impl Into<String>,
        method: impl Into<String>,
        params: impl Into<serde_json::Value>,
        requester: impl Into<String>,
        context: ExecutionContext,
    ) -> Self {
        let skill_name = skill_name.into();
        let requester = requester.into();
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            skill_name: skill_name.clone(),
            method: method.into(),
            params: params.into(),
            requester: requester.clone(),
            context,
            metadata: EventMetadata::new(requester)
                .with_recipient("skill-executor")
                .with_correlation_id(&skill_name),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "skill.execute"
    }

    /// 从房间消息创建执行事件
    pub fn from_room_message(
        skill_name: impl Into<String>,
        event: &RoomMessageEvent,
        params: impl Into<serde_json::Value>,
    ) -> Self {
        let skill_name = skill_name.into();
        let context = ExecutionContext::from_room(&event.room_id, &event.sender)
            .with_message_id(&event.event_id);
        
        Self::new(
            skill_name.clone(),
            "execute",
            params,
            &event.sender,
            context,
        )
    }
}

/// Skill 执行完成事件
///
/// 当 Skill 执行完成时触发
///
/// ## 路由
/// - **发布者**: Skill Executor
/// - **订阅者**: Matrix Bridge, Logger, Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCompletedEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 原始执行事件 ID
    pub original_event_id: String,
    /// Skill 名称
    pub skill_name: String,
    /// 执行结果
    pub result: ExecutionResult,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl SkillCompletedEvent {
    /// 创建新的 Skill 完成事件
    pub fn new(
        original_event_id: impl Into<String>,
        skill_name: impl Into<String>,
        result: ExecutionResult,
        executor_id: impl Into<String>,
    ) -> Self {
        let original_event_id = original_event_id.into();
        let skill_name = skill_name.into();
        let duration_ms = result.duration_ms;
        
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            original_event_id: original_event_id.clone(),
            skill_name: skill_name.clone(),
            result,
            duration_ms,
            metadata: EventMetadata::new(executor_id)
                .with_recipient("matrix-bridge")
                .with_correlation_id(&original_event_id),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "skill.completed"
    }

    /// 是否执行成功
    pub fn is_success(&self) -> bool {
        self.result.success
    }

    /// 获取错误信息（如果失败）
    pub fn error_message(&self) -> Option<&str> {
        self.result.error.as_deref()
    }
}

/// Agent 上线事件
///
/// 当 Agent 节点上线时触发
///
/// ## 路由
/// - **发布者**: Agent Manager
/// - **订阅者**: Federation, Discovery Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOnlineEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 节点 ID
    pub node_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Agent 能力列表
    pub capabilities: Vec<Capability>,
    /// 网络地址
    pub address: Option<String>,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl AgentOnlineEvent {
    /// 创建新的 Agent 上线事件
    pub fn new(
        node_id: impl Into<String>,
        agent_id: impl Into<String>,
        capabilities: Vec<Capability>,
    ) -> Self {
        let node_id = node_id.into();
        let agent_id = agent_id.into();
        
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            node_id: node_id.clone(),
            agent_id: agent_id.clone(),
            capabilities,
            address: None,
            metadata: EventMetadata::new(&agent_id)
                .with_source_node(&node_id)
                .with_recipient("federation"),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "agent.online"
    }

    /// 设置网络地址
    pub fn with_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    /// 检查是否支持指定能力
    pub fn has_capability(&self, capability_id: &str) -> bool {
        self.capabilities.iter().any(|c| c.id == capability_id)
    }
}

/// 联邦任务事件
///
/// 跨节点任务分发事件
///
/// ## 路由
/// - **发布者**: Federation
/// - **订阅者**: Agent Executor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationTaskEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 任务 ID
    pub task_id: String,
    /// 源节点 ID
    pub from_node: String,
    /// 目标节点 ID
    pub to_node: String,
    /// 任务定义
    pub task: Task,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl FederationTaskEvent {
    /// 创建新的联邦任务事件
    pub fn new(
        task_id: impl Into<String>,
        from_node: impl Into<String>,
        to_node: impl Into<String>,
        task: Task,
    ) -> Self {
        let task_id = task_id.into();
        let from_node = from_node.into();
        let to_node = to_node.into();
        
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            task_id: task_id.clone(),
            from_node: from_node.clone(),
            to_node: to_node.clone(),
            task,
            metadata: EventMetadata::new(&from_node)
                .with_source_node(&from_node)
                .with_target_node(&to_node)
                .with_recipient(&to_node)
                .with_correlation_id(&task_id),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "federation.task"
    }

    /// 是否是本地任务（源节点和目标节点相同）
    pub fn is_local(&self) -> bool {
        self.from_node == self.to_node
    }
}

/// Skill 注册事件
///
/// 当有新 Skill 注册到系统时触发
///
/// ## 路由
/// - **发布者**: Skill Registry
/// - **订阅者**: Skill Router, Logger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRegisteredEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// Skill 名称
    pub skill_name: String,
    /// Skill 版本
    pub version: String,
    /// Skill 描述
    pub description: Option<String>,
    /// 提供者
    pub provider: String,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

impl SkillRegisteredEvent {
    /// 创建新的 Skill 注册事件
    pub fn new(
        skill_name: impl Into<String>,
        version: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        let skill_name = skill_name.into();
        let provider = provider.into();
        
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            skill_name: skill_name.clone(),
            version: version.into(),
            description: None,
            provider: provider.clone(),
            metadata: EventMetadata::new(&provider)
                .with_recipient("skill-router"),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "skill.registered"
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// 系统事件
///
/// 系统级别的事件通知
///
/// ## 路由
/// - **发布者**: 各个子系统
/// - **订阅者**: Logger, Monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// 事件唯一 ID
    pub event_id: String,
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 事件级别
    pub level: SystemEventLevel,
    /// 事件类别
    pub category: String,
    /// 事件消息
    pub message: String,
    /// 详细数据
    pub details: Option<serde_json::Value>,
    /// 事件元数据
    #[serde(flatten)]
    pub metadata: EventMetadata,
}

/// 系统事件级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SystemEventLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl SystemEvent {
    /// 创建新的系统事件
    pub fn new(
        level: SystemEventLevel,
        category: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            event_id: format!("evt_{}", uuid::Uuid::new_v4()),
            timestamp: Utc::now(),
            level,
            category: category.into(),
            message: message.into(),
            details: None,
            metadata: EventMetadata::new(source)
                .with_recipient("logger"),
        }
    }

    /// 获取事件类型
    pub fn event_type(&self) -> &'static str {
        "system.event"
    }

    /// 设置详细数据
    pub fn with_details(mut self, details: impl Into<serde_json::Value>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// 创建 Info 级别事件
    pub fn info(category: impl Into<String>, message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SystemEventLevel::Info, category, message, source)
    }

    /// 创建 Warning 级别事件
    pub fn warning(category: impl Into<String>, message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SystemEventLevel::Warning, category, message, source)
    }

    /// 创建 Error 级别事件
    pub fn error(category: impl Into<String>, message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(SystemEventLevel::Error, category, message, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::MessageContent;
    use serde_json::json;

    #[test]
    fn test_room_message_event_command_detection() {
        let text_event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "!help arg1 arg2".to_string() },
        );
        
        assert!(text_event.is_command());
        assert_eq!(text_event.command_name(), Some("help".to_string()));
    }

    #[test]
    fn test_room_message_event_not_command() {
        let text_event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "Hello world".to_string() },
        );
        
        assert!(!text_event.is_command());
        assert_eq!(text_event.command_name(), None);
    }

    #[test]
    fn test_skill_execute_event_from_room_message() {
        let room_event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "!echo hello".to_string() },
        );
        
        let execute_event = SkillExecuteEvent::from_room_message(
            "echo",
            &room_event,
            json!({"text": "hello"}),
        );
        
        assert_eq!(execute_event.skill_name, "echo");
        assert_eq!(execute_event.requester, "user-1");
        assert_eq!(execute_event.context.room_id, Some("room-1".to_string()));
    }

    #[test]
    fn test_skill_completed_event_success() {
        let result = ExecutionResult::success(json!({"output": "test"}))
            .with_duration(100);
        
        let event = SkillCompletedEvent::new(
            "original-1",
            "test-skill",
            result,
            "executor-1",
        );
        
        assert!(event.is_success());
        assert_eq!(event.duration_ms, 100);
        assert_eq!(event.error_message(), None);
    }

    #[test]
    fn test_agent_online_capability_check() {
        let caps = vec![
            Capability {
                id: "skill.echo".to_string(),
                name: "Echo".to_string(),
                capability_type: "skill".to_string(),
                parameters: None,
            },
            Capability {
                id: "skill.chat".to_string(),
                name: "Chat".to_string(),
                capability_type: "skill".to_string(),
                parameters: None,
            },
        ];
        
        let event = AgentOnlineEvent::new("node-1", "agent-1", caps);
        
        assert!(event.has_capability("skill.echo"));
        assert!(!event.has_capability("skill.unknown"));
    }

    #[test]
    fn test_federation_task_local_check() {
        let task = Task {
            task_type: "compute".to_string(),
            parameters: json!({}),
            priority: 1,
            timeout_secs: 60,
        };
        
        let local_event = FederationTaskEvent::new("task-1", "node-1", "node-1", task.clone());
        assert!(local_event.is_local());
        
        let remote_event = FederationTaskEvent::new("task-1", "node-1", "node-2", task);
        assert!(!remote_event.is_local());
    }

    #[test]
    fn test_system_event_helpers() {
        let info = SystemEvent::info("test", "info message", "source");
        assert_eq!(info.level, SystemEventLevel::Info);
        
        let warning = SystemEvent::warning("test", "warning message", "source");
        assert_eq!(warning.level, SystemEventLevel::Warning);
        
        let error = SystemEvent::error("test", "error message", "source");
        assert_eq!(error.level, SystemEventLevel::Error);
    }
}
