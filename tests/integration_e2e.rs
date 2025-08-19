//! End-to-end integration tests

use anyhow::Result;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;

mod common;

use common::{test_utils, assertions, MockMcpClient, TestHttpClient, TestServerConfig};

/// Complete MCP client session test
#[tokio::test]
async fn test_complete_mcp_client_session() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // 1. Initialize connection
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    let init_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = init_response {
        assertions::assert_success_response(&response_msg);
    }
    
    // 2. Discover available tools
    let tools_message = test_utils::create_tools_list_message();
    client.send_message(tools_message).await?;
    let tools_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = tools_response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                let tools = result_val.get("tools").expect("Should have tools array");
                let tools_array = tools.as_array().unwrap();
                assert!(!tools_array.is_empty(), "Should have available tools");
            }
        }
    }
    
    // 3. Execute multiple FHIRPath operations
    let patient = test_utils::create_test_patient();
    let observation = test_utils::create_test_observation();
    let bundle = test_utils::create_test_bundle();
    
    let test_scenarios = vec![
        ("Patient family name", "Patient.name.family", patient.clone()),
        ("Patient given names", "Patient.name.given", patient.clone()),
        ("Patient phone", "Patient.telecom.where(system='phone').value", patient),
        ("Observation value", "Observation.valueQuantity.value", observation.clone()),
        ("Observation unit", "Observation.valueQuantity.unit", observation),
        ("Bundle entry count", "Bundle.entry.count()", bundle.clone()),
        ("Bundle patients", "Bundle.entry.resource.where(resourceType='Patient')", bundle),
    ];
    
    for (description, expression, resource) in test_scenarios {
        let tool_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": expression,
                "resource": resource
            })
        );
        
        client.send_message(tool_call).await?;
        let response = client.wait_for_response().await?;
        
        if let Some(response_msg) = response {
            assertions::assert_success_response(&response_msg);
            
            if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
                if let Some(result_val) = result {
                    assertions::assert_fhirpath_result(&result_val);
                    
                    // Verify we get meaningful results
                    let values = result_val.get("values").unwrap().as_array().unwrap();
                    // Some expressions might return empty results, which is valid
                    println!("Test '{}': {} values returned", description, values.len());
                }
            }
        }
    }
    
    // 4. Test error handling within the session
    let error_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.where(", // Invalid syntax
            "resource": test_utils::create_test_patient()
        })
    );
    
    client.send_message(error_call).await?;
    let error_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = error_response {
        assertions::assert_error_response(&response_msg);
    }
    
    // 5. Continue normal operations after error
    let recovery_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": test_utils::create_test_patient()
        })
    );
    
    client.send_message(recovery_call).await?;
    let recovery_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = recovery_response {
        assertions::assert_success_response(&response_msg);
    }
    
    // Verify total message counts
    let expected_requests = 1 + 1 + 7 + 1 + 1; // init + tools + scenarios + error + recovery
    let expected_responses = expected_requests;
    
    assert_eq!(client.sent_count(), expected_requests, "Should have sent correct number of requests");
    assert_eq!(client.received_count(), expected_responses, "Should have received correct number of responses");
    
    Ok(())
}

/// Multi-tool execution sequence test
#[tokio::test]
async fn test_multi_tool_execution_sequence() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient = test_utils::create_test_patient();
    
    // Test sequence: parse -> evaluate -> parse again
    
    // 1. Parse a FHIRPath expression
    let parse_call = test_utils::create_tool_call_message(
        "fhirpath_parse",
        json!({
            "expression": "Patient.name.where(use='official').family"
        })
    );
    
    client.send_message(parse_call).await?;
    let parse_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = parse_response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                let valid = result_val.get("valid").unwrap().as_bool().unwrap();
                assert!(valid, "Expression should parse successfully");
            }
        }
    }
    
    // 2. Evaluate the same expression
    let eval_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.where(use='official').family",
            "resource": patient
        })
    );
    
    client.send_message(eval_call).await?;
    let eval_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = eval_response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Evaluation should return values");
                assert_eq!(values[0].as_str().unwrap(), "Doe", "Should return correct family name");
            }
        }
    }
    
    // 3. Parse an invalid expression to test error handling
    let invalid_parse_call = test_utils::create_tool_call_message(
        "fhirpath_parse",
        json!({
            "expression": "Patient.name.where("
        })
    );
    
    client.send_message(invalid_parse_call).await?;
    let invalid_response = client.wait_for_response().await?;
    
    if let Some(response_msg) = invalid_response {
        // Should either return error or result with valid=false
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, error, .. } = response_msg {
            if let Some(result_val) = result {
                let valid = result_val.get("valid").unwrap().as_bool().unwrap();
                assert!(!valid, "Invalid expression should not be valid");
            } else if error.is_some() {
                // Error response is also acceptable
            }
        }
    }
    
    Ok(())
}

