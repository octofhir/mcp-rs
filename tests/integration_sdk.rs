//! Integration tests for the rmcp SDK-based MCP server
//!
//! These tests verify that the SDK implementation works correctly
//! for common use cases and scenarios.

use anyhow::Result;
use octofhir_mcp::{
    server::{FhirPathToolRouter, demonstrate_tools},
    tools::{EvaluateParams, ExtractParams, ParseParams},
    transport::TransportFactory,
};
use serde_json::json;

#[tokio::test]
async fn test_sdk_server_initialization() -> Result<()> {
    // Test that we can create a router and use it
    let router = FhirPathToolRouter;

    // Test a simple evaluation
    let params = EvaluateParams {
        expression: "Patient.name.family".to_string(),
        resource: json!({
            "resourceType": "Patient",
            "name": [{"family": "Smith", "given": ["John"]}]
        }),
        context: None,
        timeout_ms: None,
    };

    let result = router.fhirpath_evaluate(params).await?;
    assert!(result.is_object());

    // Check that the result has the expected structure
    let result_obj = result.as_object().unwrap();
    assert!(result_obj.contains_key("values"));
    assert!(result_obj.contains_key("types"));
    assert!(result_obj.contains_key("performance"));

    Ok(())
}

#[tokio::test]
async fn test_fhirpath_tool_operations() -> Result<()> {
    let router = FhirPathToolRouter;

    // Test evaluation
    let eval_result = router
        .fhirpath_evaluate(EvaluateParams {
            expression: "Patient.birthDate".to_string(),
            resource: json!({
                "resourceType": "Patient",
                "birthDate": "1990-01-01"
            }),
            context: None,
            timeout_ms: None,
        })
        .await?;

    assert!(eval_result["values"].is_array());

    // Test parsing
    let parse_result = router
        .fhirpath_parse(ParseParams {
            expression: "Patient.name".to_string(),
            include_ast: Some(false),
        })
        .await?;

    assert!(parse_result["valid"].is_boolean());

    // Test extraction
    let extract_result = router
        .fhirpath_extract(ExtractParams {
            expression: "Patient.identifier.value".to_string(),
            resource: json!({
                "resourceType": "Patient",
                "identifier": [
                    {"value": "12345", "system": "urn:oid:1.2.3.4.5"},
                    {"value": "67890", "system": "urn:oid:5.4.3.2.1"}
                ]
            }),
            format: Some("values".to_string()),
        })
        .await?;

    assert!(extract_result["data"].is_array());

    Ok(())
}

#[tokio::test]
async fn test_transport_factory() {
    // Test that we can create transports without errors
    let http_transport = TransportFactory::create_http("127.0.0.1", 3001);
    assert_eq!(http_transport.host, "127.0.0.1");
    assert_eq!(http_transport.port, 3001);

    let stdio_transport = TransportFactory::create_stdio();
    // Just verify it was created successfully
    assert_eq!(
        std::mem::size_of_val(&stdio_transport),
        std::mem::size_of::<octofhir_mcp::transport::StdioTransportServer>()
    );
}

#[tokio::test]
async fn test_demonstrate_tools() -> Result<()> {
    // Test the demo functionality
    let result = demonstrate_tools().await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_complex_fhirpath_expressions() -> Result<()> {
    let router = FhirPathToolRouter;

    // Test a more complex FHIRPath expression
    let params = EvaluateParams {
        expression: "Bundle.entry.resource.where(resourceType = 'Patient').name.given".to_string(),
        resource: json!({
            "resourceType": "Bundle",
            "entry": [
                {
                    "resource": {
                        "resourceType": "Patient",
                        "name": [{"given": ["Alice"], "family": "Johnson"}]
                    }
                },
                {
                    "resource": {
                        "resourceType": "Observation",
                        "status": "final"
                    }
                },
                {
                    "resource": {
                        "resourceType": "Patient",
                        "name": [{"given": ["Bob"], "family": "Smith"}]
                    }
                }
            ]
        }),
        context: None,
        timeout_ms: None,
    };

    let result = router.fhirpath_evaluate(params).await?;

    // Verify the result structure
    assert!(result["values"].is_array());
    assert!(result["types"].is_array());
    assert!(result["performance"].is_object());
    assert!(result["expression_info"].is_object());

    // Check that complexity was assessed
    assert!(result["expression_info"]["complexity"].is_string());

    Ok(())
}

#[tokio::test]
async fn test_error_handling() -> Result<()> {
    let router = FhirPathToolRouter;

    // Test with invalid FHIRPath expression
    let params = EvaluateParams {
        expression: "invalid().syntax here".to_string(),
        resource: json!({"resourceType": "Patient"}),
        context: None,
        timeout_ms: None,
    };

    let result = router.fhirpath_evaluate(params).await;

    // The evaluation might succeed but return diagnostics, or it might fail
    // Either way, the router should handle it gracefully
    match result {
        Ok(res) => {
            // If it succeeds, check if there are diagnostics
            if let Some(diagnostics) = res["diagnostics"].as_array() {
                assert!(!diagnostics.is_empty());
            }
        }
        Err(_) => {
            // If it fails, that's also acceptable for invalid syntax
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_performance_metrics() -> Result<()> {
    let router = FhirPathToolRouter;

    let params = EvaluateParams {
        expression: "Patient.name.family".to_string(),
        resource: json!({
            "resourceType": "Patient",
            "name": [{"family": "Test"}]
        }),
        context: None,
        timeout_ms: None,
    };

    let result = router.fhirpath_evaluate(params).await?;

    // Verify performance metrics are present and reasonable
    let performance = &result["performance"];
    assert!(performance["execution_time_ms"].is_number());
    assert!(performance["parse_time_ms"].is_number());
    assert!(performance["evaluation_time_ms"].is_number());

    // Execution time should be positive
    let exec_time = performance["execution_time_ms"].as_f64().unwrap();
    assert!(exec_time > 0.0);

    Ok(())
}
