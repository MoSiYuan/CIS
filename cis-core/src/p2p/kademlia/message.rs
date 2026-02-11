//! Kademlia RPC 消息协议

use super::node_id::NodeId;
use serde::{Serialize, Deserialize};

/// Kademlia RPC 消息类型（旧版枚举，保持兼容）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    /// PING 请求
    Ping(PingRequest),
    /// PING 响应
    Pong(PongResponse),
    /// 查找节点请求
    FindNode(FindNodeRequest),
    /// 查找节点响应
    FindNodeResponse(FindNodeResponse),
    /// 存储值请求
    Store(StoreRequest),
    /// 存储值响应
    StoreResponse(StoreResponse),
    /// 查找值请求
    FindValue(FindValueRequest),
    /// 查找值响应
    FindValueResponse(FindValueResponse),
}

/// PING 请求
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PingRequest {
    pub sender_id: NodeId,
    pub sender_addr: String,
    pub nonce: u64,
}

impl PingRequest {
    pub fn new(sender_id: NodeId, sender_addr: impl Into<String>) -> Self {
        Self {
            sender_id,
            sender_addr: sender_addr.into(),
            nonce: rand::random(),
        }
    }
}

/// PONG 响应
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PongResponse {
    pub sender_id: NodeId,
    pub nonce: u64,
}

impl PongResponse {
    pub fn new(sender_id: NodeId, nonce: u64) -> Self {
        Self { sender_id, nonce }
    }
}

/// 查找节点请求
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindNodeRequest {
    pub sender_id: NodeId,
    pub target: NodeId,
}

impl FindNodeRequest {
    pub fn new(sender_id: NodeId, target: NodeId) -> Self {
        Self { sender_id, target }
    }
}

/// 查找节点响应
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindNodeResponse {
    pub sender_id: NodeId,
    pub nodes: Vec<NodeInfoMsg>,
}

impl FindNodeResponse {
    pub fn new(sender_id: NodeId, nodes: Vec<NodeInfoMsg>) -> Self {
        Self { sender_id, nodes }
    }
}

/// 存储值请求
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreRequest {
    pub sender_id: NodeId,
    pub key: String,
    pub value: Vec<u8>,
    pub ttl_secs: u32,
}

impl StoreRequest {
    pub fn new(sender_id: NodeId, key: impl Into<String>, value: Vec<u8>, ttl_secs: u32) -> Self {
        Self {
            sender_id,
            key: key.into(),
            value,
            ttl_secs,
        }
    }
}

/// 存储值响应
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreResponse {
    pub sender_id: NodeId,
    pub success: bool,
}

impl StoreResponse {
    pub fn new(sender_id: NodeId, success: bool) -> Self {
        Self { sender_id, success }
    }
}

/// 查找值请求
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindValueRequest {
    pub sender_id: NodeId,
    pub key: String,
}

impl FindValueRequest {
    pub fn new(sender_id: NodeId, key: impl Into<String>) -> Self {
        Self { sender_id, key: key.into() }
    }
}

/// 查找值响应
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindValueResponse {
    pub sender_id: NodeId,
    pub value: Option<Vec<u8>>,
    pub nodes: Vec<NodeInfoMsg>,
}

impl FindValueResponse {
    pub fn with_value(sender_id: NodeId, value: Vec<u8>) -> Self {
        Self {
            sender_id,
            value: Some(value),
            nodes: vec![],
        }
    }

    pub fn with_nodes(sender_id: NodeId, nodes: Vec<NodeInfoMsg>) -> Self {
        Self {
            sender_id,
            value: None,
            nodes,
        }
    }
}

/// 节点信息消息（用于网络传输）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeInfoMsg {
    pub id: [u8; 20],
    pub address: String,
}

impl NodeInfoMsg {
    pub fn new(id: NodeId, address: impl Into<String>) -> Self {
        Self {
            id: *id.as_bytes(),
            address: address.into(),
        }
    }
}

/// 消息负载类型（新版）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessagePayload {
    Ping,
    Pong { nodes: Vec<NodeInfoMsg> },
    FindNode { target: NodeId },
    FindNodeResponse { nodes: Vec<NodeInfoMsg> },
    Store { key: String, value: Vec<u8> },
    FindValue { key: String },
    FindValueResponse { value: Option<Vec<u8>>, nodes: Vec<NodeInfoMsg> },
}

