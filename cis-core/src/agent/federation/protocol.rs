//! # Agent Federation Protocol
//!
//! 定义跨节点 Agent 通信的协议规范，基于 Matrix Federation (端口 7676)。
//!
//! ## 协议概述
//!
//! Agent 联邦协议允许不同 CIS 节点上的 Agent 相互通信、协调任务和共享状态。
//! 所有通信通过 Matrix Federation 事件传输，使用 CIS 自定义事件类型。
//!
//! ## 事件类型命名空间
//!
//! - `io.cis.agent.registered` - Agent 注册事件
//! - `io.cis.agent.unregistered` - Agent 注销事件
//! - `io.cis.agent.task.request` - 任务请求
//! - `io.cis.agent.task.response` - 任务响应
//! - `io.cis.agent.message` - Agent 间消息
//! - `io.cis.agent.heartbeat` - 心跳
//! - `io.cis.agent.status_update` - 状态更新
//!
//! ## Agent 地址格式
//!
//! Agent 联邦地址格式: `agent-id@node-id`
//! 示例: `claude-worker-1@kitchen.local`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::agent::persistent::{AgentStatus, RuntimeType};
use crate::error::{CisError, Result};

/// Agent 联邦事件类型
///
/// 所有跨节点 Agent 通信使用的事件类型，通过 Matrix Federation 传输。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum AgentFederationEvent {
    /// Agent 注册（上线通知）
    ///
    /// 当 Agent 启动或加入联邦时发送，通知其他节点此 Agent 可用。
    #[serde(rename = "io.cis.agent.registered")]
    AgentRegistered {
        /// Agent 唯一标识符
        agent_id: String,
        /// 所在节点 ID
        node_id: String,
        /// Runtime 类型
        runtime_type: RuntimeType,
        /// 能力列表
        capabilities: Vec<String>,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// Agent 注销（下线通知）
    ///
    /// 当 Agent 关闭或离开联邦时发送。
    #[serde(rename = "io.cis.agent.unregistered")]
    AgentUnregistered {
        /// Agent 唯一标识符
        agent_id: String,
        /// 所在节点 ID
        node_id: String,
        /// 注销原因
        reason: Option<String>,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 任务请求
    ///
    /// 一个 Agent 向另一个 Agent 发送任务执行请求。
    #[serde(rename = "io.cis.agent.task.request")]
    TaskRequest {
        /// 请求唯一标识符（用于关联响应）
        request_id: String,
        /// 发送方 Agent ID
        from_agent: String,
        /// 接收方 Agent ID
        to_agent: String,
        /// 任务负载
        task: TaskRequestPayload,
        /// 超时时间（秒）
        timeout_secs: Option<u64>,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 任务响应
    ///
    /// 对任务请求的响应。
    #[serde(rename = "io.cis.agent.task.response")]
    TaskResponse {
        /// 关联的请求 ID
        request_id: String,
        /// 发送方 Agent ID
        from_agent: String,
        /// 接收方 Agent ID
        to_agent: String,
        /// 任务结果
        result: TaskResultPayload,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 消息传递（Agent 间直接通信）
    ///
    /// 用于 Agent 间的直接消息通信，支持广播（to_agent 为 None）。
    #[serde(rename = "io.cis.agent.message")]
    Message {
        /// 消息唯一标识符
        message_id: String,
        /// 发送方 Agent ID
        from_agent: String,
        /// 接收方 Agent ID（None = 广播到所有 Agent）
        to_agent: Option<String>,
        /// 消息类型（应用层自定义）
        message_type: String,
        /// 消息负载
        payload: serde_json::Value,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 心跳
    ///
    /// 定期发送以表明 Agent 仍然活跃。
    #[serde(rename = "io.cis.agent.heartbeat")]
    Heartbeat {
        /// Agent 唯一标识符
        agent_id: String,
        /// 所在节点 ID
        node_id: String,
        /// 当前状态
        status: AgentStatus,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },

    /// 状态更新
    ///
    /// 当 Agent 状态发生变化时发送。
    #[serde(rename = "io.cis.agent.status_update")]
    StatusUpdate {
        /// Agent 唯一标识符
        agent_id: String,
        /// 所在节点 ID
        node_id: String,
        /// 当前状态
        status: AgentStatus,
        /// 当前执行的任务 ID
        current_task: Option<String>,
        /// 时间戳
        timestamp: DateTime<Utc>,
    },
}

impl AgentFederationEvent {
    /// 获取事件类型字符串
    pub fn event_type_str(&self) -> &'static str {
        match self {
            Self::AgentRegistered { .. } => "io.cis.agent.registered",
            Self::AgentUnregistered { .. } => "io.cis.agent.unregistered",
            Self::TaskRequest { .. } => "io.cis.agent.task.request",
            Self::TaskResponse { .. } => "io.cis.agent.task.response",
            Self::Message { .. } => "io.cis.agent.message",
            Self::Heartbeat { .. } => "io.cis.agent.heartbeat",
            Self::StatusUpdate { .. } => "io.cis.agent.status_update",
        }
    }

    /// 获取发送方 Agent ID（如果有）
    pub fn from_agent(&self) -> Option<&str> {
        match self {
            Self::TaskRequest { from_agent, .. }
            | Self::TaskResponse { from_agent, .. }
            | Self::Message { from_agent, .. } => Some(from_agent),
            _ => None,
        }
    }

    /// 获取接收方 Agent ID（如果有）
    pub fn to_agent(&self) -> Option<&str> {
        match self {
            Self::TaskRequest { to_agent, .. }
            | Self::TaskResponse { to_agent, .. } => Some(to_agent),
            Self::Message { to_agent, .. } => to_agent.as_deref(),
            _ => None,
        }
    }

    /// 获取时间戳
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::AgentRegistered { timestamp, .. }
            | Self::AgentUnregistered { timestamp, .. }
            | Self::TaskRequest { timestamp, .. }
            | Self::TaskResponse { timestamp, .. }
            | Self::Message { timestamp, .. }
            | Self::Heartbeat { timestamp, .. }
            | Self::StatusUpdate { timestamp, .. } => *timestamp,
        }
    }

    /// 创建 Agent 注册事件
    pub fn registered(
        agent_id: impl Into<String>,
        node_id: impl Into<String>,
        runtime_type: RuntimeType,
        capabilities: Vec<String>,
    ) -> Self {
        Self::AgentRegistered {
            agent_id: agent_id.into(),
            node_id: node_id.into(),
            runtime_type,
            capabilities,
            timestamp: Utc::now(),
        }
    }

    /// 创建 Agent 注销事件
    pub fn unregistered(
        agent_id: impl Into<String>,
        node_id: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self::AgentUnregistered {
            agent_id: agent_id.into(),
            node_id: node_id.into(),
            reason,
            timestamp: Utc::now(),
        }
    }

    /// 创建任务请求事件
    pub fn task_request(
        request_id: impl Into<String>,
        from_agent: impl Into<String>,
        to_agent: impl Into<String>,
        task: TaskRequestPayload,
        timeout_secs: Option<u64>,
    ) -> Self {
        Self::TaskRequest {
            request_id: request_id.into(),
            from_agent: from_agent.into(),
            to_agent: to_agent.into(),
            task,
            timeout_secs,
            timestamp: Utc::now(),
        }
    }

    /// 创建任务响应事件
    pub fn task_response(
        request_id: impl Into<String>,
        from_agent: impl Into<String>,
        to_agent: impl Into<String>,
        result: TaskResultPayload,
    ) -> Self {
        Self::TaskResponse {
            request_id: request_id.into(),
            from_agent: from_agent.into(),
            to_agent: to_agent.into(),
            result,
            timestamp: Utc::now(),
        }
    }

    /// 创建消息事件
    pub fn message(
        message_id: impl Into<String>,
        from_agent: impl Into<String>,
        to_agent: Option<String>,
        message_type: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self::Message {
            message_id: message_id.into(),
            from_agent: from_agent.into(),
            to_agent,
            message_type: message_type.into(),
            payload,
            timestamp: Utc::now(),
        }
    }

    /// 创建心跳事件
    pub fn heartbeat(
        agent_id: impl Into<String>,
        node_id: impl Into<String>,
        status: AgentStatus,
    ) -> Self {
        Self::Heartbeat {
            agent_id: agent_id.into(),
            node_id: node_id.into(),
            status,
            timestamp: Utc::now(),
        }
    }

    /// 创建状态更新事件
    pub fn status_update(
        agent_id: impl Into<String>,
        node_id: impl Into<String>,
        status: AgentStatus,
        current_task: Option<String>,
    ) -> Self {
        Self::StatusUpdate {
            agent_id: agent_id.into(),
            node_id: node_id.into(),
            status,
            current_task,
            timestamp: Utc::now(),
        }
    }
}

/// 任务请求负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequestPayload {
    /// 任务 ID
    pub task_id: String,
    /// 任务描述/Prompt
    pub prompt: String,
    /// 上下文信息
    pub context: String,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 指定使用的模型
    pub model: Option<String>,
    /// 额外元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskRequestPayload {
    /// 创建新的任务请求负载
    pub fn new(task_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            prompt: prompt.into(),
            context: String::new(),
            system_prompt: None,
            model: None,
            metadata: HashMap::new(),
        }
    }

    /// 设置上下文
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }

    /// 设置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> anyhow::Result<Self> {
        let value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), value);
        Ok(self)
    }
}

