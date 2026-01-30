//! OpenAI Chat Completions API endpoint
//!
//! This module implements the POST /v1/chat/completions endpoint for OpenAI API compatibility.
//! It handles request conversion from OpenAI format to Bedrock, calls the Converse API,
//! and converts responses back to OpenAI format.

use aws_sdk_bedrockruntime::types::{
    ContentBlock as SdkContentBlock, ConversationRole, ConverseStreamOutput,
    InferenceConfiguration, Message as SdkMessage, SystemContentBlock, Tool as SdkTool,
    ToolConfiguration, ToolInputSchema as SdkToolInputSchema, ToolResultContentBlock,
    ToolResultStatus, ToolSpecification, ToolUseBlock,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    Json,
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::time::Instant;
use uuid::Uuid;

use crate::converters::{OpenAIConversionError, OpenAIToBedrockConverter};
use crate::schemas::openai::{
    AssistantMessage, ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse,
    ChatRole, Choice, ChunkChoice, ChunkDelta, CompletionUsage, FunctionCall,
    OpenAIErrorResponse, ToolCall, ToolCallDelta, FunctionCallDelta,
    current_timestamp, generate_completion_id,
};
use crate::server::state::AppState;
use crate::services::{BedrockError, ConverseRequest};

// ============================================================================
// Error Types
// ============================================================================

/// OpenAI-style API error
#[derive(Debug)]
pub struct OpenAIApiError {
    pub status: StatusCode,
    pub error: OpenAIErrorResponse,
}

impl OpenAIApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error: OpenAIErrorResponse::invalid_request(&message.into()),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: OpenAIErrorResponse::authentication_error(&message.into()),
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            error: OpenAIErrorResponse::rate_limit_error(&message.into()),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: OpenAIErrorResponse::server_error(&message.into()),
        }
    }

    pub fn from_bedrock_error(err: &BedrockError) -> Self {
        match err {
            BedrockError::Throttled(msg) => Self::rate_limited(msg),
            BedrockError::ValidationError(msg) => Self::bad_request(msg),
            BedrockError::ModelNotFound(msg) => Self::bad_request(format!("Model not found: {}", msg)),
            BedrockError::AccessDenied(msg) => Self::unauthorized(msg),
            BedrockError::ServiceUnavailable(msg) => Self::internal_error(msg),
            BedrockError::InternalError(msg) => Self::internal_error(msg),
            BedrockError::Serialization(msg) => Self::bad_request(format!("Serialization error: {}", msg)),
            BedrockError::Deserialization(msg) => Self::internal_error(format!("Response error: {}", msg)),
            BedrockError::ApiError { message, .. } => Self::internal_error(message),
            BedrockError::Unknown(msg) => Self::internal_error(msg),
        }
    }

    pub fn from_conversion_error(err: &OpenAIConversionError) -> Self {
        match err {
            OpenAIConversionError::InvalidContent(msg) => Self::bad_request(msg),
            OpenAIConversionError::InvalidMessage(msg) => Self::bad_request(msg),
            OpenAIConversionError::InvalidTool(msg) => Self::bad_request(msg),
            OpenAIConversionError::Base64DecodeError(msg) => Self::bad_request(format!("Invalid base64: {}", msg)),
            OpenAIConversionError::MissingField(field) => Self::bad_request(format!("Missing required field: {}", field)),
            OpenAIConversionError::UnsupportedFeature(msg) => Self::bad_request(format!("Unsupported feature: {}", msg)),
            OpenAIConversionError::InvalidImageUrl(msg) => Self::bad_request(msg),
        }
    }
}

impl IntoResponse for OpenAIApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.error)).into_response()
    }
}

// ============================================================================
// Response Type
// ============================================================================

/// Enum to represent either a JSON response or an SSE stream (OpenAI format)
pub enum ChatCompletionApiResponse {
    Json(Json<ChatCompletionResponse>),
    Stream(Sse<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>),
}

impl IntoResponse for ChatCompletionApiResponse {
    fn into_response(self) -> Response {
        match self {
            ChatCompletionApiResponse::Json(json) => json.into_response(),
            ChatCompletionApiResponse::Stream(sse) => sse.into_response(),
        }
    }
}

