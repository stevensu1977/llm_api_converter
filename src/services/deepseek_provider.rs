//! DeepSeek Direct Provider — calls DeepSeek API directly via the OpenAI-compatible interface.
//!
//! DeepSeek API is OpenAI-compatible, so this provider reuses `OpenAIProvider` internally
//! with a different base URL and model patterns.

use super::openai_provider::{OpenAIProvider, OpenAIProviderConfig};
use super::provider::{
    model_matches_pattern, LLMProvider, ProviderError, StreamResult, UnifiedChatRequest,
    UnifiedChatResponse,
};

const DEEPSEEK_API_URL: &str = "https://api.deepseek.com/chat/completions";

const DEEPSEEK_PATTERNS: &[&str] = &[
    "deepseek-chat",
    "deepseek-reasoner",
    "deepseek-coder",
    "deepseek-*",
];

#[derive(Debug, Clone)]
pub struct DeepSeekProviderConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl DeepSeekProviderConfig {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: DEEPSEEK_API_URL.to_string(),
            timeout_seconds: 120,
        }
    }

    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout_seconds = timeout;
        self
    }
}

pub struct DeepSeekProvider {
    inner: OpenAIProvider,
    patterns: Vec<String>,
}

impl DeepSeekProvider {
    pub fn new(config: DeepSeekProviderConfig) -> Result<Self, ProviderError> {
        let openai_config = OpenAIProviderConfig::new(config.api_key)
            .with_base_url(&config.base_url)
            .with_timeout(config.timeout_seconds);

        let patterns: Vec<String> = DEEPSEEK_PATTERNS.iter().map(|s| s.to_string()).collect();
        let inner = OpenAIProvider::with_patterns(openai_config, patterns.clone())?;

        Ok(Self { inner, patterns })
    }
}

#[async_trait::async_trait]
impl LLMProvider for DeepSeekProvider {
    fn name(&self) -> &str {
        "deepseek"
    }

    fn supported_model_patterns(&self) -> Vec<String> {
        self.patterns.clone()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.patterns
            .iter()
            .any(|p| model_matches_pattern(model, p))
    }

    async fn chat(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, ProviderError> {
        self.inner.chat(request).await
    }

    async fn chat_stream(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        self.inner.chat_stream(request).await
    }

    fn health_check(&self) -> bool {
        self.inner.health_check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_supports_models() {
        let config = DeepSeekProviderConfig::new("test-key".to_string());
        let provider = DeepSeekProvider::new(config).unwrap();

        assert!(provider.supports_model("deepseek-chat"));
        assert!(provider.supports_model("deepseek-reasoner"));
        assert!(provider.supports_model("deepseek-coder"));
        assert!(provider.supports_model("deepseek-v3"));

        assert!(!provider.supports_model("gpt-4o"));
        assert!(!provider.supports_model("claude-sonnet-4"));
        assert!(!provider.supports_model("gemini-2.0-flash"));
    }

    #[test]
    fn test_deepseek_provider_name() {
        let config = DeepSeekProviderConfig::new("test-key".to_string());
        let provider = DeepSeekProvider::new(config).unwrap();
        assert_eq!(provider.name(), "deepseek");
    }
}
