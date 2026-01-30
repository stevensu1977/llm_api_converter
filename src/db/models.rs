//! DynamoDB data models
//!
//! This module defines the data structures for DynamoDB tables.

use aws_sdk_dynamodb::types::AttributeValue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API key model for authentication and rate limiting.
///
/// Stored in the api_keys table with `api_key` as partition key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// The API key (partition key), format: sk-{uuid}
    pub api_key: String,

    /// User identifier
    pub user_id: String,

    /// Human-readable name for the key
    pub name: String,

    /// Unix timestamp when the key was created
    pub created_at: i64,

    /// Unix timestamp when the key was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,

    /// Whether the key is active
    pub is_active: bool,

    /// Rate limit (requests per window)
    pub rate_limit: i32,

    /// Service tier for pricing ('default', 'flex', 'priority', 'reserved')
    pub service_tier: String,

    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Display name for the owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,

    /// Role type (e.g., "Admin", "Full Access")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Monthly budget limit in USD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_budget: Option<f64>,

    /// Total cumulative budget used (never resets)
    #[serde(default)]
    pub budget_used: f64,

    /// Month-to-date budget used (resets monthly)
    #[serde(default)]
    pub budget_used_mtd: f64,

    /// Month for MTD tracking (YYYY-MM format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_mtd_month: Option<String>,

    /// Reason for deactivation (e.g., "budget_exceeded")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivated_reason: Option<String>,

    /// Tokens per minute limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tpm_limit: Option<i32>,
}

impl ApiKey {
    /// Check if the key is valid (active and not deactivated)
    pub fn is_valid(&self) -> bool {
        self.is_active
    }

    /// Check if the key was deactivated due to budget exceeded
    pub fn is_budget_exceeded(&self) -> bool {
        self.deactivated_reason.as_deref() == Some("budget_exceeded")
    }

    /// Parse from DynamoDB item
    pub fn from_dynamodb(item: &HashMap<String, AttributeValue>) -> Option<Self> {
        Some(Self {
            api_key: get_string(item, "api_key")?,
            user_id: get_string(item, "user_id")?,
            name: get_string(item, "name").unwrap_or_default(),
            created_at: get_number(item, "created_at").unwrap_or(0),
            updated_at: get_number(item, "updated_at"),
            is_active: get_bool(item, "is_active").unwrap_or(false),
            rate_limit: get_number(item, "rate_limit").unwrap_or(100) as i32,
            service_tier: get_string(item, "service_tier").unwrap_or_else(|| "default".to_string()),
            metadata: HashMap::new(), // TODO: Parse metadata map
            owner_name: get_string(item, "owner_name"),
            role: get_string(item, "role"),
            monthly_budget: get_number_f64(item, "monthly_budget"),
            budget_used: get_number_f64(item, "budget_used").unwrap_or(0.0),
            budget_used_mtd: get_number_f64(item, "budget_used_mtd").unwrap_or(0.0),
            budget_mtd_month: get_string(item, "budget_mtd_month"),
            deactivated_reason: get_string(item, "deactivated_reason"),
            tpm_limit: get_number(item, "tpm_limit").map(|n| n as i32),
        })
    }
}

