//! Converters module
//!
//! Contains logic for converting between API formats:
//! - Anthropic <-> Bedrock
//! - Anthropic <-> Gemini
//! - OpenAI <-> Bedrock
//! - OpenAI <-> Gemini
//!
//! # Usage
//!
//! ```rust,ignore
//! use anthropic_bedrock_proxy::converters::{
//!     AnthropicToBedrockConverter,
//!     BedrockToAnthropicConverter,
//!     AnthropicToGeminiConverter,
//!     GeminiToAnthropicConverter,
//!     OpenAIToBedrockConverter,
//!     BedrockToOpenAIConverter,
//!     OpenAIToGeminiConverter,
//!     GeminiToOpenAIConverter,
//! };
//! ```

pub mod anthropic_to_bedrock;
pub mod anthropic_to_gemini;
pub mod bedrock_to_anthropic;
pub mod bedrock_to_openai;
pub mod gemini_to_anthropic;
pub mod gemini_to_openai;
pub mod openai_to_bedrock;
pub mod openai_to_gemini;

// Re-export Anthropic <-> Bedrock converters
pub use anthropic_to_bedrock::AnthropicToBedrockConverter;
pub use bedrock_to_anthropic::BedrockToAnthropicConverter;

// Re-export Anthropic <-> Gemini converters
pub use anthropic_to_gemini::AnthropicToGeminiConverter;
pub use gemini_to_anthropic::GeminiToAnthropicConverter;

// Re-export OpenAI <-> Bedrock converters
pub use bedrock_to_openai::BedrockToOpenAIConverter;
pub use openai_to_bedrock::OpenAIToBedrockConverter;

// Re-export OpenAI <-> Gemini converters
pub use gemini_to_openai::GeminiToOpenAIConverter;
pub use openai_to_gemini::OpenAIToGeminiConverter;

// Re-export error types
pub use anthropic_to_bedrock::ConversionError;
pub use anthropic_to_gemini::AnthropicToGeminiError;
pub use bedrock_to_anthropic::ResponseConversionError;
pub use bedrock_to_openai::OpenAIResponseConversionError;
pub use gemini_to_anthropic::GeminiToAnthropicError;
pub use gemini_to_openai::GeminiToOpenAIError;
pub use openai_to_bedrock::OpenAIConversionError;
pub use openai_to_gemini::OpenAIToGeminiError;
