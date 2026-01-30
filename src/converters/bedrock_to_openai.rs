//! Bedrock to OpenAI format converter
//!
//! This module handles the conversion of AWS Bedrock Converse API responses
//! to OpenAI Chat Completions API format.

use crate::schemas::bedrock::{
    BedrockContentBlock, BedrockConverseResponse, BedrockStopReason, BedrockStreamEvent,
    BedrockTokenUsage,
};
use crate::schemas::openai::{
    AssistantMessage, ChatCompletionChunk, ChatCompletionResponse, ChatRole, Choice,
    ChunkChoice, ChunkDelta, CompletionUsage, FunctionCall, ToolCall, ToolCallDelta,
    FunctionCallDelta, current_timestamp, generate_completion_id,
};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Bedrock to OpenAI conversion.
#[derive(Debug, Error)]
pub enum OpenAIResponseConversionError {
    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    #[error("Invalid content block: {0}")]
    InvalidContentBlock(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("JSON serialization error: {0}")]
    JsonError(String),
}

// ============================================================================
// Streaming State
// ============================================================================

/// Streaming state for tracking tool calls across chunks.
#[derive(Debug, Default)]
pub struct StreamingState {
    /// Current content block index
    pub current_block_index: i32,
    /// Whether we've sent the initial role chunk
    pub sent_role: bool,
    /// Current tool call index (for multiple tool calls)
    pub tool_call_index: i32,
    /// Map of block index to tool call index
    pub block_to_tool_index: std::collections::HashMap<i32, i32>,
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for Bedrock Converse API responses to OpenAI Chat Completions API format.
///
/// This converter handles the transformation of:
/// - Content (text, tool_calls)
/// - Stop reasons → finish_reason
/// - Token usage
/// - Streaming events → OpenAI chunks
#[derive(Debug, Clone)]
pub struct BedrockToOpenAIConverter {
    /// Model ID to use in responses
    model_id: Option<String>,
    /// Completion ID (generated once and reused for streaming)
    completion_id: String,
    /// Created timestamp
    created: i64,
}

impl BedrockToOpenAIConverter {
    /// Create a new converter.
    pub fn new() -> Self {
        Self {
            model_id: None,
            completion_id: generate_completion_id(),
            created: current_timestamp(),
        }
    }

    /// Create a converter with a specific model ID to use in responses.
    pub fn with_model_id(model_id: impl Into<String>) -> Self {
        Self {
            model_id: Some(model_id.into()),
            completion_id: generate_completion_id(),
            created: current_timestamp(),
        }
    }

    /// Set the model ID to use in responses.
    pub fn set_model_id(&mut self, model_id: impl Into<String>) {
        self.model_id = Some(model_id.into());
    }

    /// Get the completion ID.
    pub fn completion_id(&self) -> &str {
        &self.completion_id
    }

    /// Get the created timestamp.
    pub fn created(&self) -> i64 {
        self.created
    }

    // ========================================================================
    // Main Conversion Entry Point
    // ========================================================================

    /// Convert a Bedrock ConverseResponse to OpenAI ChatCompletionResponse.
    pub fn convert_response(
        &self,
        response: &BedrockConverseResponse,
        original_model_id: &str,
    ) -> Result<ChatCompletionResponse, OpenAIResponseConversionError> {
        let model = self
            .model_id
            .clone()
            .unwrap_or_else(|| original_model_id.to_string());

        // Convert content blocks to OpenAI format
        let (content, tool_calls) = self.convert_content_blocks(&response.output.message.content)?;

        // Convert stop reason
        let finish_reason = self.convert_stop_reason(&response.stop_reason);

        // Convert usage
        let usage = self.convert_usage(&response.usage);

        let message = AssistantMessage {
            role: ChatRole::Assistant,
            content: if content.is_empty() { None } else { Some(content) },
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        };

        Ok(ChatCompletionResponse {
            id: self.completion_id.clone(),
            object: "chat.completion".to_string(),
            created: self.created,
            model,
            choices: vec![Choice {
                index: 0,
                message,
                finish_reason: Some(finish_reason),
                logprobs: None,
            }],
            usage,
            system_fingerprint: None,
        })
    }

    // ========================================================================
    // Content Block Conversion
    // ========================================================================