/// 任务结果负载
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    /// 是否成功
    pub success: bool,
    /// 输出内容
    pub output: String,
    /// 退出码
    pub exit_code: i32,
    /// 额外元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskResultPayload {
    /// 创建成功的任务结果
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            exit_code: 0,
            metadata: HashMap::new(),
        }
    }

    /// 创建失败的任务结果
    pub fn error(output: impl Into<String>, exit_code: i32) -> Self {
        Self {
            success: false,
            output: output.into(),
            exit_code,
            metadata: HashMap::new(),
        }
    }

    /// 添加元数据
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Serialize,
    ) -> anyhow::Result<Self> {
        let value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), value);
        Ok(self)
    }
}

/// Agent 联邦地址
///
/// 格式: `agent-id@node-id`
///
/// # 示例
/// ```
/// use cis_core::agent::federation::AgentAddress;
///
/// let addr = AgentAddress::new("worker-1", "kitchen.local");
/// assert_eq!(addr.to_string(), "worker-1@kitchen.local");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentAddress {
    /// Agent ID
    pub agent_id: String,
    /// 节点 ID
    pub node_id: String,
}

impl AgentAddress {
    /// 创建新的 Agent 地址
    pub fn new(agent_id: impl Into<String>, node_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            node_id: node_id.into(),
        }
    }

    /// 解析地址字符串
    ///
    /// 格式: `"agent-id@node-id"`
    ///
    /// # 错误
    /// 如果格式无效，返回 `CisError::InvalidInput`
    pub fn parse(addr: &str) -> Result<Self> {
        let parts: Vec<&str> = addr.split('@').collect();
        if parts.len() != 2 {
            return Err(CisError::invalid_input(format!(
                "Invalid agent address: {}. Expected format: agent-id@node-id",
                addr
            )));
        }
        Ok(Self::new(parts[0], parts[1]))
    }

    /// 转换为字符串
    pub fn to_string(&self) -> String {
        format!("{}@{}", self.agent_id, self.node_id)
    }

    /// 检查是否为本地 Agent
    pub fn is_local(&self, local_node_id: &str) -> bool {
        self.node_id == local_node_id
    }

    /// 获取 Agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

