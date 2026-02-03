//! Anthropic to Gemini format converter
//!
//! This module handles the conversion of Anthropic Messages API requests
//! to Google Gemini API format.

use crate::schemas::anthropic::{
    ContentBlock, Message, MessageContent, MessageRequest, SystemContent, ToolChoice,
};
use crate::schemas::gemini::{
    FunctionCallingConfig, FunctionDeclaration, GenerationConfig, GeminiContent, GeminiRequest,
    Part, Tool as GeminiTool, ToolConfig,
};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Anthropic to Gemini conversion
#[derive(Debug, Error)]
pub enum AnthropicToGeminiError {
    #[error("Invalid content block: {0}")]
    InvalidContentBlock(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Invalid tool configuration: {0}")]
    InvalidTool(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for Anthropic Messages API requests to Gemini API format
#[derive(Debug, Clone)]
pub struct AnthropicToGeminiConverter {
    /// Model ID mapping from Anthropic to Gemini format
    model_mapping: HashMap<String, String>,
}

impl Default for AnthropicToGeminiConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnthropicToGeminiConverter {
    /// Create a new converter (no model mapping, uses model ID directly)
    pub fn new() -> Self {
        Self {
            model_mapping: HashMap::new(),
        }
    }

    /// Add a custom model mapping
    pub fn with_model_mapping(mut self, anthropic: &str, gemini: &str) -> Self {
        self.model_mapping
            .insert(anthropic.to_string(), gemini.to_string());
        self
    }

    /// Get the Gemini model ID (uses model directly, supports custom mapping if configured)
    pub fn get_gemini_model(&self, model: &str) -> String {
        // Check custom mapping first, otherwise use model ID directly
        self.model_mapping
            .get(model)
            .cloned()
            .unwrap_or_else(|| model.to_string())
    }

    /// Convert an Anthropic request to Gemini format
    pub fn convert_request(
        &self,
        request: &MessageRequest,
    ) -> Result<(String, GeminiRequest), AnthropicToGeminiError> {
        let model = self.get_gemini_model(&request.model);

        // Convert messages to Gemini contents
        let contents = self.convert_messages(&request.messages)?;

        // Convert system prompt
        let system_instruction = self.convert_system(&request.system)?;

        // Convert generation config
        let generation_config = Some(self.convert_generation_config(request));

        // Convert tools
        let tools = self.convert_tools(&request.tools)?;
        let tool_config = self.convert_tool_choice(&request.tool_choice)?;

        let gemini_request = GeminiRequest {
            contents,
            system_instruction,
            generation_config,
            safety_settings: None,
            tools,
            tool_config,
        };

        Ok((model, gemini_request))
    }

    /// Convert Anthropic messages to Gemini contents
    fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<Vec<GeminiContent>, AnthropicToGeminiError> {
        let mut contents = Vec::new();

        for message in messages {
            let role = match message.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                other => {
                    return Err(AnthropicToGeminiError::InvalidMessage(format!(
                        "Unknown role: {}",
                        other
                    )))
                }
            };

            let parts = self.convert_content(&message.content)?;

            contents.push(GeminiContent {
                role: Some(role.to_string()),
                parts,
            });
        }

        Ok(contents)
    }

    /// Convert Anthropic content to Gemini parts
    fn convert_content(
        &self,
        content: &MessageContent,
    ) -> Result<Vec<Part>, AnthropicToGeminiError> {
        match content {
            MessageContent::Text(text) => Ok(vec![Part::text(text)]),
            MessageContent::Blocks(blocks) => {
                let mut parts = Vec::new();

                for block in blocks {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            parts.push(Part::text(text));
                        }
                        ContentBlock::Image { source, .. } => {
                            // Convert image to inline_data
                            parts.push(Part::inline_data(
                                &source.media_type,
                                &source.data,
                            ));
                        }
                        ContentBlock::ToolUse { id: _, name, input, .. } => {
                            // Convert tool_use to function_call
                            parts.push(Part {
                                text: None,
                                inline_data: None,
                                function_call: Some(crate::schemas::gemini::FunctionCall {
                                    name: name.clone(),
                                    args: input.clone(),
                                }),
                                function_response: None,
                            });
                        }
                        ContentBlock::ToolResult { tool_use_id, content, .. } => {
                            // Convert tool_result to function_response
                            let response_value = match content {
                                crate::schemas::anthropic::ToolResultValue::Text(text) => {
                                    serde_json::json!({ "result": text })
                                }
                                crate::schemas::anthropic::ToolResultValue::Blocks(blocks) => {
                                    // Extract text from blocks
                                    let text: String = blocks
                                        .iter()
                                        .filter_map(|b| b.as_text().map(|s| s.to_string()))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    serde_json::json!({ "result": text })
                                }
                            };

                            parts.push(Part {
                                text: None,
                                inline_data: None,
                                function_call: None,
                                function_response: Some(
                                    crate::schemas::gemini::FunctionResponse {
                                        name: tool_use_id.clone(),
                                        response: response_value,
                                    },
                                ),
                            });
                        }
                        ContentBlock::Thinking { .. } => {
                            // Skip thinking blocks - not supported by Gemini
                        }
                        ContentBlock::RedactedThinking { .. } => {
                            // Skip redacted thinking
                        }
                        ContentBlock::Document { .. } => {
                            return Err(AnthropicToGeminiError::UnsupportedFeature(
                                "Document content not yet supported for Gemini".to_string(),
                            ));
                        }
                        ContentBlock::ServerToolUse { .. } => {
                            // Skip server tool use
                        }
                        ContentBlock::ServerToolResult { .. } => {
                            // Skip server tool result
                        }
                    }
                }

                Ok(parts)
            }
        }
    }

