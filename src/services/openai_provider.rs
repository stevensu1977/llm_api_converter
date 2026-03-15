//! OpenAI Direct Provider — calls OpenAI API directly, implements LLMProvider trait.
//!
//! Supports gpt-*, o1-*, o3-*, o4-*, chatgpt-* model patterns.
//! Uses reqwest HTTP client to call `https://api.openai.com/v1/chat/completions`.

use std::sync::Arc;

use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::provider::{
    model_matches_pattern, LLMProvider, ProviderError, StreamEvent, StreamResult,
    UnifiedChatRequest, UnifiedChatResponse, UnifiedContent, UnifiedContentBlock, UnifiedUsage,
};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

const OPENAI_PATTERNS: &[&str] = &[
    "gpt-*",
    "o1-*",
    "o3-*",
    "o4-*",
    "chatgpt-*",
];

// ============================================================================
// OpenAI API types (request/response)
// ============================================================================

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: Option<OpenAIResponseMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: i64,
    completion_tokens: i64,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: Option<OpenAIStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamToolCall {
    #[serde(default)]
    id: Option<String>,
    function: Option<OpenAIStreamFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    error: OpenAIErrorDetail,
}

#[derive(Debug, Deserialize)]
struct OpenAIErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<String>,
}

// ============================================================================
// Config
// ============================================================================

#[derive(Debug, Clone)]
pub struct OpenAIProviderConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl OpenAIProviderConfig {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: OPENAI_API_URL.to_string(),
            timeout_seconds: 120,
        }
    }

    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout_seconds = timeout;
        self
    }
}

// ============================================================================
// Provider
// ============================================================================

pub struct OpenAIProvider {
    config: OpenAIProviderConfig,
    client: Arc<Client>,
    patterns: Vec<String>,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIProviderConfig) -> Result<Self, ProviderError> {
        Self::with_patterns(config, OPENAI_PATTERNS.iter().map(|s| s.to_string()).collect())
    }

    pub fn with_patterns(config: OpenAIProviderConfig, patterns: Vec<String>) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|e| ProviderError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client: Arc::new(client),
            patterns,
        })
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn unified_to_openai_request(req: &UnifiedChatRequest, stream: bool) -> OpenAIRequest {
    let mut messages = Vec::new();

    // System message
    if let Some(ref system) = req.system {
        messages.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(serde_json::Value::String(system.clone())),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Conversation messages
    for msg in &req.messages {
        let (content, tool_calls, tool_call_id) = convert_unified_message(msg);
        messages.push(OpenAIMessage {
            role: msg.role.clone(),
            content,
            tool_calls,
            tool_call_id,
        });
    }

    let tools = req.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    });

    OpenAIRequest {
        model: req.model.clone(),
        messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stop: req.stop_sequences.clone(),
        tools,
        stream,
    }
}

fn convert_unified_message(
    msg: &super::provider::UnifiedMessage,
) -> (Option<serde_json::Value>, Option<Vec<OpenAIToolCall>>, Option<String>) {
    match &msg.content {
        UnifiedContent::Text(text) => {
            (Some(serde_json::Value::String(text.clone())), None, None)
        }
        UnifiedContent::Blocks(blocks) => {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut tool_call_id = None;

            for block in blocks {
                match block {
                    UnifiedContentBlock::Text { text } => {
                        text_parts.push(text.clone());
                    }
                    UnifiedContentBlock::ToolUse { id, name, input } => {
                        tool_calls.push(OpenAIToolCall {
                            id: id.clone(),
                            tool_type: "function".to_string(),
                            function: OpenAIFunctionCall {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        });
                    }
                    UnifiedContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => {
                        tool_call_id = Some(tool_use_id.clone());
                        text_parts.push(content.clone());
                    }
                    UnifiedContentBlock::Image { .. } => {
                        // OpenAI supports images via content parts, simplified here
                    }
                }
            }

            let content = if text_parts.is_empty() {
                None
            } else {
                Some(serde_json::Value::String(text_parts.join("")))
            };

            let tc = if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            };

            (content, tc, tool_call_id)
        }
    }
}