impl std::fmt::Display for AgentAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.agent_id, self.node_id)
    }
}

/// Agent 联邦 Room 配置
///
/// 定义 Agent 联邦使用的 Matrix Room 结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFederationRoom {
    /// Room ID
    pub room_id: String,
    /// 是否联邦同步
    pub federate: bool,
    /// 节点成员列表
    pub members: Vec<String>, // node_ids
}

impl AgentFederationRoom {
    /// 创建新的 Agent 联邦 Room
    pub fn new(room_id: impl Into<String>, federate: bool) -> Self {
        Self {
            room_id: room_id.into(),
            federate,
            members: Vec::new(),
        }
    }

    /// 创建 Agent 联邦 Room ID
    ///
    /// 格式: `!agent-federation-{namespace}:{local_node}`
    pub fn create_room_id(namespace: &str, local_node: &str) -> String {
        format!("!agent-federation-{}:{}", namespace, local_node)
    }

    /// 创建广播 Room（所有节点）
    ///
    /// 格式: `!agent-broadcast:{local_node}`
    pub fn broadcast_room(local_node: &str) -> String {
        format!("!agent-broadcast:{}", local_node)
    }

    /// 创建默认 Agent 联邦 Room
    pub fn default_room(local_node: &str) -> String {
        Self::create_room_id("default", local_node)
    }

