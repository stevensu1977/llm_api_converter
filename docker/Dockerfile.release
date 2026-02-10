# =============================================================================
# LLM API Converter - Dockerfile using pre-built GitHub Release binaries
# =============================================================================
# Downloads pre-compiled binaries from GitHub Release for fast deployment
# Supports AMD64 architecture (App Runner default)
# =============================================================================

FROM debian:bookworm-slim

# Install minimal dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set version - update this when releasing new versions
ARG VERSION=v0.1.0
ARG GITHUB_REPO=stevensu1977/llm_api_converter

# Download and extract pre-built binary from GitHub Release
RUN curl -fsSL "https://github.com/${GITHUB_REPO}/releases/download/${VERSION}/llm-api-converter-linux-amd64.tar.gz" \
    | tar -xzf - -C /usr/local/bin/ \
    && chmod +x /usr/local/bin/llm-api-converter \
    && chmod +x /usr/local/bin/create_api_key \
    && chmod +x /usr/local/bin/setup_tables

# Create non-root user
RUN useradd -m -u 1000 appuser
USER appuser

# Set environment defaults
ENV HOST=0.0.0.0 \
    PORT=8000 \
    RUST_LOG=info \
    ENVIRONMENT=production

# Expose port
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${PORT}/health || exit 1

# Run the server
ENTRYPOINT ["llm-api-converter"]
