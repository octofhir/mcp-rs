use anyhow::{anyhow, Result};
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::sse::Event,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::security::{AuthenticatedRequest, SecurityProvider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseAuthParams {
    pub client_id: Option<String>,
    pub token: Option<String>,
    pub api_key: Option<String>,
    pub refresh_token: Option<String>,
    pub timeout: Option<u64>, // Connection timeout in seconds
}

#[derive(Debug, Clone)]
pub struct SseConnection {
    pub client_id: String,
    pub authenticated_request: AuthenticatedRequest,
    pub connection_time: SystemTime,
    pub last_activity: SystemTime,
    pub timeout_seconds: u64,
    pub supports_refresh: bool,
    pub refresh_token: Option<String>,
}

impl SseConnection {
    pub fn new(
        client_id: String,
        authenticated_request: AuthenticatedRequest,
        timeout_seconds: Option<u64>,
        refresh_token: Option<String>,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            client_id,
            authenticated_request,
            connection_time: now,
            last_activity: now,
            timeout_seconds: timeout_seconds.unwrap_or(3600), // Default 1 hour
            supports_refresh: refresh_token.is_some(),
            refresh_token,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Ok(elapsed) = self.last_activity.elapsed() {
            elapsed.as_secs() > self.timeout_seconds
        } else {
            true // Assume expired if we can't determine elapsed time
        }
    }

    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    pub fn time_until_expiry(&self) -> Option<Duration> {
        if let Ok(elapsed) = self.last_activity.elapsed() {
            let elapsed_secs = elapsed.as_secs();
            if elapsed_secs < self.timeout_seconds {
                Some(Duration::from_secs(self.timeout_seconds - elapsed_secs))
            } else {
                None // Already expired
            }
        } else {
            None
        }
    }
}

pub struct SseAuthenticator {
    security_provider: Arc<SecurityProvider>,
}

impl SseAuthenticator {
    pub fn new(security_provider: Arc<SecurityProvider>) -> Self {
        Self { security_provider }
    }

    /// Authenticate SSE connection using multiple methods
    pub async fn authenticate_sse_connection(
        &self,
        headers: &HeaderMap,
        query_params: &Query<HashMap<String, String>>,
    ) -> Result<SseConnection, StatusCode> {
        // Extract parameters
        let sse_params = self.extract_sse_params(query_params);
        let client_id = sse_params
            .client_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        debug!(
            "Authenticating SSE connection for client_id: {}",
            client_id
        );

        // Try multiple authentication methods in order of preference
        let authenticated_request = self
            .try_header_auth(headers)
            .or_else(|_| self.try_query_auth(&sse_params))
            .map_err(|e| {
                warn!("SSE authentication failed for {}: {}", client_id, e);
                StatusCode::UNAUTHORIZED
            })?;

        info!(
            "SSE connection authenticated for client {} via {}",
            client_id,
            match authenticated_request.authenticated_by {
                crate::security::AuthMethod::ApiKey(_) => "API key",
                crate::security::AuthMethod::JwtToken(_) => "JWT token",
                crate::security::AuthMethod::Bypass => "bypass",
            }
        );

        Ok(SseConnection::new(
            client_id,
            authenticated_request,
            sse_params.timeout,
            sse_params.refresh_token,
        ))
    }

    fn extract_sse_params(&self, query_params: &Query<HashMap<String, String>>) -> SseAuthParams {
        SseAuthParams {
            client_id: query_params.get("client_id").cloned(),
            token: query_params.get("token").cloned(),
            api_key: query_params.get("api_key").cloned(),
            refresh_token: query_params.get("refresh_token").cloned(),
            timeout: query_params
                .get("timeout")
                .and_then(|t| t.parse().ok()),
        }
    }

    fn try_header_auth(&self, headers: &HeaderMap) -> Result<AuthenticatedRequest> {
        let auth_header = headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| anyhow!("No Authorization header found"))?;

        self.security_provider
            .authenticator()
            .parse_authorization_header(auth_header)
    }

    fn try_query_auth(&self, params: &SseAuthParams) -> Result<AuthenticatedRequest> {
        // Try JWT token from query
        if let Some(token) = &params.token {
            debug!("Attempting JWT authentication via query parameter");
            return self.security_provider.authenticator().authenticate_jwt(token);
        }

        // Try API key from query
        if let Some(api_key) = &params.api_key {
            debug!("Attempting API key authentication via query parameter");
            return self
                .security_provider
                .authenticator()
                .authenticate_api_key(api_key);
        }

        Err(anyhow!("No valid authentication credentials in query parameters"))
    }

    /// Generate token refresh event for SSE clients
    pub fn create_refresh_event(&self, connection: &SseConnection) -> Option<Event> {
        if !connection.supports_refresh {
            return None;
        }

        if let Some(time_left) = connection.time_until_expiry() {
            // Send refresh notification when 5 minutes left
            if time_left.as_secs() <= 300 {
                let refresh_data = json!({
                    "type": "token_refresh_required",
                    "client_id": connection.client_id,
                    "expires_in_seconds": time_left.as_secs(),
                    "refresh_token": connection.refresh_token,
                    "instructions": {
                        "action": "refresh_connection",
                        "method": "reconnect_with_new_token",
                        "url_pattern": "/sse?token=NEW_TOKEN&client_id={}",
                    }
                });

                return Some(
                    Event::default()
                        .event("token_refresh")
                        .data(refresh_data.to_string()),
                );
            }
        }

        None
    }

    /// Generate authentication error event
    pub fn create_auth_error_event(&self, error: &str, client_id: &str) -> Event {
        let error_data = json!({
            "type": "authentication_error",
            "client_id": client_id,
            "error": error,
            "reconnect_required": true,
            "instructions": {
                "action": "reconnect_with_valid_credentials",
                "supported_methods": [
                    "Authorization header: Bearer <token>",
                    "Query parameter: ?token=<jwt_token>",
                    "Query parameter: ?api_key=<api_key>"
                ]
            }
        });

        Event::default()
            .event("auth_error")
            .data(error_data.to_string())
    }

    /// Generate connection status event
    pub fn create_connection_event(&self, connection: &SseConnection) -> Event {
        let connection_data = json!({
            "type": "connection_established",
            "client_id": connection.client_id,
            "authenticated": true,
            "connection_time": connection.connection_time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
            "timeout_seconds": connection.timeout_seconds,
            "supports_refresh": connection.supports_refresh,
            "expires_in": connection.time_until_expiry().map(|d| d.as_secs())
        });

        Event::default()
            .event("connected")
            .data(connection_data.to_string())
    }

    /// Validate existing SSE connection and check for expiration
    pub async fn validate_connection(&self, connection: &mut SseConnection) -> Result<(), Event> {
        connection.update_activity();

        if connection.is_expired() {
            let error_event = self.create_auth_error_event(
                "Connection expired. Please reconnect with valid credentials.",
                &connection.client_id,
            );
            return Err(error_event);
        }

        // Check if refresh is needed soon
        if let Some(_refresh_event) = self.create_refresh_event(connection) {
            // Return the refresh event as an "error" to be sent to client
            // This is a notification, not a fatal error
            info!(
                "Sending refresh notification to client {}",
                connection.client_id
            );
        }

        Ok(())
    }
}

