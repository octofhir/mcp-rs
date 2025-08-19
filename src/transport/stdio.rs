//! Standard I/O transport implementation

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    sync::Mutex,
};
use tracing::{debug, error, info, warn};

use super::{MessageHandler, McpMessage, JsonRpcMessage};

/// Standard I/O transport for local CLI integration with MCP clients
pub struct StdioTransport {
    writer: Arc<Mutex<BufWriter<tokio::io::Stdout>>>,
    shutdown_signal: Arc<Mutex<bool>>,
}

impl StdioTransport {
    /// Create a new stdio transport instance
    pub fn new() -> Self {
        let stdout = tokio::io::stdout();
        let writer = Arc::new(Mutex::new(BufWriter::new(stdout)));
        let shutdown_signal = Arc::new(Mutex::new(false));
        
        Self {
            writer,
            shutdown_signal,
        }
    }

    /// Check if shutdown has been requested
    async fn is_shutdown_requested(&self) -> bool {
        *self.shutdown_signal.lock().await
    }

    /// Request shutdown
    async fn request_shutdown(&self) {
        *self.shutdown_signal.lock().await = true;
    }

    /// Send a JSON-RPC message to stdout
    async fn write_message(&self, message: &McpMessage) -> Result<()> {
        let jsonrpc_message = message.to_jsonrpc();
        let json_str = serde_json::to_string(&jsonrpc_message)
            .context("Failed to serialize message to JSON")?;
        
        debug!("Sending message: {}", json_str);
        
        let mut writer = self.writer.lock().await;
        writer.write_all(json_str.as_bytes()).await
            .context("Failed to write message to stdout")?;
        writer.write_all(b"\n").await
            .context("Failed to write newline to stdout")?;
        writer.flush().await
            .context("Failed to flush stdout")?;
        
        Ok(())
    }

    /// Read and parse a JSON-RPC message from stdin
    async fn read_message(reader: &mut BufReader<tokio::io::Stdin>) -> Result<Option<McpMessage>> {
        let mut line = String::new();
        
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF reached
                debug!("EOF received on stdin");
                return Ok(None);
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    debug!("Empty line received, ignoring");
                    return Ok(None);
                }
                
                debug!("Received line: {}", trimmed);
                
                match serde_json::from_str::<JsonRpcMessage>(trimmed) {
                    Ok(jsonrpc_message) => {
                        match McpMessage::from_jsonrpc(jsonrpc_message) {
                            Ok(message) => Ok(Some(message)),
                            Err(e) => {
                                error!("Failed to convert JSON-RPC to MCP message: {} - Line: {}", e, trimmed);
                                Err(e)
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse JSON message: {} - Line: {}", e, trimmed);
                        Err(anyhow::Error::new(e).context("Failed to parse JSON-RPC message"))
                    }
                }
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                Err(anyhow::Error::new(e).context("Failed to read from stdin"))
            }
        }
    }

    /// Main message processing loop
    async fn process_messages(&self, handler: Box<dyn MessageHandler + Send + Sync>) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        
        info!("Starting stdio message processing loop");
        
        loop {
            // Check for shutdown signal
            if self.is_shutdown_requested().await {
                info!("Shutdown requested, stopping message processing");
                break;
            }
            
            // Read next message
            match Self::read_message(&mut reader).await {
                Ok(Some(message)) => {
                    debug!("Processing message: {:?}", message);
                    
                    // Handle the message
                    match handler.handle_message(message).await {
                        Ok(Some(response)) => {
                            // Send response if one is generated
                            if let Err(e) = self.write_message(&response).await {
                                error!("Failed to send response: {}", e);
                            }
                        }
                        Ok(None) => {
                            // No response needed (e.g., for notifications)
                            debug!("No response generated for message");
                        }
                        Err(e) => {
                            error!("Handler error: {}", e);
                            // Could send error response here if needed
                        }
                    }
                }
                Ok(None) => {
                    // EOF or empty line, continue or break based on context
                    debug!("No message read, continuing");
                    continue;
                }
                Err(e) => {
                    warn!("Error reading message: {}", e);
                    // Continue processing despite errors
                    continue;
                }
            }
        }
        
        info!("Message processing loop ended");
        Ok(())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl super::Transport for StdioTransport {
    /// Start the stdio transport and begin message processing
    async fn start(&self, handler: Box<dyn MessageHandler + Send + Sync>) -> Result<()> {
        info!("Starting stdio transport for MCP communication");
        
        // Reset shutdown signal
        *self.shutdown_signal.lock().await = false;
        
        // Start processing messages
        self.process_messages(handler).await
    }

    /// Shutdown the stdio transport
    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down stdio transport");
        
        // Signal shutdown
        self.request_shutdown().await;
        
        // Flush any remaining output
        if let Ok(mut writer) = self.writer.try_lock() {
            if let Err(e) = writer.flush().await {
                warn!("Failed to flush output during shutdown: {}", e);
            }
        }
        
        debug!("Stdio transport shutdown completed");
        Ok(())
    }
    
    /// Send a message through the stdio transport
    async fn send_message(&self, message: McpMessage) -> Result<()> {
        self.write_message(&message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{InitializeParams, ClientInfo, ClientCapabilities, JsonRpcMessage};

    #[tokio::test]
    async fn test_stdio_transport_creation() {
        let transport = StdioTransport::new();
        assert!(!transport.is_shutdown_requested().await);
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let transport = StdioTransport::new();
        assert!(!transport.is_shutdown_requested().await);
        
        transport.request_shutdown().await;
        assert!(transport.is_shutdown_requested().await);
    }

    #[test]
    fn test_message_serialization() {
        let message = McpMessage::Initialize {
            id: 1,
            params: InitializeParams {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ClientCapabilities { tools: None, resources: None, prompts: None, logging: None },
                client_info: ClientInfo { 
                    name: "test-client".to_string(), 
                    version: "1.0.0".to_string() 
                },
            },
        };

        let jsonrpc_message = message.to_jsonrpc();
        let json_str = serde_json::to_string(&jsonrpc_message).expect("Should serialize");
        assert!(json_str.contains("initialize"));
        assert!(json_str.contains("test-client"));
        assert!(json_str.contains("jsonrpc"));
        assert!(json_str.contains("2.0"));
    }

    #[test]
    fn test_message_deserialization() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol_version":"2024-11-05","capabilities":{"tools":null,"resources":null,"prompts":null,"logging":null},"client_info":{"name":"test-client","version":"1.0.0"}}}"#;
        
        let jsonrpc_message: Result<JsonRpcMessage, _> = serde_json::from_str(json_str);
        assert!(jsonrpc_message.is_ok(), "Should deserialize valid JSON-RPC message");
        
        let mcp_message = McpMessage::from_jsonrpc(jsonrpc_message.unwrap());
        assert!(mcp_message.is_ok(), "Should convert to MCP message");
        
        if let Ok(McpMessage::Initialize { params, .. }) = mcp_message {
            assert_eq!(params.client_info.name, "test-client");
        } else {
            panic!("Should parse as Initialize message");
        }
    }
}