//! Gemini service for Google Gemini API interactions
//!
//! This module handles communication with Google Gemini API using REST.
//! Supports both streaming and non-streaming responses with multi-key
//! load balancing support.

use crate::schemas::gemini::{GeminiError, GeminiRequest, GeminiResponse, StreamChunk};
use crate::services::backend_pool::{
    ApiKeyCredential, Credential, CredentialPool, LoadBalanceStrategy, PoolConfig,
};
use reqwest::Client;
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// Constants
// ============================================================================

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when calling the Gemini API
#[derive(Error, Debug)]
pub enum GeminiServiceError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("API error: {code} - {message}")]
    ApiError { code: i32, message: String },

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Missing API key")]
    MissingApiKey,

    #[error("No available credentials in pool")]
    NoAvailableCredentials,

    #[error("Stream error: {0}")]
    StreamError(String),
}

// ============================================================================
// Gemini Service
// ============================================================================

/// Configuration for Gemini service
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// API keys for authentication (supports multiple keys for load balancing)
    pub api_keys: Vec<String>,

    /// Base URL (default: generativelanguage.googleapis.com)
    pub base_url: Option<String>,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Load balance strategy
    pub strategy: LoadBalanceStrategy,

    /// Maximum failures before disabling a credential
    pub max_failures: u32,

    /// Seconds to wait before retrying a disabled credential
    pub retry_after_secs: u64,
}

impl GeminiConfig {
    /// Create config with a single API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_keys: vec![api_key.into()],
            base_url: None,
            timeout_seconds: 120,
            strategy: LoadBalanceStrategy::RoundRobin,
            max_failures: 3,
            retry_after_secs: 300,
        }
    }

    /// Create config with multiple API keys
    pub fn with_keys(api_keys: Vec<String>) -> Self {
        Self {
            api_keys,
            base_url: None,
            timeout_seconds: 120,
            strategy: LoadBalanceStrategy::RoundRobin,
            max_failures: 3,
            retry_after_secs: 300,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    pub fn with_strategy(mut self, strategy: LoadBalanceStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_max_failures(mut self, max: u32) -> Self {
        self.max_failures = max;
        self
    }

    pub fn with_retry_after(mut self, secs: u64) -> Self {
        self.retry_after_secs = secs;
        self
    }
}

/// Service for interacting with Google Gemini API
/// Supports multiple API keys with load balancing
pub struct GeminiService {
    /// HTTP client
    client: Client,

    /// Base URL for API calls
    base_url: Option<String>,

    /// Credential pool for API keys
    credential_pool: Arc<CredentialPool<ApiKeyCredential>>,
}

impl Clone for GeminiService {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            base_url: self.base_url.clone(),
            credential_pool: Arc::clone(&self.credential_pool),
        }
    }
}

