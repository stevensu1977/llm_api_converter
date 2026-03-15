//! DynamoDB storage backend — wraps existing repositories into the StorageBackend trait.

use std::sync::Arc;

use crate::db::models::{ApiKey, UsageRecord};
use crate::db::repositories::{ApiKeyRepository, ModelMappingRepository, UsageRepository};
use crate::db::storage::{StorageBackend, StorageError};
use crate::db::DynamoDbClient;

/// DynamoDB implementation of StorageBackend.
///
/// Thin adapter that delegates to existing repository implementations.
pub struct DynamoDbBackend {
    api_keys: ApiKeyRepository,
    usage: UsageRepository,
    model_mapping: ModelMappingRepository,
    client: Arc<DynamoDbClient>,
}

impl DynamoDbBackend {
    pub fn new(client: Arc<DynamoDbClient>) -> Self {
        Self {
            api_keys: ApiKeyRepository::new(client.clone()),
            usage: UsageRepository::new(client.clone()),
            model_mapping: ModelMappingRepository::new(client.clone()),
            client,
        }
    }

    /// Get a reference to the underlying DynamoDbClient (for health checks, etc.)
    pub fn client(&self) -> &DynamoDbClient {
        &self.client
    }
}

#[async_trait::async_trait]
impl StorageBackend for DynamoDbBackend {
    async fn validate_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError> {
        self.api_keys
            .validate_api_key(key)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn get_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError> {
        self.api_keys
            .get_api_key(key)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn increment_budget_used(&self, key: &str, amount: f64) -> Result<bool, StorageError> {
        self.api_keys
            .increment_budget_used(key, amount)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn deactivate_api_key(
        &self,
        key: &str,
        reason: Option<&str>,
    ) -> Result<(), StorageError> {
        self.api_keys
            .deactivate_api_key(key, reason)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn record_usage(&self, record: &UsageRecord) -> Result<(), StorageError> {
        self.usage
            .record_usage(record)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn get_usage_by_api_key(
        &self,
        key: &str,
        start: Option<&str>,
        end: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<UsageRecord>, StorageError> {
        self.usage
            .get_usage_by_api_key(key, start, end, limit)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn get_model_mapping(&self, model_id: &str) -> Result<Option<String>, StorageError> {
        self.model_mapping
            .get_bedrock_model_id(model_id)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn set_model_mapping(&self, from: &str, to: &str) -> Result<(), StorageError> {
        self.model_mapping
            .set_mapping(from, to)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))
    }

    async fn health_check(&self) -> bool {
        self.client.health_check().await
    }
}
