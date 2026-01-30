//! Model mapping repository
//!
//! Data access layer for model ID mapping operations.

use aws_sdk_dynamodb::types::AttributeValue;
use std::sync::Arc;

use crate::db::models::ModelMapping;
use crate::db::DynamoDbClient;

/// Repository for model mapping operations
#[derive(Clone)]
pub struct ModelMappingRepository {
    client: Arc<DynamoDbClient>,
}

impl ModelMappingRepository {
    /// Create a new model mapping repository
    pub fn new(client: Arc<DynamoDbClient>) -> Self {
        Self { client }
    }

    /// Get Bedrock model ID for an Anthropic model ID
    ///
    /// Returns None if no mapping exists.
    pub async fn get_bedrock_model_id(
        &self,
        anthropic_model_id: &str,
    ) -> Result<Option<String>, ModelMappingError> {
        let result = self
            .client
            .client()
            .get_item()
            .table_name(self.client.model_mapping_table())
            .key(
                "anthropic_model_id",
                AttributeValue::S(anthropic_model_id.to_string()),
            )
            .send()
            .await
            .map_err(|e| ModelMappingError::DynamoDb(e.to_string()))?;

        match result.item {
            Some(item) => {
                let mapping = ModelMapping::from_dynamodb(&item);
                Ok(mapping.map(|m| m.bedrock_model_id))
            }
            None => Ok(None),
        }
    }

    /// Set a model mapping
    pub async fn set_mapping(
        &self,
        anthropic_model_id: &str,
        bedrock_model_id: &str,
    ) -> Result<(), ModelMappingError> {
        let mapping = ModelMapping {
            anthropic_model_id: anthropic_model_id.to_string(),
            bedrock_model_id: bedrock_model_id.to_string(),
        };

        self.client
            .client()
            .put_item()
            .table_name(self.client.model_mapping_table())
            .set_item(Some(mapping.to_dynamodb()))
            .send()
            .await
            .map_err(|e| ModelMappingError::DynamoDb(e.to_string()))?;

        tracing::debug!(
            anthropic_model_id = %anthropic_model_id,
            bedrock_model_id = %bedrock_model_id,
            "Set model mapping"
        );

        Ok(())
    }

    /// Delete a model mapping
    pub async fn delete_mapping(
        &self,
        anthropic_model_id: &str,
    ) -> Result<(), ModelMappingError> {
        self.client
            .client()
            .delete_item()
            .table_name(self.client.model_mapping_table())
            .key(
                "anthropic_model_id",
                AttributeValue::S(anthropic_model_id.to_string()),
            )
            .send()
            .await
            .map_err(|e| ModelMappingError::DynamoDb(e.to_string()))?;

        Ok(())
    }

    /// List all model mappings
    pub async fn list_all(&self) -> Result<Vec<ModelMapping>, ModelMappingError> {
        let result = self
            .client
            .client()
            .scan()
            .table_name(self.client.model_mapping_table())
            .send()
            .await
            .map_err(|e| ModelMappingError::DynamoDb(e.to_string()))?;

        let mappings = result
            .items
            .unwrap_or_default()
            .iter()
            .filter_map(|item| ModelMapping::from_dynamodb(item))
            .collect();

        Ok(mappings)
    }
}

/// Errors that can occur during model mapping operations
#[derive(Debug, thiserror::Error)]
pub enum ModelMappingError {
    #[error("DynamoDB error: {0}")]
    DynamoDb(String),
}