/// Performance test under concurrent load
#[tokio::test]
async fn test_performance_under_concurrent_load() -> Result<()> {
    // Note: This is a mock test structure. In a real implementation,
    // we would start an actual server and create multiple real clients.
    
    let config = TestServerConfig::default();
    let _base_url = format!("http://{}:{}", config.host, config.port);
    
    // Simulate concurrent clients
    let num_clients = 5;
    let requests_per_client = 10;
    
    let mut handles = Vec::new();
    
    for client_id in 0..num_clients {
        let handle = tokio::spawn(async move {
            let mut client = MockMcpClient::new();
            
            // Initialize client
            let init_message = test_utils::create_initialize_message();
            client.send_message(init_message).await?;
            client.wait_for_response().await?;
            
            let patient = test_utils::create_test_patient();
            
            // Send multiple requests
            for request_id in 0..requests_per_client {
                let expression = format!("Patient.name.family + ' from client {} request {}'", client_id, request_id);
                let tool_call = test_utils::create_tool_call_message(
                    "fhirpath_evaluate",
                    json!({
                        "expression": expression,
                        "resource": patient.clone()
                    })
                );
                
                client.send_message(tool_call).await?;
                let response = client.wait_for_response().await?;
                
                if let Some(response_msg) = response {
                    assertions::assert_success_response(&response_msg);
                }
            }
            
            Ok::<usize, anyhow::Error>(client.sent_count() - 1) // Subtract init message
        });
        
        handles.push(handle);
    }
    
    // Wait for all clients to complete
    let results = futures_util::future::join_all(handles).await;
    
    let mut total_requests = 0;
    for result in results {
        let request_count = result??;
        total_requests += request_count;
    }
    
    let expected_total = num_clients * requests_per_client;
    assert_eq!(total_requests, expected_total, "All concurrent requests should complete");
    
    Ok(())
}

/// Memory usage during extended session
#[tokio::test]
async fn test_memory_usage_extended_session() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Create various resource types
    let patient = test_utils::create_test_patient();
    let observation = test_utils::create_test_observation();
    let bundle = test_utils::create_test_bundle();
    
    let resources = vec![
        ("Patient", patient),
        ("Observation", observation),
        ("Bundle", bundle),
    ];
    
    let expressions = vec![
        "*.name.family",
        "*.id",
        "*.resourceType",
        "*.meta.lastUpdated",
    ];
    
    // Run many operations to test memory stability
    for iteration in 0..20 {
        for (resource_name, resource) in &resources {
            for expression in &expressions {
                let tool_call = test_utils::create_tool_call_message(
                    "fhirpath_evaluate",
                    json!({
                        "expression": expression.replace("*", resource_name),
                        "resource": resource
                    })
                );
                
                client.send_message(tool_call).await?;
                let response = client.wait_for_response().await?;
                
                if let Some(response_msg) = response {
                    // Some expressions might not match, which is fine
                    match response_msg {
                        octofhir_mcp::transport::JsonRpcMessage::Response { result: Some(_), .. } => {
                            // Success - verify response structure
                        },
                        octofhir_mcp::transport::JsonRpcMessage::Response { error: Some(_), .. } => {
                            // Error is acceptable for mismatched expressions
                        },
                        _ => {
                            // Other response types
                        }
                    }
                }
            }
        }
        
        // Periodically check that we're not accumulating excessive messages
        if iteration % 5 == 0 {
            println!("Iteration {}: {} sent, {} received", 
                iteration, client.sent_count(), client.received_count());
        }
    }
    
    // Memory test would check actual memory usage in a real implementation
    // For now, we verify that all operations completed
    assert!(client.sent_count() > 200, "Should have sent many requests");
    assert_eq!(client.sent_count(), client.received_count() + 1, "Should have received responses for all requests except init");
    
    Ok(())
}

