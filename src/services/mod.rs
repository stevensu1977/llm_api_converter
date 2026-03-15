//! Services module
//!
//! Contains business logic and external service integrations.

pub mod backend_pool;
pub mod bedrock;
pub mod bedrock_provider;
pub mod deepseek_provider;
pub mod gemini;
pub mod gemini_provider;
pub mod openai_provider;
pub mod prompt_cache;
pub mod provider;
pub mod provider_router;
pub mod ptc;
pub mod usage_tracker;

pub use backend_pool::{
    ApiKeyCredential, AwsCredential, Credential, CredentialHealth, CredentialPool,
    LoadBalanceStrategy, PoolConfig, PoolStats,
};
pub use bedrock::{
    BedrockError, BedrockService, BedrockStreamError, ConverseRequest, ConverseStreamResponse,
};
pub use bedrock_provider::BedrockProvider;
pub use deepseek_provider::{DeepSeekProvider, DeepSeekProviderConfig};
pub use gemini::{GeminiConfig, GeminiService, GeminiServiceError, GeminiStream};
pub use gemini_provider::GeminiProvider;
pub use openai_provider::{OpenAIProvider, OpenAIProviderConfig};
pub use provider::{LLMProvider, ProviderError, UnifiedChatRequest, UnifiedChatResponse};
pub use provider_router::ProviderRouter;
pub use ptc::{
    ContainerInfo, ExecutionResult, PendingToolCall, PtcError, PtcHealthStatus, PtcResponse,
    PtcResult, PtcService, PtcSession, SandboxConfig, SandboxExecutor, SessionState,
};
pub use usage_tracker::UsageTracker;
