//! Core MCP server implementation

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, instrument, warn};

use crate::tools::fhirpath::FhirPathEvaluateRequest;
use crate::transport::{MessageHandler, McpMessage, McpError};
use crate::fhirpath_engine::{get_shared_engine, initialize_shared_engine_with_config, FhirEngineConfig};

/// Main MCP Server implementing FHIRPath tools
#[derive(Default, Clone)]
pub struct McpServer {
    config: crate::config::ServerConfig,
}

/// MCP server initialization result
#[derive(Debug, Clone)]
pub struct ServerInitResult {
    pub protocol_version: String,
    pub server_name: String,
    pub server_version: String,
    pub capabilities: ServerCapabilitiesInfo,
    pub instructions: Option<String>,
}

/// Server capabilities information
#[derive(Debug, Clone)]
pub struct ServerCapabilitiesInfo {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub logging: bool,
}

/// Tool call result
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}

/// Tool definition
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpServer {
    /// Create a new MCP server instance
    pub fn new(config: crate::config::ServerConfig) -> Self {
        Self { config }
    }

    /// Get server configuration
    pub fn config(&self) -> &crate::config::ServerConfig {
        &self.config
    }

    /// Start the MCP server
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting OctoFHIR MCP Server v{}", crate::VERSION);
        info!("Server configuration: {:?}", self.config);

        // Initialize the shared FHIRPath engine with configuration
        let fhir_config = FhirEngineConfig {
            fhir_version: self.config.fhir_version.clone(),
            additional_packages: self.config.additional_packages.clone(),
        };
        initialize_shared_engine_with_config(fhir_config).await?;

        // Server startup logic will be completed in transport tasks
        info!("MCP server core initialized successfully");
        Ok(())
    }

    /// Handle FHIRPath evaluation tool call using shared engine
    pub async fn handle_fhirpath_evaluate(
        &self,
        expression: String,
        resource: Value,
        context: Option<Value>,
    ) -> Result<ToolCallResult> {
        let request = FhirPathEvaluateRequest {
            expression,
            resource,
            context,
        };

        match crate::tools::fhirpath::evaluate_fhirpath(request).await {
            Ok(response) => {
                let content = serde_json::to_string(&response)?;
                Ok(ToolCallResult {
                    success: true,
                    content,
                    error: None,
                })
            }
            Err(e) => {
                let error_msg = format!("FHIRPath evaluation failed: {}", e);
                Ok(ToolCallResult {
                    success: false,
                    content: String::new(),
                    error: Some(error_msg),
                })
            }
        }
    }

    /// Handle FHIRPath parsing tool call using shared engine
    pub async fn handle_fhirpath_parse(&self, expression: String) -> Result<ToolCallResult> {
        let engine = get_shared_engine().await?;

        match engine.parse_expression(&expression).await {
            Ok(()) => {
                let response = serde_json::json!({
                    "expression": expression,
                    "valid": true,
                    "ast": null,
                    "diagnostics": []
                });

                Ok(ToolCallResult {
                    success: true,
                    content: response.to_string(),
                    error: None,
                })
            }
            Err(e) => {
                let response = serde_json::json!({
                    "expression": expression,
                    "valid": false,
                    "ast": null,
                    "diagnostics": [e.to_string()]
                });

                Ok(ToolCallResult {
                    success: true,
                    content: response.to_string(),
                    error: None,
                })
            }
        }
    }

    /// Handle FHIRPath extraction tool call using shared engine
    pub async fn handle_fhirpath_extract(
        &self,
        resource: Value,
        expressions: Vec<String>,
    ) -> Result<ToolCallResult> {
        let engine = get_shared_engine().await?;
        let mut extractions = Vec::new();

        for expr in expressions {
            match engine.evaluate(&expr, resource.clone()).await {
                Ok(fhir_value) => {
                    let collection = crate::tools::fhirpath::fhirpath_value_to_collection(fhir_value);
                    let result: Vec<Value> = collection.iter()
                        .map(|v| crate::tools::fhirpath::fhirpath_value_to_json(v))
                        .collect();

                    extractions.push(serde_json::json!({
                        "expression": expr,
                        "result": result,
                        "success": true
                    }));
                }
                Err(e) => {
                    extractions.push(serde_json::json!({
                        "expression": expr,
                        "result": [],
                        "success": false,
                        "error": e.to_string()
                    }));
                }
            }
        }

        let response = serde_json::json!({
            "resource_type": resource.get("resourceType").unwrap_or(&Value::Null),
            "extractions": extractions
        });

        Ok(ToolCallResult {
            success: true,
            content: response.to_string(),
            error: None,
        })
    }

    /// Handle FHIRPath explanation tool call using shared engine
    pub async fn handle_fhirpath_explain(
        &self,
        expression: String,
        resource: Option<Value>,
    ) -> Result<ToolCallResult> {
        let engine = get_shared_engine().await?;

        // Try to parse the expression first
        let parse_result = engine.parse_expression(&expression).await;
        let parsed = parse_result.is_ok();

        // If resource is provided, try evaluation for additional context
        let evaluation_result = if let Some(res) = &resource {
            match engine.evaluate(&expression, res.clone()).await {
                Ok(fhir_value) => {
                    let collection = crate::tools::fhirpath::fhirpath_value_to_collection(fhir_value);
                    Some(collection.iter()
                        .map(|v| crate::tools::fhirpath::fhirpath_value_to_json(v))
                        .collect::<Vec<_>>())
                }
                Err(_) => None
            }
        } else {
            None
        };

        let response = serde_json::json!({
            "expression": expression,
            "parsed": parsed,
            "explanation": if parsed {
                format!("FHIRPath expression '{}' is syntactically valid", expression)
            } else {
                format!("FHIRPath expression '{}' has syntax errors", expression)
            },
            "steps": [], // TODO: Implement detailed step breakdown
            "resource_context": resource.is_some(),
            "evaluation_result": evaluation_result,
            "parse_error": if !parsed {
                parse_result.err().map(|e| e.to_string())
            } else {
                None
            }
        });

        Ok(ToolCallResult {
            success: true,
            content: response.to_string(),
            error: None,
        })
    }

    /// Get MCP initialize result
    pub fn get_initialize_result(&self) -> ServerInitResult {
        ServerInitResult {
            protocol_version: "2024-11-05".to_string(),
            server_name: "octofhir-mcp".to_string(),
            server_version: crate::VERSION.to_string(),
            capabilities: ServerCapabilitiesInfo {
                tools: true,
                resources: false, // Will be implemented in later phases
                prompts: false,   // Will be implemented in later phases
                logging: false,   // Will be implemented in later phases
            },
            instructions: Some("OctoFHIR MCP Server - High-performance FHIRPath evaluation and FHIR tooling".to_string()),
        }
    }

    /// Get available tools list
    pub fn get_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "fhirpath_evaluate".to_string(),
                description: "Evaluate FHIRPath expressions against FHIR resources with comprehensive diagnostics".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "FHIRPath expression to evaluate"
                        },
                        "resource": {
                            "type": "object",
                            "description": "FHIR resource to evaluate against"
                        },
                        "context": {
                            "type": "object",
                            "description": "Optional context for evaluation"
                        }
                    },
                    "required": ["expression", "resource"]
                }),
            },
            ToolDefinition {
                name: "fhirpath_parse".to_string(),
                description: "Parse and validate FHIRPath expressions, returning AST and syntax diagnostics".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "FHIRPath expression to parse"
                        }
                    },
                    "required": ["expression"]
                }),
            },
            ToolDefinition {
                name: "fhirpath_extract".to_string(),
                description: "Extract specific data patterns from FHIR resources using optimized FHIRPath queries".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "resource": {
                            "type": "object",
                            "description": "FHIR resource to extract from"
                        },
                        "expressions": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "FHIRPath expressions for extraction"
                        }
                    },
                    "required": ["resource", "expressions"]
                }),
            },
            ToolDefinition {
                name: "fhirpath_explain".to_string(),
                description: "Provide detailed explanations of FHIRPath expressions, including step-by-step evaluation".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "FHIRPath expression to explain"
                        },
                        "resource": {
                            "type": "object",
                            "description": "Optional FHIR resource for context"
                        }
                    },
                    "required": ["expression"]
                }),
            },
        ]
    }

    /// Handle MCP tool call by name
    pub async fn handle_tool_call(&self, name: &str, params: Value) -> Result<ToolCallResult> {
        match name {
            "fhirpath_evaluate" => {
                let expression = params["expression"].as_str()
                    .ok_or_else(|| anyhow::Error::msg("Missing 'expression' parameter"))?
                    .to_string();
                let resource = params["resource"].clone();
                let context = params.get("context").cloned();

                self.handle_fhirpath_evaluate(expression, resource, context).await
            }
            "fhirpath_parse" => {
                let expression = params["expression"].as_str()
                    .ok_or_else(|| anyhow::Error::msg("Missing 'expression' parameter"))?
                    .to_string();

                self.handle_fhirpath_parse(expression).await
            }
            "fhirpath_extract" => {
                let resource = params["resource"].clone();
                let expressions: Vec<String> = params["expressions"].as_array()
                    .ok_or_else(|| anyhow::Error::msg("Missing or invalid 'expressions' parameter"))?
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();

                self.handle_fhirpath_extract(resource, expressions).await
            }
            "fhirpath_explain" => {
                let expression = params["expression"].as_str()
                    .ok_or_else(|| anyhow::Error::msg("Missing 'expression' parameter"))?
                    .to_string();
                let resource = params.get("resource").cloned();

                self.handle_fhirpath_explain(expression, resource).await
            }
            _ => Err(anyhow::Error::msg(format!("Unknown tool: {}", name))),
        }
    }
}

