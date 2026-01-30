//! OpenAI API schema definitions
//!
//! This module defines the request and response types for OpenAI Chat Completions API
//! compatibility layer.

use serde::{Deserialize, Serialize};

// ============================================================================
// Request Types
// ============================================================================

/// OpenAI Chat Completion Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    /// Model ID (e.g., "gpt-4", "gpt-4o", "gpt-3.5-turbo")
    pub model: String,

    /// Messages in the conversation
    pub messages: Vec<ChatMessage>,

    /// Sampling temperature (0.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,

    /// Alternative to max_tokens for newer API versions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i32>,

    /// Whether to stream the response
    #[serde(default)]
    pub stream: bool,

    /// Stream options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,

    /// Top-p sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopSequence>,

    /// Presence penalty (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty (-2.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Tools available to the model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// Seed for deterministic sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Number of completions to generate (only n=1 supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,

    /// Log probabilities (not supported, ignored)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    /// Top log probabilities (not supported, ignored)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i32>,
}

/// Stream options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    /// Include usage in stream response
    #[serde(default)]
    pub include_usage: bool,
}

/// Stop sequence - can be string or array of strings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}

impl StopSequence {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            StopSequence::Single(s) => vec![s.clone()],
            StopSequence::Multiple(v) => v.clone(),
        }
    }
}

/// Chat message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender
    pub role: ChatRole,

    /// Message content (string or array of content parts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,

    /// Name of the participant (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool calls made by the assistant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Tool call ID (for tool role messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Message content - can be string or array of content parts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Convert to string, joining parts if necessary
    pub fn to_string_content(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }
}

/// Content part for multimodal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text content
    Text { text: String },

    /// Image URL content
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// Image URL specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// URL of the image (can be data URL with base64)
    pub url: String,

    /// Detail level for image processing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// ============================================================================
// Tool Types
// ============================================================================

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Type of tool (always "function")
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function definition
    pub function: FunctionDef,
}

/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    /// Name of the function
    pub name: String,

    /// Description of the function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Parameters schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,

    /// Whether the function should be called strictly according to schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Tool choice specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// String mode: "none", "auto", "required"
    Mode(String),

    /// Specific function choice
    Function {
        #[serde(rename = "type")]
        choice_type: String,
        function: ToolChoiceFunction,
    },
}

/// Specific function to call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

/// Tool call in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID of the tool call
    pub id: String,

    /// Type of tool (always "function")
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function call details
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function
    pub name: String,

    /// Arguments as JSON string
    pub arguments: String,
}

// ============================================================================
// Response Format
// ============================================================================

/// Response format specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Type: "text" or "json_object"
    #[serde(rename = "type")]
    pub format_type: String,
}

// ============================================================================
// Response Types
// ============================================================================

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// Unique identifier for the completion
    pub id: String,

    /// Object type (always "chat.completion")
    pub object: String,

    /// Unix timestamp of creation
    pub created: i64,

    /// Model used
    pub model: String,

    /// Completion choices
    pub choices: Vec<Choice>,

    /// Token usage statistics
    pub usage: CompletionUsage,

    /// System fingerprint (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// Index of this choice
    pub index: i32,

    /// The generated message
    pub message: AssistantMessage,

    /// Reason for stopping
    pub finish_reason: Option<String>,

    /// Log probabilities (not supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Assistant message in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// Role (always "assistant")
    pub role: ChatRole,

    /// Text content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionUsage {
    /// Tokens in the prompt
    pub prompt_tokens: i32,

    /// Tokens in the completion
    pub completion_tokens: i32,

    /// Total tokens used
    pub total_tokens: i32,

    /// Detailed completion token breakdown (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,
}

/// Detailed completion token breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i32>,
}

// ============================================================================
// Streaming Types
// ============================================================================

/// Streaming chunk response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    /// Unique identifier
    pub id: String,

    /// Object type (always "chat.completion.chunk")
    pub object: String,

    /// Unix timestamp
    pub created: i64,

    /// Model used
    pub model: String,

    /// Choices with deltas
    pub choices: Vec<ChunkChoice>,

    /// System fingerprint (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,

    /// Usage (only in final chunk if stream_options.include_usage is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CompletionUsage>,
}

/// Streaming choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    /// Index of this choice
    pub index: i32,

    /// Delta content
    pub delta: ChunkDelta,

    /// Finish reason (only in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Log probabilities (not supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Delta content in streaming
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkDelta {
    /// Role (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<ChatRole>,

    /// Text content delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

/// Tool call delta in streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    /// Index of the tool call
    pub index: i32,

    /// Tool call ID (only in first chunk for this tool)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Type (only in first chunk)
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,

    /// Function delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionCallDelta>,
}

