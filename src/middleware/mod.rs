//! Middleware module
//!
//! Contains HTTP middleware for authentication, rate limiting, logging, and metrics.

pub mod auth;
pub mod logging;
pub mod metrics;
pub mod rate_limit;

// Re-export commonly used items
pub use auth::{require_api_key, ApiKeyInfo, AuthError, AuthState};
pub use logging::{log_request, TraceId, TRACE_ID_HEADER, REQUEST_ID_HEADER};
pub use rate_limit::{rate_limit, RateLimitError, RateLimitState};
