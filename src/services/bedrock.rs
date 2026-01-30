//! Bedrock service for AWS Bedrock API interactions
//!
//! This module handles communication with AWS Bedrock for model inference.
//! It uses the Converse API and ConverseStream API for all models.

use aws_sdk_bedrockruntime::{
    operation::converse::{ConverseError, ConverseOutput},
    operation::converse_stream::ConverseStreamError,
    types::{
        ConverseStreamOutput, InferenceConfiguration, Message as BedrockMessage,
        SystemContentBlock, ToolConfiguration,
    },
    Client as BedrockRuntimeClient,
};
use aws_smithy_runtime_api::client::result::SdkError;
use crate::config::Settings;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;

/// Service for interacting with AWS Bedrock API.
///
/// This service wraps the AWS Bedrock Runtime SDK client and provides
/// methods for model inference, supporting both streaming and non-streaming responses.
#[derive(Clone)]
pub struct BedrockService {
    /// Application settings
    settings: Arc<Settings>,

    /// AWS Bedrock Runtime SDK client
    client: BedrockRuntimeClient,
}

impl BedrockService {
    /// Create a new Bedrock service.
    ///
    /// # Arguments
    /// * `settings` - Application settings containing AWS configuration
    /// * `client` - AWS Bedrock Runtime SDK client
    pub fn new(settings: Arc<Settings>, client: BedrockRuntimeClient) -> Self {
        Self { settings, client }
    }

    /// Get a reference to the underlying AWS SDK client
    pub fn client(&self) -> &BedrockRuntimeClient {
        &self.client
    }

    /// Get the Bedrock model ID for an Anthropic model ID
    ///
    /// This method looks up the mapping from Anthropic model IDs to Bedrock model ARNs.
    /// If no mapping exists, it returns the input as-is (assuming it's already a Bedrock ARN).
    pub fn get_bedrock_model_id(&self, anthropic_model_id: &str) -> String {
        self.settings
            .default_model_mapping
            .get(anthropic_model_id)
            .cloned()
            .unwrap_or_else(|| anthropic_model_id.to_string())
    }

    /// Check if the Bedrock service is healthy
    ///
    /// Note: There's no direct health check API for Bedrock Runtime.
    /// We return true as long as the client was created successfully.
    /// Actual connectivity will be verified on first request.
    pub fn health_check(&self) -> bool {
        // The Bedrock Runtime client doesn't have a simple health check API.
        // We consider the service healthy if the client exists.
        // Real connectivity issues will surface on actual API calls.
        true
    }

    /// Call Bedrock Converse API
    ///
    /// This is used for non-Claude models or when using the unified Converse API format.
    /// The request and response use Bedrock's Converse format.
    pub async fn converse(
        &self,
        request: ConverseRequest,
    ) -> Result<ConverseOutput, BedrockError> {
        let model_id = self.get_bedrock_model_id(&request.model_id);

        tracing::debug!(
            model_id = %model_id,
            message_count = request.messages.len(),
            "Calling Bedrock Converse API"
        );

        let mut converse_request = self
            .client
            .converse()
            .model_id(&model_id)
            .set_messages(Some(request.messages));

        // Set system prompts if provided
        if let Some(system) = request.system {
            converse_request = converse_request.set_system(Some(system));
        }

        // Set inference configuration if provided
        if let Some(inference_config) = request.inference_config {
            converse_request = converse_request.inference_config(inference_config);
        }

        // Set tool configuration if provided
        if let Some(tool_config) = request.tool_config {
            converse_request = converse_request.tool_config(tool_config);
        }

        let result = converse_request
            .send()
            .await
            .map_err(BedrockError::from_converse_error)?;

        tracing::debug!(
            stop_reason = ?result.stop_reason(),
            "Bedrock Converse API call completed"
        );

        Ok(result)
    }