fn openai_response_to_unified(resp: OpenAIResponse) -> UnifiedChatResponse {
    let mut content = Vec::new();

    if let Some(choice) = resp.choices.into_iter().next() {
        if let Some(msg) = choice.message {
            if let Some(text) = msg.content {
                content.push(UnifiedContentBlock::Text { text });
            }
            if let Some(tool_calls) = msg.tool_calls {
                for tc in tool_calls {
                    let input: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    content.push(UnifiedContentBlock::ToolUse {
                        id: tc.id,
                        name: tc.function.name,
                        input,
                    });
                }
            }
        }
    }

    let usage = resp
        .usage
        .map(|u| UnifiedUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            ..Default::default()
        })
        .unwrap_or_default();

    UnifiedChatResponse {
        id: resp.id,
        model: resp.model,
        content,
        stop_reason: None,
        usage,
    }
}

fn parse_openai_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    if let Ok(err) = serde_json::from_str::<OpenAIErrorResponse>(body) {
        match status.as_u16() {
            401 => ProviderError::Auth(err.error.message),
            429 => ProviderError::RateLimited(err.error.message),
            404 => ProviderError::ModelNotFound(err.error.message),
            _ => ProviderError::Api {
                code: status.as_u16() as i32,
                message: err.error.message,
            },
        }
    } else {
        ProviderError::Http(format!("HTTP {}: {}", status, body))
    }
}

