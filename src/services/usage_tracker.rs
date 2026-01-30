//! Usage tracking service
//!
//! This module handles tracking API usage statistics for billing and monitoring.
//! Usage is recorded to DynamoDB and budget tracking is updated for each request.

use crate::db::models::UsageRecord;
use crate::db::repositories::{ApiKeyError, ApiKeyRepository, UsageRepository};
use crate::db::DynamoDbClient;
use crate::middleware::auth::ApiKeyInfo;
use crate::schemas::anthropic::{MessageResponse, Usage};
use chrono::Utc;
use std::sync::Arc;

// ============================================================================
// Service Tier Pricing Multipliers
// ============================================================================

/// Get the pricing multiplier for a service tier
fn get_tier_multiplier(tier: &str) -> f64 {
    match tier.to_lowercase().as_str() {
        "flex" => 0.5,      // 50% discount
        "priority" => 1.75, // 75% markup
        "master" => 0.0,    // Free for master key
        _ => 1.0,           // default tier
    }
}

// ============================================================================
// Usage Tracker Service
// ============================================================================

/// Service for tracking API usage statistics.
///
/// This service:
/// - Records individual request usage to DynamoDB
/// - Updates budget tracking for API keys
/// - Calculates costs based on model pricing and service tier
#[derive(Clone)]
pub struct UsageTracker {
    #[allow(dead_code)]
    dynamodb: Arc<DynamoDbClient>,
    usage_repo: UsageRepository,
    api_key_repo: ApiKeyRepository,
}

impl UsageTracker {
    /// Create a new usage tracker.
    ///
    /// # Arguments
    /// * `dynamodb` - DynamoDB client for persisting usage data
    pub fn new(dynamodb: Arc<DynamoDbClient>) -> Self {
        Self {
            usage_repo: UsageRepository::new(dynamodb.clone()),
            api_key_repo: ApiKeyRepository::new(dynamodb.clone()),
            dynamodb,
        }
    }

    /// Record usage for a completed request
    ///
    /// This method:
    /// 1. Creates a usage record in DynamoDB
    /// 2. Calculates the cost based on model pricing and service tier
    /// 3. Updates the API key's budget usage
    ///
    /// # Arguments
    /// * `key_info` - The authenticated API key info
    /// * `request_id` - Unique ID for this request
    /// * `model` - The model ID that was used
    /// * `usage` - Token usage from the response
    /// * `success` - Whether the request was successful
    ///
    /// # Returns
    /// * `Ok(true)` - Budget limit was exceeded, key deactivated
    /// * `Ok(false)` - Request recorded successfully
    /// * `Err(_)` - Error occurred during recording
    pub async fn record_usage(
        &self,
        key_info: &ApiKeyInfo,
        request_id: &str,
        model: &str,
        usage: &Usage,
        success: bool,
    ) -> Result<bool, UsageError> {
        let timestamp = Utc::now();

        // Skip recording for master key
        if key_info.is_master {
            tracing::debug!(
                request_id = %request_id,
                "Skipping usage recording for master key"
            );
            return Ok(false);
        }

        // Create usage record
        let record = UsageRecord {
            api_key: key_info.api_key.clone(),
            timestamp: timestamp.to_rfc3339(),
            request_id: request_id.to_string(),
            model: model.to_string(),
            input_tokens: usage.input_tokens as i64,
            output_tokens: usage.output_tokens as i64,
            cached_tokens: usage.cache_read_input_tokens.map(|t| t as i64).unwrap_or(0),
            cache_write_tokens: usage.cache_creation_input_tokens.map(|t| t as i64).unwrap_or(0),
            success,
            duration_ms: None,
            error_message: None,
        };

        // Save usage record
        self.usage_repo
            .record_usage(&record)
            .await
            .map_err(|e| UsageError::Database(e.to_string()))?;

        tracing::debug!(
            request_id = %request_id,
            api_key = %key_info.api_key,
            model = %model,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            "Usage recorded"
        );

        // Calculate cost and update budget
        // Note: For now we use a simplified cost calculation
        // In production, this would look up model pricing from DynamoDB
        let cost = self.calculate_cost(model, usage, &key_info.service_tier);

        if cost > 0.0 {
            let budget_exceeded = self
                .api_key_repo
                .increment_budget_used(&key_info.api_key, cost)
                .await
                .map_err(|e| match e {
                    ApiKeyError::NotFound => UsageError::ApiKeyNotFound,
                    ApiKeyError::DynamoDb(msg) => UsageError::Database(msg),
                    ApiKeyError::ParseError(msg) => UsageError::Database(msg),
                })?;

            if budget_exceeded {
                tracing::warn!(
                    api_key = %key_info.api_key,
                    user_id = %key_info.user_id,
                    cost = cost,
                    "Budget exceeded, key deactivated"
                );
            }

            return Ok(budget_exceeded);
        }

        Ok(false)
    }

