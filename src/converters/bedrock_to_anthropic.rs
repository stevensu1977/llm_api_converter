//! Bedrock to Anthropic format converter
//!
//! This module handles the conversion of AWS Bedrock Converse API responses
//! to Anthropic Messages API format.

use crate::schemas::anthropic::{
    ContentBlock, MessageResponse, StopReason, StreamEvent, Usage,
};
use crate::schemas::bedrock::{
    BedrockContentBlock, BedrockConverseResponse, BedrockStopReason, BedrockStreamEvent,
    BedrockTokenUsage,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Bedrock to Anthropic conversion.
#[derive(Debug, Error)]
pub enum ResponseConversionError {
    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    #[error("Invalid content block: {0}")]
    InvalidContentBlock(String),

    #[error("Base64 encode error: {0}")]
    Base64EncodeError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("JSON serialization error: {0}")]
    JsonError(String),
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for Bedrock Converse API responses to Anthropic Messages API format.
///
/// This converter handles the transformation of:
/// - Content blocks (text, tool_use)
/// - Stop reasons
/// - Token usage
/// - Streaming events
#[derive(Debug, Clone)]
pub struct BedrockToAnthropicConverter {
    /// Original model ID (Anthropic format) for response
    model_id: Option<String>,
}

impl BedrockToAnthropicConverter {
    /// Create a new converter.
    pub fn new() -> Self {
        Self { model_id: None }
    }

    /// Create a converter with a specific model ID to use in responses.
    pub fn with_model_id(model_id: impl Into<String>) -> Self {
        Self {
            model_id: Some(model_id.into()),
        }
    }

    /// Set the model ID to use in responses.
    pub fn set_model_id(&mut self, model_id: impl Into<String>) {
        self.model_id = Some(model_id.into());
    }

    // ========================================================================
    // Main Conversion Entry Point
    // ========================================================================

    /// Convert a Bedrock ConverseResponse to Anthropic MessageResponse.
    pub fn convert_response(
        &self,
        response: &BedrockConverseResponse,
        original_model_id: &str,
    ) -> Result<MessageResponse, ResponseConversionError> {
        // Generate a unique message ID
        let message_id = format!("msg_{}", Uuid::new_v4().to_string().replace("-", ""));

        // Convert content blocks
        let content = self.convert_content_blocks(&response.output.message.content)?;

        // Convert stop reason
        let stop_reason = self.convert_stop_reason(&response.stop_reason);

        // Convert usage
        let usage = self.convert_usage(&response.usage);

        // Use the model ID from the converter, or fall back to the original
        let model = self
            .model_id
            .clone()
            .unwrap_or_else(|| original_model_id.to_string());

        Ok(MessageResponse {
            id: message_id,
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model,
            stop_reason: Some(stop_reason),
            stop_sequence: None,
            usage,
        })
    }

    // ========================================================================
    // Content Block Conversion
    // ========================================================================

    /// Convert Bedrock content blocks to Anthropic format.
    pub fn convert_content_blocks(
        &self,
        blocks: &[BedrockContentBlock],
    ) -> Result<Vec<ContentBlock>, ResponseConversionError> {
        blocks
            .iter()
            .map(|block| self.convert_content_block(block))
            .collect()
    }

    /// Convert a single Bedrock content block to Anthropic format.
    fn convert_content_block(
        &self,
        block: &BedrockContentBlock,
    ) -> Result<ContentBlock, ResponseConversionError> {
        match block {
            BedrockContentBlock::Text { text } => Ok(ContentBlock::Text {
                text: text.clone(),
                cache_control: None,
            }),

            BedrockContentBlock::Image { image } => {
                // Encode bytes to base64
                let data = BASE64.encode(&image.source.bytes);
                let media_type = format!("image/{}", image.format);

                Ok(ContentBlock::Image {
                    source: crate::schemas::anthropic::ImageSource {
                        source_type: "base64".to_string(),
                        media_type,
                        data,
                    },
                    cache_control: None,
                })
            }

            BedrockContentBlock::Document { document } => {
                // Encode bytes to base64
                let data = BASE64.encode(&document.source.bytes);
                let media_type = format!("application/{}", document.format);

                Ok(ContentBlock::Document {
                    source: crate::schemas::anthropic::DocumentSource {
                        source_type: "base64".to_string(),
                        media_type,
                        data,
                    },
                    cache_control: None,
                })
            }

            BedrockContentBlock::ToolUse { tool_use } => Ok(ContentBlock::ToolUse {
                id: tool_use.tool_use_id.clone(),
                name: tool_use.name.clone(),
                input: tool_use.input.clone(),
                caller: None, // No caller info from Bedrock
            }),

            BedrockContentBlock::ToolResult { tool_result } => {
                // Convert tool result content to text
                let content_text = tool_result
                    .content
                    .iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");

                let is_error = tool_result.status.as_deref() == Some("error");

                Ok(ContentBlock::ToolResult {
                    tool_use_id: tool_result.tool_use_id.clone(),
                    content: crate::schemas::anthropic::ToolResultValue::Text(content_text),
                    is_error: Some(is_error),
                    cache_control: None,
                })
            }
        }
    }

    // ========================================================================
    // Stop Reason Conversion
    // ========================================================================

    /// Convert Bedrock stop reason to Anthropic format.
    pub fn convert_stop_reason(&self, bedrock_reason: &str) -> StopReason {
        let parsed = BedrockStopReason::from_str(bedrock_reason);

        match parsed {
            BedrockStopReason::EndTurn => StopReason::EndTurn,
            BedrockStopReason::MaxTokens => StopReason::MaxTokens,
            BedrockStopReason::StopSequence => StopReason::StopSequence,
            BedrockStopReason::ToolUse => StopReason::ToolUse,
            BedrockStopReason::ContentFiltered => StopReason::EndTurn, // Map to end_turn
            BedrockStopReason::Unknown(_) => StopReason::EndTurn,
        }
    }

    // ========================================================================
    // Usage Conversion
    // ========================================================================

    /// Convert Bedrock token usage to Anthropic format.
    pub fn convert_usage(&self, bedrock_usage: &BedrockTokenUsage) -> Usage {
        Usage {
            input_tokens: bedrock_usage.input_tokens,
            output_tokens: bedrock_usage.output_tokens,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }
    }

    // ========================================================================
    // Streaming Event Conversion
    // ========================================================================

    /// Convert a Bedrock stream event to Anthropic stream event.
    pub fn convert_stream_event(
        &self,
        event: &BedrockStreamEvent,
        original_model_id: &str,
    ) -> Result<Option<StreamEvent>, ResponseConversionError> {
        match event {
            BedrockStreamEvent::MessageStart { message_start } => {
                let message_id = format!("msg_{}", Uuid::new_v4().to_string().replace("-", ""));
                let model = self
                    .model_id
                    .clone()
                    .unwrap_or_else(|| original_model_id.to_string());

                Ok(Some(StreamEvent::MessageStart {
                    message: serde_json::json!({
                        "id": message_id,
                        "type": "message",
                        "role": message_start.role,
                        "content": [],
                        "model": model,
                        "stop_reason": null,
                        "stop_sequence": null,
                        "usage": {
                            "input_tokens": 0,
                            "output_tokens": 0
                        }
                    }),
                }))
            }

            BedrockStreamEvent::ContentBlockStart {
                content_block_start,
            } => {
                let content_block = self.convert_stream_content_block_start(&content_block_start.start)?;

                Ok(Some(StreamEvent::ContentBlockStart {
                    index: content_block_start.content_block_index,
                    content_block,
                }))
            }

            BedrockStreamEvent::ContentBlockDelta {
                content_block_delta,
            } => {
                let delta = self.convert_stream_delta(&content_block_delta.delta)?;

                Ok(Some(StreamEvent::ContentBlockDelta {
                    index: content_block_delta.content_block_index,
                    delta,
                }))
            }

            BedrockStreamEvent::ContentBlockStop {
                content_block_stop,
            } => Ok(Some(StreamEvent::ContentBlockStop {
                index: content_block_stop.content_block_index,
            })),

            BedrockStreamEvent::MessageStop { message_stop } => {
                let stop_reason = self.convert_stop_reason(&message_stop.stop_reason);

                Ok(Some(StreamEvent::MessageDelta {
                    delta: serde_json::json!({
                        "stop_reason": stop_reason.to_string(),
                        "stop_sequence": null
                    }),
                    usage: None,
                }))
            }

            BedrockStreamEvent::Metadata { metadata } => {
                Ok(Some(StreamEvent::MessageDelta {
                    delta: serde_json::json!({}),
                    usage: Some(serde_json::json!({
                        "input_tokens": metadata.usage.input_tokens,
                        "output_tokens": metadata.usage.output_tokens
                    })),
                }))
            }
        }
    }

    /// Convert Bedrock content block start to Anthropic format.
    fn convert_stream_content_block_start(
        &self,
        start: &serde_json::Value,
    ) -> Result<serde_json::Value, ResponseConversionError> {
        // Check if this is a tool use block
        if let Some(tool_use) = start.get("toolUse") {
            let id = tool_use
                .get("toolUseId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = tool_use.get("name").and_then(|v| v.as_str()).unwrap_or("");

            Ok(serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": {}
            }))
        } else {
            // Default to text block
            Ok(serde_json::json!({
                "type": "text",
                "text": ""
            }))
        }
    }

    /// Convert Bedrock delta to Anthropic format.
    fn convert_stream_delta(
        &self,
        delta: &serde_json::Value,
    ) -> Result<serde_json::Value, ResponseConversionError> {
        // Check if this is a text delta
        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
            Ok(serde_json::json!({
                "type": "text_delta",
                "text": text
            }))
        }
        // Check if this is a tool use delta (JSON input streaming)
        else if let Some(tool_use) = delta.get("toolUse") {
            let input = tool_use.get("input").and_then(|v| v.as_str()).unwrap_or("");

            Ok(serde_json::json!({
                "type": "input_json_delta",
                "partial_json": input
            }))
        } else {
            // Unknown delta type - pass through
            Ok(serde_json::json!({
                "type": "text_delta",
                "text": ""
            }))
        }
    }

    // ========================================================================
    // SSE Formatting
    // ========================================================================

    /// Format a stream event as Server-Sent Events (SSE) string.
    pub fn format_sse_event(event_type: &str, data: &serde_json::Value) -> String {
        format!("event: {}\ndata: {}\n\n", event_type, data)
    }

    /// Format multiple events as SSE string.
    pub fn format_sse_message_start(
        message_id: &str,
        model: &str,
        role: &str,
    ) -> String {
        let data = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": role,
                "content": [],
                "model": model,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 0
                }
            }
        });
        Self::format_sse_event("message_start", &data)
    }

    /// Format content block start as SSE.
    pub fn format_sse_content_block_start(index: i32, content_block: &serde_json::Value) -> String {
        let data = serde_json::json!({
            "type": "content_block_start",
            "index": index,
            "content_block": content_block
        });
        Self::format_sse_event("content_block_start", &data)
    }

    /// Format content block delta as SSE.
    pub fn format_sse_content_block_delta(index: i32, delta: &serde_json::Value) -> String {
        let data = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": delta
        });
        Self::format_sse_event("content_block_delta", &data)
    }

    /// Format content block stop as SSE.
    pub fn format_sse_content_block_stop(index: i32) -> String {
        let data = serde_json::json!({
            "type": "content_block_stop",
            "index": index
        });
        Self::format_sse_event("content_block_stop", &data)
    }

    /// Format message delta as SSE.
    pub fn format_sse_message_delta(
        stop_reason: &str,
        usage: Option<&Usage>,
    ) -> String {
        let mut data = serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": null
            }
        });

        if let Some(u) = usage {
            data["usage"] = serde_json::json!({
                "output_tokens": u.output_tokens
            });
        }

        Self::format_sse_event("message_delta", &data)
    }

    /// Format message stop as SSE.
    pub fn format_sse_message_stop() -> String {
        let data = serde_json::json!({
            "type": "message_stop"
        });
        Self::format_sse_event("message_stop", &data)
    }

    /// Format ping event as SSE.
    pub fn format_sse_ping() -> String {
        let data = serde_json::json!({
            "type": "ping"
        });
        Self::format_sse_event("ping", &data)
    }
}

