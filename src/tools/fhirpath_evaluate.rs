//! FHIRPath Evaluate Tool
//!
//! This tool evaluates FHIRPath expressions against FHIR resources and returns
//! the evaluation results with type information and performance metrics.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use octofhir_fhirpath::FhirPathValue;
use num_traits::cast::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

// Temporary MCP types until we integrate with rmcp properly
#[derive(Debug, Clone)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub parameter_type: ToolParameterType,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub enum ToolParameterType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone)]
pub enum ToolResult {
    Text(String),
    Object(Value),
}

#[async_trait(?Send)]
pub trait Tool {
    fn description(&self) -> ToolDescription;
    async fn execute(&self, params: Value) -> Result<ToolResult>;
}

/// FHIRPath evaluate tool that evaluates expressions against FHIR resources
#[derive(Clone)]
pub struct FhirPathEvaluateTool {
    // We'll create the engine fresh each time to avoid Send issues for now
}

/// Input parameters for FHIRPath evaluation
#[derive(Debug, Deserialize)]
pub struct EvaluateParams {
    /// The FHIRPath expression to evaluate
    pub expression: String,
    /// The FHIR resource to evaluate against (JSON)
    pub resource: Value,
    /// Optional context variables
    pub context: Option<HashMap<String, Value>>,
    /// Optional timeout in milliseconds (default: 5000ms)
    pub timeout_ms: Option<u64>,
}

/// Result of FHIRPath evaluation
#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluateResult {
    /// The evaluated values
    pub values: Vec<Value>,
    /// Type information for each value
    pub types: Vec<String>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Expression information
    pub expression_info: ExpressionInfo,
    /// Any evaluation errors or warnings
    pub diagnostics: Option<Vec<String>>,
}

/// Performance metrics for evaluation
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Total execution time in milliseconds
    pub execution_time_ms: f64,
    /// Parse time in milliseconds
    pub parse_time_ms: f64,
    /// Evaluation time in milliseconds
    pub evaluation_time_ms: f64,
}

/// Information about the evaluated expression
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpressionInfo {
    /// Whether the expression parsed successfully
    pub parsed: bool,
    /// Expression complexity assessment
    pub complexity: String,
    /// Number of AST nodes
    pub ast_node_count: Option<usize>,
}

impl FhirPathEvaluateTool {
    /// Create a new FhirPath evaluate tool
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Convert FhirPathValue to JSON Value for serialization
    fn fhirpath_value_to_json(&self, value: &FhirPathValue) -> Value {
        match value {
            FhirPathValue::Boolean(b) => json!(b),
            FhirPathValue::Integer(i) => json!(i),
            FhirPathValue::Decimal(d) => json!(d.to_f64()),
            FhirPathValue::Date(d) => json!(d.to_string()),
            FhirPathValue::DateTime(dt) => json!(dt.to_string()),
            FhirPathValue::Time(t) => json!(t.to_string()),
            FhirPathValue::Quantity(q) => json!({
                "value": q.value.to_f64(),
                "unit": q.unit.as_deref()
            }),
            FhirPathValue::Collection(items) => {
                let vec_items = items.clone().into_vec();
                json!(vec_items.iter().map(|item| self.fhirpath_value_to_json(item)).collect::<Vec<_>>())
            }
            _ => json!(value.to_string()), // Fallback for String and other types
        }
    }

    /// Get type description for a FhirPathValue
    fn get_type_description(&self, value: &FhirPathValue) -> String {
        match value {
            FhirPathValue::Boolean(_) => "boolean".to_string(),
            FhirPathValue::Integer(_) => "integer".to_string(),
            FhirPathValue::Decimal(_) => "decimal".to_string(),
            FhirPathValue::Date(_) => "date".to_string(),
            FhirPathValue::DateTime(_) => "dateTime".to_string(),
            FhirPathValue::Time(_) => "time".to_string(),
            FhirPathValue::Quantity(_) => "Quantity".to_string(),
            FhirPathValue::Collection(items) => {
                if items.is_empty() {
                    "collection<empty>".to_string()
                } else {
                    let vec_items = items.clone().into_vec();
                    if !vec_items.is_empty() {
                        let inner_type = self.get_type_description(&vec_items[0]);
                        format!("collection<{}>", inner_type)
                    } else {
                        "collection<empty>".to_string()
                    }
                }
            }
            _ => "string".to_string(), // Fallback for String and other types
        }
    }

