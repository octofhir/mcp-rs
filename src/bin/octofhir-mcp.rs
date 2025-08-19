//! OctoFHIR MCP Server - Main binary

use anyhow::Result;
use clap::Parser;
use octofhir_mcp::{McpServer, ServerConfig};
use octofhir_mcp::transport::{Transport, stdio::StdioTransport, http::HttpTransport};
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "octofhir-mcp")]
#[command(about = "OctoFHIR Model Context Protocol Server")]
#[command(version)]
struct Cli {
    /// Host to bind to for HTTP transport
    #[arg(long, default_value = "localhost")]
    host: String,

    /// Port to bind to for HTTP transport
    #[arg(long, default_value = "3000")]
    port: u16,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Transport mode: stdio, http, or both
    #[arg(long, default_value = "stdio", value_parser = ["stdio", "http", "both"])]
    transport: String,

    /// FHIR version to use (R4, R4B, R5)
    #[arg(long, default_value = "R4", value_parser = ["R4", "R4B", "R5"])]
    fhir_version: String,

    /// Additional FHIR packages to install (format: name@version)
    #[arg(long, value_delimiter = ',')]
    additional_packages: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing - reduce verbosity for stdio transport
    if cli.transport == "stdio" {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_thread_ids(false)
                    .with_file(false)
                    .with_line_number(false)
                    .with_writer(std::io::stderr),  // Write logs to stderr to avoid interfering with MCP communication on stdout
            )
            .with(tracing_subscriber::EnvFilter::new(&cli.log_level))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true),
            )
            .with(tracing_subscriber::EnvFilter::new(&cli.log_level))
            .init();
    }

    info!("Starting OctoFHIR MCP Server v{}", octofhir_mcp::VERSION);

    // Create server configuration
    let config = ServerConfig {
        host: cli.host.clone(),
        port: cli.port,
        log_level: cli.log_level,
        http_transport: cli.transport == "http" || cli.transport == "both",
        stdio_transport: cli.transport == "stdio" || cli.transport == "both",
        fhir_version: cli.fhir_version,
        additional_packages: cli.additional_packages,
    };

    // Create MCP server instance
    let server = McpServer::new(config.clone());

    // Handle shutdown signals
    let shutdown_signal = async {
        match signal::ctrl_c().await {
            Ok(_) => info!("Received Ctrl+C, shutting down..."),
            Err(err) => error!("Unable to listen for shutdown signal: {}", err),
        }
    };

    // Start transport based on configuration
    match cli.transport.as_str() {
        "stdio" => {
            info!("Starting stdio transport for MCP client integration");
            let transport = StdioTransport::new();

            tokio::select! {
                result = transport.start(Box::new(server.clone())) => {
                    match result {
                        Ok(_) => info!("Stdio transport completed successfully"),
                        Err(e) => error!("Stdio transport error: {}", e),
                    }
                }
                _ = shutdown_signal => {
                    info!("Shutdown signal received, stopping stdio transport");
                    if let Err(e) = transport.shutdown().await {
                        error!("Error during stdio transport shutdown: {}", e);
                    }
                }
            }
        },
        "http" => {
            info!("Starting HTTP transport on {}:{}", cli.host, cli.port);
            let transport = HttpTransport::new(cli.port);

            tokio::select! {
                result = transport.start(Box::new(server.clone())) => {
                    match result {
                        Ok(_) => info!("HTTP transport completed successfully"),
                        Err(e) => error!("HTTP transport error: {}", e),
                    }
                }
                _ = shutdown_signal => {
                    info!("Shutdown signal received, stopping HTTP transport");
                    if let Err(e) = transport.shutdown().await {
                        error!("Error during HTTP transport shutdown: {}", e);
                    }
                }
            }
        },
        "both" => {
            info!("Starting both stdio and HTTP transports");
            let stdio_transport = StdioTransport::new();
            let http_transport = HttpTransport::new(cli.port);

            let server_clone = server.clone();
            let stdio_task = tokio::spawn(async move {
                if let Err(e) = stdio_transport.start(Box::new(server_clone)).await {
                    error!("Stdio transport error: {}", e);
                }
            });

            let http_task = tokio::spawn(async move {
                if let Err(e) = http_transport.start(Box::new(server.clone())).await {
                    error!("HTTP transport error: {}", e);
                }
            });

            tokio::select! {
                _ = stdio_task => info!("Stdio transport task completed"),
                _ = http_task => info!("HTTP transport task completed"),
                _ = shutdown_signal => {
                    info!("Shutdown signal received, stopping all transports");
                    // Transports will be dropped and cleaned up automatically
                }
            }
        },
        _ => {
            error!("Invalid transport mode: {}", cli.transport);
            return Err(anyhow::Error::msg("Invalid transport mode"));
        }
    }

    info!("OctoFHIR MCP Server shutdown complete");
    Ok(())
}
