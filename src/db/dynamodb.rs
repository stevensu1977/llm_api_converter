//! DynamoDB client wrapper
//!
//! This module provides a wrapper around the AWS DynamoDB SDK client
//! for database operations.

use aws_sdk_dynamodb::Client as DynamoDbSdkClient;
use crate::config::Settings;
use std::sync::Arc;

/// DynamoDB client wrapper for database operations.
///
/// This struct wraps the AWS DynamoDB SDK client and provides
/// application-specific database operations.
#[derive(Clone)]
pub struct DynamoDbClient {
    /// Application settings
    settings: Arc<Settings>,

    /// AWS DynamoDB SDK client
    client: DynamoDbSdkClient,
}

impl DynamoDbClient {
    /// Create a new DynamoDB client.
    ///
    /// # Arguments
    /// * `settings` - Application settings containing DynamoDB configuration
    /// * `client` - AWS DynamoDB SDK client
    pub fn new(settings: Arc<Settings>, client: DynamoDbSdkClient) -> Self {
        Self { settings, client }
    }

    /// Get a reference to the underlying AWS SDK client
    pub fn client(&self) -> &DynamoDbSdkClient {
        &self.client
    }

    /// Get the API keys table name
    pub fn api_keys_table(&self) -> &str {
        &self.settings.dynamodb_api_keys_table
    }

    /// Get the usage table name
    pub fn usage_table(&self) -> &str {
        &self.settings.dynamodb_usage_table
    }

    /// Get the usage stats table name
    pub fn usage_stats_table(&self) -> &str {
        &self.settings.dynamodb_usage_stats_table
    }

    /// Get the model mapping table name
    pub fn model_mapping_table(&self) -> &str {
        &self.settings.dynamodb_model_mapping_table
    }

    /// Get the model pricing table name
    pub fn model_pricing_table(&self) -> &str {
        &self.settings.dynamodb_model_pricing_table
    }

    /// Check if the DynamoDB connection is healthy
    ///
    /// Performs a simple list_tables operation to verify connectivity.
    pub async fn health_check(&self) -> bool {
        match self.client.list_tables().limit(1).send().await {
            Ok(_) => {
                tracing::debug!("DynamoDB health check passed");
                true
            }
            Err(e) => {
                tracing::warn!(error = %e, "DynamoDB health check failed");
                false
            }
        }
    }

    // TODO: Implement validate_api_key() in Phase 4
    // TODO: Implement record_usage() in Phase 4
    // TODO: Implement get_model_mapping() in Phase 2.2
}

#[cfg(test)]
mod tests {
    // Tests will be added in Phase 2.2 with mocking
}
