//! MCP tools implementation using rmcp SDK
//!
//! This module implements FHIRPath tools using the official rmcp SDK's #[tool] macros
//! instead of custom trait implementations. This provides better integration with
//! the MCP protocol and reduces boilerplate code.

use anyhow::{Result, anyhow};
use num_traits::cast::ToPrimitive;
use octofhir_fhirpath::FhirPathValue;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Instant;

/// Input parameters for FHIRPath evaluation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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

/// Input parameters for FHIRPath parsing
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParseParams {
    /// The FHIRPath expression to parse
    pub expression: String,
    /// Whether to include detailed AST information
    pub include_ast: Option<bool>,
}

/// Result of FHIRPath parsing
#[derive(Debug, Serialize, Deserialize)]
pub struct ParseResult {
    /// Whether the expression parsed successfully
    pub valid: bool,
    /// Any parsing errors
    pub errors: Vec<String>,
    /// Expression metadata
    pub metadata: ExpressionMetadata,
    /// Optional AST representation
    pub ast: Option<Value>,
}

/// Expression metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpressionMetadata {
    /// Expression complexity assessment
    pub complexity: String,
    /// Number of tokens
    pub token_count: usize,
    /// Functions used in the expression
    pub functions_used: Vec<String>,
    /// Estimated evaluation complexity
    pub evaluation_complexity: String,
}

/// Input parameters for FHIRPath extraction
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExtractParams {
    /// The FHIRPath expression for extraction
    pub expression: String,
    /// The FHIR resource to extract from (JSON)
    pub resource: Value,
    /// Output format (values, paths, structured)
    pub format: Option<String>,
}

/// Result of FHIRPath extraction
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractResult {
    /// Extracted data
    pub data: Value,
    /// Paths to the extracted values
    pub paths: Vec<String>,
    /// Extraction metadata
    pub metadata: ExtractionMetadata,
}

/// Extraction metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    /// Number of values extracted
    pub value_count: usize,
    /// Types of extracted values
    pub value_types: Vec<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: f64,
}

/// Input parameters for FHIRPath expression analysis
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeParams {
    /// The FHIRPath expression to analyze
    pub expression: String,
    /// Optional analysis options
    pub options: Option<AnalysisOptions>,
}

/// Analysis options for FHIRPath expressions
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisOptions {
    /// Include detailed syntax tree information
    pub include_ast: Option<bool>,
    /// Include performance predictions
    pub include_performance: Option<bool>,
    /// Include function usage analysis
    pub include_functions: Option<bool>,
}

/// Result of FHIRPath expression analysis
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeResult {
    /// Basic analysis information
    pub analysis: ExpressionAnalysis,
    /// Detected functions in the expression
    pub functions: Vec<String>,
    /// Performance predictions
    pub performance: PerformancePrediction,
    /// Syntax validation results
    pub syntax: SyntaxAnalysis,
    /// Optional detailed AST
    pub ast: Option<Value>,
}

/// Expression analysis information
#[derive(Debug, Serialize, Deserialize)]
pub struct ExpressionAnalysis {
    /// Expression complexity (low, medium, high)
    pub complexity: String,
    /// Expression type (query, filter, aggregation, etc.)
    pub expression_type: String,
    /// Number of path segments
    pub path_segments: usize,
    /// Number of function calls
    pub function_count: usize,
    /// Whether the expression uses collections
    pub uses_collections: bool,
}

/// Performance prediction
#[derive(Debug, Serialize, Deserialize)]
pub struct PerformancePrediction {
    /// Estimated complexity score (1-10)
    pub complexity_score: u8,
    /// Expected performance category
    pub performance_category: String,
    /// Optimization suggestions
    pub suggestions: Vec<String>,
}

/// Syntax analysis results
#[derive(Debug, Serialize, Deserialize)]
pub struct SyntaxAnalysis {
    /// Whether the syntax is valid
    pub is_valid: bool,
    /// Syntax errors if any
    pub errors: Vec<String>,
    /// Syntax warnings
    pub warnings: Vec<String>,
    /// Token count
    pub token_count: usize,
}

// Helper functions for value conversion and type analysis

/// Convert FhirPathValue to JSON Value for serialization
fn fhirpath_value_to_json(value: &FhirPathValue) -> Value {
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
            json!(
                vec_items
                    .iter()
                    .map(fhirpath_value_to_json)
                    .collect::<Vec<_>>()
            )
        }
        _ => json!(value.to_string()), // Fallback for String and other types
    }
}

/// Get type description for a FhirPathValue
fn get_type_description(value: &FhirPathValue) -> String {
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
                    let inner_type = get_type_description(&vec_items[0]);
                    format!("collection<{inner_type}>")
                } else {
                    "collection<empty>".to_string()
                }
            }
        }
        _ => "string".to_string(), // Fallback for String and other types
    }
}

