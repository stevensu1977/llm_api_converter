//! Anthropic to Bedrock format converter
//!
//! This module handles the conversion of Anthropic Messages API requests
//! to AWS Bedrock Converse API format.

use crate::schemas::anthropic::{
    ContentBlock, Message, MessageContent, MessageRequest, SystemContent, Tool, ToolChoice,
    ToolInputSchema, ToolResultValue,
};
use crate::schemas::bedrock::{
    BedrockContentBlock, BedrockConverseRequest, BedrockDocumentData, BedrockDocumentSource,
    BedrockImageData, BedrockImageSource, BedrockInferenceConfig, BedrockMessage,
    BedrockSystemMessage, BedrockTool, BedrockToolChoice, BedrockToolChoiceTool, BedrockToolConfig,
    BedrockToolInputSchema, BedrockToolResultData, BedrockToolSpec, BedrockToolUseData,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Anthropic to Bedrock conversion.
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Invalid content block: {0}")]
    InvalidContentBlock(String),

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
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for Anthropic Messages API requests to Bedrock Converse API format.
///
/// This converter handles the transformation of:
/// - Content blocks (text, image, document, tool_use, tool_result)
/// - Messages (user, assistant)
/// - System prompts
/// - Tool definitions
/// - Inference configuration (temperature, max_tokens, etc.)
#[derive(Debug, Clone)]
pub struct AnthropicToBedrockConverter {
    /// Model ID mapping from Anthropic to Bedrock format
    model_mapping: HashMap<String, String>,
}

impl AnthropicToBedrockConverter {
    /// Create a new converter with default model mappings.
    pub fn new() -> Self {
        let mut model_mapping = HashMap::new();

        // Default Anthropic to Bedrock model mappings
        model_mapping.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "claude-3-5-sonnet-latest".to_string(),
            "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        );
        model_mapping.insert(
            "claude-3-5-haiku-20241022".to_string(),
            "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
        );
        model_mapping.insert(
            "claude-3-opus-20240229".to_string(),
            "anthropic.claude-3-opus-20240229-v1:0".to_string(),
        );
        model_mapping.insert(
            "claude-3-sonnet-20240229".to_string(),
            "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
        );
        model_mapping.insert(
            "claude-3-haiku-20240307".to_string(),
            "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
        );
        model_mapping.insert(
            "claude-opus-4-5-20251101".to_string(),
            "anthropic.claude-opus-4-5-20251101-v1:0".to_string(),
        );
        model_mapping.insert(
            "claude-sonnet-4-5-20250929".to_string(),
            "anthropic.claude-sonnet-4-5-20250929-v1:0".to_string(),
        );

        Self { model_mapping }
    }

    /// Create a converter with custom model mappings.
    pub fn with_model_mapping(model_mapping: HashMap<String, String>) -> Self {
        Self { model_mapping }
    }

    /// Add a model mapping.
    pub fn add_model_mapping(&mut self, anthropic_id: String, bedrock_id: String) {
        self.model_mapping.insert(anthropic_id, bedrock_id);
    }

    // ========================================================================
    // Main Conversion Entry Point
    // ========================================================================

    /// Convert an Anthropic MessageRequest to Bedrock ConverseRequest.
    pub fn convert_request(
        &self,
        request: &MessageRequest,
    ) -> Result<BedrockConverseRequest, ConversionError> {
        // Convert model ID
        let model_id = self.convert_model_id(&request.model);

        // Convert messages
        let messages = self.convert_messages(&request.messages)?;

        // Create base request
        let mut bedrock_request = BedrockConverseRequest::new(model_id, messages, request.max_tokens);

        // Convert inference config
        bedrock_request.inference_config = self.convert_inference_config(request);

        // Convert system prompt
        if let Some(ref system) = request.system {
            bedrock_request.system = Some(self.convert_system(system));
        }

        // Convert tools
        if let Some(ref tools) = request.tools {
            if !tools.is_empty() {
                // Check if any tools have input_examples
                let has_input_examples = self.tools_have_input_examples(tools);

                if has_input_examples {
                    // When tools have input_examples, pass them through additionalModelRequestFields
                    // in Anthropic format since Bedrock's standard toolSpec doesn't support inputExamples
                    let mut fields = bedrock_request
                        .additional_model_request_fields
                        .unwrap_or_else(|| serde_json::json!({}));

                    if let Some(obj) = fields.as_object_mut() {
                        // Filter out code_execution tools and pass regular tools in Anthropic format
                        let anthropic_tools: Vec<_> = tools
                            .iter()
                            .filter(|t| {
                                !t.get("type")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == "code_execution_20250825")
                                    .unwrap_or(false)
                            })
                            .cloned()
                            .collect();

                        obj.insert("tools".to_string(), serde_json::json!(anthropic_tools));

                        // Also pass tool_choice if specified
                        if let Some(ref tc) = request.tool_choice {
                            obj.insert("tool_choice".to_string(), self.tool_choice_to_json(tc));
                        }
                    }

                    bedrock_request.additional_model_request_fields = Some(fields);
                } else {
                    // Standard tool config without input_examples
                    bedrock_request.tool_config = Some(self.convert_tool_config(tools, &request.tool_choice)?);
                }
            }
        }

        // Handle extended thinking
        if let Some(ref thinking) = request.thinking {
            let mut fields = bedrock_request
                .additional_model_request_fields
                .unwrap_or_else(|| serde_json::json!({}));

            if let Some(obj) = fields.as_object_mut() {
                obj.insert(
                    "thinking".to_string(),
                    serde_json::json!({
                        "type": thinking.thinking_type,
                        "budget_tokens": thinking.budget_tokens
                    }),
                );
            }

            bedrock_request.additional_model_request_fields = Some(fields);
        }

        Ok(bedrock_request)
    }

    /// Check if any tools have input_examples defined.
    fn tools_have_input_examples(&self, tools: &[serde_json::Value]) -> bool {
        tools.iter().any(|tool| {
            tool.get("input_examples")
                .map(|v| !v.is_null() && v.as_array().map(|a| !a.is_empty()).unwrap_or(false))
                .unwrap_or(false)
        })
    }

    /// Convert ToolChoice to JSON for additionalModelRequestFields.
    fn tool_choice_to_json(&self, tool_choice: &ToolChoice) -> serde_json::Value {
        match tool_choice {
            ToolChoice::Auto(s) => serde_json::json!({"type": s}),
            ToolChoice::Specific { name } => serde_json::json!({"type": "tool", "name": name}),
            ToolChoice::Object(obj) => obj.clone(),
        }
    }

    // ========================================================================
    // Model ID Conversion
    // ========================================================================

    /// Convert Anthropic model ID to Bedrock model ID.
    ///
    /// If the model ID is already a Bedrock ARN (contains "anthropic." or "arn:"),
    /// it is returned as-is. Otherwise, the mapping is looked up.
    pub fn convert_model_id(&self, anthropic_model_id: &str) -> String {
        // If it's already a Bedrock model ID or ARN, return as-is
        if anthropic_model_id.contains("anthropic.")
            || anthropic_model_id.starts_with("arn:")
            || anthropic_model_id.contains("::")
        {
            return anthropic_model_id.to_string();
        }

        // Look up in mapping, or return original if not found
        self.model_mapping
            .get(anthropic_model_id)
            .cloned()
            .unwrap_or_else(|| anthropic_model_id.to_string())
    }

    // ========================================================================
    // Message Conversion
    // ========================================================================

    /// Convert a list of Anthropic messages to Bedrock messages.
    pub fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<Vec<BedrockMessage>, ConversionError> {
        messages.iter().map(|m| self.convert_message(m)).collect()
    }

    /// Convert a single Anthropic message to Bedrock message.
    pub fn convert_message(&self, message: &Message) -> Result<BedrockMessage, ConversionError> {
        let content = self.convert_message_content(&message.content)?;

        Ok(BedrockMessage {
            role: message.role.clone(),
            content,
        })
    }

    /// Convert message content (string or blocks) to Bedrock content blocks.
    fn convert_message_content(
        &self,
        content: &MessageContent,
    ) -> Result<Vec<BedrockContentBlock>, ConversionError> {
        match content {
            MessageContent::Text(text) => Ok(vec![BedrockContentBlock::text(text)]),
            MessageContent::Blocks(blocks) => self.convert_content_blocks(blocks),
        }
    }

    // ========================================================================
    // Content Block Conversion
    // ========================================================================

    /// Convert a list of Anthropic content blocks to Bedrock content blocks.
    pub fn convert_content_blocks(
        &self,
        blocks: &[ContentBlock],
    ) -> Result<Vec<BedrockContentBlock>, ConversionError> {
        let mut result = Vec::new();

        for block in blocks {
            if let Some(converted) = self.convert_content_block(block)? {
                result.push(converted);
            }
        }

        Ok(result)
    }

    /// Convert a single Anthropic content block to Bedrock format.
    ///
    /// Returns None for blocks that should be skipped (e.g., thinking blocks
    /// that Bedrock doesn't support directly).
    fn convert_content_block(
        &self,
        block: &ContentBlock,
    ) -> Result<Option<BedrockContentBlock>, ConversionError> {
        match block {
            ContentBlock::Text { text, .. } => Ok(Some(BedrockContentBlock::text(text))),

            ContentBlock::Image { source, .. } => {
                let image = self.convert_image(source)?;
                Ok(Some(BedrockContentBlock::Image { image }))
            }

            ContentBlock::Document { source, .. } => {
                let document = self.convert_document(source)?;
                Ok(Some(BedrockContentBlock::Document { document }))
            }

            ContentBlock::ToolUse { id, name, input, .. } => {
                let tool_use = BedrockToolUseData {
                    tool_use_id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                };
                Ok(Some(BedrockContentBlock::ToolUse { tool_use }))
            }

            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            } => {
                let tool_result = self.convert_tool_result(tool_use_id, content, *is_error)?;
                Ok(Some(BedrockContentBlock::ToolResult { tool_result }))
            }

            // Thinking blocks - skip for now (Bedrock handles differently)
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => Ok(None),

            // Server tool use/result - skip (handled separately in PTC)
            ContentBlock::ServerToolUse { .. } | ContentBlock::ServerToolResult { .. } => Ok(None),
        }
    }

    /// Convert an Anthropic image source to Bedrock image data.
    fn convert_image(
        &self,
        source: &crate::schemas::anthropic::ImageSource,
    ) -> Result<BedrockImageData, ConversionError> {
        // Decode base64 data
        let bytes = BASE64
            .decode(&source.data)
            .map_err(|e| ConversionError::Base64DecodeError(e.to_string()))?;

        // Extract format from media type (e.g., "image/png" -> "png")
        let format = source
            .media_type
            .split('/')
            .nth(1)
            .unwrap_or("png")
            .to_string();

        Ok(BedrockImageData {
            format,
            source: BedrockImageSource { bytes },
        })
    }

    /// Convert an Anthropic document source to Bedrock document data.
    fn convert_document(
        &self,
        source: &crate::schemas::anthropic::DocumentSource,
    ) -> Result<BedrockDocumentData, ConversionError> {
        // Decode base64 data
        let bytes = BASE64
            .decode(&source.data)
            .map_err(|e| ConversionError::Base64DecodeError(e.to_string()))?;

        // Extract format from media type (e.g., "application/pdf" -> "pdf")
        let format = source
            .media_type
            .split('/')
            .nth(1)
            .unwrap_or("pdf")
            .to_string();

        Ok(BedrockDocumentData {
            format,
            name: "document".to_string(), // Default name
            source: BedrockDocumentSource { bytes },
        })
    }

    /// Convert a tool result to Bedrock format.
    fn convert_tool_result(
        &self,
        tool_use_id: &str,
        content: &ToolResultValue,
        is_error: Option<bool>,
    ) -> Result<BedrockToolResultData, ConversionError> {
        let result_content = match content {
            ToolResultValue::Text(text) => vec![serde_json::json!({"text": text})],
            ToolResultValue::Blocks(blocks) => {
                let mut converted = Vec::new();
                for block in blocks {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            converted.push(serde_json::json!({"text": text}));
                        }
                        ContentBlock::Image { source, .. } => {
                            let image = self.convert_image(source)?;
                            converted.push(serde_json::json!({
                                "image": {
                                    "format": image.format,
                                    "source": {"bytes": image.source.bytes}
                                }
                            }));
                        }
                        _ => {
                            // Skip other block types in tool results
                        }
                    }
                }
                converted
            }
        };

        let status = if is_error.unwrap_or(false) {
            Some("error".to_string())
        } else {
            Some("success".to_string())
        };

        Ok(BedrockToolResultData {
            tool_use_id: tool_use_id.to_string(),
            content: result_content,
            status,
        })
    }

    // ========================================================================
    // System Prompt Conversion
    // ========================================================================

    /// Convert Anthropic system content to Bedrock system messages.
    pub fn convert_system(&self, system: &SystemContent) -> Vec<BedrockSystemMessage> {
        match system {
            SystemContent::Text(text) => vec![BedrockSystemMessage::new(text)],
            SystemContent::Messages(messages) => {
                messages.iter().map(|m| BedrockSystemMessage::new(&m.text)).collect()
            }
        }
    }

    // ========================================================================
    // Inference Configuration Conversion
    // ========================================================================

    /// Convert Anthropic request parameters to Bedrock inference configuration.
    pub fn convert_inference_config(&self, request: &MessageRequest) -> BedrockInferenceConfig {
        let mut config = BedrockInferenceConfig::new(request.max_tokens);

        if let Some(temperature) = request.temperature {
            config = config.with_temperature(temperature);
        }

        if let Some(top_p) = request.top_p {
            config = config.with_top_p(top_p);
        }

        if let Some(ref stop_sequences) = request.stop_sequences {
            config = config.with_stop_sequences(stop_sequences.clone());
        }

        // Note: top_k is Anthropic-specific and not directly supported in Bedrock Converse API
        // It can be passed via additional_model_request_fields if needed

        config
    }

    // ========================================================================
    // Tool Configuration Conversion
    // ========================================================================

    /// Convert Anthropic tools to Bedrock tool configuration.
    pub fn convert_tool_config(
        &self,
        tools: &[serde_json::Value],
        tool_choice: &Option<ToolChoice>,
    ) -> Result<BedrockToolConfig, ConversionError> {
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

    /// Convert a single Anthropic tool definition to Bedrock format.
    fn convert_tool(&self, tool: &serde_json::Value) -> Result<BedrockTool, ConversionError> {
        // Try to parse as a regular Tool
        if let Ok(parsed) = serde_json::from_value::<Tool>(tool.clone()) {
            return Ok(BedrockTool {
                tool_spec: BedrockToolSpec {
                    name: parsed.name,
                    description: parsed.description,
                    input_schema: BedrockToolInputSchema {
                        json: self.convert_input_schema(&parsed.input_schema),
                    },
                },
            });
        }

        // Handle CodeExecutionTool or other special tool types
        if let Some(tool_type) = tool.get("type").and_then(|t| t.as_str()) {
            if tool_type == "code_execution_20250825" {
                // Skip code execution tools - they're handled by PTC service
                return Err(ConversionError::UnsupportedFeature(
                    "Code execution tools are handled separately".to_string(),
                ));
            }
        }

        // Fallback: try to extract fields manually
        let name = tool
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| ConversionError::MissingField("tool.name".to_string()))?;

        let description = tool
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        let input_schema = tool.get("input_schema").cloned().unwrap_or_else(|| {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        });

        Ok(BedrockTool {
            tool_spec: BedrockToolSpec {
                name: name.to_string(),
                description: description.to_string(),
                input_schema: BedrockToolInputSchema { json: input_schema },
            },
        })
    }

    /// Convert Anthropic tool input schema to Bedrock format.
    fn convert_input_schema(&self, schema: &ToolInputSchema) -> serde_json::Value {
        let mut result = serde_json::json!({
            "type": schema.schema_type,
            "properties": schema.properties,
        });

        if let Some(ref required) = schema.required {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("required".to_string(), serde_json::json!(required));
            }
        }

        result
    }

    /// Convert Anthropic tool choice to Bedrock format.
    fn convert_tool_choice(&self, tool_choice: &ToolChoice) -> BedrockToolChoice {
        match tool_choice {
            ToolChoice::Auto(s) if s == "auto" => BedrockToolChoice::Auto {
                auto: serde_json::json!({}),
            },
            ToolChoice::Auto(s) if s == "any" => BedrockToolChoice::Any {
                any: serde_json::json!({}),
            },
            ToolChoice::Specific { name } => BedrockToolChoice::Tool {
                tool: BedrockToolChoiceTool { name: name.clone() },
            },
            ToolChoice::Object(obj) => {
                // Try to parse object form
                if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                    BedrockToolChoice::Tool {
                        tool: BedrockToolChoiceTool {
                            name: name.to_string(),
                        },
                    }
                } else {
                    // Default to auto
                    BedrockToolChoice::Auto {
                        auto: serde_json::json!({}),
                    }
                }
            }
            _ => BedrockToolChoice::Auto {
                auto: serde_json::json!({}),
            },
        }
    }
}

