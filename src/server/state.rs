//! Application state container
//!
//! This module defines the shared application state that is passed
//! to all request handlers via Axum's state extraction.

use crate::config::{create_bedrock_client, create_dynamodb_client, Settings};
use crate::db::{DynamoDbBackend, DynamoDbClient, StorageBackend};
use crate::services::{
    BedrockProvider, BedrockService, DeepSeekProvider, DeepSeekProviderConfig,
    GeminiConfig as GeminiServiceConfig, GeminiProvider, GeminiService, LoadBalanceStrategy,
    OpenAIProvider, OpenAIProviderConfig, ProviderRouter, PtcService, UsageTracker,
};
use std::sync::Arc;
use std::time::Instant;

/// Shared application state
///
/// This struct holds all the shared resources that handlers need access to.
/// It is designed to be cheaply cloneable (via Arc) and thread-safe.
#[derive(Clone)]
pub struct AppState {
    /// Application settings
    pub settings: Arc<Settings>,

    /// DynamoDB client for database operations (kept for backward compatibility)
    pub dynamodb: Arc<DynamoDbClient>,

    /// Unified storage backend (DynamoDB, SQLite, etc.)
    pub storage: Arc<dyn StorageBackend>,

    /// Bedrock service for model inference
    pub bedrock: Arc<BedrockService>,

    /// Usage tracker for recording API usage
    pub usage_tracker: Arc<UsageTracker>,

    /// Application start time (for uptime calculation)
    pub start_time: Instant,

    /// PTC service for Programmatic Tool Calling (optional)
    pub ptc_service: Option<Arc<PtcService>>,

    /// Gemini service for Google Gemini API (optional)
    pub gemini_service: Option<Arc<GeminiService>>,

    /// Unified provider router for model-based routing
    pub provider_router: Arc<ProviderRouter>,
}

impl AppState {
    /// Create a new application state
    ///
    /// This initializes all services and clients needed by the application.
    /// AWS SDK clients are initialized asynchronously.
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let settings = Arc::new(settings);
        let start_time = Instant::now();

        // Initialize AWS SDK clients
        tracing::debug!(
            region = %settings.aws_region,
            dynamodb_endpoint = ?settings.dynamodb_endpoint_url,
            bedrock_endpoint = ?settings.bedrock_endpoint_url,
            "Initializing AWS SDK clients"
        );

        tracing::debug!("Creating DynamoDB client");
        let dynamodb_sdk_client = create_dynamodb_client(&settings).await;
        let dynamodb = Arc::new(DynamoDbClient::new(settings.clone(), dynamodb_sdk_client));

        // Create unified storage backend (wraps DynamoDB for now)
        let storage: Arc<dyn StorageBackend> = Arc::new(DynamoDbBackend::new(dynamodb.clone()));

        tracing::debug!("Creating Bedrock client");
        // Check if multiple Bedrock profiles are configured
        if settings.bedrock.has_multiple_profiles() {
            tracing::warn!(
                profile_count = settings.bedrock.profiles.len(),
                "Multiple Bedrock profiles configured via BEDROCK_PROFILES. \
                Multi-profile load balancing is not yet fully implemented. \
                Using default credentials for now."
            );
        }
        let bedrock_sdk_client = create_bedrock_client(&settings).await;
        let bedrock = Arc::new(BedrockService::new(settings.clone(), bedrock_sdk_client));

        tracing::debug!("Initializing usage tracker");
        let usage_tracker = Arc::new(UsageTracker::new(dynamodb.clone()));

        // Initialize PTC service if enabled
        let ptc_service = if settings.features.enable_ptc {
            tracing::info!("PTC enabled, initializing PTC service");
            match PtcService::new().await {
                Ok(service) => Some(Arc::new(service)),
                Err(e) => {
                    tracing::warn!("Failed to initialize PTC service: {}. PTC will be disabled.", e);
                    None
                }
            }
        } else {
            tracing::debug!("PTC disabled");
            None
        };

