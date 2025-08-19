//! HTTP/SSE transport implementation

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    middleware,
    response::{IntoResponse, Sse, Response},
    routing::{get, post},
    Json, Router,
};
use axum::response::sse::{Event, KeepAlive};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{broadcast, RwLock},
};
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{MessageHandler, McpMessage, ToolsCallParams};
use super::sse_auth::{SseAuthenticator, SseConnection};
use crate::security::{SecurityProvider, RequestSanitizer};
use crate::metrics::{MetricsProvider, HealthStatus};

/// HTTP transport for web applications and remote access
#[derive(Clone)]
pub struct HttpTransport {
    port: u16,
    host: String,
    cors_origins: Vec<String>,
    auth_enabled: bool,
    auth_tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    sse_clients: Arc<RwLock<HashMap<String, broadcast::Sender<SseMessage>>>>,
    security_provider: Option<Arc<SecurityProvider>>,
    metrics_provider: Option<Arc<MetricsProvider>>,
}

/// Authentication token structure
#[derive(Debug, Clone)]
pub struct AuthToken {
    pub token: String,
    pub user_id: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// SSE message types
#[derive(Debug, Clone, Serialize)]
pub struct SseMessage {
    pub id: String,
    pub event: String,
    pub data: Value,
}

/// HTTP request for MCP tool calls
#[derive(Debug, Deserialize)]
pub struct McpToolRequest {
    pub arguments: Option<Value>,
}

/// HTTP response for MCP operations
#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Application state for HTTP handlers
#[derive(Clone)]
pub struct AppState {
    pub handler: Arc<dyn MessageHandler + Send + Sync>,
    pub transport: HttpTransport,
}

impl HttpTransport {
    /// Create a new HTTP transport instance
    pub fn new(port: u16) -> Self {
        Self {
            port,
            host: "0.0.0.0".to_string(),
            cors_origins: vec!["*".to_string()],
            auth_enabled: false,
            auth_tokens: Arc::new(RwLock::new(HashMap::new())),
            sse_clients: Arc::new(RwLock::new(HashMap::new())),
            security_provider: None,
            metrics_provider: None,
        }
    }

    /// Create a new HTTP transport with custom configuration
    pub fn with_config(
        port: u16,
        host: String,
        cors_origins: Vec<String>,
        auth_enabled: bool,
    ) -> Self {
        Self {
            port,
            host,
            cors_origins,
            auth_enabled,
            auth_tokens: Arc::new(RwLock::new(HashMap::new())),
            sse_clients: Arc::new(RwLock::new(HashMap::new())),
            security_provider: None,
            metrics_provider: None,
        }
    }

    /// Create a new HTTP transport with security provider
    pub fn with_security(
        port: u16,
        host: String,
        cors_origins: Vec<String>,
        security_provider: Arc<SecurityProvider>,
    ) -> Self {
        let auth_enabled = security_provider.authenticator().is_auth_enabled();
        Self {
            port,
            host,
            cors_origins,
            auth_enabled,
            auth_tokens: Arc::new(RwLock::new(HashMap::new())),
            sse_clients: Arc::new(RwLock::new(HashMap::new())),
            security_provider: Some(security_provider),
            metrics_provider: None,
        }
    }

    /// Create a new HTTP transport with both security and metrics providers
    pub fn with_full_monitoring(
        port: u16,
        host: String,
        cors_origins: Vec<String>,
        security_provider: Arc<SecurityProvider>,
        metrics_provider: Arc<MetricsProvider>,
    ) -> Self {
        let auth_enabled = security_provider.authenticator().is_auth_enabled();
        Self {
            port,
            host,
            cors_origins,
            auth_enabled,
            auth_tokens: Arc::new(RwLock::new(HashMap::new())),
            sse_clients: Arc::new(RwLock::new(HashMap::new())),
            security_provider: Some(security_provider),
            metrics_provider: Some(metrics_provider),
        }
    }

