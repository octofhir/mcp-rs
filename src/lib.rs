//! # OctoFHIR MCP Server
//!
//! Model Context Protocol server for the OctoFHIR ecosystem, providing high-performance
//! FHIRPath evaluation and FHIR tooling through standardized MCP interfaces.

pub mod server;
pub mod transport;
pub mod tools;
pub mod resources;
pub mod prompts;
pub mod security;
pub mod cache;
pub mod metrics;
pub mod config;
pub mod fhirpath_engine;

// Re-export commonly used types
pub use server::McpServer;
pub use config::ServerConfig;
pub use fhirpath_engine::{FhirPathEngineFactory, FhirEngineConfig, get_shared_engine, initialize_shared_engine, initialize_shared_engine_with_config};

/// Current version of the MCP server
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