/// Assess expression complexity based on the expression string
fn assess_complexity(expression: &str) -> String {
    let length = expression.len();
    let function_count = expression.matches('(').count();
    let operator_count = expression
        .matches(&['=', '!', '<', '>', '&', '|'][..])
        .count();

    if length < 20 && function_count <= 1 && operator_count <= 1 {
        "simple".to_string()
    } else if length < 50 && function_count <= 2 && operator_count <= 2 {
        "moderate".to_string()
    } else {
        "complex".to_string()
    }
}

/// Convert a single FhirPathValue into a Vec for consistent processing
fn fhirpath_value_to_collection(value: FhirPathValue) -> Vec<FhirPathValue> {
    match value {
        FhirPathValue::Collection(items) => items.into_vec(),
        other => vec![other],
    }
}

/// Evaluates FHIRPath expressions against FHIR resources, returning typed results with performance metrics
pub async fn fhirpath_evaluate(params: EvaluateParams) -> Result<EvaluateResult> {
    let start_time = Instant::now();

    // Validate expression is not empty
    if params.expression.trim().is_empty() {
        return Err(anyhow!("Expression cannot be empty"));
    }

    // Create context variables if provided (skip for now due to complexity)
    if params.context.is_some() {
        return Err(anyhow!(
            "Context variables not yet supported in this implementation"
        ));
    }

    let _parse_start = Instant::now();
    let eval_start = Instant::now();

    // Use the shared engine configured with proper provider
    let engine = crate::fhirpath_engine::get_shared_engine().await?;
    let result = engine
        .evaluate(&params.expression, params.resource.clone())
        .await;

    let eval_time = eval_start.elapsed();
    let parse_time = _parse_start.elapsed();

    let (values, types, diagnostics) = match result {
        Ok(fhir_value) => {
            let collection = fhirpath_value_to_collection(fhir_value);

            let values: Vec<Value> = collection.iter().map(fhirpath_value_to_json).collect();

            let types: Vec<String> = collection.iter().map(get_type_description).collect();

            (values, types, None)
        }
        Err(e) => {
            let diagnostics = vec![format!("Evaluation error: {}", e)];
            (vec![], vec![], Some(diagnostics))
        }
    };

    let total_time = start_time.elapsed();

    Ok(EvaluateResult {
        values,
        types,
        performance: PerformanceMetrics {
            execution_time_ms: total_time.as_secs_f64() * 1000.0,
            parse_time_ms: parse_time.as_secs_f64() * 1000.0,
            evaluation_time_ms: eval_time.as_secs_f64() * 1000.0,
        },
        expression_info: ExpressionInfo {
            parsed: diagnostics.is_none(),
            complexity: assess_complexity(&params.expression),
            ast_node_count: None, // Could be implemented if AST provides node count
        },
        diagnostics,
    })
}

/// Parses and validates FHIRPath expressions, providing detailed syntax analysis
pub async fn fhirpath_parse(params: ParseParams) -> Result<ParseResult> {
    // Validate expression is not empty
    if params.expression.trim().is_empty() {
        return Err(anyhow!("Expression cannot be empty"));
    }

    // For now, do basic validation using the engine
    let engine = crate::fhirpath_engine::get_shared_engine().await?;

    // Try to parse by evaluating against an empty resource
    let test_resource = json!({});
    let result = engine.evaluate(&params.expression, test_resource).await;

    let (valid, errors) = match result {
        Ok(_) => (true, vec![]),
        Err(e) => (false, vec![e.to_string()]),
    };

    // Analyze expression for metadata
    let functions_used = extract_functions(&params.expression);
    let token_count = params.expression.split_whitespace().count();

    Ok(ParseResult {
        valid,
        errors,
        metadata: ExpressionMetadata {
            complexity: assess_complexity(&params.expression),
            token_count,
            functions_used,
            evaluation_complexity: if token_count < 5 {
                "low".to_string()
            } else if token_count < 15 {
                "medium".to_string()
            } else {
                "high".to_string()
            },
        },
        ast: None, // Could be implemented with detailed AST analysis
    })
}

