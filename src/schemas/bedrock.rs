//! AWS Bedrock Converse API schema definitions
//!
//! This module contains Rust equivalents of the Bedrock Converse API request
//! and response structures, enabling validation, serialization, and type safety.
//!
//! Note: These are simplified models focused on the conversion needs.

use serde::{Deserialize, Serialize};

// ============================================================================
// Content Block Types for Bedrock
// ============================================================================

/// Text content in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockTextContent {
    pub text: String,
}

/// Image source in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockImageSource {
    pub bytes: Vec<u8>, // Raw bytes (will be base64 decoded from Anthropic format)
}

/// Image content in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockImageContent {
    pub image: BedrockImageData,
}

/// Image data with format and source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockImageData {
    pub format: String, // "png", "jpeg", "gif", "webp"
    pub source: BedrockImageSource,
}

/// Document source in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockDocumentSource {
    pub bytes: Vec<u8>,
}

/// Document content in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockDocumentContent {
    pub document: BedrockDocumentData,
}

/// Document data with format, name, and source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockDocumentData {
    pub format: String, // "pdf"
    pub name: String,
    pub source: BedrockDocumentSource,
}

/// Tool use content in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolUseContent {
    #[serde(rename = "toolUse")]
    pub tool_use: BedrockToolUseData,
}

/// Tool use data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolUseData {
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Tool result content in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolResultContent {
    #[serde(rename = "toolResult")]
    pub tool_result: BedrockToolResultData,
}

/// Tool result data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolResultData {
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
    pub content: Vec<serde_json::Value>, // Content blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>, // "success" or "error"
}

/// Union of Bedrock content blocks (using serde_json::Value for flexibility).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BedrockContentBlock {
    Text { text: String },
    Image { image: BedrockImageData },
    Document { document: BedrockDocumentData },
    ToolUse {
        #[serde(rename = "toolUse")]
        tool_use: BedrockToolUseData,
    },
    ToolResult {
        #[serde(rename = "toolResult")]
        tool_result: BedrockToolResultData,
    },
}

impl BedrockContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        BedrockContentBlock::Text { text: text.into() }
    }

    /// Check if this is a text block.
    pub fn is_text(&self) -> bool {
        matches!(self, BedrockContentBlock::Text { .. })
    }

    /// Get text content if this is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            BedrockContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }
}

// ============================================================================
// Message Structure for Bedrock
// ============================================================================

/// Message in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockMessage {
    pub role: String, // "user" or "assistant"
    pub content: Vec<BedrockContentBlock>,
}

impl BedrockMessage {
    /// Create a user message with text content.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![BedrockContentBlock::text(text)],
        }
    }

    /// Create an assistant message with text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![BedrockContentBlock::text(text)],
        }
    }
}

// ============================================================================
// Tool Configuration for Bedrock
// ============================================================================

/// Tool input schema in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolInputSchema {
    pub json: serde_json::Value, // JSON schema
}

/// Tool specification in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolSpec {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: BedrockToolInputSchema,
}

/// Tool definition in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockTool {
    #[serde(rename = "toolSpec")]
    pub tool_spec: BedrockToolSpec,
}

/// Tool choice in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BedrockToolChoice {
    Auto {
        auto: serde_json::Value, // {}
    },
    Any {
        any: serde_json::Value, // {}
    },
    Tool {
        tool: BedrockToolChoiceTool,
    },
}

/// Specific tool choice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolChoiceTool {
    pub name: String,
}

/// Tool configuration for Bedrock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockToolConfig {
    pub tools: Vec<BedrockTool>,
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<BedrockToolChoice>,
}

// ============================================================================
// Inference Configuration
// ============================================================================