    /// 添加成员节点
    pub fn add_member(&mut self, node_id: impl Into<String>) {
        let node_id = node_id.into();
        if !self.members.contains(&node_id) {
            self.members.push(node_id);
        }
    }

    /// 移除成员节点
    pub fn remove_member(&mut self, node_id: &str) {
        self.members.retain(|m| m != node_id);
    }
}

/// Agent 路由目标
#[derive(Debug, Clone)]
pub enum AgentRoute {
    /// 本地 Agent
    Local,
    /// 远程 Agent（需要通过 Federation 发送）
    Remote {
        /// 目标节点 ID
        node_id: String,
    },
    /// 未知 Agent
    Unknown,
}

impl AgentRoute {
    /// 检查是否为本地路由
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// 检查是否为远程路由
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    /// 检查是否未知
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// 获取远程节点 ID（如果是远程路由）
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Self::Remote { node_id } => Some(node_id),
            _ => None,
        }
    }
}

/// Agent 路由表
///
/// 维护本地和远程 Agent 的路由信息。
#[derive(Debug, Default)]
pub struct AgentRoutingTable {
    /// 远程 Agent 路由表
    /// agent-id -> node-id
    remote_agents: HashMap<String, String>,
    /// 节点地址表
    /// node-id -> federation URL
    node_urls: HashMap<String, String>,
}

impl AgentRoutingTable {
    /// 创建新的路由表
    pub fn new() -> Self {
        Self {
            remote_agents: HashMap::new(),
            node_urls: HashMap::new(),
        }
    }

    /// 注册远程 Agent（通过联邦事件发现）
    pub fn register_remote(&mut self, agent_id: impl Into<String>, node_id: impl Into<String>) {
        self.remote_agents
            .insert(agent_id.into(), node_id.into());
    }

    /// 注销远程 Agent
    pub fn unregister_remote(&mut self, agent_id: &str) -> bool {
        self.remote_agents.remove(agent_id).is_some()
    }

    /// 注册节点 URL
    pub fn register_node_url(
        &mut self,
        node_id: impl Into<String>,
        url: impl Into<String>,
    ) {
        self.node_urls.insert(node_id.into(), url.into());
    }

    /// 查找 Agent 路由
    ///
    /// 返回该 Agent 的路由信息。本地 Agent 检查由调用者完成。
    pub fn route(&self, agent_id: &str, local_node_id: &str) -> AgentRoute {
        if let Some(node_id) = self.remote_agents.get(agent_id) {
            if node_id == local_node_id {
                AgentRoute::Local
            } else {
                AgentRoute::Remote {
                    node_id: node_id.clone(),
                }
            }
        } else {
            AgentRoute::Unknown
        }
    }

    /// 获取节点 URL
    pub fn node_url(&self, node_id: &str) -> Option<&str> {
        self.node_urls.get(node_id).map(|s| s.as_str())
    }

    /// 列出所有已知的远程 Agent
    pub fn remote_agents(&self) -> &HashMap<String, String> {
        &self.remote_agents
    }

    /// 列出所有已知的节点
    pub fn nodes(&self) -> &HashMap<String, String> {
        &self.node_urls
    }

    /// 清理指定节点的所有 Agent 注册
    pub fn cleanup_node(&mut self, node_id: &str) {
        self.remote_agents
            .retain(|_, n| n != node_id);
    }
}

/// Agent 联邦协议常量
pub mod constants {
    /// Agent 注册事件类型
    pub const EVENT_AGENT_REGISTERED: &str = "io.cis.agent.registered";
    /// Agent 注销事件类型
    pub const EVENT_AGENT_UNREGISTERED: &str = "io.cis.agent.unregistered";
    /// 任务请求事件类型
    pub const EVENT_TASK_REQUEST: &str = "io.cis.agent.task.request";
    /// 任务响应事件类型
    pub const EVENT_TASK_RESPONSE: &str = "io.cis.agent.task.response";
    /// Agent 消息事件类型
    pub const EVENT_MESSAGE: &str = "io.cis.agent.message";
    /// 心跳事件类型
    pub const EVENT_HEARTBEAT: &str = "io.cis.agent.heartbeat";
    /// 状态更新事件类型
    pub const EVENT_STATUS_UPDATE: &str = "io.cis.agent.status_update";