/// Kademlia 消息（新版，用于 DHT 服务）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KademliaMessage {
    pub sender_id: NodeId,
    pub nonce: u64,
    pub payload: MessagePayload,
}

impl KademliaMessage {
    /// 创建新的 PING 消息
    pub fn ping(sender_id: NodeId) -> Self {
        Self {
            sender_id,
            nonce: rand::random(),
            payload: MessagePayload::Ping,
        }
    }

    /// 创建 PONG 响应
    pub fn pong(sender_id: NodeId, nodes: Vec<NodeInfoMsg>) -> Self {
        Self {
            sender_id,
            nonce: 0, // 响应 nonce 由调用者设置
            payload: MessagePayload::Pong { nodes },
        }
    }

    /// 创建 FindNode 请求
    pub fn find_node(sender_id: NodeId, target: NodeId) -> Self {
        Self {
            sender_id,
            nonce: rand::random(),
            payload: MessagePayload::FindNode { target },
        }
    }

    /// 创建 FindNode 响应
    pub fn find_node_response(sender_id: NodeId, nodes: Vec<NodeInfoMsg>) -> Self {
        Self {
            sender_id,
            nonce: 0,
            payload: MessagePayload::FindNodeResponse { nodes },
        }
    }

    /// 创建 Store 请求
    pub fn store(sender_id: NodeId, key: String, value: Vec<u8>) -> Self {
        Self {
            sender_id,
            nonce: rand::random(),
            payload: MessagePayload::Store { key, value },
        }
    }

    /// 创建 FindValue 请求
    pub fn find_value(sender_id: NodeId, key: String) -> Self {
        Self {
            sender_id,
            nonce: rand::random(),
            payload: MessagePayload::FindValue { key },
        }
    }

    /// 创建 FindValue 响应
    pub fn find_value_response(
        sender_id: NodeId,
        value: Option<Vec<u8>>,
        nodes: Vec<NodeInfoMsg>,
    ) -> Self {
        Self {
            sender_id,
            nonce: 0,
            payload: MessagePayload::FindValueResponse { value, nodes },
        }
    }

    /// 获取 nonce
    pub fn nonce(&self) -> Option<u64> {
        Some(self.nonce)
    }

    /// 创建带 nonce 的响应
    pub fn into_response(mut self, new_payload: MessagePayload, response_nonce: u64) -> Self {
        self.payload = new_payload;
        self.nonce = response_nonce;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_pong() {
        let sender = NodeId::random();
        let ping = PingRequest::new(sender.clone(), "127.0.0.1:8000");
        let pong = PongResponse::new(NodeId::random(), ping.nonce);
        
        assert_eq!(ping.nonce, pong.nonce);
    }

    #[test]
    fn test_find_node_serialization() {
        let sender = NodeId::random();
        let target = NodeId::random();
        let request = FindNodeRequest::new(sender, target);
        
        let serialized = serde_json::to_vec(&request).unwrap();
        let deserialized: FindNodeRequest = serde_json::from_slice(&serialized).unwrap();
        
        assert_eq!(request.target, deserialized.target);
    }

    #[test]
    fn test_store_response() {
        let sender = NodeId::random();
        let response = StoreResponse::new(sender, true);
        assert!(response.success);
    }

    #[test]
    fn test_find_value_with_value() {
        let sender = NodeId::random();
        let value = b"test value".to_vec();
        let response = FindValueResponse::with_value(sender, value.clone());
        
        assert_eq!(response.value, Some(value));
        assert!(response.nodes.is_empty());
    }

    #[test]
    fn test_new_kademlia_message() {
        let sender = NodeId::random();
        let target = NodeId::random();
        
        let ping = KademliaMessage::ping(sender.clone());
        assert!(matches!(ping.payload, MessagePayload::Ping));
        
        let find_node = KademliaMessage::find_node(sender.clone(), target);
        assert!(matches!(find_node.payload, MessagePayload::FindNode { .. }));
    }
}
