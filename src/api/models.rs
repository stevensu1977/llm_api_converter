//! OpenAI Models API endpoint
//!
//! This module implements the GET /v1/models and GET /v1/models/{model_id} endpoints
//! for OpenAI API compatibility.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::schemas::openai::{Model, ModelsResponse, OpenAIErrorResponse, current_timestamp};
use crate::server::state::AppState;

// ============================================================================
// Available Models
// ============================================================================

/// Get list of available models (both OpenAI aliases and Bedrock model IDs)
fn get_available_models() -> Vec<Model> {
    let created = current_timestamp();

    vec![
        // OpenAI model aliases (mapped to Claude)
        Model {
            id: "gpt-4".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "gpt-4-turbo".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "gpt-4o".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "gpt-4o-mini".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "gpt-3.5-turbo".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "o1".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        Model {
            id: "o1-mini".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "openai".to_string(),
        },
        // Claude models (Anthropic naming)
        Model {
            id: "claude-3-5-sonnet-20241022".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "claude-3-5-haiku-20241022".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "claude-3-opus-20240229".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "claude-opus-4-5-20251101".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "claude-sonnet-4-5-20250929".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        // Bedrock model IDs (direct)
        Model {
            id: "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "anthropic.claude-3-opus-20240229-v1:0".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
        Model {
            id: "anthropic.claude-opus-4-5-20251101-v1:0".to_string(),
            object: "model".to_string(),
            created,
            owned_by: "anthropic".to_string(),
        },
    ]
}

// ============================================================================
// Handler Implementation
// ============================================================================

/// GET /v1/models - List available models
///
/// Returns a list of models available for use with the API.
pub async fn list_models(
    State(_state): State<AppState>,
) -> Json<ModelsResponse> {
    let models = get_available_models();

    tracing::debug!(model_count = models.len(), "Listing available models");

    Json(ModelsResponse {
        object: "list".to_string(),
        data: models,
    })
}

/// GET /v1/models/{model_id} - Retrieve a model
///
/// Returns information about a specific model.
pub async fn get_model(
    State(_state): State<AppState>,
    Path(model_id): Path<String>,
) -> impl IntoResponse {
    let models = get_available_models();

    // Check if model exists in our list
    if let Some(model) = models.into_iter().find(|m| m.id == model_id) {
        tracing::debug!(model_id = %model_id, "Model found");
        return (StatusCode::OK, Json(serde_json::json!(model))).into_response();
    }

    // Check if it's a valid Bedrock model format (passthrough)
    if model_id.contains("anthropic.") || model_id.contains("qwen.") || model_id.starts_with("arn:") {
        tracing::debug!(model_id = %model_id, "Returning passthrough model");
        let model = Model {
            id: model_id.clone(),
            object: "model".to_string(),
            created: current_timestamp(),
            owned_by: if model_id.contains("anthropic") {
                "anthropic"
            } else if model_id.contains("qwen") {
                "alibaba"
            } else {
                "aws"
            }
            .to_string(),
        };
        return (StatusCode::OK, Json(serde_json::json!(model))).into_response();
    }

    // Model not found
    tracing::warn!(model_id = %model_id, "Model not found");
    let error = OpenAIErrorResponse::with_code(
        "invalid_request_error",
        &format!("The model '{}' does not exist", model_id),
        "model_not_found",
    );

    (StatusCode::NOT_FOUND, Json(error)).into_response()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());

        // Check that we have OpenAI aliases
        assert!(models.iter().any(|m| m.id == "gpt-4"));
        assert!(models.iter().any(|m| m.id == "gpt-4o"));
        assert!(models.iter().any(|m| m.id == "gpt-4o-mini"));

        // Check that we have Claude models
        assert!(models.iter().any(|m| m.id.contains("claude")));

        // Check that all models have correct object type
        for model in &models {
            assert_eq!(model.object, "model");
        }
    }

    #[test]
    fn test_model_ownership() {
        let models = get_available_models();

        for model in &models {
            if model.id.starts_with("gpt") || model.id.starts_with("o1") {
                assert_eq!(model.owned_by, "openai");
            } else if model.id.contains("claude") || model.id.contains("anthropic") {
                assert_eq!(model.owned_by, "anthropic");
            }
        }
    }
}
