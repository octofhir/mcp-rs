//! Protocol compliance and error handling tests

use anyhow::Result;
use serde_json::json;

mod common;

use common::{test_utils, assertions, MockMcpClient};

/// Test MCP protocol compliance - initialization handshake
#[tokio::test]
async fn test_mcp_protocol_initialization() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Test proper MCP initialization sequence
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        // Verify initialization response structure
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                // Check required fields in initialize response
                assert!(result_val.get("capabilities").is_some(), "Initialize response must include capabilities");
                assert!(result_val.get("serverInfo").is_some(), "Initialize response must include serverInfo");
                assert!(result_val.get("protocolVersion").is_some(), "Initialize response must include protocolVersion");
                
                // Verify serverInfo structure
                let server_info = result_val.get("serverInfo").unwrap();
                assert!(server_info.get("name").is_some(), "ServerInfo must include name");
                assert!(server_info.get("version").is_some(), "ServerInfo must include version");
                
                // Verify capabilities structure
                let capabilities = result_val.get("capabilities").unwrap();
                assert!(capabilities.get("tools").is_some(), "Capabilities must include tools");
            }
        }
    }
    
    Ok(())
}

/// Test tools/list method returns correct structure
#[tokio::test]
async fn test_mcp_tools_list_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize first
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Request tools list
    let tools_message = test_utils::create_tools_list_message();
    client.send_message(tools_message).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                // Verify tools list structure
                let tools = result_val.get("tools").expect("Response must include tools array");
                assert!(tools.is_array(), "Tools must be an array");
                
                let tools_array = tools.as_array().unwrap();
                
                // Check each tool has required fields
                for tool in tools_array {
                    assert!(tool.get("name").is_some(), "Tool must have name");
                    assert!(tool.get("description").is_some(), "Tool must have description");
                    assert!(tool.get("inputSchema").is_some(), "Tool must have inputSchema");
                    
                    // Verify tool name format
                    let name = tool.get("name").unwrap().as_str().unwrap();
                    assert!(!name.is_empty(), "Tool name cannot be empty");
                    assert!(name.chars().all(|c| c.is_alphanumeric() || c == '_'), "Tool name must be alphanumeric with underscores");
                }
                
                // Verify we have expected FHIRPath tools
                let tool_names: Vec<&str> = tools_array
                    .iter()
                    .filter_map(|tool| tool.get("name")?.as_str())
                    .collect();
                
                assert!(tool_names.contains(&"fhirpath_evaluate"), "Must include fhirpath_evaluate tool");
                assert!(tool_names.contains(&"fhirpath_parse"), "Must include fhirpath_parse tool");
            }
        }
    }
    
    Ok(())
}

/// Test complete tool execution cycle compliance
#[tokio::test]
async fn test_mcp_tool_execution_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Execute tool with proper parameters
    let patient_resource = test_utils::create_test_patient();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": patient_resource
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                // Verify tool execution response structure
                assert!(result_val.get("content").is_some() || result_val.get("isError").is_some() || result_val.get("values").is_some(),
                    "Tool response must have content, isError, or values field");
                
                // For FHIRPath tools, check specific structure
                if result_val.get("values").is_some() {
                    assertions::assert_fhirpath_result(&result_val);
                }
            }
        }
    }
    
    Ok(())
}

/// Test JSON-RPC 2.0 compliance
#[tokio::test]
async fn test_jsonrpc_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Test that all messages follow JSON-RPC 2.0 spec
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Verify JSON-RPC 2.0 structure
        match response_msg {
            octofhir_mcp::transport::JsonRpcMessage::Response { jsonrpc, id, .. } => {
                assert_eq!(jsonrpc, "2.0", "Must use JSON-RPC 2.0");
                assert!(id.is_some(), "Response must include ID from request");
            }
            octofhir_mcp::transport::JsonRpcMessage::Error { jsonrpc, id, .. } => {
                assert_eq!(jsonrpc, "2.0", "Must use JSON-RPC 2.0");
                assert!(id.is_some(), "Error response must include ID from request");
            }
            _ => panic!("Unexpected message type for response"),
        }
    }
    
    Ok(())
}