    /// Convert Bedrock content blocks to OpenAI format.
    /// Returns (text_content, tool_calls).
    fn convert_content_blocks(
        &self,
        blocks: &[BedrockContentBlock],
    ) -> Result<(String, Vec<ToolCall>), OpenAIResponseConversionError> {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in blocks {
            match block {
                BedrockContentBlock::Text { text } => {
                    text_parts.push(text.clone());
                }
                BedrockContentBlock::ToolUse { tool_use } => {
                    let tool_call = ToolCall {
                        id: tool_use.tool_use_id.clone(),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: tool_use.name.clone(),
                            arguments: serde_json::to_string(&tool_use.input)
                                .unwrap_or_else(|_| "{}".to_string()),
                        },
                    };
                    tool_calls.push(tool_call);
                }
                // Images and documents in responses are not supported by OpenAI API
                BedrockContentBlock::Image { .. } | BedrockContentBlock::Document { .. } => {
                    // Skip - OpenAI doesn't return images/documents in completions
                }
                BedrockContentBlock::ToolResult { .. } => {
                    // Tool results shouldn't appear in assistant responses
                }
            }
        }

        let content = text_parts.join("");
        Ok((content, tool_calls))
    }

    // ========================================================================
    // Stop Reason Conversion
    // ========================================================================

    /// Convert Bedrock stop reason to OpenAI finish_reason.
    pub fn convert_stop_reason(&self, bedrock_reason: &str) -> String {
        let parsed = BedrockStopReason::from_str(bedrock_reason);

        match parsed {
            BedrockStopReason::EndTurn => "stop".to_string(),
            BedrockStopReason::MaxTokens => "length".to_string(),
            BedrockStopReason::StopSequence => "stop".to_string(),
            BedrockStopReason::ToolUse => "tool_calls".to_string(),
            BedrockStopReason::ContentFiltered => "content_filter".to_string(),
            BedrockStopReason::Unknown(_) => "stop".to_string(),
        }
    }

    // ========================================================================
    // Usage Conversion
    // ========================================================================

    /// Convert Bedrock token usage to OpenAI format.
    pub fn convert_usage(&self, bedrock_usage: &BedrockTokenUsage) -> CompletionUsage {
        CompletionUsage {
            prompt_tokens: bedrock_usage.input_tokens,
            completion_tokens: bedrock_usage.output_tokens,
            total_tokens: bedrock_usage.input_tokens + bedrock_usage.output_tokens,
            completion_tokens_details: None,
        }
    }

    // ========================================================================
    // Streaming Conversion
    // ========================================================================

    /// Convert a Bedrock stream event to OpenAI streaming chunk.
    pub fn convert_stream_event(
        &self,
        event: &BedrockStreamEvent,
        original_model_id: &str,
        state: &mut StreamingState,
    ) -> Result<Option<ChatCompletionChunk>, OpenAIResponseConversionError> {
        let model = self
            .model_id
            .clone()
            .unwrap_or_else(|| original_model_id.to_string());

        match event {
            BedrockStreamEvent::MessageStart { .. } => {
                // Send initial chunk with role
                state.sent_role = true;

                Ok(Some(ChatCompletionChunk {
                    id: self.completion_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: self.created,
                    model,
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChunkDelta {
                            role: Some(ChatRole::Assistant),
                            content: None,
                            tool_calls: None,
                        },
                        finish_reason: None,
                        logprobs: None,
                    }],
                    system_fingerprint: None,
                    usage: None,
                }))
            }