// ============================================================================
// LLMProvider impl
// ============================================================================

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn supported_model_patterns(&self) -> Vec<String> {
        self.patterns.clone()
    }

    fn supports_model(&self, model: &str) -> bool {
        self.patterns
            .iter()
            .any(|p| model_matches_pattern(model, p))
    }

    async fn chat(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<UnifiedChatResponse, ProviderError> {
        let openai_req = unified_to_openai_request(&request, false);

        let resp = self
            .client
            .post(&self.config.base_url)
            .bearer_auth(&self.config.api_key)
            .json(&openai_req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout
                } else {
                    ProviderError::Http(e.to_string())
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(parse_openai_error(status, &body));
        }

        let openai_resp: OpenAIResponse = resp.json().await.map_err(|e| {
            ProviderError::Internal(format!("Failed to parse OpenAI response: {}", e))
        })?;

        Ok(openai_response_to_unified(openai_resp))
    }

    async fn chat_stream(
        &self,
        request: UnifiedChatRequest,
    ) -> Result<StreamResult, ProviderError> {
        let openai_req = unified_to_openai_request(&request, true);

        let resp = self
            .client
            .post(&self.config.base_url)
            .bearer_auth(&self.config.api_key)
            .json(&openai_req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout
                } else {
                    ProviderError::Http(e.to_string())
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(parse_openai_error(status, &body));
        }

        let byte_stream = resp.bytes_stream();

        let stream = async_stream::stream! {
            let mut buffer = String::new();

            futures::pin_mut!(byte_stream);

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(ProviderError::Http(e.to_string()));
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete SSE lines
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].trim().to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    let data = if let Some(stripped) = line.strip_prefix("data: ") {
                        stripped
                    } else {
                        continue;
                    };

                    if data == "[DONE]" {
                        return;
                    }

                    let chunk: OpenAIStreamChunk = match serde_json::from_str(data) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    for choice in &chunk.choices {
                        if let Some(ref delta) = choice.delta {
                            if let Some(ref text) = delta.content {
                                yield Ok(StreamEvent::ContentDelta { text: text.clone() });
                            }
                            if let Some(ref tool_calls) = delta.tool_calls {
                                for tc in tool_calls {
                                    yield Ok(StreamEvent::ToolUseDelta {
                                        id: tc.id.clone().unwrap_or_default(),
                                        name: tc.function.as_ref().and_then(|f| f.name.clone()),
                                        input_json: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                                    });
                                }
                            }
                        }
                        if let Some(ref reason) = choice.finish_reason {
                            yield Ok(StreamEvent::Stop { reason: reason.clone() });
                        }
                    }

                    if let Some(ref usage) = chunk.usage {
                        yield Ok(StreamEvent::Usage(UnifiedUsage {
                            input_tokens: usage.prompt_tokens,
                            output_tokens: usage.completion_tokens,
                            ..Default::default()
                        }));
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    fn health_check(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_supports_models() {
        let config = OpenAIProviderConfig::new("test-key".to_string());
        let provider = OpenAIProvider::new(config).unwrap();

        assert!(provider.supports_model("gpt-4o"));
        assert!(provider.supports_model("gpt-4o-mini"));
        assert!(provider.supports_model("gpt-4-turbo"));
        assert!(provider.supports_model("o1-preview"));
        assert!(provider.supports_model("o3-mini"));
        assert!(provider.supports_model("o4-mini"));
        assert!(provider.supports_model("chatgpt-4o-latest"));

        assert!(!provider.supports_model("claude-sonnet-4"));
        assert!(!provider.supports_model("gemini-2.0-flash"));
        assert!(!provider.supports_model("deepseek-chat"));
    }

    #[test]
    fn test_unified_to_openai_request() {
        use super::super::provider::{UnifiedMessage, UnifiedTool};

        let req = UnifiedChatRequest {
            model: "gpt-4o".to_string(),
            messages: vec![UnifiedMessage {
                role: "user".to_string(),
                content: UnifiedContent::Text("Hello".to_string()),
            }],
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            stream: false,
            tools: Some(vec![UnifiedTool {
                name: "get_weather".to_string(),
                description: Some("Get weather".to_string()),
                input_schema: serde_json::json!({"type": "object"}),
            }]),
            system: Some("You are helpful.".to_string()),
        };

        let openai_req = unified_to_openai_request(&req, false);
        assert_eq!(openai_req.model, "gpt-4o");
        assert_eq!(openai_req.messages.len(), 2); // system + user
        assert_eq!(openai_req.messages[0].role, "system");
        assert_eq!(openai_req.messages[1].role, "user");
        assert!(openai_req.tools.is_some());
        assert_eq!(openai_req.tools.unwrap().len(), 1);
    }

    #[test]
    fn test_openai_response_to_unified() {
        let resp = OpenAIResponse {
            id: "chatcmpl-123".to_string(),
            model: "gpt-4o".to_string(),
            choices: vec![OpenAIChoice {
                message: Some(OpenAIResponseMessage {
                    content: Some("Hello!".to_string()),
                    tool_calls: None,
                }),
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
            }),
        };

        let unified = openai_response_to_unified(resp);
        assert_eq!(unified.id, "chatcmpl-123");
        assert_eq!(unified.content.len(), 1);
        assert_eq!(unified.usage.input_tokens, 10);
        assert_eq!(unified.usage.output_tokens, 5);
    }

    #[test]
    fn test_openai_response_with_tool_calls() {
        let resp = OpenAIResponse {
            id: "chatcmpl-456".to_string(),
            model: "gpt-4o".to_string(),
            choices: vec![OpenAIChoice {
                message: Some(OpenAIResponseMessage {
                    content: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        id: "call_123".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: "get_weather".to_string(),
                            arguments: r#"{"city":"NYC"}"#.to_string(),
                        },
                    }]),
                }),
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: None,
        };

        let unified = openai_response_to_unified(resp);
        assert_eq!(unified.content.len(), 1);
        match &unified.content[0] {
            UnifiedContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["city"], "NYC");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_parse_openai_error_auth() {
        let body = r#"{"error":{"message":"Invalid API key","type":"auth","code":"invalid_api_key"}}"#;
        let err = parse_openai_error(reqwest::StatusCode::UNAUTHORIZED, body);
        assert!(matches!(err, ProviderError::Auth(_)));
    }

    #[test]
    fn test_parse_openai_error_rate_limit() {
        let body = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit","code":null}}"#;
        let err = parse_openai_error(reqwest::StatusCode::TOO_MANY_REQUESTS, body);
        assert!(matches!(err, ProviderError::RateLimited(_)));
    }

    #[test]
    fn test_custom_patterns() {
        let config = OpenAIProviderConfig::new("key".to_string());
        let patterns = vec!["my-model-*".to_string()];
        let provider = OpenAIProvider::with_patterns(config, patterns).unwrap();

        assert!(provider.supports_model("my-model-v1"));
        assert!(!provider.supports_model("gpt-4o"));
    }
}
