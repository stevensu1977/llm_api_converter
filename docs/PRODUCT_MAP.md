# LLM API Converter - 产品路线图

> 最后更新: 2026-02-04

## 项目愿景

**统一的 LLM API 网关** - 让开发者使用一套 API 接口即可访问多种 LLM 服务商，降低接入成本，提高开发效率。

## 核心价值主张

- **统一 API 接口**: 一套代码兼容 Anthropic/OpenAI 格式，轻松切换后端
- **成本优化**: 通过 Bedrock 等云服务降低 API 调用成本
- **企业级特性**: 认证、限流、预算管理、审计日志等开箱即用

## 目标用户

| 用户类型 | 核心需求 |
|---------|---------|
| 企业开发团队 | 统一 LLM 接口、成本控制、使用审计 |
| 独立开发者 | 低成本访问多种 LLM、简化集成 |
| SaaS 平台提供商 | 为客户提供 LLM 能力、多租户支持 |

---

## 当前状态 (v0.2.0)

### 已完成功能

#### 核心功能
- [x] Anthropic Messages API 完整支持 (streaming/non-streaming)
- [x] OpenAI Chat Completions API 完整支持 (streaming/non-streaming)
- [x] AWS Bedrock 后端集成
- [x] Google Gemini 后端集成 ✨ **NEW**
- [x] Tool Calling / Function Calling 支持
- [x] Extended Thinking (Claude) 支持
- [x] 多模态内容 (文本、图片、文档)

#### 支持的后端
| 后端 | 状态 | 说明 |
|-----|------|------|
| AWS Bedrock | ✅ | Claude, Llama, Mistral 等 |
| Google Gemini | ✅ | gemini-2.5-flash, gemini-3-pro-preview 等 |
| DeepSeek | 🔜 | 计划中 |
| OpenAI 直连 | 🔜 | 计划中 |
| Anthropic 直连 | 🔜 | 计划中 |

#### 企业特性
- [x] API Key 认证 (x-api-key / Bearer token)
- [x] 基于 Token Bucket 的限流
- [x] 月度预算管理
- [x] Master Key 管理员访问
- [x] 请求追踪 (Trace ID)

#### 高级功能
- [x] PTC (Programmatic Tool Calling) - Docker 沙箱代码执行

#### 部署支持
- [x] Docker 镜像 (amd64/arm64)
- [x] AWS App Runner 部署脚本
- [x] DynamoDB 集成
- [x] 健康检查端点

---

## Phase 开发计划

### ~~Phase 1: 稳定性与文档~~ (延后)

**目标**: 生产就绪的稳定版本
**状态**: 延后，根据需求插入

| 任务 | 描述 | 状态 |
|-----|------|------|
| 单元测试 | 核心模块测试覆盖 >85% | 进行中 |
| 集成测试 | API 端到端测试 | [ ] |
| OpenAPI 文档 | Swagger 规范文档 | [ ] |
| 使用文档 | 快速入门、配置说明、最佳实践 | [ ] |
| 性能测试 | 压测基准、瓶颈分析 | [ ] |

---

### ~~Phase 2: Google Gemini 后端~~ ✅ 已完成

**目标**: 支持 Google Gemini 作为后端
**完成日期**: 2026-02-04

| 任务 | 描述 | 状态 |
|-----|------|------|
| Gemini 客户端 | API 客户端实现 | ✅ |
| 格式转换器 | Anthropic/OpenAI ↔ Gemini | ✅ |
| 流式响应 | SSE 流式输出支持 | ✅ |
| Tool Calling | 工具调用映射 | ✅ |
| 模型路由 | 基于模型名前缀路由 | ✅ |

**支持模型**:
- `gemini-2.5-flash`
- `gemini-2.5-flash-preview-09-2025`
- `gemini-2.5-flash-image`
- `gemini-3-flash-preview`
- `gemini-3-pro-preview`
- `gemini-3-pro-image-preview`

---

### Phase 2.5: 通用多 Key 管理框架 ✅ 核心完成

**目标**: 支持所有后端的多 API Key/凭证管理，实现负载均衡、故障切换、成本分摊
**完成日期**: 2026-02-04

#### 支持的后端

| 后端 | 凭证类型 | 状态 | 说明 |
|-----|---------|------|------|
| Gemini | API Key | ✅ | 完整支持多 Key 负载均衡 |
| Bedrock | AWS Profile / Access Key | ⚠️ | 框架就绪，待完整实现 |
| DeepSeek | API Key | 🔜 | 框架就绪，待后端集成 |
| Anthropic | API Key | 🔜 | 框架就绪，待后端集成 |
| OpenAI | API Key | 🔜 | 框架就绪，待后端集成 |
| Azure OpenAI | API Key + Endpoint | 🔜 | 框架就绪，待后端集成 |

#### 核心功能

