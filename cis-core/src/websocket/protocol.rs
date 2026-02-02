//! # WebSocket Federation Protocol
//!
//! Protobuf-based message protocol for WebSocket federation.
//!
//! ## Message Types
//!
//! - `Handshake`: Initial connection handshake with Noise protocol
//! - `Auth`: DID-based authentication
//! - `Event`: Matrix event forwarding
//! - `Heartbeat`: Keep-alive ping/pong
//! - `Error`: Error responses

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    /// Handshake initiation (Noise protocol)
    #[serde(rename = "handshake")]
    Handshake(HandshakeMessage),

    /// Authentication message (DID)
    #[serde(rename = "auth")]
    Auth(AuthMessage),

    /// Matrix event
    #[serde(rename = "event")]
    Event(EventMessage),

    /// Heartbeat ping
    #[serde(rename = "ping")]
    Ping(PingMessage),

    /// Heartbeat pong
    #[serde(rename = "pong")]
    Pong(PongMessage),

    /// Error message
    #[serde(rename = "error")]
    Error(ErrorMessage),

    /// Acknowledgment
    #[serde(rename = "ack")]
    Ack(AckMessage),
}

/// Handshake message for Noise protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandshakeMessage {
    /// Protocol version
    pub version: u32,
    /// Noise handshake payload
    pub payload: Vec<u8>,
    /// Server name (DID)
    pub server_name: String,
    /// Timestamp for replay protection
    pub timestamp: u64,
}

impl HandshakeMessage {
    /// Create a new handshake message
    pub fn new(version: u32, payload: Vec<u8>, server_name: impl Into<String>) -> Self {
        Self {
            version,
            payload,
            server_name: server_name.into(),
            timestamp: current_timestamp(),
        }
    }
}

/// Authentication message using DID
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthMessage {
    /// DID of the node
    pub did: String,
    /// Challenge response (signed)
    pub challenge_response: Vec<u8>,
    /// Public key for verification
    pub public_key: Vec<u8>,
    /// Timestamp
    pub timestamp: u64,
}

impl AuthMessage {
    /// Create a new auth message
    pub fn new(did: impl Into<String>, challenge_response: Vec<u8>, public_key: Vec<u8>) -> Self {
        Self {
            did: did.into(),
            challenge_response,
            public_key,
            timestamp: current_timestamp(),
        }
    }
}

/// Event message containing Matrix events
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMessage {
    /// Unique message ID
    pub message_id: String,
    /// Event payload (JSON serialized)
    pub event_data: Vec<u8>,
    /// Event type hint
    pub event_type: String,
    /// Sender node
    pub sender: String,
    /// Target room (optional)
    pub room_id: Option<String>,
    /// Timestamp
    pub timestamp: u64,
    /// Sequence number for ordering
    pub sequence: u64,
}

impl EventMessage {
    /// Create a new event message
    pub fn new(
        message_id: impl Into<String>,
        event_data: Vec<u8>,
        event_type: impl Into<String>,
        sender: impl Into<String>,
    ) -> Self {
        Self {
            message_id: message_id.into(),
            event_data,
            event_type: event_type.into(),
            sender: sender.into(),
            room_id: None,
            timestamp: current_timestamp(),
            sequence: 0,
        }
    }

    /// Set room ID
    pub fn with_room_id(mut self, room_id: impl Into<String>) -> Self {
        self.room_id = Some(room_id.into());
        self
    }

    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }
}

/// Ping message for heartbeat
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PingMessage {
    /// Ping ID
    pub ping_id: u64,
    /// Timestamp
    pub timestamp: u64,
}

impl PingMessage {
    /// Create a new ping
    pub fn new(ping_id: u64) -> Self {
        Self {
            ping_id,
            timestamp: current_timestamp(),
        }
    }
}

/// Pong message for heartbeat response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PongMessage {
    /// Ping ID being responded to
    pub ping_id: u64,
    /// Server timestamp
    pub timestamp: u64,
}

impl PongMessage {
    /// Create a new pong response
    pub fn new(ping_id: u64) -> Self {
        Self {
            ping_id,
            timestamp: current_timestamp(),
        }
    }
}

/// Error message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorMessage {
    /// Error code
    pub code: ErrorCode,
    /// Error description
    pub message: String,
    /// Related message ID (if any)
    pub related_id: Option<String>,
}

impl ErrorMessage {
    /// Create a new error message
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            related_id: None,
        }
    }

    /// Set related message ID
    pub fn with_related_id(mut self, id: impl Into<String>) -> Self {
        self.related_id = Some(id.into());
        self
    }
}

/// Error codes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Invalid message format
    InvalidFormat,
    /// Authentication failed
    AuthFailed,
    /// Unauthorized
    Unauthorized,
    /// Rate limited
    RateLimited,
    /// Internal server error
    InternalError,
    /// Protocol version mismatch
    VersionMismatch,
    /// Timeout
    Timeout,
}

/// Acknowledgment message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AckMessage {
    /// Message ID being acknowledged
    pub message_id: String,
    /// Status
    pub status: AckStatus,
    /// Optional error if failed
    pub error: Option<String>,
}

impl AckMessage {
    /// Create a success acknowledgment
    pub fn success(message_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            status: AckStatus::Success,
            error: None,
        }
    }

    /// Create a failure acknowledgment
    pub fn failed(message_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            status: AckStatus::Failed,
            error: Some(error.into()),
        }
    }
}

/// Acknowledgment status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    /// Message received and processed
    Success,
    /// Message processing failed
    Failed,
    /// Message received, processing async
    Pending,
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// Default WebSocket port
pub const DEFAULT_WS_PORT: u16 = 6768;

/// WebSocket path
pub const WS_PATH: &str = "/_cis/ws/v1/federation";

/// Build WebSocket URL from components
pub fn build_ws_url(host: &str, port: u16, use_tls: bool) -> String {
    let scheme = if use_tls { "wss" } else { "ws" };
    format!("{}://{}:{}{}", scheme, host, port, WS_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_message_serialization() {
        let ping = WsMessage::Ping(PingMessage::new(1));
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("ping"));

        let decoded: WsMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, WsMessage::Ping(_)));
    }

    #[test]
    fn test_event_message() {
        let event = EventMessage::new(
            "msg-123",
            b"test data".to_vec(),
            "m.room.message",
            "@alice:cis.local",
        )
        .with_room_id("!room:cis.local")
        .with_sequence(42);

        assert_eq!(event.message_id, "msg-123");
        assert_eq!(event.room_id, Some("!room:cis.local".to_string()));
        assert_eq!(event.sequence, 42);
    }

    #[test]
    fn test_ack_message() {
        let success = AckMessage::success("msg-123");
        assert!(matches!(success.status, AckStatus::Success));
        assert!(success.error.is_none());

        let failed = AckMessage::failed("msg-456", "timeout");
        assert!(matches!(failed.status, AckStatus::Failed));
        assert_eq!(failed.error, Some("timeout".to_string()));
    }

    #[test]
    fn test_build_ws_url() {
        let url = build_ws_url("localhost", 6768, false);
        assert_eq!(url, "ws://localhost:6768/_cis/ws/v1/federation");

        let url_tls = build_ws_url("secure.example.com", 443, true);
        assert_eq!(url_tls, "wss://secure.example.com:443/_cis/ws/v1/federation");
    }
}
