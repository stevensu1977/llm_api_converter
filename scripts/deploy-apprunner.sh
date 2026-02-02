#!/bin/bash
set -e

# AWS App Runner Deployment Script
# Usage: ./scripts/deploy-apprunner.sh [options]
#
# Options:
#   -r, --region        AWS region (default: us-east-1)
#   -n, --name          Service name (default: llm-api-converter)
#   -p, --platform      Platform: arm64 or amd64 (default: arm64)
#   --create            Create new service (default: update existing)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Defaults
REGION="${AWS_REGION:-us-east-1}"
SERVICE_NAME="llm-api-converter"
PLATFORM="arm64"
CREATE_NEW=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--region) REGION="$2"; shift 2 ;;
        -n|--name) SERVICE_NAME="$2"; shift 2 ;;
        -p|--platform) PLATFORM="$2"; shift 2 ;;
        --create) CREATE_NEW=true; shift ;;
        -h|--help)
            echo "Usage: $0 [-r region] [-n name] [-p platform] [--create]"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Get AWS account ID
ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
ECR_REPO="${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com/${SERVICE_NAME}"

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  AWS App Runner Deployment${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "Region:    ${YELLOW}${REGION}${NC}"
echo -e "Service:   ${YELLOW}${SERVICE_NAME}${NC}"
echo -e "Platform:  ${YELLOW}${PLATFORM}${NC}"
echo -e "ECR Repo:  ${YELLOW}${ECR_REPO}${NC}"
echo -e "${GREEN}========================================${NC}"
echo

# Step 1: Create ECR repository if not exists
echo -e "${YELLOW}[1/4] Checking ECR repository...${NC}"
aws ecr describe-repositories --repository-names ${SERVICE_NAME} --region ${REGION} 2>/dev/null || \
    aws ecr create-repository --repository-name ${SERVICE_NAME} --region ${REGION}

# Step 2: Build binary for Linux and create Docker image
echo -e "${YELLOW}[2/4] Building binary for Linux...${NC}"

if [[ "$PLATFORM" == "arm64" ]]; then
    RUST_TARGET="aarch64-unknown-linux-gnu"
    DOCKER_PLATFORM="linux/arm64"
else
    RUST_TARGET="x86_64-unknown-linux-gnu"
    DOCKER_PLATFORM="linux/amd64"
fi

# Check if cross-compilation is needed (macOS -> Linux)
if [[ "$(uname)" == "Darwin" ]]; then
    echo -e "${YELLOW}Cross-compiling for Linux (${RUST_TARGET}) using cargo-zigbuild...${NC}"

    # Check if cargo-zigbuild is installed
    if ! command -v cargo-zigbuild &> /dev/null; then
        echo -e "${RED}Error: cargo-zigbuild not found. Install with:${NC}"
        echo "  brew install zig"
        echo "  cargo install cargo-zigbuild"
        exit 1
    fi

    cargo zigbuild --release --target ${RUST_TARGET} --bin llm-api-converter
else
    # Native Linux build
    cargo build --release --target ${RUST_TARGET} --bin llm-api-converter
fi

# Build lightweight Docker image with pre-built binary
echo -e "${YELLOW}Building Docker image...${NC}"
BINARY_PATH="target/${RUST_TARGET}/release/llm-api-converter"
docker build --platform ${DOCKER_PLATFORM} \
    --build-arg BINARY_PATH=${BINARY_PATH} \
    -f docker/Dockerfile.prebuilt \
    -t ${SERVICE_NAME}:latest .

# Step 3: Push to ECR
echo -e "${YELLOW}[3/4] Pushing to ECR...${NC}"
aws ecr get-login-password --region ${REGION} | docker login --username AWS --password-stdin ${ACCOUNT_ID}.dkr.ecr.${REGION}.amazonaws.com
docker tag ${SERVICE_NAME}:latest ${ECR_REPO}:latest
docker push ${ECR_REPO}:latest

# Step 4: Create/Update App Runner service
echo -e "${YELLOW}[4/4] Deploying to App Runner...${NC}"

# Create IAM role for App Runner to access ECR and Bedrock
ROLE_NAME="${SERVICE_NAME}-apprunner-role"
INSTANCE_ROLE_NAME="${SERVICE_NAME}-instance-role"

# Check if access role exists, create if not
if ! aws iam get-role --role-name ${ROLE_NAME} 2>/dev/null; then
    echo -e "${YELLOW}Creating App Runner access role...${NC}"
    aws iam create-role --role-name ${ROLE_NAME} --assume-role-policy-document '{
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Service": "build.apprunner.amazonaws.com"},
            "Action": "sts:AssumeRole"
        }]
    }'
    aws iam attach-role-policy --role-name ${ROLE_NAME} \
        --policy-arn arn:aws:iam::aws:policy/service-role/AWSAppRunnerServicePolicyForECRAccess
    sleep 10  # Wait for role propagation
fi

# Check if instance role exists, create if not
if ! aws iam get-role --role-name ${INSTANCE_ROLE_NAME} 2>/dev/null; then
    echo -e "${YELLOW}Creating App Runner instance role...${NC}"
    aws iam create-role --role-name ${INSTANCE_ROLE_NAME} --assume-role-policy-document '{
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Service": "tasks.apprunner.amazonaws.com"},
            "Action": "sts:AssumeRole"
        }]
    }'

    # Add Bedrock and DynamoDB permissions
    aws iam put-role-policy --role-name ${INSTANCE_ROLE_NAME} --policy-name bedrock-dynamodb-access --policy-document '{
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
    }'
    sleep 10  # Wait for role propagation
fi

ACCESS_ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/${ROLE_NAME}"
INSTANCE_ROLE_ARN="arn:aws:iam::${ACCOUNT_ID}:role/${INSTANCE_ROLE_NAME}"

# Check if service exists
SERVICE_EXISTS=$(aws apprunner list-services --region ${REGION} --query "ServiceSummaryList[?ServiceName=='${SERVICE_NAME}'].ServiceArn" --output text)

if [[ -z "$SERVICE_EXISTS" ]] || [[ "$CREATE_NEW" == true ]]; then
    echo -e "${YELLOW}Creating new App Runner service...${NC}"

    aws apprunner create-service --region ${REGION} --service-name ${SERVICE_NAME} \
        --source-configuration "{
            \"AuthenticationConfiguration\": {
                \"AccessRoleArn\": \"${ACCESS_ROLE_ARN}\"
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
            \"InstanceRoleArn\": \"${INSTANCE_ROLE_ARN}\"
        }" \
        --health-check-configuration "{
            \"Protocol\": \"HTTP\",
            \"Path\": \"/health\",
            \"Interval\": 10,
            \"Timeout\": 5,
            \"HealthyThreshold\": 1,
            \"UnhealthyThreshold\": 3
        }"
else
    echo -e "${YELLOW}Updating existing App Runner service...${NC}"

    # Trigger new deployment
    aws apprunner start-deployment --region ${REGION} --service-arn ${SERVICE_EXISTS}
fi

echo
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Deployment initiated!${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "${YELLOW}Check status:${NC}"
echo -e "  aws apprunner list-services --region ${REGION}"
echo
echo -e "${YELLOW}Get service URL:${NC}"
echo -e "  aws apprunner describe-service --service-arn <SERVICE_ARN> --query 'Service.ServiceUrl'"
echo -e "${GREEN}========================================${NC}"
