#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_fhirpath_evaluate_basic() {
        let tool = FhirPathEvaluateTool::new().unwrap();
        
        let params = json!({
            "expression": "Patient.name.given",
            "resource": {
                "resourceType": "Patient",
                "name": [
                    {
                        "given": ["John", "Q"],
                        "family": "Doe"
                    }
                ]
            }
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());
        
        let tool_result = result.unwrap();
        match tool_result {
            ToolResult::Text(text) => {
                let eval_result: EvaluateResult = serde_json::from_str(&text).unwrap();
                assert!(!eval_result.values.is_empty());
                assert!(eval_result.diagnostics.is_none());
                assert!(eval_result.expression_info.parsed);
            }
            _ => panic!("Expected text result"),
        }
    }

    #[test]
    fn test_complexity_assessment() {
        let tool = FhirPathEvaluateTool::new().unwrap();
        
        assert_eq!(tool.assess_complexity("name"), "simple");
        assert_eq!(tool.assess_complexity("Patient.name.given.first()"), "moderate");
        assert_eq!(tool.assess_complexity("Patient.name.where(use = 'official').given.first() and Patient.birthDate < today()"), "complex");
    }

    #[tokio::test]
    async fn test_empty_expression_error() {
        let tool = FhirPathEvaluateTool::new().unwrap();
        
        let params = json!({
            "expression": "",
            "resource": {"resourceType": "Patient"}
        });

        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }
}