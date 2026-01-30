//! Messages API endpoint
//!
//! This module implements the POST /v1/messages endpoint for the Anthropic Messages API.
//! It handles request conversion, Bedrock Converse API calls, and response conversion.
//! Supports both streaming and non-streaming responses using the Converse API.

use aws_sdk_bedrockruntime::types::{
    ContentBlock as SdkContentBlock, ConversationRole, ConverseStreamOutput,
    InferenceConfiguration, Message as SdkMessage, SystemContentBlock, Tool as SdkTool,
    ToolConfiguration, ToolInputSchema as SdkToolInputSchema, ToolResultContentBlock,
    ToolResultStatus, ToolSpecification, ToolUseBlock,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Instant;
use uuid::Uuid;

use crate::converters::ConversionError;
use crate::schemas::anthropic::{
    ContentBlock, ErrorResponse, Message, MessageContent, MessageRequest, MessageResponse,
    StopReason, SystemContent, ToolResultValue, Usage,
};
use crate::server::state::AppState;
use crate::services::{BedrockError, ConverseRequest};
use crate::utils::truncate_str;

// ============================================================================
// Error Types
// ============================================================================

/// API error response with HTTP status code
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub error_type: String,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error_type: "invalid_request_error".to_string(),
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error_type: "authentication_error".to_string(),
            message: message.into(),
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            error_type: "rate_limit_error".to_string(),
            message: message.into(),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error_type: "api_error".to_string(),
            message: message.into(),
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            error_type: "overloaded_error".to_string(),
            message: message.into(),
        }
    }

    pub fn from_bedrock_error(err: &BedrockError) -> Self {
        match err {
            BedrockError::Throttled(msg) => Self::rate_limited(msg),
            BedrockError::ValidationError(msg) => Self::bad_request(msg),
            BedrockError::ModelNotFound(msg) => Self::bad_request(format!("Model not found: {}", msg)),
            BedrockError::AccessDenied(msg) => Self::unauthorized(msg),
            BedrockError::ServiceUnavailable(msg) => Self::service_unavailable(msg),
            BedrockError::InternalError(msg) => Self::internal_error(msg),
            BedrockError::Serialization(msg) => Self::bad_request(format!("Serialization error: {}", msg)),
            BedrockError::Deserialization(msg) => Self::internal_error(format!("Response error: {}", msg)),
            BedrockError::ApiError { message, .. } => Self::internal_error(message),
            BedrockError::Unknown(msg) => Self::internal_error(msg),
        }
    }

    pub fn from_conversion_error(err: &ConversionError) -> Self {
        match err {
            ConversionError::InvalidContentBlock(msg) => Self::bad_request(msg),
            ConversionError::InvalidMessage(msg) => Self::bad_request(msg),
            ConversionError::InvalidTool(msg) => Self::bad_request(msg),
            ConversionError::Base64DecodeError(msg) => Self::bad_request(format!("Invalid base64: {}", msg)),
            ConversionError::MissingField(field) => Self::bad_request(format!("Missing required field: {}", field)),
            ConversionError::UnsupportedFeature(msg) => Self::bad_request(format!("Unsupported feature: {}", msg)),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse::new(&self.error_type, &self.message);
        (self.status, Json(error_response)).into_response()
    }
}

// ============================================================================
// Streaming Response Type
// ============================================================================

/// Enum to represent either a JSON response or an SSE stream
pub enum MessageApiResponse {
    Json(Json<MessageResponse>),
    Stream(Sse<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>),
}

impl IntoResponse for MessageApiResponse {
    fn into_response(self) -> Response {
        match self {
            MessageApiResponse::Json(json) => json.into_response(),
            MessageApiResponse::Stream(sse) => sse.into_response(),
        }
    }
}

// ============================================================================
// Handler Implementation
// ============================================================================

