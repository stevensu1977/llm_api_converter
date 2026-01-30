//! OpenAI to Bedrock format converter
//!
//! This module handles the conversion of OpenAI Chat Completions API requests
//! to AWS Bedrock Converse API format.

use crate::schemas::bedrock::{
    BedrockContentBlock, BedrockConverseRequest, BedrockImageData, BedrockImageSource,
    BedrockInferenceConfig, BedrockMessage, BedrockSystemMessage, BedrockTool,
    BedrockToolChoice, BedrockToolChoiceTool, BedrockToolConfig, BedrockToolInputSchema,
    BedrockToolResultData, BedrockToolSpec, BedrockToolUseData,
};
use crate::schemas::openai::{
    ChatCompletionRequest, ChatMessage, ChatRole, ContentPart, MessageContent, Tool, ToolChoice,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during OpenAI to Bedrock conversion.
#[derive(Debug, Error)]
pub enum OpenAIConversionError {
    #[error("Invalid content: {0}")]
    InvalidContent(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Invalid tool configuration: {0}")]
    InvalidTool(String),

    #[error("Base64 decode error: {0}")]
    Base64DecodeError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("Invalid image URL: {0}")]
    InvalidImageUrl(String),
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for OpenAI Chat Completions API requests to Bedrock Converse API format.
///
/// This converter handles the transformation of:
/// - Chat messages (system, user, assistant, tool)
/// - Content (text, images)
/// - Tool definitions
/// - Tool calls and results
/// - Inference configuration (temperature, max_tokens, etc.)
#[derive(Debug, Clone)]
pub struct OpenAIToBedrockConverter {
    /// Model ID mapping from OpenAI to Bedrock format
    model_mapping: HashMap<String, String>,
}

impl OpenAIToBedrockConverter {
    /// Create a new converter with default model mappings.
    pub fn new() -> Self {
        let mut model_mapping = HashMap::new();

        // Map OpenAI model names to Bedrock Claude models
        // GPT-4 class models → Claude Sonnet
        model_mapping.insert(
            "gpt-4".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4-turbo".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4-turbo-preview".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4o".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4o-2024-05-13".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4o-2024-08-06".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );

        // GPT-4o-mini and GPT-3.5 class → Claude Haiku
        model_mapping.insert(
            "gpt-4o-mini".to_string(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        );
        model_mapping.insert(
            "gpt-4o-mini-2024-07-18".to_string(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        );
        model_mapping.insert(
            "gpt-3.5-turbo".to_string(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        );
        model_mapping.insert(
            "gpt-3.5-turbo-16k".to_string(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        );

        // o1 reasoning models → Claude Opus
        model_mapping.insert(
            "o1".to_string(),
            "anthropic.claude-opus-4-5-20251101-v1:0".to_string(),
        );
        model_mapping.insert(
            "o1-preview".to_string(),
            "anthropic.claude-opus-4-5-20251101-v1:0".to_string(),
        );
        model_mapping.insert(
            "o1-mini".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );

        Self { model_mapping }
    }

    /// Create a converter with custom model mappings.
    pub fn with_model_mapping(model_mapping: HashMap<String, String>) -> Self {
        Self { model_mapping }
    }

    /// Add a model mapping.
    pub fn add_model_mapping(&mut self, openai_id: String, bedrock_id: String) {
        self.model_mapping.insert(openai_id, bedrock_id);
    }

    // ========================================================================
    // Main Conversion Entry Point
    // ========================================================================

    /// Convert an OpenAI ChatCompletionRequest to Bedrock ConverseRequest.
    pub fn convert_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<BedrockConverseRequest, OpenAIConversionError> {
        // Convert model ID
        let model_id = self.convert_model_id(&request.model);

        // Extract system messages and convert regular messages
        let (system_messages, chat_messages) = self.split_messages(&request.messages);

        // Convert messages
        let messages = self.convert_messages(&chat_messages)?;

        // Get max tokens (prefer max_completion_tokens over max_tokens)
        let max_tokens = request
            .max_completion_tokens
            .or(request.max_tokens)
            .unwrap_or(4096) as i32;

        // Create base request
        let mut bedrock_request = BedrockConverseRequest::new(model_id, messages, max_tokens);

        // Convert inference config
        bedrock_request.inference_config = self.convert_inference_config(request, max_tokens);

        // Convert system prompt
        if !system_messages.is_empty() {
            bedrock_request.system = Some(self.convert_system_messages(&system_messages));
        }

        // Convert tools
        if let Some(ref tools) = request.tools {
            if !tools.is_empty() {
                bedrock_request.tool_config =
                    Some(self.convert_tool_config(tools, &request.tool_choice)?);
            }
        }

        Ok(bedrock_request)
    }

    // ========================================================================
    // Model ID Conversion
    // ========================================================================

    /// Convert OpenAI model ID to Bedrock model ID.
    ///
    /// If the model ID is already a Bedrock ARN or contains known Bedrock prefixes,
    /// it is returned as-is. Otherwise, the mapping is looked up.
    pub fn convert_model_id(&self, openai_model_id: &str) -> String {
        // If it's already a Bedrock model ID or ARN, return as-is
        // Bedrock model IDs look like: anthropic.claude-xxx, amazon.xxx, or ARNs
        if openai_model_id.starts_with("anthropic.")
            || openai_model_id.starts_with("amazon.")
            || openai_model_id.starts_with("meta.")
            || openai_model_id.starts_with("ai21.")
            || openai_model_id.starts_with("cohere.")
            || openai_model_id.starts_with("mistral.")
            || openai_model_id.starts_with("stability.")
            || openai_model_id.starts_with("global.")
            || openai_model_id.starts_with("arn:")
        {
            return openai_model_id.to_string();
        }

        // Look up in mapping, or return original if not found
        self.model_mapping
            .get(openai_model_id)
            .cloned()
            .unwrap_or_else(|| {
                // Default to Claude Sonnet for unknown models
                "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()
            })
    }

    // ========================================================================
    // Message Conversion
    // ========================================================================

    /// Split messages into system messages and regular messages.
    fn split_messages<'a>(&self, messages: &'a [ChatMessage]) -> (Vec<&'a ChatMessage>, Vec<&'a ChatMessage>) {
        let system: Vec<_> = messages
            .iter()
            .filter(|m| m.role == ChatRole::System)
            .collect();

        let others: Vec<_> = messages
            .iter()
            .filter(|m| m.role != ChatRole::System)
            .collect();

        (system, others)
    }

    /// Convert a list of OpenAI messages to Bedrock messages.
    pub fn convert_messages(
        &self,
        messages: &[&ChatMessage],
    ) -> Result<Vec<BedrockMessage>, OpenAIConversionError> {
        let mut result = Vec::new();

        for message in messages {
            if let Some(converted) = self.convert_message(message)? {
                result.push(converted);
            }
        }

        Ok(result)
    }

    /// Convert a single OpenAI message to Bedrock message.
    fn convert_message(
        &self,
        message: &ChatMessage,
    ) -> Result<Option<BedrockMessage>, OpenAIConversionError> {
        let role = match message.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool => "user", // Tool results come as user messages in Bedrock
            ChatRole::System => return Ok(None), // System messages handled separately
        };

        let content = self.convert_message_content(message)?;

        if content.is_empty() {
            return Ok(None);
        }

        Ok(Some(BedrockMessage {
            role: role.to_string(),
            content,
        }))
    }

    /// Convert message content to Bedrock content blocks.
    fn convert_message_content(
        &self,
        message: &ChatMessage,
    ) -> Result<Vec<BedrockContentBlock>, OpenAIConversionError> {
        // Handle tool role messages (tool results)
        if message.role == ChatRole::Tool {
            return self.convert_tool_result_message(message);
        }

        // Handle assistant messages with tool calls
        if message.role == ChatRole::Assistant {
            if let Some(ref tool_calls) = message.tool_calls {
                let mut blocks = Vec::new();

                // Add text content if present
                if let Some(ref content) = message.content {
                    let text = content.to_string_content();
                    if !text.is_empty() {
                        blocks.push(BedrockContentBlock::text(&text));
                    }
                }

                // Add tool use blocks
                for tool_call in tool_calls {
                    let tool_use = BedrockToolUseData {
                        tool_use_id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        input: serde_json::from_str(&tool_call.function.arguments)
                            .unwrap_or_else(|_| serde_json::json!({})),
                    };
                    blocks.push(BedrockContentBlock::ToolUse { tool_use });
                }

                return Ok(blocks);
            }
        }

        // Handle regular content
        match &message.content {
            Some(MessageContent::Text(text)) => Ok(vec![BedrockContentBlock::text(text)]),
            Some(MessageContent::Parts(parts)) => self.convert_content_parts(parts),
            None => Ok(vec![]),
        }
    }

    /// Convert content parts to Bedrock content blocks.
    fn convert_content_parts(
        &self,
        parts: &[ContentPart],
    ) -> Result<Vec<BedrockContentBlock>, OpenAIConversionError> {
        let mut blocks = Vec::new();

        for part in parts {
            match part {
                ContentPart::Text { text } => {
                    blocks.push(BedrockContentBlock::text(text));
                }
                ContentPart::ImageUrl { image_url } => {
                    let image = self.convert_image_url(&image_url.url)?;
                    blocks.push(BedrockContentBlock::Image { image });
                }
            }
        }

        Ok(blocks)
    }

    /// Convert an image URL to Bedrock image data.
    ///
    /// Supports data URLs with base64 encoding.
    fn convert_image_url(&self, url: &str) -> Result<BedrockImageData, OpenAIConversionError> {
        // Handle data URLs (base64 encoded images)
        if url.starts_with("data:") {
            return self.convert_data_url(url);
        }

        // External URLs are not supported (would require fetching)
        Err(OpenAIConversionError::InvalidImageUrl(
            "External image URLs are not supported. Use base64 data URLs instead.".to_string(),
        ))
    }

    /// Convert a data URL to Bedrock image data.
    fn convert_data_url(&self, url: &str) -> Result<BedrockImageData, OpenAIConversionError> {
        // Parse data URL: data:image/png;base64,<data>
        let parts: Vec<&str> = url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(OpenAIConversionError::InvalidImageUrl(
                "Invalid data URL format".to_string(),
            ));
        }

        let metadata = parts[0];
        let data = parts[1];

        // Extract media type
        let media_type = metadata
            .strip_prefix("data:")
            .and_then(|s| s.split(';').next())
            .ok_or_else(|| {
                OpenAIConversionError::InvalidImageUrl("Could not parse media type".to_string())
            })?;

        // Extract format from media type
        let format = media_type.split('/').nth(1).unwrap_or("png").to_string();

        // Decode base64
        let bytes = BASE64
            .decode(data)
            .map_err(|e| OpenAIConversionError::Base64DecodeError(e.to_string()))?;

        Ok(BedrockImageData {
            format,
            source: BedrockImageSource { bytes },
        })
    }

    /// Convert a tool result message to Bedrock format.
    fn convert_tool_result_message(
        &self,
        message: &ChatMessage,
    ) -> Result<Vec<BedrockContentBlock>, OpenAIConversionError> {
        let tool_use_id = message.tool_call_id.as_ref().ok_or_else(|| {
            OpenAIConversionError::MissingField("tool_call_id for tool message".to_string())
        })?;

        let content_text = message
            .content
            .as_ref()
            .map(|c| c.to_string_content())
            .unwrap_or_default();

        let tool_result = BedrockToolResultData {
            tool_use_id: tool_use_id.clone(),
            content: vec![serde_json::json!({"text": content_text})],
            status: Some("success".to_string()),
        };

        Ok(vec![BedrockContentBlock::ToolResult { tool_result }])
    }

    // ========================================================================
    // System Message Conversion
    // ========================================================================

    /// Convert OpenAI system messages to Bedrock system messages.
    fn convert_system_messages(&self, messages: &[&ChatMessage]) -> Vec<BedrockSystemMessage> {
        messages
            .iter()
            .filter_map(|m| {
                m.content.as_ref().map(|c| {
                    let text = c.to_string_content();
                    BedrockSystemMessage::new(&text)
                })
            })
            .collect()
    }

    // ========================================================================
    // Inference Configuration Conversion
    // ========================================================================

    /// Convert OpenAI request parameters to Bedrock inference configuration.
    fn convert_inference_config(
        &self,
        request: &ChatCompletionRequest,
        max_tokens: i32,
    ) -> BedrockInferenceConfig {
        let mut config = BedrockInferenceConfig::new(max_tokens);

        if let Some(temperature) = request.temperature {
            // OpenAI temperature range is 0-2, Bedrock expects 0-1
            // We clamp to 0-1 for safety
            config = config.with_temperature(temperature.min(1.0).max(0.0));
        }

        if let Some(top_p) = request.top_p {
            config = config.with_top_p(top_p);
        }

        if let Some(ref stop) = request.stop {
            config = config.with_stop_sequences(stop.to_vec());
        }

        config
    }

    // ========================================================================
    // Tool Configuration Conversion
    // ========================================================================

    /// Convert OpenAI tools to Bedrock tool configuration.
    fn convert_tool_config(
        &self,
        tools: &[Tool],
        tool_choice: &Option<ToolChoice>,
    ) -> Result<BedrockToolConfig, OpenAIConversionError> {
        let bedrock_tools: Vec<BedrockTool> = tools
            .iter()
            .filter_map(|t| self.convert_tool(t).ok())
            .collect();

        let bedrock_tool_choice = tool_choice.as_ref().map(|tc| self.convert_tool_choice(tc));

        Ok(BedrockToolConfig {
            tools: bedrock_tools,
            tool_choice: bedrock_tool_choice,
        })
    }

    /// Convert a single OpenAI tool definition to Bedrock format.
    fn convert_tool(&self, tool: &Tool) -> Result<BedrockTool, OpenAIConversionError> {
        // Only function tools are supported
        if tool.tool_type != "function" {
            return Err(OpenAIConversionError::InvalidTool(format!(
                "Unsupported tool type: {}",
                tool.tool_type
            )));
        }

        let input_schema = tool
            .function
            .parameters
            .clone()
            .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

        Ok(BedrockTool {
            tool_spec: BedrockToolSpec {
                name: tool.function.name.clone(),
                description: tool.function.description.clone().unwrap_or_default(),
                input_schema: BedrockToolInputSchema { json: input_schema },
            },
        })
    }

    /// Convert OpenAI tool choice to Bedrock format.
    fn convert_tool_choice(&self, tool_choice: &ToolChoice) -> BedrockToolChoice {
        match tool_choice {
            ToolChoice::Mode(mode) => match mode.as_str() {
                "none" => BedrockToolChoice::Auto {
                    auto: serde_json::json!({}),
                }, // No direct equivalent, use auto
                "auto" => BedrockToolChoice::Auto {
                    auto: serde_json::json!({}),
                },
                "required" => BedrockToolChoice::Any {
                    any: serde_json::json!({}),
                },
                _ => BedrockToolChoice::Auto {
                    auto: serde_json::json!({}),
                },
            },
            ToolChoice::Function { function, .. } => BedrockToolChoice::Tool {
                tool: BedrockToolChoiceTool {
                    name: function.name.clone(),
                },
            },
        }
    }
}

impl Default for OpenAIToBedrockConverter {
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
    use crate::schemas::openai::{FunctionCall, FunctionDef, StopSequence, ToolCall};

    #[test]
    fn test_converter_creation() {
        let converter = OpenAIToBedrockConverter::new();
        assert!(!converter.model_mapping.is_empty());
    }

    #[test]
    fn test_model_id_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        // Known mappings
        assert!(converter.convert_model_id("gpt-4").contains("claude"));
        assert!(converter.convert_model_id("gpt-4o").contains("claude"));
        assert!(converter.convert_model_id("gpt-4o-mini").contains("haiku"));
        assert!(converter.convert_model_id("gpt-3.5-turbo").contains("haiku"));

        // Already Bedrock format
        let bedrock_id = "anthropic.claude-3-sonnet";
        assert_eq!(converter.convert_model_id(bedrock_id), bedrock_id);

        // Unknown model defaults to Sonnet
        assert!(converter.convert_model_id("unknown").contains("sonnet"));
    }

    #[test]
    fn test_simple_message_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let refs: Vec<_> = messages.iter().collect();
        let result = converter.convert_messages(&refs).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content.len(), 1);
        assert!(result[0].content[0].is_text());
    }

    #[test]
    fn test_system_message_split() {
        let converter = OpenAIToBedrockConverter::new();

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: Some(MessageContent::Text("You are helpful".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: ChatRole::User,
                content: Some(MessageContent::Text("Hi".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let (system, others) = converter.split_messages(&messages);

        assert_eq!(system.len(), 1);
        assert_eq!(others.len(), 1);
        assert_eq!(system[0].role, ChatRole::System);
        assert_eq!(others[0].role, ChatRole::User);
    }

    #[test]
    fn test_full_request_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: Some(MessageContent::Text("You are helpful".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: Some(MessageContent::Text("Hello".to_string())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(1024),
            max_completion_tokens: None,
            stream: false,
            stream_options: None,
            top_p: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            user: None,
            n: None,
            logprobs: None,
            top_logprobs: None,
        };

        let result = converter.convert_request(&request).unwrap();

        assert!(result.model_id.contains("claude"));
        assert_eq!(result.messages.len(), 1);
        assert!(result.system.is_some());
        assert_eq!(result.inference_config.max_tokens, 1024);
        assert_eq!(result.inference_config.temperature, Some(0.7));
    }

    #[test]
    fn test_tool_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let tool = Tool {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "get_weather".to_string(),
                description: Some("Get weather for a location".to_string()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    },
                    "required": ["location"]
                })),
                strict: None,
            },
        };

        let result = converter.convert_tool(&tool).unwrap();

        assert_eq!(result.tool_spec.name, "get_weather");
        assert_eq!(
            result.tool_spec.description,
            "Get weather for a location"
        );
    }

    #[test]
    fn test_tool_choice_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        // Auto mode
        let auto = ToolChoice::Mode("auto".to_string());
        let result = converter.convert_tool_choice(&auto);
        assert!(matches!(result, BedrockToolChoice::Auto { .. }));

        // Required mode
        let required = ToolChoice::Mode("required".to_string());
        let result = converter.convert_tool_choice(&required);
        assert!(matches!(result, BedrockToolChoice::Any { .. }));

        // Specific function
        let specific = ToolChoice::Function {
            choice_type: "function".to_string(),
            function: crate::schemas::openai::ToolChoiceFunction {
                name: "get_weather".to_string(),
            },
        };
        let result = converter.convert_tool_choice(&specific);
        if let BedrockToolChoice::Tool { tool } = result {
            assert_eq!(tool.name, "get_weather");
        } else {
            panic!("Expected Tool choice");
        }
    }

    #[test]
    fn test_assistant_tool_call_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let message = ChatMessage {
            role: ChatRole::Assistant,
            content: None,
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_123".to_string(),
                tool_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"location": "San Francisco"}"#.to_string(),
                },
            }]),
            tool_call_id: None,
        };

        let result = converter.convert_message_content(&message).unwrap();

        assert_eq!(result.len(), 1);
        if let BedrockContentBlock::ToolUse { tool_use } = &result[0] {
            assert_eq!(tool_use.tool_use_id, "call_123");
            assert_eq!(tool_use.name, "get_weather");
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_tool_result_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let message = ChatMessage {
            role: ChatRole::Tool,
            content: Some(MessageContent::Text("72°F and sunny".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        };

        let result = converter.convert_tool_result_message(&message).unwrap();

        assert_eq!(result.len(), 1);
        if let BedrockContentBlock::ToolResult { tool_result } = &result[0] {
            assert_eq!(tool_result.tool_use_id, "call_123");
            assert_eq!(tool_result.status, Some("success".to_string()));
        } else {
            panic!("Expected ToolResult block");
        }
    }

    #[test]
    fn test_data_url_image_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        // Small valid base64 PNG (1x1 pixel)
        let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

        let result = converter.convert_image_url(data_url).unwrap();

        assert_eq!(result.format, "png");
        assert!(!result.source.bytes.is_empty());
    }

    #[test]
    fn test_external_url_rejected() {
        let converter = OpenAIToBedrockConverter::new();

        let result = converter.convert_image_url("https://example.com/image.png");

        assert!(result.is_err());
    }

    #[test]
    fn test_stop_sequence_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: Some(100),
            max_completion_tokens: None,
            stream: false,
            stream_options: None,
            top_p: None,
            stop: Some(StopSequence::Multiple(vec![
                "STOP".to_string(),
                "END".to_string(),
            ])),
            presence_penalty: None,
            frequency_penalty: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            user: None,
            n: None,
            logprobs: None,
            top_logprobs: None,
        };

        let config = converter.convert_inference_config(&request, 100);

        assert_eq!(
            config.stop_sequences,
            Some(vec!["STOP".to_string(), "END".to_string()])
        );
    }

    #[test]
    fn test_temperature_clamping() {
        let converter = OpenAIToBedrockConverter::new();

        // Temperature > 1 should be clamped to 1
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: Some(1.5),
            max_tokens: Some(100),
            max_completion_tokens: None,
            stream: false,
            stream_options: None,
            top_p: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            user: None,
            n: None,
            logprobs: None,
            top_logprobs: None,
        };

        let config = converter.convert_inference_config(&request, 100);

        assert_eq!(config.temperature, Some(1.0));
    }

    #[test]
    fn test_multipart_content_conversion() {
        let converter = OpenAIToBedrockConverter::new();

        let message = ChatMessage {
            role: ChatRole::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "What's in this image?".to_string(),
                },
                ContentPart::ImageUrl {
                    image_url: crate::schemas::openai::ImageUrl {
                        url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string(),
                        detail: None,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };

        let result = converter.convert_message_content(&message).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result[0].is_text());
        assert!(matches!(result[1], BedrockContentBlock::Image { .. }));
    }

    #[test]
    fn test_max_completion_tokens_preference() {
        let converter = OpenAIToBedrockConverter::new();

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: Some(MessageContent::Text("Hi".to_string())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: Some(1000),
            max_completion_tokens: Some(2000), // This should take precedence
            stream: false,
            stream_options: None,
            top_p: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            user: None,
            n: None,
            logprobs: None,
            top_logprobs: None,
        };

        let result = converter.convert_request(&request).unwrap();

        assert_eq!(result.inference_config.max_tokens, 2000);
    }
}
