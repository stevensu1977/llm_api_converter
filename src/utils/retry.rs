//! Retry utilities with exponential backoff
//!
//! This module provides retry functionality for transient failures,
//! using exponential backoff with jitter to prevent thundering herd problems.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use rand::Rng;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial attempt)
    pub max_retries: u32,

    /// Initial delay before the first retry
    pub initial_delay: Duration,

    /// Maximum delay between retries (caps exponential growth)
    pub max_delay: Duration,

    /// Multiplier for exponential backoff (typically 2.0)
    pub multiplier: f64,

    /// Whether to add jitter to delays (recommended to prevent thundering herd)
    pub use_jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            use_jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with custom settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum retry attempts
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set backoff multiplier
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    pub fn with_jitter(mut self, use_jitter: bool) -> Self {
        self.use_jitter = use_jitter;
        self
    }

    /// Calculate delay for a given attempt number (0-indexed)
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        // Calculate exponential delay
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.multiplier.powi(attempt as i32);

        // Cap at max delay
        let delay_ms = delay_ms.min(self.max_delay.as_millis() as f64);

        // Add jitter if enabled (random value between 0 and delay)
        let delay_ms = if self.use_jitter {
            let jitter = rand::thread_rng().gen_range(0.0..delay_ms);
            delay_ms + jitter
        } else {
            delay_ms
        };

        Duration::from_millis(delay_ms as u64)
    }
}

/// Result of a retry operation
#[derive(Debug)]
pub struct RetryResult<T, E> {
    /// The final result (success or last error)
    pub result: Result<T, E>,

    /// Number of attempts made
    pub attempts: u32,

    /// Total time spent retrying
    pub total_delay: Duration,
}

/// Execute an async operation with retry logic
///
/// # Arguments
/// * `config` - Retry configuration
/// * `is_retryable` - Function to determine if an error is retryable
/// * `operation` - The async operation to execute
///
/// # Returns
/// `RetryResult` containing the final result and retry statistics
pub async fn retry_with_backoff<T, E, F, Fut, R>(
    config: &RetryConfig,
    is_retryable: R,
    mut operation: F,
) -> RetryResult<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
{
    let mut attempts = 0;
    let mut total_delay = Duration::ZERO;

    loop {
        attempts += 1;

        match operation().await {
            Ok(value) => {
                return RetryResult {
                    result: Ok(value),
                    attempts,
                    total_delay,
                };
            }
            Err(err) => {
                // Check if we've exhausted retries or error is not retryable
                if attempts > config.max_retries || !is_retryable(&err) {
                    return RetryResult {
                        result: Err(err),
                        attempts,
                        total_delay,
                    };
                }

                // Calculate and apply delay
                let delay = config.calculate_delay(attempts - 1);
                total_delay += delay;

                tracing::debug!(
                    attempt = attempts,
                    delay_ms = delay.as_millis(),
                    "Retrying after transient failure"
                );

                sleep(delay).await;
            }
        }
    }
}

