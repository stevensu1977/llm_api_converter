//! Gemini provider — wraps GeminiService to implement the LLMProvider trait.

use std::sync::Arc;

use super::gemini::GeminiService;
use super::provider::{
    model_matches_pattern, LLMProvider, ProviderError, StreamResult, UnifiedChatRequest,
    UnifiedChatResponse,
};

/// Gemini model patterns.
const GEMINI_PATTERNS: &[&str] = &["gemini-*"];

/// LLMProvider implementation that wraps the existing GeminiService.
pub struct GeminiProvider {
    service: Arc<GeminiService>,
}

impl GeminiProvider {
    pub fn new(service: Arc<GeminiService>) -> Self {
        Self { service }
    }
}

#[async_trait::async_trait]
impl LLMProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn supported_model_patterns(&self) -> Vec<String> {
        GEMINI_PATTERNS.iter().map(|s| s.to_string()).collect()
    }

    fn supports_model(&self, model: &str) -> bool {
        GEMINI_PATTERNS
            .iter()
            .any(|pattern| model_matches_pattern(model, pattern))
    }

    async fn chat(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, ProviderError> {
        // NOTE: Full conversion from UnifiedChatRequest → GeminiRequest → UnifiedChatResponse
        // will be implemented when handlers are migrated to use ProviderRouter (TASK 4).
        Err(ProviderError::Internal(
            "GeminiProvider.chat() not yet wired — use GeminiService directly for now".into(),
        ))
    }

    async fn chat_stream(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        Err(ProviderError::Internal(
            "GeminiProvider.chat_stream() not yet wired — use GeminiService directly for now"
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
    fn test_gemini_model_patterns() {
        let check = |model: &str| -> bool {
            GEMINI_PATTERNS
                .iter()
                .any(|p| model_matches_pattern(model, p))
        };

        assert!(check("gemini-2.0-flash"));
        assert!(check("gemini-2.5-pro"));
        assert!(check("gemini-1.5-pro-latest"));

        assert!(!check("gpt-4o"));
        assert!(!check("claude-sonnet-4"));
        assert!(!check("us.anthropic.claude-sonnet-4-v1:0"));
    }
}