/// Usage record for tracking API usage per request.
///
/// Stored in the usage table with `api_key` as partition key and `timestamp` as sort key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// API key (partition key)
    pub api_key: String,

    /// ISO 8601 timestamp (sort key)
    pub timestamp: String,

    /// Unique request identifier
    pub request_id: String,

    /// Model used for the request
    pub model: String,

    /// Number of input tokens
    pub input_tokens: i64,

    /// Number of output tokens
    pub output_tokens: i64,

    /// Number of cached input tokens
    #[serde(default)]
    pub cached_tokens: i64,

    /// Number of cache write tokens
    #[serde(default)]
    pub cache_write_tokens: i64,

    /// Whether the request was successful
    pub success: bool,

    /// Request duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,

    /// Error message if the request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl UsageRecord {
    /// Convert to DynamoDB item
    pub fn to_dynamodb(&self) -> HashMap<String, AttributeValue> {
        let mut item = HashMap::new();
        item.insert("api_key".to_string(), AttributeValue::S(self.api_key.clone()));
        item.insert("timestamp".to_string(), AttributeValue::S(self.timestamp.clone()));
        item.insert("request_id".to_string(), AttributeValue::S(self.request_id.clone()));
        item.insert("model".to_string(), AttributeValue::S(self.model.clone()));
        item.insert("input_tokens".to_string(), AttributeValue::N(self.input_tokens.to_string()));
        item.insert("output_tokens".to_string(), AttributeValue::N(self.output_tokens.to_string()));
        item.insert("cached_tokens".to_string(), AttributeValue::N(self.cached_tokens.to_string()));
        item.insert("cache_write_tokens".to_string(), AttributeValue::N(self.cache_write_tokens.to_string()));
        item.insert("success".to_string(), AttributeValue::Bool(self.success));

        if let Some(duration_ms) = self.duration_ms {
            item.insert("duration_ms".to_string(), AttributeValue::N(duration_ms.to_string()));
        }
        if let Some(ref error_message) = self.error_message {
            item.insert("error_message".to_string(), AttributeValue::S(error_message.clone()));
        }

        item
    }

    /// Parse from DynamoDB item
    pub fn from_dynamodb(item: &HashMap<String, AttributeValue>) -> Option<Self> {
        Some(Self {
            api_key: get_string(item, "api_key")?,
            timestamp: get_string(item, "timestamp")?,
            request_id: get_string(item, "request_id")?,
            model: get_string(item, "model").unwrap_or_default(),
            input_tokens: get_number(item, "input_tokens").unwrap_or(0),
            output_tokens: get_number(item, "output_tokens").unwrap_or(0),
            cached_tokens: get_number(item, "cached_tokens").unwrap_or(0),
            cache_write_tokens: get_number(item, "cache_write_tokens").unwrap_or(0),
            success: get_bool(item, "success").unwrap_or(false),
            duration_ms: get_number(item, "duration_ms"),
            error_message: get_string(item, "error_message"),
        })
    }
}

/// Aggregated usage statistics per API key.
///
/// Stored in the usage_stats table with `api_key` as partition key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// API key (partition key)
    pub api_key: String,

    /// Total input tokens across all requests
    pub total_input_tokens: i64,

    /// Total output tokens across all requests
    pub total_output_tokens: i64,

    /// Total cached tokens
    pub total_cached_tokens: i64,

    /// Total cache write tokens
    pub total_cache_write_tokens: i64,

    /// Total number of requests
    pub total_requests: i64,

    /// Timestamp of last aggregation (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_aggregated_timestamp: Option<String>,
}

impl UsageStats {
    /// Parse from DynamoDB item
    pub fn from_dynamodb(item: &HashMap<String, AttributeValue>) -> Option<Self> {
        Some(Self {
            api_key: get_string(item, "api_key")?,
            total_input_tokens: get_number(item, "total_input_tokens").unwrap_or(0),
            total_output_tokens: get_number(item, "total_output_tokens").unwrap_or(0),
            total_cached_tokens: get_number(item, "total_cached_tokens").unwrap_or(0),
            total_cache_write_tokens: get_number(item, "total_cache_write_tokens").unwrap_or(0),
            total_requests: get_number(item, "total_requests").unwrap_or(0),
            last_aggregated_timestamp: get_string(item, "last_aggregated_timestamp"),
        })
    }
}

/// Model mapping from Anthropic model ID to Bedrock model ID.
///
/// Stored in the model_mapping table with `anthropic_model_id` as partition key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// Anthropic model ID (partition key)
    pub anthropic_model_id: String,

    /// Bedrock model ID (ARN format)
    pub bedrock_model_id: String,
}

impl ModelMapping {
    /// Parse from DynamoDB item
    pub fn from_dynamodb(item: &HashMap<String, AttributeValue>) -> Option<Self> {
        Some(Self {
            anthropic_model_id: get_string(item, "anthropic_model_id")?,
            bedrock_model_id: get_string(item, "bedrock_model_id")?,
        })
    }

    /// Convert to DynamoDB item
    pub fn to_dynamodb(&self) -> HashMap<String, AttributeValue> {
        let mut item = HashMap::new();
        item.insert(
            "anthropic_model_id".to_string(),
            AttributeValue::S(self.anthropic_model_id.clone()),
        );
        item.insert(
            "bedrock_model_id".to_string(),
            AttributeValue::S(self.bedrock_model_id.clone()),
        );
        item
    }
}

