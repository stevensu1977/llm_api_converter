//! Health check endpoints
//!
//! This module provides health check endpoints for monitoring
//! and container orchestration (Kubernetes, ECS, etc.)

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::server::state::AppState;

/// Response for the main health check endpoint
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub environment: String,
    pub uptime_seconds: u64,
}

/// Response for readiness probe
#[derive(Serialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub checks: ReadinessChecks,
}

/// Individual readiness checks
#[derive(Debug, Serialize)]
pub struct ReadinessChecks {
    pub config_loaded: bool,
    pub dynamodb: bool,
    pub bedrock: bool,
    // TODO: Add Docker check in Phase 7 (when PTC enabled)
    // pub docker: Option<bool>,
}

/// Response for liveness probe
#[derive(Serialize)]
pub struct LivenessResponse {
    pub alive: bool,
}

/// Main health check endpoint
///
/// Returns overall service health status with version and uptime information.
/// Use this for general health monitoring.
///
/// GET /health
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: state.settings.app_version.clone(),
        environment: state.settings.environment.to_string(),
        uptime_seconds: state.uptime_seconds(),
    })
}

/// Readiness probe endpoint
///
/// Returns whether the service is ready to accept traffic.
/// Used by load balancers and container orchestrators to determine
/// if the instance should receive traffic.
///
/// GET /ready
pub async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    // Check AWS service health
    let aws_health = state.check_aws_health().await;

    let checks = ReadinessChecks {
        config_loaded: true,
        dynamodb: aws_health.dynamodb,
        bedrock: aws_health.bedrock,
    };

    // Service is ready if all critical checks pass
    // Note: DynamoDB is optional for development, so we don't require it for readiness
    // In production, you might want to make this check mandatory
    let ready = checks.config_loaded;

    let status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    // Log if not ready or if AWS services are unhealthy (for debugging)
    if !ready {
        tracing::warn!(
            checks = ?checks,
            "Service not ready"
        );
    } else if !aws_health.all_healthy() {
        tracing::debug!(
            dynamodb = aws_health.dynamodb,
            bedrock = aws_health.bedrock,
            "Some AWS services are not healthy (non-critical)"
        );
    }

    (status, Json(ReadinessResponse { ready, checks }))
}

/// Liveness probe endpoint
///
/// Returns whether the service is alive and should not be restarted.
/// Used by container orchestrators to detect deadlocks or other fatal issues.
///
/// GET /liveness
pub async fn liveness() -> Json<LivenessResponse> {
    // Simple liveness check - if we can respond, we're alive
    Json(LivenessResponse { alive: true })
}

/// Response for PTC health check endpoint
#[derive(Serialize)]
pub struct PtcHealthResponse {
    pub status: String,
    pub docker: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_version: Option<String>,
    pub active_sessions: usize,
    pub ptc_enabled: bool,
}

/// PTC health check endpoint
///
/// Returns Docker and PTC session status.
/// Only available when PTC is enabled.
///
/// GET /health/ptc
pub async fn ptc_health(State(state): State<AppState>) -> (StatusCode, Json<PtcHealthResponse>) {
    // Check if PTC is enabled
    if !state.settings.features.enable_ptc {
        return (
            StatusCode::OK,
            Json(PtcHealthResponse {
                status: "disabled".to_string(),
                docker: "not_checked".to_string(),
                docker_version: None,
                active_sessions: 0,
                ptc_enabled: false,
            }),
        );
    }

    // Check PTC service health
    match &state.ptc_service {
        Some(ptc) => {
            let health = ptc.health_check().await;

            let status = if health.healthy {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };

            (
                status,
                Json(PtcHealthResponse {
                    status: if health.healthy {
                        "healthy".to_string()
                    } else {
                        "unhealthy".to_string()
                    },
                    docker: if health.docker_available {
                        "connected".to_string()
                    } else {
                        "disconnected".to_string()
                    },
                    docker_version: health.docker_version,
                    active_sessions: health.active_sessions,
                    ptc_enabled: true,
                }),
            )
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(PtcHealthResponse {
                status: "not_initialized".to_string(),
                docker: "not_checked".to_string(),
                docker_version: None,
                active_sessions: 0,
                ptc_enabled: true,
            }),
        ),
    }
}
