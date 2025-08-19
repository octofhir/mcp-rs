//! Security and authentication implementations

pub mod auth;
pub mod validation;

use auth::{AuthConfig, Authenticator};
use validation::{ValidationConfig, InputValidator};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub enable_auth: bool,
    pub api_keys: Vec<String>,
    pub jwt_secret: Option<String>,
    pub max_expression_length: usize,
    pub max_expression_depth: usize,
    pub max_resource_size: usize,
    pub enable_request_logging: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_auth: true,
            api_keys: vec![],
            jwt_secret: None,
            max_expression_length: 1000,
            max_expression_depth: 10,
            max_resource_size: 1024 * 1024, // 1MB
            enable_request_logging: true,
        }
    }
}

pub struct SecurityProvider {
    authenticator: Authenticator,
    validator: InputValidator,
}

impl SecurityProvider {
    pub fn new(config: SecurityConfig) -> Self {
        let auth_config = AuthConfig {
            enable_auth: config.enable_auth,
            api_keys: config.api_keys.into_iter().collect::<HashSet<_>>(),
            jwt_secret: config.jwt_secret.clone(),
            enable_request_logging: config.enable_request_logging,
        };

        let validation_config = ValidationConfig {
            max_expression_length: config.max_expression_length,
            max_expression_depth: config.max_expression_depth,
            max_resource_size: config.max_resource_size,
            ..ValidationConfig::default()
        };

        Self {
            authenticator: Authenticator::new(auth_config),
            validator: InputValidator::new(validation_config),
        }
    }

    pub fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }

    pub fn validator(&self) -> &InputValidator {
        &self.validator
    }
}

pub use auth::{AuthenticatedRequest, AuthMethod};
pub use validation::RequestSanitizer;