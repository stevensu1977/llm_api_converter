//! Usage repository
//!
//! Data access layer for usage tracking operations.

use aws_sdk_dynamodb::types::AttributeValue;
use std::sync::Arc;

use crate::db::models::{UsageRecord, UsageStats};
use crate::db::DynamoDbClient;

/// Repository for usage tracking operations
#[derive(Clone)]
pub struct UsageRepository {
    client: Arc<DynamoDbClient>,
}

impl UsageRepository {
    /// Create a new usage repository
    pub fn new(client: Arc<DynamoDbClient>) -> Self {
        Self { client }
    }

    /// Record a usage event
    pub async fn record_usage(&self, record: &UsageRecord) -> Result<(), UsageError> {
        self.client
            .client()
            .put_item()
            .table_name(self.client.usage_table())
            .set_item(Some(record.to_dynamodb()))
            .send()
            .await
            .map_err(|e| UsageError::DynamoDb(e.to_string()))?;

        tracing::debug!(
            api_key = %&record.api_key[..20.min(record.api_key.len())],
            request_id = %record.request_id,
            model = %record.model,
            input_tokens = record.input_tokens,
            output_tokens = record.output_tokens,
            "Recorded usage"
        );

        Ok(())
    }

    /// Get usage records for an API key within a time range
    pub async fn get_usage_by_api_key(
        &self,
        api_key: &str,
        start_timestamp: Option<&str>,
        end_timestamp: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<UsageRecord>, UsageError> {
        let mut key_condition = "api_key = :api_key".to_string();
        let mut expr_values = vec![(":api_key", AttributeValue::S(api_key.to_string()))];

        if let (Some(start), Some(end)) = (start_timestamp, end_timestamp) {
            key_condition.push_str(" AND #ts BETWEEN :start AND :end");
            expr_values.push((":start", AttributeValue::S(start.to_string())));
            expr_values.push((":end", AttributeValue::S(end.to_string())));
        } else if let Some(start) = start_timestamp {
            key_condition.push_str(" AND #ts >= :start");
            expr_values.push((":start", AttributeValue::S(start.to_string())));
        }

        let mut query = self
            .client
            .client()
            .query()
            .table_name(self.client.usage_table())
            .key_condition_expression(key_condition)
            .expression_attribute_names("#ts", "timestamp");

        for (name, value) in expr_values {
            query = query.expression_attribute_values(name, value);
        }

        if let Some(limit) = limit {
            query = query.limit(limit);
        }

        // Order by timestamp descending (most recent first)
        query = query.scan_index_forward(false);

        let result = query
            .send()
            .await
            .map_err(|e| UsageError::DynamoDb(e.to_string()))?;

        let records = result
            .items
            .unwrap_or_default()
            .iter()
            .filter_map(|item| UsageRecord::from_dynamodb(item))
            .collect();

        Ok(records)
    }

    /// Get aggregated usage stats for an API key
    pub async fn get_usage_stats(&self, api_key: &str) -> Result<Option<UsageStats>, UsageError> {
        let result = self
            .client
            .client()
            .get_item()
            .table_name(self.client.usage_stats_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .send()
            .await
            .map_err(|e| UsageError::DynamoDb(e.to_string()))?;

        match result.item {
            Some(item) => Ok(UsageStats::from_dynamodb(&item)),
            None => Ok(None),
        }
    }

    /// Update aggregated usage stats (atomic increment)
    pub async fn increment_usage_stats(
        &self,
        api_key: &str,
        input_tokens: i64,
        output_tokens: i64,
        cached_tokens: i64,
        cache_write_tokens: i64,
        timestamp: &str,
    ) -> Result<(), UsageError> {
        self.client
            .client()
            .update_item()
            .table_name(self.client.usage_stats_table())
            .key("api_key", AttributeValue::S(api_key.to_string()))
            .update_expression(
                "SET total_input_tokens = if_not_exists(total_input_tokens, :zero) + :input, \
                 total_output_tokens = if_not_exists(total_output_tokens, :zero) + :output, \
                 total_cached_tokens = if_not_exists(total_cached_tokens, :zero) + :cached, \
                 total_cache_write_tokens = if_not_exists(total_cache_write_tokens, :zero) + :cache_write, \
                 total_requests = if_not_exists(total_requests, :zero) + :one, \
                 last_aggregated_timestamp = :timestamp",
            )
            .expression_attribute_values(":zero", AttributeValue::N("0".to_string()))
            .expression_attribute_values(":one", AttributeValue::N("1".to_string()))
            .expression_attribute_values(":input", AttributeValue::N(input_tokens.to_string()))
            .expression_attribute_values(":output", AttributeValue::N(output_tokens.to_string()))
            .expression_attribute_values(":cached", AttributeValue::N(cached_tokens.to_string()))
            .expression_attribute_values(":cache_write", AttributeValue::N(cache_write_tokens.to_string()))
            .expression_attribute_values(":timestamp", AttributeValue::S(timestamp.to_string()))
            .send()
            .await
            .map_err(|e| UsageError::DynamoDb(e.to_string()))?;

        Ok(())
    }

    /// Get usage records since a timestamp (for incremental aggregation)
    pub async fn get_usage_since(
        &self,
        api_key: &str,
        since_timestamp: &str,
    ) -> Result<Vec<UsageRecord>, UsageError> {
        self.get_usage_by_api_key(api_key, Some(since_timestamp), None, None).await
    }
}

/// Errors that can occur during usage operations
#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("DynamoDB error: {0}")]
    DynamoDb(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}
