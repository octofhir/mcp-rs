//! # OctoFHIR MCP Server
//!
//! Model Context Protocol server for the OctoFHIR ecosystem, providing high-performance
//! FHIRPath evaluation and FHIR tooling through standardized MCP interfaces.

pub mod cache;
pub mod config;
pub mod fhirpath_engine;
pub mod metrics;
pub mod prompts;
pub mod resources;
pub mod security;
pub mod server;
pub mod tools;
pub mod transport;

// Re-export main types
pub use config::ServerConfig;
pub use fhirpath_engine::{
    FhirEngineConfig, FhirPathEngineFactory, get_shared_engine, initialize_shared_engine,
    initialize_shared_engine_with_config,
};
pub use server::{FhirPathToolRouter, demonstrate_tools, start_sdk_server};
pub use transport::TransportFactory;

/// Current version of the MCP server
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
