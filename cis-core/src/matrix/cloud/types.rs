//! # Cloud Anchor 类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

/// NAT 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NatType {
    /// 开放网络（无 NAT）
    Open,
    /// 全锥型 NAT
    FullCone,
    /// 地址限制锥型 NAT
    AddressRestricted,
    /// 端口限制锥型 NAT
    PortRestricted,
    /// 对称型 NAT
    Symmetric,
    /// 未知类型
    #[default]
    Unknown,
}


impl NatType {
    /// 判断是否容易穿透
    pub fn is_easy_traversal(&self) -> bool {
        matches!(
            self,
            NatType::Open | NatType::FullCone | NatType::AddressRestricted
        )
    }

    /// 判断是否需要 TURN 中继
    pub fn needs_turn(&self) -> bool {
        matches!(self, NatType::Symmetric)
    }

    /// 判断是否可能打洞成功
    pub fn can_hole_punch(&self) -> bool {
        !matches!(self, NatType::Symmetric | NatType::Unknown)
    }
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatType::Open => write!(f, "open"),
            NatType::FullCone => write!(f, "full_cone"),
            NatType::AddressRestricted => write!(f, "address_restricted"),
            NatType::PortRestricted => write!(f, "port_restricted"),
            NatType::Symmetric => write!(f, "symmetric"),
            NatType::Unknown => write!(f, "unknown"),
        }
    }
}

/// 节点能力
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeCapabilities {
    /// 支持的协议版本
    #[serde(default)]
    pub protocol_version: String,

    /// 支持的功能列表
    #[serde(default)]
    pub features: Vec<String>,

    /// 最大并发连接数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,

    /// 是否支持中继
    #[serde(default)]
    pub relay_supported: bool,

    /// 支持的加密算法
    #[serde(default)]
    pub encryption: Vec<String>,

    /// 平台信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,

    /// 版本信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// 自定义元数据
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl NodeCapabilities {
    /// 创建新的能力描述
    pub fn new(protocol_version: impl Into<String>) -> Self {
        Self {
            protocol_version: protocol_version.into(),
            features: Vec::new(),
            max_connections: None,
            relay_supported: false,
            encryption: vec!["noise".to_string()],
            platform: None,
            version: None,
            metadata: HashMap::new(),
        }
    }

    /// 添加功能
    pub fn with_feature(mut self, feature: impl Into<String>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// 设置最大连接数
    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = Some(max);
        self
    }

    /// 启用中继支持
    pub fn with_relay(mut self, supported: bool) -> Self {
        self.relay_supported = supported;
        self
    }

    /// 设置平台信息
    pub fn with_platform(mut self, platform: impl Into<String>) -> Self {
        self.platform = Some(platform.into());
        self
    }

    /// 设置版本信息
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 检查是否支持某功能
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

/// 节点注册信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRegistration {
    /// 节点 ID（全局唯一）
    pub node_id: String,

    /// DID（去中心化身份标识）
    pub did: String,

    /// 公网地址
    pub public_addr: SocketAddr,

    /// 内网地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_addr: Option<SocketAddr>,

    /// NAT 类型
    #[serde(default)]
    pub nat_type: NatType,

    /// 节点能力
    #[serde(default)]
    pub capabilities: NodeCapabilities,

    /// 房间 ID（用于分组）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,

    /// 公钥（用于验证身份）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,

    /// 标签（用于发现和过滤）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl NodeRegistration {
    /// 创建新的节点注册信息
    pub fn new(
        node_id: impl Into<String>,
        did: impl Into<String>,
        public_addr: SocketAddr,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            did: did.into(),
            public_addr,
            local_addr: None,
            nat_type: NatType::Unknown,
            capabilities: NodeCapabilities::default(),
            room_id: None,
            public_key: None,
            tags: Vec::new(),
        }
    }

    /// 设置内网地址
    pub fn with_local_addr(mut self, addr: SocketAddr) -> Self {
        self.local_addr = Some(addr);
        self
    }

    /// 设置 NAT 类型
    pub fn with_nat_type(mut self, nat_type: NatType) -> Self {
        self.nat_type = nat_type;
        self
    }

    /// 设置节点能力
    pub fn with_capabilities(mut self, capabilities: NodeCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// 设置房间 ID
    pub fn with_room_id(mut self, room_id: impl Into<String>) -> Self {
        self.room_id = Some(room_id.into());
        self
    }

    /// 设置公钥
    pub fn with_public_key(mut self, key: impl Into<String>) -> Self {
        self.public_key = Some(key.into());
        self
    }

    /// 添加标签
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// 注册响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    /// 是否注册成功
    pub success: bool,

    /// 注册令牌（用于心跳和后续请求）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// 过期时间（Unix 时间戳）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,

    /// 错误信息（如果失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// 服务器分配的节点信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_info: Option<HashMap<String, serde_json::Value>>,
}

impl RegistrationResponse {
    /// 创建成功响应
    pub fn success(token: impl Into<String>, expires_at: u64) -> Self {
        Self {
            success: true,
            token: Some(token.into()),
            expires_at: Some(expires_at),
            error: None,
            assigned_info: None,
        }
    }

    /// 创建失败响应
    pub fn error(error: impl Into<String>) -> Self {
        Self {
            success: false,
            token: None,
            expires_at: None,
            error: Some(error.into()),
            assigned_info: None,
        }
    }
}

/// 发现的节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    /// 节点 ID
    pub node_id: String,

    /// DID
    pub did: String,

    /// 公网端点
    pub public_endpoint: SocketAddr,

    /// 内网端点（如果有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_endpoint: Option<SocketAddr>,

    /// NAT 类型
    #[serde(default)]
    pub nat_type: NatType,

    /// 最后活跃时间（Unix 时间戳）
    pub last_seen: u64,

    /// 节点能力
    #[serde(default)]
    pub capabilities: NodeCapabilities,

    /// 房间 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,

    /// 公钥
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,

    /// 标签
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// 距离（跳数或延迟，用于排序）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<u32>,

    /// 连接成功率（0-100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate: Option<u8>,
}