| 功能 | 描述 | 状态 |
|-----|------|------|
| CredentialPool | 通用凭证池管理 | ✅ |
| LoadBalancer | 负载均衡策略 (round_robin/weighted/random/failover) | ✅ |
| HealthChecker | 健康检查，自动禁用/恢复 | ✅ |
| 配置解析 | 环境变量解析 (兼容旧格式) | ✅ |
| Gemini 适配 | 多 Key 负载均衡，自动故障切换 | ✅ |

#### 已实现架构

```
┌─────────────────────────────────────────────────────────┐
│                    CredentialPool<C>                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐        │
│  │ Credential 1│ │ Credential 2│ │ Credential 3│        │
│  │  weight=2   │ │  weight=1   │ │  weight=1   │        │
│  │  enabled ✓  │ │  enabled ✓  │ │  disabled ✗ │        │
│  └─────────────┘ └─────────────┘ └─────────────┘        │
│         │                                                │
│         ▼                                                │
│  ┌─────────────────────────────────────────────┐        │
│  │           LoadBalanceStrategy                │        │
│  │  - RoundRobin (默认)                         │        │
│  │  - Weighted (按权重分配)                     │        │
│  │  - Random (随机选择)                         │        │
│  │  - Failover (主备模式)                       │        │
│  └─────────────────────────────────────────────┘        │
│         │                                                │
│         ▼                                                │
│  ┌─────────────────────────────────────────────┐        │
│  │           CredentialHealth                   │        │
│  │  - 失败计数 → 自动禁用                       │        │
│  │  - 成功请求 → 重置计数                       │        │
│  │  - 定时恢复检测                              │        │
│  └─────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────┘
```

#### 配置示例
```bash
# Gemini 多 API Key (逗号分隔)
GEMINI_API_KEY=primary_key                    # 单 Key (兼容旧配置)
GEMINI_API_KEYS=key1,key2,key3               # 多 Key (新配置)
GEMINI_ENABLED=true

# Bedrock 多 Profile (格式: profile:region 或 name=profile:region)
BEDROCK_PROFILES=account1:us-east-1,account2:us-west-2

# 负载均衡配置
BACKEND_LOAD_BALANCE_STRATEGY=round_robin    # round_robin | weighted | random | failover
BACKEND_MAX_FAILURES=3                        # 最大失败次数后禁用
BACKEND_RETRY_AFTER_SECS=300                  # 禁用后重试间隔 (秒)
```

#### 关键文件
- `src/services/backend_pool/mod.rs` - 模块入口
- `src/services/backend_pool/credential.rs` - Credential trait 和实现
- `src/services/backend_pool/strategy.rs` - 负载均衡策略
- `src/services/backend_pool/pool.rs` - CredentialPool 泛型实现
- `src/services/gemini.rs` - Gemini 多 Key 实现
- `src/config/settings.rs` - 配置解析

---

### Phase 2.6: 审计日志系统

**目标**: 记录所有 API 请求用于合规审计
**预计周期**: 3-4 天

| 任务 | 描述 | 状态 |
|-----|------|------|
| 日志结构 | 请求/响应/Token/延迟等字段 | [ ] |
| 异步写入 | 高性能日志写入器 | [ ] |
| 日志轮转 | 按大小/日期自动轮转 | [ ] |
| 中间件 | Axum 审计中间件 | [ ] |

#### 日志字段
```json
{
  "timestamp": "2026-02-04T10:30:00Z",
  "request_id": "req_abc123",
  "api_key_id": "key_xyz",
  "model": "gemini-2.5-flash",
  "backend": "gemini",
  "backend_key": "primary",
  "input_tokens": 150,
  "output_tokens": 320,
  "latency_ms": 1250,
  "status": 200
}
```

#### 配置
```bash
AUDIT_LOG_ENABLED=true
AUDIT_LOG_DIR=./logs/audit
AUDIT_LOG_MAX_SIZE_MB=100
AUDIT_LOG_MAX_FILES=30
```

---

### Phase 2.7: Token 计费统计

**目标**: 按 API Key + 模型维度统计 Token 使用量
**预计周期**: 3-4 天

| 任务 | 描述 | 状态 |
|-----|------|------|
| 数据模型 | UsageStats 结构 | [ ] |
| 内存聚合 | 5分钟窗口聚合 | [ ] |
| DynamoDB 存储 | 持久化统计数据 | [ ] |
| 查询 API | /admin/usage 接口 | [ ] |

#### 查询 API
| 端点 | 描述 |
|-----|------|
| `GET /admin/usage?key_id=xxx` | 查询指定 Key 用量 |
| `GET /admin/usage?model=xxx` | 查询指定模型用量 |
| `GET /admin/usage/summary` | 聚合统计摘要 |

---

### Phase 3: 更多后端支持

**目标**: 扩展后端生态
**预计周期**: 4-5 周

