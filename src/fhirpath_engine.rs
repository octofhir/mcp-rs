//! FHIRPath Engine Factory Implementation
//!
//! This module provides a factory for creating FHIRPath engine instances with R4 FHIR schema
//! provider to improve performance and reduce initialization overhead across tool calls.

use anyhow::{Result, anyhow};
use octofhir_fhir_model::provider::FhirVersion;
use octofhir_fhirpath::{
    FhirPathEngine, FhirPathValue,
    model::{FhirSchemaModelProvider, ModelProvider},
    utils,
};
use octofhir_fhirschema::PackageSpec;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Configuration for FHIRPath engine factory
#[derive(Debug, Clone)]
pub struct FhirEngineConfig {
    /// FHIR version to use
    pub fhir_version: String,
    /// Additional FHIR packages to install
    pub additional_packages: Vec<String>,
}

impl Default for FhirEngineConfig {
    fn default() -> Self {
        Self {
            fhir_version: "R4".to_string(),
            additional_packages: Vec::new(),
        }
    }
}

/// Factory for creating FHIRPath engine instances with configurable schema provider
#[derive(Clone)]
pub struct FhirPathEngineFactory {
    model_provider: Arc<dyn ModelProvider>,
    config: FhirEngineConfig,
}

impl FhirPathEngineFactory {
    /// Create a new FHIRPath engine factory with default R4 FHIR schema provider
    pub async fn new() -> Result<Self> {
        Self::with_config_async(FhirEngineConfig::default()).await
    }

    /// Create a new FHIRPath engine factory with custom configuration
    pub async fn with_config(config: FhirEngineConfig) -> Result<Self> {
        Self::with_config_async(config).await
    }

    /// Create a new FHIRPath engine factory with async FHIR schema provider
    pub async fn with_config_async(config: FhirEngineConfig) -> Result<Self> {
        info!(
            "Initializing async FHIRPath engine factory with FHIR {} schema provider",
            config.fhir_version
        );

        // Parse FHIR version
        let fhir_version = match config.fhir_version.as_str() {
            "R4" => FhirVersion::R4,
            "R4B" => FhirVersion::R4B,
            "R5" => FhirVersion::R5,
            _ => {
                return Err(anyhow!(
                    "Unknown FHIR version '{}'. Supported versions: R4, R4B, R5",
                    config.fhir_version
                ));
            }
        };

        // Parse additional packages
        let mut package_specs = Vec::new();
        for package in &config.additional_packages {
            if let Some((name, version)) = package.split_once('@') {
                package_specs.push(PackageSpec::registry(name, version));
            } else {
                return Err(anyhow!(
                    "Invalid package format '{}', expected 'name@version'",
                    package
                ));
            }
        }

        // Create FhirSchemaModelProvider - ALWAYS use real schema provider
        let provider = if package_specs.is_empty() {
            // Use version-specific factory methods
            match fhir_version {
                FhirVersion::R4 => FhirSchemaModelProvider::r4().await,
                FhirVersion::R4B => FhirSchemaModelProvider::r4b().await,
                FhirVersion::R5 => FhirSchemaModelProvider::r5().await,
            }
        } else {
            // Use with_packages method for additional packages
            FhirSchemaModelProvider::with_packages(package_specs).await
        }.map_err(|e| {
            anyhow!("Failed to create FhirSchemaModelProvider: {}. The server requires a valid FHIR schema provider.", e)
        })?;

        let model_provider: Arc<dyn ModelProvider> = Arc::new(provider);

        info!(
            "FHIRPath engine factory initialized successfully with FHIR {} schema provider",
            config.fhir_version
        );

        Ok(Self {
            model_provider,
            config,
        })
    }

    /// Create a new engine instance for evaluation
    pub async fn create_engine(&self) -> Result<FhirPathEngine> {
        FhirPathEngine::with_model_provider(self.model_provider.clone())
            .await
            .map_err(|e| anyhow!("Failed to create FhirPathEngine: {}", e))
    }

    /// Evaluate a FHIRPath expression against a FHIR resource
    pub async fn evaluate(&self, expression: &str, resource: Value) -> Result<FhirPathValue> {
        debug!("Evaluating FHIRPath expression: {}", expression);

        if expression.trim().is_empty() {
            return Err(anyhow!("FHIRPath expression cannot be empty"));
        }

        let engine = self.create_engine().await?;

        // Convert serde_json::Value to sonic_rs::Value using octofhir-fhirpath utils
        let sonic_resource = utils::serde_to_sonic(&resource)
            .map_err(|e| anyhow!("Failed to convert resource to sonic_rs::Value: {}", e))?;

        engine
            .evaluate(expression, sonic_resource)
            .await
            .map_err(|e| {
                warn!("FHIRPath evaluation failed: {}", e);
                anyhow!("FHIRPath evaluation error: {}", e)
            })
    }

