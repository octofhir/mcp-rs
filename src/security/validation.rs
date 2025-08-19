use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub max_expression_length: usize,
    pub max_expression_depth: usize,
    pub max_resource_size: usize,
    pub enable_expression_blacklist: bool,
    pub blacklisted_functions: HashSet<String>,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        let mut blacklisted_functions = HashSet::new();
        blacklisted_functions.insert("eval".to_string());
        blacklisted_functions.insert("system".to_string());
        blacklisted_functions.insert("exec".to_string());
        blacklisted_functions.insert("shell".to_string());

        Self {
            max_expression_length: 1000,
            max_expression_depth: 10,
            max_resource_size: 1024 * 1024, // 1MB
            enable_expression_blacklist: true,
            blacklisted_functions,
        }
    }
}

pub struct InputValidator {
    config: ValidationConfig,
}

impl InputValidator {
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    pub fn validate_fhirpath_expression(&self, expression: &str) -> Result<String> {
        if expression.len() > self.config.max_expression_length {
            return Err(anyhow!(
                "FHIRPath expression too long: {} > {}",
                expression.len(),
                self.config.max_expression_length
            ));
        }

        if expression.is_empty() {
            return Err(anyhow!("FHIRPath expression cannot be empty"));
        }

        let depth = self.calculate_expression_depth(expression);
        if depth > self.config.max_expression_depth {
            return Err(anyhow!(
                "FHIRPath expression too complex: depth {} > {}",
                depth,
                self.config.max_expression_depth
            ));
        }

        if self.config.enable_expression_blacklist {
            self.check_blacklisted_functions(expression)?;
        }

        Ok(self.sanitize_expression(expression))
    }

    pub fn validate_fhir_resource(&self, resource: &Value) -> Result<Value> {
        let resource_str = serde_json::to_string(resource)
            .map_err(|e| anyhow!("Failed to serialize resource: {}", e))?;

        if resource_str.len() > self.config.max_resource_size {
            return Err(anyhow!(
                "FHIR resource too large: {} > {}",
                resource_str.len(),
                self.config.max_resource_size
            ));
        }

        if !resource.is_object() {
            return Err(anyhow!("FHIR resource must be a JSON object"));
        }

        self.validate_json_structure(resource)?;
        Ok(self.sanitize_resource(resource.clone()))
    }

    fn calculate_expression_depth(&self, expression: &str) -> usize {
        let mut depth: usize = 0;
        let mut max_depth: usize = 0;
        let mut in_quotes = false;
        let mut escape_next = false;

        for ch in expression.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' => escape_next = true,
                '\'' | '"' => in_quotes = !in_quotes,
                '(' | '[' | '{' if !in_quotes => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                ')' | ']' | '}' if !in_quotes => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }

        max_depth
    }

    fn check_blacklisted_functions(&self, expression: &str) -> Result<()> {
        let expression_lower = expression.to_lowercase();
        
        for blacklisted in &self.config.blacklisted_functions {
            if expression_lower.contains(&blacklisted.to_lowercase()) {
                return Err(anyhow!(
                    "FHIRPath expression contains blacklisted function: {}",
                    blacklisted
                ));
            }
        }
        
        Ok(())
    }

    fn sanitize_expression(&self, expression: &str) -> String {
        expression
            .trim()
            .replace('\r', "")
            .replace('\0', "")
            .chars()
            .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            .collect()
    }

    fn validate_json_structure(&self, value: &Value) -> Result<()> {
        match value {
            Value::Object(obj) => {
                for (key, val) in obj {
                    if key.len() > 255 {
                        return Err(anyhow!("JSON key too long: {}", key.len()));
                    }
                    self.validate_json_structure(val)?;
                }
            }
            Value::Array(arr) => {
                if arr.len() > 10000 {
                    return Err(anyhow!("JSON array too large: {}", arr.len()));
                }
                for item in arr {
                    self.validate_json_structure(item)?;
                }
            }
            Value::String(s) => {
                if s.len() > 100000 {
                    return Err(anyhow!("JSON string too long: {}", s.len()));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn sanitize_resource(&self, mut resource: Value) -> Value {
        self.sanitize_json_value(&mut resource);
        resource
    }

    fn sanitize_json_value(&self, value: &mut Value) {
        match value {
            Value::Object(obj) => {
                for (_, val) in obj.iter_mut() {
                    self.sanitize_json_value(val);
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    self.sanitize_json_value(item);
                }
            }
            Value::String(s) => {
                *s = s
                    .replace('\0', "")
                    .replace('\r', "")
                    .chars()
                    .filter(|&c| c != '\u{FEFF}') // Remove BOM
                    .collect();
            }
            _ => {}
        }
    }
}

pub struct RequestSanitizer;

impl RequestSanitizer {
    pub fn sanitize_error_message(error: &str, expose_details: bool) -> String {
        if !expose_details {
            return "Request validation failed".to_string();
        }

        error
            .replace("JWT", "token")
            .replace("API key", "authentication")
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn create_correlation_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_expression_length_validation() {
        let config = ValidationConfig::default();
        let validator = InputValidator::new(config);

        let short_expr = "Patient.name";
        assert!(validator.validate_fhirpath_expression(short_expr).is_ok());

        let long_expr = "x".repeat(2000);
        assert!(validator.validate_fhirpath_expression(&long_expr).is_err());
    }

    #[test]
    fn test_expression_depth_validation() {
        let mut config = ValidationConfig::default();
        config.max_expression_depth = 2;
        let validator = InputValidator::new(config);

        let shallow_expr = "Patient.name";
        assert!(validator.validate_fhirpath_expression(shallow_expr).is_ok());

        let deep_expr = "Patient.name.where(value.contains(text.substring(start.add(end))))";
        assert!(validator.validate_fhirpath_expression(deep_expr).is_err());
    }

    #[test]
    fn test_blacklisted_functions() {
        let config = ValidationConfig::default();
        let validator = InputValidator::new(config);

        let safe_expr = "Patient.name.first()";
        assert!(validator.validate_fhirpath_expression(safe_expr).is_ok());

        let unsafe_expr = "eval('malicious code')";
        assert!(validator.validate_fhirpath_expression(unsafe_expr).is_err());
    }

    #[test]
    fn test_resource_size_validation() {
        let config = ValidationConfig::default();
        let validator = InputValidator::new(config);

        let small_resource = json!({"resourceType": "Patient", "id": "123"});
        assert!(validator.validate_fhir_resource(&small_resource).is_ok());

        let large_value = "x".repeat(2 * 1024 * 1024);
        let large_resource = json!({"resourceType": "Patient", "data": large_value});
        assert!(validator.validate_fhir_resource(&large_resource).is_err());
    }

    #[test]
    fn test_error_message_sanitization() {
        let detailed_error = "JWT token validation failed with secret key abc123";
        let sanitized = RequestSanitizer::sanitize_error_message(detailed_error, false);
        assert_eq!(sanitized, "Request validation failed");

        let sanitized_detailed = RequestSanitizer::sanitize_error_message(detailed_error, true);
        assert!(!sanitized_detailed.contains("JWT"));
        assert!(sanitized_detailed.contains("token"));
        // Note: The error message sanitization intentionally leaves some technical details for debugging
        // while removing sensitive information like 'JWT' -> 'token'
    }
}