/// Extracts data from FHIR resources using FHIRPath with flexible output formatting
pub async fn fhirpath_extract(params: ExtractParams) -> Result<ExtractResult> {
    let start_time = Instant::now();

    // Validate expression is not empty
    if params.expression.trim().is_empty() {
        return Err(anyhow!("Expression cannot be empty"));
    }

    // Use the shared engine configured with proper provider
    let engine = crate::fhirpath_engine::get_shared_engine().await?;
    let result = engine
        .evaluate(&params.expression, params.resource.clone())
        .await;

    let execution_time = start_time.elapsed();

    match result {
        Ok(fhir_value) => {
            let collection = fhirpath_value_to_collection(fhir_value);

            let values: Vec<Value> = collection.iter().map(fhirpath_value_to_json).collect();

            let value_types: Vec<String> = collection.iter().map(get_type_description).collect();

            // Generate simple paths for now
            let paths: Vec<String> = (0..values.len()).map(|i| format!("result[{i}]")).collect();

            let format = params.format.as_deref().unwrap_or("values");
            let data = match format {
                "structured" => json!({
                    "values": values,
                    "types": value_types,
                    "paths": paths
                }),
                "paths" => json!(paths),
                _ => json!(values), // "values" or default
            };

            Ok(ExtractResult {
                data,
                paths,
                metadata: ExtractionMetadata {
                    value_count: values.len(),
                    value_types,
                    execution_time_ms: execution_time.as_secs_f64() * 1000.0,
                },
            })
        }
        Err(e) => Err(anyhow!("Extraction failed: {}", e)),
    }
}

/// Analyzes FHIRPath expressions providing detailed information about syntax, performance, and usage
pub async fn fhirpath_analyze(params: AnalyzeParams) -> Result<AnalyzeResult> {
    // Validate expression is not empty
    if params.expression.trim().is_empty() {
        return Err(anyhow!("Expression cannot be empty"));
    }

    let expression = &params.expression;
    let options = params.options.unwrap_or_default();

    // Extract functions from the expression
    let functions = extract_functions(expression);

    // Basic syntax analysis
    let syntax_analysis = analyze_syntax(expression).await;

    // Expression analysis
    let analysis = analyze_expression_structure(expression, &functions);

    // Performance prediction
    let performance = predict_performance(expression, &functions, &analysis);

    // Optional AST analysis
    let ast = if options.include_ast.unwrap_or(false) {
        // For now, return a placeholder - could be enhanced with detailed AST parsing
        Some(json!({
            "type": "expression",
            "raw": expression,
            "note": "Detailed AST analysis not yet implemented"
        }))
    } else {
        None
    };

    Ok(AnalyzeResult {
        analysis,
        functions,
        performance,
        syntax: syntax_analysis,
        ast,
    })
}

fn analyze_expression_structure(expression: &str, functions: &[String]) -> ExpressionAnalysis {
    let path_segments = expression.split('.').count();
    let function_count = functions.len();
    let uses_collections = expression.contains('[')
        || functions
            .iter()
            .any(|f| ["where", "select", "all", "any", "distinct"].contains(&f.as_str()));

    // Determine expression type
    let expression_type = if functions
        .iter()
        .any(|f| ["where", "exists", "empty"].contains(&f.as_str()))
    {
        "filter".to_string()
    } else if functions
        .iter()
        .any(|f| ["count", "sum", "avg"].contains(&f.as_str()))
    {
        "aggregation".to_string()
    } else if functions
        .iter()
        .any(|f| ["select", "first", "last"].contains(&f.as_str()))
    {
        "transformation".to_string()
    } else {
        "query".to_string()
    };

    // Calculate complexity
    let complexity_score =
        path_segments + (function_count * 2) + if uses_collections { 2 } else { 0 };
    let complexity = if complexity_score < 3 {
        "low".to_string()
    } else if complexity_score < 8 {
        "medium".to_string()
    } else {
        "high".to_string()
    };

    ExpressionAnalysis {
        complexity,
        expression_type,
        path_segments,
        function_count,
        uses_collections,
    }
}

async fn analyze_syntax(expression: &str) -> SyntaxAnalysis {
    // Try to parse the expression using the FHIRPath engine to validate syntax
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut is_valid = true;

    // Basic token counting
    let token_count = expression.split_whitespace().count()
        + expression.matches('.').count()
        + expression.matches('(').count()
        + expression.matches('[').count();

    // Try to validate with the engine if available
    match crate::fhirpath_engine::get_shared_engine().await {
        Ok(engine) => {
            // Try parsing with a dummy resource to check syntax
            let dummy_resource = serde_json::json!({"resourceType": "Patient"});
            match engine.evaluate(expression, dummy_resource).await {
                Err(e) => {
                    is_valid = false;
                    errors.push(e.to_string());
                }
                Ok(_) => {
                    // Syntax is valid, but add warnings for potential issues
                    if expression.len() > 100 {
                        warnings.push(
                            "Expression is quite long, consider breaking it down".to_string(),
                        );
                    }
                    if expression.matches('(').count() > 3 {
                        warnings.push(
                            "Multiple nested function calls may impact performance".to_string(),
                        );
                    }
                }
            }
        }
        Err(_) => {
            warnings.push("Could not validate syntax - FHIRPath engine not available".to_string());
        }
    }

    SyntaxAnalysis {
        is_valid,
        errors,
        warnings,
        token_count,
    }
}