impl GeminiService {
    /// Create a new Gemini service
    pub fn new(config: GeminiConfig) -> Result<Self, GeminiServiceError> {
        if config.api_keys.is_empty() {
            return Err(GeminiServiceError::MissingApiKey);
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()?;

        // Create credentials from API keys
        let credentials: Vec<ApiKeyCredential> = config
            .api_keys
            .iter()
            .enumerate()
            .map(|(idx, key)| ApiKeyCredential::new(key, format!("gemini_key_{}", idx + 1), 1))
            .collect();

        // Create pool config
        let pool_config = PoolConfig::new(config.strategy)
            .with_max_failures(config.max_failures)
            .with_retry_after(config.retry_after_secs);

        let credential_pool = CredentialPool::new(credentials, pool_config);

        tracing::info!(
            key_count = credential_pool.len(),
            strategy = %config.strategy,
            "Initialized Gemini service with credential pool"
        );

        Ok(Self {
            client,
            base_url: config.base_url,
            credential_pool: Arc::new(credential_pool),
        })
    }

    /// Create a Gemini service with a single API key (backward compatibility)
    pub fn with_single_key(api_key: impl Into<String>) -> Result<Self, GeminiServiceError> {
        Self::new(GeminiConfig::new(api_key))
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.base_url.as_deref().unwrap_or(GEMINI_API_BASE)
    }

    /// Get the next available credential from the pool
    fn get_credential(&self) -> Result<&ApiKeyCredential, GeminiServiceError> {
        self.credential_pool
            .get_next()
            .ok_or(GeminiServiceError::NoAvailableCredentials)
    }

    /// Record a successful request for a credential
    pub fn record_success(&self, credential_name: &str) {
        self.credential_pool.record_success(credential_name);
    }

    /// Record a failed request for a credential
    /// Returns true if the credential was disabled due to max failures
    pub fn record_failure(&self, credential_name: &str) -> bool {
        self.credential_pool.record_failure(credential_name)
    }

    /// Get pool statistics
    pub fn pool_stats(&self) -> crate::services::backend_pool::PoolStats {
        self.credential_pool.stats()
    }

    /// Generate content (non-streaming)
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "gemini-2.0-flash")
    /// * `request` - The request body
    pub async fn generate_content(
        &self,
        model: &str,
        request: &GeminiRequest,
    ) -> Result<GeminiResponse, GeminiServiceError> {
        let credential = self.get_credential()?;
        let credential_name = credential.name().to_string();
        let api_key = credential.api_key().to_string();

        let url = format!("{}/models/{}:generateContent", self.base_url(), model);

        tracing::debug!(
            model = %model,
            url = %url,
            credential = %credential_name,
            "Calling Gemini generateContent API"
        );

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();

                if !status.is_success() {
                    let error_text = resp.text().await.unwrap_or_default();

                    // Record failure for rate limit or server errors
                    if status.as_u16() == 429 || status.as_u16() >= 500 {
                        let disabled = self.record_failure(&credential_name);
                        if disabled {
                            tracing::warn!(
                                credential = %credential_name,
                                "Credential disabled due to repeated failures"
                            );
                        }
                    }

                    // Try to parse as Gemini error
                    if let Ok(gemini_error) = serde_json::from_str::<GeminiError>(&error_text) {
                        return Err(GeminiServiceError::ApiError {
                            code: gemini_error.error.code,
                            message: gemini_error.error.message,
                        });
                    }

                    return Err(GeminiServiceError::ApiError {
                        code: status.as_u16() as i32,
                        message: error_text,
                    });
                }

                // Record success
                self.record_success(&credential_name);

                let response_text = resp.text().await?;

                serde_json::from_str(&response_text).map_err(|e| {
                    tracing::error!(error = %e, body = %response_text, "Failed to parse Gemini response");
                    GeminiServiceError::ParseError(e.to_string())
                })
            }
            Err(e) => {
                // Record failure on connection/timeout errors
                self.record_failure(&credential_name);
                Err(GeminiServiceError::HttpError(e))
            }
        }
    }

    /// Generate content with streaming
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "gemini-2.0-flash")
    /// * `request` - The request body
    ///
    /// Returns a tuple of (stream, credential_name) so the caller can record success/failure
    pub async fn generate_content_stream(
        &self,
        model: &str,
        request: &GeminiRequest,
    ) -> Result<(GeminiStream, String), GeminiServiceError> {
        let credential = self.get_credential()?;
        let credential_name = credential.name().to_string();
        let api_key = credential.api_key().to_string();

        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url(),
            model
        );

        tracing::debug!(
            model = %model,
            url = %url,
            credential = %credential_name,
            "Calling Gemini streamGenerateContent API"
        );

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();

                if !status.is_success() {
                    let error_text = resp.text().await.unwrap_or_default();

                    // Record failure for rate limit or server errors
                    if status.as_u16() == 429 || status.as_u16() >= 500 {
                        self.record_failure(&credential_name);
                    }

                    if let Ok(gemini_error) = serde_json::from_str::<GeminiError>(&error_text) {
                        return Err(GeminiServiceError::ApiError {
                            code: gemini_error.error.code,
                            message: gemini_error.error.message,
                        });
                    }

                    return Err(GeminiServiceError::ApiError {
                        code: status.as_u16() as i32,
                        message: error_text,
                    });
                }

                Ok((GeminiStream::new(resp), credential_name))
            }
            Err(e) => {
                self.record_failure(&credential_name);
                Err(GeminiServiceError::HttpError(e))
            }
        }
    }

    /// Check if the service is healthy (at least one credential available)
    pub fn health_check(&self) -> bool {
        self.credential_pool.healthy_count() > 0
    }

    /// Get the number of API keys in the pool
    pub fn key_count(&self) -> usize {
        self.credential_pool.len()
    }

    /// Get the number of healthy keys
    pub fn healthy_key_count(&self) -> usize {
        self.credential_pool.healthy_count()
    }
}

// ============================================================================
// Streaming Support
// ============================================================================

/// A stream of Gemini response chunks
pub struct GeminiStream {
    response: reqwest::Response,
    buffer: String,
}

impl GeminiStream {
    fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            buffer: String::new(),
        }
    }

    /// Receive the next chunk from the stream
    pub async fn recv(&mut self) -> Result<Option<StreamChunk>, GeminiServiceError> {
        loop {
            // Check if we have a complete event in the buffer
            if let Some(pos) = self.buffer.find("\n\n") {
                let event = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + 2..].to_string();

                // Parse SSE event
                if let Some(data) = event.strip_prefix("data: ") {
                    let data = data.trim();

                    // Skip empty data or [DONE]
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }

                    match serde_json::from_str::<StreamChunk>(data) {
                        Ok(chunk) => return Ok(Some(chunk)),
                        Err(e) => {
                            tracing::warn!(error = %e, data = %data, "Failed to parse stream chunk");
                            continue;
                        }
                    }
                }
                continue;
            }

            // Read more data from the response
            match self.response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                        self.buffer.push_str(&text);
                    }
                }
                Ok(None) => {
                    // Stream ended
                    return Ok(None);
                }
                Err(e) => {
                    return Err(GeminiServiceError::StreamError(e.to_string()));
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_config_single_key() {
        let config = GeminiConfig::new("test-api-key").with_base_url("https://custom.api.com");

        assert_eq!(config.api_keys, vec!["test-api-key".to_string()]);
        assert_eq!(config.base_url, Some("https://custom.api.com".to_string()));
    }

    #[test]
    fn test_gemini_config_multiple_keys() {
        let config = GeminiConfig::with_keys(vec![
            "key1".to_string(),
            "key2".to_string(),
            "key3".to_string(),
        ])
        .with_strategy(LoadBalanceStrategy::Weighted);

        assert_eq!(config.api_keys.len(), 3);
        assert_eq!(config.strategy, LoadBalanceStrategy::Weighted);
    }

    #[test]
    fn test_gemini_service_creation() {
        let config = GeminiConfig::with_keys(vec!["key1".to_string(), "key2".to_string()]);

        let service = GeminiService::new(config).expect("Should create service");

        assert_eq!(service.key_count(), 2);
        assert_eq!(service.healthy_key_count(), 2);
        assert!(service.health_check());
    }

    #[test]
    fn test_gemini_service_empty_keys_error() {
        let config = GeminiConfig::with_keys(vec![]);

        let result = GeminiService::new(config);

        assert!(result.is_err());
    }
}