/// POST /v1/messages - Create a message
///
/// This endpoint accepts Anthropic Messages API requests, converts them to Bedrock format,
/// calls the Bedrock Converse/ConverseStream API, and returns the response in Anthropic format.
///
/// Supports both streaming and non-streaming responses.
pub async fn create_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MessageRequest>,
) -> Result<MessageApiResponse, ApiError> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();

    // Get the Bedrock model ID early for logging
    let bedrock_model = state.bedrock.get_bedrock_model_id(&request.model);

    tracing::info!(
        request_id = %request_id,
        model = %request.model,
        bedrock_model = %bedrock_model,
        message_count = request.messages.len(),
        max_tokens = request.max_tokens,
        stream = request.stream,
        "Processing messages request"
    );

    // Print prompts if enabled (for debugging)
    if state.settings.print_prompts {
        print_request_prompts(&request_id, &request);
    }

    // Extract beta headers for feature flags
    let _beta_header = headers
        .get("anthropic-beta")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Build Converse request (uses the same model mapping)
    let converse_request = build_converse_request(&state, &request)?;

    // Handle streaming vs non-streaming
    if request.stream {
        let sse_stream = create_streaming_response(&state, converse_request, &request_id, &request.model, &bedrock_model).await?;
        return Ok(MessageApiResponse::Stream(sse_stream));
    }

    // Non-streaming response using Converse API
    let converse_output = state
        .bedrock
        .converse(converse_request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Bedrock Converse API call failed");
            ApiError::from_bedrock_error(&e)
        })?;

    // Convert Converse response to Anthropic format
    let response = convert_converse_response(converse_output, &request.model)?;

    let duration_ms = start_time.elapsed().as_millis();

    tracing::info!(
        request_id = %request_id,
        model = %response.model,
        bedrock_model = %bedrock_model,
        input_tokens = response.usage.input_tokens,
        output_tokens = response.usage.output_tokens,
        stop_reason = ?response.stop_reason,
        duration_ms = duration_ms,
        "Request completed successfully"
    );

    Ok(MessageApiResponse::Json(Json(response)))
}

// ============================================================================
// Request Building
// ============================================================================

/// Build a Converse request from Anthropic MessageRequest
fn build_converse_request(
    state: &AppState,
    request: &MessageRequest,
) -> Result<ConverseRequest, ApiError> {
    let model_id = state.bedrock.get_bedrock_model_id(&request.model);

    // Convert messages
    let messages = convert_messages_to_sdk(&request.messages)?;

    // Build inference config
    let mut inference_config = InferenceConfiguration::builder()
        .max_tokens(request.max_tokens);

    if let Some(temp) = request.temperature {
        inference_config = inference_config.temperature(temp);
    }
    if let Some(top_p) = request.top_p {
        inference_config = inference_config.top_p(top_p);
    }
    if let Some(ref stop_seqs) = request.stop_sequences {
        inference_config = inference_config.set_stop_sequences(Some(stop_seqs.clone()));
    }

    let mut converse_req = ConverseRequest::new(model_id)
        .with_messages(messages)
        .with_inference_config(inference_config.build());

    // Convert system prompt
    if let Some(ref system) = request.system {
        let system_blocks = convert_system_to_sdk(system);
        converse_req = converse_req.with_system(system_blocks);
    }

    // Convert tools
    if let Some(ref tools) = request.tools {
        if !tools.is_empty() {
            let tool_config = convert_tools_to_sdk(tools)?;
            converse_req = converse_req.with_tool_config(tool_config);
        }
    }

    // Handle extended thinking via additional fields
    if let Some(ref thinking) = request.thinking {
        let mut thinking_map = std::collections::HashMap::new();
        thinking_map.insert("type".to_string(), aws_smithy_types::Document::String(thinking.thinking_type.clone()));
        if let Some(budget) = thinking.budget_tokens {
            thinking_map.insert("budget_tokens".to_string(), aws_smithy_types::Document::Number(
                aws_smithy_types::Number::PosInt(budget as u64)
            ));
        }

        let additional = aws_smithy_types::Document::Object(std::collections::HashMap::from([
            ("thinking".to_string(), aws_smithy_types::Document::Object(thinking_map)),
        ]));
        converse_req = converse_req.with_additional_fields(additional);
    }

    Ok(converse_req)
}

