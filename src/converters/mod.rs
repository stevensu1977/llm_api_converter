//! Converters module
//!
//! Contains logic for converting between API formats:
//! - Anthropic <-> Bedrock
//! - OpenAI <-> Bedrock
//!
//! # Usage
//!
//! ```rust,ignore
//! use anthropic_bedrock_proxy::converters::{
//!     AnthropicToBedrockConverter,
//!     BedrockToAnthropicConverter,
//!     OpenAIToBedrockConverter,
//!     BedrockToOpenAIConverter,
//! };
//!
//! // Convert request: Anthropic -> Bedrock
//! let request_converter = AnthropicToBedrockConverter::new();
//! let bedrock_request = request_converter.convert_request(&anthropic_request)?;
//!
//! // Convert response: Bedrock -> Anthropic
//! let response_converter = BedrockToAnthropicConverter::new();
//! let anthropic_response = response_converter.convert_response(&bedrock_response, "model-id")?;
//!
//! // Convert request: OpenAI -> Bedrock
//! let openai_converter = OpenAIToBedrockConverter::new();
//! let bedrock_request = openai_converter.convert_request(&openai_request)?;
//!
//! // Convert response: Bedrock -> OpenAI
//! let openai_response_converter = BedrockToOpenAIConverter::new();
//! let openai_response = openai_response_converter.convert_response(&bedrock_response, "model-id")?;
//! ```

pub mod anthropic_to_bedrock;
pub mod bedrock_to_anthropic;
pub mod bedrock_to_openai;
pub mod openai_to_bedrock;

// Re-export Anthropic converters
pub use anthropic_to_bedrock::AnthropicToBedrockConverter;
pub use bedrock_to_anthropic::BedrockToAnthropicConverter;

// Re-export OpenAI converters
pub use bedrock_to_openai::BedrockToOpenAIConverter;
pub use openai_to_bedrock::OpenAIToBedrockConverter;

// Re-export error types
pub use anthropic_to_bedrock::ConversionError;
pub use bedrock_to_anthropic::ResponseConversionError;
pub use bedrock_to_openai::OpenAIResponseConversionError;
pub use openai_to_bedrock::OpenAIConversionError;
