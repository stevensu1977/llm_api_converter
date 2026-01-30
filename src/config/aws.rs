//! AWS SDK configuration
//!
//! This module provides AWS SDK configuration for Bedrock and DynamoDB clients,
//! supporting custom endpoints for local development and testing.

use aws_config::{meta::region::RegionProviderChain, BehaviorVersion, Region, SdkConfig};
use aws_sdk_bedrockruntime::Client as BedrockRuntimeClient;
use aws_sdk_dynamodb::Client as DynamoDbSdkClient;

use crate::config::Settings;

/// AWS configuration builder
///
/// Creates AWS SDK configuration with support for:
/// - Custom regions
/// - Credential providers (environment, instance profile, etc.)
/// - Custom endpoint URLs for local testing
pub struct AwsConfigBuilder<'a> {
    settings: &'a Settings,
}

impl<'a> AwsConfigBuilder<'a> {
    /// Create a new AWS configuration builder
    pub fn new(settings: &'a Settings) -> Self {
        Self { settings }
    }

    /// Build the base AWS SDK configuration
    ///
    /// This configuration is used as the foundation for all AWS service clients.
    /// It handles:
    /// - Region configuration from settings
    /// - Credential chain (env vars, instance profile, etc.)
    pub async fn build_sdk_config(&self) -> SdkConfig {
        let region_provider = RegionProviderChain::first_try(Region::new(self.settings.aws_region.clone()))
            .or_default_provider();

        aws_config::defaults(BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await
    }

    /// Create a DynamoDB client with optional custom endpoint
    ///
    /// If `DYNAMODB_ENDPOINT_URL` is set in settings, the client will use
    /// that endpoint (useful for DynamoDB Local or LocalStack).
    pub async fn build_dynamodb_client(&self) -> DynamoDbSdkClient {
        let sdk_config = self.build_sdk_config().await;

        if let Some(endpoint_url) = &self.settings.dynamodb_endpoint_url {
            tracing::info!(endpoint = %endpoint_url, "Using custom DynamoDB endpoint");

            let dynamodb_config = aws_sdk_dynamodb::config::Builder::from(&sdk_config)
                .endpoint_url(endpoint_url)
                .build();

            DynamoDbSdkClient::from_conf(dynamodb_config)
        } else {
            DynamoDbSdkClient::new(&sdk_config)
        }
    }

    /// Create a Bedrock Runtime client with optional custom endpoint
    ///
    /// If `BEDROCK_ENDPOINT_URL` is set in settings, the client will use
    /// that endpoint (useful for testing with mocks).
    pub async fn build_bedrock_client(&self) -> BedrockRuntimeClient {
        let sdk_config = self.build_sdk_config().await;

        if let Some(endpoint_url) = &self.settings.bedrock_endpoint_url {
            tracing::info!(endpoint = %endpoint_url, "Using custom Bedrock endpoint");

            let bedrock_config = aws_sdk_bedrockruntime::config::Builder::from(&sdk_config)
                .endpoint_url(endpoint_url)
                .build();

            BedrockRuntimeClient::from_conf(bedrock_config)
        } else {
            BedrockRuntimeClient::new(&sdk_config)
        }
    }
}

/// Build AWS SDK config from settings (convenience function)
pub async fn build_aws_config(settings: &Settings) -> SdkConfig {
    AwsConfigBuilder::new(settings).build_sdk_config().await
}

/// Create a DynamoDB client from settings (convenience function)
pub async fn create_dynamodb_client(settings: &Settings) -> DynamoDbSdkClient {
    AwsConfigBuilder::new(settings).build_dynamodb_client().await
}

/// Create a Bedrock Runtime client from settings (convenience function)
pub async fn create_bedrock_client(settings: &Settings) -> BedrockRuntimeClient {
    AwsConfigBuilder::new(settings).build_bedrock_client().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_sdk_config() {
        let settings = Settings::default();
        let config = build_aws_config(&settings).await;

        // Verify region is set
        assert!(config.region().is_some());
        assert_eq!(config.region().unwrap().as_ref(), "us-east-1");
    }

    #[tokio::test]
    async fn test_dynamodb_client_creation() {
        let settings = Settings::default();
        let _client = create_dynamodb_client(&settings).await;
        // Client created successfully
    }

    #[tokio::test]
    async fn test_bedrock_client_creation() {
        let settings = Settings::default();
        let _client = create_bedrock_client(&settings).await;
        // Client created successfully
    }

    #[tokio::test]
    async fn test_custom_endpoint_dynamodb() {
        let mut settings = Settings::default();
        settings.dynamodb_endpoint_url = Some("http://localhost:8001".to_string());

        let _client = create_dynamodb_client(&settings).await;
        // Client created with custom endpoint
    }
}
