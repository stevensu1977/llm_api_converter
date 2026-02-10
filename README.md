# LLM API Converter

[中文文档](README.zh.md)

A high-performance LLM API gateway/proxy written in Rust that unifies different AI provider APIs, enabling seamless model interoperability across platforms.

## The Problem

Different AI providers use incompatible API formats:
- **Anthropic** uses the Messages API (`/v1/messages`)
- **OpenAI** uses the Chat Completions API (`/v1/chat/completions`)
- **AWS Bedrock** uses the Converse API
- **Google Gemini**, **DeepSeek**, and others have their own formats

This fragmentation means:
- Tools built for one API can't use models from other providers
- Switching providers requires significant code changes
- You're locked into specific ecosystems

## The Solution

LLM API Converter acts as a universal translation layer:

```
┌─────────────────┐      ┌──────────────────────┐      ┌─────────────────┐
│  Claude Code    │      │                      │      │  AWS Bedrock    │
│  (Anthropic)    │─────▶│                      │─────▶│  (Claude/Llama) │
├─────────────────┤      │                      │      ├─────────────────┤
│  OpenAI Client  │─────▶│  LLM API Converter   │─────▶│  DeepSeek API   │
├─────────────────┤      │                      │      ├─────────────────┤
│  Custom App     │─────▶│                      │─────▶│  OpenAI API     │
└─────────────────┘      └──────────────────────┘      └─────────────────┘
```

**Use Cases:**
- Run **Claude Code** with DeepSeek models
- Use **Gemini clients** with Claude models
- Access **AWS Bedrock models** via OpenAI-compatible API
- Build once, deploy anywhere

## Features

- **Multi-Protocol Support**: Anthropic Messages API, OpenAI Chat Completions API
- **Multiple Backends**: AWS Bedrock, with more coming (DeepSeek, OpenAI, etc.)
- **High Performance**: Written in Rust with async/await, handles thousands of concurrent requests
- **Streaming Support**: Full SSE streaming for real-time responses
- **Tool Calling**: Support for function/tool calling across different formats
- **Extended Thinking**: Support for Claude's extended thinking feature
- **Authentication**: API key management with DynamoDB
- **Rate Limiting**: Token bucket algorithm for fair usage
- **Metrics**: Prometheus metrics for monitoring
- **Docker Ready**: Production-ready Docker images

## Quick Start

### Prerequisites

- Rust 1.75+ (for building from source)
- AWS credentials configured (for Bedrock backend)
- Docker (optional, for containerized deployment)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/llm_api_converter.git
cd llm_api_converter

# Copy and configure environment
cp .env.example .env
# Edit .env with your settings

# Build and run
cargo build --release
cargo run --release
```

### Using Docker

```bash
# Build the image
docker build -f docker/Dockerfile -t llm-api-converter .

# Run with environment file
docker run -p 8000:8000 --env-file .env llm-api-converter
```

### Docker Compose (Full Stack)

```bash
# Start all services (proxy, DynamoDB Local, Prometheus, Grafana)
docker-compose up -d
```

## Configuration

Key environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `PORT` | Server port | `8000` |
| `AWS_REGION` | AWS region for Bedrock | `us-east-1` |
| `REQUIRE_API_KEY` | Enable API key auth | `true` |
| `RATE_LIMIT_ENABLED` | Enable rate limiting | `true` |
| `ENABLE_TOOL_USE` | Enable tool/function calling | `true` |
| `ENABLE_EXTENDED_THINKING` | Enable thinking blocks | `true` |

See [.env.example](.env.example) for full configuration options.

## API Endpoints

### Anthropic-Compatible

```bash
# Messages API
POST /v1/messages
Content-Type: application/json
x-api-key: your-api-key

{
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "Hello!"}
  ]
}
```

### OpenAI-Compatible

```bash
# Chat Completions API
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer your-api-key

{
  "model": "gpt-4",
  "messages": [
    {"role": "user", "content": "Hello!"}
  ]
}
```

### Health Check

```bash
GET /health
```

## Client Configuration

### Claude Code

Configure Claude Code to use this proxy instead of the official Anthropic API:

```bash
export CLAUDE_CODE_USE_BEDROCK=0
export ANTHROPIC_BASE_URL=<your-proxy-url>        # e.g., https://xxx.us-east-1.awsapprunner.com
export ANTHROPIC_API_KEY=sk-<your-api-key>        # API key from this proxy
export ANTHROPIC_MODEL=<model-id>                 # e.g., claude-sonnet-4-20250514
```

**Example with Bedrock models:**

```bash
export CLAUDE_CODE_USE_BEDROCK=0
export ANTHROPIC_BASE_URL=https://your-app-runner.us-east-1.awsapprunner.com
export ANTHROPIC_API_KEY=sk-your-api-key-here
export ANTHROPIC_MODEL=us.anthropic.claude-sonnet-4-20250514-v1:0
```

**Example with third-party models (via Bedrock):**

```bash
# Qwen
export CLAUDE_CODE_USE_BEDROCK=0
export ANTHROPIC_BASE_URL=https://your-app-runner.us-east-1.awsapprunner.com
export ANTHROPIC_API_KEY=sk-your-api-key-here
export ANTHROPIC_MODEL=qwen.qwen3-coder-480b-a35b-v1:0

# DeepSeek
export CLAUDE_CODE_USE_BEDROCK=0
export ANTHROPIC_BASE_URL=https://your-app-runner.us-east-1.awsapprunner.com
export ANTHROPIC_API_KEY=sk-your-api-key-here
export ANTHROPIC_MODEL=deepseek.deepseek-v3-v1:0
```

After setting these environment variables, run `claude` to start Claude Code with your configured backend.

## Architecture

```
src/
├── api/           # HTTP endpoint handlers
├── converters/    # API format converters
│   ├── anthropic_to_bedrock.rs
│   ├── bedrock_to_anthropic.rs
│   ├── openai_to_bedrock.rs
│   └── bedrock_to_openai.rs
├── schemas/       # Request/Response models
├── services/      # Business logic (Bedrock client, etc.)
├── middleware/    # Auth, rate limiting, logging
├── db/            # DynamoDB repositories
└── config/        # Configuration management
```

## Roadmap

- [x] Anthropic Messages API support
- [x] OpenAI Chat Completions API support
- [x] AWS Bedrock backend
- [x] Streaming responses
- [x] Tool/Function calling
- [ ] DeepSeek API backend
- [ ] Direct OpenAI backend
- [ ] Google Gemini backend
- [ ] Azure OpenAI backend
- [ ] Model aliasing and routing
- [ ] Request/Response caching
- [ ] Admin dashboard

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see [LICENSE](LICENSE) for details.