impl Default for AnthropicToBedrockConverter {
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
    use crate::schemas::anthropic::{ImageSource, Message, SystemMessage};

    #[test]
    fn test_converter_creation() {
        let converter = AnthropicToBedrockConverter::new();
        assert!(!converter.model_mapping.is_empty());
    }

    #[test]
    fn test_model_id_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        // Known mapping
        let result = converter.convert_model_id("claude-3-5-sonnet-20241022");
        assert_eq!(result, "anthropic.claude-3-5-sonnet-20241022-v2:0");

        // Already Bedrock format
        let result = converter.convert_model_id("anthropic.claude-3-sonnet");
        assert_eq!(result, "anthropic.claude-3-sonnet");

        // ARN format
        let result = converter.convert_model_id("arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-3");
        assert!(result.starts_with("arn:"));

        // Unknown model (passthrough)
        let result = converter.convert_model_id("unknown-model");
        assert_eq!(result, "unknown-model");
    }

    #[test]
    fn test_text_content_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::Text {
            text: "Hello, world!".to_string(),
            cache_control: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        assert!(result.is_some());

        let bedrock_block = result.unwrap();
        assert!(bedrock_block.is_text());
        assert_eq!(bedrock_block.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_message_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let message = Message::user("Hello");
        let result = converter.convert_message(&message).unwrap();

        assert_eq!(result.role, "user");
        assert_eq!(result.content.len(), 1);
        assert!(result.content[0].is_text());
    }

    #[test]
    fn test_message_content_blocks_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let content = MessageContent::Blocks(vec![
            ContentBlock::text("First"),
            ContentBlock::text("Second"),
        ]);

        let result = converter.convert_message_content(&content).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_system_prompt_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        // Simple text
        let system = SystemContent::Text("You are a helpful assistant".to_string());
        let result = converter.convert_system(&system);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "You are a helpful assistant");

        // Multiple system messages
        let system = SystemContent::Messages(vec![
            SystemMessage::new("First instruction"),
            SystemMessage::new("Second instruction"),
        ]);
        let result = converter.convert_system(&system);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_inference_config_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let request = MessageRequest::new("claude-3-sonnet", vec![Message::user("Hi")], 1024)
            .with_temperature(0.7);

        let config = converter.convert_inference_config(&request);

        assert_eq!(config.max_tokens, 1024);
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_tool_use_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::ToolUse {
            id: "tool_123".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({"location": "San Francisco"}),
            caller: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        assert!(result.is_some());

        if let Some(BedrockContentBlock::ToolUse { tool_use }) = result {
            assert_eq!(tool_use.tool_use_id, "tool_123");
            assert_eq!(tool_use.name, "get_weather");
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_tool_result_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: ToolResultValue::Text("72°F and sunny".to_string()),
            is_error: Some(false),
            cache_control: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        assert!(result.is_some());

        if let Some(BedrockContentBlock::ToolResult { tool_result }) = result {
            assert_eq!(tool_result.tool_use_id, "tool_123");
            assert_eq!(tool_result.status, Some("success".to_string()));
        } else {
            panic!("Expected ToolResult block");
        }
    }

    #[test]
    fn test_tool_config_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let tools = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        })];

        let result = converter.convert_tool_config(&tools, &None).unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].tool_spec.name, "get_weather");
    }

    #[test]
    fn test_tool_choice_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        // Auto
        let choice = ToolChoice::Auto("auto".to_string());
        let result = converter.convert_tool_choice(&choice);
        assert!(matches!(result, BedrockToolChoice::Auto { .. }));

        // Any
        let choice = ToolChoice::Auto("any".to_string());
        let result = converter.convert_tool_choice(&choice);
        assert!(matches!(result, BedrockToolChoice::Any { .. }));

        // Specific tool
        let choice = ToolChoice::Specific {
            name: "get_weather".to_string(),
        };
        let result = converter.convert_tool_choice(&choice);
        if let BedrockToolChoice::Tool { tool } = result {
            assert_eq!(tool.name, "get_weather");
        } else {
            panic!("Expected Tool choice");
        }

        // Object form with name
        let choice = ToolChoice::Object(serde_json::json!({"type": "tool", "name": "search_docs"}));
        let result = converter.convert_tool_choice(&choice);
        if let BedrockToolChoice::Tool { tool } = result {
            assert_eq!(tool.name, "search_docs");
        } else {
            panic!("Expected Tool choice from object");
        }

        // Object form without name (falls back to auto)
        let choice = ToolChoice::Object(serde_json::json!({"type": "auto"}));
        let result = converter.convert_tool_choice(&choice);
        assert!(matches!(result, BedrockToolChoice::Auto { .. }));

        // Unknown string (falls back to auto)
        let choice = ToolChoice::Auto("unknown".to_string());
        let result = converter.convert_tool_choice(&choice);
        assert!(matches!(result, BedrockToolChoice::Auto { .. }));
    }

    #[test]
    fn test_thinking_blocks_skipped() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::Thinking {
            thinking: "Let me think...".to_string(),
            signature: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        assert!(result.is_none()); // Thinking blocks should be skipped
    }

    #[test]
    fn test_full_request_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let request = MessageRequest::new(
            "claude-3-5-sonnet-20241022",
            vec![Message::user("Hello")],
            1024,
        )
        .with_system("You are helpful")
        .with_temperature(0.7);

        let result = converter.convert_request(&request).unwrap();

        assert_eq!(result.model_id, "anthropic.claude-3-5-sonnet-20241022-v2:0");
        assert_eq!(result.messages.len(), 1);
        assert!(result.system.is_some());
        assert_eq!(result.inference_config.max_tokens, 1024);
        assert_eq!(result.inference_config.temperature, Some(0.7));
    }

    #[test]
    fn test_image_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        // Small valid base64 PNG (1x1 pixel transparent PNG)
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

        let source = ImageSource {
            source_type: "base64".to_string(),
            media_type: "image/png".to_string(),
            data: png_data.to_string(),
        };

        let result = converter.convert_image(&source).unwrap();
        assert_eq!(result.format, "png");
        assert!(!result.source.bytes.is_empty());
    }

    #[test]
    fn test_error_tool_result_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: ToolResultValue::Text("Error: Invalid location".to_string()),
            is_error: Some(true),
            cache_control: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        if let Some(BedrockContentBlock::ToolResult { tool_result }) = result {
            assert_eq!(tool_result.status, Some("error".to_string()));
        } else {
            panic!("Expected ToolResult block");
        }
    }

    #[test]
    fn test_document_conversion() {
        use crate::schemas::anthropic::DocumentSource;

        let converter = AnthropicToBedrockConverter::new();

        // Small valid base64 PDF-like data
        let pdf_data = "JVBERi0xLjQKMSAwIG9iago8PAo+PgplbmRvYmoK"; // Simple PDF header

        let source = DocumentSource {
            source_type: "base64".to_string(),
            media_type: "application/pdf".to_string(),
            data: pdf_data.to_string(),
        };

        let result = converter.convert_document(&source).unwrap();
        assert_eq!(result.format, "pdf");
        assert!(!result.source.bytes.is_empty());
        assert_eq!(result.name, "document");
    }

    #[test]
    fn test_invalid_base64_error() {
        let converter = AnthropicToBedrockConverter::new();

        let source = ImageSource {
            source_type: "base64".to_string(),
            media_type: "image/png".to_string(),
            data: "not-valid-base64!!!".to_string(),
        };

        let result = converter.convert_image(&source);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConversionError::Base64DecodeError(_)));
    }

    #[test]
    fn test_tool_result_with_blocks() {
        let converter = AnthropicToBedrockConverter::new();

        let block = ContentBlock::ToolResult {
            tool_use_id: "tool_456".to_string(),
            content: ToolResultValue::Blocks(vec![
                ContentBlock::text("First part"),
                ContentBlock::text("Second part"),
            ]),
            is_error: Some(false),
            cache_control: None,
        };

        let result = converter.convert_content_block(&block).unwrap();
        if let Some(BedrockContentBlock::ToolResult { tool_result }) = result {
            assert_eq!(tool_result.tool_use_id, "tool_456");
            assert_eq!(tool_result.content.len(), 2);
            assert_eq!(tool_result.status, Some("success".to_string()));
        } else {
            panic!("Expected ToolResult block");
        }
    }

    #[test]
    fn test_empty_messages_conversion() {
        let converter = AnthropicToBedrockConverter::new();

        let request = MessageRequest::new(
            "claude-3-sonnet",
            vec![], // Empty messages
            1024,
        );

        let result = converter.convert_request(&request).unwrap();
        assert!(result.messages.is_empty());
    }

    #[test]
    fn test_multi_turn_conversation() {
        let converter = AnthropicToBedrockConverter::new();

        let request = MessageRequest::new(
            "claude-3-sonnet",
            vec![
                Message::user("Hello"),
                Message::assistant("Hi there!"),
                Message::user("How are you?"),
            ],
            1024,
        );

        let result = converter.convert_request(&request).unwrap();
        assert_eq!(result.messages.len(), 3);
        assert_eq!(result.messages[0].role, "user");
        assert_eq!(result.messages[1].role, "assistant");
        assert_eq!(result.messages[2].role, "user");
    }

    #[test]
    fn test_tools_have_input_examples() {
        let converter = AnthropicToBedrockConverter::new();

        // Tools without input_examples
        let tools_without = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        })];
        assert!(!converter.tools_have_input_examples(&tools_without));

        // Tools with empty input_examples
        let tools_empty = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {
                "type": "object",
                "properties": {}
            },
            "input_examples": []
        })];
        assert!(!converter.tools_have_input_examples(&tools_empty));

        // Tools with input_examples
        let tools_with = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            },
            "input_examples": [
                {"location": "San Francisco, CA"},
                {"location": "Tokyo, Japan"}
            ]
        })];
        assert!(converter.tools_have_input_examples(&tools_with));

        // Mixed - one tool has examples
        let tools_mixed = vec![
            serde_json::json!({
                "name": "tool1",
                "description": "No examples",
                "input_schema": {"type": "object", "properties": {}}
            }),
            serde_json::json!({
                "name": "tool2",
                "description": "With examples",
                "input_schema": {"type": "object", "properties": {}},
                "input_examples": [{"arg": "value"}]
            }),
        ];
        assert!(converter.tools_have_input_examples(&tools_mixed));
    }

    #[test]
    fn test_tool_choice_to_json() {
        let converter = AnthropicToBedrockConverter::new();

        // Auto
        let auto = ToolChoice::Auto("auto".to_string());
        let json = converter.tool_choice_to_json(&auto);
        assert_eq!(json["type"], "auto");

        // Any
        let any = ToolChoice::Auto("any".to_string());
        let json = converter.tool_choice_to_json(&any);
        assert_eq!(json["type"], "any");

        // Specific
        let specific = ToolChoice::Specific { name: "get_weather".to_string() };
        let json = converter.tool_choice_to_json(&specific);
        assert_eq!(json["type"], "tool");
        assert_eq!(json["name"], "get_weather");
    }

    #[test]
    fn test_tools_with_input_examples_use_additional_fields() {
        let converter = AnthropicToBedrockConverter::new();

        let tools = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"},
                    "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                },
                "required": ["location"]
            },
            "input_examples": [
                {"location": "San Francisco, CA", "unit": "fahrenheit"},
                {"location": "Tokyo, Japan", "unit": "celsius"}
            ]
        })];

        let mut request = MessageRequest::new(
            "claude-3-sonnet",
            vec![Message::user("What's the weather in SF?")],
            1024,
        );
        request.tools = Some(tools);
        request.tool_choice = Some(ToolChoice::Auto("auto".to_string()));

        let result = converter.convert_request(&request).unwrap();

        // Should NOT have tool_config when using input_examples
        assert!(result.tool_config.is_none());

        // Should have additionalModelRequestFields with tools
        let fields = result.additional_model_request_fields.unwrap();
        assert!(fields.get("tools").is_some());

        let tools_array = fields["tools"].as_array().unwrap();
        assert_eq!(tools_array.len(), 1);
        assert_eq!(tools_array[0]["name"], "get_weather");
        assert!(tools_array[0]["input_examples"].is_array());

        // Should have tool_choice in additionalModelRequestFields
        assert_eq!(fields["tool_choice"]["type"], "auto");
    }

    #[test]
    fn test_tools_without_input_examples_use_tool_config() {
        let converter = AnthropicToBedrockConverter::new();

        let tools = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get weather for a location",
            "input_schema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        })];

        let mut request = MessageRequest::new(
            "claude-3-sonnet",
            vec![Message::user("What's the weather?")],
            1024,
        );
        request.tools = Some(tools);

        let result = converter.convert_request(&request).unwrap();

        // Should have tool_config
        assert!(result.tool_config.is_some());

        // Should NOT have tools in additionalModelRequestFields
        assert!(result.additional_model_request_fields.is_none()
            || result.additional_model_request_fields.as_ref()
                .and_then(|f| f.get("tools"))
                .is_none());

        let tool_config = result.tool_config.unwrap();
        assert_eq!(tool_config.tools.len(), 1);
        assert_eq!(tool_config.tools[0].tool_spec.name, "get_weather");
    }

    #[test]
    fn test_multi_turn_tool_use_conversation() {
        let converter = AnthropicToBedrockConverter::new();

        // Simulate a multi-turn conversation with tool use
        let messages = vec![
            Message::user("What's the weather in San Francisco?"),
            Message {
                role: "assistant".to_string(),
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolUse {
                        id: "toolu_123".to_string(),
                        name: "get_weather".to_string(),
                        input: serde_json::json!({"location": "San Francisco"}),
                        caller: None,
                    }
                ]),
            },
            Message {
                role: "user".to_string(),
                content: MessageContent::Blocks(vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "toolu_123".to_string(),
                        content: ToolResultValue::Text("72°F, sunny".to_string()),
                        is_error: Some(false),
                        cache_control: None,
                    }
                ]),
            },
        ];

        let request = MessageRequest::new("claude-3-sonnet", messages, 1024);

        let result = converter.convert_request(&request).unwrap();
        assert_eq!(result.messages.len(), 3);

        // Check that tool use was converted correctly
        let assistant_msg = &result.messages[1];
        assert_eq!(assistant_msg.role, "assistant");

        // Check that tool result was converted correctly
        let user_msg = &result.messages[2];
        assert_eq!(user_msg.role, "user");
    }
}