// ============================================================================
// Handler Implementation
// ============================================================================

/// POST /v1/chat/completions - Create a chat completion
///
/// This endpoint accepts OpenAI Chat Completions API requests, converts them to Bedrock format,
/// calls the Bedrock Converse API, and returns the response in OpenAI format.
///
/// Supports both streaming and non-streaming responses.
pub async fn chat_completions(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<ChatCompletionApiResponse, OpenAIApiError> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();

    // Use converter to get Bedrock model ID
    let openai_converter = OpenAIToBedrockConverter::new();
    let bedrock_model = openai_converter.convert_model_id(&request.model);

    // Apply settings overrides if available
    let bedrock_model = state.bedrock.get_bedrock_model_id(&bedrock_model);

    tracing::info!(
        request_id = %request_id,
        openai_model = %request.model,
        bedrock_model = %bedrock_model,
        message_count = request.messages.len(),
        max_tokens = request.max_tokens.or(request.max_completion_tokens),
        stream = request.stream,
        "Processing OpenAI chat completions request"
    );

    // Check for unsupported features
    if request.n.map(|n| n > 1).unwrap_or(false) {
        return Err(OpenAIApiError::bad_request(
            "Only n=1 is supported. Multiple completions are not available.",
        ));
    }

    // Build Converse request
    let converse_request = build_converse_request_from_openai(&state, &request, &bedrock_model)?;

    // Handle streaming vs non-streaming
    if request.stream {
        let include_usage = request
            .stream_options
            .as_ref()
            .map(|o| o.include_usage)
            .unwrap_or(false);

        let sse_stream = create_openai_streaming_response(
            &state,
            converse_request,
            &request_id,
            &request.model,
            include_usage,
        )
        .await?;

        return Ok(ChatCompletionApiResponse::Stream(sse_stream));
    }

    // Non-streaming response
    let converse_output = state
        .bedrock
        .converse(converse_request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Bedrock Converse API call failed");
            OpenAIApiError::from_bedrock_error(&e)
        })?;

    // Convert response to OpenAI format
    let response = convert_converse_to_openai(converse_output, &request.model)?;

    let duration_ms = start_time.elapsed().as_millis();

    tracing::info!(
        request_id = %request_id,
        model = %response.model,
        bedrock_model = %bedrock_model,
        prompt_tokens = response.usage.prompt_tokens,
        completion_tokens = response.usage.completion_tokens,
        finish_reason = ?response.choices.first().and_then(|c| c.finish_reason.as_ref()),
        duration_ms = duration_ms,
        "OpenAI chat completion request completed"
    );

    Ok(ChatCompletionApiResponse::Json(Json(response)))
}

// ============================================================================
// Request Building
// ============================================================================

/// Build a Converse request from OpenAI ChatCompletionRequest
fn build_converse_request_from_openai(
    _state: &AppState,
    request: &ChatCompletionRequest,
    bedrock_model: &str,
) -> Result<ConverseRequest, OpenAIApiError> {
    // Convert messages
    let (system_messages, chat_messages): (Vec<_>, Vec<_>) = request
        .messages
        .iter()
        .partition(|m| m.role == ChatRole::System);

    let sdk_messages = convert_openai_messages_to_sdk(&chat_messages)?;

    // Build inference config
    let max_tokens = request
        .max_completion_tokens
        .or(request.max_tokens)
        .unwrap_or(4096);

    let mut inference_config = InferenceConfiguration::builder().max_tokens(max_tokens);

    if let Some(temp) = request.temperature {
        // Clamp temperature to 0-1 range for Bedrock
        inference_config = inference_config.temperature(temp.min(1.0).max(0.0));
    }
    if let Some(top_p) = request.top_p {
        inference_config = inference_config.top_p(top_p);
    }
    if let Some(ref stop) = request.stop {
        inference_config = inference_config.set_stop_sequences(Some(stop.to_vec()));
    }

    let mut converse_req = ConverseRequest::new(bedrock_model.to_string())
        .with_messages(sdk_messages)
        .with_inference_config(inference_config.build());

    // Convert system messages
    if !system_messages.is_empty() {
        let system_blocks: Vec<SystemContentBlock> = system_messages
            .iter()
            .filter_map(|m| {
                m.content.as_ref().map(|c| {
                    SystemContentBlock::Text(c.to_string_content())
                })
            })
            .collect();

        if !system_blocks.is_empty() {
            converse_req = converse_req.with_system(system_blocks);
        }
    }

    // Convert tools
    if let Some(ref tools) = request.tools {
        if !tools.is_empty() {
            let tool_config = convert_openai_tools_to_sdk(tools)?;
            converse_req = converse_req.with_tool_config(tool_config);
        }
    }

    Ok(converse_req)
}