/// Implementation of MessageHandler for McpServer
#[async_trait]
impl MessageHandler for McpServer {
    async fn handle_message(&self, message: McpMessage) -> Result<Option<McpMessage>> {
        match message {
            McpMessage::Initialize { id, params } => {
                info!("Received initialize request from client: {}", params.client_info.name);

                let init_result = self.get_initialize_result();
                let response = McpMessage::Response {
                    id,
                    result: Some(serde_json::json!({
                        "protocolVersion": init_result.protocol_version,
                        "serverInfo": {
                            "name": init_result.server_name,
                            "version": init_result.server_version
                        },
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            }
                        },
                        "instructions": init_result.instructions
                    })),
                    error: None,
                };

                Ok(Some(response))
            }

            McpMessage::ToolsList { id } => {
                info!("Received tools list request");

                let tools = self.get_tools();
                let tools_json: Vec<Value> = tools.into_iter().map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.input_schema
                    })
                }).collect();

                let response = McpMessage::Response {
                    id,
                    result: Some(serde_json::json!({
                        "tools": tools_json
                    })),
                    error: None,
                };

                Ok(Some(response))
            }

            McpMessage::ToolsCall { id, params } => {
                info!("Received tool call: {}", params.name);

                let tool_params = params.arguments.unwrap_or(Value::Null);
                match self.handle_tool_call(&params.name, tool_params).await {
                    Ok(result) => {
                        let response = if result.success {
                            McpMessage::Response {
                                id,
                                result: Some(serde_json::json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": result.content
                                        }
                                    ]
                                })),
                                error: None,
                            }
                        } else {
                            McpMessage::Response {
                                id,
                                result: None,
                                error: Some(McpError {
                                    code: -1,
                                    message: result.error.unwrap_or("Unknown error".to_string()),
                                    data: None,
                                }),
                            }
                        };

                        Ok(Some(response))
                    }
                    Err(e) => {
                        warn!("Tool call failed: {}", e);
                        let response = McpMessage::Response {
                            id,
                            result: None,
                            error: Some(McpError {
                                code: -1,
                                message: e.to_string(),
                                data: None,
                            }),
                        };

                        Ok(Some(response))
                    }
                }
            }

            McpMessage::Notification { method, params: _ } => {
                info!("Received notification: {}", method);
                // Handle notifications (no response needed)
                Ok(None)
            }

            McpMessage::Response { id: _, result: _, error: _ } => {
                // This server doesn't initiate requests, so we shouldn't receive responses
                warn!("Received unexpected response message");
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_server() -> McpServer {
        let config = crate::config::ServerConfig::default();
        McpServer::new(config)
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let server = create_test_server();
        let result = server.get_initialize_result();

        assert_eq!(result.protocol_version, "2024-11-05");
        assert_eq!(result.server_name, "octofhir-mcp");
        assert_eq!(result.server_version, crate::VERSION);
        assert!(result.capabilities.tools);
        assert!(!result.capabilities.resources);
        assert!(!result.capabilities.prompts);
    }

    #[tokio::test]
    async fn test_tools_list() {
        let server = create_test_server();
        let tools = server.get_tools();

        assert_eq!(tools.len(), 4);

        let tool_names: Vec<&String> = tools.iter().map(|t| &t.name).collect();
        assert!(tool_names.contains(&&"fhirpath_evaluate".to_string()));
        assert!(tool_names.contains(&&"fhirpath_parse".to_string()));
        assert!(tool_names.contains(&&"fhirpath_extract".to_string()));
        assert!(tool_names.contains(&&"fhirpath_explain".to_string()));
    }

    #[tokio::test]
    async fn test_fhirpath_parse_tool() {
        let server = create_test_server();
        let params = json!({
            "expression": "Patient.name.family"
        });

        let result = server.handle_tool_call("fhirpath_parse", params).await;
        assert!(result.is_ok());

        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(!tool_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let server = create_test_server();
        let params = json!({});

        let result = server.handle_tool_call("unknown_tool", params).await;
        assert!(result.is_err());
    }
}