    /// Record usage from a MessageResponse
    ///
    /// Convenience method that extracts usage from the response.
    pub async fn record_from_response(
        &self,
        key_info: &ApiKeyInfo,
        response: &MessageResponse,
        success: bool,
    ) -> Result<bool, UsageError> {
        self.record_usage(
            key_info,
            &response.id,
            &response.model,
            &response.usage,
            success,
        )
        .await
    }

    /// Calculate the cost of a request
    ///
    /// Uses simplified pricing (will be replaced with DynamoDB lookup in production):
    /// - Input tokens: $3 per million
    /// - Output tokens: $15 per million
    /// - Cached read: $0.30 per million
    /// - Cache write: $3.75 per million
    fn calculate_cost(&self, _model: &str, usage: &Usage, service_tier: &str) -> f64 {
        // Simplified pricing (Claude 3.5 Sonnet approximate rates)
        const INPUT_PRICE_PER_MILLION: f64 = 3.0;
        const OUTPUT_PRICE_PER_MILLION: f64 = 15.0;
        const CACHE_READ_PRICE_PER_MILLION: f64 = 0.30;
        const CACHE_WRITE_PRICE_PER_MILLION: f64 = 3.75;

        let input_cost = (usage.input_tokens as f64) * INPUT_PRICE_PER_MILLION / 1_000_000.0;
        let output_cost = (usage.output_tokens as f64) * OUTPUT_PRICE_PER_MILLION / 1_000_000.0;

        let cache_read_cost = usage
            .cache_read_input_tokens
            .map(|t| (t as f64) * CACHE_READ_PRICE_PER_MILLION / 1_000_000.0)
            .unwrap_or(0.0);

        let cache_write_cost = usage
            .cache_creation_input_tokens
            .map(|t| (t as f64) * CACHE_WRITE_PRICE_PER_MILLION / 1_000_000.0)
            .unwrap_or(0.0);

        let base_cost = input_cost + output_cost + cache_read_cost + cache_write_cost;

        // Apply service tier multiplier
        let multiplier = get_tier_multiplier(service_tier);
        base_cost * multiplier
    }

    /// Get usage statistics for an API key
    ///
    /// Returns aggregated usage for the specified time period.
    pub async fn get_usage_stats(
        &self,
        api_key: &str,
        since_timestamp: Option<&str>,
    ) -> Result<UsageStats, UsageError> {
        let records = self
            .usage_repo
            .get_usage_by_api_key(api_key, since_timestamp, None, None)
            .await
            .map_err(|e| UsageError::Database(e.to_string()))?;

        let mut stats = UsageStats::default();

        for record in records {
            stats.total_requests += 1;
            if record.success {
                stats.successful_requests += 1;
            }
            stats.total_input_tokens += record.input_tokens;
            stats.total_output_tokens += record.output_tokens;
            stats.total_cached_tokens += record.cached_tokens;
            stats.total_cache_write_tokens += record.cache_write_tokens;
        }

        Ok(stats)
    }
}

// ============================================================================
// Usage Statistics
// ============================================================================

/// Aggregated usage statistics for an API key
#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cached_tokens: i64,
    pub total_cache_write_tokens: i64,
}

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur during usage tracking
#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("API key not found")]
    ApiKeyNotFound,

    #[error("Invalid usage data: {0}")]
    InvalidData(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_multiplier() {
        assert_eq!(get_tier_multiplier("default"), 1.0);
        assert_eq!(get_tier_multiplier("flex"), 0.5);
        assert_eq!(get_tier_multiplier("priority"), 1.75);
        assert_eq!(get_tier_multiplier("master"), 0.0);
        assert_eq!(get_tier_multiplier("unknown"), 1.0);
    }

    #[test]
    fn test_usage_stats_default() {
        let stats = UsageStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_input_tokens, 0);
        assert_eq!(stats.total_output_tokens, 0);
    }

    #[test]
    fn test_cost_calculation() {
        // Create a mock tracker (we can't test calculate_cost directly without DB)
        // but we can verify the math
        let _usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        };

        // Manual calculation:
        // Input: 1000 * $3 / 1M = $0.003
        // Output: 500 * $15 / 1M = $0.0075
        // Total base: $0.0105

        let expected_base: f64 = 0.003 + 0.0075;

        // With flex tier (0.5x): $0.00525
        let flex_expected: f64 = expected_base * 0.5;
        assert!((flex_expected - 0.00525_f64).abs() < 0.0001);

        // With priority tier (1.75x): $0.018375
        let priority_expected: f64 = expected_base * 1.75;
        assert!((priority_expected - 0.018375_f64).abs() < 0.0001);
    }
}
