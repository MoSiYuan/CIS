//! # Matrix 事件类型映射
//!
//! 提供 Matrix 标准事件和 CIS 自定义事件的类型安全映射。
//!
//! ## 标准 Matrix 事件
//!
//! - `m.room.message` - 房间消息
//! - `m.room.member` - 成员变更
//! - `m.room.create` - 房间创建
//! - `m.room.power_levels` - 权限变更
//! - `m.room.join_rules` - 加入规则
//!
//! ## CIS 自定义事件
//!
//! - `cis.task.request` - 任务请求
//! - `cis.task.response` - 任务响应
//! - `cis.skill.invoke` - Skill 调用
//! - `cis.skill.result` - Skill 结果
//! - `cis.room.federate` - 联邦标记变更
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::matrix::events::{MatrixEventType, EventTypeMapper};
//!
//! // 从字符串解析事件类型
//! let event_type = EventTypeMapper::from_string("m.room.message");
//! assert_eq!(event_type, Some(MatrixEventType::RoomMessage));
//!
//! // 将事件类型转换为字符串
//! let s = EventTypeMapper::to_string(MatrixEventType::CisTaskRequest);
//! assert_eq!(s, "cis.task.request");
//! ```

use serde::{de::Visitor, Deserialize, Serialize};
use std::fmt;

/// Matrix 事件类型枚举
///
/// 包含标准 Matrix 事件和 CIS 自定义事件
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatrixEventType {
    // ==================== 标准 Matrix 事件 ====================
    /// m.room.message - 房间消息
    RoomMessage,

    /// m.room.member - 成员变更
    RoomMember,

    /// m.room.create - 房间创建
    RoomCreate,

    /// m.room.power_levels - 权限变更
    RoomPowerLevels,

    /// m.room.join_rules - 加入规则
    RoomJoinRules,

    /// m.room.name - 房间名称
    RoomName,

    /// m.room.topic - 房间主题
    RoomTopic,

    /// m.room.avatar - 房间头像
    RoomAvatar,

    /// m.room.encryption - 加密设置
    RoomEncryption,

    /// m.room.redaction - 消息撤回
    RoomRedaction,

    // ==================== CIS 自定义事件 ====================
    /// cis.task.request - 任务请求
    CisTaskRequest,

    /// cis.task.response - 任务响应
    CisTaskResponse,

    /// cis.skill.invoke - Skill 调用
    CisSkillInvoke,

    /// cis.skill.result - Skill 结果
    CisSkillResult,

    /// cis.room.federate - 联邦标记变更
    CisRoomFederate,

    /// cis.peer.discover - 节点发现
    CisPeerDiscover,

    /// cis.peer.heartbeat - 节点心跳
    CisPeerHeartbeat,
}

impl MatrixEventType {
    /// 获取事件类型的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            // 标准 Matrix 事件
            MatrixEventType::RoomMessage => "m.room.message",
            MatrixEventType::RoomMember => "m.room.member",
            MatrixEventType::RoomCreate => "m.room.create",
            MatrixEventType::RoomPowerLevels => "m.room.power_levels",
            MatrixEventType::RoomJoinRules => "m.room.join_rules",
            MatrixEventType::RoomName => "m.room.name",
            MatrixEventType::RoomTopic => "m.room.topic",
            MatrixEventType::RoomAvatar => "m.room.avatar",
            MatrixEventType::RoomEncryption => "m.room.encryption",
            MatrixEventType::RoomRedaction => "m.room.redaction",

