//! API error types

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Bedrock error: {0}")]
    BedrockError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                "authentication_error",
                msg,
            ),
            ApiError::Forbidden(msg) => (
                StatusCode::FORBIDDEN,
                "forbidden_error",
                msg,
            ),
            ApiError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_error",
                "Rate limit exceeded".to_string(),
            ),
            ApiError::InvalidRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                msg,
            ),
            ApiError::BedrockError(msg) => (
                StatusCode::BAD_GATEWAY,
                "api_error",
                msg,
            ),
            ApiError::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "api_error",
                msg,
            ),
            ApiError::Internal(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "api_error",
                err.to_string(),
            ),
        };

        let body = Json(ErrorResponse {
            type_: "error".to_string(),
            error: ErrorDetail {
                type_: error_type.to_string(),
                message,
            },
        });

        (status, body).into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    #[serde(rename = "type")]
    type_: String,
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    #[serde(rename = "type")]
    type_: String,
    message: String,
}