            BedrockStreamEvent::ContentBlockStart {
                content_block_start,
            } => {
                state.current_block_index = content_block_start.content_block_index;

                // Check if this is a tool use block
                if let Some(tool_use) = content_block_start.start.get("toolUse") {
                    let id = tool_use
                        .get("toolUseId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = tool_use
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Assign a tool call index
                    let tool_index = state.tool_call_index;
                    state
                        .block_to_tool_index
                        .insert(content_block_start.content_block_index, tool_index);
                    state.tool_call_index += 1;

                    return Ok(Some(ChatCompletionChunk {
                        id: self.completion_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created: self.created,
                        model,
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: None,
                                tool_calls: Some(vec![ToolCallDelta {
                                    index: tool_index,
                                    id: Some(id),
                                    tool_type: Some("function".to_string()),
                                    function: Some(FunctionCallDelta {
                                        name: Some(name),
                                        arguments: None,
                                    }),
                                }]),
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        system_fingerprint: None,
                        usage: None,
                    }));
                }

                // Text block start - no content to send yet
                Ok(None)
            }

            BedrockStreamEvent::ContentBlockDelta {
                content_block_delta,
            } => {
                // Check for text delta
                if let Some(text) = content_block_delta.delta.get("text").and_then(|v| v.as_str())
                {
                    return Ok(Some(ChatCompletionChunk {
                        id: self.completion_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created: self.created,
                        model,
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: Some(text.to_string()),
                                tool_calls: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        system_fingerprint: None,
                        usage: None,
                    }));
                }

                // Check for tool use delta (JSON arguments)
                if let Some(tool_use) = content_block_delta.delta.get("toolUse") {
                    if let Some(input) = tool_use.get("input").and_then(|v| v.as_str()) {
                        let tool_index = state
                            .block_to_tool_index
                            .get(&content_block_delta.content_block_index)
                            .copied()
                            .unwrap_or(0);

                        return Ok(Some(ChatCompletionChunk {
                            id: self.completion_id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created: self.created,
                            model,
                            choices: vec![ChunkChoice {
                                index: 0,
                                delta: ChunkDelta {
                                    role: None,
                                    content: None,
                                    tool_calls: Some(vec![ToolCallDelta {
                                        index: tool_index,
                                        id: None,
                                        tool_type: None,
                                        function: Some(FunctionCallDelta {
                                            name: None,
                                            arguments: Some(input.to_string()),
                                        }),
                                    }]),
                                },
                                finish_reason: None,
                                logprobs: None,
                            }],
                            system_fingerprint: None,
                            usage: None,
                        }));
                    }
                }

                Ok(None)
            }

            BedrockStreamEvent::ContentBlockStop { .. } => {
                // No action needed for OpenAI format
                Ok(None)
            }

            BedrockStreamEvent::MessageStop { message_stop } => {
                let finish_reason = self.convert_stop_reason(&message_stop.stop_reason);

                Ok(Some(ChatCompletionChunk {
                    id: self.completion_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: self.created,
                    model,
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChunkDelta::default(),
                        finish_reason: Some(finish_reason),
                        logprobs: None,
                    }],
                    system_fingerprint: None,
                    usage: None,
                }))
            }

            BedrockStreamEvent::Metadata { metadata } => {
                // Optionally include usage in final chunk
                Ok(Some(ChatCompletionChunk {
                    id: self.completion_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: self.created,
                    model,
                    choices: vec![],
                    system_fingerprint: None,
                    usage: Some(CompletionUsage {
                        prompt_tokens: metadata.usage.input_tokens,
                        completion_tokens: metadata.usage.output_tokens,
                        total_tokens: metadata.usage.input_tokens + metadata.usage.output_tokens,
                        completion_tokens_details: None,
                    }),
                }))
            }
        }
    }

    // ========================================================================
    // SSE Formatting
    // ========================================================================

    /// Format a chunk as OpenAI SSE format.
    pub fn format_sse_chunk(chunk: &ChatCompletionChunk) -> String {
        let json = serde_json::to_string(chunk).unwrap_or_else(|_| "{}".to_string());
        format!("data: {}\n\n", json)
    }

    /// Format the final [DONE] message.
    pub fn format_sse_done() -> String {
        "data: [DONE]\n\n".to_string()
    }
}

