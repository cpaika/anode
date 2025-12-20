# Multi-stage Dockerfile for Anode Object Storage
# Optimized for layer caching and minimal image size

# ============================================================================
# Builder Stage
# ============================================================================
FROM rust:1.75-bookworm AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app user for building
RUN useradd -m -u 1000 -U -s /bin/sh anode

# Set working directory
WORKDIR /build

# Copy dependency manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY .cargo .cargo

# Create dummy source files to cache dependencies
RUN mkdir -p crates/anode/src \
    crates/anode-protocol/src \
    crates/anode-storage/src \
    crates/anode-raft/src \
    crates/anode-s3/src \
    crates/anode-parquet/src \
    crates/anode-client/src && \
    echo "fn main() {}" > crates/anode/src/main.rs && \
    for crate in anode-protocol anode-storage anode-raft anode-s3 anode-parquet anode-client; do \
        echo "pub fn dummy() {}" > crates/${crate}/src/lib.rs; \
    done

# Copy individual Cargo.toml files
COPY crates/anode/Cargo.toml crates/anode/
COPY crates/anode-protocol/Cargo.toml crates/anode-protocol/
COPY crates/anode-storage/Cargo.toml crates/anode-storage/
COPY crates/anode-raft/Cargo.toml crates/anode-raft/
COPY crates/anode-s3/Cargo.toml crates/anode-s3/
COPY crates/anode-parquet/Cargo.toml crates/anode-parquet/
COPY crates/anode-client/Cargo.toml crates/anode-client/

# Build dependencies only (this layer will be cached)
RUN cargo build --release --workspace && \
    rm -rf target/release/.fingerprint/anode* target/release/deps/anode* && \
    rm -rf crates/*/src

# Copy protobuf definitions
COPY proto proto

# Copy actual source code
COPY crates crates

# Touch main.rs to force rebuild of the actual binary
RUN touch crates/anode/src/main.rs

# Build the actual release binary
RUN cargo build --release --bin anode && \
    strip target/release/anode

# Verify binary
RUN ldd target/release/anode

# ============================================================================
# Runtime Stage
# ============================================================================
FROM debian:bookworm-slim

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

# Copy entrypoint script
COPY deploy/docker/scripts/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Switch to non-root user
USER anode

# Expose ports
# 8080 - S3 HTTP API
# 8081 - Admin API
# 9090 - gRPC inter-node communication
EXPOSE 8080 8081 9090

# Define volume mount points
VOLUME ["/data", "/config"]

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

# Set environment variables
ENV ANODE_DATA_DIR=/data \
    ANODE_CONFIG_DIR=/config \
    ANODE_LOG_LEVEL=info \
    RUST_BACKTRACE=1

# Entrypoint with proper signal handling
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["anode", "start", "--config", "/config/anode.toml"]