/// Enhanced authentication support for EventSource clients
pub fn create_client_auth_instructions() -> serde_json::Value {
    json!({
        "sse_authentication": {
            "description": "SSE connections support multiple authentication methods for better client compatibility",
            "methods": [
                {
                    "name": "Authorization Header",
                    "description": "Standard HTTP Authorization header (preferred for browsers)",
                    "example": "Authorization: Bearer <jwt_token_or_api_key>",
                    "client_support": "Modern browsers, curl, most HTTP clients"
                },
                {
                    "name": "Query Parameter Token",
                    "description": "JWT token via query parameter (for EventSource compatibility)",
                    "example": "/sse?token=<jwt_token>&client_id=<optional_id>",
                    "client_support": "EventSource, older browsers, simple clients"
                },
                {
                    "name": "Query Parameter API Key",
                    "description": "API key via query parameter",
                    "example": "/sse?api_key=<api_key>&client_id=<optional_id>",
                    "client_support": "All clients"
                }
            ],
            "optional_parameters": {
                "client_id": "Custom client identifier (auto-generated if not provided)",
                "timeout": "Connection timeout in seconds (default: 3600)",
                "refresh_token": "Token for connection refresh notifications"
            },
            "token_refresh": {
                "description": "Long-lived connections receive refresh notifications",
                "event_type": "token_refresh",
                "notification_timing": "5 minutes before expiration",
                "recommended_action": "Reconnect with new credentials"
            },
            "error_handling": {
                "auth_failures": "auth_error events with reconnection instructions",
                "token_expiry": "Automatic connection termination with error event",
                "network_issues": "Standard EventSource reconnection logic applies"
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{SecurityConfig, SecurityProvider};

    fn create_test_security_provider() -> Arc<SecurityProvider> {
        let mut config = SecurityConfig::default();
        config.api_keys = vec!["test-api-key".to_string()];
        config.jwt_secret = Some("test-secret".to_string());
        Arc::new(SecurityProvider::new(config))
    }

    #[test]
    fn test_sse_connection_expiry() {
        let auth_req = crate::security::AuthenticatedRequest {
            request_id: Uuid::new_v4(),
            authenticated_by: crate::security::AuthMethod::Bypass,
            subject: "test".to_string(),
        };

        let mut connection = SseConnection::new(
            "test-client".to_string(),
            auth_req,
            Some(1), // 1 second timeout
            None,
        );

        // Should not be expired immediately
        assert!(!connection.is_expired());

        // Wait and check expiry
        std::thread::sleep(Duration::from_secs(2));
        assert!(connection.is_expired());
    }

    #[test]
    fn test_extract_sse_params() {
        let security_provider = create_test_security_provider();
        let authenticator = SseAuthenticator::new(security_provider);

        let mut query_map = HashMap::new();
        query_map.insert("client_id".to_string(), "test-client".to_string());
        query_map.insert("token".to_string(), "test-token".to_string());
        query_map.insert("timeout".to_string(), "7200".to_string());

        let query = Query(query_map);
        let params = authenticator.extract_sse_params(&query);

        assert_eq!(params.client_id, Some("test-client".to_string()));
        assert_eq!(params.token, Some("test-token".to_string()));
        assert_eq!(params.timeout, Some(7200));
    }

    #[test]
    fn test_connection_refresh_timing() {
        let auth_req = crate::security::AuthenticatedRequest {
            request_id: Uuid::new_v4(),
            authenticated_by: crate::security::AuthMethod::Bypass,
            subject: "test".to_string(),
        };

        let connection = SseConnection::new(
            "test-client".to_string(),
            auth_req,
            Some(600), // 10 minutes
            Some("refresh-token".to_string()),
        );

        let security_provider = create_test_security_provider();
        let authenticator = SseAuthenticator::new(security_provider);

        // Should not need refresh immediately (more than 5 minutes left)
        assert!(authenticator.create_refresh_event(&connection).is_none());
    }

    #[test]
    fn test_client_auth_instructions() {
        let instructions = create_client_auth_instructions();
        
        assert!(instructions["sse_authentication"]["methods"].is_array());
        assert_eq!(instructions["sse_authentication"]["methods"].as_array().unwrap().len(), 3);
        
        let methods = &instructions["sse_authentication"]["methods"];
        assert!(methods[0]["name"].as_str().unwrap().contains("Authorization Header"));
        assert!(methods[1]["name"].as_str().unwrap().contains("Query Parameter Token"));
        assert!(methods[2]["name"].as_str().unwrap().contains("Query Parameter API Key"));
    }
}