/// Convert Anthropic messages to SDK messages
fn convert_messages_to_sdk(messages: &[Message]) -> Result<Vec<SdkMessage>, ApiError> {
    let mut sdk_messages = Vec::new();

    for msg in messages {
        let role = match msg.role.as_str() {
            "user" => ConversationRole::User,
            "assistant" => ConversationRole::Assistant,
            _ => {
                return Err(ApiError::bad_request(format!(
                    "Invalid role: {}",
                    msg.role
                )));
            }
        };

        let content_blocks = convert_content_to_sdk(&msg.content)?;

        let sdk_msg = SdkMessage::builder()
            .role(role)
            .set_content(Some(content_blocks))
            .build()
            .map_err(|e| ApiError::bad_request(format!("Failed to build message: {}", e)))?;

        sdk_messages.push(sdk_msg);
    }

    Ok(sdk_messages)
}

/// Convert Anthropic content to SDK content blocks
fn convert_content_to_sdk(content: &MessageContent) -> Result<Vec<SdkContentBlock>, ApiError> {
    match content {
        MessageContent::Text(text) => Ok(vec![SdkContentBlock::Text(text.clone())]),
        MessageContent::Blocks(blocks) => {
            let mut sdk_blocks = Vec::new();
            for block in blocks {
                if let Some(sdk_block) = convert_content_block_to_sdk(block)? {
                    sdk_blocks.push(sdk_block);
                }
            }
            Ok(sdk_blocks)
        }
    }
}

/// Convert a single content block to SDK format
fn convert_content_block_to_sdk(block: &ContentBlock) -> Result<Option<SdkContentBlock>, ApiError> {
    match block {
        ContentBlock::Text { text, .. } => Ok(Some(SdkContentBlock::Text(text.clone()))),

        ContentBlock::Image { source, .. } => {
            use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource};
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

            let bytes = BASE64
                .decode(&source.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid base64: {}", e)))?;

            let format = match source.media_type.as_str() {
                "image/png" => ImageFormat::Png,
                "image/jpeg" => ImageFormat::Jpeg,
                "image/gif" => ImageFormat::Gif,
                "image/webp" => ImageFormat::Webp,
                _ => ImageFormat::Png,
            };

            let image = ImageBlock::builder()
                .format(format)
                .source(ImageSource::Bytes(aws_sdk_bedrockruntime::primitives::Blob::new(bytes)))
                .build()
                .map_err(|e| ApiError::bad_request(format!("Failed to build image: {}", e)))?;

            Ok(Some(SdkContentBlock::Image(image)))
        }

        ContentBlock::ToolUse { id, name, input, .. } => {
            let tool_use = ToolUseBlock::builder()
                .tool_use_id(id)
                .name(name)
                .input(json_to_document(input))
                .build()
                .map_err(|e| ApiError::bad_request(format!("Failed to build tool use: {}", e)))?;

            Ok(Some(SdkContentBlock::ToolUse(tool_use)))
        }

        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => {
            use aws_sdk_bedrockruntime::types::ToolResultBlock;

            let result_content = match content {
                ToolResultValue::Text(text) => vec![ToolResultContentBlock::Text(text.clone())],
                ToolResultValue::Blocks(blocks) => {
                    let mut result_blocks = Vec::new();
                    for b in blocks {
                        if let ContentBlock::Text { text, .. } = b {
                            result_blocks.push(ToolResultContentBlock::Text(text.clone()));
                        }
                    }
                    result_blocks
                }
            };

            let status = if is_error.unwrap_or(false) {
                ToolResultStatus::Error
            } else {
                ToolResultStatus::Success
            };

            let tool_result = ToolResultBlock::builder()
                .tool_use_id(tool_use_id)
                .set_content(Some(result_content))
                .status(status)
                .build()
                .map_err(|e| ApiError::bad_request(format!("Failed to build tool result: {}", e)))?;

            Ok(Some(SdkContentBlock::ToolResult(tool_result)))
        }

        ContentBlock::Document { source, .. } => {
            use aws_sdk_bedrockruntime::types::{DocumentBlock, DocumentFormat, DocumentSource};
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

            let bytes = BASE64
                .decode(&source.data)
                .map_err(|e| ApiError::bad_request(format!("Invalid base64: {}", e)))?;

            let format = match source.media_type.as_str() {
                "application/pdf" => DocumentFormat::Pdf,
                "text/plain" => DocumentFormat::Txt,
                "text/html" => DocumentFormat::Html,
                "text/csv" => DocumentFormat::Csv,
                _ => DocumentFormat::Pdf,
            };

            let doc = DocumentBlock::builder()
                .format(format)
                .name("document")
                .source(DocumentSource::Bytes(aws_sdk_bedrockruntime::primitives::Blob::new(bytes)))
                .build()
                .map_err(|e| ApiError::bad_request(format!("Failed to build document: {}", e)))?;

            Ok(Some(SdkContentBlock::Document(doc)))
        }

        // Skip thinking blocks
        ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => Ok(None),

        // Skip server tool blocks
        ContentBlock::ServerToolUse { .. } | ContentBlock::ServerToolResult { .. } => Ok(None),
    }
}

