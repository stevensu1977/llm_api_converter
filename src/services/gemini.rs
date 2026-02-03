//! Gemini service for Google Gemini API interactions
//!
//! This module handles communication with Google Gemini API using REST.
//! Supports both streaming and non-streaming responses.

use crate::schemas::gemini::{GeminiError, GeminiRequest, GeminiResponse, StreamChunk};
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

    #[error("Stream error: {0}")]
    StreamError(String),
}

// ============================================================================
// Gemini Service
// ============================================================================

/// Configuration for Gemini service
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// API key for authentication
    pub api_key: String,

    /// Base URL (default: generativelanguage.googleapis.com)
    pub base_url: Option<String>,

    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl GeminiConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: None,
            timeout_seconds: 120,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }
}

/// Service for interacting with Google Gemini API
#[derive(Clone)]
pub struct GeminiService {
    /// HTTP client
    client: Client,

    /// Service configuration
    config: Arc<GeminiConfig>,
}

impl GeminiService {
    /// Create a new Gemini service
    pub fn new(config: GeminiConfig) -> Result<Self, GeminiServiceError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()?;

        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Get the base URL
    fn base_url(&self) -> &str {
        self.config
            .base_url
            .as_deref()
            .unwrap_or(GEMINI_API_BASE)
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
        let url = format!(
            "{}/models/{}:generateContent",
            self.base_url(),
            model
        );

        tracing::debug!(
            model = %model,
            url = %url,
            "Calling Gemini generateContent API"
        );

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

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

        let response_text = response.text().await?;

        serde_json::from_str(&response_text).map_err(|e| {
            tracing::error!(error = %e, body = %response_text, "Failed to parse Gemini response");
            GeminiServiceError::ParseError(e.to_string())
        })
    }

    /// Generate content with streaming
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "gemini-2.0-flash")
    /// * `request` - The request body
    pub async fn generate_content_stream(
        &self,
        model: &str,
        request: &GeminiRequest,
    ) -> Result<GeminiStream, GeminiServiceError> {
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_url(),
            model
        );

        tracing::debug!(
            model = %model,
            url = %url,
            "Calling Gemini streamGenerateContent API"
        );

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.config.api_key)
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

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

        Ok(GeminiStream::new(response))
    }

    /// Check if the service is healthy
    pub fn health_check(&self) -> bool {
        !self.config.api_key.is_empty()
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
    fn test_gemini_config() {
        let config = GeminiConfig::new("test-api-key")
            .with_base_url("https://custom.api.com");

        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.base_url, Some("https://custom.api.com".to_string()));
    }
}
