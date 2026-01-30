//! Rate limiting middleware
//!
//! This module provides token bucket rate limiting for the Anthropic-Bedrock proxy.
//! Each API key gets its own rate limiter, cached in memory for efficiency.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use governor::{
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use moka::future::Cache;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Settings;
use crate::middleware::auth::ApiKeyInfo;
use crate::schemas::anthropic::ErrorResponse;

// ============================================================================
// Types
// ============================================================================

/// Type alias for our rate limiter instance
type KeyedRateLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Rate limit state shared across requests
#[derive(Clone)]
pub struct RateLimitState {
    /// Application settings
    pub settings: Arc<Settings>,

    /// Cache of rate limiters per API key
    /// Key: API key (truncated), Value: rate limiter
    pub limiters: Cache<String, Arc<KeyedRateLimiter>>,
}

impl RateLimitState {
    /// Create a new rate limit state
    pub fn new(settings: Arc<Settings>) -> Self {
        // Create a cache with 10,000 max entries and 10 minute TTL
        let limiters = Cache::builder()
            .max_capacity(10_000)
            .time_to_idle(Duration::from_secs(600))
            .build();

        Self { settings, limiters }
    }

    /// Get or create a rate limiter for the given API key info
    pub async fn get_limiter(&self, key_info: &ApiKeyInfo) -> Arc<KeyedRateLimiter> {
        let cache_key = key_info.api_key.clone();

        // Try to get from cache
        if let Some(limiter) = self.limiters.get(&cache_key).await {
            return limiter;
        }

        // Create new limiter
        let rate_limit = key_info.effective_rate_limit(self.settings.rate_limit.requests_per_window);
        let limiter = Arc::new(self.create_limiter(rate_limit));

        // Insert into cache
        self.limiters.insert(cache_key, limiter.clone()).await;

        limiter
    }

    /// Create a new rate limiter with the given requests per window
    fn create_limiter(&self, requests_per_window: u32) -> KeyedRateLimiter {
        let window_seconds = self.settings.rate_limit.window_seconds;

        // Convert to requests per second for governor
        // If window is 60 seconds and limit is 100 requests, that's ~1.67 req/sec
        // We use a quota that allows bursts up to the full window limit

        let quota = if window_seconds > 0 && requests_per_window > 0 {
            // Allow bursts, replenish over the window period
            let replenish_period = Duration::from_secs(window_seconds) / requests_per_window;
            Quota::with_period(replenish_period)
                .unwrap()
                .allow_burst(NonZeroU32::new(requests_per_window).unwrap())
        } else {
            // Fallback: 100 requests per minute
            Quota::per_minute(NonZeroU32::new(100).unwrap())
        };

        RateLimiter::direct(quota)
    }
}

// ============================================================================
// Rate Limit Errors
// ============================================================================

/// Rate limit error with retry information
#[derive(Debug)]
pub struct RateLimitError {
    /// Seconds until the next request is allowed
    pub retry_after_seconds: u64,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse::new(
            "rate_limit_error",
            &format!(
                "Rate limit exceeded. Please retry after {} seconds.",
                self.retry_after_seconds
            ),
        );

        let mut response = (StatusCode::TOO_MANY_REQUESTS, Json(error_response)).into_response();

        // Add rate limit headers
        let headers = response.headers_mut();
        headers.insert(
            "retry-after",
            self.retry_after_seconds.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-reset",
            self.retry_after_seconds.to_string().parse().unwrap(),
        );

        response
    }
}

// ============================================================================
// Rate Limit Middleware
// ============================================================================