/// Convert system content to SDK format
fn convert_system_to_sdk(system: &SystemContent) -> Vec<SystemContentBlock> {
    match system {
        SystemContent::Text(text) => vec![SystemContentBlock::Text(text.clone())],
        SystemContent::Messages(messages) => messages
            .iter()
            .map(|m| SystemContentBlock::Text(m.text.clone()))
            .collect(),
    }
}

/// Convert tools to SDK ToolConfiguration
fn convert_tools_to_sdk(tools: &[serde_json::Value]) -> Result<ToolConfiguration, ApiError> {
    let mut sdk_tools = Vec::new();

    for tool in tools {
        // Skip code execution tools
        if tool.get("type").and_then(|v| v.as_str()) == Some("code_execution_20250825") {
            continue;
        }

        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::bad_request("Tool missing name"))?;

        let description = tool
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let input_schema = tool
            .get("input_schema")
            .cloned()
            .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

        let tool_spec = ToolSpecification::builder()
            .name(name)
            .description(description)
            .input_schema(SdkToolInputSchema::Json(json_to_document(&input_schema)))
            .build()
            .map_err(|e| ApiError::bad_request(format!("Failed to build tool spec: {}", e)))?;

        sdk_tools.push(SdkTool::ToolSpec(tool_spec));
    }

    Ok(ToolConfiguration::builder()
        .set_tools(Some(sdk_tools))
        .build()
        .map_err(|e| ApiError::bad_request(format!("Failed to build tool config: {}", e)))?)
}

/// Convert serde_json::Value to aws_smithy_types::Document
fn json_to_document(value: &serde_json::Value) -> aws_smithy_types::Document {
    match value {
        serde_json::Value::Null => aws_smithy_types::Document::Null,
        serde_json::Value::Bool(b) => aws_smithy_types::Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 0 {
                    aws_smithy_types::Document::Number(aws_smithy_types::Number::PosInt(i as u64))
                } else {
                    aws_smithy_types::Document::Number(aws_smithy_types::Number::NegInt(i))
                }
            } else if let Some(f) = n.as_f64() {
                aws_smithy_types::Document::Number(aws_smithy_types::Number::Float(f))
            } else {
                aws_smithy_types::Document::Null
            }
        }
        serde_json::Value::String(s) => aws_smithy_types::Document::String(s.clone()),
        serde_json::Value::Array(arr) => {
            aws_smithy_types::Document::Array(arr.iter().map(json_to_document).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: std::collections::HashMap<String, aws_smithy_types::Document> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_document(v)))
                .collect();
            aws_smithy_types::Document::Object(map)
        }
    }
}

