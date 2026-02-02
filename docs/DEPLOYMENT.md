# AWS App Runner 部署指南

本文档介绍如何将 LLM API Converter 部署到 AWS App Runner，支持 AMD64 和 ARM64 两种架构。

## 前置要求

### 1. 安装工具

```bash
# macOS
brew install zig
cargo install cargo-zigbuild

# 添加 Rust 交叉编译目标
rustup target add x86_64-unknown-linux-gnu
rustup target add aarch64-unknown-linux-gnu
```

### 2. 配置 AWS CLI

```bash
aws configure
# 确保有以下权限：ECR, App Runner, IAM, DynamoDB, Bedrock
```

### 3. 创建 DynamoDB 表

```bash
cargo run --bin setup_tables
```

### 4. 创建 API Key

```bash
./scripts/create-api-key.sh -u your-email@example.com -n "Production Key"
```

---

## 方案一：AMD64 部署（推荐）

AMD64 兼容性最好，适合大多数场景。

### 步骤 1：交叉编译

```bash
cargo zigbuild --release --target x86_64-unknown-linux-gnu
```

### 步骤 2：构建 Docker 镜像

```bash
docker buildx build --platform linux/amd64 \
  --build-arg BINARY_PATH=target/x86_64-unknown-linux-gnu/release/llm-api-converter \
  -f docker/Dockerfile.prebuilt \
  -t llm-api-converter:latest \
  --load .
```

### 步骤 3：推送到 ECR

```bash
# 设置变量
REGION=us-west-2
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
ECR_REPO="${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com/llm-api-converter"

# 创建 ECR 仓库（如果不存在）
aws ecr create-repository --repository-name llm-api-converter --region ${REGION} 2>/dev/null || true

# 登录 ECR
aws ecr get-login-password --region ${REGION} | \
  docker login --username AWS --password-stdin ${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com

# 推送镜像
docker tag llm-api-converter:latest ${ECR_REPO}:latest
docker push ${ECR_REPO}:latest
```

### 步骤 4：部署到 App Runner

```bash
./scripts/deploy-apprunner.sh --create -p amd64 -r us-west-2
```

或手动创建：

```bash
REGION=us-west-2
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
ECR_REPO="${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com/llm-api-converter"

aws apprunner create-service \
  --region ${REGION} \
  --service-name llm-api-converter \
  --source-configuration "{
    \"AuthenticationConfiguration\": {
      \"AccessRoleArn\": \"arn:aws:iam::${ACCOUNT_ID}:role/llm-api-converter-apprunner-role\"
    },
    \"ImageRepository\": {
      \"ImageIdentifier\": \"${ECR_REPO}:latest\",
      \"ImageRepositoryType\": \"ECR\",
      \"ImageConfiguration\": {
        \"Port\": \"8000\",
        \"RuntimeEnvironmentVariables\": {
          \"AWS_REGION\": \"${REGION}\",
          \"REQUIRE_API_KEY\": \"true\",
          \"LOG_LEVEL\": \"info\"
        }
      }
    }
  }" \
  --instance-configuration "{
    \"Cpu\": \"1 vCPU\",
    \"Memory\": \"2 GB\",
    \"InstanceRoleArn\": \"arn:aws:iam::${ACCOUNT_ID}:role/llm-api-converter-instance-role\"
  }" \
  --health-check-configuration "{
    \"Protocol\": \"HTTP\",
    \"Path\": \"/health\",
    \"Interval\": 10,
    \"Timeout\": 5,
    \"HealthyThreshold\": 1,
    \"UnhealthyThreshold\": 3
  }"
```

---

## 方案二：ARM64 部署（Graviton）

ARM64 使用 AWS Graviton 处理器，成本更低但需要确保镜像架构正确。

### 步骤 1：交叉编译

```bash
cargo zigbuild --release --target aarch64-unknown-linux-gnu
```

### 步骤 2：构建 Docker 镜像

```bash
docker buildx build --platform linux/arm64 \
  --build-arg BINARY_PATH=target/aarch64-unknown-linux-gnu/release/llm-api-converter \
  -f docker/Dockerfile.prebuilt \
  -t llm-api-converter:latest \
  --load .
```

### 步骤 3：推送到 ECR

与 AMD64 相同：

```bash
REGION=us-west-2
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
ECR_REPO="${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com/llm-api-converter"

aws ecr get-login-password --region ${REGION} | \
  docker login --username AWS --password-stdin ${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com

docker tag llm-api-converter:latest ${ECR_REPO}:latest
docker push ${ECR_REPO}:latest
```

### 步骤 4：部署到 App Runner

```bash
./scripts/deploy-apprunner.sh --create -p arm64 -r us-west-2
```

---

## 一键部署脚本

使用 `deploy-apprunner.sh` 可以简化部署流程：

