//! Request logging middleware
//!
//! This module provides middleware for logging HTTP requests and responses,
//! including request duration, status codes, and trace IDs for correlation.

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use uuid::Uuid;

/// Header name for trace ID
pub const TRACE_ID_HEADER: &str = "x-trace-id";

/// Header name for request ID (alias for trace ID)
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Extension type for storing trace ID in request extensions
#[derive(Clone, Debug)]
pub struct TraceId(pub String);

impl TraceId {
    /// Generate a new trace ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the trace ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Middleware to log HTTP requests and responses
///
/// This middleware:
/// - Generates or extracts a trace ID for request correlation
/// - Logs request details (method, path, headers)
/// - Logs response details (status, duration)
/// - Adds trace ID to response headers
///
/// # Example
///
/// ```ignore
/// Router::new()
///     .layer(axum::middleware::from_fn(log_request))
/// ```
pub async fn log_request(request: Request, next: Next) -> Response<Body> {
    let start = Instant::now();

    // Extract or generate trace ID
    let trace_id = extract_or_generate_trace_id(&request);

    // Extract request details for logging
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let query = uri.query().map(|q| q.to_string());
    let version = format!("{:?}", request.version());

    // Extract useful headers
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let content_length = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    // Log the incoming request with clean JSON format (no Rust Some/None)
    tracing::info!(
        trace_id = %trace_id,
        method = %method,
        path = %path,
        query = %query.as_deref().unwrap_or("-"),
        version = %version,
        user_agent = %user_agent.as_deref().unwrap_or("-"),
        content_length = content_length.unwrap_or(0),
        "Incoming request"
    );

    // Create a span for this request
    let span = tracing::info_span!(
        "http_request",
        trace_id = %trace_id,
        method = %method,
        path = %path,
    );

    // Execute the request within the span
    let response = {
        let _guard = span.enter();
        next.run(request).await
    };

    // Calculate request duration
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;

    // Get response status
    let status = response.status();
    let status_code = status.as_u16();

    // Log the response
    if status.is_success() {
        tracing::info!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            status = %status_code,
            duration_ms = %format!("{:.2}", duration_ms),
            "Request completed"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            status = %status_code,
            duration_ms = %format!("{:.2}", duration_ms),
            "Client error"
        );
    } else if status.is_server_error() {
        tracing::error!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            status = %status_code,
            duration_ms = %format!("{:.2}", duration_ms),
            "Server error"
        );
    } else {
        tracing::info!(
            trace_id = %trace_id,
            method = %method,
            path = %path,
            status = %status_code,
            duration_ms = %format!("{:.2}", duration_ms),
            "Request completed"
        );
    }

    // Add trace ID to response headers
    let mut response = response;
    if let Ok(header_value) = HeaderValue::from_str(&trace_id.0) {
        response
            .headers_mut()
            .insert(TRACE_ID_HEADER, header_value.clone());
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, header_value);
    }

    response
}

/// Extract trace ID from request headers or generate a new one
fn extract_or_generate_trace_id(request: &Request) -> TraceId {
    // Try to extract from x-trace-id header first
    if let Some(trace_id) = request
        .headers()
        .get(TRACE_ID_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        return TraceId(trace_id.to_string());
    }

    // Try x-request-id header as fallback
    if let Some(request_id) = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        return TraceId(request_id.to_string());
    }

    // Generate a new trace ID
    TraceId::new()
}

/// Middleware for detailed request body logging (use with caution in production)
///
/// This should only be enabled in development mode as it can impact
/// performance and potentially log sensitive data.
#[allow(dead_code)]
pub async fn log_request_body(request: Request, next: Next) -> Response<Body> {
    // This is a placeholder for detailed body logging
    // In production, you typically don't want to log request bodies
    // due to performance and privacy concerns
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id_generation() {
        let trace_id = TraceId::new();
        assert!(!trace_id.0.is_empty());
        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(trace_id.0.len(), 36);
    }

    #[test]
    fn test_trace_id_display() {
        let trace_id = TraceId("test-trace-id".to_string());
        assert_eq!(format!("{}", trace_id), "test-trace-id");
    }
}
