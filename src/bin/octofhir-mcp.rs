//! OctoFHIR MCP Server - Main binary using rmcp SDK
//!
//! This is the primary binary for the OctoFHIR MCP server, now powered by the
//! official rmcp SDK for better protocol compliance and maintainability.

use anyhow::Result;
use clap::{Parser, Subcommand};
use octofhir_mcp::{server::demonstrate_tools, transport::TransportFactory};
use tracing::{Level, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "octofhir-mcp")]
#[command(about = "OctoFHIR Model Context Protocol Server")]
#[command(version = octofhir_mcp::VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Set the log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server with stdio transport (recommended for MCP clients)
    Stdio,
    /// Start the MCP server with HTTP streamable transport
    Http {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },
    /// Demonstrate FHIRPath tools functionality
    Demo,
    /// Show server information
    Info,
    /// Validate server configuration
    Validate,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = match cli.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let filter = EnvFilter::builder()
        .with_default_directive(log_level.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(filter).init();

    match cli.command {
        Commands::Stdio => {
            info!("Starting OctoFHIR MCP Server with stdio transport");
            info!("Protocol version: 2025-06-18");
            info!("Available tools: fhirpath_evaluate, fhirpath_parse, fhirpath_extract");

            let transport = TransportFactory::create_stdio();
            transport.start().await?;
        }
        Commands::Http { host, port } => {
            info!(
                "Starting OctoFHIR MCP Server with HTTP transport on {}:{}",
                host, port
            );
            info!("Protocol version: 2025-06-18");
            info!("Available tools: fhirpath_evaluate, fhirpath_parse, fhirpath_extract");

            let transport = TransportFactory::create_http(&host, port);
            transport.start().await?;
        }
        Commands::Demo => {
            info!("Demonstrating FHIRPath tools functionality");
            demonstrate_tools().await?;
            info!("Demo completed successfully");
        }
        Commands::Info => {
            println!("OctoFHIR MCP Server");
            println!("Version: {}", octofhir_mcp::VERSION);
            println!("Protocol: MCP 2025-06-18");
            println!("SDK: rmcp v0.6");
            println!();
            println!("Available Tools:");
            println!("  - fhirpath_evaluate: Evaluate FHIRPath expressions against FHIR resources");
            println!("  - fhirpath_parse: Parse and validate FHIRPath expressions");
            println!("  - fhirpath_extract: Extract data from FHIR resources using FHIRPath");
            println!();
            println!("Transports:");
            println!("  - Stdio transport (recommended for MCP clients)");
            println!("  - HTTP Streamable transport (for web applications)");
            println!();
            println!("Features:");
            println!("  - High-performance FHIRPath evaluation");
            println!("  - FHIR R4/R5 schema support");
            println!("  - Comprehensive error diagnostics");
            println!("  - Performance metrics and complexity analysis");
        }
        Commands::Validate => {
            info!("Validating server configuration...");

            // Test FHIRPath engine initialization
            octofhir_mcp::fhirpath_engine::initialize_shared_engine().await?;
            info!("✓ FHIRPath engine initialized successfully");

            // Test tool demonstration
            demonstrate_tools().await?;
            info!("✓ All tools validated successfully");

            info!("✓ Server configuration is valid");
        }
    }

    Ok(())
}
