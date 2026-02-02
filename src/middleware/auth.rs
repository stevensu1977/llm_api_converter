//! Authentication middleware
//!
//! This module provides API key authentication for the Anthropic-Bedrock proxy.
//! It validates API keys against DynamoDB and supports a master key for admin access.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::Settings;
use crate::db::repositories::{ApiKeyError, ApiKeyRepository};
use crate::db::DynamoDbClient;
use crate::schemas::anthropic::ErrorResponse;
use crate::utils::truncate_str;

// ============================================================================
// API Key Info
// ============================================================================

/// Information about the authenticated API key
///
/// This struct is injected into request extensions after successful authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    /// The API key string (truncated for security in logs)
    pub api_key: String,

    /// The user ID associated with this key
    pub user_id: String,

    /// Whether this is the master API key
    pub is_master: bool,

    /// Custom rate limit for this key (requests per minute)
    pub rate_limit: Option<u32>,

    /// Service tier (affects pricing multiplier)
    pub service_tier: String,

    /// Monthly budget limit (if set)
    pub monthly_budget: Option<f64>,

    /// Current month-to-date budget usage
    pub budget_used_mtd: f64,
}

impl ApiKeyInfo {
    /// Create ApiKeyInfo for master key
    pub fn master(api_key: &str) -> Self {
        Self {
            api_key: Self::truncate_key(api_key),
            user_id: "master".to_string(),
            is_master: true,
            rate_limit: None, // No rate limit for master key
            service_tier: "master".to_string(),
            monthly_budget: None,
            budget_used_mtd: 0.0,
        }
    }

    /// Create ApiKeyInfo from a validated DynamoDB API key
    pub fn from_db_key(key: &crate::db::models::ApiKey) -> Self {
        Self {
            api_key: Self::truncate_key(&key.api_key),
            user_id: key.user_id.clone(),
            is_master: false,
            rate_limit: if key.rate_limit > 0 { Some(key.rate_limit as u32) } else { None },
            service_tier: key.service_tier.clone(),
            monthly_budget: key.monthly_budget,
            budget_used_mtd: key.budget_used_mtd,
        }
    }

    /// Truncate API key for safe logging (show first 8 chars + ...)
    fn truncate_key(key: &str) -> String {
        if key.chars().count() > 12 {
            format!("{}...", truncate_str(key, 8))
        } else {
            key.to_string()
        }
    }

    /// Check if rate limiting should be bypassed
    pub fn bypass_rate_limit(&self) -> bool {
        self.is_master
    }

    /// Get effective rate limit (requests per minute)
    pub fn effective_rate_limit(&self, default: u32) -> u32 {
        self.rate_limit.unwrap_or(default)
    }
}

// ============================================================================
// Authentication Errors
// ============================================================================

/// Authentication error types
#[derive(Debug)]
pub enum AuthError {
    /// No API key provided in request
    MissingApiKey,
    /// API key is invalid or not found
    InvalidApiKey,
    /// API key is inactive (deactivated)
    InactiveKey { reason: Option<String> },
    /// Internal error during authentication
    InternalError(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            AuthError::MissingApiKey => (
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                "Missing API key. Include 'x-api-key' or 'Authorization: Bearer <key>' header in your request.",
            ),
            AuthError::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                "Invalid API key. Please check your API key and try again.",
            ),
            AuthError::InactiveKey { reason } => {
                let msg = match reason.as_deref() {
                    Some("budget_exceeded") => "API key has been deactivated due to budget limit exceeded.",
                    Some(r) => return (
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse::new("permission_error", &format!("API key is inactive: {}", r))),
                    ).into_response(),
                    None => "API key is inactive.",
                };
                (StatusCode::FORBIDDEN, "permission_error", msg)
            }
            AuthError::InternalError(msg) => {
                tracing::error!(error = %msg, "Authentication internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "api_error",
                    "An internal error occurred during authentication.",
                )
            }
        };

        let error_response = ErrorResponse::new(error_type, message);
        (status, Json(error_response)).into_response()
    }
}

// ============================================================================
// Authentication Middleware
// ============================================================================

/// Authentication state required by the middleware
#[derive(Clone)]
pub struct AuthState {
    pub settings: Arc<Settings>,
    pub api_key_repo: ApiKeyRepository,
}

impl AuthState {
    pub fn new(settings: Arc<Settings>, dynamodb: Arc<DynamoDbClient>) -> Self {
        Self {
            settings,
            api_key_repo: ApiKeyRepository::new(dynamodb),
        }
    }
}

