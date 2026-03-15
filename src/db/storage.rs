//! Storage backend trait abstraction.
//!
//! Defines a provider-agnostic storage interface so the application can use
//! DynamoDB, SQLite, PostgreSQL, or other backends interchangeably.

use crate::db::models::{ApiKey, UsageRecord};

/// Unified storage backend trait.
///
/// All storage operations (API keys, usage, model mappings) go through this trait,
/// enabling the application to work with different database backends.
#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync {
    // ── API Key operations ──────────────────────────────────────────

    /// Validate an API key: return it if active, auto-reactivate if budget-exceeded in new month.
    async fn validate_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError>;

    /// Get an API key without validation logic.
    async fn get_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError>;

    /// Increment budget usage. Returns `true` if budget was exceeded (key deactivated).
    async fn increment_budget_used(&self, key: &str, amount: f64) -> Result<bool, StorageError>;

    /// Deactivate an API key with an optional reason.
    async fn deactivate_api_key(&self, key: &str, reason: Option<&str>)
        -> Result<(), StorageError>;

    // ── Usage operations ────────────────────────────────────────────

    /// Record a usage event.
    async fn record_usage(&self, record: &UsageRecord) -> Result<(), StorageError>;

    /// Get usage records for an API key within an optional time range.
    async fn get_usage_by_api_key(
        &self,
        key: &str,
        start: Option<&str>,
        end: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<UsageRecord>, StorageError>;

    // ── Model mapping operations ────────────────────────────────────

    /// Look up a model mapping (e.g., anthropic model → bedrock model ID).
    async fn get_model_mapping(&self, model_id: &str) -> Result<Option<String>, StorageError>;

    /// Set a model mapping.
    async fn set_model_mapping(&self, from: &str, to: &str) -> Result<(), StorageError>;

    // ── Health ──────────────────────────────────────────────────────

    /// Check if the storage backend is healthy / reachable.
    async fn health_check(&self) -> bool;
}

/// Errors from storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Not found")]
    NotFound,

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Parse error: {0}")]
    Parse(String),
}