```bash
# AMD64 部署
./scripts/deploy-apprunner.sh --create -p amd64 -r us-west-2

# ARM64 部署
./scripts/deploy-apprunner.sh --create -p arm64 -r us-west-2

# 更新已有服务
./scripts/deploy-apprunner.sh -p amd64 -r us-west-2
```

### 脚本参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `-r, --region` | AWS 区域 | us-east-1 |
| `-n, --name` | 服务名称 | llm-api-converter |
| `-p, --platform` | 平台架构 (amd64/arm64) | arm64 |
| `--create` | 创建新服务 | 更新已有服务 |

---

## 环境变量配置

在 App Runner 控制台或部署时可配置以下环境变量：

| 变量 | 说明 | 必需 |
|------|------|------|
| `AWS_REGION` | AWS 区域 | ✅ |
| `REQUIRE_API_KEY` | 启用 API Key 认证 | ✅ |
| `LOG_LEVEL` | 日志级别 (info/debug/warn) | ❌ |
| `ANTHROPIC_DEFAULT_MODEL` | 默认模型映射 | ❌ |
| `ANTHROPIC_DEFAULT_SONNET_MODEL` | Sonnet 模型映射 | ❌ |
| `ANTHROPIC_DEFAULT_HAIKU_MODEL` | Haiku 模型映射 | ❌ |
| `ANTHROPIC_DEFAULT_OPUS_MODEL` | Opus 模型映射 | ❌ |

### 模型映射示例

将所有请求转发到 Qwen 模型：

```
ANTHROPIC_DEFAULT_MODEL=qwen.qwen3-coder-480b-a35b-v1:0
```

---

## IAM 权限

部署脚本会自动创建两个 IAM 角色：

### 1. App Runner 访问角色

用于从 ECR 拉取镜像：

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Principal": {"Service": "build.apprunner.amazonaws.com"},
    "Action": "sts:AssumeRole"
  }]
}
```

附加策略：`AWSAppRunnerServicePolicyForECRAccess`

### 2. 实例角色

用于访问 Bedrock 和 DynamoDB：

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "bedrock:InvokeModel",
        "bedrock:InvokeModelWithResponseStream"
      ],
      "Resource": "*"
    },
    {
      "Effect": "Allow",
      "Action": [
        "dynamodb:GetItem",
        "dynamodb:PutItem",
        "dynamodb:UpdateItem",
        "dynamodb:Query",
        "dynamodb:Scan"
      ],
      "Resource": "arn:aws:dynamodb:*:*:table/anthropic-proxy-*"
    }
  ]
}
```

---

## 验证部署

### 获取服务 URL

```bash
aws apprunner list-services --region us-west-2 \
  --query "ServiceSummaryList[?ServiceName=='llm-api-converter'].ServiceUrl" \
  --output text
```

### 健康检查

```bash
curl https://<service-url>/health
```

### 测试 API

```bash
curl -X POST https://<service-url>/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-your-api-key" \
  -d '{
    "model": "claude-3-5-sonnet-20241022",
    "max_tokens": 100,
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

---

## 常见问题

### 1. exec format error

**原因**：二进制架构与容器架构不匹配。

**解决**：确保使用正确的交叉编译目标和 Docker 平台：
- AMD64: `--target x86_64-unknown-linux-gnu` + `--platform linux/amd64`
- ARM64: `--target aarch64-unknown-linux-gnu` + `--platform linux/arm64`

### 2. OpenSSL 编译错误

**原因**：交叉编译时缺少 OpenSSL。

**解决**：项目已配置使用 `rustls`，无需系统 OpenSSL。如仍有问题，确保 Cargo.toml 中的依赖使用 `rustls` feature。

### 3. 服务创建失败

**原因**：可能是权限不足或配置错误。

**解决**：
```bash
# 查看日志
aws logs get-log-events \
  --log-group-name "/aws/apprunner/llm-api-converter/<service-id>/application" \
  --log-stream-name <stream-name> \
  --region us-west-2

# 删除失败的服务并重试
aws apprunner delete-service --service-arn <service-arn> --region us-west-2
```

### 4. Bedrock 权限错误

**原因**：实例角色没有 Bedrock 权限，或模型未开通。

**解决**：
1. 确保实例角色有 `bedrock:InvokeModel` 权限
2. 在 AWS Bedrock 控制台开通对应模型访问权限

---

## 更新部署

推送新镜像后触发重新部署：

```bash
# 方式 1：使用脚本
./scripts/deploy-apprunner.sh -p amd64 -r us-west-2

# 方式 2：手动触发
aws apprunner start-deployment \
  --service-arn <service-arn> \
  --region us-west-2
```

---

## 删除服务

```bash
aws apprunner delete-service \
  --service-arn <service-arn> \
  --region us-west-2
```

清理 ECR 镜像：

```bash
aws ecr delete-repository \
  --repository-name llm-api-converter \
  --region us-west-2 \
  --force
```