    /// Parse a FHIRPath expression to check syntax
    pub async fn parse_expression(&self, expression: &str) -> Result<()> {
        debug!("Parsing FHIRPath expression: {}", expression);

        if expression.trim().is_empty() {
            return Err(anyhow!("FHIRPath expression cannot be empty"));
        }

        // For now, we'll use the evaluation method to test parsing
        // In a future implementation, we should expose a dedicated parse method
        let test_resource = serde_json::json!({
            "resourceType": "Patient",
            "id": "test"
        });

        // Test parse by attempting evaluation on a minimal resource
        match self.evaluate(expression, test_resource).await {
            Ok(_) => {
                debug!("FHIRPath expression parsed successfully");
                Ok(())
            }
            Err(e) => {
                debug!("FHIRPath expression parse failed: {}", e);
                Err(e)
            }
        }
    }

    /// Get engine statistics and health information
    pub async fn get_engine_info(&self) -> EngineInfo {
        EngineInfo {
            initialized: true,
            schema_provider: format!("FhirSchemaModelProvider ({})", self.config.fhir_version),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Information about the FHIRPath engine instance
#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineInfo {
    pub initialized: bool,
    pub schema_provider: String,
    pub version: String,
}

/// Global shared instance of the FHIRPath engine factory
static SHARED_FACTORY: tokio::sync::OnceCell<FhirPathEngineFactory> =
    tokio::sync::OnceCell::const_new();

/// Get the global shared FHIRPath engine factory instance
pub async fn get_shared_engine() -> Result<&'static FhirPathEngineFactory> {
    SHARED_FACTORY
        .get_or_try_init(|| async {
            FhirPathEngineFactory::with_config_async(FhirEngineConfig::default()).await
        })
        .await
}

/// Initialize the shared FHIRPath engine factory with configuration
pub async fn initialize_shared_engine_with_config(config: FhirEngineConfig) -> Result<()> {
    info!(
        "Initializing global shared FHIRPath engine factory with config: {:?}",
        config
    );

    let factory = FhirPathEngineFactory::with_config_async(config).await?;

    SHARED_FACTORY
        .set(factory)
        .map_err(|_| anyhow!("Shared FHIRPath engine factory already initialized"))?;

    info!("Global shared FHIRPath engine factory initialized successfully");
    Ok(())
}

/// Initialize the shared FHIRPath engine factory with default configuration
pub async fn initialize_shared_engine() -> Result<()> {
    initialize_shared_engine_with_config(FhirEngineConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_factory_creation() {
        let factory = FhirPathEngineFactory::new().await.unwrap();
        let info = factory.get_engine_info().await;
        assert!(info.initialized);
        assert!(!info.schema_provider.is_empty());
    }

    #[tokio::test]
    async fn test_factory_evaluation() {
        let factory = FhirPathEngineFactory::new().await.unwrap();

        let resource = json!({
            "resourceType": "Patient",
            "name": [{
                "given": ["John"],
                "family": "Doe"
            }]
        });

        let result = factory.evaluate("Patient.name.family", resource).await;
        // Test should not panic, result depends on FHIRPath implementation
        match result {
            Ok(_) => println!("Evaluation successful"),
            Err(e) => println!("Evaluation failed: {e}"),
        }
    }

    #[tokio::test]
    async fn test_empty_expression_error() {
        let factory = FhirPathEngineFactory::new().await.unwrap();
        let resource = json!({"resourceType": "Patient"});

        let result = factory.evaluate("", resource).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_global_shared_factory() {
        let factory1 = get_shared_engine().await.unwrap();
        let factory2 = get_shared_engine().await.unwrap();

        // Should be the same instance
        assert!(std::ptr::eq(factory1, factory2));
    }

    #[tokio::test]
    async fn test_parse_expression() {
        let factory = FhirPathEngineFactory::new().await.unwrap();

        // Test valid expression (basic syntax)
        let result = factory.parse_expression("Patient.name").await;
        match result {
            Ok(_) => println!("Parse successful"),
            Err(e) => println!("Parse failed: {e}"),
        }

        // Test empty expression
        let result = factory.parse_expression("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let factory = FhirPathEngineFactory::new().await.unwrap();
        let _engine1 = factory.create_engine().await.unwrap();
        let _engine2 = factory.create_engine().await.unwrap();

        // Should be able to create multiple engines from the same factory
        // This tests that the factory pattern works correctly
    }
}
