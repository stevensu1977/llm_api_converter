//! Timeout utilities for request handling
//!
//! This module provides timeout configuration and helpers for various operations.

use std::time::Duration;

/// Timeout configuration for different operations
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Timeout for Bedrock API calls (default: 120s for long generations)
    pub bedrock_timeout: Duration,

    /// Timeout for DynamoDB operations (default: 5s)
    pub dynamodb_timeout: Duration,

    /// Timeout for streaming connections (default: 300s)
    pub streaming_timeout: Duration,

    /// Timeout for health checks (default: 5s)
    pub health_check_timeout: Duration,

    /// Connection timeout for HTTP clients (default: 10s)
    pub connect_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            bedrock_timeout: Duration::from_secs(120),
            dynamodb_timeout: Duration::from_secs(5),
            streaming_timeout: Duration::from_secs(300),
            health_check_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(10),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set Bedrock API timeout
    pub fn with_bedrock_timeout(mut self, timeout: Duration) -> Self {
        self.bedrock_timeout = timeout;
        self
    }

    /// Set DynamoDB timeout
    pub fn with_dynamodb_timeout(mut self, timeout: Duration) -> Self {
        self.dynamodb_timeout = timeout;
        self
    }

    /// Set streaming timeout
    pub fn with_streaming_timeout(mut self, timeout: Duration) -> Self {
        self.streaming_timeout = timeout;
        self
    }

    /// Set health check timeout
    pub fn with_health_check_timeout(mut self, timeout: Duration) -> Self {
        self.health_check_timeout = timeout;
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Create config from environment variables with defaults
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = std::env::var("BEDROCK_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                config.bedrock_timeout = Duration::from_secs(secs);
            }
        }

        if let Ok(val) = std::env::var("DYNAMODB_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                config.dynamodb_timeout = Duration::from_secs(secs);
            }
        }

        if let Ok(val) = std::env::var("STREAMING_TIMEOUT_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                config.streaming_timeout = Duration::from_secs(secs);
            }
        }

        config
    }
}

/// Apply timeout to an async operation
///
/// Returns `Err` with the original error type if the operation times out.
pub async fn with_timeout<T, E>(
    timeout: Duration,
    future: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, TimeoutError<E>> {
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(TimeoutError::Inner(err)),
        Err(_) => Err(TimeoutError::Timeout(timeout)),
    }
}

/// Error type for timeout operations
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError<E> {
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    #[error(transparent)]
    Inner(E),
}

impl<E> TimeoutError<E> {
    /// Check if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, TimeoutError::Timeout(_))
    }

    /// Get the inner error if not a timeout
    pub fn into_inner(self) -> Option<E> {
        match self {
            TimeoutError::Inner(e) => Some(e),
            TimeoutError::Timeout(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TimeoutConfig::default();
        assert_eq!(config.bedrock_timeout, Duration::from_secs(120));
        assert_eq!(config.dynamodb_timeout, Duration::from_secs(5));
        assert_eq!(config.streaming_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_config_builder() {
        let config = TimeoutConfig::new()
            .with_bedrock_timeout(Duration::from_secs(60))
            .with_dynamodb_timeout(Duration::from_secs(10))
            .with_streaming_timeout(Duration::from_secs(600));

        assert_eq!(config.bedrock_timeout, Duration::from_secs(60));
        assert_eq!(config.dynamodb_timeout, Duration::from_secs(10));
        assert_eq!(config.streaming_timeout, Duration::from_secs(600));
    }

    #[tokio::test]
    async fn test_with_timeout_success() {
        let result: Result<i32, TimeoutError<String>> = with_timeout(
            Duration::from_secs(1),
            async { Ok::<_, String>(42) },
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_timeout_inner_error() {
        let result: Result<i32, TimeoutError<String>> = with_timeout(
            Duration::from_secs(1),
            async { Err::<i32, _>("inner error".to_string()) },
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_timeout());
        assert_eq!(err.into_inner(), Some("inner error".to_string()));
    }

    #[tokio::test]
    async fn test_with_timeout_timeout() {
        let result: Result<i32, TimeoutError<String>> = with_timeout(
            Duration::from_millis(10),
            async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok::<_, String>(42)
            },
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_timeout());
        assert!(err.into_inner().is_none());
    }
}