    /// Call Bedrock ConverseStream API
    ///
    /// This is used for streaming responses using the Converse API format.
    /// Returns a stream of ConverseStreamOutput events from Bedrock.
    pub async fn converse_stream(
        &self,
        request: ConverseRequest,
    ) -> Result<ConverseStreamResponse, BedrockError> {
        let model_id = self.get_bedrock_model_id(&request.model_id);

        tracing::debug!(
            model_id = %model_id,
            message_count = request.messages.len(),
            "Calling Bedrock ConverseStream API"
        );

        let mut converse_request = self
            .client
            .converse_stream()
            .model_id(&model_id)
            .set_messages(Some(request.messages));

        // Set system prompts if provided
        if let Some(system) = request.system {
            converse_request = converse_request.set_system(Some(system));
        }

        // Set inference configuration if provided
        if let Some(inference_config) = request.inference_config {
            converse_request = converse_request.inference_config(inference_config);
        }

        // Set tool configuration if provided
        if let Some(tool_config) = request.tool_config {
            converse_request = converse_request.tool_config(tool_config);
        }

        // Set additional model request fields if provided
        if let Some(additional_fields) = request.additional_model_request_fields {
            converse_request = converse_request.additional_model_request_fields(additional_fields);
        }

        let result = converse_request
            .send()
            .await
            .map_err(BedrockError::from_converse_stream_error)?;

        tracing::debug!("Bedrock ConverseStream response initiated");

        Ok(ConverseStreamResponse {
            inner: result.stream,
        })
    }
}

/// Request for Bedrock Converse API
#[derive(Debug, Clone)]
pub struct ConverseRequest {
    /// Model ID (Anthropic or Bedrock format)
    pub model_id: String,

    /// Conversation messages
    pub messages: Vec<BedrockMessage>,

    /// System prompts
    pub system: Option<Vec<SystemContentBlock>>,

    /// Inference configuration (temperature, max_tokens, etc.)
    pub inference_config: Option<InferenceConfiguration>,

    /// Tool configuration for function calling
    pub tool_config: Option<ToolConfiguration>,

    /// Additional model-specific request fields (for extended thinking, etc.)
    pub additional_model_request_fields: Option<aws_smithy_types::Document>,
}

impl ConverseRequest {
    /// Create a new Converse request
    pub fn new(model_id: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            messages: Vec::new(),
            system: None,
            inference_config: None,
            tool_config: None,
            additional_model_request_fields: None,
        }
    }

    /// Add a message to the conversation
    pub fn with_message(mut self, message: BedrockMessage) -> Self {
        self.messages.push(message);
        self
    }

    /// Set messages
    pub fn with_messages(mut self, messages: Vec<BedrockMessage>) -> Self {
        self.messages = messages;
        self
    }

    /// Set system prompts
    pub fn with_system(mut self, system: Vec<SystemContentBlock>) -> Self {
        self.system = Some(system);
        self
    }

    /// Set inference configuration
    pub fn with_inference_config(mut self, config: InferenceConfiguration) -> Self {
        self.inference_config = Some(config);
        self
    }

    /// Set tool configuration
    pub fn with_tool_config(mut self, config: ToolConfiguration) -> Self {
        self.tool_config = Some(config);
        self
    }

    /// Set additional model request fields
    pub fn with_additional_fields(mut self, fields: aws_smithy_types::Document) -> Self {
        self.additional_model_request_fields = Some(fields);
        self
    }
}

// ============================================================================
// Streaming Response Types
// ============================================================================

use aws_sdk_bedrockruntime::primitives::event_stream::EventReceiver;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;

/// Wrapper for Bedrock ConverseStream response
///
/// This struct wraps the AWS SDK's EventReceiver to provide a more
/// ergonomic API for consuming streaming events.
pub struct ConverseStreamResponse {
    inner: EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
}