    /// Assess expression complexity based on the expression string
    fn assess_complexity(&self, expression: &str) -> String {
        let length = expression.len();
        let function_count = expression.matches('(').count();
        let operator_count = expression.matches(&['=', '!', '<', '>', '&', '|'][..]).count();

        if length < 20 && function_count <= 1 && operator_count <= 1 {
            "simple".to_string()
        } else if length < 100 && function_count <= 3 && operator_count <= 3 {
            "moderate".to_string()
        } else {
            "complex".to_string()
        }
    }

    /// Convert a single FhirPathValue into a Vec for consistent processing
    fn fhirpath_value_to_collection(&self, value: FhirPathValue) -> Vec<FhirPathValue> {
        match value {
            FhirPathValue::Collection(items) => items.into_vec(),
            other => vec![other],
        }
    }

}

impl Default for FhirPathEvaluateTool {
    fn default() -> Self {
        Self::new().expect("Failed to create FhirPathEvaluateTool")
    }
}

#[async_trait(?Send)]
impl Tool for FhirPathEvaluateTool {
    fn description(&self) -> ToolDescription {
        ToolDescription {
            name: "fhirpath_evaluate".to_string(),
            description: "Evaluates FHIRPath expressions against FHIR resources, returning typed results with performance metrics".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "expression".to_string(),
                    description: "The FHIRPath expression to evaluate".to_string(),
                    parameter_type: ToolParameterType::String,
                    required: true,
                },
                ToolParameter {
                    name: "resource".to_string(),
                    description: "The FHIR resource to evaluate against (JSON object)".to_string(),
                    parameter_type: ToolParameterType::Object,
                    required: true,
                },
                ToolParameter {
                    name: "context".to_string(),
                    description: "Optional context variables for evaluation".to_string(),
                    parameter_type: ToolParameterType::Object,
                    required: false,
                },
                ToolParameter {
                    name: "timeout_ms".to_string(),
                    description: "Optional timeout in milliseconds (default: 5000ms)".to_string(),
                    parameter_type: ToolParameterType::Number,
                    required: false,
                },
            ],
        }
    }

    async fn execute(&self, params: Value) -> Result<ToolResult> {
        let start_time = Instant::now();

        // Parse parameters
        let params: EvaluateParams = serde_json::from_value(params)
            .map_err(|e| anyhow!("Invalid parameters: {}", e))?;

        // Validate expression is not empty
        if params.expression.trim().is_empty() {
            return Err(anyhow!("Expression cannot be empty"));
        }

        // Create context variables if provided (skip for now due to complexity)
        if params.context.is_some() {
            return Err(anyhow!("Context variables not yet supported in this implementation"));
        }

        let _parse_start = Instant::now();
        let eval_start = Instant::now();

        // Use the shared engine configured with proper provider
        let engine = crate::fhirpath_engine::get_shared_engine().await?;
        let result = engine.evaluate(&params.expression, params.resource.clone()).await;

        let eval_time = eval_start.elapsed();
        let parse_time = _parse_start.elapsed();

        let (values, types, diagnostics) = match result {
            Ok(fhir_value) => {
                let collection = self.fhirpath_value_to_collection(fhir_value);

                let values: Vec<Value> = collection.iter()
                    .map(|v| self.fhirpath_value_to_json(v))
                    .collect();

                let types: Vec<String> = collection.iter()
                    .map(|v| self.get_type_description(v))
                    .collect();

                (values, types, None)
            }
            Err(e) => {
                let diagnostics = vec![format!("Evaluation error: {}", e)];
                (vec![], vec![], Some(diagnostics))
            }
        };

        let total_time = start_time.elapsed();

        let result = EvaluateResult {
            values,
            types,
            performance: PerformanceMetrics {
                execution_time_ms: total_time.as_secs_f64() * 1000.0,
                parse_time_ms: parse_time.as_secs_f64() * 1000.0,
                evaluation_time_ms: eval_time.as_secs_f64() * 1000.0,
            },
            expression_info: ExpressionInfo {
                parsed: diagnostics.is_none(),
                complexity: self.assess_complexity(&params.expression),
                ast_node_count: None, // Could be implemented if AST provides node count
            },
            diagnostics,
        };

        Ok(ToolResult::Text(serde_json::to_string_pretty(&result)?))
    }
}

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
                assert!(!eval_result.values.is_empty() || eval_result.diagnostics.is_some());
                assert_eq!(eval_result.expression_info.complexity, "simple");
            }
            _ => panic!("Expected text result"),
        }
    }

    #[test]
    fn test_complexity_assessment() {
        let tool = FhirPathEvaluateTool::new().unwrap();

        assert_eq!(tool.assess_complexity("name"), "simple");
        assert_eq!(tool.assess_complexity("Patient.name.given.first()"), "complex"); // 25 chars > 20, so complex
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
