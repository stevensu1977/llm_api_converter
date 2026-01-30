//! Event logging endpoint
//!
//! This module provides an endpoint for receiving telemetry events
//! from Claude Code CLI and other clients.

use axum::{http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for batch event logging
#[derive(Debug, Deserialize)]
pub struct BatchEventRequest {
    /// Array of events to log
    #[serde(default)]
    pub events: Vec<Event>,
    /// Fallback: accept any JSON structure
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

/// Individual event in the batch
#[derive(Debug, Deserialize)]
pub struct Event {
    /// Event type/name
    #[serde(rename = "type", default)]
    pub event_type: Option<String>,
    /// Event timestamp
    #[serde(default)]
    pub timestamp: Option<String>,
    /// Event properties
    #[serde(default)]
    pub properties: Option<Value>,
    /// Additional fields
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

/// Response for batch event logging
#[derive(Debug, Serialize)]
pub struct BatchEventResponse {
    /// Whether the events were accepted
    pub success: bool,
    /// Number of events received
    pub events_received: usize,
}

/// Batch event logging endpoint
///
/// Receives telemetry events from Claude Code CLI.
/// This is a simple sink that accepts events and returns success.
///
/// POST /api/event_logging/batch
pub async fn batch_events(
    Json(payload): Json<Value>,
) -> (StatusCode, Json<BatchEventResponse>) {
    // Count events if the payload has an events array
    let events_count = payload
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);

    // Log the event for debugging (at debug level to avoid noise)
    tracing::debug!(
        events_count = events_count,
        "Received batch events"
    );

    // Return success - we accept all events
    (
        StatusCode::OK,
        Json(BatchEventResponse {
            success: true,
            events_received: events_count,
        }),
    )
}