            // CIS 自定义事件
            MatrixEventType::CisTaskRequest => "cis.task.request",
            MatrixEventType::CisTaskResponse => "cis.task.response",
            MatrixEventType::CisSkillInvoke => "cis.skill.invoke",
            MatrixEventType::CisSkillResult => "cis.skill.result",
            MatrixEventType::CisRoomFederate => "cis.room.federate",
            MatrixEventType::CisPeerDiscover => "cis.peer.discover",
            MatrixEventType::CisPeerHeartbeat => "cis.peer.heartbeat",
        }
    }

    /// 判断是否为 CIS 自定义事件
    pub fn is_cis_event(&self) -> bool {
        matches!(
            self,
            MatrixEventType::CisTaskRequest
                | MatrixEventType::CisTaskResponse
                | MatrixEventType::CisSkillInvoke
                | MatrixEventType::CisSkillResult
                | MatrixEventType::CisRoomFederate
                | MatrixEventType::CisPeerDiscover
                | MatrixEventType::CisPeerHeartbeat
        )
    }

    /// 判断是否为标准 Matrix 事件
    pub fn is_matrix_event(&self) -> bool {
        !self.is_cis_event()
    }

    /// 判断是否为状态事件
    pub fn is_state_event(&self) -> bool {
        matches!(
            self,
            MatrixEventType::RoomMember
                | MatrixEventType::RoomCreate
                | MatrixEventType::RoomPowerLevels
                | MatrixEventType::RoomJoinRules
                | MatrixEventType::RoomName
                | MatrixEventType::RoomTopic
                | MatrixEventType::RoomAvatar
                | MatrixEventType::RoomEncryption
                | MatrixEventType::CisRoomFederate
        )
    }

    /// 判断是否为消息事件
    pub fn is_message_event(&self) -> bool {
        !self.is_state_event()
    }

    /// 获取事件类别
    pub fn category(&self) -> EventCategory {
        match self {
            MatrixEventType::RoomMessage => EventCategory::Message,
            MatrixEventType::RoomMember => EventCategory::Membership,
            MatrixEventType::RoomCreate => EventCategory::Room,
            MatrixEventType::RoomPowerLevels => EventCategory::Permission,
            MatrixEventType::RoomJoinRules => EventCategory::Permission,
            MatrixEventType::RoomName => EventCategory::Room,
            MatrixEventType::RoomTopic => EventCategory::Room,
            MatrixEventType::RoomAvatar => EventCategory::Room,
            MatrixEventType::RoomEncryption => EventCategory::Encryption,
            MatrixEventType::RoomRedaction => EventCategory::Moderation,
            MatrixEventType::CisTaskRequest => EventCategory::CisTask,
            MatrixEventType::CisTaskResponse => EventCategory::CisTask,
            MatrixEventType::CisSkillInvoke => EventCategory::CisSkill,
            MatrixEventType::CisSkillResult => EventCategory::CisSkill,
            MatrixEventType::CisRoomFederate => EventCategory::CisFederation,
            MatrixEventType::CisPeerDiscover => EventCategory::CisFederation,
            MatrixEventType::CisPeerHeartbeat => EventCategory::CisFederation,
        }
    }
}

impl fmt::Display for MatrixEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl AsRef<str> for MatrixEventType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<MatrixEventType> for String {
    fn from(event_type: MatrixEventType) -> Self {
        event_type.as_str().to_string()
    }
}

impl Serialize for MatrixEventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MatrixEventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MatrixEventTypeVisitor;

        impl<'de> Visitor<'de> for MatrixEventTypeVisitor {
            type Value = MatrixEventType;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid Matrix event type string")
            }

            fn visit_str<E>(self, value: &str) -> Result<MatrixEventType, E>
            where
                E: serde::de::Error,
            {
                EventTypeMapper::from_string(value)
                    .ok_or_else(|| E::custom(format!("unknown event type: {}", value)))
            }
        }

        deserializer.deserialize_str(MatrixEventTypeVisitor)
    }
}

/// 事件类别
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventCategory {
    /// 消息事件
    Message,
    /// 成员关系事件
    Membership,
    /// 房间设置事件
    Room,
    /// 权限事件
    Permission,
    /// 加密事件
    Encryption,
    ///  moderation 事件
    Moderation,
    /// CIS 任务事件
    CisTask,
    /// CIS Skill 事件
    CisSkill,
    /// CIS 联邦事件
    CisFederation,
}

impl EventCategory {
    /// 获取类别字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            EventCategory::Message => "message",
            EventCategory::Membership => "membership",
            EventCategory::Room => "room",
            EventCategory::Permission => "permission",
            EventCategory::Encryption => "encryption",
            EventCategory::Moderation => "moderation",
            EventCategory::CisTask => "cis_task",
            EventCategory::CisSkill => "cis_skill",
            EventCategory::CisFederation => "cis_federation",
        }
    }
}

impl fmt::Display for EventCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 事件类型映射器
///
/// 提供事件类型字符串和枚举之间的双向映射
pub struct EventTypeMapper;

impl EventTypeMapper {
    /// 将事件类型枚举转换为字符串
    ///
    /// # 示例
    ///
    /// ```
    /// use cis_core::matrix::events::{MatrixEventType, EventTypeMapper};
    ///
    /// let s = EventTypeMapper::to_string(MatrixEventType::RoomMessage);
    /// assert_eq!(s, "m.room.message");
    /// ```
    pub fn to_string(event_type: MatrixEventType) -> String {
        event_type.as_str().to_string()
    }