/// Simple retry helper that uses default config
pub async fn retry<T, E, F, Fut, R>(
    is_retryable: R,
    operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
{
    let config = RetryConfig::default();
    retry_with_backoff(&config, is_retryable, operation).await.result
}

/// Retry configuration presets for different use cases
pub mod presets {
    use super::*;

    /// Configuration for Bedrock API calls
    /// - More retries for transient failures
    /// - Longer delays to handle rate limiting
    pub fn bedrock() -> RetryConfig {
        RetryConfig::new()
            .with_max_retries(3)
            .with_initial_delay(Duration::from_millis(500))
            .with_max_delay(Duration::from_secs(30))
            .with_multiplier(2.0)
            .with_jitter(true)
    }

    /// Configuration for DynamoDB operations
    /// - Quick retries for fast operations
    /// - Fewer max retries
    pub fn dynamodb() -> RetryConfig {
        RetryConfig::new()
            .with_max_retries(3)
            .with_initial_delay(Duration::from_millis(50))
            .with_max_delay(Duration::from_secs(1))
            .with_multiplier(2.0)
            .with_jitter(true)
    }

    /// Configuration for aggressive retry (internal operations)
    pub fn aggressive() -> RetryConfig {
        RetryConfig::new()
            .with_max_retries(5)
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(5))
            .with_multiplier(1.5)
            .with_jitter(true)
    }

    /// No retry configuration
    pub fn no_retry() -> RetryConfig {
        RetryConfig::new()
            .with_max_retries(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert!(config.use_jitter);
    }

    #[test]
    fn test_config_builder() {
        let config = RetryConfig::new()
            .with_max_retries(5)
            .with_initial_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(30))
            .with_multiplier(3.0)
            .with_jitter(false);

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.multiplier, 3.0);
        assert!(!config.use_jitter);
    }

    #[test]
    fn test_calculate_delay_without_jitter() {
        let config = RetryConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_multiplier(2.0)
            .with_jitter(false);

        assert_eq!(config.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(2), Duration::from_millis(400));
        assert_eq!(config.calculate_delay(3), Duration::from_millis(800));
    }

    #[test]
    fn test_calculate_delay_respects_max() {
        let config = RetryConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_millis(500))
            .with_multiplier(2.0)
            .with_jitter(false);

        assert_eq!(config.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(config.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(config.calculate_delay(2), Duration::from_millis(400));
        // Should be capped at 500ms
        assert_eq!(config.calculate_delay(3), Duration::from_millis(500));
        assert_eq!(config.calculate_delay(10), Duration::from_millis(500));
    }

    #[test]
    fn test_calculate_delay_with_jitter() {
        let config = RetryConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_multiplier(2.0)
            .with_jitter(true);

        // With jitter, delay should be between base and 2*base
        let delay = config.calculate_delay(0);
        assert!(delay >= Duration::from_millis(100));
        assert!(delay <= Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let config = RetryConfig::new().with_max_retries(3);
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = retry_with_backoff(
            &config,
            |_: &String| true,
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, String>(42)
                }
            },
        )
        .await;

        assert!(result.result.is_ok());
        assert_eq!(result.result.unwrap(), 42);
        assert_eq!(result.attempts, 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig::new()
            .with_max_retries(3)
            .with_initial_delay(Duration::from_millis(1))
            .with_jitter(false);

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = retry_with_backoff(
            &config,
            |_: &String| true,
            || {
                let count = call_count_clone.clone();
                async move {
                    let current = count.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err("transient error".to_string())
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;

        assert!(result.result.is_ok());
        assert_eq!(result.result.unwrap(), 42);
        assert_eq!(result.attempts, 3);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig::new()
            .with_max_retries(2)
            .with_initial_delay(Duration::from_millis(1))
            .with_jitter(false);

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = retry_with_backoff(
            &config,
            |_: &String| true,
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, _>("always fails".to_string())
                }
            },
        )
        .await;

        assert!(result.result.is_err());
        // Initial attempt + 2 retries = 3 attempts
        assert_eq!(result.attempts, 3);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error() {
        let config = RetryConfig::new()
            .with_max_retries(3)
            .with_initial_delay(Duration::from_millis(1));

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = retry_with_backoff(
            &config,
            |err: &String| !err.contains("permanent"),
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, _>("permanent error".to_string())
                }
            },
        )
        .await;

        assert!(result.result.is_err());
        // Should not retry non-retryable errors
        assert_eq!(result.attempts, 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_presets() {
        let bedrock = presets::bedrock();
        assert_eq!(bedrock.max_retries, 3);
        assert_eq!(bedrock.initial_delay, Duration::from_millis(500));

        let dynamodb = presets::dynamodb();
        assert_eq!(dynamodb.max_retries, 3);
        assert_eq!(dynamodb.initial_delay, Duration::from_millis(50));

        let no_retry = presets::no_retry();
        assert_eq!(no_retry.max_retries, 0);
    }
}
