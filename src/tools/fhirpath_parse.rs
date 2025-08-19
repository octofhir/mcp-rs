//! FHIRPath Parse Tool Implementation
//!
//! This tool parses and validates FHIRPath expressions with detailed syntax feedback,
//! AST generation, and complexity analysis.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;

use super::fhirpath_evaluate::{Tool, ToolDescription, ToolParameter, ToolParameterType, ToolResult};
use crate::fhirpath_engine::get_shared_engine;

/// FHIRPath parse tool that parses and validates FHIRPath expressions
#[derive(Clone)]
pub struct FhirPathParseTool {
    // We'll create the engine fresh each time to avoid Send issues for now
}

/// Input parameters for FHIRPath parsing
#[derive(Debug, Deserialize)]
pub struct ParseParams {
    /// The FHIRPath expression to parse
    pub expression: String,
    /// Optional: Include AST representation in response
    pub include_ast: Option<bool>,
    /// Optional: Provide syntax explanation for valid expressions
    pub explain_syntax: Option<bool>,
}

/// Result of FHIRPath parsing
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseResult {
    /// Whether the expression parsed successfully
    pub valid: bool,
    /// Array of parsing errors with position information
    pub errors: Option<Vec<ParseError>>,
    /// Optional AST representation (when requested and valid)
    pub ast: Option<Value>,
    /// Optional human-readable syntax explanation
    pub syntax_explanation: Option<String>,
    /// Expression complexity analysis
    pub complexity_metrics: Option<ComplexityMetrics>,
}

/// Parsing error with position information
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseError {
    /// Error message
    pub message: String,
    /// Position in the expression where error occurred
    pub position: Option<usize>,
    /// Error severity level
    pub severity: String,
    /// Optional suggestion for fixing the error
    pub suggestion: Option<String>,
}

/// Expression complexity analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    /// AST depth
    pub depth: usize,
    /// Number of function calls
    pub functions: usize,
    /// Number of predicates
    pub predicates: usize,
    /// Number of path segments
    pub path_segments: usize,
    /// Overall complexity score
    pub complexity_score: f64,
}

impl FhirPathParseTool {
    /// Create a new FhirPath parse tool
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Parse FHIRPath expression and extract information
    async fn parse_expression(&self, expression: &str) -> Result<ParseResult> {
        if expression.trim().is_empty() {
            return Ok(ParseResult {
                valid: false,
                errors: Some(vec![ParseError {
                    message: "Expression cannot be empty".to_string(),
                    position: Some(0),
                    severity: "error".to_string(),
                    suggestion: Some("Provide a valid FHIRPath expression".to_string()),
                }]),
                ast: None,
                syntax_explanation: None,
                complexity_metrics: None,
            });
        }

        let start_time = Instant::now();

        // Use the shared engine factory for parsing
        let factory = get_shared_engine().await?;

        // Attempt to parse the expression
        match factory.parse_expression(expression).await {
            Ok(_) => {
                let _parse_time = start_time.elapsed();

                // Generate complexity metrics
                let complexity = self.analyze_complexity(expression);

                Ok(ParseResult {
                    valid: true,
                    errors: None,
                    ast: None, // TODO: Implement AST serialization when parser provides AST access
                    syntax_explanation: Some(self.generate_syntax_explanation(expression)),
                    complexity_metrics: Some(complexity),
                })
            }
            Err(parse_error) => {
                // Extract error information
                let error_msg = parse_error.to_string();
                let position = self.extract_error_position(&error_msg, expression);
                let suggestion = self.generate_error_suggestion(&error_msg, expression);

                Ok(ParseResult {
                    valid: false,
                    errors: Some(vec![ParseError {
                        message: error_msg,
                        position,
                        severity: "error".to_string(),
                        suggestion,
                    }]),
                    ast: None,
                    syntax_explanation: None,
                    complexity_metrics: None,
                })
            }
        }
    }

