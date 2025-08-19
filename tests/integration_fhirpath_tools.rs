//! Integration tests for FHIRPath tools

use anyhow::Result;
use serde_json::{json, Value};

mod common;

use common::{test_utils, assertions, MockMcpClient};

/// Test fhirpath_evaluate tool with Patient resource
#[tokio::test]
async fn test_fhirpath_evaluate_patient_basic() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test basic Patient name extraction
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
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Should return family name values");
                assert_eq!(values[0].as_str().unwrap(), "Doe", "Should return correct family name");
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_evaluate with complex expressions
#[tokio::test]
async fn test_fhirpath_evaluate_complex_expressions() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Test complex expression with where clause
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.where(use='official').given.join(' ')",
            "resource": patient_resource
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Should return joined given names");
                assert_eq!(values[0].as_str().unwrap(), "John Michael", "Should return correct joined names");
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_evaluate with Observation resource
#[tokio::test]
async fn test_fhirpath_evaluate_observation() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let observation_resource = test_utils::create_test_observation();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Observation.valueQuantity.value",
            "resource": observation_resource
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Should return observation value");
                assert_eq!(values[0].as_f64().unwrap(), 36.5, "Should return correct temperature value");
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_evaluate with Bundle resource
#[tokio::test]
async fn test_fhirpath_evaluate_bundle() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let bundle_resource = test_utils::create_test_bundle();
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Bundle.entry.count()",
            "resource": bundle_resource
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                assert!(!values.is_empty(), "Should return bundle entry count");
                assert_eq!(values[0].as_i64().unwrap(), 2, "Should return correct entry count");
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_parse tool
#[tokio::test]
async fn test_fhirpath_parse() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_parse",
        json!({
            "expression": "Patient.name.where(use='official').family"
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                // Check parse result structure
                assert!(result_val.get("ast").is_some(), "Parse result should include AST");
                assert!(result_val.get("valid").is_some(), "Parse result should include validity flag");
                
                let valid = result_val.get("valid").unwrap().as_bool().unwrap();
                assert!(valid, "Expression should be valid");
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_parse with invalid expressions
#[tokio::test]
async fn test_fhirpath_parse_invalid_expression() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_parse",
        json!({
            "expression": "Patient.name.where(use='official'" // Missing closing parenthesis
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // This could be either an error response or a success response with valid=false
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, error, .. } = response_msg {
            if let Some(result_val) = result {
                // If we get a result, it should indicate the expression is invalid
                let valid = result_val.get("valid").unwrap().as_bool().unwrap();
                assert!(!valid, "Invalid expression should not be valid");
                assert!(result_val.get("errors").is_some(), "Invalid expression should include errors");
            } else if let Some(_error_val) = error {
                // If we get an error, that's also acceptable for invalid syntax
                // The error should contain information about the syntax issue
            }
        }
    }
    
    Ok(())
}

/// Test fhirpath_evaluate error handling
#[tokio::test]
async fn test_fhirpath_evaluate_error_handling() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    // Test with invalid FHIR resource
    let invalid_resource = json!({
        "resourceType": "InvalidResource",
        "id": "test",
        "invalidField": "should not exist"
    });
    
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.name.family",
            "resource": invalid_resource
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Should either error or return empty results gracefully
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, error, .. } = response_msg {
            if let Some(result_val) = result {
                // If we get a result, it should handle the type mismatch gracefully
                let values = result_val.get("values").unwrap().as_array().unwrap();
                // Should return empty values for Patient expression on non-Patient resource
                assert!(values.is_empty(), "Should return empty values for type mismatch");
            } else if let Some(_error_val) = error {
                // Error response is also acceptable for invalid resource
            }
        }
    }
    
    Ok(())
}

/// Test performance characteristics of FHIRPath evaluation
#[tokio::test]
async fn test_fhirpath_evaluate_performance() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Test simple expression
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
                assertions::assert_fhirpath_result(&result_val);
                
                // Check performance metrics
                let performance = result_val.get("performance").unwrap();
                assert!(performance.get("evaluation_time_ms").is_some(), "Should include evaluation time");
                
                let eval_time = performance.get("evaluation_time_ms").unwrap().as_f64().unwrap();
                assert!(eval_time >= 0.0, "Evaluation time should be non-negative");
                assert!(eval_time < 1000.0, "Simple expression should evaluate quickly");
            }
        }
    }
    
    Ok(())
}

/// Test batch evaluation with multiple expressions
#[tokio::test]
async fn test_fhirpath_batch_evaluation() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Test multiple expressions on the same resource
    let expressions = vec![
        "Patient.name.family",
        "Patient.name.given.first()",
        "Patient.gender",
        "Patient.birthDate",
        "Patient.telecom.where(system='phone').value",
    ];
    
    for expression in expressions {
        let tool_call = test_utils::create_tool_call_message(
            "fhirpath_evaluate",
            json!({
                "expression": expression,
                "resource": patient_resource.clone()
            })
        );
        
        client.send_message(tool_call).await?;
        let response = client.wait_for_response().await?;
        
        if let Some(response_msg) = response {
            assertions::assert_success_response(&response_msg);
            
            if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
                if let Some(result_val) = result {
                    assertions::assert_fhirpath_result(&result_val);
                    
                    let values = result_val.get("values").unwrap().as_array().unwrap();
                    // Each expression should return at least one value for our test patient
                    assert!(!values.is_empty(), "Expression '{}' should return values", expression);
                }
            }
        }
    }
    
    Ok(())
}

/// Test edge cases and boundary conditions
#[tokio::test]
async fn test_fhirpath_edge_cases() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Test empty expression
    let tool_call = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "",
            "resource": patient_resource.clone()
        })
    );
    
    client.send_message(tool_call).await?;
    let response = client.wait_for_response().await?;
    
    if let Some(response_msg) = response {
        // Empty expression should either error or return empty results
        assertions::assert_error_response(&response_msg);
    }
    
    // Test expression accessing non-existent properties
    let tool_call2 = test_utils::create_tool_call_message(
        "fhirpath_evaluate",
        json!({
            "expression": "Patient.nonExistentProperty",
            "resource": patient_resource
        })
    );
    
    client.send_message(tool_call2).await?;
    let response2 = client.wait_for_response().await?;
    
    if let Some(response_msg) = response2 {
        assertions::assert_success_response(&response_msg);
        
        if let octofhir_mcp::transport::JsonRpcMessage::Response { result, .. } = response_msg {
            if let Some(result_val) = result {
                assertions::assert_fhirpath_result(&result_val);
                
                let values = result_val.get("values").unwrap().as_array().unwrap();
                // Non-existent properties should return empty results, not errors
                assert!(values.is_empty(), "Non-existent property should return empty values");
            }
        }
    }
    
    Ok(())
}

/// Test type checking and validation
#[tokio::test]
async fn test_fhirpath_type_validation() -> Result<()> {
    let mut client = MockMcpClient::new();
    
    // Initialize
    let init_message = test_utils::create_initialize_message();
    client.send_message(init_message).await?;
    client.wait_for_response().await?;
    
    let patient_resource = test_utils::create_test_patient();
    
    // Test expression that should return specific types
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
                assertions::assert_fhirpath_result(&result_val);
                
                let types = result_val.get("types").unwrap().as_array().unwrap();
                let values = result_val.get("values").unwrap().as_array().unwrap();
                
                // Types and values should have same length
                assert_eq!(types.len(), values.len(), "Types and values should have same length");
                
                // Family name should be of string type
                if !types.is_empty() {
                    assert_eq!(types[0].as_str().unwrap(), "string", "Family name should be string type");
                }
            }
        }
    }
    
    Ok(())
}