//! Application state container
//!
//! This module defines the shared application state that is passed
//! to all request handlers via Axum's state extraction.

use crate::config::{create_bedrock_client, create_dynamodb_client, Settings};
use crate::db::DynamoDbClient;
use crate::services::{BedrockService, GeminiConfig as GeminiServiceConfig, GeminiService, PtcService, UsageTracker};
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

    /// DynamoDB client for database operations
    pub dynamodb: Arc<DynamoDbClient>,

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

        tracing::debug!("Creating Bedrock client");
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
            tracing::info!("Gemini enabled, initializing Gemini service");
            let gemini_config = GeminiServiceConfig::new(
                settings.gemini.api_key.clone().unwrap_or_default(),
            );
            let gemini_config = if let Some(ref base_url) = settings.gemini.base_url {
                gemini_config.with_base_url(base_url)
            } else {
                gemini_config
            };

            match GeminiService::new(gemini_config) {
                Ok(service) => Some(Arc::new(service)),
                Err(e) => {
                    tracing::warn!("Failed to initialize Gemini service: {}. Gemini will be disabled.", e);
                    None
                }
            }
        } else {
            tracing::debug!("Gemini disabled or no API key configured");
            None
        };

        tracing::info!("Application state initialized successfully");

        Ok(Self {
            settings,
            dynamodb,
            bedrock,
            usage_tracker,
            start_time,
            ptc_service,
            gemini_service,
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
