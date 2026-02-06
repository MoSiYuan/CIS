# CIS (Cluster of Independent Systems) - DAG Executor
# Multi-stage build for production deployment

# ==================== Builder Stage ====================
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY cis-core/Cargo.toml cis-core/
COPY cis-node/Cargo.toml cis-node/
COPY cis-gui/Cargo.toml cis-gui/
COPY skills/dag-executor/Cargo.toml skills/dag-executor/

# Copy source code
COPY cis-core/src cis-core/src
COPY cis-node/src cis-node/src
COPY cis-gui/src cis-gui/src
COPY skills/dag-executor/src skills/dag-executor/src

# Build release binary
RUN cargo build --release -p cis-node

# ==================== Runtime Stage ====================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create data directory
RUN mkdir -p /app/data /app/config

# Copy binary from builder
COPY --from=builder /build/target/release/cis-node /usr/local/bin/cis-node
COPY --from=builder /build/target/release/cis-cli /usr/local/bin/cis-cli 2>/dev/null || true

# Copy default config
COPY config.example.toml /app/config/config.toml

# Create non-root user
RUN groupadd -r cis && useradd -r -g cis cis && \
    chown -R cis:cis /app

# Expose ports
# 7676: GLM API (HTTP)
# 8080: Matrix Federation (HTTP)
# 8448: Matrix Federation (HTTPS)
EXPOSE 7676 8080 8448

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:7676/health || exit 1

# Switch to non-root user
USER cis

# Default command
ENTRYPOINT ["cis-node"]
CMD ["--help"]
