#!/bin/bash
# =============================================================================
# Docker Build Script for Anthropic-Bedrock Proxy
# =============================================================================
# Usage: ./scripts/build-docker.sh [OPTIONS]
#
# Options:
#   -t, --tag TAG         Image tag (default: latest)
#   -p, --platform PLAT   Platform: amd64, arm64, or all (default: current)
#   -i, --image TYPE      Image type: minimal, alpine, ptc (default: minimal)
#   -r, --registry REG    Docker registry prefix
#   --push                Push to registry after build
#   --no-cache            Build without cache
#   -h, --help            Show this help message
#
# Examples:
#   ./scripts/build-docker.sh                           # Build minimal for current platform
#   ./scripts/build-docker.sh -t v1.0.0 -p all         # Build for all platforms
#   ./scripts/build-docker.sh -i ptc -p arm64          # Build PTC image for ARM64
#   ./scripts/build-docker.sh -r 123456.dkr.ecr.us-east-1.amazonaws.com --push
# =============================================================================

set -euo pipefail

# Default values
TAG="latest"
PLATFORM=""
IMAGE_TYPE="minimal"
REGISTRY=""
PUSH=false
NO_CACHE=""
IMAGE_NAME="anthropic-bedrock-proxy"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Help function
show_help() {
    head -30 "$0" | tail -28 | sed 's/^# //' | sed 's/^#//'
    exit 0
}

# Log functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--tag)
            TAG="$2"
            shift 2
            ;;
        -p|--platform)
            PLATFORM="$2"
            shift 2
            ;;
        -i|--image)
            IMAGE_TYPE="$2"
            shift 2
            ;;
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        --push)
            PUSH=true
            shift
            ;;
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        -h|--help)
            show_help
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            ;;
    esac
done

# Determine Dockerfile
case $IMAGE_TYPE in
    minimal)
        DOCKERFILE="docker/Dockerfile"
        ;;
    alpine)
        DOCKERFILE="docker/Dockerfile.alpine"
        ;;
    ptc)
        DOCKERFILE="docker/Dockerfile.ptc"
        ;;
    *)
        log_error "Unknown image type: $IMAGE_TYPE"
        exit 1
        ;;
esac

# Determine full image name
if [[ -n "$REGISTRY" ]]; then
    FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}"
else
    FULL_IMAGE_NAME="${IMAGE_NAME}"
fi

# Add image type suffix if not minimal
if [[ "$IMAGE_TYPE" != "minimal" ]]; then
    FULL_IMAGE_NAME="${FULL_IMAGE_NAME}-${IMAGE_TYPE}"
fi

log_info "Building ${FULL_IMAGE_NAME}:${TAG}"
log_info "Dockerfile: ${DOCKERFILE}"
log_info "Image type: ${IMAGE_TYPE}"

cd "$PROJECT_DIR"

# Build function
build_image() {
    local platform=$1
    local tag_suffix=$2

    local build_platform=""
    case $platform in
        amd64)
            build_platform="linux/amd64"
            ;;
        arm64)
            build_platform="linux/arm64"
            ;;
    esac

    local full_tag="${FULL_IMAGE_NAME}:${TAG}${tag_suffix}"

    log_info "Building for platform: ${platform}"
    log_info "Tag: ${full_tag}"

    docker build \
        --platform "${build_platform}" \
        --build-arg TARGETARCH="${platform}" \
        -f "${DOCKERFILE}" \
        -t "${full_tag}" \
        ${NO_CACHE} \
        .

    # Show image size
    local size=$(docker images --format "{{.Size}}" "${full_tag}")
    log_info "Image size: ${size}"

    if [[ "$PUSH" == true ]]; then
        log_info "Pushing ${full_tag}..."
        docker push "${full_tag}"
    fi
}

# Build based on platform selection
case $PLATFORM in
    amd64)
        build_image "amd64" ""
        ;;
    arm64)
        build_image "arm64" ""
        ;;
    all)
        # Build multi-platform manifest
        log_info "Building multi-platform image..."

        docker buildx build \
            --platform "linux/amd64,linux/arm64" \
            -f "${DOCKERFILE}" \
            -t "${FULL_IMAGE_NAME}:${TAG}" \
            ${NO_CACHE} \
            $(if [[ "$PUSH" == true ]]; then echo "--push"; else echo "--load 2>/dev/null || true"; fi) \
            .

        if [[ "$PUSH" != true ]]; then
            log_warn "Multi-platform builds require --push or manual platform selection"
            log_info "Building for current platform instead..."
            docker build \
                -f "${DOCKERFILE}" \
                -t "${FULL_IMAGE_NAME}:${TAG}" \
                ${NO_CACHE} \
                .
        fi
        ;;
    "")
        # Default: build for current platform
        log_info "Building for current platform..."
        docker build \
            -f "${DOCKERFILE}" \
            -t "${FULL_IMAGE_NAME}:${TAG}" \
            ${NO_CACHE} \
            .

        # Show image size
        local size=$(docker images --format "{{.Size}}" "${FULL_IMAGE_NAME}:${TAG}")
        log_info "Image size: ${size}"

        if [[ "$PUSH" == true ]]; then
            log_info "Pushing ${FULL_IMAGE_NAME}:${TAG}..."
            docker push "${FULL_IMAGE_NAME}:${TAG}"
        fi
        ;;
    *)
        log_error "Unknown platform: $PLATFORM"
        exit 1
        ;;
esac

log_info "Build complete!"
echo ""
echo "To run the container:"
echo "  docker run -p 8000:8000 --env-file .env ${FULL_IMAGE_NAME}:${TAG}"
echo ""
echo "To run with PTC (Docker socket):"
echo "  docker run -p 8000:8000 -v /var/run/docker.sock:/var/run/docker.sock --env-file .env ${FULL_IMAGE_NAME}:${TAG}"
