# 🦀 Rust Migration TODO

This document tracks the implementation progress of migrating the Anthropic-Bedrock API Proxy from Python to Rust.

**Project Goal**: Migrate the Python FastAPI service to Rust (axum) to achieve better performance, smaller footprint, and faster deployment.

**Total Duration**: 18-24 weeks (4.5-6 months)

---

## 📊 Progress Overview

| Phase | Status | Progress | Duration |
|-------|--------|----------|----------|
| Phase 1: Foundation | 🟡 In Progress | 80% | 2-3 weeks |
| Phase 2: AWS Integration | ✅ Complete | 100% | 2-3 weeks |
| Phase 3: Core Conversion | ✅ Complete | 100% | 3-4 weeks |
| Phase 4: Auth & Rate Limiting | ✅ Complete | 100% | 1-2 weeks |
| Phase 5: Streaming | ✅ Complete | 100% | 2-3 weeks |
| Phase 6: Tool Calling | ✅ Complete | 100% | 2 weeks |
| Phase 7: PTC & Code Execution | ✅ Complete | 100% | 3-4 weeks |
| Phase 8: Observability | ⬜ Not Started | 0% | 1-2 weeks |
| Phase 9: Deployment | ✅ Complete | 100% | 2 weeks |
| Phase 10: Testing & Docs | ⬜ Not Started | 0% | 1-2 weeks |

**Overall Progress**: 8/10 phases complete (Phase 1.1-1.4 complete, Phase 2-7, Phase 9 complete)

---

## 🎯 Phase 1: Foundation (2-3 weeks)

**Goal**: Establish project skeleton and core framework

### 1.1 Project Initialization ✅
- [x] Create `Cargo.toml` with dependencies
  - [x] Add axum 0.7+ with macros feature
  - [x] Add tokio 1.35+ with full features
  - [x] Add serde with derive feature
  - [x] Add tower and tower-http
  - [x] Add tracing and tracing-subscriber
  - [x] Add config and dotenvy
  - [x] Add anyhow and thiserror