impl ConverseStreamResponse {
    /// Get the next event from the stream
    ///
    /// Returns `Ok(Some(event))` for each event, `Ok(None)` when the stream ends,
    /// or `Err` on error.
    pub async fn recv(&mut self) -> Result<Option<ConverseStreamOutput>, BedrockStreamError> {
        match self.inner.recv().await {
            Ok(Some(event)) => Ok(Some(event)),
            Ok(None) => Ok(None),
            Err(e) => Err(BedrockStreamError::StreamError(e.to_string())),
        }
    }

    /// Convert the stream response into an async iterator
    pub fn into_stream(
        self,
    ) -> Pin<Box<dyn Stream<Item = Result<ConverseStreamOutput, BedrockStreamError>> + Send>> {
        Box::pin(async_stream::stream! {
            let mut receiver = self.inner;
            loop {
                match receiver.recv().await {
                    Ok(Some(event)) => yield Ok(event),
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(BedrockStreamError::StreamError(e.to_string()));
                        break;
                    }
                }
            }
        })
    }
}

/// Errors that can occur during streaming
#[derive(Debug, thiserror::Error)]
pub enum BedrockStreamError {
    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Event parse error: {0}")]
    ParseError(String),
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Bedrock API calls
#[derive(Debug, thiserror::Error)]
pub enum BedrockError {
    /// Bedrock API returned an error
    #[error("Bedrock API error: {message}")]
    ApiError {
        message: String,
        error_type: BedrockErrorType,
        is_retryable: bool,
    },

    /// Request serialization failed
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Response deserialization failed
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Throttling error (rate limited)
    #[error("Throttled: {0}")]
    Throttled(String),

    /// Validation error (invalid request)
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Service unavailable
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Access denied
    #[error("Access denied: {0}")]
    AccessDenied(String),

    /// Internal service error
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Unknown error
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Type of Bedrock error for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BedrockErrorType {
    /// Client error (4xx)
    Client,
    /// Server error (5xx)
    Server,
    /// Throttling error
    Throttling,
    /// Validation error
    Validation,
    /// Unknown error type
    Unknown,
}

impl BedrockError {
    /// Create BedrockError from Converse API error
    pub fn from_converse_error<R>(err: SdkError<ConverseError, R>) -> Self
    where
        R: std::fmt::Debug,
    {
        match &err {
            SdkError::ServiceError(service_err) => {
                let error = service_err.err();
                match error {
                    ConverseError::ThrottlingException(e) => BedrockError::Throttled(
                        e.message().unwrap_or("Rate limited").to_string(),
                    ),
                    ConverseError::ValidationException(e) => BedrockError::ValidationError(
                        e.message().unwrap_or("Validation failed").to_string(),
                    ),
                    ConverseError::ModelNotReadyException(e) => BedrockError::ServiceUnavailable(
                        e.message().unwrap_or("Model not ready").to_string(),
                    ),
                    ConverseError::ModelTimeoutException(e) => BedrockError::ServiceUnavailable(
                        e.message().unwrap_or("Model timeout").to_string(),
                    ),
                    ConverseError::InternalServerException(e) => BedrockError::InternalError(
                        e.message().unwrap_or("Internal server error").to_string(),
                    ),
                    ConverseError::AccessDeniedException(e) => BedrockError::AccessDenied(
                        e.message().unwrap_or("Access denied").to_string(),
                    ),
                    ConverseError::ResourceNotFoundException(e) => BedrockError::ModelNotFound(
                        e.message().unwrap_or("Resource not found").to_string(),
                    ),
                    _ => BedrockError::Unknown(format!("{:?}", error)),
                }
            }
            _ => BedrockError::Unknown(format!("{:?}", err)),
        }
    }

