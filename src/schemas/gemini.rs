//! Google Gemini API schema definitions
//!
//! This module contains Rust structures for the Google Gemini REST API
//! request and response formats.

use serde::{Deserialize, Serialize};

// ============================================================================
// Request Types
// ============================================================================

/// Gemini API request body for generateContent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiRequest {
    /// The content of the conversation
    pub contents: Vec<GeminiContent>,

    /// System instruction (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent>,

    /// Generation configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,

    /// Safety settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_settings: Option<Vec<SafetySetting>>,

    /// Tool configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
}

/// Content block containing role and parts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiContent {
    /// Role: "user" or "model"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Content parts
    pub parts: Vec<Part>,
}

impl GeminiContent {
    /// Create a user content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Some("user".to_string()),
            parts: vec![Part::text(text)],
        }
    }

    /// Create a model content
    pub fn model(text: impl Into<String>) -> Self {
        Self {
            role: Some("model".to_string()),
            parts: vec![Part::text(text)],
        }
    }

    /// Create a system instruction (no role)
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: None,
            parts: vec![Part::text(text)],
        }
    }
}

/// A part of the content - can be text, inline data, or function call/response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// Text content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Inline data (images, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,

    /// Function call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,

    /// Function response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
}

impl Part {
    /// Create a text part
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            inline_data: None,
            function_call: None,
            function_response: None,
        }
    }

    /// Create an inline data part
    pub fn inline_data(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: None,
            inline_data: Some(InlineData {
                mime_type: mime_type.into(),
                data: data.into(),
            }),
            function_call: None,
            function_response: None,
        }
    }
}

/// Inline data for images and other binary content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    /// MIME type (e.g., "image/jpeg", "image/png")
    pub mime_type: String,

    /// Base64-encoded data
    pub data: String,
}

/// Function call from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name
    pub name: String,

    /// Function arguments as JSON object
    pub args: serde_json::Value,
}

/// Function response to send back to the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    /// Function name
    pub name: String,

    /// Response content
    pub response: serde_json::Value,
}

/// Generation configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    /// Temperature (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top P (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top K
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Maximum output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Candidate count (usually 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,
}

/// Safety setting
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySetting {
    /// Harm category
    pub category: String,

    /// Block threshold
    pub threshold: String,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Function declarations
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Function declaration for tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    /// Function name
    pub name: String,

    /// Function description
    pub description: String,

    /// Parameters schema (JSON Schema format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    /// Function calling config
    pub function_calling_config: FunctionCallingConfig,
}

/// Function calling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallingConfig {
    /// Mode: "AUTO", "ANY", "NONE"
    pub mode: String,

    /// Allowed function names (for ANY mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Gemini API response for generateContent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    /// Generated candidates
    pub candidates: Vec<Candidate>,

    /// Usage metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,

    /// Model version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

/// A candidate response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The generated content
    pub content: GeminiContent,

    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Safety ratings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,

    /// Citation metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_metadata: Option<CitationMetadata>,

    /// Index of this candidate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<i32>,
}

/// Safety rating
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    /// Category
    pub category: String,

    /// Probability
    pub probability: String,
}

/// Citation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationMetadata {
    /// Citation sources
    pub citation_sources: Vec<CitationSource>,
}

/// Citation source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationSource {
    /// Start index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,

    /// End index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,

    /// URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// License
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Usage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Prompt token count
    pub prompt_token_count: i32,

    /// Candidates token count
    pub candidates_token_count: i32,

    /// Total token count
    pub total_token_count: i32,
}

// ============================================================================
// Streaming Response Types
// ============================================================================

/// Streaming response chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    /// Candidates (partial)
    pub candidates: Vec<Candidate>,

    /// Usage metadata (usually in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

// ============================================================================
// Error Types
// ============================================================================

/// Gemini API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiError {
    /// Error details
    pub error: GeminiErrorDetail,
}

/// Gemini error detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiErrorDetail {
    /// Error code
    pub code: i32,

    /// Error message
    pub message: String,

    /// Error status
    pub status: String,
}

// ============================================================================
// Model Constants
// ============================================================================

/// Supported Gemini models
pub mod models {
    pub const GEMINI_2_0_FLASH: &str = "gemini-2.0-flash";
    pub const GEMINI_2_0_FLASH_LITE: &str = "gemini-2.0-flash-lite";
    pub const GEMINI_1_5_PRO: &str = "gemini-1.5-pro";
    pub const GEMINI_1_5_FLASH: &str = "gemini-1.5-flash";
    pub const GEMINI_1_5_FLASH_8B: &str = "gemini-1.5-flash-8b";
}

/// Finish reasons
pub mod finish_reason {
    pub const STOP: &str = "STOP";
    pub const MAX_TOKENS: &str = "MAX_TOKENS";
    pub const SAFETY: &str = "SAFETY";
    pub const RECITATION: &str = "RECITATION";
    pub const OTHER: &str = "OTHER";
}