// ============================================================================
// Response Conversion
// ============================================================================

/// Convert Converse response to Anthropic MessageResponse
fn convert_converse_response(
    output: aws_sdk_bedrockruntime::operation::converse::ConverseOutput,
    original_model: &str,
) -> Result<MessageResponse, ApiError> {
    let message_id = format!("msg_{}", Uuid::new_v4().to_string().replace("-", ""));

    // Convert content blocks
    let mut content = Vec::new();
    if let Some(output_content) = output.output() {
        if let aws_sdk_bedrockruntime::types::ConverseOutput::Message(msg) = output_content {
            for block in msg.content() {
                if let Some(converted) = convert_sdk_content_to_anthropic(block) {
                    content.push(converted);
                }
            }
        }
    }

    // Convert stop reason (stop_reason() returns &StopReason directly)
    let stop_reason = Some(match output.stop_reason() {
        aws_sdk_bedrockruntime::types::StopReason::EndTurn => StopReason::EndTurn,
        aws_sdk_bedrockruntime::types::StopReason::MaxTokens => StopReason::MaxTokens,
        aws_sdk_bedrockruntime::types::StopReason::StopSequence => StopReason::StopSequence,
        aws_sdk_bedrockruntime::types::StopReason::ToolUse => StopReason::ToolUse,
        aws_sdk_bedrockruntime::types::StopReason::ContentFiltered => StopReason::EndTurn,
        aws_sdk_bedrockruntime::types::StopReason::GuardrailIntervened => StopReason::EndTurn,
        _ => StopReason::EndTurn,
    });

    // Get usage
    let usage = output.usage().map(|u| Usage {
        input_tokens: u.input_tokens(),
        output_tokens: u.output_tokens(),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    }).unwrap_or(Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    });

    Ok(MessageResponse {
        id: message_id,
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content,
        model: original_model.to_string(),
        stop_reason,
        stop_sequence: None,
        usage,
    })
}

/// Convert SDK content block to Anthropic ContentBlock
fn convert_sdk_content_to_anthropic(block: &SdkContentBlock) -> Option<ContentBlock> {
    match block {
        SdkContentBlock::Text(text) => Some(ContentBlock::Text {
            text: text.clone(),
            cache_control: None,
        }),
        SdkContentBlock::ToolUse(tool_use) => Some(ContentBlock::ToolUse {
            id: tool_use.tool_use_id().to_string(),
            name: tool_use.name().to_string(),
            input: document_to_json(tool_use.input()),
            caller: None,
        }),
        _ => None,
    }
}

/// Convert aws_smithy_types::Document to serde_json::Value
fn document_to_json(doc: &aws_smithy_types::Document) -> serde_json::Value {
    match doc {
        aws_smithy_types::Document::Null => serde_json::Value::Null,
        aws_smithy_types::Document::Bool(b) => serde_json::Value::Bool(*b),
        aws_smithy_types::Document::Number(n) => match n {
            aws_smithy_types::Number::PosInt(i) => serde_json::json!(*i),
            aws_smithy_types::Number::NegInt(i) => serde_json::json!(*i),
            aws_smithy_types::Number::Float(f) => serde_json::json!(*f),
        },
        aws_smithy_types::Document::String(s) => serde_json::Value::String(s.clone()),
        aws_smithy_types::Document::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(document_to_json).collect())
        }
        aws_smithy_types::Document::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), document_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

// ============================================================================
// Streaming Response Handler
// ============================================================================

