//! Anthropic Messages API schema definitions
//!
//! This module contains Rust equivalents of the Anthropic Messages API request
//! and response structures, enabling validation, serialization, and type safety.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Cache Control
// ============================================================================

/// Cache control for prompt caching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String, // "ephemeral"
}

impl Default for CacheControl {
    fn default() -> Self {
        Self {
            cache_type: "ephemeral".to_string(),
        }
    }
}

// ============================================================================
// Content Block Types
// ============================================================================

/// Text content block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl TextContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
            cache_control: None,
        }
    }
}

/// Image source data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String,  // "image/jpeg", "image/png", "image/gif", "image/webp"
    pub data: String,        // base64 encoded
}

/// Image content block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "image"
    pub source: ImageSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Document source data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String,  // "application/pdf"
    pub data: String,        // base64 encoded
}

/// Document content block (PDF support).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "document"
    pub source: DocumentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Extended thinking content block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "thinking"
    pub thinking: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Redacted thinking content block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedactedThinkingContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "redacted_thinking"
    pub data: String,         // Base64 encoded redacted data
}

/// Information about who invoked a tool (for PTC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallerInfo {
    #[serde(rename = "type")]
    pub caller_type: String, // "direct" or "code_execution_20250825"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_id: Option<String>,
}

/// Tool use content block in assistant messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolUseContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "tool_use"
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<CallerInfo>, // PTC: who called the tool
}

/// Server tool use content block (e.g., code_execution for PTC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerToolUseContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "server_tool_use"
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Content block for code execution result (PTC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeExecutionResultContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "code_execution_result"
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub return_code: i32,
}

/// Server tool result content block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerToolResultContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "server_tool_result"
    pub tool_use_id: String,
    pub content: Vec<serde_json::Value>, // Can contain various result types
}

/// Tool result content block in user messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResultContent {
    #[serde(rename = "type")]
    pub content_type: String, // Always "tool_result"
    pub tool_use_id: String,
    pub content: ToolResultValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Tool result value - can be string or list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolResultValue {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Union of all content block types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "document")]
    Document {
        source: DocumentSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        caller: Option<CallerInfo>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultValue,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "server_tool_result")]
    ServerToolResult {
        tool_use_id: String,
        content: Vec<serde_json::Value>,
    },
}

impl ContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    /// Check if this is a text block.
    pub fn is_text(&self) -> bool {
        matches!(self, ContentBlock::Text { .. })
    }

    /// Check if this is a tool use block.
    pub fn is_tool_use(&self) -> bool {
        matches!(self, ContentBlock::ToolUse { .. })
    }

    /// Get text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

// ============================================================================
// Message Structure
// ============================================================================

/// Message content - can be string or list of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Convert to list of content blocks.
    pub fn into_blocks(self) -> Vec<ContentBlock> {
        match self {
            MessageContent::Text(text) => vec![ContentBlock::text(text)],
            MessageContent::Blocks(blocks) => blocks,
        }
    }

    /// Get as text if this is a simple text message.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// Message in the conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: MessageContent,
}

impl Message {
    /// Create a user message with text content.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: MessageContent::Text(content.into()),
        }
    }

    /// Create an assistant message with text content.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: MessageContent::Text(content.into()),
        }
    }

    /// Create a message with content blocks.
    pub fn with_blocks(role: impl Into<String>, blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: role.into(),
            content: MessageContent::Blocks(blocks),
        }
    }
}

// ============================================================================
// Tool Definitions
// ============================================================================

/// JSON schema for tool input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String, // Usually "object"
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

impl Default for ToolInputSchema {
    fn default() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: None,
        }
    }
}

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Input examples (beta feature)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_examples: Option<Vec<serde_json::Value>>,
    /// PTC-specific: tool type
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    /// PTC-specific: allowed callers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<Vec<String>>,
}

/// Code execution tool for PTC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeExecutionTool {
    #[serde(rename = "type")]
    pub tool_type: String, // "code_execution_20250825"
    pub name: String,      // "code_execution"
}

impl Default for CodeExecutionTool {
    fn default() -> Self {
        Self {
            tool_type: "code_execution_20250825".to_string(),
            name: "code_execution".to_string(),
        }
    }
}

/// Tool choice configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto(String),                      // "auto" or "any"
    Specific { name: String },         // {"type": "tool", "name": "tool_name"}
    Object(serde_json::Value),         // Generic object form
}

// ============================================================================
// System Message
// ============================================================================

/// System message with optional cache control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemMessage {
    #[serde(rename = "type")]
    pub message_type: String, // "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            message_type: "text".to_string(),
            text: text.into(),
            cache_control: None,
        }
    }
}

/// System content - can be string or list of system messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SystemContent {
    Text(String),
    Messages(Vec<SystemMessage>),
}

impl SystemContent {
    /// Convert to list of system messages.
    pub fn into_messages(self) -> Vec<SystemMessage> {
        match self {
            SystemContent::Text(text) => vec![SystemMessage::new(text)],
            SystemContent::Messages(messages) => messages,
        }
    }
}

// ============================================================================
// Request Metadata
// ============================================================================

/// Request metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// ============================================================================
// Thinking Configuration
// ============================================================================

/// Extended thinking configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String, // "enabled"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<i32>,
}

// ============================================================================
// Request Models
// ============================================================================

/// Anthropic Messages API request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i32,

    // Optional parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub stream: bool,

    // Tool use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>, // Can include Tool or CodeExecutionTool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    // Extended thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,

    // Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,

    // PTC container for session reuse
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
}