impl DiscoveredPeer {
    /// 判断是否可直接连接
    pub fn is_direct_connectable(&self) -> bool {
        self.nat_type.is_easy_traversal()
    }

    /// 判断是否需要打洞
    pub fn needs_hole_punch(&self) -> bool {
        matches!(
            self.nat_type,
            NatType::PortRestricted | NatType::AddressRestricted
        )
    }

    /// 判断是否需要中继
    pub fn needs_relay(&self) -> bool {
        self.nat_type.needs_turn()
    }

    /// 检查是否有特定标签
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// 检查是否支持某功能
    pub fn has_feature(&self, feature: &str) -> bool {
        self.capabilities.has_feature(feature)
    }
}

/// 打洞请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolePunchRequest {
    /// 发起者节点 ID
    pub initiator_id: String,

    /// 目标节点 ID
    pub target_id: String,

    /// 发起者的公网地址
    pub initiator_public_addr: SocketAddr,

    /// 发起者的内网地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initiator_local_addr: Option<SocketAddr>,

    /// 发起者的 NAT 类型
    #[serde(default)]
    pub initiator_nat_type: NatType,

    /// 请求时间戳
    pub timestamp: u64,

    /// 会话 ID
    pub session_id: String,
}

impl HolePunchRequest {
    /// 创建新的打洞请求
    pub fn new(
        initiator_id: impl Into<String>,
        target_id: impl Into<String>,
        public_addr: SocketAddr,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            initiator_id: initiator_id.into(),
            target_id: target_id.into(),
            initiator_public_addr: public_addr,
            initiator_local_addr: None,
            initiator_nat_type: NatType::Unknown,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            session_id: session_id.into(),
        }
    }

    /// 设置内网地址
    pub fn with_local_addr(mut self, addr: SocketAddr) -> Self {
        self.initiator_local_addr = Some(addr);
        self
    }

    /// 设置 NAT 类型
    pub fn with_nat_type(mut self, nat_type: NatType) -> Self {
        self.initiator_nat_type = nat_type;
        self
    }
}

