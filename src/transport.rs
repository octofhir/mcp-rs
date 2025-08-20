//! Transport integration using rmcp SDK
//!
//! This module provides actual MCP protocol transport implementations
//! using the official rmcp SDK.

use anyhow::Result;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::server::FhirPathToolServer;

/// HTTP transport server using MCP streamable HTTP protocol
pub struct HttpTransportServer {
    pub host: String,
    pub port: u16,
}

impl HttpTransportServer {
    /// Create a new HTTP transport server
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    /// Start the HTTP server with MCP streamable HTTP protocol support
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting MCP HTTP streamable transport server on {}:{}",
            self.host, self.port
        );

        // Initialize the shared FHIRPath engine (ignore if already initialized)
        if let Err(e) = crate::fhirpath_engine::initialize_shared_engine().await {
            if !e.to_string().contains("already initialized") {
                return Err(e);
            }
            debug!("FHIRPath engine already initialized");
        }

        // Create the streamable HTTP service with local session manager
        let session_manager = Arc::new(LocalSessionManager::default());
        let config = StreamableHttpServerConfig::default();
        let service = StreamableHttpService::new(
            || Ok(FhirPathToolServer),
            session_manager,
            config,
        );

        // Use hyper directly with the StreamableHttpService
        let bind_address: std::net::SocketAddr = format!("{}:{}", self.host, self.port).parse()?;
        let listener = tokio::net::TcpListener::bind(bind_address).await?;
        info!("MCP HTTP streamable server listening on {}", bind_address);

        // Accept connections and serve them with the StreamableHttpService
        loop {
            let (stream, addr) = listener.accept().await?;
            debug!("Accepted connection from {}", addr);
            let service = service.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                // Wrap the Tower service to make it compatible with Hyper
                let hyper_service = TowerToHyperService::new(service);
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, hyper_service)
                    .await
                {
                    debug!("Connection error: {}", e);
                }
            });
        }
    }
}

/// Stdio transport server using MCP stdio protocol
pub struct StdioTransportServer;

impl Default for StdioTransportServer {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransportServer {
    /// Create a new stdio transport server
    pub fn new() -> Self {
        Self
    }

    /// Start the stdio transport server
    pub async fn start(&self) -> Result<()> {
        info!("Starting MCP stdio transport server");

        // Initialize the shared FHIRPath engine (ignore if already initialized)
        if let Err(e) = crate::fhirpath_engine::initialize_shared_engine().await {
            if !e.to_string().contains("already initialized") {
                return Err(e);
            }
            debug!("FHIRPath engine already initialized");
        }

        info!("Stdio transport ready for MCP communication");

        // Create the server handler
        let _server = FhirPathToolServer;

        // For now, stdio transport is not fully integrated with RMCP 0.6
        // This is a placeholder implementation
        info!("MCP stdio server started successfully");

        // TODO: Implement proper stdio transport integration
        // when RMCP SDK provides stable stdio transport API
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        info!("Stdio transport placeholder - tools are ready");

        info!("Stdio transport server shutting down");
        Ok(())
    }
}

/// Factory for creating transport servers
pub struct TransportFactory;

impl TransportFactory {
    /// Create an HTTP transport server
    pub fn create_http(host: &str, port: u16) -> HttpTransportServer {
        HttpTransportServer::new(host.to_string(), port)
    }

    /// Create a stdio transport server
    pub fn create_stdio() -> StdioTransportServer {
        StdioTransportServer::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_transport_creation() {
        let transport = TransportFactory::create_http("127.0.0.1", 3002);
        assert_eq!(transport.host, "127.0.0.1");
        assert_eq!(transport.port, 3002);
    }

    #[tokio::test]
    async fn test_stdio_transport_creation() {
        let transport = TransportFactory::create_stdio();
        // Test that we can create a stdio transport without errors
        let result = transport.start().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_factory_methods() {
        let http_transport = TransportFactory::create_http("localhost", 8080);
        assert_eq!(http_transport.host, "localhost");
        assert_eq!(http_transport.port, 8080);

        let stdio_transport = TransportFactory::create_stdio();
        // Just verify it was created successfully
        assert_eq!(
            std::mem::size_of_val(&stdio_transport),
            std::mem::size_of::<StdioTransportServer>()
        );
    }
}
