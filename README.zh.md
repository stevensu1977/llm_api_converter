# LLM API Converter

[English](README.md)

一个用 Rust 编写的高性能 LLM API 网关/代理，统一不同 AI 厂商的 API 格式，实现跨平台的模型互操作。

## 问题背景

不同的 AI 厂商使用互不兼容的 API 格式：
- **Anthropic** 使用 Messages API (`/v1/messages`)
- **OpenAI** 使用 Chat Completions API (`/v1/chat/completions`)
- **AWS Bedrock** 使用 Converse API
- **Google Gemini**、**DeepSeek** 等都有各自的格式

这种碎片化导致：
- 为某个 API 构建的工具无法使用其他厂商的模型
- 切换厂商需要大量代码修改
- 用户被锁定在特定生态系统中

## 解决方案

LLM API Converter 作为通用的 API 翻译层：

```
┌─────────────────┐      ┌──────────────────────┐      ┌─────────────────┐
│  Claude Code    │      │                      │      │  AWS Bedrock    │
│  (Anthropic)    │─────▶│                      │─────▶│  (Claude/Llama) │
├─────────────────┤      │                      │      ├─────────────────┤
│  OpenAI 客户端  │─────▶│  LLM API Converter   │─────▶│  DeepSeek API   │
├─────────────────┤      │                      │      ├─────────────────┤
│  自定义应用     │─────▶│                      │─────▶│  OpenAI API     │
└─────────────────┘      └──────────────────────┘      └─────────────────┘
```

**典型用例：**
- 让 **Claude Code** 使用 DeepSeek 模型
- 让 **Gemini 客户端** 使用 Claude 模型
- 通过 OpenAI 兼容 API 访问 **AWS Bedrock 模型**
- 一次开发，随处部署

## 功能特性

- **多协议支持**：Anthropic Messages API、OpenAI Chat Completions API
- **多后端支持**：AWS Bedrock（更多后端开发中：DeepSeek、OpenAI 等）
- **高性能**：Rust 编写，async/await 异步架构，支持数千并发请求
- **流式响应**：完整的 SSE 流式支持，实时返回响应
- **工具调用**：跨不同格式支持函数/工具调用
- **扩展思考**：支持 Claude 的扩展思考功能
- **身份认证**：基于 DynamoDB 的 API 密钥管理
- **速率限制**：令牌桶算法实现公平使用
- **监控指标**：Prometheus 指标导出
- **容器就绪**：生产级 Docker 镜像

## 快速开始

### 前置要求

- Rust 1.75+（源码编译）
- AWS 凭证配置（用于 Bedrock 后端）
- Docker（可选，用于容器化部署）

### 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/llm_api_converter.git
cd llm_api_converter

# 复制并配置环境变量
cp .env.example .env
# 编辑 .env 配置您的设置

# 构建并运行
cargo build --release
cargo run --release
```

### 使用 Docker

```bash
# 构建镜像
docker build -f docker/Dockerfile -t llm-api-converter .

# 使用环境变量文件运行
docker run -p 8000:8000 --env-file .env llm-api-converter
```

### Docker Compose（完整堆栈）

```bash
# 启动所有服务（代理、DynamoDB Local、Prometheus、Grafana）
docker-compose up -d
```

## 配置说明

关键环境变量：

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `PORT` | 服务端口 | `8000` |
| `AWS_REGION` | Bedrock 的 AWS 区域 | `us-east-1` |
| `REQUIRE_API_KEY` | 启用 API 密钥认证 | `true` |
| `RATE_LIMIT_ENABLED` | 启用速率限制 | `true` |
| `ENABLE_TOOL_USE` | 启用工具/函数调用 | `true` |
| `ENABLE_EXTENDED_THINKING` | 启用思考块 | `true` |

完整配置选项请参见 [.env.example](.env.example)。

## API 端点

### Anthropic 兼容接口

```bash
# Messages API
POST /v1/messages
Content-Type: application/json
x-api-key: your-api-key

{
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "你好！"}
  ]
}
```

### OpenAI 兼容接口

```bash
# Chat Completions API
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer your-api-key

{
  "model": "gpt-4",
  "messages": [
    {"role": "user", "content": "你好！"}
  ]
}
```

### 健康检查

```bash
GET /health
```

## 项目架构

```
src/
├── api/           # HTTP 端点处理器
├── converters/    # API 格式转换器
│   ├── anthropic_to_bedrock.rs
│   ├── bedrock_to_anthropic.rs
│   ├── openai_to_bedrock.rs
│   └── bedrock_to_openai.rs
├── schemas/       # 请求/响应模型
├── services/      # 业务逻辑（Bedrock 客户端等）
├── middleware/    # 认证、速率限制、日志
├── db/            # DynamoDB 数据访问层
└── config/        # 配置管理
```

## 开发路线图

- [x] Anthropic Messages API 支持
- [x] OpenAI Chat Completions API 支持
- [x] AWS Bedrock 后端
- [x] 流式响应
- [x] 工具/函数调用
- [ ] DeepSeek API 后端
- [ ] 直连 OpenAI 后端
- [ ] Google Gemini 后端
- [ ] Azure OpenAI 后端
- [ ] 模型别名和路由
- [ ] 请求/响应缓存
- [ ] 管理后台

## 贡献

欢迎贡献！请随时提交 Pull Request。

## 许可证

MIT 许可证 - 详见 [LICENSE](LICENSE)。