/// Model pricing information.
///
/// Stored in the model_pricing table with `model_id` as partition key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model ID (Bedrock format, partition key)
    pub model_id: String,

    /// Provider name (e.g., "anthropic")
    pub provider: String,

    /// Display name for the model
    pub display_name: String,

    /// Price per 1M input tokens
    pub input_price: f64,

    /// Price per 1M output tokens
    pub output_price: f64,

    /// Price per 1M cache read tokens
    #[serde(default)]
    pub cache_read_price: f64,

    /// Price per 1M cache write tokens
    #[serde(default)]
    pub cache_write_price: f64,

    /// Model status (e.g., "active", "deprecated")
    pub status: String,
}

impl ModelPricing {
    /// Parse from DynamoDB item
    pub fn from_dynamodb(item: &HashMap<String, AttributeValue>) -> Option<Self> {
        Some(Self {
            model_id: get_string(item, "model_id")?,
            provider: get_string(item, "provider").unwrap_or_default(),
            display_name: get_string(item, "display_name").unwrap_or_default(),
            input_price: get_number_f64(item, "input_price").unwrap_or(0.0),
            output_price: get_number_f64(item, "output_price").unwrap_or(0.0),
            cache_read_price: get_number_f64(item, "cache_read_price").unwrap_or(0.0),
            cache_write_price: get_number_f64(item, "cache_write_price").unwrap_or(0.0),
            status: get_string(item, "status").unwrap_or_else(|| "active".to_string()),
        })
    }
}

// Helper functions for parsing DynamoDB AttributeValues

fn get_string(item: &HashMap<String, AttributeValue>, key: &str) -> Option<String> {
    item.get(key).and_then(|v| v.as_s().ok()).map(|s| s.to_string())
}

fn get_number(item: &HashMap<String, AttributeValue>, key: &str) -> Option<i64> {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse().ok())
}

fn get_number_f64(item: &HashMap<String, AttributeValue>, key: &str) -> Option<f64> {
    item.get(key)
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse().ok())
}

fn get_bool(item: &HashMap<String, AttributeValue>, key: &str) -> Option<bool> {
    item.get(key).and_then(|v| v.as_bool().ok()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_is_valid() {
        let key = ApiKey {
            api_key: "sk-test".to_string(),
            user_id: "user1".to_string(),
            name: "Test Key".to_string(),
            created_at: 0,
            updated_at: None,
            is_active: true,
            rate_limit: 100,
            service_tier: "default".to_string(),
            metadata: HashMap::new(),
            owner_name: None,
            role: None,
            monthly_budget: None,
            budget_used: 0.0,
            budget_used_mtd: 0.0,
            budget_mtd_month: None,
            deactivated_reason: None,
            tpm_limit: None,
        };

        assert!(key.is_valid());
    }

    #[test]
    fn test_api_key_budget_exceeded() {
        let key = ApiKey {
            api_key: "sk-test".to_string(),
            user_id: "user1".to_string(),
            name: "Test Key".to_string(),
            created_at: 0,
            updated_at: None,
            is_active: false,
            rate_limit: 100,
            service_tier: "default".to_string(),
            metadata: HashMap::new(),
            owner_name: None,
            role: None,
            monthly_budget: Some(100.0),
            budget_used: 100.0,
            budget_used_mtd: 100.0,
            budget_mtd_month: Some("2024-01".to_string()),
            deactivated_reason: Some("budget_exceeded".to_string()),
            tpm_limit: None,
        };

        assert!(!key.is_valid());
        assert!(key.is_budget_exceeded());
    }

    #[test]
    fn test_usage_record_to_dynamodb() {
        let record = UsageRecord {
            api_key: "sk-test".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            request_id: "req-123".to_string(),
            model: "claude-3-sonnet".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cached_tokens: 0,
            cache_write_tokens: 0,
            success: true,
            duration_ms: Some(500),
            error_message: None,
        };

        let item = record.to_dynamodb();
        assert_eq!(item.get("api_key").unwrap().as_s().unwrap(), "sk-test");
        assert_eq!(item.get("input_tokens").unwrap().as_n().unwrap(), "100");
    }
}