fn default_max_tokens() -> i32 {
    4096
}

impl MessageRequest {
    /// Create a new message request with required fields.
    pub fn new(model: impl Into<String>, messages: Vec<Message>, max_tokens: i32) -> Self {
        Self {
            model: model.into(),
            messages,
            max_tokens,
            system: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            stream: false,
            tools: None,
            tool_choice: None,
            thinking: None,
            metadata: None,
            container: None,
        }
    }

    /// Set system prompt.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(SystemContent::Text(system.into()));
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Enable streaming.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

// ============================================================================
// Response Models
// ============================================================================

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
}

impl Usage {
    pub fn new(input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }
    }
}

/// Stop reason enumeration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::EndTurn => write!(f, "end_turn"),
            StopReason::MaxTokens => write!(f, "max_tokens"),
            StopReason::StopSequence => write!(f, "stop_sequence"),
            StopReason::ToolUse => write!(f, "tool_use"),
        }
    }
}

/// Anthropic Messages API response (non-streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String, // "message"
    pub role: String,          // "assistant"
    pub content: Vec<ContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

impl MessageResponse {
    /// Create a new message response.
    pub fn new(
        id: impl Into<String>,
        model: impl Into<String>,
        content: Vec<ContentBlock>,
        usage: Usage,
    ) -> Self {
        Self {
            id: id.into(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model: model.into(),
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage,
        }
    }

    /// Set stop reason.
    pub fn with_stop_reason(mut self, reason: StopReason) -> Self {
        self.stop_reason = Some(reason);
        self
    }
}

// ============================================================================
// Streaming Event Models
// ============================================================================

/// Stream event: message_start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "message_start"
    pub message: serde_json::Value, // Partial message
}

/// Stream event: content_block_start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockStartEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "content_block_start"
    pub index: i32,
    pub content_block: serde_json::Value,
}

/// Stream event: content_block_delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "content_block_delta"
    pub index: i32,
    pub delta: serde_json::Value,
}

/// Stream event: content_block_stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockStopEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "content_block_stop"
    pub index: i32,
}

/// Stream event: message_delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "message_delta"
    pub delta: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<serde_json::Value>,
}

/// Stream event: message_stop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStopEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "message_stop"
}

/// Stream event: ping (keep-alive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "ping"
}

/// Stream event: error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    #[serde(rename = "type")]
    pub event_type: String, // "error"
    pub error: serde_json::Value,
}

/// Union of all stream events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: serde_json::Value },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: i32,
        content_block: serde_json::Value,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: i32,
        delta: serde_json::Value,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: i32 },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<serde_json::Value>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: serde_json::Value },
}

// ============================================================================
// Error Response
// ============================================================================

/// Error detail structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Error response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub response_type: String, // "error"
    pub error: ErrorDetail,
}

impl ErrorResponse {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            response_type: "error".to_string(),
            error: ErrorDetail {
                error_type: error_type.into(),
                message: message.into(),
            },
        }
    }
}

// ============================================================================
// Count Tokens Models
// ============================================================================

/// Request to count tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountTokensRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

/// Response with token count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountTokensResponse {
    pub input_tokens: i32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_content_serialization() {
        let content = ContentBlock::text("Hello, world!");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello, world!\""));
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, "user");
        assert!(matches!(msg.content, MessageContent::Text(ref t) if t == "Hello"));
    }

    #[test]
    fn test_message_request_builder() {
        let request = MessageRequest::new(
            "claude-3-sonnet",
            vec![Message::user("Hello")],
            1024,
        )
        .with_system("You are a helpful assistant")
        .with_temperature(0.7)
        .with_stream(true);

        assert_eq!(request.model, "claude-3-sonnet");
        assert_eq!(request.max_tokens, 1024);
        assert!(request.system.is_some());
        assert_eq!(request.temperature, Some(0.7));
        assert!(request.stream);
    }

    #[test]
    fn test_message_response_serialization() {
        let response = MessageResponse::new(
            "msg_123",
            "claude-3-sonnet",
            vec![ContentBlock::text("Hello!")],
            Usage::new(10, 5),
        );

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":\"msg_123\""));
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("\"role\":\"assistant\""));
    }

    #[test]
    fn test_content_block_deserialization() {
        let json = r#"{"type":"text","text":"Hello"}"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(block.is_text());
        assert_eq!(block.as_text(), Some("Hello"));
    }

    #[test]
    fn test_tool_use_content_deserialization() {
        let json = r#"{
            "type": "tool_use",
            "id": "tool_123",
            "name": "get_weather",
            "input": {"location": "San Francisco"}
        }"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(block.is_tool_use());
    }

    #[test]
    fn test_message_content_conversion() {
        let content = MessageContent::Text("Hello".to_string());
        let blocks = content.into_blocks();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].is_text());
    }

    #[test]
    fn test_system_content_conversion() {
        let system = SystemContent::Text("Be helpful".to_string());
        let messages = system.into_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Be helpful");
    }

    #[test]
    fn test_error_response() {
        let error = ErrorResponse::new("invalid_request", "Missing required field");
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"type\":\"invalid_request\""));
    }

    #[test]
    fn test_stop_reason_display() {
        assert_eq!(StopReason::EndTurn.to_string(), "end_turn");
        assert_eq!(StopReason::ToolUse.to_string(), "tool_use");
    }
}