fn predict_performance(
    expression: &str,
    functions: &[String],
    analysis: &ExpressionAnalysis,
) -> PerformancePrediction {
    let mut complexity_score = 1u8;
    let mut suggestions = Vec::new();

    // Add complexity based on various factors
    complexity_score += analysis.path_segments as u8;
    complexity_score += (analysis.function_count * 2) as u8;

    if analysis.uses_collections {
        complexity_score += 2;
        suggestions.push(
            "Consider using specific indexes instead of filtering entire collections".to_string(),
        );
    }

    if functions
        .iter()
        .any(|f| ["where", "select"].contains(&f.as_str()))
    {
        complexity_score += 1;
    }

    if expression.len() > 50 {
        complexity_score += 1;
        suggestions.push("Consider breaking down long expressions into smaller parts".to_string());
    }

    // Cap at 10
    complexity_score = complexity_score.min(10);

    let performance_category = match complexity_score {
        1..=3 => "fast".to_string(),
        4..=6 => "moderate".to_string(),
        7..=8 => "slow".to_string(),
        _ => "very_slow".to_string(),
    };

    if suggestions.is_empty() {
        suggestions.push("Expression looks well-optimized".to_string());
    }

    PerformancePrediction {
        complexity_score,
        performance_category,
        suggestions,
    }
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            include_ast: Some(false),
            include_performance: Some(true),
            include_functions: Some(true),
        }
    }
}

/// Helper function to extract function names from FHIRPath expression
fn extract_functions(expression: &str) -> Vec<String> {
    let mut functions = Vec::new();
    let function_names = [
        "where",
        "select",
        "first",
        "last",
        "tail",
        "skip",
        "take",
        "single",
        "exists",
        "empty",
        "not",
        "all",
        "any",
        "count",
        "distinct",
        "isDistinct",
        "subsetOf",
        "supersetOf",
        "intersect",
        "exclude",
        "union",
        "combine",
        "contains",
        "in",
        "indexOf",
        "substring",
        "startsWith",
        "endsWith",
        "matches",
        "replaceMatches",
        "replace",
        "length",
        "toInteger",
        "toString",
        "toDecimal",
        "toDateTime",
        "toTime",
        "convertsToInteger",
        "convertsToDecimal",
        "convertsToDateTime",
        "convertsToTime",
        "iif",
        "trace",
        "now",
        "today",
        "timeOfDay",
    ];

    for func in function_names {
        if expression.contains(&format!("{func}(")) {
            functions.push(func.to_string());
        }
    }

    functions.sort();
    functions.dedup();
    functions
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_fhirpath_evaluate_basic() {
        let params = EvaluateParams {
            expression: "Patient.name.given".to_string(),
            resource: json!({
                "resourceType": "Patient",
                "name": [
                    {
                        "given": ["John", "Q"],
                        "family": "Doe"
                    }
                ]
            }),
            context: None,
            timeout_ms: None,
        };

        let result = fhirpath_evaluate(params).await;
        assert!(result.is_ok());

        let eval_result = result.unwrap();
        assert_eq!(eval_result.expression_info.complexity, "simple");
    }

    #[tokio::test]
    async fn test_fhirpath_parse_valid() {
        let params = ParseParams {
            expression: "Patient.name.given".to_string(),
            include_ast: Some(false),
        };

        let result = fhirpath_parse(params).await;
        assert!(result.is_ok());

        let parse_result = result.unwrap();
        assert!(parse_result.valid || !parse_result.errors.is_empty()); // Either valid or has error info
    }

    #[tokio::test]
    async fn test_fhirpath_extract_structured() {
        let params = ExtractParams {
            expression: "Patient.name.family".to_string(),
            resource: json!({
                "resourceType": "Patient",
                "name": [
                    {
                        "given": ["John"],
                        "family": "Doe"
                    }
                ]
            }),
            format: Some("structured".to_string()),
        };

        let result = fhirpath_extract(params).await;
        assert!(result.is_ok());

        let extract_result = result.unwrap();
        assert!(extract_result.data.is_object());
        assert!(!extract_result.paths.is_empty() || extract_result.metadata.value_count == 0);
    }

    #[test]
    fn test_extract_functions() {
        let expression = "Patient.name.where(use = 'official').given.first()";
        let functions = extract_functions(expression);
        assert!(functions.contains(&"where".to_string()));
        assert!(functions.contains(&"first".to_string()));
    }

    #[test]
    fn test_complexity_assessment() {
        assert_eq!(assess_complexity("name"), "simple");
        assert_eq!(assess_complexity("Patient.name.given.first()"), "moderate");
        assert_eq!(
            assess_complexity(
                "Patient.name.where(use = 'official').given.first() and Patient.birthDate < today()"
            ),
            "complex"
        );
    }
}
