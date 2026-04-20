# Multi-stage Dockerfile for Anode Object Storage
# Optimized for layer caching, build speed, and minimal image size

# ============================================================================
# Chef Planner Stage - Analyze dependencies
# ============================================================================
FROM rust:1.95-bookworm AS chef

RUN cargo install cargo-chef --locked
WORKDIR /build

# ============================================================================
# Dependency Planning Stage
# ============================================================================
FROM chef AS planner

# Copy all necessary files for cargo-chef to analyze
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY .cargo .cargo
COPY crates crates

# Generate recipe.json with dependency information
RUN cargo chef prepare --recipe-path recipe.json

# ============================================================================
# Builder Stage - Compile dependencies and application
# ============================================================================
FROM chef AS builder

# Install system dependencies required for compilation
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    libssl-dev \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency recipe from planner
COPY --from=planner /build/recipe.json recipe.json

# Build dependencies only (this layer will be cached unless dependencies change)
RUN cargo chef cook --release --recipe-path recipe.json

# Copy source code and configuration
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY .cargo .cargo
COPY crates crates

# Build the actual release binary with optimizations
RUN cargo build --release --bin anode

# Strip debug symbols to reduce binary size
RUN strip target/release/anode

# Verify binary is functional
RUN ldd target/release/anode && \
    target/release/anode --version

# ============================================================================
# Runtime Stage - Minimal distroless image
# ============================================================================
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy SSL certificates for HTTPS support
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary from builder
COPY --from=builder /build/target/release/anode /usr/local/bin/anode

# Create necessary directories with proper permissions
# Note: distroless runs as nonroot user (uid=65532) by default
USER nonroot:nonroot

# Expose ports
# 8080 - S3 HTTP API
# 8081 - Admin API
# 9090 - gRPC inter-node communication
EXPOSE 8080 8081 9090

# Define volume mount points
VOLUME ["/data", "/config"]

# Set environment variables
ENV ANODE_DATA_DIR=/data \
    ANODE_CONFIG_DIR=/config \
    ANODE_LOG_LEVEL=info \
    RUST_BACKTRACE=1

# Health check
# Note: Distroless images lack shell and utilities (curl/wget/sh) required for HEALTHCHECK
# For Docker standalone deployments with health checks, use the runtime-debian stage instead:
#   docker build --target runtime-debian -t anode:debian .
# For Kubernetes deployments, use liveness/readiness probes with the /live and /ready endpoints:
#   livenessProbe:
#     httpGet:
#       path: /live
#       port: 8081
#   readinessProbe:
#     httpGet:
#       path: /ready
#       port: 8081
HEALTHCHECK NONE

# Entrypoint - directly run anode (distroless has no shell)
ENTRYPOINT ["/usr/local/bin/anode"]
CMD ["start", "--config", "/config/anode.toml"]

# ============================================================================
# Alternative Runtime Stage - Debian slim (for debugging)
# ============================================================================
FROM debian:bookworm-slim AS runtime-debian

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user and group
RUN groupadd -g 1000 anode && \
    useradd -m -u 1000 -g anode -s /bin/sh anode

# Create necessary directories with proper permissions
RUN mkdir -p /data /config /var/log/anode && \
    chown -R anode:anode /data /config /var/log/anode

# Copy binary from builder
COPY --from=builder /build/target/release/anode /usr/local/bin/anode

# Copy entrypoint script if needed
COPY deploy/docker/scripts/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Switch to non-root user
USER anode

# Expose ports
EXPOSE 8080 8081 9090

# Define volume mount points
VOLUME ["/data", "/config"]

# Health check
# Uses the /live endpoint which returns 200 OK when the service is alive
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/live || exit 1

# Set environment variables
ENV ANODE_DATA_DIR=/data \
    ANODE_CONFIG_DIR=/config \
    ANODE_LOG_LEVEL=info \
    RUST_BACKTRACE=1

# Entrypoint with proper signal handling
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["anode", "start", "--config", "/config/anode.toml"]