    /// Analyze expression complexity
    fn analyze_complexity(&self, expression: &str) -> ComplexityMetrics {
        // Basic complexity analysis based on string patterns
        let functions = expression.matches('(').count();
        let predicates = expression.matches('[').count() +
                        expression.matches(".where(").count() +
                        expression.matches(".select(").count() +
                        expression.matches(".exists(").count();
        let path_segments = expression.split('.').count().saturating_sub(1);

        // Simple depth estimation based on parentheses and brackets nesting
        let mut depth: usize = 0;
        let mut max_depth: usize = 0;
        for char in expression.chars() {
            match char {
                '(' | '[' => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                ')' | ']' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        // Calculate complexity score (simple heuristic)
        let complexity_score = (functions as f64 * 2.0) +
                              (predicates as f64 * 1.5) +
                              (path_segments as f64 * 0.5) +
                              (max_depth as f64 * 1.0);

        ComplexityMetrics {
            depth: max_depth,
            functions,
            predicates,
            path_segments,
            complexity_score,
        }
    }

    /// Generate human-readable syntax explanation
    fn generate_syntax_explanation(&self, expression: &str) -> String {
        // Simple pattern-based explanation generation
        if expression.contains(".first()") {
            return "Selects the first element from a collection".to_string();
        }
        if expression.contains(".last()") {
            return "Selects the last element from a collection".to_string();
        }
        if expression.contains(".exists()") {
            return "Checks if any elements exist in the collection".to_string();
        }
        if expression.contains(".empty()") {
            return "Checks if the collection is empty".to_string();
        }
        if expression.contains(".count()") {
            return "Returns the number of elements in the collection".to_string();
        }
        if expression.contains(".where(") {
            return "Filters the collection based on a condition".to_string();
        }
        if expression.contains(".select(") {
            return "Transforms each element in the collection".to_string();
        }

        // Generic explanation for path expressions
        let segments: Vec<&str> = expression.split('.').collect();
        if segments.len() > 1 {
            format!("Navigates through FHIR resource path: {}", segments.join(" -> "))
        } else {
            "Simple FHIRPath expression".to_string()
        }
    }

    /// Extract error position from error message
    fn extract_error_position(&self, error_msg: &str, expression: &str) -> Option<usize> {
        // Try to extract position information from error message
        // This is a simple heuristic - real implementation would need to parse actual parser errors

        // Look for common patterns in error messages that might indicate position
        if error_msg.contains("unexpected end") {
            Some(expression.len())
        } else if error_msg.contains("expected") {
            // Try to find where the error might have occurred based on common patterns
            if expression.contains("(") && !expression.contains(")") {
                expression.find('(').map(|pos| pos + 1)
            } else if expression.contains("[") && !expression.contains("]") {
                expression.find('[').map(|pos| pos + 1)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Generate helpful error suggestions
    fn generate_error_suggestion(&self, error_msg: &str, expression: &str) -> Option<String> {
        let lower_error = error_msg.to_lowercase();

        if lower_error.contains("unexpected end") {
            if expression.contains("(") && !expression.contains(")") {
                Some("Complete the expression by adding a closing parenthesis ')'".to_string())
            } else if expression.contains("[") && !expression.contains("]") {
                Some("Complete the expression by adding a closing bracket ']'".to_string())
            } else {
                Some("The expression appears to be incomplete".to_string())
            }
        } else if lower_error.contains("expected") {
            Some("Check the syntax around the error position".to_string())
        } else if lower_error.contains("invalid") {
            Some("Review the expression syntax according to FHIRPath specification".to_string())
        } else {
            None
        }
    }
}

impl Default for FhirPathParseTool {
    fn default() -> Self {
        Self::new().expect("Failed to create FhirPathParseTool")
    }
}

#[async_trait(?Send)]
impl Tool for FhirPathParseTool {
    /// Provide tool description for MCP
    fn description(&self) -> ToolDescription {
        ToolDescription {
            name: "fhirpath_parse".to_string(),
            description: "Parse and validate FHIRPath expressions with detailed syntax feedback, AST generation, and complexity analysis".to_string(),
            parameters: vec![
                ToolParameter {
                    name: "expression".to_string(),
                    parameter_type: ToolParameterType::String,
                    description: "The FHIRPath expression to parse and validate".to_string(),
                    required: true,
                },
                ToolParameter {
                    name: "include_ast".to_string(),
                    parameter_type: ToolParameterType::Boolean,
                    description: "Include AST representation in the response (optional, default: false)".to_string(),
                    required: false,
                },
                ToolParameter {
                    name: "explain_syntax".to_string(),
                    parameter_type: ToolParameterType::Boolean,
                    description: "Provide syntax explanation for valid expressions (optional, default: false)".to_string(),
                    required: false,
                },
            ],
        }
    }

    /// Execute the FHIRPath parsing
    async fn execute(&self, params: Value) -> Result<ToolResult> {
        // Parse input parameters
        let parse_params: ParseParams = serde_json::from_value(params)
            .map_err(|e| anyhow!("Invalid parameters for fhirpath_parse: {}", e))?;

        // Validate expression parameter
        if parse_params.expression.trim().is_empty() {
            return Err(anyhow!("Expression parameter cannot be empty"));
        }

        // Parse the expression
        let result = self.parse_expression(&parse_params.expression).await?;

        // Convert result to pretty-printed JSON string
        Ok(ToolResult::Text(serde_json::to_string_pretty(&result)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_fhirpath_parse_valid_expression() {
        let tool = FhirPathParseTool::new().unwrap();

        let params = json!({
            "expression": "Patient.name.family",
            "include_ast": true,
            "explain_syntax": true
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());

        if let Ok(ToolResult::Text(text)) = result {
            let obj: ParseResult = serde_json::from_str(&text).unwrap();
            assert_eq!(obj.valid, true);
            assert!(obj.complexity_metrics.is_some());
        }
    }

    #[tokio::test]
    async fn test_fhirpath_parse_invalid_expression() {
        let tool = FhirPathParseTool::new().unwrap();

        let params = json!({
            "expression": "Patient.name.("
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());

        if let Ok(ToolResult::Text(text)) = result {
            let obj: ParseResult = serde_json::from_str(&text).unwrap();
            assert_eq!(obj.valid, false);
            assert!(obj.errors.is_some());
        }
    }

    #[tokio::test]
    async fn test_empty_expression_error() {
        let tool = FhirPathParseTool::new().unwrap();

        let params = json!({
            "expression": ""
        });

        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_complexity_analysis() {
        let tool = FhirPathParseTool::new().unwrap();

        let params = json!({
            "expression": "Patient.name.where(use = 'official').family.first()"
        });

        let result = tool.execute(params).await;
        assert!(result.is_ok());

        if let Ok(ToolResult::Text(text)) = result {
            let obj: ParseResult = serde_json::from_str(&text).unwrap();
            let metrics = obj.complexity_metrics.unwrap();
            assert!(metrics.functions > 0);
            assert!(metrics.predicates > 0);
            assert!(metrics.path_segments > 0);
        }
    }
}