    /// Create the Axum router with all routes and middleware
    fn create_router(&self, handler: Arc<dyn MessageHandler + Send + Sync>) -> Router {
        let app_state = AppState {
            handler,
            transport: self.clone(),
        };

        let mut cors = CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any);

        // Configure CORS origins
        if self.cors_origins.contains(&"*".to_string()) {
            cors = cors.allow_origin(Any);
        } else {
            for origin in &self.cors_origins {
                match origin.parse::<hyper::http::HeaderValue>() {
                    Ok(header_value) => {
                        cors = cors.allow_origin(header_value);
                    }
                    Err(_) => {
                        warn!("Invalid CORS origin: {}", origin);
                        if let Ok(fallback) = "http://localhost:3000".parse::<hyper::http::HeaderValue>() {
                            cors = cors.allow_origin(fallback);
                        }
                    }
                }
            }
        }

        let mut router = Router::new()
            // MCP tool endpoints
            .route("/mcp/tools/{tool_name}", post(handle_tool_call))
            .route("/mcp/tools/list", get(handle_tools_list))
            // SSE endpoint for streaming (GET for stream, POST for messages)
            .route("/sse", get(handle_sse_connection).post(handle_sse_message))
            // Health and monitoring endpoints
            .route("/health", get(handle_health_check))
            .route("/ready", get(handle_readiness_check))
            .route("/metrics", get(handle_metrics))
            .route("/stats", get(handle_stats))
            .route("/sse/auth-info", get(handle_sse_auth_info))
            // Add CORS middleware
            .layer(cors)
            .with_state(app_state);

        // Add metrics collection middleware if enabled
        if let Some(metrics_provider) = &self.metrics_provider {
            let metrics_provider = metrics_provider.clone();
            router = router.layer(middleware::from_fn_with_state(metrics_provider, metrics_middleware));
        }

        // Add authentication middleware if enabled
        if self.auth_enabled {
            if let Some(security_provider) = &self.security_provider {
                let security_provider = security_provider.clone();
                router = router.layer(middleware::from_fn_with_state(security_provider, security_auth_middleware));
            } else {
                router = router.layer(middleware::from_fn(auth_middleware));
            }
        }

        router
    }

    /// Add authentication token
    pub async fn add_auth_token(&self, user_id: String, expires_in: Duration) -> String {
        let token = Uuid::new_v4().to_string();
        let auth_token = AuthToken {
            token: token.clone(),
            user_id,
            expires_at: chrono::Utc::now() + chrono::Duration::from_std(expires_in).unwrap_or_default(),
        };

        self.auth_tokens.write().await.insert(token.clone(), auth_token);
        token
    }

    /// Validate authentication token
    pub async fn validate_token(&self, token: &str) -> Option<String> {
        let tokens = self.auth_tokens.read().await;
        tokens.get(token).and_then(|auth_token| {
            if auth_token.expires_at > chrono::Utc::now() {
                Some(auth_token.user_id.clone())
            } else {
                None
            }
        })
    }

    /// Send SSE message to client
    pub async fn send_sse_message(&self, client_id: &str, message: SseMessage) -> Result<()> {
        let clients = self.sse_clients.read().await;
        if let Some(sender) = clients.get(client_id) {
            sender.send(message).map_err(|e| {
                anyhow::Error::msg(format!("Failed to send SSE message: {}", e))
            })?;
        }
        Ok(())
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new(3000)
    }
}

#[async_trait]
impl super::Transport for HttpTransport {
    /// Start the HTTP transport server
    async fn start(&self, handler: Box<dyn MessageHandler + Send + Sync>) -> Result<()> {
        let handler = Arc::from(handler);
        info!("Starting HTTP transport on {}:{}", self.host, self.port);

        let app = self.create_router(handler);
        let addr = format!("{}:{}", self.host, self.port);
        
        let listener = TcpListener::bind(&addr).await
            .with_context(|| format!("Failed to bind HTTP server to {}", addr))?;

        info!("HTTP server listening on http://{}", addr);
        
        axum::serve(listener, app).await
            .context("HTTP server error")?;

        Ok(())
    }