    /// Convert Anthropic system prompt to Gemini system instruction
    fn convert_system(
        &self,
        system: &Option<SystemContent>,
    ) -> Result<Option<GeminiContent>, AnthropicToGeminiError> {
        match system {
            None => Ok(None),
            Some(SystemContent::Text(text)) => Ok(Some(GeminiContent::system(text))),
            Some(SystemContent::Messages(messages)) => {
                let text: String = messages
                    .iter()
                    .map(|m| m.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(Some(GeminiContent::system(text)))
            }
        }
    }

    /// Convert generation parameters
    fn convert_generation_config(&self, request: &MessageRequest) -> GenerationConfig {
        GenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: request.top_k,
            max_output_tokens: Some(request.max_tokens),
            stop_sequences: request.stop_sequences.clone(),
            candidate_count: None,
        }
    }

    /// Convert Anthropic tools to Gemini tools
    fn convert_tools(
        &self,
        tools: &Option<Vec<serde_json::Value>>,
    ) -> Result<Option<Vec<GeminiTool>>, AnthropicToGeminiError> {
        match tools {
            None => Ok(None),
            Some(tools) if tools.is_empty() => Ok(None),
            Some(tools) => {
                let mut function_declarations = Vec::new();

                for tool in tools {
                    // Try to extract tool info from JSON value
                    if let Some(obj) = tool.as_object() {
                        // Skip code_execution tools
                        if obj.get("type").and_then(|v| v.as_str()) == Some("code_execution_20250825") {
                            continue;
                        }

                        let name = obj
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let description = obj
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let parameters = obj.get("input_schema").cloned();

                        function_declarations.push(FunctionDeclaration {
                            name,
                            description,
                            parameters,
                        });
                    }
                }

                if function_declarations.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(vec![GeminiTool {
                        function_declarations,
                    }]))
                }
            }
        }
    }

    /// Convert Anthropic tool choice to Gemini tool config
    fn convert_tool_choice(
        &self,
        tool_choice: &Option<ToolChoice>,
    ) -> Result<Option<ToolConfig>, AnthropicToGeminiError> {
        match tool_choice {
            None => Ok(None),
            Some(ToolChoice::Auto(s)) => {
                let mode = match s.as_str() {
                    "any" => "ANY",
                    _ => "AUTO",
                };
                Ok(Some(ToolConfig {
                    function_calling_config: FunctionCallingConfig {
                        mode: mode.to_string(),
                        allowed_function_names: None,
                    },
                }))
            }
            Some(ToolChoice::Specific { name }) => Ok(Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "ANY".to_string(),
                    allowed_function_names: Some(vec![name.clone()]),
                },
            })),
            Some(ToolChoice::Object(obj)) => {
                // Try to parse the object
                if let Some(tool_type) = obj.get("type").and_then(|v| v.as_str()) {
                    match tool_type {
                        "auto" => Ok(Some(ToolConfig {
                            function_calling_config: FunctionCallingConfig {
                                mode: "AUTO".to_string(),
                                allowed_function_names: None,
                            },
                        })),
                        "any" => Ok(Some(ToolConfig {
                            function_calling_config: FunctionCallingConfig {
                                mode: "ANY".to_string(),
                                allowed_function_names: None,
                            },
                        })),
                        "tool" => {
                            let name = obj
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            Ok(Some(ToolConfig {
                                function_calling_config: FunctionCallingConfig {
                                    mode: "ANY".to_string(),
                                    allowed_function_names: Some(vec![name]),
                                },
                            }))
                        }
                        _ => Ok(None),
                    }
                } else {
                    Ok(None)
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
    fn test_model_passthrough() {
        let converter = AnthropicToGeminiConverter::new();

        // Model ID is used directly (no mapping)
        assert_eq!(
            converter.get_gemini_model("gemini-2.5-flash"),
            "gemini-2.5-flash"
        );
        assert_eq!(
            converter.get_gemini_model("gemini-3-pro-preview"),
            "gemini-3-pro-preview"
        );
        assert_eq!(
            converter.get_gemini_model("gemini-2.5-flash-image"),
            "gemini-2.5-flash-image"
        );
    }

    #[test]
    fn test_custom_model_mapping() {
        let converter = AnthropicToGeminiConverter::new()
            .with_model_mapping("my-custom-model", "gemini-2.5-flash");

        assert_eq!(
            converter.get_gemini_model("my-custom-model"),
            "gemini-2.5-flash"
        );
        // Unmapped models pass through directly
        assert_eq!(
            converter.get_gemini_model("gemini-3-flash-preview"),
            "gemini-3-flash-preview"
        );
    }

    #[test]
    fn test_convert_simple_message() {
        let converter = AnthropicToGeminiConverter::new();

        let content = MessageContent::Text("Hello".to_string());
        let parts = converter.convert_content(&content).unwrap();

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].text, Some("Hello".to_string()));
    }
}
