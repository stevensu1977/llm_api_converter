//! Unified LLM Provider trait and types.
//!
//! Defines a provider-agnostic interface for LLM backends, enabling the system
//! to route requests to different providers (Bedrock, Gemini, OpenAI, etc.)
//! through a common API.

use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

// ============================================================================
// Unified Request Types
// ============================================================================

/// Provider-agnostic chat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatRequest {
    pub model: String,
    pub messages: Vec<UnifiedMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<UnifiedTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

/// A message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMessage {
    pub role: String,
    pub content: UnifiedContent,
}

/// Message content — simple text or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UnifiedContent {
    Text(String),
    Blocks(Vec<UnifiedContentBlock>),
}

/// A single content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UnifiedContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageData },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Image data for multimodal requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub media_type: String,
    pub data: String,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

// ============================================================================
// Unified Response Types
// ============================================================================

/// Provider-agnostic chat response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedChatResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<UnifiedContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: UnifiedUsage,
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnifiedUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<i64>,
}

// ============================================================================
// Streaming Types
// ============================================================================

/// A single event in a streaming response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    ContentDelta { text: String },
    ToolUseDelta { id: String, name: Option<String>, input_json: Option<String> },
    Stop { reason: String },
    Usage(UnifiedUsage),
    Error(String),
}

/// Boxed stream of streaming events.
pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;

// ============================================================================
// Error Types
// ============================================================================

/// Errors from LLM providers.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("API error ({code}): {message}")]
    Api { code: i32, message: String },

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("No available credentials")]
    NoCredentials,

    #[error("Timeout")]
    Timeout,

    #[error("Internal: {0}")]
    Internal(String),
}

// ============================================================================
// Provider Trait
// ============================================================================

/// The core trait that all LLM providers must implement.
///
/// This enables the system to route requests to different backends
/// (Bedrock, Gemini, OpenAI, DeepSeek, etc.) through a unified interface.
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Provider name (e.g., "bedrock", "gemini", "openai").
    fn name(&self) -> &str;

    /// Model ID patterns this provider supports (e.g., ["us.anthropic.*", "gemini-*"]).
    fn supported_model_patterns(&self) -> Vec<String>;

    /// Check if this provider supports a specific model.
    fn supports_model(&self, model: &str) -> bool;

    /// Non-streaming chat completion.
    async fn chat(&self, request: UnifiedChatRequest) -> Result<UnifiedChatResponse, ProviderError>;

    /// Streaming chat completion.
    async fn chat_stream(&self, request: UnifiedChatRequest) -> Result<StreamResult, ProviderError>;

    /// Health check.
    fn health_check(&self) -> bool;
}

/// Check if a model name matches a glob-like pattern (supports `*` wildcard).
pub fn model_matches_pattern(model: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    model == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_matches_exact() {
        assert!(model_matches_pattern("gpt-4o", "gpt-4o"));
        assert!(!model_matches_pattern("gpt-4o", "gpt-4"));
    }

    #[test]
    fn test_model_matches_wildcard() {
        assert!(model_matches_pattern("gemini-2.0-flash", "gemini-*"));
        assert!(model_matches_pattern("us.anthropic.claude-sonnet-4-20250514-v1:0", "us.anthropic.*"));
        assert!(!model_matches_pattern("gpt-4o", "gemini-*"));
    }

    #[test]
    fn test_model_matches_star() {
        assert!(model_matches_pattern("anything", "*"));
    }

    #[test]
    fn test_unified_content_serde() {
        let text = UnifiedContent::Text("hello".into());
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, "\"hello\"");

        let blocks = UnifiedContent::Blocks(vec![UnifiedContentBlock::Text {
            text: "hello".into(),
        }]);
        let json = serde_json::to_string(&blocks).unwrap();
        assert!(json.contains("\"type\":\"text\""));
    }
}