/// Create a streaming response using SSE with ConverseStream API
async fn create_streaming_response(
    state: &AppState,
    request: ConverseRequest,
    request_id: &str,
    original_model: &str,
    bedrock_model: &str,
) -> Result<Sse<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>, ApiError>
{
    // Get streaming response from Bedrock
    let mut stream_response = state
        .bedrock
        .converse_stream(request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Bedrock ConverseStream API call failed");
            ApiError::from_bedrock_error(&e)
        })?;

    let model_id = original_model.to_string();
    let bedrock_model_id = bedrock_model.to_string();
    let req_id = request_id.to_string();

    // Create the SSE stream
    let stream = async_stream::stream! {
        let message_id = format!("msg_{}", Uuid::new_v4().to_string().replace("-", ""));
        let mut content_block_index: i32 = 0;
        let mut total_input_tokens: i32 = 0;
        let mut total_output_tokens: i32 = 0;
        let mut stop_reason = "end_turn".to_string();

        tracing::debug!(request_id = %req_id, "Starting SSE stream");

        // Emit message_start event first
        let message_start_data = serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": model_id,
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 0
                }
            }
        });
        yield Ok(Event::default().event("message_start").data(message_start_data.to_string()));

        // Process Bedrock ConverseStream events
        loop {
            match stream_response.recv().await {
                Ok(Some(event)) => {
                    match event {
                        ConverseStreamOutput::MessageStart(start_event) => {
                            // Capture role info if needed
                            tracing::debug!(request_id = %req_id, role = ?start_event.role(), "Message start");
                        }

                        ConverseStreamOutput::ContentBlockStart(block_start) => {
                            let index = block_start.content_block_index();
                            content_block_index = index;

                            // Determine content block type
                            let content_block = if let Some(start) = block_start.start() {
                                match start {
                                    aws_sdk_bedrockruntime::types::ContentBlockStart::ToolUse(tool_start) => {
                                        serde_json::json!({
                                            "type": "tool_use",
                                            "id": tool_start.tool_use_id(),
                                            "name": tool_start.name(),
                                            "input": {}
                                        })
                                    }
                                    _ => serde_json::json!({"type": "text", "text": ""})
                                }
                            } else {
                                serde_json::json!({"type": "text", "text": ""})
                            };

                            let data = serde_json::json!({
                                "type": "content_block_start",
                                "index": index,
                                "content_block": content_block
                            });
                            yield Ok(Event::default().event("content_block_start").data(data.to_string()));
                        }

                        ConverseStreamOutput::ContentBlockDelta(block_delta) => {
                            let index = block_delta.content_block_index();

                            if let Some(delta) = block_delta.delta() {
                                let delta_json = match delta {
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::Text(text) => {
                                        serde_json::json!({"type": "text_delta", "text": text})
                                    }
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::ToolUse(tool_delta) => {
                                        serde_json::json!({
                                            "type": "input_json_delta",
                                            "partial_json": tool_delta.input()
                                        })
                                    }
                                    _ => continue,
                                };

                                let data = serde_json::json!({
                                    "type": "content_block_delta",
                                    "index": index,
                                    "delta": delta_json
                                });
                                yield Ok(Event::default().event("content_block_delta").data(data.to_string()));
                            }
                        }

                        ConverseStreamOutput::ContentBlockStop(block_stop) => {
                            let index = block_stop.content_block_index();
                            let data = serde_json::json!({
                                "type": "content_block_stop",
                                "index": index
                            });
                            yield Ok(Event::default().event("content_block_stop").data(data.to_string()));
                            content_block_index = index + 1;
                        }

                        ConverseStreamOutput::MessageStop(stop_event) => {
                            // stop_reason() returns &StopReason directly (not an Option)
                            stop_reason = match stop_event.stop_reason() {
                                aws_sdk_bedrockruntime::types::StopReason::EndTurn => "end_turn".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::MaxTokens => "max_tokens".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::StopSequence => "stop_sequence".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::ToolUse => "tool_use".to_string(),
                                _ => "end_turn".to_string(),
                            };
                        }

                        ConverseStreamOutput::Metadata(metadata_event) => {
                            if let Some(usage) = metadata_event.usage() {
                                total_input_tokens = usage.input_tokens();
                                total_output_tokens = usage.output_tokens();
                            }
                        }

                        _ => {
                            tracing::debug!(request_id = %req_id, "Unknown stream event");
                        }
                    }
                }
                Ok(None) => {
                    // Stream ended
                    tracing::debug!(request_id = %req_id, "Stream ended");
                    break;
                }
                Err(e) => {
                    tracing::error!(request_id = %req_id, error = %e, "Stream error");
                    let error_data = serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": e.to_string()
                        }
                    });
                    yield Ok(Event::default()
                        .event("error")
                        .data(error_data.to_string()));
                    break;
                }
            }
        }

        // Emit message_delta with final usage
        let message_delta_data = serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": null
            },
            "usage": {
                "output_tokens": total_output_tokens
            }
        });
        yield Ok(Event::default().event("message_delta").data(message_delta_data.to_string()));

        // Emit message_stop event
        let message_stop_data = serde_json::json!({
            "type": "message_stop"
        });
        yield Ok(Event::default().event("message_stop").data(message_stop_data.to_string()));

        tracing::info!(
            request_id = %req_id,
            model = %model_id,
            bedrock_model = %bedrock_model_id,
            input_tokens = total_input_tokens,
            output_tokens = total_output_tokens,
            stop_reason = %stop_reason,
            "Streaming response completed"
        );
    };

    Ok(Sse::new(Box::pin(stream)))
}