/// Convert OpenAI messages to SDK messages
fn convert_openai_messages_to_sdk(
    messages: &[&crate::schemas::openai::ChatMessage],
) -> Result<Vec<SdkMessage>, OpenAIApiError> {
    let mut sdk_messages = Vec::new();

    for msg in messages {
        let role = match msg.role {
            ChatRole::User => ConversationRole::User,
            ChatRole::Assistant => ConversationRole::Assistant,
            ChatRole::Tool => ConversationRole::User, // Tool results come as user messages
            ChatRole::System => continue, // Skip system messages (handled separately)
        };

        let content_blocks = convert_openai_content_to_sdk(msg)?;

        if content_blocks.is_empty() {
            continue;
        }

        let sdk_msg = SdkMessage::builder()
            .role(role)
            .set_content(Some(content_blocks))
            .build()
            .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build message: {}", e)))?;

        sdk_messages.push(sdk_msg);
    }

    Ok(sdk_messages)
}

/// Convert OpenAI message content to SDK content blocks
fn convert_openai_content_to_sdk(
    msg: &crate::schemas::openai::ChatMessage,
) -> Result<Vec<SdkContentBlock>, OpenAIApiError> {
    use crate::schemas::openai::{ContentPart, MessageContent};

    // Handle tool role (tool results)
    if msg.role == ChatRole::Tool {
        let tool_use_id = msg.tool_call_id.as_ref().ok_or_else(|| {
            OpenAIApiError::bad_request("Tool message missing tool_call_id")
        })?;

        let content_text = msg
            .content
            .as_ref()
            .map(|c| c.to_string_content())
            .unwrap_or_default();

        use aws_sdk_bedrockruntime::types::ToolResultBlock;

        let tool_result = ToolResultBlock::builder()
            .tool_use_id(tool_use_id)
            .set_content(Some(vec![ToolResultContentBlock::Text(content_text)]))
            .status(ToolResultStatus::Success)
            .build()
            .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build tool result: {}", e)))?;

        return Ok(vec![SdkContentBlock::ToolResult(tool_result)]);
    }

    // Handle assistant messages with tool calls
    if msg.role == ChatRole::Assistant {
        if let Some(ref tool_calls) = msg.tool_calls {
            let mut blocks = Vec::new();

            // Add text content if present
            if let Some(ref content) = msg.content {
                let text = content.to_string_content();
                if !text.is_empty() {
                    blocks.push(SdkContentBlock::Text(text));
                }
            }

            // Add tool use blocks
            for tool_call in tool_calls {
                let input: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));

                let tool_use = ToolUseBlock::builder()
                    .tool_use_id(&tool_call.id)
                    .name(&tool_call.function.name)
                    .input(json_to_document(&input))
                    .build()
                    .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build tool use: {}", e)))?;

                blocks.push(SdkContentBlock::ToolUse(tool_use));
            }

            return Ok(blocks);
        }
    }

    // Handle regular content
    match &msg.content {
        Some(MessageContent::Text(text)) => Ok(vec![SdkContentBlock::Text(text.clone())]),
        Some(MessageContent::Parts(parts)) => {
            let mut blocks = Vec::new();
            for part in parts {
                match part {
                    ContentPart::Text { text } => {
                        blocks.push(SdkContentBlock::Text(text.clone()));
                    }
                    ContentPart::ImageUrl { image_url } => {
                        // Parse data URL
                        if image_url.url.starts_with("data:") {
                            let image_block = parse_data_url_to_image(&image_url.url)?;
                            blocks.push(image_block);
                        } else {
                            return Err(OpenAIApiError::bad_request(
                                "External image URLs are not supported. Use base64 data URLs.",
                            ));
                        }
                    }
                }
            }
            Ok(blocks)
        }
        None => Ok(vec![]),
    }
}

