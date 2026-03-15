//! Bedrock provider — wraps BedrockService to implement the LLMProvider trait.

use std::sync::Arc;

use super::bedrock::BedrockService;
use super::provider::{
    model_matches_pattern, LLMProvider, ProviderError, StreamResult, UnifiedChatRequest,
    UnifiedChatResponse, UnifiedContentBlock, UnifiedUsage,
};

/// Bedrock model patterns — models that should be routed to AWS Bedrock.
const BEDROCK_PATTERNS: &[&str] = &[
    "us.anthropic.*",
    "anthropic.*",
    "us.meta.*",
    "meta.*",
    "us.amazon.*",
    "amazon.*",
    "us.deepseek.*",
    "us.ai21.*",
    "ai21.*",
    "cohere.*",
    "mistral.*",
    "stability.*",
];

/// LLMProvider implementation that wraps the existing BedrockService.
pub struct BedrockProvider {
    service: Arc<BedrockService>,
}

impl BedrockProvider {
    pub fn new(service: Arc<BedrockService>) -> Self {
        Self { service }
    }
}

#[async_trait::async_trait]
impl LLMProvider for BedrockProvider {
    fn name(&self) -> &str {
        "bedrock"
    }

    fn supported_model_patterns(&self) -> Vec<String> {
        BEDROCK_PATTERNS.iter().map(|s| s.to_string()).collect()
    }

    fn supports_model(&self, model: &str) -> bool {
        BEDROCK_PATTERNS
            .iter()
            .any(|pattern| model_matches_pattern(model, pattern))
    }

    async fn chat(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, ProviderError> {
        // NOTE: Full conversion from UnifiedChatRequest → ConverseRequest → UnifiedChatResponse
        // will be implemented when chat_completions.rs and messages.rs are migrated to use
        // the ProviderRouter (TASK 4). For now this provides the trait skeleton.
        Err(ProviderError::Internal(
            "BedrockProvider.chat() not yet wired — use BedrockService directly for now".into(),
        ))
    }

    async fn chat_stream(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        Err(ProviderError::Internal(
            "BedrockProvider.chat_stream() not yet wired — use BedrockService directly for now"
                .into(),
        ))
    }

    fn health_check(&self) -> bool {
        self.service.health_check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bedrock_supports_anthropic_models() {
        // We can't easily create a real BedrockService without AWS SDK setup,
        // so test the pattern matching directly.
        let patterns = BEDROCK_PATTERNS;
        let check = |model: &str| -> bool {
            patterns
                .iter()
                .any(|p| model_matches_pattern(model, p))
        };

        assert!(check("us.anthropic.claude-sonnet-4-20250514-v1:0"));
        assert!(check("anthropic.claude-3-haiku-20240307-v1:0"));
        assert!(check("us.meta.llama3-3-70b-instruct-v1:0"));
        assert!(check("amazon.titan-text-express-v1"));
        assert!(check("us.deepseek.deepseek-r1-v1:0"));
        assert!(check("mistral.mistral-large-2407-v1:0"));

        assert!(!check("gpt-4o"));
        assert!(!check("gemini-2.0-flash"));
        assert!(!check("deepseek-chat"));
    }
}