impl Default for BedrockToOpenAIConverter {
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
        BedrockOutput, BedrockOutputMessage, BedrockToolUseData,
    };

    #[test]
    fn test_converter_creation() {
        let converter = BedrockToOpenAIConverter::new();
        assert!(converter.model_id.is_none());
        assert!(converter.completion_id.starts_with("chatcmpl-"));

        let converter = BedrockToOpenAIConverter::with_model_id("gpt-4");
        assert_eq!(converter.model_id, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_stop_reason_conversion() {
        let converter = BedrockToOpenAIConverter::new();

        assert_eq!(converter.convert_stop_reason("end_turn"), "stop");
        assert_eq!(converter.convert_stop_reason("max_tokens"), "length");
        assert_eq!(converter.convert_stop_reason("stop_sequence"), "stop");
        assert_eq!(converter.convert_stop_reason("tool_use"), "tool_calls");
        assert_eq!(converter.convert_stop_reason("content_filtered"), "content_filter");
        assert_eq!(converter.convert_stop_reason("unknown"), "stop");
    }

    #[test]
    fn test_usage_conversion() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_usage = BedrockTokenUsage::new(100, 50);
        let result = converter.convert_usage(&bedrock_usage);

        assert_eq!(result.prompt_tokens, 100);
        assert_eq!(result.completion_tokens, 50);
        assert_eq!(result.total_tokens, 150);
    }

    #[test]
    fn test_text_response_conversion() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![BedrockContentBlock::text("Hello, world!")],
                },
            },
            stop_reason: "end_turn".to_string(),
            usage: BedrockTokenUsage::new(10, 5),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "gpt-4")
            .unwrap();

        assert_eq!(result.object, "chat.completion");
        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.choices.len(), 1);
        assert_eq!(result.choices[0].message.content, Some("Hello, world!".to_string()));
        assert_eq!(result.choices[0].finish_reason, Some("stop".to_string()));
        assert_eq!(result.usage.prompt_tokens, 10);
        assert_eq!(result.usage.completion_tokens, 5);
    }

    #[test]
    fn test_tool_call_response_conversion() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![BedrockContentBlock::ToolUse {
                        tool_use: BedrockToolUseData {
                            tool_use_id: "call_123".to_string(),
                            name: "get_weather".to_string(),
                            input: serde_json::json!({"location": "San Francisco"}),
                        },
                    }],
                },
            },
            stop_reason: "tool_use".to_string(),
            usage: BedrockTokenUsage::new(20, 30),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "gpt-4")
            .unwrap();

        assert_eq!(result.choices[0].finish_reason, Some("tool_calls".to_string()));

        let tool_calls = result.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_123");
        assert_eq!(tool_calls[0].tool_type, "function");
        assert_eq!(tool_calls[0].function.name, "get_weather");
        assert!(tool_calls[0].function.arguments.contains("San Francisco"));
    }

    #[test]
    fn test_mixed_content_conversion() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![
                        BedrockContentBlock::text("Let me check the weather."),
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "call_456".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "NYC"}),
                            },
                        },
                    ],
                },
            },
            stop_reason: "tool_use".to_string(),
            usage: BedrockTokenUsage::new(15, 25),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "gpt-4")
            .unwrap();

        assert_eq!(
            result.choices[0].message.content,
            Some("Let me check the weather.".to_string())
        );
        assert!(result.choices[0].message.tool_calls.is_some());
        let tool_calls = result.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
    }

    #[test]
    fn test_sse_formatting() {
        let converter = BedrockToOpenAIConverter::new();

        let chunk = ChatCompletionChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    role: None,
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        let sse = BedrockToOpenAIConverter::format_sse_chunk(&chunk);
        assert!(sse.starts_with("data: "));
        assert!(sse.ends_with("\n\n"));
        assert!(sse.contains("\"content\":\"Hello\""));

        let done = BedrockToOpenAIConverter::format_sse_done();
        assert_eq!(done, "data: [DONE]\n\n");
    }

    #[test]
    fn test_streaming_state() {
        use super::StreamingState;

        let mut state = StreamingState::default();
        assert_eq!(state.current_block_index, 0);
        assert!(!state.sent_role);
        assert_eq!(state.tool_call_index, 0);
        assert!(state.block_to_tool_index.is_empty());
    }

    #[test]
    fn test_custom_model_id_in_response() {
        let converter = BedrockToOpenAIConverter::with_model_id("custom-model");

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![BedrockContentBlock::text("Test")],
                },
            },
            stop_reason: "end_turn".to_string(),
            usage: BedrockTokenUsage::new(5, 3),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "original-model")
            .unwrap();

        // Should use the converter's model_id, not the original
        assert_eq!(result.model, "custom-model");
    }

    #[test]
    fn test_empty_content_response() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![],
                },
            },
            stop_reason: "end_turn".to_string(),
            usage: BedrockTokenUsage::new(5, 0),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "gpt-4")
            .unwrap();

        assert!(result.choices[0].message.content.is_none());
        assert!(result.choices[0].message.tool_calls.is_none());
    }

    #[test]
    fn test_multiple_tool_calls() {
        let converter = BedrockToOpenAIConverter::new();

        let bedrock_response = BedrockConverseResponse {
            output: BedrockOutput {
                message: BedrockOutputMessage {
                    role: "assistant".to_string(),
                    content: vec![
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "call_1".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "NYC"}),
                            },
                        },
                        BedrockContentBlock::ToolUse {
                            tool_use: BedrockToolUseData {
                                tool_use_id: "call_2".to_string(),
                                name: "get_weather".to_string(),
                                input: serde_json::json!({"location": "LA"}),
                            },
                        },
                    ],
                },
            },
            stop_reason: "tool_use".to_string(),
            usage: BedrockTokenUsage::new(20, 40),
            metrics: None,
        };

        let result = converter
            .convert_response(&bedrock_response, "gpt-4")
            .unwrap();

        let tool_calls = result.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[1].id, "call_2");
    }
}