    /// 默认心跳间隔（秒）
    pub const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 30;
    /// 默认任务超时（秒）
    pub const DEFAULT_TASK_TIMEOUT_SECS: u64 = 300;
    /// Agent 超时时间（秒）
    /// 如果超过这个时间没有收到心跳，认为 Agent 已离线
    pub const AGENT_TIMEOUT_SECS: u64 = 120;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_address_parsing() {
        let addr = AgentAddress::parse("worker-1@kitchen.local").unwrap();
        assert_eq!(addr.agent_id, "worker-1");
        assert_eq!(addr.node_id, "kitchen.local");
        assert!(addr.is_local("kitchen.local"));
        assert!(!addr.is_local("living.local"));
    }

    #[test]
    fn test_agent_address_formatting() {
        let addr = AgentAddress::new("worker-1", "kitchen.local");
        assert_eq!(addr.to_string(), "worker-1@kitchen.local");
    }

    #[test]
    fn test_agent_address_invalid() {
        assert!(AgentAddress::parse("invalid-address").is_err());
        assert!(AgentAddress::parse("too@many@parts").is_err());
    }

    #[test]
    fn test_room_id_creation() {
        let room_id = AgentFederationRoom::create_room_id("default", "kitchen.local");
        assert_eq!(room_id, "!agent-federation-default:kitchen.local");

        let broadcast = AgentFederationRoom::broadcast_room("kitchen.local");
        assert_eq!(broadcast, "!agent-broadcast:kitchen.local");
    }

    #[test]
    fn test_routing_table() {
        let mut table = AgentRoutingTable::new();

        // 注册远程 Agent
        table.register_remote("agent-1", "node-a");
        table.register_remote("agent-2", "node-b");

        // 测试路由
        assert!(matches!(table.route("agent-1", "node-a"), AgentRoute::Local));
        assert!(
            matches!(table.route("agent-1", "node-b"), AgentRoute::Remote { node_id } if node_id == "node-a")
        );
        assert!(matches!(table.route("unknown", "node-a"), AgentRoute::Unknown));
    }

    #[test]
    fn test_agent_federation_event_creation() {
        let event = AgentFederationEvent::registered(
            "agent-1",
            "node-a",
            RuntimeType::Claude,
            vec!["coding".to_string()],
        );

        assert_eq!(event.event_type_str(), "io.cis.agent.registered");
        if let AgentFederationEvent::AgentRegistered { agent_id, .. } = event {
            assert_eq!(agent_id, "agent-1");
        } else {
            panic!("Expected AgentRegistered event");
        }
    }

    #[test]
    fn test_task_payload() {
        let task = TaskRequestPayload::new("task-1", "Write a function")
            .with_context("Rust project")
            .with_system_prompt("You are a helpful assistant")
            .with_model("claude-3-sonnet");

        assert_eq!(task.task_id, "task-1");
        assert_eq!(task.prompt, "Write a function");
        assert_eq!(task.context, "Rust project");
        assert_eq!(task.system_prompt, Some("You are a helpful assistant".to_string()));
        assert_eq!(task.model, Some("claude-3-sonnet".to_string()));
    }

    #[test]
    fn test_task_result() {
        let result = TaskResultPayload::success("Task completed successfully");
        assert!(result.success);
        assert_eq!(result.output, "Task completed successfully");
        assert_eq!(result.exit_code, 0);

        let error = TaskResultPayload::error("Task failed", 1);
        assert!(!error.success);
        assert_eq!(error.exit_code, 1);
    }

    #[test]
    fn test_event_serialization() {
        let event = AgentFederationEvent::heartbeat("agent-1", "node-a", AgentStatus::Idle);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("io.cis.agent.heartbeat"));

        let decoded: AgentFederationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.event_type_str(), "io.cis.agent.heartbeat");
    }
}
