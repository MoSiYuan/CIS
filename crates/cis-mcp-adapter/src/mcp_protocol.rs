//! MCP Protocol Types and helpers
//! 
//! Based on Model Context Protocol specification

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Request
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method")]
pub enum McpRequest {
    #[serde(rename = "initialize")]
    Initialize { params: InitializeParams },
    
    #[serde(rename = "tools/list")]
    ToolsList,
    
    #[serde(rename = "tools/call")]
    ToolsCall { params: ToolCallParams },
    
    #[serde(rename = "resources/list")]
    ResourcesList,
    
    #[serde(rename = "resources/read")]
    ResourcesRead { params: ResourceReadParams },
    
    #[serde(rename = "ping")]
    Ping,
}

/// MCP Response
#[derive(Debug, Clone, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError { code, message }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

/// Initialize parameters
#[derive(Debug, Clone, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: Value,
}

/// Initialize result
#[derive(Debug, Clone, Serialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourcesCapability {
    pub subscribe: bool,
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Tool definition
#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Tool call parameters
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: Value,
}

/// Tool call result
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
}

/// Resource read parameters
#[derive(Debug, Clone, Deserialize)]
pub struct ResourceReadParams {
    pub uri: String,
}

/// Resource definition (simplified for protocol)
#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ResourceMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ResourceAnnotations>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

/// Resource content
#[derive(Debug, Clone, Serialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>, // Base64 encoded binary
}

/// Prompt definition
#[derive(Debug, Clone, Serialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}

/// Prompt message
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role")]
pub enum PromptMessage {
    #[serde(rename = "user")]
    User { content: Content },
    #[serde(rename = "assistant")]
    Assistant { content: Content },
    #[serde(rename = "system")]
    System { content: Content },
}

/// Error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}