    /// Create BedrockError from ConverseStream API error
    pub fn from_converse_stream_error<R>(err: SdkError<ConverseStreamError, R>) -> Self
    where
        R: std::fmt::Debug,
    {
        match &err {
            SdkError::ServiceError(service_err) => {
                let error = service_err.err();
                match error {
                    ConverseStreamError::ThrottlingException(e) => BedrockError::Throttled(
                        e.message().unwrap_or("Rate limited").to_string(),
                    ),
                    ConverseStreamError::ValidationException(e) => BedrockError::ValidationError(
                        e.message().unwrap_or("Validation failed").to_string(),
                    ),
                    ConverseStreamError::ModelNotReadyException(e) => BedrockError::ServiceUnavailable(
                        e.message().unwrap_or("Model not ready").to_string(),
                    ),
                    ConverseStreamError::ModelTimeoutException(e) => BedrockError::ServiceUnavailable(
                        e.message().unwrap_or("Model timeout").to_string(),
                    ),
                    ConverseStreamError::InternalServerException(e) => BedrockError::InternalError(
                        e.message().unwrap_or("Internal server error").to_string(),
                    ),
                    ConverseStreamError::AccessDeniedException(e) => BedrockError::AccessDenied(
                        e.message().unwrap_or("Access denied").to_string(),
                    ),
                    ConverseStreamError::ResourceNotFoundException(e) => BedrockError::ModelNotFound(
                        e.message().unwrap_or("Resource not found").to_string(),
                    ),
                    ConverseStreamError::ServiceUnavailableException(e) => {
                        BedrockError::ServiceUnavailable(
                            e.message().unwrap_or("Service unavailable").to_string(),
                        )
                    }
                    ConverseStreamError::ModelErrorException(e) => BedrockError::ApiError {
                        message: e.message().unwrap_or("Model error").to_string(),
                        error_type: BedrockErrorType::Server,
                        is_retryable: true,
                    },
                    _ => BedrockError::Unknown(format!("{:?}", error)),
                }
            }
            _ => BedrockError::Unknown(format!("{:?}", err)),
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            BedrockError::Throttled(_)
                | BedrockError::ServiceUnavailable(_)
                | BedrockError::InternalError(_)
                | BedrockError::ApiError { is_retryable: true, .. }
        )
    }

    /// Get the error type for categorization
    pub fn error_type(&self) -> BedrockErrorType {
        match self {
            BedrockError::Throttled(_) => BedrockErrorType::Throttling,
            BedrockError::ValidationError(_) => BedrockErrorType::Validation,
            BedrockError::ModelNotFound(_)
            | BedrockError::AccessDenied(_)
            | BedrockError::Serialization(_)
            | BedrockError::Deserialization(_) => BedrockErrorType::Client,
            BedrockError::ServiceUnavailable(_) | BedrockError::InternalError(_) => {
                BedrockErrorType::Server
            }
            BedrockError::ApiError { error_type, .. } => *error_type,
            BedrockError::Unknown(_) => BedrockErrorType::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{ContentBlock as BedrockContentBlock, ConversationRole};

    #[test]
    fn test_bedrock_error_is_retryable() {
        assert!(BedrockError::Throttled("test".to_string()).is_retryable());
        assert!(BedrockError::ServiceUnavailable("test".to_string()).is_retryable());
        assert!(BedrockError::InternalError("test".to_string()).is_retryable());
        assert!(!BedrockError::ValidationError("test".to_string()).is_retryable());
        assert!(!BedrockError::AccessDenied("test".to_string()).is_retryable());
    }

    #[test]
    fn test_converse_request_builder() {
        let request = ConverseRequest::new("claude-3-sonnet")
            .with_inference_config(
                InferenceConfiguration::builder()
                    .max_tokens(1024)
                    .temperature(0.7_f32)
                    .build(),
            );

        assert_eq!(request.model_id, "claude-3-sonnet");
        assert!(request.inference_config.is_some());
    }

    #[test]
    fn test_converse_request_with_messages() {
        let message = BedrockMessage::builder()
            .role(ConversationRole::User)
            .content(BedrockContentBlock::Text("Hello".to_string()))
            .build()
            .unwrap();

        let request = ConverseRequest::new("claude-3-sonnet")
            .with_messages(vec![message]);

        assert_eq!(request.messages.len(), 1);
    }
}