/// Inference configuration for Bedrock.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockInferenceConfig {
    #[serde(rename = "maxTokens")]
    pub max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(rename = "stopSequences", skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl BedrockInferenceConfig {
    /// Create a new inference config with max tokens.
    pub fn new(max_tokens: i32) -> Self {
        Self {
            max_tokens,
            temperature: None,
            top_p: None,
            stop_sequences: None,
        }
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set top_p.
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set stop sequences.
    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

// ============================================================================
// System Message for Bedrock
// ============================================================================

/// System message in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockSystemMessage {
    pub text: String,
}

impl BedrockSystemMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

// ============================================================================
// Request Model
// ============================================================================

/// Bedrock Converse API request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConverseRequest {
    #[serde(rename = "modelId")]
    pub model_id: String,
    pub messages: Vec<BedrockMessage>,
    #[serde(rename = "inferenceConfig")]
    pub inference_config: BedrockInferenceConfig,

    // Optional parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<BedrockSystemMessage>>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<BedrockToolConfig>,
    #[serde(rename = "additionalModelRequestFields", skip_serializing_if = "Option::is_none")]
    pub additional_model_request_fields: Option<serde_json::Value>,
}

impl BedrockConverseRequest {
    /// Create a new Bedrock request with required fields.
    pub fn new(
        model_id: impl Into<String>,
        messages: Vec<BedrockMessage>,
        max_tokens: i32,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            messages,
            inference_config: BedrockInferenceConfig::new(max_tokens),
            system: None,
            tool_config: None,
            additional_model_request_fields: None,
        }
    }

    /// Set system messages.
    pub fn with_system(mut self, system: Vec<BedrockSystemMessage>) -> Self {
        self.system = Some(system);
        self
    }

    /// Set tool configuration.
    pub fn with_tools(mut self, tool_config: BedrockToolConfig) -> Self {
        self.tool_config = Some(tool_config);
        self
    }

    /// Set additional model request fields.
    pub fn with_additional_fields(mut self, fields: serde_json::Value) -> Self {
        self.additional_model_request_fields = Some(fields);
        self
    }
}

// ============================================================================
// Response Models
// ============================================================================

/// Token usage in Bedrock format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockTokenUsage {
    #[serde(rename = "inputTokens")]
    pub input_tokens: i32,
    #[serde(rename = "outputTokens")]
    pub output_tokens: i32,
    #[serde(rename = "totalTokens")]
    pub total_tokens: i32,
}

impl BedrockTokenUsage {
    pub fn new(input_tokens: i32, output_tokens: i32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        }
    }
}

/// Output message in Bedrock response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockOutputMessage {
    pub role: String,
    pub content: Vec<BedrockContentBlock>,
}

/// Output wrapper in Bedrock response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockOutput {
    pub message: BedrockOutputMessage,
}

/// Bedrock Converse API response (non-streaming).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockConverseResponse {
    pub output: BedrockOutput,
    #[serde(rename = "stopReason")]
    pub stop_reason: String, // "end_turn", "max_tokens", "stop_sequence", "tool_use", "content_filtered"
    pub usage: BedrockTokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<BedrockMetrics>,
}

/// Performance metrics in Bedrock response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BedrockMetrics {
    #[serde(rename = "latencyMs")]
    pub latency_ms: i64,
}

// ============================================================================
// Streaming Event Models
// ============================================================================

/// Bedrock stream event: messageStart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockMessageStartEvent {
    pub role: String, // "assistant"
}

/// Bedrock stream event: contentBlockStart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentBlockStartEvent {
    pub start: serde_json::Value, // {"toolUse": {"toolUseId": "...", "name": "..."}}
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: i32,
}

/// Bedrock stream event: contentBlockDelta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentBlockDeltaEvent {
    pub delta: serde_json::Value, // {"text": "..."} or {"toolUse": {"input": "..."}}
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: i32,
}

/// Bedrock stream event: contentBlockStop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockContentBlockStopEvent {
    #[serde(rename = "contentBlockIndex")]
    pub content_block_index: i32,
}

/// Bedrock stream event: messageStop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockMessageStopEvent {
    #[serde(rename = "stopReason")]
    pub stop_reason: String,
    #[serde(rename = "additionalModelResponseFields", skip_serializing_if = "Option::is_none")]
    pub additional_model_response_fields: Option<serde_json::Value>,
}

/// Bedrock stream event: metadata (usage information).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockMetadataEvent {
    pub usage: BedrockTokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<BedrockMetrics>,
}

/// Union of Bedrock streaming events.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BedrockStreamEvent {
    MessageStart {
        #[serde(rename = "messageStart")]
        message_start: BedrockMessageStartEvent,
    },
    ContentBlockStart {
        #[serde(rename = "contentBlockStart")]
        content_block_start: BedrockContentBlockStartEvent,
    },
    ContentBlockDelta {
        #[serde(rename = "contentBlockDelta")]
        content_block_delta: BedrockContentBlockDeltaEvent,
    },
    ContentBlockStop {
        #[serde(rename = "contentBlockStop")]
        content_block_stop: BedrockContentBlockStopEvent,
    },
    MessageStop {
        #[serde(rename = "messageStop")]
        message_stop: BedrockMessageStopEvent,
    },
    Metadata {
        metadata: BedrockMetadataEvent,
    },
}