    /// 从字符串解析事件类型
    ///
    /// # 示例
    ///
    /// ```
    /// use cis_core::matrix::events::{MatrixEventType, EventTypeMapper};
    ///
    /// let event_type = EventTypeMapper::from_string("m.room.member");
    /// assert_eq!(event_type, Some(MatrixEventType::RoomMember));
    ///
    /// let unknown = EventTypeMapper::from_string("unknown.event");
    /// assert_eq!(unknown, None);
    /// ```
    pub fn from_string(s: &str) -> Option<MatrixEventType> {
        match s {
            // 标准 Matrix 事件
            "m.room.message" => Some(MatrixEventType::RoomMessage),
            "m.room.member" => Some(MatrixEventType::RoomMember),
            "m.room.create" => Some(MatrixEventType::RoomCreate),
            "m.room.power_levels" => Some(MatrixEventType::RoomPowerLevels),
            "m.room.join_rules" => Some(MatrixEventType::RoomJoinRules),
            "m.room.name" => Some(MatrixEventType::RoomName),
            "m.room.topic" => Some(MatrixEventType::RoomTopic),
            "m.room.avatar" => Some(MatrixEventType::RoomAvatar),
            "m.room.encryption" => Some(MatrixEventType::RoomEncryption),
            "m.room.redaction" => Some(MatrixEventType::RoomRedaction),

            // CIS 自定义事件
            "cis.task.request" => Some(MatrixEventType::CisTaskRequest),
            "cis.task.response" => Some(MatrixEventType::CisTaskResponse),
            "cis.skill.invoke" => Some(MatrixEventType::CisSkillInvoke),
            "cis.skill.result" => Some(MatrixEventType::CisSkillResult),
            "cis.room.federate" => Some(MatrixEventType::CisRoomFederate),
            "cis.peer.discover" => Some(MatrixEventType::CisPeerDiscover),
            "cis.peer.heartbeat" => Some(MatrixEventType::CisPeerHeartbeat),

            _ => None,
        }
    }

    /// 获取所有标准 Matrix 事件类型
    pub fn all_matrix_events() -> Vec<MatrixEventType> {
        vec![
            MatrixEventType::RoomMessage,
            MatrixEventType::RoomMember,
            MatrixEventType::RoomCreate,
            MatrixEventType::RoomPowerLevels,
            MatrixEventType::RoomJoinRules,
            MatrixEventType::RoomName,
            MatrixEventType::RoomTopic,
            MatrixEventType::RoomAvatar,
            MatrixEventType::RoomEncryption,
            MatrixEventType::RoomRedaction,
        ]
    }

    /// 获取所有 CIS 自定义事件类型
    pub fn all_cis_events() -> Vec<MatrixEventType> {
        vec![
            MatrixEventType::CisTaskRequest,
            MatrixEventType::CisTaskResponse,
            MatrixEventType::CisSkillInvoke,
            MatrixEventType::CisSkillResult,
            MatrixEventType::CisRoomFederate,
            MatrixEventType::CisPeerDiscover,
            MatrixEventType::CisPeerHeartbeat,
        ]
    }

    /// 获取所有事件类型
    pub fn all_events() -> Vec<MatrixEventType> {
        let mut events = Self::all_matrix_events();
        events.extend(Self::all_cis_events());
        events
    }

    /// 根据类别获取事件类型
    pub fn events_by_category(category: EventCategory) -> Vec<MatrixEventType> {
        Self::all_events()
            .into_iter()
            .filter(|e| e.category() == category)
            .collect()
    }
}

/// 带事件类型的 CIS Matrix 事件
///
/// 这是 `CisMatrixEvent` 的类型安全版本，使用 `MatrixEventType` 代替字符串
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedCisMatrixEvent {
    /// Event ID (globally unique)
    pub event_id: String,

    /// Room ID
    pub room_id: String,

    /// Sender user ID
    pub sender: String,

    /// Event type (typed)
    #[serde(rename = "type")]
    pub event_type: MatrixEventType,

    /// Event content
    pub content: serde_json::Value,

    /// Origin server timestamp (milliseconds since epoch)
    pub origin_server_ts: i64,

    /// Unsigned data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsigned: Option<serde_json::Value>,

    /// State key for state events (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,

    /// Origin server name (set by receiving server)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,

    /// Signatures from origin server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,

    /// Hash of the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashes: Option<std::collections::HashMap<String, String>>,
}