// ============================================================================
// Count Tokens Endpoint
// ============================================================================

/// Count tokens request
#[derive(Debug, Clone, Deserialize)]
pub struct CountTokensRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

/// Count tokens response
#[derive(Debug, Clone, Serialize)]
pub struct CountTokensResponse {
    pub input_tokens: i32,
}

/// POST /v1/messages/count_tokens - Count tokens in a message
///
/// This endpoint estimates the number of tokens that would be used by a request.
/// Note: Bedrock doesn't provide a direct token counting API, so this returns an estimate.
pub async fn count_tokens(
    State(_state): State<AppState>,
    Json(request): Json<CountTokensRequest>,
) -> Result<Json<CountTokensResponse>, ApiError> {
    tracing::debug!(
        model = %request.model,
        message_count = request.messages.len(),
        "Counting tokens"
    );

    // Simple estimation: ~4 characters per token (rough estimate)
    let mut char_count = 0;

    for message in &request.messages {
        if let Some(content) = message.get("content") {
            char_count += content.to_string().len();
        }
    }

    if let Some(ref system) = request.system {
        char_count += system.to_string().len();
    }

    if let Some(ref tools) = request.tools {
        for tool in tools {
            char_count += tool.to_string().len();
        }
    }

    let estimated_tokens = (char_count / 4).max(1) as i32;

    Ok(Json(CountTokensResponse {
        input_tokens: estimated_tokens,
    }))
}

// ============================================================================
// Debug Utilities
// ============================================================================

