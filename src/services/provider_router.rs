//! Provider Router — routes requests to the correct LLM provider based on model name.

use std::sync::Arc;

use super::provider::{LLMProvider, ProviderError};

/// Routes requests to the correct provider based on model name.
pub struct ProviderRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
}

impl ProviderRouter {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn LLMProvider>) {
        tracing::info!(
            provider = provider.name(),
            patterns = ?provider.supported_model_patterns(),
            "Registered LLM provider"
        );
        self.providers.push(provider);
    }

    /// Find the right provider for a model.
    pub fn route(&self, model: &str) -> Result<&dyn LLMProvider, ProviderError> {
        for provider in &self.providers {
            if provider.supports_model(model) {
                return Ok(provider.as_ref());
            }
        }
        Err(ProviderError::ModelNotFound(format!(
            "No provider found for model: {}",
            model
        )))
    }

    /// List all supported model patterns across all providers.
    pub fn list_model_patterns(&self) -> Vec<(String, String)> {
        self.providers
            .iter()
            .flat_map(|p| {
                let name = p.name().to_string();
                p.supported_model_patterns()
                    .into_iter()
                    .map(move |pattern| (pattern, name.clone()))
            })
            .collect()
    }

    /// Get the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Check health of all providers.
    pub fn health_status(&self) -> Vec<(&str, bool)> {
        self.providers
            .iter()
            .map(|p| (p.name(), p.health_check()))
            .collect()
    }
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::provider::{
        ProviderError, StreamResult, UnifiedChatRequest, UnifiedChatResponse,
    };

    /// Mock provider for testing.
    struct MockProvider {
        name: String,
        patterns: Vec<String>,
    }

    impl MockProvider {
        fn new(name: &str, patterns: Vec<&str>) -> Self {
            Self {
                name: name.to_string(),
                patterns: patterns.into_iter().map(String::from).collect(),
            }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn supported_model_patterns(&self) -> Vec<String> {
            self.patterns.clone()
        }

        fn supports_model(&self, model: &str) -> bool {
            self.patterns
                .iter()
                .any(|p| crate::services::provider::model_matches_pattern(model, p))
        }

        async fn chat(
            &self,
            _request: UnifiedChatRequest,
        ) -> Result<UnifiedChatResponse, ProviderError> {
            unimplemented!("mock")
        }

        async fn chat_stream(
            &self,
            _request: UnifiedChatRequest,
        ) -> Result<StreamResult, ProviderError> {
            unimplemented!("mock")
        }

        fn health_check(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_route_finds_correct_provider() {
        let mut router = ProviderRouter::new();
        router.register(Arc::new(MockProvider::new("bedrock", vec!["us.anthropic.*", "anthropic.*"])));
        router.register(Arc::new(MockProvider::new("gemini", vec!["gemini-*"])));

        let provider = router.route("us.anthropic.claude-sonnet-4-v1:0").unwrap();
        assert_eq!(provider.name(), "bedrock");

        let provider = router.route("gemini-2.0-flash").unwrap();
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn test_route_returns_error_for_unknown_model() {
        let router = ProviderRouter::new();
        let result = router.route("unknown-model");
        assert!(result.is_err());
    }

    #[test]
    fn test_route_priority_first_match_wins() {
        let mut router = ProviderRouter::new();
        router.register(Arc::new(MockProvider::new("provider_a", vec!["claude-*"])));
        router.register(Arc::new(MockProvider::new("provider_b", vec!["claude-*"])));

        let provider = router.route("claude-sonnet-4").unwrap();
        assert_eq!(provider.name(), "provider_a");
    }

    #[test]
    fn test_list_model_patterns() {
        let mut router = ProviderRouter::new();
        router.register(Arc::new(MockProvider::new("bedrock", vec!["us.anthropic.*"])));
        router.register(Arc::new(MockProvider::new("gemini", vec!["gemini-*"])));

        let patterns = router.list_model_patterns();
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_provider_count() {
        let mut router = ProviderRouter::new();
        assert_eq!(router.provider_count(), 0);
        router.register(Arc::new(MockProvider::new("test", vec!["*"])));
        assert_eq!(router.provider_count(), 1);
    }

    #[test]
    fn test_health_status() {
        let mut router = ProviderRouter::new();
        router.register(Arc::new(MockProvider::new("healthy", vec!["*"])));

        let status = router.health_status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0], ("healthy", true));
    }
}
