#!/bin/bash
# AWS App Runner Status Script
# Usage: ./scripts/apprunner-status.sh [-r region] [-n name]

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Defaults
REGION="${AWS_REGION:-us-west-2}"
SERVICE_NAME="llm-api-converter"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--region) REGION="$2"; shift 2 ;;
        -n|--name) SERVICE_NAME="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [-r region] [-n name]"
            echo "  -r, --region   AWS region (default: us-west-2)"
            echo "  -n, --name     Service name (default: llm-api-converter)"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  App Runner Status${NC}"
echo -e "${GREEN}========================================${NC}"

# Get service ARN
SERVICE_ARN=$(aws apprunner list-services --region ${REGION} \
    --query "ServiceSummaryList[?ServiceName=='${SERVICE_NAME}'].ServiceArn" \
    --output text 2>/dev/null)

if [[ -z "$SERVICE_ARN" ]]; then
    echo -e "${YELLOW}Service '${SERVICE_NAME}' not found in region ${REGION}${NC}"
    exit 1
fi

# Get service details
SERVICE_INFO=$(aws apprunner describe-service --service-arn ${SERVICE_ARN} --region ${REGION} 2>/dev/null)

SERVICE_URL=$(echo "$SERVICE_INFO" | jq -r '.Service.ServiceUrl')
SERVICE_STATUS=$(echo "$SERVICE_INFO" | jq -r '.Service.Status')
CREATED_AT=$(echo "$SERVICE_INFO" | jq -r '.Service.CreatedAt')
UPDATED_AT=$(echo "$SERVICE_INFO" | jq -r '.Service.UpdatedAt')

echo -e "Region:     ${CYAN}${REGION}${NC}"
echo -e "Service:    ${CYAN}${SERVICE_NAME}${NC}"
echo -e "Status:     ${CYAN}${SERVICE_STATUS}${NC}"
echo -e "URL:        ${CYAN}https://${SERVICE_URL}${NC}"
echo -e "Created:    ${CYAN}${CREATED_AT}${NC}"
echo -e "Updated:    ${CYAN}${UPDATED_AT}${NC}"
echo -e "${GREEN}========================================${NC}"

# Health check
echo
echo -e "${YELLOW}Health Check:${NC}"
HEALTH_RESPONSE=$(curl -s --max-time 5 "https://${SERVICE_URL}/health" 2>/dev/null)
if [[ -n "$HEALTH_RESPONSE" ]]; then
    echo -e "${GREEN}✓ Service is responding${NC}"
    echo "$HEALTH_RESPONSE" | jq . 2>/dev/null || echo "$HEALTH_RESPONSE"
else
    echo -e "${YELLOW}⚠ Service not responding (may still be deploying)${NC}"
fi

echo -e "${GREEN}========================================${NC}"