/// Middleware to require API key authentication
///
/// This middleware:
/// 1. Extracts the `x-api-key` header from the request
/// 2. Checks if it matches the master key (if configured)
/// 3. Validates the key against DynamoDB
/// 4. Injects `ApiKeyInfo` into request extensions on success
///
/// # Errors
/// - 401 Unauthorized: Missing or invalid API key
/// - 403 Forbidden: API key is inactive
/// - 500 Internal Server Error: Database error
pub async fn require_api_key(
    State(auth_state): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    // Check if authentication is required
    if !auth_state.settings.require_api_key {
        tracing::debug!("API key authentication disabled, skipping");
        // Inject a placeholder ApiKeyInfo for disabled auth
        request.extensions_mut().insert(ApiKeyInfo {
            api_key: "disabled".to_string(),
            user_id: "anonymous".to_string(),
            is_master: false,
            rate_limit: None,
            service_tier: "default".to_string(),
            monthly_budget: None,
            budget_used_mtd: 0.0,
        });
        return Ok(next.run(request).await);
    }

    // Extract API key from header
    // Support both x-api-key (Anthropic style) and Authorization: Bearer (OpenAI style)
    let api_key = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            // Try Authorization: Bearer <token> format (OpenAI style)
            request
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    let Some(api_key) = api_key else {
        tracing::warn!("Request missing API key (x-api-key or Authorization: Bearer)");
        return Err(AuthError::MissingApiKey);
    };

    // Check if it's the master key
    if let Some(ref master_key) = auth_state.settings.master_api_key {
        if api_key == *master_key {
            tracing::debug!(key = %ApiKeyInfo::truncate_key(&api_key), "Master key authenticated");
            request.extensions_mut().insert(ApiKeyInfo::master(&api_key));
            return Ok(next.run(request).await);
        }
    }

    // Check if it's the ephemeral key (generated at startup)
    if let Some(ref ephemeral_key) = auth_state.settings.ephemeral_api_key {
        if api_key == *ephemeral_key {
            tracing::debug!(key = %ApiKeyInfo::truncate_key(&api_key), "Ephemeral key authenticated");
            request.extensions_mut().insert(ApiKeyInfo {
                api_key: ApiKeyInfo::truncate_key(&api_key),
                user_id: "ephemeral".to_string(),
                is_master: false,
                rate_limit: None,
                service_tier: "default".to_string(),
                monthly_budget: None,
                budget_used_mtd: 0.0,
            });
            return Ok(next.run(request).await);
        }
    }

    // Validate against DynamoDB
    let validation_result = auth_state
        .api_key_repo
        .validate_api_key(&api_key)
        .await
        .map_err(|e| match e {
            ApiKeyError::DynamoDb(msg) => AuthError::InternalError(msg),
            ApiKeyError::NotFound => AuthError::InvalidApiKey,
            ApiKeyError::ParseError(msg) => AuthError::InternalError(msg),
        })?;

    match validation_result {
        Some(db_key) if db_key.is_active => {
            tracing::debug!(
                key = %ApiKeyInfo::truncate_key(&api_key),
                user_id = %db_key.user_id,
                "API key authenticated"
            );
            request.extensions_mut().insert(ApiKeyInfo::from_db_key(&db_key));
            Ok(next.run(request).await)
        }
        Some(db_key) => {
            // Key exists but is inactive
            tracing::warn!(
                key = %ApiKeyInfo::truncate_key(&api_key),
                user_id = %db_key.user_id,
                reason = ?db_key.deactivated_reason,
                "Inactive API key used"
            );
            Err(AuthError::InactiveKey {
                reason: db_key.deactivated_reason,
            })
        }
        None => {
            tracing::warn!(key = %ApiKeyInfo::truncate_key(&api_key), "Invalid API key");
            Err(AuthError::InvalidApiKey)
        }
    }
}

// ============================================================================
// Extension Extraction
// ============================================================================

/// Extract ApiKeyInfo from request extensions
///
/// Use this in handlers to access the authenticated user's information.
pub fn get_api_key_info<B>(request: &Request<B>) -> Option<&ApiKeyInfo> {
    request.extensions().get::<ApiKeyInfo>()
}

/// Extract API key from request headers
///
/// Supports both `x-api-key` (Anthropic style) and `Authorization: Bearer` (OpenAI style).
/// Returns None if no API key is found.
pub fn extract_api_key<B>(request: &Request<B>) -> Option<String> {
    request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            // Try Authorization: Bearer <token> format (OpenAI style)
            request
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_info_master() {
        let info = ApiKeyInfo::master("sk-ant-master-key-12345");
        assert!(info.is_master);
        assert_eq!(info.user_id, "master");
        assert_eq!(info.service_tier, "master");
        assert!(info.bypass_rate_limit());
    }

    #[test]
    fn test_api_key_truncation() {
        let truncated = ApiKeyInfo::truncate_key("sk-ant-api-key-very-long-string");
        assert_eq!(truncated, "sk-ant-a...");

        let short = ApiKeyInfo::truncate_key("short");
        assert_eq!(short, "short");
    }

    #[test]
    fn test_effective_rate_limit() {
        let mut info = ApiKeyInfo::master("key");
        info.rate_limit = Some(50);
        assert_eq!(info.effective_rate_limit(100), 50);

        info.rate_limit = None;
        assert_eq!(info.effective_rate_limit(100), 100);
    }

    #[test]
    fn test_auth_error_status_codes() {
        // Test that error types map to correct status codes
        let missing = AuthError::MissingApiKey;
        let response = missing.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let invalid = AuthError::InvalidApiKey;
        let response = invalid.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let inactive = AuthError::InactiveKey { reason: None };
        let response = inactive.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