        // Initialize Gemini service if enabled
        let gemini_service = if settings.gemini.is_available() {
            let api_keys = settings.gemini.get_all_keys();
            tracing::info!(
                key_count = api_keys.len(),
                "Gemini enabled, initializing Gemini service with multi-key support"
            );

            // Create Gemini config with all keys
            let mut gemini_config = GeminiServiceConfig::with_keys(api_keys)
                .with_timeout(settings.gemini.timeout_seconds);

            // Apply base URL if specified
            if let Some(ref base_url) = settings.gemini.base_url {
                gemini_config = gemini_config.with_base_url(base_url);
            }

            // Apply load balancing settings from backend_pool config
            let strategy = LoadBalanceStrategy::from_str(&settings.backend_pool.strategy);
            gemini_config = gemini_config
                .with_strategy(strategy)
                .with_max_failures(settings.backend_pool.max_failures)
                .with_retry_after(settings.backend_pool.retry_after_secs);

            match GeminiService::new(gemini_config) {
                Ok(service) => {
                    tracing::info!(
                        healthy_keys = service.healthy_key_count(),
                        strategy = %strategy,
                        "Gemini service initialized successfully"
                    );
                    Some(Arc::new(service))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize Gemini service: {}. Gemini will be disabled.", e);
                    None
                }
            }
        } else {
            tracing::debug!("Gemini disabled or no API key configured");
            None
        };

        // Initialize ProviderRouter with all available providers
        let mut provider_router = ProviderRouter::new();

        // Always register Bedrock provider
        provider_router.register(Arc::new(BedrockProvider::new(bedrock.clone())));

        // Register Gemini provider if available
        if let Some(ref gemini_svc) = gemini_service {
            provider_router.register(Arc::new(GeminiProvider::new(gemini_svc.clone())));
        }

        // Register OpenAI provider if configured
        if settings.openai.is_available() {
            let mut openai_config =
                OpenAIProviderConfig::new(settings.openai.api_key.clone().unwrap())
                    .with_timeout(settings.openai.timeout_seconds);
            if let Some(ref base_url) = settings.openai.base_url {
                openai_config = openai_config.with_base_url(base_url);
            }
            match OpenAIProvider::new(openai_config) {
                Ok(provider) => {
                    tracing::info!("OpenAI provider initialized");
                    provider_router.register(Arc::new(provider));
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize OpenAI provider: {}. OpenAI will be disabled.", e);
                }
            }
        } else {
            tracing::debug!("OpenAI disabled or no API key configured");
        }

        // Register DeepSeek provider if configured
        if settings.deepseek.is_available() {
            let mut deepseek_config =
                DeepSeekProviderConfig::new(settings.deepseek.api_key.clone().unwrap())
                    .with_timeout(settings.deepseek.timeout_seconds);
            if let Some(ref base_url) = settings.deepseek.base_url {
                deepseek_config = deepseek_config.with_base_url(base_url);
            }
            match DeepSeekProvider::new(deepseek_config) {
                Ok(provider) => {
                    tracing::info!("DeepSeek provider initialized");
                    provider_router.register(Arc::new(provider));
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize DeepSeek provider: {}. DeepSeek will be disabled.", e);
                }
            }
        } else {
            tracing::debug!("DeepSeek disabled or no API key configured");
        }

        let provider_router = Arc::new(provider_router);

        tracing::info!("Application state initialized successfully");

        Ok(Self {
            settings,
            dynamodb,
            storage,
            bedrock,
            usage_tracker,
            start_time,
            ptc_service,
            gemini_service,
            provider_router,
        })
    }

    /// Get the application uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Check if PTC is enabled
    pub fn is_ptc_enabled(&self) -> bool {
        self.settings.features.enable_ptc
    }

    /// Check if API key authentication is required
    pub fn requires_api_key(&self) -> bool {
        self.settings.require_api_key
    }

    /// Check if Gemini is available
    pub fn is_gemini_available(&self) -> bool {
        self.gemini_service.is_some()
    }

    /// Check the health of AWS services
    ///
    /// Returns a struct with the health status of DynamoDB and Bedrock.
    pub async fn check_aws_health(&self) -> AwsHealthStatus {
        let dynamodb_healthy = self.dynamodb.health_check().await;
        let bedrock_healthy = self.bedrock.health_check();
        let gemini_healthy = self.gemini_service
            .as_ref()
            .map(|s| s.health_check())
            .unwrap_or(false);

        AwsHealthStatus {
            dynamodb: dynamodb_healthy,
            bedrock: bedrock_healthy,
            gemini: gemini_healthy,
        }
    }
}

/// Health status of backend services
#[derive(Debug, Clone, serde::Serialize)]
pub struct AwsHealthStatus {
    pub dynamodb: bool,
    pub bedrock: bool,
    pub gemini: bool,
}

impl AwsHealthStatus {
    /// Check if all core services are healthy (DynamoDB + at least one backend)
    pub fn all_healthy(&self) -> bool {
        self.dynamodb && (self.bedrock || self.gemini)
    }
}
