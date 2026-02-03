//! Gemini to OpenAI format converter
//!
//! This module handles the conversion of Google Gemini API responses
//! to OpenAI Chat Completions API format.

use crate::schemas::gemini::{Candidate, GeminiResponse, StreamChunk, UsageMetadata};
use crate::schemas::openai::{
    AssistantMessage, ChatCompletionChunk, ChatCompletionResponse, ChatRole, Choice, ChunkChoice,
    ChunkDelta, CompletionUsage, FunctionCall, FunctionCallDelta, ToolCall, ToolCallDelta,
};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Gemini to OpenAI conversion
#[derive(Debug, Error)]
pub enum GeminiToOpenAIError {
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Missing content: {0}")]
    MissingContent(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),
}

// ============================================================================
// Converter Implementation
// ============================================================================

/// Converter for Gemini API responses to OpenAI Chat Completions API format
#[derive(Debug, Clone, Default)]
pub struct GeminiToOpenAIConverter;

impl GeminiToOpenAIConverter {
    /// Create a new converter
    pub fn new() -> Self {
        Self
    }

    /// Convert a Gemini response to OpenAI format
    pub fn convert_response(
        &self,
        response: &GeminiResponse,
        model: &str,
    ) -> Result<ChatCompletionResponse, GeminiToOpenAIError> {
        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| GeminiToOpenAIError::MissingContent("No candidates".to_string()))?;

        let message = self.convert_candidate_to_message(candidate)?;
        let finish_reason = self.convert_finish_reason(candidate.finish_reason.as_deref());
        let usage = self.convert_usage(response.usage_metadata.as_ref());

        let id = format!("chatcmpl-{}", Uuid::new_v4().to_string().replace("-", ""));

        Ok(ChatCompletionResponse {
            id,
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
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

    /// Convert Gemini candidate to OpenAI message
    fn convert_candidate_to_message(
        &self,
        candidate: &Candidate,
    ) -> Result<AssistantMessage, GeminiToOpenAIError> {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for part in &candidate.content.parts {
            if let Some(ref text) = part.text {
                text_parts.push(text.clone());
            }

            if let Some(ref function_call) = part.function_call {
                let call_id = format!("call_{}", Uuid::new_v4().to_string().replace("-", ""));
                tool_calls.push(ToolCall {
                    id: call_id,
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: function_call.name.clone(),
                        arguments: serde_json::to_string(&function_call.args)
                            .unwrap_or_else(|_| "{}".to_string()),
                    },
                });
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        };

        Ok(AssistantMessage {
            role: ChatRole::Assistant,
            content,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        })
    }

    /// Convert Gemini finish reason to OpenAI finish reason string
    fn convert_finish_reason(&self, finish_reason: Option<&str>) -> String {
        match finish_reason {
            Some("STOP") => "stop".to_string(),
            Some("MAX_TOKENS") => "length".to_string(),
            Some("SAFETY") => "content_filter".to_string(),
            Some("RECITATION") => "content_filter".to_string(),
            Some("OTHER") => "stop".to_string(),
            _ => "stop".to_string(),
        }
    }

    /// Convert Gemini usage to OpenAI usage
    fn convert_usage(&self, usage: Option<&UsageMetadata>) -> CompletionUsage {
        match usage {
            Some(u) => CompletionUsage {
                prompt_tokens: u.prompt_token_count,
                completion_tokens: u.candidates_token_count,
                total_tokens: u.total_token_count,
                completion_tokens_details: None,
            },
            None => CompletionUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                completion_tokens_details: None,
            },
        }
    }

    /// Convert streaming chunk to OpenAI stream response
    pub fn convert_stream_chunk(
        &self,
        chunk: &StreamChunk,
        model: &str,
        chunk_index: i32,
    ) -> Result<ChatCompletionChunk, GeminiToOpenAIError> {
        let id = format!("chatcmpl-{}", Uuid::new_v4().to_string().replace("-", ""));

        let mut delta = ChunkDelta {
            role: None,
            content: None,
            tool_calls: None,
        };

        let mut finish_reason = None;

        if let Some(candidate) = chunk.candidates.first() {
            // Extract text delta
            for part in &candidate.content.parts {
                if let Some(ref text) = part.text {
                    delta.content = Some(text.clone());
                }

                // Handle function calls in streaming
                if let Some(ref function_call) = part.function_call {
                    let call_id = format!("call_{}", Uuid::new_v4().to_string().replace("-", ""));
                    delta.tool_calls = Some(vec![ToolCallDelta {
                        index: 0,
                        id: Some(call_id),
                        tool_type: Some("function".to_string()),
                        function: Some(FunctionCallDelta {
                            name: Some(function_call.name.clone()),
                            arguments: Some(
                                serde_json::to_string(&function_call.args)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            ),
                        }),
                    }]);
                }
            }

            // Extract finish reason
            if let Some(ref reason) = candidate.finish_reason {
                finish_reason = Some(self.convert_finish_reason(Some(reason)));
            }
        }

        // Set role on first chunk
        if chunk_index == 0 {
            delta.role = Some(ChatRole::Assistant);
        }

        Ok(ChatCompletionChunk {
            id,
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChunkChoice {
                index: 0,
                delta,
                finish_reason,
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        })
    }

    /// Create a final stream response with usage
    pub fn create_final_stream_response(
        &self,
        model: &str,
        usage: Option<&UsageMetadata>,
    ) -> ChatCompletionChunk {
        let id = format!("chatcmpl-{}", Uuid::new_v4().to_string().replace("-", ""));

        ChatCompletionChunk {
            id,
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![],
            usage: usage.map(|u| CompletionUsage {
                prompt_tokens: u.prompt_token_count,
                completion_tokens: u.candidates_token_count,
                total_tokens: u.total_token_count,
                completion_tokens_details: None,
            }),
            system_fingerprint: None,
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
    fn test_convert_finish_reason() {
        let converter = GeminiToOpenAIConverter::new();

        assert_eq!(converter.convert_finish_reason(Some("STOP")), "stop");
        assert_eq!(converter.convert_finish_reason(Some("MAX_TOKENS")), "length");
        assert_eq!(
            converter.convert_finish_reason(Some("SAFETY")),
            "content_filter"
        );
        assert_eq!(converter.convert_finish_reason(None), "stop");
    }

    #[test]
    fn test_convert_usage() {
        let converter = GeminiToOpenAIConverter::new();

        let usage = UsageMetadata {
            prompt_token_count: 100,
            candidates_token_count: 50,
            total_token_count: 150,
        };

        let converted = converter.convert_usage(Some(&usage));
        assert_eq!(converted.prompt_tokens, 100);
        assert_eq!(converted.completion_tokens, 50);
        assert_eq!(converted.total_tokens, 150);
    }
}