/// Parse a data URL and convert to SDK ImageBlock
fn parse_data_url_to_image(url: &str) -> Result<SdkContentBlock, OpenAIApiError> {
    use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource};
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    let parts: Vec<&str> = url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err(OpenAIApiError::bad_request("Invalid data URL format"));
    }

    let metadata = parts[0];
    let data = parts[1];

    // Extract media type
    let media_type = metadata
        .strip_prefix("data:")
        .and_then(|s| s.split(';').next())
        .ok_or_else(|| OpenAIApiError::bad_request("Could not parse media type"))?;

    let format = match media_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        _ => ImageFormat::Png,
    };

    let bytes = BASE64
        .decode(data)
        .map_err(|e| OpenAIApiError::bad_request(format!("Invalid base64: {}", e)))?;

    let image = ImageBlock::builder()
        .format(format)
        .source(ImageSource::Bytes(aws_sdk_bedrockruntime::primitives::Blob::new(bytes)))
        .build()
        .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build image: {}", e)))?;

    Ok(SdkContentBlock::Image(image))
}

/// Convert OpenAI tools to SDK ToolConfiguration
fn convert_openai_tools_to_sdk(
    tools: &[crate::schemas::openai::Tool],
) -> Result<ToolConfiguration, OpenAIApiError> {
    let mut sdk_tools = Vec::new();

    for tool in tools {
        if tool.tool_type != "function" {
            continue;
        }

        let input_schema = tool
            .function
            .parameters
            .clone()
            .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

        let tool_spec = ToolSpecification::builder()
            .name(&tool.function.name)
            .description(tool.function.description.as_deref().unwrap_or(""))
            .input_schema(SdkToolInputSchema::Json(json_to_document(&input_schema)))
            .build()
            .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build tool spec: {}", e)))?;

        sdk_tools.push(SdkTool::ToolSpec(tool_spec));
    }

    Ok(ToolConfiguration::builder()
        .set_tools(Some(sdk_tools))
        .build()
        .map_err(|e| OpenAIApiError::bad_request(format!("Failed to build tool config: {}", e)))?)
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
// Response Conversion
// ============================================================================

/// Convert Converse response to OpenAI ChatCompletionResponse
fn convert_converse_to_openai(
    output: aws_sdk_bedrockruntime::operation::converse::ConverseOutput,
    original_model: &str,
) -> Result<ChatCompletionResponse, OpenAIApiError> {
    let completion_id = generate_completion_id();
    let created = current_timestamp();

    // Convert content blocks
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    if let Some(output_content) = output.output() {
        if let aws_sdk_bedrockruntime::types::ConverseOutput::Message(msg) = output_content {
            for block in msg.content() {
                match block {
                    SdkContentBlock::Text(text) => {
                        text_parts.push(text.clone());
                    }
                    SdkContentBlock::ToolUse(tool_use) => {
                        let input_json = document_to_json(tool_use.input());
                        tool_calls.push(ToolCall {
                            id: tool_use.tool_use_id().to_string(),
                            tool_type: "function".to_string(),
                            function: FunctionCall {
                                name: tool_use.name().to_string(),
                                arguments: serde_json::to_string(&input_json)
                                    .unwrap_or_else(|_| "{}".to_string()),
                            },
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // Convert stop reason
    let finish_reason = match output.stop_reason() {
        aws_sdk_bedrockruntime::types::StopReason::EndTurn => "stop".to_string(),
        aws_sdk_bedrockruntime::types::StopReason::MaxTokens => "length".to_string(),
        aws_sdk_bedrockruntime::types::StopReason::StopSequence => "stop".to_string(),
        aws_sdk_bedrockruntime::types::StopReason::ToolUse => "tool_calls".to_string(),
        aws_sdk_bedrockruntime::types::StopReason::ContentFiltered => "content_filter".to_string(),
        _ => "stop".to_string(),
    };

    // Get usage
    let usage = output
        .usage()
        .map(|u| CompletionUsage {
            prompt_tokens: u.input_tokens(),
            completion_tokens: u.output_tokens(),
            total_tokens: u.input_tokens() + u.output_tokens(),
            completion_tokens_details: None,
        })
        .unwrap_or(CompletionUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            completion_tokens_details: None,
        });

    let content = text_parts.join("");

    Ok(ChatCompletionResponse {
        id: completion_id,
        object: "chat.completion".to_string(),
        created,
        model: original_model.to_string(),
        choices: vec![Choice {
            index: 0,
            message: AssistantMessage {
                role: ChatRole::Assistant,
                content: if content.is_empty() { None } else { Some(content) },
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            },
            finish_reason: Some(finish_reason),
            logprobs: None,
        }],
        usage,
        system_fingerprint: None,
    })
}

// ============================================================================
// Streaming Response Handler
// ============================================================================

/// Create a streaming response using SSE with OpenAI format
async fn create_openai_streaming_response(
    state: &AppState,
    request: ConverseRequest,
    request_id: &str,
    original_model: &str,
    include_usage: bool,
) -> Result<Sse<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>, OpenAIApiError>
{
    // Get streaming response from Bedrock
    let mut stream_response = state
        .bedrock
        .converse_stream(request)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Bedrock ConverseStream API call failed");
            OpenAIApiError::from_bedrock_error(&e)
        })?;

    let model_id = original_model.to_string();
    let req_id = request_id.to_string();
    let completion_id = generate_completion_id();
    let created = current_timestamp();

    // Create the SSE stream
    let stream = async_stream::stream! {
        let mut tool_call_index: i32 = 0;
        let mut block_to_tool_index: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
        let mut total_input_tokens: i32 = 0;
        let mut total_output_tokens: i32 = 0;
        let mut sent_role = false;

        tracing::debug!(request_id = %req_id, "Starting OpenAI SSE stream");

        // Process Bedrock ConverseStream events
        loop {
            match stream_response.recv().await {
                Ok(Some(event)) => {
                    match event {
                        ConverseStreamOutput::MessageStart(_) => {
                            // Send initial chunk with role
                            if !sent_role {
                                sent_role = true;
                                let chunk = ChatCompletionChunk {
                                    id: completion_id.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model_id.clone(),
                                    choices: vec![ChunkChoice {
                                        index: 0,
                                        delta: ChunkDelta {
                                            role: Some(ChatRole::Assistant),
                                            content: None,
                                            tool_calls: None,
                                        },
                                        finish_reason: None,
                                        logprobs: None,
                                    }],
                                    system_fingerprint: None,
                                    usage: None,
                                };
                                let json = serde_json::to_string(&chunk).unwrap_or_default();
                                yield Ok(Event::default().data(json));
                            }
                        }

                        ConverseStreamOutput::ContentBlockStart(block_start) => {
                            let block_index = block_start.content_block_index();

                            if let Some(start) = block_start.start() {
                                if let aws_sdk_bedrockruntime::types::ContentBlockStart::ToolUse(tool_start) = start {
                                    // Assign tool call index
                                    block_to_tool_index.insert(block_index, tool_call_index);

                                    let chunk = ChatCompletionChunk {
                                        id: completion_id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_id.clone(),
                                        choices: vec![ChunkChoice {
                                            index: 0,
                                            delta: ChunkDelta {
                                                role: None,
                                                content: None,
                                                tool_calls: Some(vec![ToolCallDelta {
                                                    index: tool_call_index,
                                                    id: Some(tool_start.tool_use_id().to_string()),
                                                    tool_type: Some("function".to_string()),
                                                    function: Some(FunctionCallDelta {
                                                        name: Some(tool_start.name().to_string()),
                                                        arguments: None,
                                                    }),
                                                }]),
                                            },
                                            finish_reason: None,
                                            logprobs: None,
                                        }],
                                        system_fingerprint: None,
                                        usage: None,
                                    };
                                    let json = serde_json::to_string(&chunk).unwrap_or_default();
                                    yield Ok(Event::default().data(json));

                                    tool_call_index += 1;
                                }
                            }
                        }

                        ConverseStreamOutput::ContentBlockDelta(block_delta) => {
                            let block_index = block_delta.content_block_index();

                            if let Some(delta) = block_delta.delta() {
                                match delta {
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::Text(text) => {
                                        let chunk = ChatCompletionChunk {
                                            id: completion_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            created,
                                            model: model_id.clone(),
                                            choices: vec![ChunkChoice {
                                                index: 0,
                                                delta: ChunkDelta {
                                                    role: None,
                                                    content: Some(text.to_string()),
                                                    tool_calls: None,
                                                },
                                                finish_reason: None,
                                                logprobs: None,
                                            }],
                                            system_fingerprint: None,
                                            usage: None,
                                        };
                                        let json = serde_json::to_string(&chunk).unwrap_or_default();
                                        yield Ok(Event::default().data(json));
                                    }
                                    aws_sdk_bedrockruntime::types::ContentBlockDelta::ToolUse(tool_delta) => {
                                        let tc_index = block_to_tool_index.get(&block_index).copied().unwrap_or(0);

                                        let chunk = ChatCompletionChunk {
                                            id: completion_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            created,
                                            model: model_id.clone(),
                                            choices: vec![ChunkChoice {
                                                index: 0,
                                                delta: ChunkDelta {
                                                    role: None,
                                                    content: None,
                                                    tool_calls: Some(vec![ToolCallDelta {
                                                        index: tc_index,
                                                        id: None,
                                                        tool_type: None,
                                                        function: Some(FunctionCallDelta {
                                                            name: None,
                                                            arguments: Some(tool_delta.input().to_string()),
                                                        }),
                                                    }]),
                                                },
                                                finish_reason: None,
                                                logprobs: None,
                                            }],
                                            system_fingerprint: None,
                                            usage: None,
                                        };
                                        let json = serde_json::to_string(&chunk).unwrap_or_default();
                                        yield Ok(Event::default().data(json));
                                    }
                                    _ => {}
                                }
                            }
                        }

                        ConverseStreamOutput::ContentBlockStop(_) => {
                            // No action needed for OpenAI format
                        }

                        ConverseStreamOutput::MessageStop(stop_event) => {
                            let finish_reason = match stop_event.stop_reason() {
                                aws_sdk_bedrockruntime::types::StopReason::EndTurn => "stop".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::MaxTokens => "length".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::StopSequence => "stop".to_string(),
                                aws_sdk_bedrockruntime::types::StopReason::ToolUse => "tool_calls".to_string(),
                                _ => "stop".to_string(),
                            };

                            // Send final chunk with finish_reason
                            let chunk = ChatCompletionChunk {
                                id: completion_id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model_id.clone(),
                                choices: vec![ChunkChoice {
                                    index: 0,
                                    delta: ChunkDelta::default(),
                                    finish_reason: Some(finish_reason),
                                    logprobs: None,
                                }],
                                system_fingerprint: None,
                                usage: None,
                            };
                            let json = serde_json::to_string(&chunk).unwrap_or_default();
                            yield Ok(Event::default().data(json));
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
                    tracing::debug!(request_id = %req_id, "OpenAI stream ended");

                    // Send usage chunk if requested
                    if include_usage {
                        let usage_chunk = ChatCompletionChunk {
                            id: completion_id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model_id.clone(),
                            choices: vec![],
                            system_fingerprint: None,
                            usage: Some(CompletionUsage {
                                prompt_tokens: total_input_tokens,
                                completion_tokens: total_output_tokens,
                                total_tokens: total_input_tokens + total_output_tokens,
                                completion_tokens_details: None,
                            }),
                        };
                        let json = serde_json::to_string(&usage_chunk).unwrap_or_default();
                        yield Ok(Event::default().data(json));
                    }

                    // Send [DONE] marker
                    yield Ok(Event::default().data("[DONE]"));
                    break;
                }
                Err(e) => {
                    tracing::error!(request_id = %req_id, error = %e, "Stream error");
                    let error_response = OpenAIErrorResponse::server_error(&e.to_string());
                    let json = serde_json::to_string(&error_response).unwrap_or_default();
                    yield Ok(Event::default().data(json));
                    break;
                }
            }
        }
    };

    Ok(Sse::new(Box::pin(stream)))
}