impl Default for BedrockToAnthropicConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::bedrock::{
        BedrockOutput, BedrockOutputMessage, BedrockToolResultData, BedrockToolUseData,
    };

    #[test]
    fn test_converter_creation() {
        let converter = BedrockToAnthropicConverter::new();
        assert!(converter.model_id.is_none());

        let converter = BedrockToAnthropicConverter::with_model_id("claude-3-sonnet");
        assert_eq!(converter.model_id, Some("claude-3-sonnet".to_string()));
    }

    #[test]
    fn test_text_content_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_block = BedrockContentBlock::Text {
            text: "Hello, world!".to_string(),
        };

        let result = converter.convert_content_block(&bedrock_block).unwrap();

        match result {
            ContentBlock::Text { text, .. } => {
                assert_eq!(text, "Hello, world!");
            }
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_tool_use_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_block = BedrockContentBlock::ToolUse {
            tool_use: BedrockToolUseData {
                tool_use_id: "tool_123".to_string(),
                name: "get_weather".to_string(),
                input: serde_json::json!({"location": "San Francisco"}),
            },
        };

        let result = converter.convert_content_block(&bedrock_block).unwrap();

        match result {
            ContentBlock::ToolUse { id, name, input, .. } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "San Francisco");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_stop_reason_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        assert!(matches!(
            converter.convert_stop_reason("end_turn"),
            StopReason::EndTurn
        ));
        assert!(matches!(
            converter.convert_stop_reason("max_tokens"),
            StopReason::MaxTokens
        ));
        assert!(matches!(
            converter.convert_stop_reason("stop_sequence"),
            StopReason::StopSequence
        ));
        assert!(matches!(
            converter.convert_stop_reason("tool_use"),
            StopReason::ToolUse
        ));
        assert!(matches!(
            converter.convert_stop_reason("content_filtered"),
            StopReason::EndTurn
        ));
        assert!(matches!(
            converter.convert_stop_reason("unknown"),
            StopReason::EndTurn
        ));
    }

    #[test]
    fn test_usage_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_usage = BedrockTokenUsage::new(100, 50);
        let result = converter.convert_usage(&bedrock_usage);

        assert_eq!(result.input_tokens, 100);
        assert_eq!(result.output_tokens, 50);
        assert!(result.cache_creation_input_tokens.is_none());
        assert!(result.cache_read_input_tokens.is_none());
    }

    #[test]
    fn test_full_response_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![BedrockContentBlock::text("Hello!")],
                },
            },
            stop_reason: "end_turn".to_string(),
            usage: BedrockTokenUsage::new(10, 5),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "claude-3-sonnet")
            .unwrap();

        assert!(result.id.starts_with("msg_"));
        assert_eq!(result.response_type, "message");
        assert_eq!(result.role, "assistant");
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.model, "claude-3-sonnet");
        assert!(matches!(result.stop_reason, Some(StopReason::EndTurn)));
        assert_eq!(result.usage.input_tokens, 10);
        assert_eq!(result.usage.output_tokens, 5);
    }

    #[test]
    fn test_response_with_custom_model_id() {
        let converter = BedrockToAnthropicConverter::with_model_id("claude-3-5-sonnet-20241022");

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![BedrockContentBlock::text("Hello!")],
                },
            },
            stop_reason: "end_turn".to_string(),
            usage: BedrockTokenUsage::new(10, 5),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "anthropic.claude-3-5-sonnet-20241022-v2:0")
            .unwrap();

        // Should use the converter's model_id, not the original
        assert_eq!(result.model, "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_sse_formatting() {
        // Test message start
        let sse = BedrockToAnthropicConverter::format_sse_message_start(
            "msg_123",
            "claude-3-sonnet",
            "assistant",
        );
        assert!(sse.starts_with("event: message_start\n"));
        assert!(sse.contains("\"id\":\"msg_123\""));

        // Test content block delta
        let delta = serde_json::json!({"type": "text_delta", "text": "Hello"});
        let sse = BedrockToAnthropicConverter::format_sse_content_block_delta(0, &delta);
        assert!(sse.starts_with("event: content_block_delta\n"));
        assert!(sse.contains("\"index\":0"));

        // Test message stop
        let sse = BedrockToAnthropicConverter::format_sse_message_stop();
        assert!(sse.starts_with("event: message_stop\n"));
        assert!(sse.contains("\"type\":\"message_stop\""));
    }

    #[test]
    fn test_stream_content_block_start_text() {
        let converter = BedrockToAnthropicConverter::new();

        let start = serde_json::json!({});
        let result = converter.convert_stream_content_block_start(&start).unwrap();

        assert_eq!(result["type"], "text");
        assert_eq!(result["text"], "");
    }

    #[test]
    fn test_stream_content_block_start_tool_use() {
        let converter = BedrockToAnthropicConverter::new();

        let start = serde_json::json!({
            "toolUse": {
                "toolUseId": "tool_123",
                "name": "get_weather"
            }
        });
        let result = converter.convert_stream_content_block_start(&start).unwrap();

        assert_eq!(result["type"], "tool_use");
        assert_eq!(result["id"], "tool_123");
        assert_eq!(result["name"], "get_weather");
    }

    #[test]
    fn test_stream_delta_text() {
        let converter = BedrockToAnthropicConverter::new();

        let delta = serde_json::json!({"text": "Hello"});
        let result = converter.convert_stream_delta(&delta).unwrap();

        assert_eq!(result["type"], "text_delta");
        assert_eq!(result["text"], "Hello");
    }

    #[test]
    fn test_stream_delta_tool_use() {
        let converter = BedrockToAnthropicConverter::new();

        let delta = serde_json::json!({
            "toolUse": {
                "input": "{\"location\":"
            }
        });
        let result = converter.convert_stream_delta(&delta).unwrap();

        assert_eq!(result["type"], "input_json_delta");
        assert_eq!(result["partial_json"], "{\"location\":");
    }

    #[test]
    fn test_multiple_content_blocks_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let blocks = vec![
            BedrockContentBlock::text("First part"),
            BedrockContentBlock::ToolUse {
                tool_use: BedrockToolUseData {
                    tool_use_id: "tool_1".to_string(),
                    name: "get_weather".to_string(),
                    input: serde_json::json!({}),
                },
            },
            BedrockContentBlock::text("Second part"),
        ];

        let result = converter.convert_content_blocks(&blocks).unwrap();

        assert_eq!(result.len(), 3);
        assert!(matches!(&result[0], ContentBlock::Text { text, .. } if text == "First part"));
        assert!(matches!(&result[1], ContentBlock::ToolUse { name, .. } if name == "get_weather"));
        assert!(matches!(&result[2], ContentBlock::Text { text, .. } if text == "Second part"));
    }

    #[test]
    fn test_ping_and_stop_sse() {
        let ping = BedrockToAnthropicConverter::format_sse_ping();
        assert!(ping.contains("event: ping"));
        assert!(ping.contains("\"type\":\"ping\""));

        let stop = BedrockToAnthropicConverter::format_sse_message_stop();
        assert!(stop.contains("event: message_stop"));
    }

    #[test]
    fn test_tool_use_response_with_tool_stop_reason() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![
                        BedrockContentBlock::text("Let me check the weather."),
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "toolu_123".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "San Francisco"}),
                            },
                        },
                    ],
                },
            },
            stop_reason: "tool_use".to_string(),
            usage: BedrockTokenUsage::new(50, 100),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "claude-3-sonnet")
            .unwrap();

        assert_eq!(result.content.len(), 2);
        assert!(matches!(result.stop_reason, Some(StopReason::ToolUse)));

        // Check first block is text
        assert!(matches!(&result.content[0], ContentBlock::Text { text, .. } if text == "Let me check the weather."));

        // Check second block is tool use
        match &result.content[1] {
            ContentBlock::ToolUse { id, name, input, .. } => {
                assert_eq!(id, "toolu_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "San Francisco");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_multiple_tool_uses_in_response() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "toolu_1".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "San Francisco"}),
                            },
                        },
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "toolu_2".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "Tokyo"}),
                            },
                        },
                    ],
                },
            },
            stop_reason: "tool_use".to_string(),
            usage: BedrockTokenUsage::new(30, 80),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "claude-3-sonnet")
            .unwrap();

        assert_eq!(result.content.len(), 2);

        // Both should be tool uses
        for (i, block) in result.content.iter().enumerate() {
            match block {
                ContentBlock::ToolUse { id, name, .. } => {
                    assert_eq!(id, &format!("toolu_{}", i + 1));
                    assert_eq!(name, "get_weather");
                }
                _ => panic!("Expected ToolUse block at index {}", i),
            }
        }
    }

    #[test]
    fn test_tool_result_conversion_from_bedrock() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_block = BedrockContentBlock::ToolResult {
            tool_result: BedrockToolResultData {
                tool_use_id: "toolu_123".to_string(),
                content: vec![
                    serde_json::json!({"text": "72°F, sunny"}),
                    serde_json::json!({"text": "with light winds"}),
                ],
                status: Some("success".to_string()),
            },
        };

        let result = converter.convert_content_block(&bedrock_block).unwrap();

        match result {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "toolu_123");
                assert_eq!(is_error, Some(false));

                // Content should be concatenated text
                if let crate::schemas::anthropic::ToolResultValue::Text(text) = content {
                    assert!(text.contains("72°F, sunny"));
                    assert!(text.contains("with light winds"));
                } else {
                    panic!("Expected Text content");
                }
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_tool_result_error_conversion() {
        let converter = BedrockToAnthropicConverter::new();

        let bedrock_block = BedrockContentBlock::ToolResult {
            tool_result: BedrockToolResultData {
                tool_use_id: "toolu_456".to_string(),
                content: vec![serde_json::json!({"text": "Error: Location not found"})],
                status: Some("error".to_string()),
            },
        };

        let result = converter.convert_content_block(&bedrock_block).unwrap();

        match result {
            ContentBlock::ToolResult { is_error, .. } => {
                assert_eq!(is_error, Some(true));
            }
            _ => panic!("Expected ToolResult block"),
        }
    }
}
