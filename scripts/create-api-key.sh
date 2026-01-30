#!/bin/bash
set -e

# Script to create API keys in DynamoDB for the Anthropic Proxy (Rust version)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
TABLE_NAME="anthropic-proxy-api-keys"
ENDPOINT_URL="${DYNAMODB_ENDPOINT_URL:-}"
REGION="${AWS_REGION:-us-east-1}"
USER_ID=""
KEY_NAME=""
RATE_LIMIT="100"
SERVICE_TIER="default"
MONTHLY_BUDGET=""

# Usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Create an API key for the Anthropic Proxy service (Rust version)

OPTIONS:
    -u, --user-id ID         User ID (required)
    -n, --name NAME          Key name/description (required)
    -l, --rate-limit LIMIT   Rate limit requests per minute [default: 100]
    -t, --service-tier TIER  Service tier [default: default]
                             Options: default, flex, priority, reserved
    -b, --budget AMOUNT      Monthly budget in USD (optional)
    -T, --table TABLE        DynamoDB table name [default: anthropic-proxy-api-keys]
    -e, --endpoint URL       DynamoDB endpoint URL (for local dev)
    -r, --region REGION      AWS region [default: us-east-1]
    -h, --help               Show this help message

EXAMPLES:
    # Create a key for local development (DynamoDB Local on port 8002)
    ./scripts/create-api-key.sh -u dev-user -n "Development Key" -e http://localhost:8002

    # Create a key with flex tier
    ./scripts/create-api-key.sh -u batch-user -n "Batch Processing Key" -t flex

    # Create a key with monthly budget
    ./scripts/create-api-key.sh -u user@example.com -n "API Key" -b 100.00

    # Use environment variable for endpoint
    DYNAMODB_ENDPOINT_URL=http://localhost:8002 ./scripts/create-api-key.sh -u dev -n "Dev Key"

EOF
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -u|--user-id)
            USER_ID="$2"
            shift 2
            ;;
        -n|--name)
            KEY_NAME="$2"
            shift 2
            ;;
        -l|--rate-limit)
            RATE_LIMIT="$2"
            shift 2
            ;;
        -t|--service-tier)
            SERVICE_TIER="$2"
            shift 2
            ;;
        -b|--budget)
            MONTHLY_BUDGET="$2"
            shift 2
            ;;
        -T|--table)
            TABLE_NAME="$2"
            shift 2
            ;;
        -e|--endpoint)
            ENDPOINT_URL="$2"
            shift 2
            ;;
        -r|--region)
            REGION="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            ;;
    esac
done

# Validate inputs
if [[ -z "$USER_ID" ]]; then
    echo -e "${RED}Error: User ID is required${NC}"
    usage
fi

if [[ -z "$KEY_NAME" ]]; then
    echo -e "${RED}Error: Key name is required${NC}"
    usage
fi

if [[ ! "$SERVICE_TIER" =~ ^(default|flex|priority|reserved)$ ]]; then
    echo -e "${RED}Error: Service tier must be 'default', 'flex', 'priority', or 'reserved'${NC}"
    exit 1
fi

# Generate API key
API_KEY="sk-$(openssl rand -hex 16)"

# Build AWS CLI command
AWS_CMD="aws dynamodb put-item --region $REGION --table-name $TABLE_NAME"

if [[ -n "$ENDPOINT_URL" ]]; then
    AWS_CMD="$AWS_CMD --endpoint-url $ENDPOINT_URL"
    echo -e "${YELLOW}Using local DynamoDB: ${ENDPOINT_URL}${NC}"
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Creating API Key${NC}"
echo -e "${GREEN}========================================${NC}"
echo -e "Table:        ${YELLOW}${TABLE_NAME}${NC}"
echo -e "User ID:      ${YELLOW}${USER_ID}${NC}"
echo -e "Key Name:     ${YELLOW}${KEY_NAME}${NC}"
echo -e "Rate Limit:   ${YELLOW}${RATE_LIMIT} req/min${NC}"
echo -e "Service Tier: ${YELLOW}${SERVICE_TIER}${NC}"
[ -n "$MONTHLY_BUDGET" ] && echo -e "Monthly Budget: ${YELLOW}\$${MONTHLY_BUDGET}${NC}"
echo -e "${GREEN}========================================${NC}"
echo

# Get current timestamp
TIMESTAMP=$(date +%s)

# Build item JSON
ITEM="{
  \"api_key\": {\"S\": \"$API_KEY\"},
  \"user_id\": {\"S\": \"$USER_ID\"},
  \"name\": {\"S\": \"$KEY_NAME\"},
  \"is_active\": {\"BOOL\": true},
  \"created_at\": {\"N\": \"$TIMESTAMP\"},
  \"rate_limit\": {\"N\": \"$RATE_LIMIT\"},
  \"service_tier\": {\"S\": \"$SERVICE_TIER\"},
  \"budget_used\": {\"N\": \"0\"},
  \"budget_used_mtd\": {\"N\": \"0\"}
}"

# Add monthly budget if specified
if [ -n "$MONTHLY_BUDGET" ]; then
    ITEM=$(echo "$ITEM" | jq --arg budget "$MONTHLY_BUDGET" \
        '.monthly_budget = {"N": $budget}')
fi

# Create item in DynamoDB
echo -e "${YELLOW}Creating API key in DynamoDB...${NC}"

if $AWS_CMD --item "$ITEM" 2>/dev/null; then
    echo -e "${GREEN}API key created successfully!${NC}"
    echo
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}API Key Details${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo -e "${YELLOW}API Key:${NC}      $API_KEY"
    echo -e "${YELLOW}Service Tier:${NC} $SERVICE_TIER"
    echo
    echo -e "${YELLOW}IMPORTANT: Save this API key securely!${NC}"
    echo -e "${YELLOW}It will not be shown again.${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo
    echo -e "${YELLOW}Use with:${NC}"
    echo -e "  export ANTHROPIC_API_KEY=\"$API_KEY\""
    echo -e "  export ANTHROPIC_BASE_URL=\"http://localhost:8000\""
else
    echo -e "${RED}Error: Failed to create API key${NC}"
    echo -e "${RED}Make sure DynamoDB is running and the table exists.${NC}"
    echo -e "${YELLOW}To start DynamoDB Local:${NC}"
    echo -e "  docker-compose up -d dynamodb-local"
    echo -e "${YELLOW}To create tables:${NC}"
    echo -e "  docker-compose up dynamodb-init"
    exit 1
fi