- [x] Create module structure following [architecture design](../RUST_MIGRATION.md#项目目录结构)
- [x] Configure compile optimizations (`.cargo/config.toml`)
  - [x] Set opt-level = 3
  - [x] Enable LTO
  - [x] Configure strip = true
  - [x] Set panic = "abort"

### 1.2 Configuration Management ✅
- [x] Implement `Settings` struct in `src/config/settings.rs`
  - [x] Define app settings (name, version, environment)
  - [x] Define server settings (host, port)
  - [x] Define AWS settings (region, credentials)
  - [x] Define DynamoDB table names
  - [x] Define authentication settings
  - [x] Define rate limit config
  - [x] Define feature flags
  - [x] Define model mapping
- [x] Implement `Settings::load()` method
  - [x] Load from environment variables
  - [x] Support .env file via dotenvy
  - [x] Implement config validation
- [x] Create `.env.example` file

### 1.3 Application Framework ✅
- [x] Implement `main.rs` entry point
  - [x] Initialize tracing subscriber
  - [x] Load settings
  - [x] Build and run app
  - [x] Handle graceful shutdown
- [x] Implement `AppState` in `src/server/state.rs`
  - [x] Define state struct with Arc wrappers
  - [x] Implement `AppState::new()` async constructor
  - [x] Initialize AWS SDK clients (placeholder, full in Phase 2)
- [x] Implement basic router in `src/server/routes.rs`
  - [x] Create router structure
  - [x] Add placeholder routes
- [x] Implement `src/server/app.rs`
  - [x] Define App struct
  - [x] Implement app builder
  - [x] Implement server runner

### 1.4 Logging & Tracing ✅
- [x] Configure tracing-subscriber
  - [x] JSON output format
  - [x] Environment filter (RUST_LOG)
  - [x] Request tracing middleware
- [x] Implement logging middleware in `src/middleware/logging.rs`
  - [x] Log request method, path, status
  - [x] Log request duration
  - [x] Include trace IDs

### 1.5 Testing Framework
- [ ] Configure unit test setup
  - [ ] Create `tests/` directory
  - [ ] Add test utilities
- [ ] Configure integration test setup
  - [ ] Add `tests/integration/` directory
  - [ ] Setup test fixtures
- [ ] Add AWS service mocking capability
  - [ ] Research aws-smithy-mocks-experimental
  - [ ] Create mock helpers

### ✅ Phase 1 Deliverables
- [x] Working web server that responds with "Hello World"
- [x] Configuration management functional
- [x] Structured JSON logging
- [ ] Basic test framework ready

---

## 🔌 Phase 2: AWS Integration (2-3 weeks)

**Goal**: Implement AWS Bedrock and DynamoDB integration

### 2.1 AWS SDK Setup ✅
- [x] Add AWS SDK dependencies to Cargo.toml
  - [x] aws-config 1.1+
  - [x] aws-sdk-bedrockruntime 1.11+
  - [x] aws-sdk-dynamodb 1.11+
- [x] Configure AWS SDK initialization
  - [x] Load region from settings
  - [x] Support credential providers
  - [x] Configure endpoint URLs (for local testing)

### 2.2 DynamoDB Operations ✅
- [x] Create `src/db/dynamodb.rs`
  - [x] Implement DynamoDbClient wrapper
  - [x] Add connection pooling/client caching
- [x] Create `src/db/models.rs`
  - [x] Define ApiKey model
  - [x] Define Usage model
  - [x] Define ModelMapping model
  - [x] Define UsageStats model
  - [x] Define ModelPricing model
- [x] Create repository pattern in `src/db/repositories/`
  - [x] Implement `api_key.rs`
    - [x] validate_api_key()
    - [x] get_api_key()
    - [x] increment_budget_used()
  - [x] Implement `usage.rs`
    - [x] record_usage()
    - [x] get_usage_stats()
    - [x] aggregate_usage()
  - [x] Implement `model_mapping.rs`
    - [x] get_bedrock_model_id()
    - [x] set_mapping()

### 2.3 Bedrock Service ✅
- [x] Create `src/services/bedrock.rs`
  - [x] Implement BedrockService struct
  - [x] Add Bedrock runtime client
  - [x] Implement Converse API call
  - [x] Implement InvokeModel API call
  - [x] Detect which API to use based on model ID
- [x] Implement error mapping
  - [x] Map Bedrock SDK errors to ApiError
  - [x] Handle throttling errors
  - [x] Handle validation errors

### 2.4 Retry & Timeout Strategy ✅
- [x] Create `src/utils/retry.rs`
  - [x] Implement exponential backoff
  - [x] Configure max retry attempts
  - [x] Add jitter to retry delays
- [x] Configure request timeouts
  - [x] Set Bedrock timeout
  - [x] Set DynamoDB timeout
- [x] (Optional) Implement circuit breaker pattern
  - [x] Skipped - can add later with tower-rs if needed

### ✅ Phase 2 Deliverables
- [x] Successful Bedrock Converse API calls
- [x] DynamoDB read/write operations working
- [x] Error handling comprehensive
- [x] Retry logic functional

---

## 🔄 Phase 3: Core Conversion Logic (3-4 weeks)

**Goal**: Implement Anthropic ↔ Bedrock format conversion

### 3.1 Schema Definitions ✅
- [x] Create `src/schemas/anthropic.rs`
  - [x] Define MessageRequest struct
  - [x] Define MessageResponse struct
  - [x] Define Message struct
  - [x] Define Content types (Text, Image, ToolUse, ToolResult)
  - [x] Define Tool struct
  - [x] Define ToolChoice enum
  - [x] Define Usage struct
  - [x] Add serde attributes
  - [x] Add validator rules
- [x] Create `src/schemas/bedrock.rs`
  - [x] Define ConverseRequest struct
  - [x] Define ConverseResponse struct
  - [x] Define ContentBlock types
  - [x] Define ToolSpec struct
  - [x] Define InferenceConfig struct
  - [x] Add serde attributes
- [ ] Create `src/schemas/ptc.rs`
  - [ ] Define PTC-specific types
  - [ ] Define ServerToolUse/ServerToolResult
  - [ ] Define Container struct

### 3.2 Request Conversion (Anthropic → Bedrock) ✅
- [x] Create `src/converters/anthropic_to_bedrock.rs`
- [x] Implement AnthropicToBedrockConverter struct
- [x] Implement `convert_request()` method
  - [x] Map model ID (Anthropic → Bedrock ARN)
  - [x] Convert messages (role, content)
  - [x] Convert system messages
  - [x] Convert tool definitions
  - [x] Map beta headers
  - [x] Convert inference config (temperature, max_tokens, etc.)
- [x] Implement content block conversion
  - [x] TextContent → text block
  - [x] ImageContent → image block (Base64)
  - [x] ToolUseContent → toolUse block
  - [x] ToolResultContent → toolResult block
- [x] Handle special features
  - [x] Multi-modal content (images, documents)
  - [x] Thinking blocks
  - [x] Cache control (cachePoint)
  - [x] Tool input examples

### 3.3 Response Conversion (Bedrock → Anthropic) ✅
- [x] Create `src/converters/bedrock_to_anthropic.rs`
- [x] Implement BedrockToAnthropicConverter struct
- [x] Implement `convert_response()` method
  - [x] Convert content blocks
  - [x] Map stop reasons
  - [x] Aggregate token usage
  - [x] Handle error responses
- [x] Implement content block conversion
  - [x] text → TextContent
  - [x] toolUse → ToolUseContent
  - [x] thinking → extended thinking block
- [x] Implement streaming event conversion
  - [x] messageStart → message_start
  - [x] contentBlockStart → content_block_start
  - [x] contentBlockDelta → content_block_delta
  - [x] contentBlockStop → content_block_stop
  - [x] messageStop → message_stop

### 3.4 API Endpoint Implementation ✅
- [x] Create `src/api/messages.rs`
- [x] Implement POST /v1/messages handler
  - [x] Parse request body
  - [x] Validate request
  - [x] Call BedrockService (InvokeModel for Claude models)
  - [x] Return response
- [x] Implement non-streaming path
- [x] Add error handling (ApiError type with HTTP status codes)
- [x] Add request/response logging
- [x] Implement POST /v1/messages/count_tokens endpoint
- [x] Wire up routes in routes.rs

### 3.5 Unit Testing ✅
- [x] Test request conversion
  - [x] Basic text messages
  - [x] Multi-modal content (images, documents)
  - [x] Tool definitions
  - [x] Edge cases (empty messages, multi-turn)
- [x] Test response conversion
  - [x] Text responses
  - [x] Tool use responses
  - [x] Error handling (invalid base64)
- [x] Test model ID mapping
- [x] 88 tests passing (good coverage for converters)

### ✅ Phase 3 Deliverables
- [x] Complete POST /v1/messages endpoint (non-streaming)
- [x] Unit test coverage - 88 tests passing
- [ ] Integration tests (deferred to Phase 10)

---

## 🔐 Phase 4: Auth & Rate Limiting (1-2 weeks) ✅

**Goal**: Implement security mechanisms

### 4.1 Authentication Middleware ✅
- [x] Create `src/middleware/auth.rs`
- [x] Implement `require_api_key` middleware
  - [x] Extract x-api-key header
  - [x] Check master key
  - [x] Validate against DynamoDB
  - [x] Inject ApiKeyInfo into request extensions
- [x] Define ApiKeyInfo struct
  - [x] api_key field (truncated for security)
  - [x] user_id field
  - [x] is_master field
  - [x] rate_limit field
  - [x] service_tier field
  - [x] monthly_budget and budget_used_mtd fields
- [x] Handle authentication errors
  - [x] Return 401 for missing key
  - [x] Return 401 for invalid key
  - [x] Return 403 for inactive key (with budget_exceeded reason)

### 4.2 Rate Limiting Middleware ✅
- [x] Add governor crate to dependencies
- [x] Add moka crate for caching
- [x] Create `src/middleware/rate_limit.rs`
- [x] Implement token bucket algorithm
  - [x] Use governor RateLimiter
  - [x] Per-API-key limiters
  - [x] Cache limiters in moka with TTL
- [x] Implement `rate_limit` middleware
  - [x] Get API key info from extensions
  - [x] Get or create rate limiter
  - [x] Check quota
  - [x] Return 429 if exceeded
- [x] Add rate limit response headers
  - [x] X-RateLimit-Limit
  - [x] X-RateLimit-Reset
  - [x] Retry-After (when limited)

### 4.3 Usage Tracking ✅
- [x] Update `src/services/usage_tracker.rs`
- [x] Implement UsageTracker struct
- [x] Implement record_usage method
  - [x] Capture request metadata
  - [x] Capture token counts
  - [x] Write to DynamoDB via UsageRepository
- [x] Implement budget tracking
  - [x] Calculate cost with service tier multiplier
  - [x] Increment budget_used via ApiKeyRepository
  - [x] Auto-deactivate on budget exceeded

### 4.4 Integration with Routes ✅
- [x] Add auth middleware to API routes
- [x] Add rate limit middleware to API routes
- [x] Ensure health check routes bypass auth
- [x] Middleware order: auth -> rate_limit -> handler

### ✅ Phase 4 Deliverables
- [x] Authentication middleware working (100 tests passing)
- [x] Rate limiting blocks excess requests
- [x] Usage tracking ready for DynamoDB writes
- [x] Budget enforcement functional

---

## 🌊 Phase 5: Streaming Responses (2-3 weeks) ✅

**Goal**: Implement Server-Sent Events streaming

### 5.1 SSE Foundation ✅
- [x] Research axum SSE support
  - [x] Study axum::response::Sse
  - [x] Review streaming examples
- [x] Create SSE helper utilities
  - [x] Event builder (in bedrock_to_anthropic.rs)
  - [x] Error handling in streams

### 5.2 Bedrock Streaming Integration ✅
- [x] Update `src/services/bedrock.rs`
- [x] Implement `invoke_model_with_response_stream()` method
  - [x] Call Bedrock InvokeModelWithResponseStream API
  - [x] Parse event stream via EventReceiver
  - [x] Handle stream errors via BedrockStreamError
- [x] Implement BedrockStreamResponse wrapper
  - [x] recv() for async iteration
  - [x] into_stream() for Stream trait
- [x] Implement StreamEventData for event parsing
  - [x] Chunk extraction
  - [x] Exception handling

### 5.3 Anthropic SSE Format ✅
- [x] Implement SSE event conversion (in bedrock_to_anthropic.rs)
  - [x] message_start event
  - [x] content_block_start event
  - [x] content_block_delta event
  - [x] content_block_stop event
  - [x] message_delta event
  - [x] message_stop event
  - [x] ping event (keep-alive)
  - [x] error event
- [x] Implement token accumulation
  - [x] Track input_tokens
  - [x] Track output_tokens
- [x] Format SSE output
  - [x] `event: <type>\n`
  - [x] `data: <json>\n\n`

### 5.4 Streaming Endpoint ✅
- [x] Update `src/api/messages.rs`
- [x] Implement MessageApiResponse enum (Json | Stream)
- [x] Implement streaming branch in handler
  - [x] Check `stream` parameter
  - [x] Return `Sse<Stream>`
  - [x] Set proper headers
- [x] Implement create_streaming_response()
  - [x] Build native Anthropic request with stream=true
  - [x] Convert Bedrock streaming events to SSE
- [x] Implement convert_native_stream_event()
  - [x] Handle all event types

### 5.5 Testing ✅
- [x] Unit tests for streaming event conversion
  - [x] test_native_stream_event_parsing
  - [x] test_convert_native_stream_event_content_block_start
  - [x] test_convert_native_stream_event_content_block_delta
  - [x] test_convert_native_stream_event_content_block_stop
  - [x] test_convert_native_stream_event_message_delta
  - [x] test_convert_native_stream_event_message_start
  - [x] test_convert_native_stream_event_ping
  - [x] test_build_native_request_with_stream
- [x] 108 tests passing

### ✅ Phase 5 Deliverables
- [x] POST /v1/messages with stream=true working
- [x] SSE format compliant with Anthropic API
- [x] Errors propagated as SSE error events
- [x] Token tracking in streaming responses

---

## ✅ Phase 6: Tool Calling - COMPLETED

**Goal**: Implement Tool Use functionality

### 6.1 Tool Definition Conversion ✅
- [x] Update `src/converters/anthropic_to_bedrock.rs`
- [x] Implement tool definition conversion
  - [x] Anthropic Tool → Bedrock ToolSpec
  - [x] Map input_schema
  - [x] Support input_examples via additionalModelRequestFields
- [x] Handle beta header for tool examples
  - [x] Pass tools with input_examples through additionalModelRequestFields.tools

### 6.2 Tool Use Response Conversion ✅
- [x] Update `src/converters/bedrock_to_anthropic.rs`
- [x] Implement toolUse block conversion
  - [x] Extract tool_use_id
  - [x] Extract name
  - [x] Extract input
- [x] Implement toolResult conversion
  - [x] Handle tool_result from client
  - [x] Convert to Bedrock format
  - [x] Handle error status

### 6.3 Multi-Turn Conversation ✅
- [x] Support tool call → result → continue flow
  - [x] Preserve conversation history
  - [x] Append tool results to messages
  - [x] Continue conversation with Bedrock

### 6.4 Tool Choice Configuration ✅
- [x] Implement ToolChoice mapping
  - [x] auto → auto mode
  - [x] any → any mode
  - [x] tool → specific tool mode
  - [x] Object form → parse and convert
  - [x] Unknown values → fallback to auto
- [x] Validate tool choice settings

### 6.5 Testing ✅
- [x] Test tool definitions (test_tool_config_conversion)
- [x] Test tool use responses (test_tool_use_response_with_tool_stop_reason)
- [x] Test multi-turn tool conversations (test_multi_turn_tool_use_conversation)
- [x] Test tool choice settings (test_tool_choice_conversion with all cases)
- [x] Test streaming with tools (test_stream_content_block_start_tool_use, test_stream_delta_tool_use)
- [x] Test input_examples handling (test_tools_have_input_examples, test_tools_with_input_examples_use_additional_fields)
- [x] 117 tests passing

### ✅ Phase 6 Deliverables
- [x] Tool calling fully functional
- [x] Multi-turn conversations work
- [x] Streaming with tools works
- [x] Tool examples supported via additionalModelRequestFields

---

## ✅ Phase 7: PTC & Code Execution - COMPLETED

**Goal**: Implement Programmatic Tool Calling and code execution

### 7.1 Docker Integration ✅
- [x] Add bollard crate to dependencies
- [x] Create `src/services/ptc/sandbox.rs`
- [x] Implement Docker client wrapper
  - [x] Initialize Docker connection
  - [x] Test Docker availability
- [x] Implement container lifecycle management
  - [x] Create container
  - [x] Start container
  - [x] Stop container
  - [x] Remove container
  - [x] Get container logs
- [x] Configure container settings
  - [x] Memory limits
  - [x] CPU limits
  - [x] Network disabled
  - [x] Security options

### 7.2 PTC Service ✅
- [x] Create `src/services/ptc/service.rs`
- [x] Implement PtcService struct
  - [x] Session management (HashMap<String, Session>)
  - [x] Container pool
- [x] Implement PTC detection
  - [x] Check beta header
  - [x] Check for code_execution tool
  - [x] Check for allowed_callers
- [x] Implement `is_ptc_request()` method
  - [x] Detect PTC mode via beta header and code_execution tool
- [x] Implement `execute_code()` method
  - [x] Create/reuse container
  - [x] Inject runner.py script
  - [x] Execute code
  - [x] Capture stdout/stderr
- [x] Implement session state management
  - [x] SessionState enum (Active, WaitingForToolResults, Executing, Completed, Expired)
  - [x] Pending tool call tracking

### 7.3 Sandbox Communication ✅
- [x] Implement IPC protocol
  - [x] File-based communication (TOOL_CALLS_FILE, TOOL_RESULTS_FILE, STATUS_FILE)
  - [x] JSON message format
  - [x] Tool call request message
  - [x] Tool result response message
- [x] Implement runner.py injection
  - [x] Use put_archive API (not bind mount)
  - [x] Copy file to /tmp/runner.py via tar archive
  - [x] Set executable permissions
- [x] Implement tool call batching
  - [x] Collect parallel tool calls (100ms window via ToolCallBatcher)
  - [x] Return batched tool_use blocks
  - [x] Accept batched tool_result blocks

### 7.4 Session Management ✅
- [x] Implement session storage
  - [x] In-memory HashMap with Arc<RwLock>
  - [x] Session timeout (4.5 minutes - DEFAULT_SESSION_TIMEOUT_SECS)
  - [x] Automatic cleanup (cleanup_expired_sessions)
- [x] Implement container reuse
  - [x] Include container.id in response
  - [x] Accept container.id in continuation
  - [x] Validate container exists (validate_container_id)
- [x] Handle session expiry
  - [x] cleanup_expired_sessions() method
  - [x] Return error for expired session (PtcError::SessionExpired)

### 7.5 Code Execution ✅
- [x] Create `src/services/ptc/runner.rs`
- [x] Implement Python runner script (RUNNER_SCRIPT)
  - [x] execute_code() function
  - [x] call_tool() function
  - [x] async_call_tool() for parallel execution
- [x] Implement iteration limits
  - [x] Max code execution rounds (DEFAULT_MAX_ITERATIONS)
  - [x] Prevent infinite loops (MaxIterationsExceeded error)

### 7.6 Error Handling ✅
- [x] Create `src/services/ptc/exceptions.rs`
- [x] Handle code execution timeout
  - [x] Kill container on timeout
  - [x] Return timeout error (PtcError::ExecutionTimeout)
- [x] Handle container crashes
  - [x] Detect container exit
  - [x] Clean up resources
  - [x] Return error to client (PtcError::ContainerDied)
- [x] Handle Docker daemon errors
  - [x] Connection lost (PtcError::DockerError)
  - [x] Image pull failures (PtcError::ImagePullFailed)

### 7.7 Health Check ✅
- [x] Update `src/api/health.rs`
- [x] Implement GET /health/ptc endpoint
  - [x] Check Docker connection
  - [x] Return active sessions count
  - [x] Return container status
- [x] Add PtcHealthResponse struct
- [x] Integrate PtcService into AppState

### ✅ Phase 7 Deliverables
- [x] PTC functionality working (134 tests passing)
- [x] Code execution functional (sandbox.rs)
- [x] Container lifecycle managed (SandboxExecutor)
- [x] Session management working (PtcSession, HashMap<String, PtcSession>)
- [x] Multi-round code execution supported (iteration tracking)
- [x] Parallel tool calls supported (100ms batch window)
- [x] Health check endpoint working (/health/ptc)

---

## 📊 Phase 8: Observability (1-2 weeks)

**Goal**: Implement Prometheus metrics and health checks

### 8.1 Prometheus Metrics
- [ ] Add prometheus crate to dependencies
- [ ] Create `src/middleware/metrics.rs`
- [ ] Define metric collectors
  - [ ] http_requests_total (Counter)
  - [ ] http_request_duration_seconds (Histogram)
  - [ ] bedrock_calls_total (Counter)
  - [ ] bedrock_call_duration_seconds (Histogram)
  - [ ] token_usage_total (Counter)
  - [ ] rate_limit_exceeded_total (Counter)
  - [ ] auth_failures_total (Counter)
  - [ ] ptc_sessions_active (Gauge)
  - [ ] ptc_container_errors_total (Counter)
- [ ] Implement metrics middleware
  - [ ] Record request start
  - [ ] Record request completion
  - [ ] Update counters and histograms
- [ ] Implement GET /metrics endpoint
  - [ ] Export Prometheus format
  - [ ] Include process metrics

### 8.2 Health Checks
- [ ] Update `src/api/health.rs`
- [ ] Implement GET /health endpoint
  - [ ] Overall service status
  - [ ] Component health checks
- [ ] Implement GET /ready endpoint
  - [ ] Readiness probe for k8s
  - [ ] Check AWS connectivity
  - [ ] Check DynamoDB connectivity
  - [ ] Check Docker (if PTC enabled)
- [ ] Implement GET /liveness endpoint
  - [ ] Liveness probe for k8s
  - [ ] Simple "alive" check

### 8.3 Structured Logging
- [ ] Review tracing setup
- [ ] Add structured fields
  - [ ] request_id
  - [ ] user_id
  - [ ] model_id
  - [ ] duration_ms
  - [ ] status_code
- [ ] Add trace IDs
  - [ ] Generate trace ID per request
  - [ ] Propagate through services

### 8.4 (Optional) Distributed Tracing
- [ ] Research OpenTelemetry integration
  - [ ] opentelemetry crate
  - [ ] tracing-opentelemetry
- [ ] Configure OTLP export
  - [ ] Jaeger endpoint
  - [ ] Tempo endpoint
- [ ] Add span instrumentation
  - [ ] HTTP handlers
  - [ ] Bedrock calls
  - [ ] DynamoDB operations

### 8.5 Grafana Dashboard
- [ ] Create Grafana dashboard JSON
  - [ ] Request rate panel
  - [ ] Latency percentiles panel
  - [ ] Error rate panel
  - [ ] Token usage panel
  - [ ] PTC sessions panel
- [ ] Document dashboard import

### ✅ Phase 8 Deliverables
- [ ] Prometheus metrics endpoint working
- [ ] Grafana dashboard available
- [ ] Health check endpoints working
- [ ] Structured logging comprehensive

---

## ✅ Phase 9: Deployment Optimization - COMPLETED

**Goal**: Optimize Docker images and deployment

### 9.1 Docker Image Optimization ✅
- [x] Create `docker/Dockerfile` (scratch-based, minimal image)
- [x] Implement multi-stage build
  - [x] Stage 1: Build (rust:1.75-slim-bookworm with musl)
  - [x] Stage 2: Runtime (scratch)
- [x] Configure static linking
  - [x] Install musl-tools
  - [x] Add x86_64-unknown-linux-musl target
  - [x] Add aarch64-unknown-linux-musl target (for ARM)
  - [x] Build with musl target
- [x] Optimize binary
  - [x] Strip symbols (strip = true)
  - [x] Enable LTO (lto = "fat")
  - [x] Set panic=abort
- [x] Copy CA certificates (for HTTPS)
- [x] Use non-root user (UID 1000)
- [x] Target image size: <20MB

### 9.2 Alternative Alpine Image ✅
- [x] Create `docker/Dockerfile.alpine`
- [x] Use alpine:3.19 as runtime base
- [x] Install minimal dependencies (ca-certificates, curl)
- [x] Include health check with curl

### 9.3 Compile Optimizations ✅
- [x] Review `.cargo/config.toml` (already configured)
- [x] Set release profile optimizations
  - [x] opt-level = 3
  - [x] lto = "fat"
  - [x] codegen-units = 1
  - [x] strip = true
  - [x] panic = "abort"
- [x] Configure parallel compilation (jobs = 8)
- [x] Configure musl targets with crt-static

### 9.4 CDK Integration ✅
- [x] Create `DEPLOYMENT.md` with CDK update instructions
- [x] Document Docker build path changes for rust-project
- [x] Document memory/CPU allocation recommendations
- [x] Document environment variable mapping
- [x] Document Fargate vs EC2 deployment options

### 9.5 Build Scripts & Docker Compose ✅
- [x] Create `scripts/build-docker.sh`
  - [x] Support platform selection (amd64, arm64, all)
  - [x] Support image type selection (minimal, alpine, ptc)
  - [x] Support registry push
  - [x] Multi-platform buildx support
- [x] Create `docker-compose.yml`
  - [x] Main proxy service
  - [x] PTC-enabled proxy service (profile: ptc)
  - [x] DynamoDB Local
  - [x] DynamoDB table init (profile: init)
  - [x] Prometheus & Grafana (profile: monitoring)
- [x] Create `.dockerignore` for efficient builds

### 9.6 PTC Docker Image ✅
- [x] Create `docker/Dockerfile.ptc`
- [x] Include Docker CLI for PTC support
- [x] Configure for Docker socket mounting
- [x] Set PTC environment defaults

### 9.7 Monitoring Configuration ✅
- [x] Create `deployment/prometheus/prometheus.yml`
- [x] Create `deployment/grafana/provisioning/datasources/prometheus.yml`

### ✅ Phase 9 Deliverables
- [x] Docker images (3 variants: minimal/alpine/ptc)
- [x] Build scripts with multi-platform support
- [x] Docker Compose for local development
- [x] Deployment documentation (DEPLOYMENT.md)
- [x] Compile optimizations configured

---

## 📚 Phase 10: Testing & Documentation (1-2 weeks)

**Goal**: Complete testing and documentation

### 10.1 Unit Testing
- [ ] Achieve >85% test coverage
  - [ ] Run `cargo tarpaulin` or similar
  - [ ] Identify gaps
  - [ ] Write missing tests
- [ ] Test edge cases
  - [ ] Empty requests
  - [ ] Invalid JSON
  - [ ] Missing required fields
  - [ ] Oversized requests
- [ ] Test error scenarios
  - [ ] Network failures
  - [ ] Timeout errors
  - [ ] Rate limit exceeded
  - [ ] Auth failures

### 10.2 Integration Testing
- [ ] Create end-to-end test suite
  - [ ] Test complete request flow
  - [ ] Test streaming flow
  - [ ] Test tool calling flow
  - [ ] Test PTC flow
- [ ] Mock AWS services
  - [ ] Use aws-smithy-mocks or similar
  - [ ] Mock DynamoDB responses
  - [ ] Mock Bedrock responses
- [ ] Test concurrent requests
- [ ] Test resource cleanup

### 10.3 Documentation
- [ ] Update README.md
  - [ ] Installation instructions (Rust)
  - [ ] Building from source
  - [ ] Running locally
  - [ ] Configuration guide
  - [ ] Docker instructions
- [ ] Create API documentation
  - [ ] Add utoipa crate
  - [ ] Annotate handlers with OpenAPI
  - [ ] Generate Swagger UI
  - [ ] Host at /docs endpoint
- [ ] Create deployment documentation
  - [ ] Docker deployment
  - [ ] Kubernetes deployment
  - [ ] ECS deployment (CDK)
  - [ ] Environment variables reference
- [ ] Create migration guide
  - [ ] Python → Rust comparison
  - [ ] Breaking changes (if any)
  - [ ] Configuration mapping
- [ ] Create troubleshooting guide
  - [ ] Common errors
  - [ ] Debug steps
  - [ ] Performance tuning

### 10.4 Performance Report
- [ ] Document benchmark results
  - [ ] Throughput comparison
  - [ ] Latency comparison
  - [ ] Memory usage comparison
  - [ ] Cold start comparison
  - [ ] Image size comparison
- [ ] Create cost analysis
  - [ ] ECS Fargate cost savings
  - [ ] Resource utilization
  - [ ] ROI calculation
- [ ] Create performance tuning guide

### 10.5 Code Quality
- [ ] Run cargo clippy
  - [ ] Fix all warnings
  - [ ] Enable clippy in CI
- [ ] Run cargo fmt
  - [ ] Format all code
  - [ ] Enable fmt check in CI
- [ ] Setup CI/CD pipeline
  - [ ] GitHub Actions workflow
  - [ ] Test on PR
  - [ ] Build Docker image
  - [ ] Security scan
  - [ ] Deploy to staging

### ✅ Phase 10 Deliverables
- [ ] Test suite complete (>85% coverage)
- [ ] User documentation complete
- [ ] API documentation (Swagger)
- [ ] Performance report published
- [ ] CI/CD pipeline running

---

## 🎯 Acceptance Criteria

### Functional Completeness
- [ ] All Python features implemented
- [ ] 100% API compatibility with Anthropic SDK
- [ ] PTC working
- [ ] Independent code execution working
- [ ] Streaming working
- [ ] Tool calling working
- [ ] Budget tracking working
- [ ] Rate limiting working

### Performance Targets
- [ ] Cold start <100ms
- [ ] Memory usage <50MB (idle)
- [ ] P99 latency <200ms
- [ ] Throughput >5000 RPS
- [ ] Docker image <20MB

### Quality Standards
- [ ] Unit test coverage >85%
- [ ] Integration tests 100% passing
- [ ] No memory leaks
- [ ] No data races (cargo clippy clean)
- [ ] Code formatted (cargo fmt)

### Documentation Completeness
- [ ] README.md updated
- [ ] API documentation generated
- [ ] Deployment guide written
- [ ] Migration guide written
- [ ] Troubleshooting handbook created

---

## 📈 Risk Tracking

### High Priority Risks

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| Team learning curve | 🔴 High | Phased migration, training | Open |
| PTC complexity | 🟡 Medium | Incremental implementation | Open |
| SSE streaming bugs | 🟡 Medium | Thorough testing, examples | Open |
| Performance not meeting targets | 🟡 Medium | Early benchmarking | Open |

---

## 🚦 Next Steps

### Immediate Actions (Week 1)
1. [ ] Initialize Cargo project
   ```bash
   cargo new anthropic-bedrock-proxy --bin
   cd anthropic-bedrock-proxy
   ```
2. [ ] Add core dependencies to Cargo.toml
3. [ ] Create module structure
4. [ ] Setup development environment (rust-analyzer, IDE)
5. [ ] Install cargo-watch for auto-reload
6. [ ] Begin Phase 1 implementation

### Short Term (Week 2-4)
1. [ ] Complete Phase 1 (Foundation)
2. [ ] Start Phase 2 (AWS Integration)
3. [ ] Setup testing framework
4. [ ] Create first integration test

### Medium Term (Month 2-3)
1. [ ] Complete Phase 2-4
2. [ ] Achieve basic API compatibility
3. [ ] Run first performance benchmarks
4. [ ] Demo to stakeholders

### Long Term (Month 4-6)
1. [ ] Complete all phases
2. [ ] Production deployment
3. [ ] Monitor performance
4. [ ] Gather feedback

---

## 📝 Notes

- This TODO is a living document - update as progress is made
- Mark items with ✅ when complete
- Add notes/blockers as needed
- Update progress percentages weekly
- Review and adjust timeline based on actual progress

---

**Document Version**: 1.0.0
**Last Updated**: 2026-01-12
**Owner**: Development Team
**Status**: In Progress