/// 打洞响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolePunchResponse {
    /// 会话 ID
    pub session_id: String,

    /// 目标节点是否在线
    pub target_online: bool,

    /// 目标节点的公网地址（如果在线）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_public_addr: Option<SocketAddr>,

    /// 目标节点的内网地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_local_addr: Option<SocketAddr>,

    /// 目标节点的 NAT 类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_nat_type: Option<NatType>,

    /// 建议的打洞时间（Unix 时间戳，毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_punch_time: Option<u64>,

    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 打洞协调信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PunchCoordination {
    /// 会话 ID
    pub session_id: String,

    /// 我的公网地址（从服务器视角）
    pub my_public_addr: SocketAddr,

    /// 对方的公网地址
    pub peer_public_addr: SocketAddr,

    /// 对方的内网地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_local_addr: Option<SocketAddr>,

    /// 对方的 NAT 类型
    pub peer_nat_type: NatType,

    /// 打洞开始时间（Unix 时间戳，毫秒）
    pub start_time: u64,

    /// 超时时间（毫秒）
    pub timeout_ms: u64,

    /// 建议的端口号范围
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range: Option<(u16, u16)>,

    /// 协调令牌
    pub coordination_token: String,
}

impl PunchCoordination {
    /// 计算打洞延迟（从当前时间到开始时间）
    pub fn time_until_start(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.start_time as i64 - now
    }

    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now > self.start_time + self.timeout_ms
    }
}

/// 打洞信息（用于报告结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolePunchInfo {
    /// 会话 ID
    pub session_id: String,

    /// 是否成功
    pub success: bool,

    /// 使用的本地地址
    pub local_addr: SocketAddr,

    /// 连接到的对端地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_addr: Option<SocketAddr>,

    /// 耗时（毫秒）
    pub duration_ms: u64,

    /// 尝试次数
    pub attempts: u32,

    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 中继消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMessage {
    /// 消息 ID
    pub message_id: String,

    /// 发送者节点 ID
    pub from: String,

    /// 接收者节点 ID
    pub to: String,

    /// 消息类型
    #[serde(rename = "type")]
    pub msg_type: String,

    /// 消息内容（Base64 编码的加密数据）
    pub payload: String,

    /// 时间戳
    pub timestamp: u64,

    /// TTL（剩余跳数）
    #[serde(default = "default_ttl")]
    pub ttl: u8,

    /// 消息大小（字节）
    pub size: u32,
}

fn default_ttl() -> u8 {
    5
}

impl RelayMessage {
    /// 创建新的中继消息
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        msg_type: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        let payload_str = payload.into();
        let size = payload_str.len() as u32;
        Self {
            message_id: uuid::Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            msg_type: msg_type.into(),
            payload: payload_str,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ttl: default_ttl(),
            size,
        }
    }

    /// 设置 TTL
    pub fn with_ttl(mut self, ttl: u8) -> Self {
        self.ttl = ttl;
        self
    }

    /// 递减 TTL
    pub fn decrement_ttl(&mut self) -> bool {
        if self.ttl > 0 {
            self.ttl -= 1;
            true
        } else {
            false
        }
    }
}

/// 配额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaInfo {
    /// 总配额（字节）
    pub total_bytes: u64,

    /// 已使用（字节）
    pub used_bytes: u64,

    /// 剩余（字节）
    pub remaining_bytes: u64,

    /// 重置时间（Unix 时间戳）
    pub reset_at: u64,
}

impl QuotaInfo {
    /// 计算使用百分比
    pub fn usage_percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// 检查是否已超限
    pub fn is_exceeded(&self) -> bool {
        self.used_bytes >= self.total_bytes
    }

    /// 检查是否接近限额（80%）
    pub fn is_near_limit(&self) -> bool {
        self.usage_percentage() >= 80.0
    }
}

/// 心跳请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    /// 节点 ID
    pub node_id: String,

    /// 注册令牌
    pub token: String,

    /// 当前公网地址（可选，用于检测地址变化）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_public_addr: Option<SocketAddr>,

    /// 活跃连接数
    #[serde(default)]
    pub active_connections: u32,

    /// 系统负载信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_load: Option<f32>,
}

