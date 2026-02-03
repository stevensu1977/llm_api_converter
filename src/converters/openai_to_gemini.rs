//! OpenAI to Gemini format converter
//!
//! This module handles the conversion of OpenAI Chat Completions API requests
//! to Google Gemini API format.

use crate::schemas::gemini::{
    FunctionCallingConfig, FunctionDeclaration, GenerationConfig, GeminiContent, GeminiRequest,
    Part, Tool as GeminiTool, ToolConfig,
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

/// Errors that can occur during OpenAI to Gemini conversion
#[derive(Debug, Error)]
pub enum OpenAIToGeminiError {
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

/// Converter for OpenAI Chat Completions API requests to Gemini API format
#[derive(Debug, Clone)]
pub struct OpenAIToGeminiConverter {
    /// Model ID mapping from OpenAI to Gemini format
    model_mapping: HashMap<String, String>,
}

impl Default for OpenAIToGeminiConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIToGeminiConverter {
    /// Create a new converter (no model mapping, uses model ID directly)
    pub fn new() -> Self {
        Self {
            model_mapping: HashMap::new(),
        }
    }

    /// Add a custom model mapping
    pub fn with_model_mapping(mut self, openai: &str, gemini: &str) -> Self {
        self.model_mapping
            .insert(openai.to_string(), gemini.to_string());
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

    /// Convert an OpenAI request to Gemini format
    pub fn convert_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<(String, GeminiRequest), OpenAIToGeminiError> {
        let model = self.get_gemini_model(&request.model);

        // Split system and regular messages
        let (system_messages, chat_messages) = self.split_messages(&request.messages);

        // Convert messages to Gemini contents
        let contents = self.convert_messages(&chat_messages)?;

        // Convert system messages
        let system_instruction = self.convert_system_messages(&system_messages)?;

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

    /// Split messages into system and regular messages
    fn split_messages<'a>(
        &self,
        messages: &'a [ChatMessage],
    ) -> (Vec<&'a ChatMessage>, Vec<&'a ChatMessage>) {
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

    /// Convert OpenAI messages to Gemini contents
    fn convert_messages(
        &self,
        messages: &[&ChatMessage],
    ) -> Result<Vec<GeminiContent>, OpenAIToGeminiError> {
        let mut contents = Vec::new();

        for message in messages {
            if let Some(content) = self.convert_message(message)? {
                contents.push(content);
            }
        }

        Ok(contents)
    }

    /// Convert a single message
    fn convert_message(
        &self,
        message: &ChatMessage,
    ) -> Result<Option<GeminiContent>, OpenAIToGeminiError> {
        let role = match message.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "model",
            ChatRole::Tool => "user", // Tool results come as user messages
            ChatRole::System => return Ok(None),
        };

        let parts = self.convert_message_content(message)?;

        if parts.is_empty() {
            return Ok(None);
        }

        Ok(Some(GeminiContent {
            role: Some(role.to_string()),
            parts,
        }))
    }

    /// Convert message content to Gemini parts
    fn convert_message_content(
        &self,
        message: &ChatMessage,
    ) -> Result<Vec<Part>, OpenAIToGeminiError> {
        // Handle tool role messages (function responses)
        if message.role == ChatRole::Tool {
            return self.convert_tool_result_message(message);
        }

        // Handle assistant messages with tool calls
        if message.role == ChatRole::Assistant {
            if let Some(ref tool_calls) = message.tool_calls {
                let mut parts = Vec::new();

                // Add text content if present
                if let Some(ref content) = message.content {
                    let text = content.to_string_content();
                    if !text.is_empty() {
                        parts.push(Part::text(&text));
                    }
                }

                // Add function calls
                for tool_call in tool_calls {
                    let args: serde_json::Value =
                        serde_json::from_str(&tool_call.function.arguments)
                            .unwrap_or_else(|_| serde_json::json!({}));

                    parts.push(Part {
                        text: None,
                        inline_data: None,
                        function_call: Some(crate::schemas::gemini::FunctionCall {
                            name: tool_call.function.name.clone(),
                            args,
                        }),
                        function_response: None,
                    });
                }

                return Ok(parts);
            }
        }

        // Handle regular content
        match &message.content {
            Some(MessageContent::Text(text)) => Ok(vec![Part::text(text)]),
            Some(MessageContent::Parts(parts)) => self.convert_content_parts(parts),
            None => Ok(vec![]),
        }
    }

    /// Convert content parts to Gemini parts
    fn convert_content_parts(&self, parts: &[ContentPart]) -> Result<Vec<Part>, OpenAIToGeminiError> {
        let mut result = Vec::new();

        for part in parts {
            match part {
                ContentPart::Text { text } => {
                    result.push(Part::text(text));
                }
                ContentPart::ImageUrl { image_url } => {
                    let (media_type, data) = self.convert_image_url(&image_url.url)?;
                    result.push(Part::inline_data(&media_type, &data));
                }
            }
        }

        Ok(result)
    }

    /// Convert an image URL to base64 data
    fn convert_image_url(&self, url: &str) -> Result<(String, String), OpenAIToGeminiError> {
        // Handle data URLs (base64 encoded images)
        if url.starts_with("data:") {
            return self.convert_data_url(url);
        }

        // External URLs are not supported
        Err(OpenAIToGeminiError::InvalidImageUrl(
            "External image URLs are not supported. Use base64 data URLs instead.".to_string(),
        ))
    }

    /// Convert a data URL to media type and base64 data
    fn convert_data_url(&self, url: &str) -> Result<(String, String), OpenAIToGeminiError> {
        // Parse data URL: data:image/png;base64,<data>
        let parts: Vec<&str> = url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(OpenAIToGeminiError::InvalidImageUrl(
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
                OpenAIToGeminiError::InvalidImageUrl("Could not parse media type".to_string())
            })?;

        // Verify base64 is valid
        BASE64
            .decode(data)
            .map_err(|e| OpenAIToGeminiError::Base64DecodeError(e.to_string()))?;

        Ok((media_type.to_string(), data.to_string()))
    }

    /// Convert tool result message
    fn convert_tool_result_message(
        &self,
        message: &ChatMessage,
    ) -> Result<Vec<Part>, OpenAIToGeminiError> {
        let tool_call_id = message.tool_call_id.as_ref().ok_or_else(|| {
            OpenAIToGeminiError::MissingField("tool_call_id for tool message".to_string())
        })?;

        let content_text = message
            .content
            .as_ref()
            .map(|c| c.to_string_content())
            .unwrap_or_default();

        Ok(vec![Part {
            text: None,
            inline_data: None,
            function_call: None,
            function_response: Some(crate::schemas::gemini::FunctionResponse {
                name: tool_call_id.clone(),
                response: serde_json::json!({ "result": content_text }),
            }),
        }])
    }

    /// Convert system messages to Gemini system instruction
    fn convert_system_messages(
        &self,
        messages: &[&ChatMessage],
    ) -> Result<Option<GeminiContent>, OpenAIToGeminiError> {
        if messages.is_empty() {
            return Ok(None);
        }

        let text: String = messages
            .iter()
            .filter_map(|m| m.content.as_ref().map(|c| c.to_string_content()))
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            return Ok(None);
        }

        Ok(Some(GeminiContent::system(&text)))
    }

    /// Convert generation parameters
    fn convert_generation_config(&self, request: &ChatCompletionRequest) -> GenerationConfig {
        let max_tokens = request
            .max_completion_tokens
            .or(request.max_tokens)
            .unwrap_or(4096);

        GenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: None, // OpenAI doesn't have top_k
            max_output_tokens: Some(max_tokens),
            stop_sequences: request.stop.as_ref().map(|s| s.to_vec()),
            candidate_count: None,
        }
    }

    /// Convert OpenAI tools to Gemini tools
    fn convert_tools(
        &self,
        tools: &Option<Vec<Tool>>,
    ) -> Result<Option<Vec<GeminiTool>>, OpenAIToGeminiError> {
        match tools {
            None => Ok(None),
            Some(tools) if tools.is_empty() => Ok(None),
            Some(tools) => {
                let mut function_declarations = Vec::new();

                for tool in tools {
                    // Only function tools are supported
                    if tool.tool_type != "function" {
                        continue;
                    }

                    function_declarations.push(FunctionDeclaration {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone().unwrap_or_default(),
                        parameters: tool.function.parameters.clone(),
                    });
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

    /// Convert OpenAI tool choice to Gemini tool config
    fn convert_tool_choice(
        &self,
        tool_choice: &Option<ToolChoice>,
    ) -> Result<Option<ToolConfig>, OpenAIToGeminiError> {
        match tool_choice {
            None => Ok(None),
            Some(ToolChoice::Mode(mode)) => {
                let gemini_mode = match mode.as_str() {
                    "none" => "NONE",
                    "auto" => "AUTO",
                    "required" => "ANY",
                    _ => "AUTO",
                };

                Ok(Some(ToolConfig {
                    function_calling_config: FunctionCallingConfig {
                        mode: gemini_mode.to_string(),
                        allowed_function_names: None,
                    },
                }))
            }
            Some(ToolChoice::Function { function, .. }) => Ok(Some(ToolConfig {
                function_calling_config: FunctionCallingConfig {
                    mode: "ANY".to_string(),
                    allowed_function_names: Some(vec![function.name.clone()]),
                },
            })),
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
        let converter = OpenAIToGeminiConverter::new();

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
            converter.get_gemini_model("gemini-3-flash-preview"),
            "gemini-3-flash-preview"
        );
    }

    #[test]
    fn test_custom_model_mapping() {
        let converter = OpenAIToGeminiConverter::new()
            .with_model_mapping("my-custom-model", "gemini-2.5-flash");

        assert_eq!(
            converter.get_gemini_model("my-custom-model"),
            "gemini-2.5-flash"
        );
        // Unmapped models pass through directly
        assert_eq!(
            converter.get_gemini_model("gemini-3-pro-image-preview"),
            "gemini-3-pro-image-preview"
        );
    }

    #[test]
    fn test_convert_simple_message() {
        let converter = OpenAIToGeminiConverter::new();

        let message = ChatMessage {
            role: ChatRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };

        let result = converter.convert_message(&message).unwrap().unwrap();

        assert_eq!(result.role, Some("user".to_string()));
        assert_eq!(result.parts.len(), 1);
        assert_eq!(result.parts[0].text, Some("Hello".to_string()));
    }

    #[test]
    fn test_convert_generation_config() {
        let converter = OpenAIToGeminiConverter::new();

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(1024),
            max_completion_tokens: None,
            stream: false,
            stream_options: None,
            top_p: Some(0.9),
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

        let config = converter.convert_generation_config(&request);

        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.top_p, Some(0.9));
        assert_eq!(config.max_output_tokens, Some(1024));
    }
}