/// Test error handling compliance
#[tokio::test]
async fn test_error_handling_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test invalid tool name
    let invalid_tool_call = test_utils::create_tool_call_message(
        "nonexistent_tool",
        json!({
            "some": "parameter"
        })
    );
    
    client.send_message(invalid_tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Should get either an error response or a result with isError=true
        match response_msg {
            octofhir_mcp::transport::JsonRpcMessage::Response { error: Some(error), .. } => {
                // Verify error structure
                assert!(!error.message.is_empty(), "Error message cannot be empty");
                assert!(error.code != 0, "Error code must be non-zero");
            }
            octofhir_mcp::transport::JsonRpcMessage::Response { result: Some(result), .. } => {
                // Check if result indicates error
                if let Some(is_error) = result.get("isError") {
                    assert!(is_error.as_bool().unwrap_or(false), "Tool should indicate error for invalid tool");
                }
            }
            octofhir_mcp::transport::JsonRpcMessage::Error { error, .. } => {
                // Direct error response is also valid
                assert!(!error.message.is_empty(), "Error message cannot be empty");
            }
            _ => panic!("Expected error response for invalid tool"),
        }
    }
    
    Ok(())
}

/// Test parameter validation compliance
#[tokio::test]
async fn test_parameter_validation_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test missing required parameters
    let invalid_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            // Missing required 'expression' and 'resource' parameters
        })
    );
    
    client.send_message(invalid_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Should indicate parameter validation error
        assertions::assert_error_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { error: Some(error), .. } = response_msg {
            assert!(error.message.to_lowercase().contains("parameter") || 
                    error.message.to_lowercase().contains("required") ||
                    error.message.to_lowercase().contains("missing"),
                "Error message should indicate parameter issue");
        }
    }
    
    // Test invalid parameter types
    let invalid_type_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": 123, // Should be string
            "resource": "not an object" // Should be object
        })
    );
    
    client.send_message(invalid_type_call).await?;
    let response2 = client.wait_for_response().await?;
    
    if let Some(response_msg) = response2 {
        // Should indicate type validation error
        assertions::assert_error_response(&response_msg);
    }
    
    Ok(())
}

/// Test timeout and resource limits compliance
#[tokio::test]
async fn test_resource_limits_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test with very large resource
    let large_bundle = json!({
        "resourceType": "Bundle",
        "entry": (0..1000).map(|i| json!({
            "resource": {
                "resourceType": "Patient",
                "id": format!("patient-{}", i),
                "name": [{"family": format!("Family{}", i)}]
            }
        })).collect::<Vec<_>>()
    });
    
    let large_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Bundle.entry.count()",
            "resource": large_bundle
        })
    );
    
    client.send_message(large_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Should either succeed or fail gracefully with resource limit error
        match response_msg {
            octofhir_mcp::transport::JsonRpcMessage::Response { result: Some(_), .. } => {
                // Success is acceptable
            }
            octofhir_mcp::transport::JsonRpcMessage::Response { error: Some(error), .. } => {
                // Resource limit error is acceptable
                assert!(!error.message.is_empty(), "Error message should be provided");
            }
            _ => {
                // Other response types are not expected
            }
        }
    }
    
    Ok(())
}

/// Test concurrent request handling compliance
#[tokio::test]
async fn test_concurrent_request_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Send multiple requests quickly
    for i in 0..5 {
        let tool_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": format!("Patient.name.family + ' {}'", i),
                "resource": patient_resource.clone()
            })
        );
        
        client.send_message(tool_call).await?;
    }
    
    // Receive all responses
    let mut responses = Vec::new();
    for _i in 0..5 {
        let response = client.wait_for_response().await?;
        if let Some(response_msg) = response {
            responses.push(response_msg);
        }
    }
    
    // Verify all responses were received
    assert_eq!(responses.len(), 5, "Should receive response for each request");
    
    // Verify each response is valid
    for response in responses {
        // Should be either success or error, but not malformed
        match response {
            octofhir_mcp::transport::JsonRpcMessage::Response { .. } => {
                // Valid response structure
            }
            octofhir_mcp::transport::JsonRpcMessage::Error { .. } => {
                // Valid error structure
            }
            _ => panic!("Unexpected response type"),
        }
    }
    
    Ok(())
}