/// 心跳响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    /// 是否成功
    pub success: bool,

    /// 新的令牌（如果需要刷新）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_token: Option<String>,

    /// 令牌过期时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,

    /// 建议的心跳间隔（秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_interval: Option<u64>,

    /// 错误信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_type() {
        assert!(NatType::Open.is_easy_traversal());
        assert!(NatType::FullCone.is_easy_traversal());
        assert!(!NatType::Symmetric.is_easy_traversal());
        assert!(NatType::Symmetric.needs_turn());
        assert!(NatType::Open.can_hole_punch());
        assert!(!NatType::Symmetric.can_hole_punch());
    }

    #[test]
    fn test_node_capabilities() {
        let caps = NodeCapabilities::new("1.0")
            .with_feature("relay")
            .with_feature("encryption")
            .with_max_connections(100)
            .with_platform("linux");

        assert!(caps.has_feature("relay"));
        assert!(!caps.has_feature("unknown"));
        assert_eq!(caps.max_connections, Some(100));
        assert_eq!(caps.platform, Some("linux".to_string()));
    }

    #[test]
    fn test_node_registration() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        let reg = NodeRegistration::new("node1", "did:example:123", addr)
            .with_room_id("room-1")
            .with_tag("test")
            .with_nat_type(NatType::FullCone);

        assert_eq!(reg.node_id, "node1");
        assert_eq!(reg.room_id, Some("room-1".to_string()));
        assert_eq!(reg.nat_type, NatType::FullCone);
    }

    #[test]
    fn test_punch_coordination() {
        let coordination = PunchCoordination {
            session_id: "sess-1".to_string(),
            my_public_addr: "1.2.3.4:1000".parse().unwrap(),
            peer_public_addr: "5.6.7.8:2000".parse().unwrap(),
            peer_local_addr: Some("192.168.1.1:2000".parse().unwrap()),
            peer_nat_type: NatType::PortRestricted,
            start_time: 1000000,
            timeout_ms: 30000,
            port_range: Some((10000, 20000)),
            coordination_token: "token-123".to_string(),
        };

        assert_eq!(coordination.session_id, "sess-1");
        assert_eq!(coordination.peer_nat_type, NatType::PortRestricted);
        // start_time 是过去的，所以应该已过期
        assert!(coordination.is_expired() || coordination.time_until_start() < 0);
    }

    #[test]
    fn test_relay_message() {
        let msg = RelayMessage::new("node1", "node2", "test", "payload-data")
            .with_ttl(3);

        assert_eq!(msg.from, "node1");
        assert_eq!(msg.to, "node2");
        assert_eq!(msg.msg_type, "test");
        assert_eq!(msg.ttl, 3);
        assert!(msg.size > 0);

        let mut msg = msg;
        assert!(msg.decrement_ttl());
        assert_eq!(msg.ttl, 2);
    }

    #[test]
    fn test_quota_info() {
        let quota = QuotaInfo {
            total_bytes: 1000,
            used_bytes: 800,
            remaining_bytes: 200,
            reset_at: 0,
        };

        assert_eq!(quota.usage_percentage(), 80.0);
        assert!(quota.is_near_limit());
        assert!(!quota.is_exceeded());

        let exceeded = QuotaInfo {
            total_bytes: 1000,
            used_bytes: 1000,
            remaining_bytes: 0,
            reset_at: 0,
        };
        assert!(exceeded.is_exceeded());
    }

    #[test]
    fn test_discovered_peer() {
        let addr = "1.2.3.4:8080".parse().unwrap();
        let peer = DiscoveredPeer {
            node_id: "node1".to_string(),
            did: "did:example:123".to_string(),
            public_endpoint: addr,
            local_endpoint: None,
            nat_type: NatType::FullCone,
            last_seen: 0,
            capabilities: NodeCapabilities::default(),
            room_id: None,
            public_key: None,
            tags: vec!["test".to_string(), "prod".to_string()],
            distance: Some(1),
            success_rate: Some(95),
        };

        assert!(peer.is_direct_connectable());
        assert!(peer.has_tag("test"));
        assert!(!peer.has_tag("missing"));
    }
}
