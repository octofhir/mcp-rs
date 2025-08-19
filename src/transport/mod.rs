//! Transport layer implementations for MCP protocol
//!
//! Supports multiple transport methods:
//! - stdio: Standard input/output for local CLI integration
//! - http: HTTP/SSE for web applications and remote access
//! - websocket: Real-time applications (future implementation)

pub mod stdio;
pub mod http;
pub mod sse_auth;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use serde::{Deserialize, Serialize};

pub use stdio::StdioTransport;
pub use http::HttpTransport;

/// JSON-RPC 2.0 message wrapper for proper serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request {
        jsonrpc: String,
        id: Option<u64>,
        method: String,
        params: Option<Value>,
    },
    Response {
        jsonrpc: String,
        id: Option<u64>,
        result: Option<Value>,
        error: Option<McpError>,
    },
    Notification {
        jsonrpc: String,
        method: String,
        params: Option<Value>,
    },
}

/// MCP message types (internal representation)
#[derive(Debug, Clone)]
pub enum McpMessage {
    Initialize {
        id: u64,
        params: InitializeParams,
    },
    ToolsList {
        id: u64,
    },
    ToolsCall {
        id: u64,
        params: ToolsCallParams,
    },
    Notification {
        method: String,
        params: Option<Value>,
    },
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<McpError>,
    },
}

/// Initialize parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// Client capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
    pub prompts: Option<PromptsCapability>,
    pub logging: Option<LoggingCapability>,
}

/// Client info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Tools capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    pub list_changed: Option<bool>,
}

/// Resources capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    pub list_changed: Option<bool>,
    pub subscribe: Option<bool>,
}

/// Prompts capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    pub list_changed: Option<bool>,
}

/// Logging capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingCapability;

/// Tool call parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCallParams {
    pub name: String,
    pub arguments: Option<Value>,
}

/// MCP error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl McpMessage {
    /// Convert MCP message to JSON-RPC message for serialization
    pub fn to_jsonrpc(&self) -> JsonRpcMessage {
        match self {
            McpMessage::Initialize { id, params } => JsonRpcMessage::Request {
                jsonrpc: "2.0".to_string(),
                id: Some(*id),
                method: "initialize".to_string(),
                params: serde_json::to_value(params).ok(),
            },
            McpMessage::ToolsList { id } => JsonRpcMessage::Request {
                jsonrpc: "2.0".to_string(),
                id: Some(*id),
                method: "tools/list".to_string(),
                params: None,
            },
            McpMessage::ToolsCall { id, params } => JsonRpcMessage::Request {
                jsonrpc: "2.0".to_string(),
                id: Some(*id),
                method: "tools/call".to_string(),
                params: serde_json::to_value(params).ok(),
            },
            McpMessage::Notification { method, params } => JsonRpcMessage::Notification {
                jsonrpc: "2.0".to_string(),
                method: method.clone(),
                params: params.clone(),
            },
            McpMessage::Response { id, result, error } => JsonRpcMessage::Response {
                jsonrpc: "2.0".to_string(),
                id: Some(*id),
                result: result.clone(),
                error: error.clone(),
            },
        }
    }

    /// Convert JSON-RPC message to MCP message
    pub fn from_jsonrpc(jsonrpc: JsonRpcMessage) -> Result<Self> {
        match jsonrpc {
            JsonRpcMessage::Request { id, method, params, .. } => {
                let id = id.ok_or_else(|| anyhow::Error::msg("Missing request ID"))?;
                match method.as_str() {
                    "initialize" => {
                        let params: InitializeParams = match params {
                            Some(p) => serde_json::from_value(p)
                                .map_err(|e| anyhow::Error::new(e).context("Failed to parse initialize params"))?,
                            None => return Err(anyhow::Error::msg("Missing initialize params")),
                        };
                        Ok(McpMessage::Initialize { id, params })
                    }
                    "tools/list" => Ok(McpMessage::ToolsList { id }),
                    "tools/call" => {
                        let params: ToolsCallParams = match params {
                            Some(p) => serde_json::from_value(p)
                                .map_err(|e| anyhow::Error::new(e).context("Failed to parse tool call params"))?,
                            None => return Err(anyhow::Error::msg("Missing tool call params")),
                        };
                        Ok(McpMessage::ToolsCall { id, params })
                    }
                    _ => Err(anyhow::Error::msg(format!("Unknown method: {}", method))),
                }
            }
            JsonRpcMessage::Response { id, result, error, .. } => {
                let id = id.ok_or_else(|| anyhow::Error::msg("Missing response ID"))?;
                Ok(McpMessage::Response { id, result, error })
            }
            JsonRpcMessage::Notification { method, params, .. } => {
                Ok(McpMessage::Notification { method, params })
            }
        }
    }
}

impl JsonRpcMessage {
    /// Create JsonRpcMessage from a JSON Value
    pub fn from_json_value(value: Value) -> Result<Self> {
        serde_json::from_value(value).map_err(|e| anyhow::Error::new(e).context("Failed to parse JSON-RPC message"))
    }
    
    /// Create JsonRpcMessage from MCP message  
    pub fn from_mcp_message(mcp_message: McpMessage) -> Self {
        mcp_message.to_jsonrpc()
    }
    
    /// Convert to MCP message
    pub fn to_mcp_message(self) -> Result<McpMessage> {
        McpMessage::from_jsonrpc(self)
    }
    
    /// Convert to JSON Value
    pub fn to_json_value(self) -> Value {
        serde_json::to_value(self).expect("JsonRpcMessage should always serialize")
    }
}

/// Message handler trait for processing incoming MCP messages
#[async_trait]
pub trait MessageHandler {
    /// Handle an incoming MCP message
    async fn handle_message(&self, message: McpMessage) -> Result<Option<McpMessage>>;
}

/// Trait for all transport implementations
#[async_trait]
pub trait Transport {
    /// Start the transport and begin handling connections
    async fn start(&self, handler: Box<dyn MessageHandler + Send + Sync>) -> Result<()>;
    
    /// Stop the transport gracefully
    async fn shutdown(&self) -> Result<()>;
    
    /// Send a message through the transport
    async fn send_message(&self, message: McpMessage) -> Result<()>;
}