    /// Shutdown the HTTP transport
    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down HTTP transport");
        // Clean up SSE clients
        self.sse_clients.write().await.clear();
        // Clean up auth tokens
        self.auth_tokens.write().await.clear();
        debug!("HTTP transport shutdown completed");
        Ok(())
    }
    
    /// Send a message through the HTTP transport (used for SSE)
    async fn send_message(&self, message: McpMessage) -> Result<()> {
        debug!("Broadcasting message via HTTP transport SSE");
        
        let sse_message = SseMessage {
            id: Uuid::new_v4().to_string(),
            event: match message {
                McpMessage::Response { .. } => "response".to_string(),
                McpMessage::Notification { .. } => "notification".to_string(),
                _ => "message".to_string(),
            },
            data: serde_json::to_value(&message.to_jsonrpc())?,
        };

        // Broadcast to all connected SSE clients
        let clients = self.sse_clients.read().await;
        for (client_id, sender) in clients.iter() {
            if let Err(e) = sender.send(sse_message.clone()) {
                warn!("Failed to send SSE message to client {}: {}", client_id, e);
            }
        }

        Ok(())
    }
}

/// Handle MCP tool calls via HTTP POST
async fn handle_tool_call(
    State(state): State<AppState>,
    Path(tool_name): Path<String>,
    Json(request): Json<McpToolRequest>,
) -> impl IntoResponse {
    debug!("HTTP tool call: {}", tool_name);

    // Validate input if security provider is available
    if let Some(security_provider) = &state.transport.security_provider {
        // Validate tool arguments if they contain FHIRPath expressions or resources
        if let Some(ref arguments) = request.arguments {
            // Check for FHIRPath expression in arguments
            if let Some(expression) = arguments.get("expression").and_then(|v| v.as_str()) {
                if let Err(e) = security_provider.validator().validate_fhirpath_expression(expression) {
                    warn!("FHIRPath expression validation failed: {}", e);
                    let error_msg = RequestSanitizer::sanitize_error_message(&e.to_string(), false);
                    let response = McpResponse {
                        success: false,
                        result: None,
                        error: Some(error_msg),
                    };
                    return (StatusCode::BAD_REQUEST, Json(response));
                }
            }

            // Check for FHIR resource in arguments
            if let Some(resource) = arguments.get("resource") {
                if let Err(e) = security_provider.validator().validate_fhir_resource(resource) {
                    warn!("FHIR resource validation failed: {}", e);
                    let error_msg = RequestSanitizer::sanitize_error_message(&e.to_string(), false);
                    let response = McpResponse {
                        success: false,
                        result: None,
                        error: Some(error_msg),
                    };
                    return (StatusCode::BAD_REQUEST, Json(response));
                }
            }
        }
    }

    let mcp_message = McpMessage::ToolsCall {
        id: chrono::Utc::now().timestamp() as u64,
        params: ToolsCallParams {
            name: tool_name.clone(),
            arguments: request.arguments,
        },
    };

    match state.handler.handle_message(mcp_message).await {
        Ok(Some(McpMessage::Response { result, error, .. })) => {
            let response = McpResponse {
                success: error.is_none(),
                result,
                error: error.map(|e| e.message),
            };
            (StatusCode::OK, Json(response))
        }
        Ok(_) => {
            let response = McpResponse {
                success: false,
                result: None,
                error: Some("No response generated".to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
        Err(e) => {
            error!("Tool call error: {}", e);
            let response = McpResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

/// Handle MCP tools list request
async fn handle_tools_list(State(state): State<AppState>) -> impl IntoResponse {
    debug!("HTTP tools list request");

    let mcp_message = McpMessage::ToolsList {
        id: chrono::Utc::now().timestamp() as u64,
    };

    match state.handler.handle_message(mcp_message).await {
        Ok(Some(McpMessage::Response { result, error, .. })) => {
            let response = McpResponse {
                success: error.is_none(),
                result,
                error: error.map(|e| e.message),
            };
            (StatusCode::OK, Json(response))
        }
        Ok(_) => {
            let response = McpResponse {
                success: false,
                result: None,
                error: Some("No response generated".to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
        Err(e) => {
            error!("Tools list error: {}", e);
            let response = McpResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

/// Handle SSE connections for streaming responses
async fn handle_sse_connection(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> impl IntoResponse {
    // Extract SSE connection info from request extensions (set by auth middleware)
    let sse_connection = request.extensions().get::<SseConnection>().cloned();
    
    let sse_connection = match sse_connection {
        Some(conn) => conn,
        None => {
            error!("SSE connection info not found in request extensions");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let client_id = sse_connection.client_id.clone();
    info!("Establishing SSE connection for authenticated client: {}", client_id);

    let (sender, receiver) = broadcast::channel(100);
    
    // Store the SSE client
    state.transport.sse_clients.write().await.insert(client_id.clone(), sender.clone());

    // Create SSE authenticator for refresh events
    let sse_authenticator = if let Some(security_provider) = &state.transport.security_provider {
        Some(SseAuthenticator::new(security_provider.clone()))
    } else {
        None
    };

    // Create enhanced SSE stream with authentication monitoring
    let stream = async_stream::stream! {
        let mut rx = receiver;
        let mut connection = sse_connection;
        
        // Send initial connection event with authentication info
        if let Some(ref authenticator) = sse_authenticator {
            let connection_event = authenticator.create_connection_event(&connection);
            yield Ok::<Event, axum::Error>(connection_event);
        } else {
            // Fallback connection event
            yield Ok::<Event, axum::Error>(Event::default()
                .event("connected")
                .data(json!({
                    "client_id": connection.client_id,
                    "status": "connected",
                    "authenticated": true
                }).to_string()));
        }

        // Start connection monitoring loop
        let mut last_refresh_check = std::time::Instant::now();
        let refresh_check_interval = Duration::from_secs(60); // Check every minute

        loop {
            // Check for token refresh requirements periodically
            if last_refresh_check.elapsed() >= refresh_check_interval {
                if let Some(ref authenticator) = sse_authenticator {
                    match authenticator.validate_connection(&mut connection).await {
                        Ok(()) => {
                            // Check if refresh notification needed
                            if let Some(refresh_event) = authenticator.create_refresh_event(&connection) {
                                yield Ok(refresh_event);
                            }
                        }
                        Err(error_event) => {
                            // Connection expired or authentication failed
                            yield Ok(error_event);
                            break;
                        }
                    }
                }
                last_refresh_check = std::time::Instant::now();
            }

            // Handle incoming messages with timeout
            let timeout_duration = Duration::from_secs(30);
            match tokio::time::timeout(timeout_duration, rx.recv()).await {
                Ok(msg_result) => {
                    match msg_result {
                        Ok(msg) => {
                            connection.update_activity(); // Update activity on message
                            yield Ok::<Event, axum::Error>(Event::default()
                                .id(msg.id)
                                .event(&msg.event)
                                .data(msg.data.to_string()));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("SSE client {} disconnected", connection.client_id);
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            warn!("SSE client {} lagged behind", connection.client_id);
                            continue;
                        }
                    }
                }
                Err(_) => {
                    // Timeout occurred, continue loop for keep-alive and refresh checks
                    continue;
                }
            }
        }
        
        // Send disconnection event and clean up
        yield Ok(Event::default()
            .event("disconnected")
            .data(json!({
                "client_id": connection.client_id,
                "reason": "connection_ended"
            }).to_string()));
        
        state.transport.sse_clients.write().await.remove(&connection.client_id);
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Handle MCP messages sent to SSE endpoint
async fn handle_sse_message(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Json(json_rpc_message): Json<Value>,
) -> impl IntoResponse {
    let client_id = params.get("client_id")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    debug!("SSE message from client {}: {:?}", client_id, json_rpc_message);

    // Parse JSON-RPC message and convert to MCP message
    match crate::transport::JsonRpcMessage::from_json_value(json_rpc_message) {
        Ok(json_rpc) => {
            match json_rpc.to_mcp_message() {
                Ok(mcp_message) => {
                    // Handle the MCP message
                    match state.handler.handle_message(mcp_message).await {
                        Ok(Some(response)) => {
                            // Convert response back to JSON-RPC and send via SSE
                            let json_rpc_response = crate::transport::JsonRpcMessage::from_mcp_message(response);
                            let response_json = json_rpc_response.to_json_value();
                            
                            // Send response via SSE
                            let sse_message = SseMessage {
                                id: chrono::Utc::now().timestamp().to_string(),
                                event: "response".to_string(),
                                data: response_json,
                            };
                            
                            if let Err(e) = state.transport.send_sse_message(&client_id, sse_message).await {
                                warn!("Failed to send SSE response: {}", e);
                            }
                            
                            StatusCode::OK
                        }
                        Ok(None) => {
                            debug!("No response generated for MCP message");
                            StatusCode::NO_CONTENT
                        }
                        Err(e) => {
                            error!("Failed to handle MCP message: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to convert JSON-RPC to MCP message: {}", e);
                    StatusCode::BAD_REQUEST
                }
            }
        }
        Err(e) => {
            warn!("Failed to parse JSON-RPC message: {}", e);
            StatusCode::BAD_REQUEST
        }
    }
}

/// Health check endpoint with comprehensive monitoring
async fn handle_health_check(State(state): State<AppState>) -> Response {
    if let Some(metrics_provider) = &state.transport.metrics_provider {
        let health_response = metrics_provider.get_health_status().await;
        let status_code = match health_response.status {
            HealthStatus::Healthy => StatusCode::OK,
            HealthStatus::Degraded => StatusCode::OK, // Still serving requests
            HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        };
        (status_code, Json(health_response)).into_response()
    } else {
        // Fallback to basic health check  
        let response = serde_json::json!({
            "status": "healthy",
            "version": crate::VERSION,
            "timestamp": chrono::Utc::now()
        });
        (StatusCode::OK, Json(response)).into_response()
    }
}

/// Readiness check endpoint
async fn handle_readiness_check(State(state): State<AppState>) -> Response {
    if let Some(metrics_provider) = &state.transport.metrics_provider {
        let readiness_response = metrics_provider.get_readiness_status().await;
        let status_code = if readiness_response.ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        };
        (status_code, Json(readiness_response)).into_response()
    } else {
        // Fallback - always ready if no metrics
        let response = serde_json::json!({
            "ready": true,
            "timestamp": chrono::Utc::now(),
            "message": "No health checks configured"
        });
        (StatusCode::OK, Json(response)).into_response()
    }
}

/// Prometheus-style metrics endpoint
async fn handle_metrics(State(state): State<AppState>) -> Response {
    if let Some(metrics_provider) = &state.transport.metrics_provider {
        let prometheus_metrics = metrics_provider.get_prometheus_metrics().await;
        let content_type = prometheus_metrics.content_type.clone();
        (
            StatusCode::OK,
            [(hyper::header::CONTENT_TYPE, content_type.as_str())],
            prometheus_metrics.data,
        ).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            [(hyper::header::CONTENT_TYPE, "text/plain")],
            "Metrics not available".to_string(),
        ).into_response()
    }
}

/// JSON stats endpoint
async fn handle_stats(State(state): State<AppState>) -> Response {
    if let Some(metrics_provider) = &state.transport.metrics_provider {
        let stats = metrics_provider.get_metrics_snapshot().await;
        (StatusCode::OK, Json(stats)).into_response()
    } else {
        let empty_stats = serde_json::json!({
            "error": "Metrics not available",
            "timestamp": chrono::Utc::now()
        });
        (StatusCode::NOT_FOUND, Json(empty_stats)).into_response()
    }
}

/// SSE authentication info endpoint
async fn handle_sse_auth_info() -> impl IntoResponse {
    let auth_info = super::sse_auth::create_client_auth_instructions();
    (StatusCode::OK, Json(auth_info))
}

/// Authentication middleware (legacy)
async fn auth_middleware(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check and SSE endpoints
    let path = request.uri().path();
    if path == "/health" || path == "/sse" {
        return Ok(next.run(request).await);
    }

    // Check for Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    match auth_header {
        Some(_token) => {
            // TODO: Implement actual token validation
            Ok(next.run(request).await)
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Security-aware authentication middleware
async fn security_auth_middleware(
    State(security_provider): State<Arc<SecurityProvider>>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check endpoint
    let path = request.uri().path();
    if path == "/health" {
        return Ok(next.run(request).await);
    }

    // Special handling for SSE endpoints with enhanced authentication
    if path == "/sse" {
        return handle_sse_authentication(security_provider, headers, request, next).await;
    }

    // Standard authentication for other endpoints
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let authenticated_request = match auth_header {
        Some(header) => {
            security_provider
                .authenticator()
                .parse_authorization_header(header)
                .map_err(|e| {
                    error!("Authentication failed: {}", e);
                    StatusCode::UNAUTHORIZED
                })?
        }
        None => {
            error!("Missing Authorization header");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Add correlation ID to request headers for logging
    let correlation_id = RequestSanitizer::create_correlation_id();
    if let Ok(header_value) = correlation_id.parse() {
        request.headers_mut().insert("X-Correlation-ID", header_value);
    }

    // Store authenticated request information in extensions for handlers to use
    request.extensions_mut().insert(authenticated_request);

    info!("Request authenticated successfully with correlation ID: {}", correlation_id);

    Ok(next.run(request).await)
}

/// Enhanced SSE authentication handler
async fn handle_sse_authentication(
    security_provider: Arc<SecurityProvider>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    // Extract query parameters for SSE authentication
    let uri = request.uri();
    let _query_string = uri.query().unwrap_or("");
    
    // Parse query parameters
    let query_params = axum::extract::Query::<HashMap<String, String>>::try_from_uri(uri)
        .map_err(|_| {
            error!("Failed to parse SSE query parameters");
            StatusCode::BAD_REQUEST
        })?;

    // Create SSE authenticator and authenticate the connection
    let sse_authenticator = SseAuthenticator::new(security_provider.clone());
    
    match sse_authenticator.authenticate_sse_connection(&headers, &query_params).await {
        Ok(sse_connection) => {
            info!("SSE connection authenticated for client: {}", sse_connection.client_id);
            
            // Store SSE connection info for handlers
            request.extensions_mut().insert(sse_connection.authenticated_request.clone());
            request.extensions_mut().insert(sse_connection);
            
            // Add correlation ID
            let correlation_id = RequestSanitizer::create_correlation_id();
            if let Ok(header_value) = correlation_id.parse() {
                request.headers_mut().insert("X-Correlation-ID", header_value);
            }
            
            Ok(next.run(request).await)
        }
        Err(status_code) => {
            error!("SSE authentication failed");
            Err(status_code)
        }
    }
}

/// Metrics collection middleware
async fn metrics_middleware(
    State(metrics_provider): State<Arc<MetricsProvider>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    let start_time = std::time::Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Track active connections
    metrics_provider.increment_active_connections();

    // Process the request
    let response = next.run(request).await;
    
    // Calculate response time
    let response_time = start_time.elapsed();
    let is_error = response.status().is_client_error() || response.status().is_server_error();

    // Record metrics
    metrics_provider.record_request(response_time, is_error);

    // Track tool-specific metrics
    if path.starts_with("/mcp/tools/") {
        let tool_name = path.strip_prefix("/mcp/tools/").unwrap_or("unknown");
        let metric_name = format!("tool_{}_requests", tool_name);
        metrics_provider.increment_custom_metric(&metric_name, 1).await;
        
        if is_error {
            let error_metric = format!("tool_{}_errors", tool_name);
            metrics_provider.increment_custom_metric(&error_metric, 1).await;
        }
    }

    // Track active connections (decrement)
    metrics_provider.decrement_active_connections();

    tracing::debug!(
        "Request {} {} completed in {}ms with status {}",
        method,
        path,
        response_time.as_millis(),
        response.status()
    );

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MessageHandler;
    use tokio::time::Duration;

    #[derive(Clone)]
    struct MockMessageHandler;

    #[async_trait]
    impl MessageHandler for MockMessageHandler {
        async fn handle_message(&self, message: McpMessage) -> Result<Option<McpMessage>> {
            match message {
                McpMessage::ToolsList { id } => Ok(Some(McpMessage::Response {
                    id,
                    result: Some(serde_json::json!({
                        "tools": [
                            {
                                "name": "test_tool",
                                "description": "A test tool",
                                "inputSchema": {}
                            }
                        ]
                    })),
                    error: None,
                })),
                McpMessage::ToolsCall { id, params } => Ok(Some(McpMessage::Response {
                    id,
                    result: Some(serde_json::json!({
                        "result": format!("Called {} with args: {:?}", params.name, params.arguments)
                    })),
                    error: None,
                })),
                _ => Ok(None),
            }
        }
    }

    #[tokio::test]
    async fn test_http_transport_creation() {
        let transport = HttpTransport::new(3001);
        assert_eq!(transport.port, 3001);
        assert_eq!(transport.host, "0.0.0.0");
        assert!(!transport.auth_enabled);
    }

    #[tokio::test]
    async fn test_http_transport_with_config() {
        let transport = HttpTransport::with_config(
            8080,
            "127.0.0.1".to_string(),
            vec!["http://localhost:3000".to_string()],
            true,
        );
        
        assert_eq!(transport.port, 8080);
        assert_eq!(transport.host, "127.0.0.1");
        assert_eq!(transport.cors_origins, vec!["http://localhost:3000"]);
        assert!(transport.auth_enabled);
    }

    #[tokio::test]
    async fn test_auth_token_management() {
        let transport = HttpTransport::new(3002);
        
        // Add a token
        let token = transport.add_auth_token(
            "user123".to_string(), 
            Duration::from_secs(3600)
        ).await;
        
        // Validate the token
        let user_id = transport.validate_token(&token).await;
        assert_eq!(user_id, Some("user123".to_string()));
        
        // Try invalid token
        let invalid_user = transport.validate_token("invalid_token").await;
        assert_eq!(invalid_user, None);
    }

    #[tokio::test]
    async fn test_router_creation() {
        let transport = HttpTransport::new(3003);
        let handler = Arc::new(MockMessageHandler);
        let _router = transport.create_router(handler);
        
        // Router creation should succeed
        // This is a basic test - in a full implementation we'd test actual HTTP requests
        assert!(true); // Placeholder assertion
    }

    #[test]
    fn test_mcp_response_serialization() {
        let response = McpResponse {
            success: true,
            result: Some(serde_json::json!({"test": "data"})),
            error: None,
        };
        
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("success"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
            timestamp: chrono::Utc::now(),
        };
        
        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("healthy"));
        assert!(json.contains("1.0.0"));
    }

    #[tokio::test]
    async fn test_sse_message_sending() {
        let transport = HttpTransport::new(3004);
        
        // Add a mock SSE client
        let (sender, _receiver) = broadcast::channel(100);
        transport.sse_clients.write().await.insert("test_client".to_string(), sender);
        
        let message = SseMessage {
            id: "test_id".to_string(),
            event: "test_event".to_string(),
            data: serde_json::json!({"test": "data"}),
        };
        
        let result = transport.send_sse_message("test_client", message).await;
        assert!(result.is_ok());
        
        // Test non-existent client
        let result2 = transport.send_sse_message("non_existent", SseMessage {
            id: "test_id".to_string(),
            event: "test_event".to_string(),
            data: serde_json::json!({"test": "data"}),
        }).await;
        assert!(result2.is_ok()); // Should not fail even if client doesn't exist
    }
}