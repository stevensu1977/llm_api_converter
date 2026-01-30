//! API key repository
//!
//! Data access layer for API key operations.

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::Utc;
use std::sync::Arc;

use crate::db::models::ApiKey;
use crate::db::DynamoDbClient;

/// Repository for API key operations
#[derive(Clone)]
pub struct ApiKeyRepository {
    client: Arc<DynamoDbClient>,
}

impl ApiKeyRepository {
    /// Create a new API key repository
    pub fn new(client: Arc<DynamoDbClient>) -> Self {
        Self { client }
    }

    /// Validate an API key and return its details
    ///
    /// Auto-reactivates keys that were deactivated due to budget_exceeded
    /// if a new month has started.
    pub async fn validate_api_key(&self, api_key: &str) -> Result<Option<ApiKey>, ApiKeyError> {
        let result = self
            .client
            .client()
            .get_item()
            .table_name(self.client.api_keys_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .send()
            .await
            .map_err(|e| ApiKeyError::DynamoDb(e.to_string()))?;

        let Some(item) = result.item else {
            return Ok(None);
        };

        let Some(key) = ApiKey::from_dynamodb(&item) else {
            return Err(ApiKeyError::ParseError("Failed to parse API key".to_string()));
        };

        // If key is active, return it
        if key.is_active {
            return Ok(Some(key));
        }

        // Check if key was deactivated due to budget exceeded
        // and if a new month has started - auto-reactivate it
        if key.is_budget_exceeded() {
            let current_month = Utc::now().format("%Y-%m").to_string();
            if key.budget_mtd_month.as_deref() != Some(&current_month) {
                // New month - reactivate
                self.reactivate_for_new_month(api_key, &current_month).await?;
                // Fetch updated item
                return self.get_api_key(api_key).await;
            }
        }

        // Key is not active and not eligible for reactivation
        Ok(None)
    }

    /// Get an API key by its value (without validation)
    pub async fn get_api_key(&self, api_key: &str) -> Result<Option<ApiKey>, ApiKeyError> {
        let result = self
            .client
            .client()
            .get_item()
            .table_name(self.client.api_keys_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .send()
            .await
            .map_err(|e| ApiKeyError::DynamoDb(e.to_string()))?;

        match result.item {
            Some(item) => Ok(ApiKey::from_dynamodb(&item)),
            None => Ok(None),
        }
    }

    /// Reactivate an API key for a new month
    async fn reactivate_for_new_month(
        &self,
        api_key: &str,
        current_month: &str,
    ) -> Result<(), ApiKeyError> {
        let now = Utc::now().timestamp();

        self.client
            .client()
            .update_item()
            .table_name(self.client.api_keys_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .update_expression(
                "SET is_active = :active, budget_used_mtd = :zero, \
                 budget_mtd_month = :month, deactivated_reason = :null, \
                 updated_at = :updated_at",
            )
            .expression_attribute_values(":active", AttributeValue::Bool(true))
            .expression_attribute_values(":zero", AttributeValue::N("0".to_string()))
            .expression_attribute_values(":month", AttributeValue::S(current_month.to_string()))
            .expression_attribute_values(":null", AttributeValue::Null(true))
            .expression_attribute_values(":updated_at", AttributeValue::N(now.to_string()))
            .send()
            .await
            .map_err(|e| ApiKeyError::DynamoDb(e.to_string()))?;

        tracing::info!(
            api_key = %&api_key[..20.min(api_key.len())],
            month = %current_month,
            "Auto-reactivated key for new month"
        );

        Ok(())
    }

    /// Increment budget used for an API key
    ///
    /// Updates both total budget_used and month-to-date budget_used_mtd.
    /// If the MTD exceeds monthly_budget, the key is deactivated.
    pub async fn increment_budget_used(
        &self,
        api_key: &str,
        amount: f64,
    ) -> Result<bool, ApiKeyError> {
        let current_month = Utc::now().format("%Y-%m").to_string();
        let now = Utc::now().timestamp();

        // First, get current key to check budget
        let key = self.get_api_key(api_key).await?;
        let Some(key) = key else {
            return Err(ApiKeyError::NotFound);
        };

        // Check if we need to reset MTD for new month
        let should_reset_mtd = key.budget_mtd_month.as_deref() != Some(&current_month);

        // Calculate new values
        let new_budget_used = key.budget_used + amount;
        let new_budget_used_mtd = if should_reset_mtd {
            amount
        } else {
            key.budget_used_mtd + amount
        };

        // Update the key
        let mut update_expr = "SET budget_used = :budget_used, \
            budget_used_mtd = :budget_used_mtd, \
            budget_mtd_month = :month, \
            updated_at = :updated_at"
            .to_string();

        let mut request = self
            .client
            .client()
            .update_item()
            .table_name(self.client.api_keys_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .expression_attribute_values(":budget_used", AttributeValue::N(new_budget_used.to_string()))
            .expression_attribute_values(":budget_used_mtd", AttributeValue::N(new_budget_used_mtd.to_string()))
            .expression_attribute_values(":month", AttributeValue::S(current_month.clone()))
            .expression_attribute_values(":updated_at", AttributeValue::N(now.to_string()));

        // Check if budget exceeded
        let budget_exceeded = key.monthly_budget
            .map(|budget| new_budget_used_mtd >= budget)
            .unwrap_or(false);

        if budget_exceeded {
            update_expr.push_str(", is_active = :inactive, deactivated_reason = :reason");
            request = request
                .expression_attribute_values(":inactive", AttributeValue::Bool(false))
                .expression_attribute_values(":reason", AttributeValue::S("budget_exceeded".to_string()));

            tracing::warn!(
                api_key = %&api_key[..20.min(api_key.len())],
                budget_used_mtd = new_budget_used_mtd,
                monthly_budget = ?key.monthly_budget,
                "Deactivating key due to budget exceeded"
            );
        }

        request
            .update_expression(update_expr)
            .send()
            .await
            .map_err(|e| ApiKeyError::DynamoDb(e.to_string()))?;

        Ok(budget_exceeded)
    }

    /// Deactivate an API key
    pub async fn deactivate_api_key(
        &self,
        api_key: &str,
        reason: Option<&str>,
    ) -> Result<(), ApiKeyError> {
        let now = Utc::now().timestamp();

        let mut update_expr = "SET is_active = :inactive, updated_at = :updated_at".to_string();

        let mut request = self
            .client
            .client()
            .update_item()
            .table_name(self.client.api_keys_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .expression_attribute_values(":inactive", AttributeValue::Bool(false))
            .expression_attribute_values(":updated_at", AttributeValue::N(now.to_string()));

        if let Some(reason) = reason {
            update_expr.push_str(", deactivated_reason = :reason");
            request = request.expression_attribute_values(":reason", AttributeValue::S(reason.to_string()));
        }

        request
            .update_expression(update_expr)
            .send()
            .await
            .map_err(|e| ApiKeyError::DynamoDb(e.to_string()))?;

        Ok(())
    }
}

/// Errors that can occur during API key operations
#[derive(Debug, thiserror::Error)]
pub enum ApiKeyError {
    #[error("DynamoDB error: {0}")]
    DynamoDb(String),

    #[error("API key not found")]
    NotFound,

    #[error("Parse error: {0}")]
    ParseError(String),
}
