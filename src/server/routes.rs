//! Application routing
//!
//! This module defines all HTTP routes for the application.

use axum::{middleware, routing::{get, post}, Router};
use tower_http::cors::{Any, CorsLayer};

use crate::api::{chat_completions, event_logging, health, messages, models};
use crate::middleware::{
    auth::{require_api_key, AuthState},
    logging::log_request,
    rate_limit::{rate_limit, RateLimitState},
};
use crate::server::state::AppState;

/// Create the main application router
pub fn create_router(state: AppState) -> Router {
    // Health check routes (no authentication required)
    let health_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness))
        .route("/liveness", get(health::liveness));

    // Event logging routes (no authentication required - telemetry)
    let event_logging_routes = Router::new()
        .route("/batch", post(event_logging::batch_events));

    // Create middleware state
    let auth_state = AuthState::new(state.settings.clone(), state.dynamodb.clone());
    let auth_state_clone = auth_state.clone();
    let rate_limit_state = RateLimitState::new(state.settings.clone());
    let rate_limit_state_clone = rate_limit_state.clone();

    // Anthropic API routes (POST /v1/messages)
    // Layer order: last added = outermost = runs first
    // So auth runs before rate_limit
    let anthropic_routes = Router::new()
        .route("/messages", post(messages::create_message))
        .route("/messages/count_tokens", post(messages::count_tokens))
        // Rate limiting layer (runs after auth, uses ApiKeyInfo)
        .layer(middleware::from_fn_with_state(
            rate_limit_state.clone(),
            rate_limit,
        ))
        // Authentication layer (runs first, sets ApiKeyInfo in extensions)
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            require_api_key,
        ));

    // OpenAI API routes (POST /v1/chat/completions, GET /v1/models)
    // Same authentication and rate limiting as Anthropic routes
    let openai_routes = Router::new()
        .route("/chat/completions", post(chat_completions::chat_completions))
        .route("/models", get(models::list_models))
        .route("/models/:model_id", get(models::get_model))
        // Rate limiting layer
        .layer(middleware::from_fn_with_state(
            rate_limit_state_clone,
            rate_limit,
        ))
        // Authentication layer
        .layer(middleware::from_fn_with_state(
            auth_state_clone,
            require_api_key,
        ));

    // Combine all routes
    // Both Anthropic and OpenAI routes are under /v1
    Router::new()
        .nest("/v1", anthropic_routes)
        .nest("/v1", openai_routes)
        .nest("/api/event_logging", event_logging_routes)
        .merge(health_routes)
        // Apply middleware layers (order matters: first added = outermost = runs first)
        .layer(create_cors_layer())
        // Custom request logging with trace IDs
        .layer(middleware::from_fn(log_request))
        .with_state(state)
}

/// Create CORS layer with permissive settings for development
fn create_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers([
            // Expose trace ID headers to clients
            "x-trace-id".parse().unwrap(),
            "x-request-id".parse().unwrap(),
            // Expose rate limit headers
            "x-ratelimit-limit".parse().unwrap(),
            "x-ratelimit-remaining".parse().unwrap(),
            "x-ratelimit-reset".parse().unwrap(),
            "retry-after".parse().unwrap(),
        ])
}
