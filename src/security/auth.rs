use anyhow::{Result, anyhow};
use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub enable_auth: bool,
    pub api_keys: HashSet<String>,
    pub jwt_secret: Option<String>,
    pub enable_request_logging: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            api_keys: HashSet::new(),
            jwt_secret: None,
            enable_request_logging: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedRequest {
    pub request_id: Uuid,
    pub authenticated_by: AuthMethod,
    pub subject: String,
}

#[derive(Debug, Clone)]
pub enum AuthMethod {
    ApiKey(String),
    JwtToken(Claims),
    Bypass,
}

pub struct Authenticator {
    config: AuthConfig,
}

impl Authenticator {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }

    pub fn authenticate_api_key(&self, api_key: &str) -> Result<AuthenticatedRequest> {
        if !self.config.enable_auth {
            return Ok(AuthenticatedRequest {
                request_id: Uuid::new_v4(),
                authenticated_by: AuthMethod::Bypass,
                subject: "local".to_string(),
            });
        }

        if self.config.api_keys.contains(api_key) {
            Ok(AuthenticatedRequest {
                request_id: Uuid::new_v4(),
                authenticated_by: AuthMethod::ApiKey(api_key.to_string()),
                subject: format!("api_key:{}", &api_key[..8]),
            })
        } else {
            Err(anyhow!("Invalid API key"))
        }
    }

    pub fn authenticate_jwt(&self, token: &str) -> Result<AuthenticatedRequest> {
        if !self.config.enable_auth {
            return Ok(AuthenticatedRequest {
                request_id: Uuid::new_v4(),
                authenticated_by: AuthMethod::Bypass,
                subject: "local".to_string(),
            });
        }

        let jwt_secret = self
            .config
            .jwt_secret
            .as_ref()
            .ok_or_else(|| anyhow!("JWT secret not configured"))?;

        let token_data: TokenData<Claims> = decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|e| anyhow!("JWT validation failed: {}", e))?;

        Ok(AuthenticatedRequest {
            request_id: Uuid::new_v4(),
            authenticated_by: AuthMethod::JwtToken(token_data.claims.clone()),
            subject: token_data.claims.sub.clone(),
        })
    }

    pub fn parse_authorization_header(&self, auth_header: &str) -> Result<AuthenticatedRequest> {
        if let Some(bearer_token) = auth_header.strip_prefix("Bearer ") {
            if bearer_token.starts_with("eyJ") {
                self.authenticate_jwt(bearer_token)
            } else {
                self.authenticate_api_key(bearer_token)
            }
        } else {
            Err(anyhow!("Invalid authorization header format"))
        }
    }

    pub fn bypass_for_stdio(&self) -> AuthenticatedRequest {
        AuthenticatedRequest {
            request_id: Uuid::new_v4(),
            authenticated_by: AuthMethod::Bypass,
            subject: "stdio".to_string(),
        }
    }

    pub fn is_auth_enabled(&self) -> bool {
        self.config.enable_auth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_authentication() {
        let mut config = AuthConfig::default();
        config.api_keys.insert("test-key-123".to_string());
        let auth = Authenticator::new(config);

        let result = auth.authenticate_api_key("test-key-123");
        assert!(result.is_ok());

        let result = auth.authenticate_api_key("invalid-key");
        assert!(result.is_err());
    }

    #[test]
    fn test_disabled_auth() {
        let mut config = AuthConfig::default();
        config.enable_auth = false;
        let auth = Authenticator::new(config);

        let result = auth.authenticate_api_key("any-key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdio_bypass() {
        let config = AuthConfig::default();
        let auth = Authenticator::new(config);
        let result = auth.bypass_for_stdio();

        matches!(result.authenticated_by, AuthMethod::Bypass);
        assert_eq!(result.subject, "stdio");
    }
}