#### 3.1 DeepSeek
| 任务 | 描述 | 状态 |
|-----|------|------|
| DeepSeek 客户端 | API 集成 | [ ] |
| 格式转换 | Anthropic/OpenAI -> DeepSeek | [ ] |
| 推理模型支持 | DeepSeek-R1 等 | [ ] |

#### 3.2 OpenAI 直连
| 任务 | 描述 | 状态 |
|-----|------|------|
| OpenAI 客户端 | API 客户端实现 | [ ] |
| 格式直通 | OpenAI 请求直接转发 | [ ] |

#### 3.3 Anthropic 直连
| 任务 | 描述 | 状态 |
|-----|------|------|
| Anthropic 客户端 | API 集成 | [ ] |
| 格式直通 | Anthropic 请求直接转发 | [ ] |

#### 3.4 Azure OpenAI
| 任务 | 描述 | 状态 |
|-----|------|------|
| Azure 客户端 | Azure OpenAI Service 集成 | [ ] |
| Azure AD 认证 | Managed Identity 支持 | [ ] |
| 部署配置 | 资源名/部署名映射 | [ ] |

---

### Phase 4: 企业功能增强

**目标**: 完善企业级管理能力
**预计周期**: 3-4 周

#### 4.1 管理 API
| 端点 | 描述 | 状态 |
|-----|------|------|
| `POST /admin/api-keys` | 创建 API Key | [ ] |
| `GET /admin/api-keys` | 列出所有 Keys | [ ] |
| `GET /admin/api-keys/{id}` | 获取 Key 详情 | [ ] |
| `PATCH /admin/api-keys/{id}` | 更新 Key 配置 | [ ] |
| `DELETE /admin/api-keys/{id}` | 删除/停用 Key | [ ] |

#### 4.2 计费系统增强
| 任务 | 描述 | 状态 |
|-----|------|------|
| 成本计算 | 按模型定价计算 | [ ] |
| 用量报表 | 日/周/月统计 | [ ] |
| 账单导出 | CSV/JSON 导出 | [ ] |

---

### Phase 5: 多云部署

**目标**: 支持主流云平台部署
**预计周期**: 3-4 周

| 平台 | 任务 | 状态 |
|-----|------|------|
| AWS ECS | Task Definition + Service 模板 | [ ] |
| AWS Lambda | Serverless 适配 (评估冷启动) | [ ] |
| GCP Cloud Run | 部署脚本 + 配置 | [ ] |
| CloudFlare Workers | Edge 部署 (架构调整) | [ ] |

#### 配套工具
| 工具 | 描述 | 状态 |
|-----|------|------|
| Terraform 模块 | IaC 一键部署 | [ ] |
| Helm Chart | Kubernetes 部署 | [ ] |
| 部署文档 | 各平台详细指南 | [ ] |

---

### Phase 6: v1.0.0 正式发布

**目标**: 首个稳定大版本
**前置条件**: Phase 2.5-4 完成

**发布检查清单**:
- [ ] 核心功能测试通过
- [ ] API 文档完整
- [ ] 至少支持 4 个后端 (Bedrock + Gemini + DeepSeek + OpenAI)
- [ ] 多 Key 负载均衡稳定
- [ ] 审计日志完整可用
- [ ] 安全审计通过
- [ ] 性能达到基准要求

---

## 长期愿景 (v2.0+)

### 计划中的功能

| 功能 | 描述 | 优先级 |
|-----|------|--------|
| 智能路由 | 根据成本/延迟/可用性自动选择后端 | 中 |
| 响应缓存 | 相同请求缓存减少 API 调用 | 中 |
| 管理后台 | Web UI 管理 API Key、查看统计 | 低 |
| 多租户 | 组织/团队级别隔离 | 低 |
| 插件系统 | 可扩展的请求/响应处理管道 | 低 |

---

## 发布节奏

- **发布周期**: 1-2 周
- **版本规范**: [Semantic Versioning](https://semver.org/)
- **分支策略**:
  - `main` - 稳定版本
  - `develop` - 开发分支
  - `feature/*` - 功能分支

---

## 技术栈

| 组件 | 技术 |
|-----|------|
| 语言 | Rust |
| Web 框架 | Axum 0.7 |
| 异步运行时 | Tokio |
| 数据库 | DynamoDB |
| 限流 | Governor (Token Bucket) |
| 缓存 | Moka |
| 容器化 | Docker (多架构) |
| CI/CD | GitHub Actions |

---

## 贡献指南

欢迎社区贡献！请参阅 [CONTRIBUTING.md](./CONTRIBUTING.md)

### 优先领域
1. 测试用例编写
2. 文档完善
3. 新后端适配
4. Bug 修复

---

## 联系方式

- **GitHub Issues**: [提交问题](https://github.com/stevensu1977/llm_api_converter/issues)
- **Discussions**: [讨论区](https://github.com/stevensu1977/llm_api_converter/discussions)