/// Middleware to enforce rate limits
///
/// This middleware:
/// 1. Extracts `ApiKeyInfo` from request extensions (set by auth middleware)
/// 2. Gets or creates a rate limiter for the API key
/// 3. Checks if the request is allowed
/// 4. Returns 429 Too Many Requests if rate limited
///
/// # Prerequisites
/// - Auth middleware must run first to set `ApiKeyInfo` in extensions
///
/// # Headers
/// On rate limit exceeded:
/// - `Retry-After`: Seconds until next request allowed
/// - `X-RateLimit-Reset`: Same as Retry-After
pub async fn rate_limit(
    State(rate_state): State<RateLimitState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Check if rate limiting is enabled
    if !rate_state.settings.rate_limit.enabled {
        return Ok(next.run(request).await);
    }

    // Get API key info from extensions (set by auth middleware)
    let key_info = request
        .extensions()
        .get::<ApiKeyInfo>()
        .cloned();

    let Some(key_info) = key_info else {
        // No API key info - this shouldn't happen if auth middleware ran first
        // Let the request through (auth will handle the error)
        tracing::warn!("Rate limit middleware: No ApiKeyInfo in extensions");
        return Ok(next.run(request).await);
    };

    // Master key bypasses rate limiting
    if key_info.bypass_rate_limit() {
        tracing::debug!(key = %key_info.api_key, "Master key bypasses rate limit");
        return Ok(next.run(request).await);
    }

    // Get rate limiter for this key
    let limiter = rate_state.get_limiter(&key_info).await;

    // Check rate limit
    match limiter.check() {
        Ok(_) => {
            // Request allowed
            let mut response = next.run(request).await;

            // Add rate limit info headers
            add_rate_limit_headers(&mut response, &key_info, &rate_state.settings);

            Ok(response)
        }
        Err(not_until) => {
            // Rate limited
            let retry_after = not_until.wait_time_from(DefaultClock::default().now());
            let retry_after_seconds = retry_after.as_secs().max(1);

            tracing::warn!(
                key = %key_info.api_key,
                user_id = %key_info.user_id,
                retry_after_seconds = retry_after_seconds,
                "Rate limit exceeded"
            );

            Err(RateLimitError { retry_after_seconds })
        }
    }
}

/// Add rate limit information headers to response
fn add_rate_limit_headers(response: &mut Response, key_info: &ApiKeyInfo, settings: &Settings) {
    let headers = response.headers_mut();

    // X-RateLimit-Limit: Maximum requests per window
    let limit = key_info.effective_rate_limit(settings.rate_limit.requests_per_window);
    if let Ok(v) = limit.to_string().parse() {
        headers.insert("x-ratelimit-limit", v);
    }

    // Note: Calculating remaining is complex with token bucket
    // We'd need to query the limiter state which isn't directly exposed
    // For now, we only add limit and reset headers
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_state_creation() {
        let settings = Arc::new(Settings::default());
        let state = RateLimitState::new(settings);
        assert_eq!(state.limiters.entry_count(), 0);
    }

    #[test]
    fn test_create_limiter() {
        let settings = Arc::new(Settings::default());
        let state = RateLimitState::new(settings);

        let limiter = state.create_limiter(100);

        // First request should be allowed
        assert!(limiter.check().is_ok());
    }

    #[test]
    fn test_rate_limit_error_response() {
        let error = RateLimitError {
            retry_after_seconds: 30,
        };

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        // Check headers
        assert!(response.headers().contains_key("retry-after"));
    }

    #[tokio::test]
    async fn test_get_limiter_caching() {
        let settings = Arc::new(Settings::default());
        let state = RateLimitState::new(settings);

        let key_info = ApiKeyInfo {
            api_key: "test-key".to_string(),
            user_id: "user-1".to_string(),
            is_master: false,
            rate_limit: Some(50),
            service_tier: "default".to_string(),
            monthly_budget: None,
            budget_used_mtd: 0.0,
        };

        // Get limiter twice
        let limiter1 = state.get_limiter(&key_info).await;
        let limiter2 = state.get_limiter(&key_info).await;

        // Should be the same instance (Arc pointer comparison)
        assert!(Arc::ptr_eq(&limiter1, &limiter2));
    }

    #[test]
    fn test_burst_allowance() {
        let mut settings = Settings::default();
        settings.rate_limit.requests_per_window = 10;
        settings.rate_limit.window_seconds = 60;

        let state = RateLimitState::new(Arc::new(settings));
        let limiter = state.create_limiter(10);

        // Should allow burst of requests
        for i in 0..10 {
            assert!(limiter.check().is_ok(), "Request {} should be allowed", i);
        }

        // 11th request should be rate limited
        assert!(limiter.check().is_err(), "Request 11 should be rate limited");
    }
}
