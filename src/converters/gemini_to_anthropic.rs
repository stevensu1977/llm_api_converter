//! Gemini to Anthropic format converter
//!
//! This module handles the conversion of Google Gemini API responses
//! to Anthropic Messages API format.

use crate::schemas::anthropic::{
    ContentBlock, MessageResponse, StopReason, Usage,
};
use crate::schemas::gemini::{Candidate, GeminiResponse, StreamChunk, UsageMetadata};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during Gemini to Anthropic conversion
#[derive(Debug, Error)]
pub enum GeminiToAnthropicError {
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

/// Converter for Gemini API responses to Anthropic Messages API format
#[derive(Debug, Clone, Default)]
pub struct GeminiToAnthropicConverter;

impl GeminiToAnthropicConverter {
    /// Create a new converter
    pub fn new() -> Self {
        Self
    }

    /// Convert a Gemini response to Anthropic format
    pub fn convert_response(
        &self,
        response: &GeminiResponse,
        model: &str,
    ) -> Result<MessageResponse, GeminiToAnthropicError> {
        let candidate = response
            .candidates
            .first()
            .ok_or_else(|| GeminiToAnthropicError::MissingContent("No candidates".to_string()))?;

        let content = self.convert_content(candidate)?;
        let stop_reason = self.convert_finish_reason(candidate.finish_reason.as_deref());
        let usage = self.convert_usage(response.usage_metadata.as_ref());

        Ok(MessageResponse {
            id: format!("msg_{}", Uuid::new_v4().to_string().replace("-", "")),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            content,
            model: model.to_string(),
            stop_reason: Some(stop_reason),
            stop_sequence: None,
            usage,
        })
    }

    /// Convert Gemini candidate content to Anthropic content blocks
    fn convert_content(
        &self,
        candidate: &Candidate,
    ) -> Result<Vec<ContentBlock>, GeminiToAnthropicError> {
        let mut blocks = Vec::new();

        for part in &candidate.content.parts {
            if let Some(ref text) = part.text {
                blocks.push(ContentBlock::Text {
                    text: text.clone(),
                    cache_control: None,
                });
            }

            if let Some(ref function_call) = part.function_call {
                blocks.push(ContentBlock::ToolUse {
                    id: format!("toolu_{}", Uuid::new_v4().to_string().replace("-", "")),
                    name: function_call.name.clone(),
                    input: function_call.args.clone(),
                    caller: None,
                });
            }
        }

        Ok(blocks)
    }

    /// Convert Gemini finish reason to Anthropic stop reason
    fn convert_finish_reason(&self, finish_reason: Option<&str>) -> StopReason {
        match finish_reason {
            Some("STOP") => StopReason::EndTurn,
            Some("MAX_TOKENS") => StopReason::MaxTokens,
            Some("SAFETY") => StopReason::EndTurn, // Anthropic doesn't have safety stop
            Some("RECITATION") => StopReason::EndTurn,
            Some("OTHER") => StopReason::EndTurn,
            _ => StopReason::EndTurn,
        }
    }

    /// Convert Gemini usage to Anthropic usage
    fn convert_usage(&self, usage: Option<&UsageMetadata>) -> Usage {
        match usage {
            Some(u) => Usage {
                input_tokens: u.prompt_token_count,
                output_tokens: u.candidates_token_count,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            None => Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        }
    }

    /// Convert streaming chunk to partial content for SSE
    pub fn convert_stream_chunk(
        &self,
        chunk: &StreamChunk,
    ) -> Result<(Option<String>, Option<StopReason>), GeminiToAnthropicError> {
        // Extract text delta and finish reason
        let mut text_delta = None;
        let mut finish_reason = None;

        if let Some(candidate) = chunk.candidates.first() {
            // Extract text from parts
            for part in &candidate.content.parts {
                if let Some(ref text) = part.text {
                    text_delta = Some(text.clone());
                }
            }

            // Extract finish reason
            if let Some(ref reason) = candidate.finish_reason {
                finish_reason = Some(self.convert_finish_reason(Some(reason)));
            }
        }

        Ok((text_delta, finish_reason))
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
        let converter = GeminiToAnthropicConverter::new();

        assert_eq!(
            converter.convert_finish_reason(Some("STOP")),
            StopReason::EndTurn
        );
        assert_eq!(
            converter.convert_finish_reason(Some("MAX_TOKENS")),
            StopReason::MaxTokens
        );
        assert_eq!(
            converter.convert_finish_reason(None),
            StopReason::EndTurn
        );
    }

    #[test]
    fn test_convert_usage() {
        let converter = GeminiToAnthropicConverter::new();

        let usage = UsageMetadata {
            prompt_token_count: 100,
            candidates_token_count: 50,
            total_token_count: 150,
        };

        let converted = converter.convert_usage(Some(&usage));
        assert_eq!(converted.input_tokens, 100);
        assert_eq!(converted.output_tokens, 50);
    }
}
