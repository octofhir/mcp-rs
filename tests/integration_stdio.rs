//! Integration tests for stdio transport

use anyhow::Result;
use octofhir_mcp::transport::{JsonRpcMessage};
use serde_json::json;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;
use tokio::process::{Command as TokioCommand, ChildStdin, ChildStdout};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

mod common;

use common::{test_utils, assertions, MockMcpClient};

/// Test basic MCP protocol over stdio
#[tokio::test]
async fn test_stdio_initialize_handshake() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Send initialize message
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    
    // Wait for response
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        // Check that capabilities are returned
        if let JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assert!(result_val.get("capabilities").is_some(), "Response should include capabilities");
                assert!(result_val.get("serverInfo").is_some(), "Response should include server info");
            }
        }
    }
    
    Ok(())
}

/// Test tool discovery via stdio
#[tokio::test]
async fn test_stdio_tool_discovery() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // First initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?; // Wait for init response
    
    // Request tools list
    let tools_message = test_utils::create_tools_list_message();
    client.send_message(tools_message).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                let tools = result_val.get("tools").expect("Response should include tools array");
                assert!(tools.is_array(), "Tools should be an array");
                
                let tools_array = tools.as_array().unwrap();
                assert!(!tools_array.is_empty(), "Should have at least one tool");
                
                // Check for FHIRPath tools
                let tool_names: Vec<&str> = tools_array
                    .iter()
                    .filter_map(|tool| tool.get("name")?.as_str())
                    .collect();
                
                assert!(tool_names.contains(&"fhirpath_evaluate"), "Should include fhirpath_evaluate tool");
                assert!(tool_names.contains(&"fhirpath_parse"), "Should include fhirpath_parse tool");
            }
        }
    }
    
    Ok(())
}

/// Test complete tool execution cycle via stdio
#[tokio::test]
async fn test_stdio_tool_execution_cycle() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Execute FHIRPath evaluation
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
        
        if let JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                // Check that we got the expected family name
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Should return at least one value");
                assert_eq!(values[0].as_str().unwrap(), "Doe", "Should return correct family name");
            }
        }
    }
    
    Ok(())
}

/// Test error handling in stdio mode
#[tokio::test]
async fn test_stdio_error_handling() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Send invalid FHIRPath expression
    let patient_resource = test_utils::create_test_patient();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.where(use='official'", // Missing closing parenthesis
            "resource": patient_resource
        })
    );
    
    client.send_message(tool_call).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_error_response(&response_msg);
        
        if let JsonRpcMessage::Response { error, .. } = response_msg {
            if let Some(error_val) = error {
                assert!(error_val.message.contains("syntax"), "Error should mention syntax issue");
            }
        }
    }
    
    Ok(())
}

/// Test shutdown behavior
#[tokio::test]
async fn test_stdio_shutdown_behavior() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Send a few operations
    for _i in 0..3 {
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
        }
    }
    
    // Check that all messages were processed
    assert_eq!(client.sent_count(), 4); // 1 init + 3 tool calls
    assert_eq!(client.received_count(), 4); // 1 init response + 3 tool responses
    
    Ok(())
}

/// Test stdio transport with actual subprocess (integration test)
#[tokio::test]
async fn test_stdio_subprocess_integration() -> Result<()> {
    // Build the binary first to ensure it exists
    let output = Command::new("cargo")
        .args(&["build", "--bin", "octofhir-mcp"])
        .output()
        .expect("Failed to build binary");
    
    if !output.status.success() {
        panic!("Failed to build octofhir-mcp binary: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Start the MCP server as subprocess using tokio::process
    let mut child = TokioCommand::new("target/debug/octofhir-mcp")
        .args(&["--transport", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start octofhir-mcp subprocess");
    
    let stdin = child.stdin.take().expect("Failed to get stdin");
    let stdout = child.stdout.take().expect("Failed to get stdout");
    
    let mut reader = BufReader::new(stdout);
    let mut writer = stdin;
    
    // Send initialize message
    let init_msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "1.0.0",
            "capabilities": {
                "tools": {
                    "listChanged": true
                }
            },
            "clientInfo": {
                "name": "integration-test-client",
                "version": "1.0.0"
            }
        }
    });
    
    let init_line = format!("{}\n", init_msg.to_string());
    writer.write_all(init_line.as_bytes()).await?;
    writer.flush().await?;
    
    // Read response with timeout
    let mut response_line = String::new();
    let result = timeout(Duration::from_secs(10), reader.read_line(&mut response_line)).await;
    
    match result {
        Ok(Ok(_)) => {
            let response: serde_json::Value = serde_json::from_str(&response_line)?;
            
            // Verify initialize response
            assert_eq!(response["jsonrpc"], "2.0");
            assert_eq!(response["id"], 1);
            assert!(response["result"]["capabilities"].is_object());
            assert!(response["result"]["serverInfo"].is_object());
        }
        Ok(Err(e)) => panic!("Failed to read from subprocess: {}", e),
        Err(_) => panic!("Timeout waiting for subprocess response"),
    }
    
    // Clean up
    child.kill().expect("Failed to kill subprocess");
    child.wait().expect("Failed to wait for subprocess");
    
    Ok(())
}

/// Test large message handling via stdio
#[tokio::test]
async fn test_stdio_large_message_handling() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Create a large Bundle resource
    let large_bundle = test_utils::create_test_bundle();
    
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Bundle.entry.count()",
            "resource": large_bundle
        })
    );
    
    client.send_message(tool_call).await?;
    
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                // Check bundle entry count
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert_eq!(values[0].as_i64().unwrap(), 2, "Bundle should have 2 entries");
            }
        }
    }
    
    Ok(())
}

/// Test concurrent requests via stdio (should be processed sequentially)
#[tokio::test]
async fn test_stdio_sequential_processing() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Send multiple tool calls quickly
    for _i in 0..5 {
        let tool_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": format!("Patient.name.family + ' {}'", _i),
                "resource": patient_resource.clone()
            })
        );
        
        client.send_message(tool_call).await?;
    }
    
    // Receive all responses
    for _i in 0..5 {
        let response = client.wait_for_response().await?;
        
        if let Some(response_msg) = response {
            assertions::assert_success_response(&response_msg);
        }
    }
    
    // All requests should be processed
    assert_eq!(client.received_count(), 6); // 1 init + 5 tool responses
    
    Ok(())
}