/// Function call delta in streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    /// Function name (only in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Arguments delta
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ============================================================================
// Models API Types
// ============================================================================

/// List models response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    /// Object type (always "list")
    pub object: String,

    /// Available models
    pub data: Vec<Model>,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Model ID
    pub id: String,

    /// Object type (always "model")
    pub object: String,

    /// Unix timestamp of creation
    pub created: i64,

    /// Owner of the model
    pub owned_by: String,
}

// ============================================================================
// Error Types
// ============================================================================

/// OpenAI-style error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIError,
}

/// OpenAI error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIError {
    /// Error message
    pub message: String,

    /// Error type
    #[serde(rename = "type")]
    pub error_type: String,

    /// Parameter that caused the error (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,

    /// Error code (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl OpenAIErrorResponse {
    pub fn new(error_type: &str, message: &str) -> Self {
        Self {
            error: OpenAIError {
                message: message.to_string(),
                error_type: error_type.to_string(),
                param: None,
                code: None,
            },
        }
    }

    pub fn with_code(error_type: &str, message: &str, code: &str) -> Self {
        Self {
            error: OpenAIError {
                message: message.to_string(),
                error_type: error_type.to_string(),
                param: None,
                code: Some(code.to_string()),
            },
        }
    }

    pub fn invalid_request(message: &str) -> Self {
        Self::new("invalid_request_error", message)
    }

    pub fn authentication_error(message: &str) -> Self {
        Self::new("authentication_error", message)
    }

    pub fn rate_limit_error(message: &str) -> Self {
        Self::new("rate_limit_error", message)
    }

    pub fn server_error(message: &str) -> Self {
        Self::new("server_error", message)
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Generate a unique completion ID
pub fn generate_completion_id() -> String {
    format!("chatcmpl-{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..24].to_string())
}

/// Get current Unix timestamp
pub fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_content_text() {
        let content: MessageContent = serde_json::from_str(r#""Hello, world!""#).unwrap();
        assert_eq!(content.to_string_content(), "Hello, world!");
    }

    #[test]
    fn test_message_content_parts() {
        let content: MessageContent = serde_json::from_str(
            r#"[{"type": "text", "text": "Hello"}, {"type": "text", "text": "World"}]"#,
        )
        .unwrap();
        assert_eq!(content.to_string_content(), "Hello\nWorld");
    }

    #[test]
    fn test_stop_sequence_single() {
        let stop: StopSequence = serde_json::from_str(r#""stop""#).unwrap();
        assert_eq!(stop.to_vec(), vec!["stop"]);
    }

    #[test]
    fn test_stop_sequence_multiple() {
        let stop: StopSequence = serde_json::from_str(r#"["stop1", "stop2"]"#).unwrap();
        assert_eq!(stop.to_vec(), vec!["stop1", "stop2"]);
    }

    #[test]
    fn test_chat_role_serialization() {
        assert_eq!(serde_json::to_string(&ChatRole::System).unwrap(), r#""system""#);
        assert_eq!(serde_json::to_string(&ChatRole::User).unwrap(), r#""user""#);
        assert_eq!(serde_json::to_string(&ChatRole::Assistant).unwrap(), r#""assistant""#);
        assert_eq!(serde_json::to_string(&ChatRole::Tool).unwrap(), r#""tool""#);
    }

    #[test]
    fn test_tool_choice_mode() {
        let choice: ToolChoice = serde_json::from_str(r#""auto""#).unwrap();
        matches!(choice, ToolChoice::Mode(s) if s == "auto");
    }

    #[test]
    fn test_tool_choice_function() {
        let choice: ToolChoice = serde_json::from_str(
            r#"{"type": "function", "function": {"name": "my_func"}}"#,
        )
        .unwrap();
        matches!(choice, ToolChoice::Function { .. });
    }

    #[test]
    fn test_error_response() {
        let err = OpenAIErrorResponse::invalid_request("Invalid model");
        assert_eq!(err.error.error_type, "invalid_request_error");
        assert_eq!(err.error.message, "Invalid model");
    }

    #[test]
    fn test_generate_completion_id() {
        let id = generate_completion_id();
        assert!(id.starts_with("chatcmpl-"));
        assert_eq!(id.len(), 33); // "chatcmpl-" (9) + 24 chars
    }
}