/// Test security and input sanitization compliance
#[tokio::test]
async fn test_security_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test potentially malicious expressions
    let malicious_expressions = vec![
        "'; DROP TABLE patients; --",
        "<script>alert('xss')</script>",
        "../../../../etc/passwd",
        "\\x00\\x01\\x02", // Binary data
        "a".repeat(10000), // Very long string
    ];
    
    let patient_resource = test_utils::create_test_patient();
    
    for malicious_expr in malicious_expressions {
        let malicious_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": malicious_expr,
                "resource": patient_resource.clone()
            })
        );
        
        client.send_message(malicious_call).await?;
        let response = client.wait_for_response().await?;
        
        if let Some(response_msg) = response {
            // Should either reject with error or sanitize safely
            match response_msg {
                octofhir_mcp::transport::JsonRpcMessage::Response { error: Some(_), .. } => {
                    // Error response is expected for malicious input
                }
                octofhir_mcp::transport::JsonRpcMessage::Response { result: Some(result), .. } => {
                    // If processed, should not contain the malicious content in output
                    let result_str = serde_json::to_string(&result).unwrap_or_default();
                    assert!(!result_str.contains("DROP TABLE"), "Output should not contain SQL injection");
                    assert!(!result_str.contains("<script>"), "Output should not contain XSS");
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}

/// Test message ordering and ID compliance
#[tokio::test]
async fn test_message_ordering_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    let init_response = client.wait_for_response().await?;
    
    // Verify init response has correct ID
    if let Some(octofhir_mcp::transport::JsonRpcMessage::Response { id, .. }) = init_response {
        assert_eq!(id, Some(1), "Initialize response should have ID 1");
    }
    
    // Send multiple requests with different IDs
    let patient_resource = test_utils::create_test_patient();
    
    let tool_call_2 = test_utils::create_tool_call_message(
        "fhirpath_parse",
        json!({
            "expression": "Patient.name.family"
        })
    );
    
    let tool_call_3 = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": patient_resource
        })
    );
    
    client.send_message(tool_call_2).await?;
    client.send_message(tool_call_3).await?;
    
    // Receive responses and verify IDs
    let response_2 = client.wait_for_response().await?;
    let response_3 = client.wait_for_response().await?;
    
    if let Some(octofhir_mcp::transport::JsonRpcMessage::Response { id, .. }) = response_2 {
        assert!(id.is_some(), "Response should have ID");
    }
    
    if let Some(octofhir_mcp::transport::JsonRpcMessage::Response { id, .. }) = response_3 {
        assert!(id.is_some(), "Response should have ID");
    }
    
    Ok(())
}

/// Test graceful shutdown compliance
#[tokio::test]
async fn test_graceful_shutdown_compliance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Send a few operations
    let patient_resource = test_utils::create_test_patient();
    
    for _i in 0..3 {
        let tool_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": "Patient.name.family",
                "resource": patient_resource.clone()
            })
        );
        
        client.send_message(tool_call).await?;
        let response = client.wait_for_response().await?;
        
        if let Some(response_msg) = response {
            // Each response should be properly formed
            match response_msg {
                octofhir_mcp::transport::JsonRpcMessage::Response { .. } => {}
                octofhir_mcp::transport::JsonRpcMessage::Error { .. } => {}
                _ => panic!("Unexpected message type"),
            }
        }
    }
    
    // Verify all messages were processed
    assert!(client.sent_count() >= 4, "Should have sent init + tool calls");
    assert!(client.received_count() >= 4, "Should have received corresponding responses");
    
    Ok(())
}