// ============================================================================
// Model Information
// ============================================================================

/// Information about a Bedrock model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockModelSummary {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    #[serde(rename = "providerName")]
    pub provider_name: String,
    #[serde(rename = "inputModalities")]
    pub input_modalities: Vec<String>,
    #[serde(rename = "outputModalities")]
    pub output_modalities: Vec<String>,
    #[serde(rename = "responseStreamingSupported")]
    pub response_streaming_supported: bool,
    #[serde(rename = "customizationsSupported", skip_serializing_if = "Option::is_none")]
    pub customizations_supported: Option<Vec<String>>,
}

// ============================================================================
// Stop Reason Mapping
// ============================================================================

/// Bedrock stop reason enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockStopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    ContentFiltered,
    Unknown(String),
}

impl BedrockStopReason {
    /// Parse from Bedrock stop reason string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "end_turn" => BedrockStopReason::EndTurn,
            "max_tokens" => BedrockStopReason::MaxTokens,
            "stop_sequence" => BedrockStopReason::StopSequence,
            "tool_use" => BedrockStopReason::ToolUse,
            "content_filtered" => BedrockStopReason::ContentFiltered,
            other => BedrockStopReason::Unknown(other.to_string()),
        }
    }

    /// Convert to Anthropic stop reason string.
    pub fn to_anthropic_string(&self) -> &str {
        match self {
            BedrockStopReason::EndTurn => "end_turn",
            BedrockStopReason::MaxTokens => "max_tokens",
            BedrockStopReason::StopSequence => "stop_sequence",
            BedrockStopReason::ToolUse => "tool_use",
            BedrockStopReason::ContentFiltered => "end_turn", // Map to end_turn
            BedrockStopReason::Unknown(_) => "end_turn",
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
    fn test_bedrock_text_content() {
        let content = BedrockContentBlock::text("Hello");
        assert!(content.is_text());
        assert_eq!(content.as_text(), Some("Hello"));
    }

    #[test]
    fn test_bedrock_message_creation() {
        let msg = BedrockMessage::user("Hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.len(), 1);
        assert!(msg.content[0].is_text());
    }

    #[test]
    fn test_bedrock_inference_config() {
        let config = BedrockInferenceConfig::new(1024)
            .with_temperature(0.7)
            .with_top_p(0.9)
            .with_stop_sequences(vec!["STOP".to_string()]);

        assert_eq!(config.max_tokens, 1024);
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.top_p, Some(0.9));
        assert!(config.stop_sequences.is_some());
    }

    #[test]
    fn test_bedrock_request_serialization() {
        let request = BedrockConverseRequest::new(
            "anthropic.claude-3-sonnet",
            vec![BedrockMessage::user("Hello")],
            1024,
        );

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"modelId\":\"anthropic.claude-3-sonnet\""));
        assert!(json.contains("\"maxTokens\":1024"));
    }

    #[test]
    fn test_bedrock_token_usage() {
        let usage = BedrockTokenUsage::new(100, 50);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_stop_reason_mapping() {
        assert_eq!(
            BedrockStopReason::from_str("end_turn"),
            BedrockStopReason::EndTurn
        );
        assert_eq!(
            BedrockStopReason::from_str("tool_use"),
            BedrockStopReason::ToolUse
        );
        assert_eq!(
            BedrockStopReason::EndTurn.to_anthropic_string(),
            "end_turn"
        );
        assert_eq!(
            BedrockStopReason::ContentFiltered.to_anthropic_string(),
            "end_turn"
        );
    }

    #[test]
    fn test_bedrock_tool_config() {
        let tool = BedrockTool {
            tool_spec: BedrockToolSpec {
                name: "get_weather".to_string(),
                description: "Get weather".to_string(),
                input_schema: BedrockToolInputSchema {
                    json: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "location": {"type": "string"}
                        }
                    }),
                },
            },
        };

        let config = BedrockToolConfig {
            tools: vec![tool],
            tool_choice: Some(BedrockToolChoice::Auto {
                auto: serde_json::json!({}),
            }),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"name\":\"get_weather\""));
    }
}
