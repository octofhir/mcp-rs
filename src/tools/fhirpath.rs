//! FHIRPath tool implementations

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::fhirpath_engine::get_shared_engine;
use octofhir_fhirpath::FhirPathValue;
use num_traits::cast::ToPrimitive;

/// Request for FHIRPath evaluation
#[derive(Debug, Deserialize)]
pub struct FhirPathEvaluateRequest {
    pub expression: String,
    pub resource: Value,
    pub context: Option<Value>,
}

/// Response from FHIRPath evaluation
#[derive(Debug, Serialize)]
pub struct FhirPathEvaluateResponse {
    pub result: Vec<Value>,
    pub diagnostics: Option<Vec<String>>,
}

/// Convert FhirPathValue to JSON Value for serialization
pub fn fhirpath_value_to_json(value: &FhirPathValue) -> Value {
    match value {
        FhirPathValue::Boolean(b) => serde_json::json!(b),
        FhirPathValue::Integer(i) => serde_json::json!(i),
        FhirPathValue::Decimal(d) => serde_json::json!(d.to_f64()),
        FhirPathValue::Date(d) => serde_json::json!(d.to_string()),
        FhirPathValue::DateTime(dt) => serde_json::json!(dt.to_string()),
        FhirPathValue::Time(t) => serde_json::json!(t.to_string()),
        FhirPathValue::Quantity(q) => serde_json::json!({
            "value": q.value.to_f64(),
            "unit": q.unit.as_deref()
        }),
        FhirPathValue::Collection(items) => {
            let vec_items = items.clone().into_vec();
            serde_json::json!(vec_items.iter().map(|item| fhirpath_value_to_json(item)).collect::<Vec<_>>())
        }
        _ => serde_json::json!(value.to_string()), // Fallback for String and other types
    }
}

/// Convert a single FhirPathValue into a Vec for consistent processing
pub fn fhirpath_value_to_collection(value: FhirPathValue) -> Vec<FhirPathValue> {
    match value {
        FhirPathValue::Collection(items) => items.into_vec(),
        other => vec![other],
    }
}

/// Evaluate a FHIRPath expression against a FHIR resource using the shared factory
pub async fn evaluate_fhirpath(request: FhirPathEvaluateRequest) -> Result<FhirPathEvaluateResponse> {
    let factory = get_shared_engine().await?;
    
    match factory.evaluate(&request.expression, request.resource).await {
        Ok(fhir_value) => {
            let collection = fhirpath_value_to_collection(fhir_value);
            let result: Vec<Value> = collection.iter()
                .map(|v| fhirpath_value_to_json(v))
                .collect();
            
            Ok(FhirPathEvaluateResponse {
                result,
                diagnostics: None,
            })
        }
        Err(e) => {
            let diagnostics = vec![format!("Evaluation error: {}", e)];
            Ok(FhirPathEvaluateResponse {
                result: vec![],
                diagnostics: Some(diagnostics),
            })
        }
    }
}