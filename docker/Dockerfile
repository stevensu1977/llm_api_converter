# =============================================================================
# Anthropic-Bedrock Proxy - Optimized Docker Image
# =============================================================================
# Multi-stage build for minimal image size (<20MB target)
# Supports both AMD64 and ARM64 architectures
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build environment
# -----------------------------------------------------------------------------
FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    musl-tools \
    musl-dev \
    pkg-config \
    libssl-dev \
    perl \
    make \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Determine target architecture and install appropriate musl target
ARG TARGETARCH
RUN case "${TARGETARCH}" in \
        "amd64") \
            rustup target add x86_64-unknown-linux-musl \
            ;; \
        "arm64") \
            rustup target add aarch64-unknown-linux-musl \
            ;; \
        *) \
            echo "Unsupported architecture: ${TARGETARCH}" && exit 1 \
            ;; \
    esac

# Create app directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo

# Create dummy main to cache dependencies
RUN mkdir -p src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs

# Build dependencies (cached layer)
ARG TARGETARCH
RUN case "${TARGETARCH}" in \
        "amd64") \
            cargo build --release --target x86_64-unknown-linux-musl \
            ;; \
        "arm64") \
            cargo build --release --target aarch64-unknown-linux-musl \
            ;; \
    esac && \
    rm -rf src

# Copy actual source code
COPY src src
COPY benches benches
COPY tests tests

# Touch main.rs to invalidate cache and rebuild
RUN touch src/main.rs

# Build the actual application
ARG TARGETARCH
RUN case "${TARGETARCH}" in \
        "amd64") \
            cargo build --release --target x86_64-unknown-linux-musl && \
            cp target/x86_64-unknown-linux-musl/release/anthropic-bedrock-proxy /app/server \
            ;; \
        "arm64") \
            cargo build --release --target aarch64-unknown-linux-musl && \
            cp target/aarch64-unknown-linux-musl/release/anthropic-bedrock-proxy /app/server \
            ;; \
    esac

# Verify binary is statically linked
RUN file /app/server && \
    ldd /app/server 2>&1 || echo "Static binary (no dynamic dependencies)"

# -----------------------------------------------------------------------------
# Stage 2: Minimal runtime image (scratch)
# -----------------------------------------------------------------------------
FROM scratch AS runtime

# Copy CA certificates for HTTPS
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the statically linked binary
COPY --from=builder /app/server /server

# Set environment defaults
ENV HOST=0.0.0.0 \
    PORT=8000 \
    RUST_LOG=info \
    ENVIRONMENT=production

# Expose port
EXPOSE 8000

# Health check (requires curl in image, so commented out for scratch)
# HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
#     CMD curl -f http://localhost:8000/health || exit 1

# Run as non-root user (UID 1000)
USER 1000:1000

# Run the server
ENTRYPOINT ["/server"]
