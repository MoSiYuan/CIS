use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Ping,
    Pong,
    Request { id: String, payload: Vec<u8> },
    Response { id: String, payload: Vec<u8> },
    Error { id: String, message: String },
}

impl Message {
    pub fn request(id: impl Into<String>, payload: Vec<u8>) -> Self {
        Message::Request {
            id: id.into(),
            payload,
        }
    }

    pub fn response(id: impl Into<String>, payload: Vec<u8>) -> Self {
        Message::Response {
            id: id.into(),
            payload,
        }
    }

    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Self {
        Message::Error {
            id: id.into(),
            message: message.into(),
        }
    }
}