impl TypedCisMatrixEvent {
    /// Create a new typed CIS Matrix event
    pub fn new(
        event_id: impl Into<String>,
        room_id: impl Into<String>,
        sender: impl Into<String>,
        event_type: MatrixEventType,
        content: serde_json::Value,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            room_id: room_id.into(),
            sender: sender.into(),
            event_type,
            content,
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
            unsigned: None,
            state_key: None,
            origin: None,
            signatures: None,
            hashes: None,
        }
    }

    /// Set the origin server
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Set unsigned data
    pub fn with_unsigned(mut self, unsigned: serde_json::Value) -> Self {
        self.unsigned = Some(unsigned);
        self
    }

    /// Set state key
    pub fn with_state_key(mut self, state_key: impl Into<String>) -> Self {
        self.state_key = Some(state_key.into());
        self
    }

    /// 检查是否为状态事件
    pub fn is_state_event(&self) -> bool {
        self.event_type.is_state_event()
    }

    /// 检查是否为 CIS 事件
    pub fn is_cis_event(&self) -> bool {
        self.event_type.is_cis_event()
    }
}

/// 事件类型错误
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum EventTypeError {
    #[error("Unknown event type: {0}")]
    UnknownEventType(String),

    #[error("Invalid event type format: {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_event_type_as_str() {
        assert_eq!(MatrixEventType::RoomMessage.as_str(), "m.room.message");
        assert_eq!(MatrixEventType::RoomMember.as_str(), "m.room.member");
        assert_eq!(MatrixEventType::RoomCreate.as_str(), "m.room.create");
        assert_eq!(
            MatrixEventType::RoomPowerLevels.as_str(),
            "m.room.power_levels"
        );
        assert_eq!(MatrixEventType::RoomJoinRules.as_str(), "m.room.join_rules");
        assert_eq!(MatrixEventType::CisTaskRequest.as_str(), "cis.task.request");
        assert_eq!(
            MatrixEventType::CisTaskResponse.as_str(),
            "cis.task.response"
        );
        assert_eq!(MatrixEventType::CisSkillInvoke.as_str(), "cis.skill.invoke");
        assert_eq!(MatrixEventType::CisSkillResult.as_str(), "cis.skill.result");
        assert_eq!(
            MatrixEventType::CisRoomFederate.as_str(),
            "cis.room.federate"
        );
    }

    #[test]
    fn test_event_type_mapper_to_string() {
        assert_eq!(
            EventTypeMapper::to_string(MatrixEventType::RoomMessage),
            "m.room.message"
        );
        assert_eq!(
            EventTypeMapper::to_string(MatrixEventType::CisTaskRequest),
            "cis.task.request"
        );
    }

    #[test]
    fn test_event_type_mapper_from_string() {
        assert_eq!(
            EventTypeMapper::from_string("m.room.message"),
            Some(MatrixEventType::RoomMessage)
        );
        assert_eq!(
            EventTypeMapper::from_string("m.room.member"),
            Some(MatrixEventType::RoomMember)
        );
        assert_eq!(
            EventTypeMapper::from_string("cis.task.request"),
            Some(MatrixEventType::CisTaskRequest)
        );
        assert_eq!(EventTypeMapper::from_string("unknown.event"), None);
    }

    #[test]
    fn test_is_cis_event() {
        assert!(!MatrixEventType::RoomMessage.is_cis_event());
        assert!(!MatrixEventType::RoomMember.is_cis_event());
        assert!(MatrixEventType::CisTaskRequest.is_cis_event());
        assert!(MatrixEventType::CisSkillInvoke.is_cis_event());
    }

    #[test]
    fn test_is_matrix_event() {
        assert!(MatrixEventType::RoomMessage.is_matrix_event());
        assert!(MatrixEventType::RoomMember.is_matrix_event());
        assert!(!MatrixEventType::CisTaskRequest.is_matrix_event());
        assert!(!MatrixEventType::CisSkillInvoke.is_matrix_event());
    }

    #[test]
    fn test_is_state_event() {
        assert!(!MatrixEventType::RoomMessage.is_state_event());
        assert!(MatrixEventType::RoomMember.is_state_event());
        assert!(MatrixEventType::RoomCreate.is_state_event());
        assert!(MatrixEventType::RoomPowerLevels.is_state_event());
        assert!(MatrixEventType::CisRoomFederate.is_state_event());
        assert!(!MatrixEventType::CisTaskRequest.is_state_event());
    }

    #[test]
    fn test_event_category() {
        assert_eq!(MatrixEventType::RoomMessage.category(), EventCategory::Message);
        assert_eq!(
            MatrixEventType::RoomMember.category(),
            EventCategory::Membership
        );
        assert_eq!(
            MatrixEventType::RoomCreate.category(),
            EventCategory::Room
        );
        assert_eq!(
            MatrixEventType::RoomPowerLevels.category(),
            EventCategory::Permission
        );
        assert_eq!(
            MatrixEventType::CisTaskRequest.category(),
            EventCategory::CisTask
        );
        assert_eq!(
            MatrixEventType::CisSkillInvoke.category(),
            EventCategory::CisSkill
        );
        assert_eq!(
            MatrixEventType::CisRoomFederate.category(),
            EventCategory::CisFederation
        );
    }

    #[test]
    fn test_all_matrix_events() {
        let events = EventTypeMapper::all_matrix_events();
        assert!(events.contains(&MatrixEventType::RoomMessage));
        assert!(events.contains(&MatrixEventType::RoomMember));
        assert!(!events.contains(&MatrixEventType::CisTaskRequest));
    }

    #[test]
    fn test_all_cis_events() {
        let events = EventTypeMapper::all_cis_events();
        assert!(events.contains(&MatrixEventType::CisTaskRequest));
        assert!(events.contains(&MatrixEventType::CisSkillInvoke));
        assert!(!events.contains(&MatrixEventType::RoomMessage));
    }

    #[test]
    fn test_events_by_category() {
        let task_events = EventTypeMapper::events_by_category(EventCategory::CisTask);
        assert!(task_events.contains(&MatrixEventType::CisTaskRequest));
        assert!(task_events.contains(&MatrixEventType::CisTaskResponse));

        let room_events = EventTypeMapper::events_by_category(EventCategory::Room);
        assert!(room_events.contains(&MatrixEventType::RoomCreate));
        assert!(room_events.contains(&MatrixEventType::RoomName));
    }

    #[test]
    fn test_serialize_event_type() {
        let event_type = MatrixEventType::RoomMessage;
        let json = serde_json::to_string(&event_type).unwrap();
        assert_eq!(json, "\"m.room.message\"");

        let cis_event = MatrixEventType::CisTaskRequest;
        let json = serde_json::to_string(&cis_event).unwrap();
        assert_eq!(json, "\"cis.task.request\"");
    }

    #[test]
    fn test_deserialize_event_type() {
        let event_type: MatrixEventType = serde_json::from_str("\"m.room.message\"").unwrap();
        assert_eq!(event_type, MatrixEventType::RoomMessage);

        let cis_event: MatrixEventType = serde_json::from_str("\"cis.task.request\"").unwrap();
        assert_eq!(cis_event, MatrixEventType::CisTaskRequest);
    }

    #[test]
    fn test_deserialize_unknown_event_type() {
        let result: Result<MatrixEventType, _> = serde_json::from_str("\"unknown.event\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", MatrixEventType::RoomMessage),
            "m.room.message"
        );
        assert_eq!(
            format!("{}", MatrixEventType::CisTaskRequest),
            "cis.task.request"
        );
    }

    #[test]
    fn test_typed_cis_matrix_event() {
        let event = TypedCisMatrixEvent::new(
            "$event123",
            "!room123:cis.local",
            "@alice:cis.local",
            MatrixEventType::CisTaskRequest,
            serde_json::json!({
                "task_id": "task-456",
                "skill_name": "git",
                "method": "push"
            }),
        );

        assert_eq!(event.event_id, "$event123");
        assert_eq!(event.room_id, "!room123:cis.local");
        assert_eq!(event.sender, "@alice:cis.local");
        assert_eq!(event.event_type, MatrixEventType::CisTaskRequest);
        assert!(event.is_cis_event());
        assert!(!event.is_state_event());
    }

    #[test]
    fn test_typed_cis_matrix_event_serialization() {
        let event = TypedCisMatrixEvent::new(
            "$event123",
            "!room123:cis.local",
            "@alice:cis.local",
            MatrixEventType::RoomMessage,
            serde_json::json!({ "body": "Hello", "msgtype": "m.text" }),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"m.room.message\""));
        assert!(json.contains("\"event_id\":\"$event123\""));

        let deserialized: TypedCisMatrixEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type, MatrixEventType::RoomMessage);
    }

    #[test]
    fn test_event_type_conversion() {
        let event_type = MatrixEventType::CisSkillInvoke;
        let string: String = event_type.into();
        assert_eq!(string, "cis.skill.invoke");

        let str_ref: &str = event_type.as_ref();
        assert_eq!(str_ref, "cis.skill.invoke");
    }

    #[test]
    fn test_category_as_str() {
        assert_eq!(EventCategory::Message.as_str(), "message");
        assert_eq!(EventCategory::CisTask.as_str(), "cis_task");
        assert_eq!(EventCategory::CisFederation.as_str(), "cis_federation");
    }
}