/// Print request prompts to stdout for debugging
fn print_request_prompts(request_id: &str, request: &MessageRequest) {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();

    // Header
    writeln!(stdout, "\n{}", "=".repeat(80)).ok();
    writeln!(stdout, "REQUEST [{request_id}]").ok();
    writeln!(stdout, "{}", "=".repeat(80)).ok();
    writeln!(stdout, "Model: {}", request.model).ok();
    writeln!(stdout, "Max tokens: {}", request.max_tokens).ok();
    if let Some(temp) = request.temperature {
        writeln!(stdout, "Temperature: {temp}").ok();
    }
    writeln!(stdout, "Stream: {}", request.stream).ok();
    writeln!(stdout, "{}", "-".repeat(80)).ok();

    // System prompt
    if let Some(ref system) = request.system {
        writeln!(stdout, "SYSTEM:").ok();
        match system {
            SystemContent::Text(text) => {
                writeln!(stdout, "{text}").ok();
            }
            SystemContent::Messages(messages) => {
                for msg in messages {
                    writeln!(stdout, "{}", msg.text).ok();
                }
            }
        }
        writeln!(stdout, "{}", "-".repeat(80)).ok();
    }

    // Messages
    writeln!(stdout, "MESSAGES ({} total):", request.messages.len()).ok();
    for (i, msg) in request.messages.iter().enumerate() {
        let role_icon = match msg.role.as_str() {
            "user" => "U",
            "assistant" => "A",
            _ => "?",
        };
        writeln!(stdout, "\n[{i}] {role_icon} {}", msg.role.to_uppercase()).ok();

        match &msg.content {
            MessageContent::Text(text) => {
                let char_count = text.chars().count();
                let display_text = if char_count > 2000 {
                    format!("{}... [truncated, {} chars total]", truncate_str(text, 2000), char_count)
                } else {
                    text.clone()
                };
                writeln!(stdout, "{display_text}").ok();
            }
            MessageContent::Blocks(blocks) => {
                for content in blocks {
                    match content {
                        ContentBlock::Text { text, .. } => {
                            let display_text = if text.chars().count() > 2000 {
                                format!("{}... [truncated]", truncate_str(text, 2000))
                            } else {
                                text.clone()
                            };
                            writeln!(stdout, "{display_text}").ok();
                        }
                        ContentBlock::Image { source, .. } => {
                            writeln!(stdout, "[Image: {} bytes, type: {}]", source.data.len(), source.media_type).ok();
                        }
                        ContentBlock::ToolUse { id, name, input, .. } => {
                            writeln!(stdout, "[Tool Use: {name} (id: {id})]").ok();
                            if let Ok(json) = serde_json::to_string_pretty(&input) {
                                writeln!(stdout, "  Input: {json}").ok();
                            }
                        }
                        ContentBlock::ToolResult { tool_use_id, content, .. } => {
                            writeln!(stdout, "[Tool Result for: {tool_use_id}]").ok();
                            match content {
                                ToolResultValue::Text(text) => {
                                    let display = if text.chars().count() > 500 {
                                        format!("{}... [truncated]", truncate_str(text, 500))
                                    } else {
                                        text.clone()
                                    };
                                    writeln!(stdout, "  Result: {display}").ok();
                                }
                                ToolResultValue::Blocks(blocks) => {
                                    writeln!(stdout, "  Result: [{} blocks]", blocks.len()).ok();
                                }
                            }
                        }
                        _ => {
                            writeln!(stdout, "[Other content block]").ok();
                        }
                    }
                }
            }
        }
    }

    // Tools
    if let Some(ref tools) = request.tools {
        writeln!(stdout, "\n{}", "-".repeat(80)).ok();
        writeln!(stdout, "TOOLS ({} defined):", tools.len()).ok();
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                writeln!(stdout, "  - {name}").ok();
            }
        }
    }

    writeln!(stdout, "{}\n", "=".repeat(80)).ok();
    stdout.flush().ok();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_status_codes() {
        assert_eq!(ApiError::bad_request("test").status, StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::unauthorized("test").status, StatusCode::UNAUTHORIZED);
        assert_eq!(ApiError::rate_limited("test").status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(ApiError::internal_error("test").status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(ApiError::service_unavailable("test").status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_json_to_document() {
        let json = serde_json::json!({"key": "value", "num": 42});
        let doc = json_to_document(&json);

        if let aws_smithy_types::Document::Object(map) = doc {
            assert!(map.contains_key("key"));
            assert!(map.contains_key("num"));
        } else {
            panic!("Expected Document::Object");
        }
    }

    #[test]
    fn test_document_to_json() {
        let doc = aws_smithy_types::Document::Object(std::collections::HashMap::from([
            ("key".to_string(), aws_smithy_types::Document::String("value".to_string())),
        ]));
        let json = document_to_json(&doc);

        assert_eq!(json["key"], "value");
    }

    #[test]
    fn test_count_tokens_estimation() {
        let char_count = 400;
        let estimated_tokens = (char_count / 4).max(1);
        assert_eq!(estimated_tokens, 100);
    }
}
