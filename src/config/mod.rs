//! Configuration management

use serde::{Deserialize, Serialize};

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server host (default: localhost)
    pub host: String,
    /// Server port (default: 3000)
    pub port: u16,
    /// Log level (default: info)
    pub log_level: String,
    /// Enable HTTP transport
    pub http_transport: bool,
    /// Enable stdio transport
    pub stdio_transport: bool,
    /// FHIR version to use (default: R4)
    pub fhir_version: String,
    /// Additional FHIR packages to install
    pub additional_packages: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 3000,
            log_level: "info".to_string(),
            http_transport: true,
            stdio_transport: true,
            fhir_version: "R4".to_string(),
            additional_packages: Vec::new(),
        }
    }
}
