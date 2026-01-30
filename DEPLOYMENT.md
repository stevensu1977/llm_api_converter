# Deployment Guide

This guide covers deploying the Rust-based Anthropic-Bedrock Proxy.

## Quick Start

### Local Development

```bash
# 1. Copy and configure environment
cp .env.example .env
# Edit .env with your AWS credentials

# 2. Run directly with Cargo
cargo run --release

# Or with Docker Compose
docker-compose up -d
```

### Docker Build

```bash
# Build for current platform
./scripts/build-docker.sh

# Build for specific platform
./scripts/build-docker.sh -p arm64 -t v1.0.0

# Build with Alpine (includes shell for debugging)
./scripts/build-docker.sh -i alpine

# Build with PTC support
./scripts/build-docker.sh -i ptc

# Build and push to ECR
./scripts/build-docker.sh -r 123456789.dkr.ecr.us-east-1.amazonaws.com --push
```

## Docker Images

| Image Type | Size Target | Use Case |
|------------|-------------|----------|
| `minimal` (scratch) | <20MB | Production, Fargate |
| `alpine` | ~25MB | Debugging, development |
| `ptc` | ~50MB | PTC-enabled deployments (EC2) |

## AWS ECS Deployment

### Prerequisites

1. AWS CLI configured with appropriate credentials
2. CDK CLI installed (`npm install -g aws-cdk`)
3. Docker installed and running

### Option 1: Using Existing CDK (Recommended)

Modify the parent project's CDK to use the Rust Dockerfile:

```typescript
// In cdk/lib/ecs-stack.ts, change:
image: ecs.ContainerImage.fromAsset(path.join(__dirname, '../../'), {
  file: 'Dockerfile',
  ...
})

// To:
image: ecs.ContainerImage.fromAsset(path.join(__dirname, '../../rust-project'), {
  file: 'docker/Dockerfile.alpine',  // or Dockerfile for minimal
  ...
})
```

Then deploy:

```bash
cd ../cdk

# Fargate deployment (standard)
./scripts/deploy.sh -e dev -p arm64

# EC2 deployment (PTC support)
./scripts/deploy.sh -e dev -p arm64 -l ec2
```

### Option 2: Manual ECR Deployment

```bash
# 1. Create ECR repository
aws ecr create-repository --repository-name anthropic-bedrock-proxy-rust

# 2. Login to ECR
aws ecr get-login-password --region us-east-1 | docker login --username AWS --password-stdin 123456789.dkr.ecr.us-east-1.amazonaws.com

# 3. Build and push
./scripts/build-docker.sh \
  -r 123456789.dkr.ecr.us-east-1.amazonaws.com \
  -t latest \
  -p arm64 \
  --push

# 4. Update ECS service to use new image
aws ecs update-service \
  --cluster anthropic-proxy-dev \
  --service anthropic-proxy-dev-service \
  --force-new-deployment
```

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `AWS_REGION` | AWS region | `us-east-1` |
| `MASTER_API_KEY` | Admin API key | `sk-xxx` |

### DynamoDB Tables

| Variable | Default |
|----------|---------|
| `DYNAMODB_API_KEYS_TABLE` | `anthropic-proxy-api-keys` |
| `DYNAMODB_USAGE_TABLE` | `anthropic-proxy-usage` |
| `DYNAMODB_USAGE_STATS_TABLE` | `anthropic-proxy-usage-stats` |
| `DYNAMODB_MODEL_MAPPING_TABLE` | `anthropic-proxy-model-mapping` |
| `DYNAMODB_MODEL_PRICING_TABLE` | `anthropic-proxy-model-pricing` |

### Feature Flags

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_TOOL_USE` | `true` | Enable tool calling |
| `ENABLE_PTC` | `false` | Enable Programmatic Tool Calling |
| `ENABLE_EXTENDED_THINKING` | `true` | Enable thinking blocks |
| `ENABLE_DOCUMENT_SUPPORT` | `true` | Enable document content |

### PTC Settings (when ENABLE_PTC=true)

| Variable | Default | Description |
|----------|---------|-------------|
| `PTC_SANDBOX_IMAGE` | `python:3.11-slim` | Docker image for sandbox |
| `PTC_SESSION_TIMEOUT` | `270` | Session timeout (seconds) |
| `PTC_EXECUTION_TIMEOUT` | `60` | Code execution timeout |
| `PTC_MEMORY_LIMIT` | `256m` | Container memory limit |
| `PTC_NETWORK_DISABLED` | `true` | Disable network in sandbox |

## Architecture Considerations

### Fargate vs EC2

| Feature | Fargate | EC2 |
|---------|---------|-----|
| PTC Support | ❌ No | ✅ Yes |
| Management | Zero | Some |
| Scaling | Fast (seconds) | Slower (minutes) |
| Cost | Pay-per-use | Instance-based |
| Docker Access | ❌ No | ✅ Yes |

**Recommendation**: Use Fargate unless PTC is required.

### Multi-AZ Deployment

For production:
- Deploy across 2+ availability zones
- Use ALB for load balancing
- Enable sticky sessions for PTC (300s duration)

### Resource Sizing

| Environment | CPU | Memory | Instances |
|-------------|-----|--------|-----------|
| Development | 256 | 512MB | 1 |
| Production | 512-1024 | 1-2GB | 2-4 |

The Rust version uses ~5x less memory than Python equivalent.

## Health Checks

| Endpoint | Purpose | Path |
|----------|---------|------|
| Health | General status | `/health` |
| Ready | Kubernetes readiness | `/ready` |
| Liveness | Kubernetes liveness | `/liveness` |
| PTC Health | PTC/Docker status | `/health/ptc` |

## Monitoring

### Docker Compose (Local)

```bash
# Start with monitoring stack
docker-compose --profile monitoring up -d

# Access:
# - Prometheus: http://localhost:9090
# - Grafana: http://localhost:3000 (admin/admin)
```

### AWS CloudWatch

The CDK deployment automatically configures:
- CloudWatch Logs (`/ecs/anthropic-proxy-{env}`)
- Container Insights (if enabled)
- ALB access logs

## Troubleshooting

### Image Size Too Large

Check `.dockerignore` includes:
- `target/`
- `.git/`
- `tests/`

### PTC Not Working

1. Verify Docker socket is mounted:
   ```bash
   docker run -v /var/run/docker.sock:/var/run/docker.sock ...
   ```

2. Check PTC health:
   ```bash
   curl http://localhost:8000/health/ptc
   ```

3. For ECS EC2, ensure:
   - Launch type is EC2 (not Fargate)
   - Docker socket volume is configured
   - Container runs as root or docker group

### Build Failures

For musl static linking issues:
```bash
# Ensure musl-tools are installed
apt-get install musl-tools musl-dev

# Add Rust target
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl
```

## Security Considerations

1. **Never commit `.env` files** with secrets
2. Use **AWS Secrets Manager** for production API keys
3. Keep **Docker images updated** (base images have CVEs)
4. Enable **VPC endpoints** for private Bedrock/DynamoDB access
5. Use **non-root user** (UID 1000) in containers