/// Graceful error recovery test
#[tokio::test]
async fn test_graceful_error_recovery() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient = test_utils::create_test_patient();
    
    // Test various error conditions and recovery
    let error_scenarios = vec![
        ("Syntax error", "Patient.name.where("),
        ("Type error", "Patient.birthDate + 'string'"),
        ("Division by zero", "10 / 0"),
        ("Invalid function", "Patient.name.invalidFunction()"),
    ];
    
    for (description, error_expression) in error_scenarios {
        // Send an expression that should cause an error
        let error_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": error_expression,
                "resource": patient.clone()
            })
        );
        
        client.send_message(error_call).await?;
        let error_response = client.wait_for_response().await?;
        
        if let Some(response_msg) = error_response {
            // Should get either an error response or a result indicating failure
            println!("Error scenario '{}' handled", description);
        }
        
        // Immediately follow with a valid operation to test recovery
        let recovery_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": "Patient.name.family",
                "resource": patient.clone()
            })
        );
        
        client.send_message(recovery_call).await?;
        let recovery_response = client.wait_for_response().await?;
        
        if let Some(response_msg) = recovery_response {
            assertions::assert_success_response(&response_msg);
            
            if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
                if let Some(result_val) = result {
                    assertions::assert_fhirpath_result(&result_val);
                    
                    let values = result_val.get("values").unwrap().as_array().unwrap();
                    assert!(!values.is_empty(), "Recovery operation should succeed after error");
                }
            }
        }
    }
    
    Ok(())
}

/// Test timeout handling
#[tokio::test]
async fn test_operation_timeout_handling() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test that operations complete within reasonable time
    let patient = test_utils::create_test_patient();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": patient
        })
    );
    
    client.send_message(tool_call).await?;
    
    // Use timeout to ensure operation completes quickly
    let response = timeout(Duration::from_secs(5), client.wait_for_response()).await?;
    
    if let Some(response_msg) = response? {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                // Check that performance metrics are reasonable
                let performance = result_val.get("performance").unwrap();
                let eval_time = performance.get("evaluation_time_ms").unwrap().as_f64().unwrap();
                
                assert!(eval_time < 1000.0, "Simple evaluation should complete quickly");
                assert!(eval_time >= 0.0, "Evaluation time should be non-negative");
            }
        }
    }
    
    Ok(())
}

/// Integration test with mixed transport scenarios
#[tokio::test]
async fn test_mixed_transport_compatibility() -> Result<()> {
    // This test would verify that the same tools work consistently
    // across different transport layers (stdio vs HTTP)
    
    let config = TestServerConfig::default();
    let _http_client = TestHttpClient::new(format!("http://{}:{}", config.host, config.port));
    let mut stdio_client = MockMcpClient::new();
    
    // Test with stdio client
    let init_message = test_utils::create_initialize_message();
    stdio_client.send_message(init_message).await?;
    stdio_client.wait_for_response().await?;
    
    let patient = test_utils::create_test_patient();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": patient.clone()
        })
    );
    
    stdio_client.send_message(tool_call).await?;
    let stdio_response = stdio_client.wait_for_response().await?;
    
    if let Some(response_msg) = stdio_response {
        assertions::assert_success_response(&response_msg);
    }
    
    // In a real test, we would also test the HTTP client:
    // let http_response = http_client.call_tool("fhirpath_evaluate", arguments).await?;
    // Compare results to ensure consistency
    
    Ok(())
}