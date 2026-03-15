//! SQLite storage backend for local development and self-hosted deployments.
//!
//! Requires the `sqlite` feature flag:
//! ```toml
//! llm-api-converter = { features = ["sqlite"] }
//! ```

#![cfg(feature = "sqlite")]

use chrono::Utc;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

use crate::db::models::{ApiKey, UsageRecord};
use crate::db::storage::{StorageBackend, StorageError};

/// SQLite implementation of StorageBackend.
pub struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    /// Create a new SQLite backend and run migrations.
    ///
    /// # Arguments
    /// * `database_url` - SQLite connection string, e.g. `"sqlite:///data/llm_proxy.db"` or `"sqlite::memory:"`
    pub async fn new(database_url: &str) -> Result<Self, StorageError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        let backend = Self { pool };
        backend.run_migrations().await?;
        Ok(backend)
    }

    async fn run_migrations(&self) -> Result<(), StorageError> {
        let queries = [
            r#"CREATE TABLE IF NOT EXISTS api_keys (
                api_key TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                updated_at INTEGER,
                is_active INTEGER NOT NULL DEFAULT 1,
                rate_limit INTEGER NOT NULL DEFAULT 100,
                service_tier TEXT NOT NULL DEFAULT 'default',
                owner_name TEXT,
                role TEXT,
                monthly_budget REAL,
                budget_used REAL NOT NULL DEFAULT 0.0,
                budget_used_mtd REAL NOT NULL DEFAULT 0.0,
                budget_mtd_month TEXT,
                deactivated_reason TEXT,
                tpm_limit INTEGER
            )"#,
            r#"CREATE TABLE IF NOT EXISTS usage_records (
                api_key TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                request_id TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT '',
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                cached_tokens INTEGER NOT NULL DEFAULT 0,
                cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                error_message TEXT,
                PRIMARY KEY (api_key, timestamp)
            )"#,
            r#"CREATE TABLE IF NOT EXISTS model_mappings (
                source_model_id TEXT PRIMARY KEY,
                target_model_id TEXT NOT NULL
            )"#,
        ];

        for query in &queries {
            sqlx::query(query)
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::Query(e.to_string()))?;
        }

        Ok(())
    }

    fn row_to_api_key(row: &sqlx::sqlite::SqliteRow) -> ApiKey {
        use sqlx::Row;
        ApiKey {
            api_key: row.get("api_key"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            is_active: row.get::<i32, _>("is_active") != 0,
            rate_limit: row.get("rate_limit"),
            service_tier: row.get("service_tier"),
            metadata: std::collections::HashMap::new(),
            owner_name: row.get("owner_name"),
            role: row.get("role"),
            monthly_budget: row.get("monthly_budget"),
            budget_used: row.get("budget_used"),
            budget_used_mtd: row.get("budget_used_mtd"),
            budget_mtd_month: row.get("budget_mtd_month"),
            deactivated_reason: row.get("deactivated_reason"),
            tpm_limit: row.get("tpm_limit"),
        }
    }

    fn row_to_usage(row: &sqlx::sqlite::SqliteRow) -> UsageRecord {
        use sqlx::Row;
        UsageRecord {
            api_key: row.get("api_key"),
            timestamp: row.get("timestamp"),
            request_id: row.get("request_id"),
            model: row.get("model"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            cached_tokens: row.get("cached_tokens"),
            cache_write_tokens: row.get("cache_write_tokens"),
            success: row.get::<i32, _>("success") != 0,
            duration_ms: row.get("duration_ms"),
            error_message: row.get("error_message"),
        }
    }
}

#[async_trait::async_trait]
impl StorageBackend for SqliteBackend {
    async fn validate_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE api_key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let api_key = Self::row_to_api_key(&row);

        if api_key.is_active {
            return Ok(Some(api_key));
        }

        // Auto-reactivate if budget_exceeded and new month
        if api_key.is_budget_exceeded() {
            let current_month = Utc::now().format("%Y-%m").to_string();
            if api_key.budget_mtd_month.as_deref() != Some(&current_month) {
                let now = Utc::now().timestamp();
                sqlx::query(
                    "UPDATE api_keys SET is_active = 1, budget_used_mtd = 0, \
                     budget_mtd_month = ?, deactivated_reason = NULL, updated_at = ? \
                     WHERE api_key = ?",
                )
                .bind(&current_month)
                .bind(now)
                .bind(key)
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::Query(e.to_string()))?;

                return self.get_api_key(key).await;
            }
        }

        Ok(None)
    }

    async fn get_api_key(&self, key: &str) -> Result<Option<ApiKey>, StorageError> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE api_key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(row.as_ref().map(Self::row_to_api_key))
    }

    async fn increment_budget_used(&self, key: &str, amount: f64) -> Result<bool, StorageError> {
        let api_key = self
            .get_api_key(key)
            .await?
            .ok_or(StorageError::NotFound)?;

        let current_month = Utc::now().format("%Y-%m").to_string();
        let now = Utc::now().timestamp();

        let should_reset = api_key.budget_mtd_month.as_deref() != Some(&current_month);
        let new_budget_used = api_key.budget_used + amount;
        let new_budget_used_mtd = if should_reset {
            amount
        } else {
            api_key.budget_used_mtd + amount
        };

        let budget_exceeded = api_key
            .monthly_budget
            .map(|budget| new_budget_used_mtd >= budget)
            .unwrap_or(false);

        if budget_exceeded {
            sqlx::query(
                "UPDATE api_keys SET budget_used = ?, budget_used_mtd = ?, \
                 budget_mtd_month = ?, updated_at = ?, is_active = 0, \
                 deactivated_reason = 'budget_exceeded' WHERE api_key = ?",
            )
            .bind(new_budget_used)
            .bind(new_budget_used_mtd)
            .bind(&current_month)
            .bind(now)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        } else {
            sqlx::query(
                "UPDATE api_keys SET budget_used = ?, budget_used_mtd = ?, \
                 budget_mtd_month = ?, updated_at = ? WHERE api_key = ?",
            )
            .bind(new_budget_used)
            .bind(new_budget_used_mtd)
            .bind(&current_month)
            .bind(now)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        }

        Ok(budget_exceeded)
    }

    async fn deactivate_api_key(
        &self,
        key: &str,
        reason: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE api_keys SET is_active = 0, deactivated_reason = ?, updated_at = ? \
             WHERE api_key = ?",
        )
        .bind(reason)
        .bind(now)
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(())
    }

    async fn record_usage(&self, record: &UsageRecord) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO usage_records (api_key, timestamp, request_id, model, \
             input_tokens, output_tokens, cached_tokens, cache_write_tokens, \
             success, duration_ms, error_message) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&record.api_key)
        .bind(&record.timestamp)
        .bind(&record.request_id)
        .bind(&record.model)
        .bind(record.input_tokens)
        .bind(record.output_tokens)
        .bind(record.cached_tokens)
        .bind(record.cache_write_tokens)
        .bind(record.success as i32)
        .bind(record.duration_ms)
        .bind(&record.error_message)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(())
    }

    async fn get_usage_by_api_key(
        &self,
        key: &str,
        start: Option<&str>,
        end: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<UsageRecord>, StorageError> {
        let limit = limit.unwrap_or(100);

        let rows = match (start, end) {
            (Some(s), Some(e)) => {
                sqlx::query(
                    "SELECT * FROM usage_records WHERE api_key = ? \
                     AND timestamp BETWEEN ? AND ? ORDER BY timestamp DESC LIMIT ?",
                )
                .bind(key)
                .bind(s)
                .bind(e)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
            _ => {
                sqlx::query(
                    "SELECT * FROM usage_records WHERE api_key = ? \
                     ORDER BY timestamp DESC LIMIT ?",
                )
                .bind(key)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(rows.iter().map(Self::row_to_usage).collect())
    }

    async fn get_model_mapping(&self, model_id: &str) -> Result<Option<String>, StorageError> {
        let row = sqlx::query("SELECT target_model_id FROM model_mappings WHERE source_model_id = ?")
            .bind(model_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(row.map(|r| {
            use sqlx::Row;
            r.get("target_model_id")
        }))
    }

    async fn set_model_mapping(&self, from: &str, to: &str) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT OR REPLACE INTO model_mappings (source_model_id, target_model_id) VALUES (?, ?)",
        )
        .bind(from)
        .bind(to)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(())
    }

    async fn health_check(&self) -> bool {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_backend() -> SqliteBackend {
        SqliteBackend::new("sqlite::memory:").await.unwrap()
    }

    async fn insert_test_key(backend: &SqliteBackend, key: &str, active: bool) {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO api_keys (api_key, user_id, name, created_at, is_active, rate_limit, service_tier) \
             VALUES (?, 'test_user', 'Test Key', ?, ?, 100, 'default')",
        )
        .bind(key)
        .bind(now)
        .bind(active as i32)
        .execute(&backend.pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_health_check() {
        let backend = create_test_backend().await;
        assert!(backend.health_check().await);
    }

    #[tokio::test]
    async fn test_validate_active_key() {
        let backend = create_test_backend().await;
        insert_test_key(&backend, "sk-test-123", true).await;

        let result = backend.validate_api_key("sk-test-123").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().api_key, "sk-test-123");
    }

    #[tokio::test]
    async fn test_validate_inactive_key() {
        let backend = create_test_backend().await;
        insert_test_key(&backend, "sk-test-inactive", false).await;

        let result = backend.validate_api_key("sk-test-inactive").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_validate_nonexistent_key() {
        let backend = create_test_backend().await;
        let result = backend.validate_api_key("sk-does-not-exist").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_record_and_get_usage() {
        let backend = create_test_backend().await;

        let record = UsageRecord {
            api_key: "sk-test".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            request_id: "req-1".to_string(),
            model: "claude-3-sonnet".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cached_tokens: 0,
            cache_write_tokens: 0,
            success: true,
            duration_ms: Some(500),
            error_message: None,
        };

        backend.record_usage(&record).await.unwrap();

        let records = backend
            .get_usage_by_api_key("sk-test", None, None, None)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].input_tokens, 100);
        assert_eq!(records[0].model, "claude-3-sonnet");
    }

    #[tokio::test]
    async fn test_model_mapping() {
        let backend = create_test_backend().await;

        backend
            .set_model_mapping("claude-sonnet-4", "us.anthropic.claude-sonnet-4-v1:0")
            .await
            .unwrap();

        let result = backend.get_model_mapping("claude-sonnet-4").await.unwrap();
        assert_eq!(
            result,
            Some("us.anthropic.claude-sonnet-4-v1:0".to_string())
        );

        let result = backend.get_model_mapping("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_increment_budget() {
        let backend = create_test_backend().await;
        let now = Utc::now().timestamp();
        let month = Utc::now().format("%Y-%m").to_string();

        sqlx::query(
            "INSERT INTO api_keys (api_key, user_id, name, created_at, is_active, rate_limit, \
             service_tier, monthly_budget, budget_used, budget_used_mtd, budget_mtd_month) \
             VALUES ('sk-budget', 'user', 'Key', ?, 1, 100, 'default', 10.0, 0.0, 0.0, ?)",
        )
        .bind(now)
        .bind(&month)
        .execute(&backend.pool)
        .await
        .unwrap();

        // Small increment — should not exceed
        let exceeded = backend
            .increment_budget_used("sk-budget", 5.0)
            .await
            .unwrap();
        assert!(!exceeded);

        // Another increment that exceeds the $10 budget
        let exceeded = backend
            .increment_budget_used("sk-budget", 6.0)
            .await
            .unwrap();
        assert!(exceeded);

        // Key should now be inactive
        let key = backend.get_api_key("sk-budget").await.unwrap().unwrap();
        assert!(!key.is_active);
        assert_eq!(key.deactivated_reason.as_deref(), Some("budget_exceeded"));
    }

    #[tokio::test]
    async fn test_deactivate_key() {
        let backend = create_test_backend().await;
        insert_test_key(&backend, "sk-deactivate", true).await;

        backend
            .deactivate_api_key("sk-deactivate", Some("admin_action"))
            .await
            .unwrap();

        let key = backend
            .get_api_key("sk-deactivate")
            .await
            .unwrap()
            .unwrap();
        assert!(!key.is_active);
        assert_eq!(key.deactivated_reason.as_deref(), Some("admin_action"));
    }
}
