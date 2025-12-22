# Standalone Deployment Guide

This guide walks you through deploying Anode as a standalone server on a single machine. This deployment mode is ideal for development, testing, small-scale production workloads, or as a starting point before expanding to a multi-node cluster.

## Table of Contents

- [System Requirements](#system-requirements)
- [Installation Methods](#installation-methods)
  - [Building from Source](#building-from-source)
  - [Using Docker](#using-docker)
  - [Using Pre-built Binaries](#using-pre-built-binaries)
- [Configuration](#configuration)
- [Starting the Server](#starting-the-server)
- [Verifying Installation](#verifying-installation)
- [Systemd Service Setup](#systemd-service-setup)
- [Common Configuration Scenarios](#common-configuration-scenarios)
- [Upgrading](#upgrading)
- [Troubleshooting](#troubleshooting)

## System Requirements

### Minimum Requirements

For development or testing:

- **OS**: Linux (Ubuntu 20.04+, Debian 11+, RHEL 8+, or similar)
- **CPU**: 2 cores
- **RAM**: 4GB
- **Storage**: 20GB available disk space
- **Network**: Internet connectivity for installation

### Recommended Production Requirements

For production standalone deployments:

- **OS**: Linux with kernel 5.4+ (Ubuntu 22.04 LTS or Debian 12 recommended)
- **CPU**: 8+ cores (x86_64 architecture)
- **RAM**: 16GB+ (more for larger metadata caches)
- **Storage**:
  - 100GB+ SSD/NVMe for data storage
  - Separate disk for OS recommended
  - XFS, ext4, or Btrfs filesystem recommended
- **Network**:
  - 1Gbps+ network interface
  - Fixed IP address or stable hostname
  - Firewall allowing ports 8080 (S3), 8081 (admin), 9090 (gRPC)

### Supported Operating Systems

Anode has been tested on:

- Ubuntu 20.04, 22.04, 24.04 LTS
- Debian 11 (Bullseye), 12 (Bookworm)
- RHEL/Rocky Linux/AlmaLinux 8+
- Fedora 38+
- macOS 12+ (development only, not recommended for production)

## Installation Methods

### Building from Source

Building from source gives you the most flexibility and ensures you have the latest features.

#### 1. Install System Dependencies

**Ubuntu/Debian:**
```bash
# Update package list
sudo apt-get update

# Install build essentials
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    libprotobuf-dev \
    curl \
    git

# Verify protobuf installation
protoc --version  # Should be 3.12.0 or newer
```

**RHEL/Rocky Linux/AlmaLinux:**
```bash
# Install EPEL repository (if not already installed)
sudo dnf install -y epel-release

# Install build dependencies
sudo dnf install -y \
    gcc \
    gcc-c++ \
    make \
    pkg-config \
    openssl-devel \
    protobuf-compiler \
    protobuf-devel \
    curl \
    git
```

**Fedora:**
```bash
sudo dnf install -y \
    gcc \
    gcc-c++ \
    make \
    pkg-config \
    openssl-devel \
    protobuf-compiler \
    protobuf-devel \
    curl \
    git
```

**macOS:**
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install protobuf pkg-config openssl
```

#### 2. Install Rust

Anode requires Rust 1.75 or newer:

```bash
# Install rustup (Rust installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow prompts (default installation is recommended)
# Source the cargo environment
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.75.0 or newer
cargo --version
```

#### 3. Clone and Build Anode

```bash
# Clone the repository
git clone https://github.com/christopherpaika/anode.git
cd anode

# Build release binary (this will take 5-15 minutes)
cargo build --release

# The binary will be located at: target/release/anode
# Verify the build
./target/release/anode --version
```

#### 4. Install the Binary

```bash
# Install to system path
sudo cp target/release/anode /usr/local/bin/anode
sudo chmod +x /usr/local/bin/anode

# Verify installation
anode --version
```

### Using Docker

Docker provides a quick way to run Anode without building from source.

#### 1. Install Docker

**Ubuntu/Debian:**
```bash
# Install Docker using official script
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Add your user to docker group (optional, for non-root access)
sudo usermod -aG docker $USER
# Log out and back in for group changes to take effect
```

**Other platforms:** See [Docker installation docs](https://docs.docker.com/engine/install/)

#### 2. Pull Anode Image

```bash
# Pull latest image
docker pull ghcr.io/christopherpaika/anode:latest

# Or pull specific version
docker pull ghcr.io/christopherpaika/anode:v0.1.0
```

#### 3. Run with Docker

```bash
# Create data directory
mkdir -p /var/lib/anode/data
mkdir -p /etc/anode

# Copy example configuration
curl -o /etc/anode/config.toml \
    https://raw.githubusercontent.com/christopherpaika/anode/master/crates/anode/config/anode.example.toml

# Run standalone node
docker run -d \
    --name anode \
    -p 8080:8080 \
    -p 8081:8081 \
    -p 9090:9090 \
    -v /var/lib/anode/data:/data \
    -v /etc/anode:/config \
    -e RUST_LOG=info \
    ghcr.io/christopherpaika/anode:latest \
    --config /config/config.toml \
    --node-id 1 \
    --bootstrap
```

### Using Pre-built Binaries

Pre-built binaries are available for Linux x86_64:

```bash
# Download latest release (replace VERSION with actual version)
VERSION="v0.1.0"
curl -LO "https://github.com/christopherpaika/anode/releases/download/${VERSION}/anode-${VERSION}-linux-x86_64.tar.gz"

# Verify checksum (optional but recommended)
curl -LO "https://github.com/christopherpaika/anode/releases/download/${VERSION}/anode-${VERSION}-linux-x86_64.tar.gz.sha256"
sha256sum -c anode-${VERSION}-linux-x86_64.tar.gz.sha256

# Extract binary
tar -xzf anode-${VERSION}-linux-x86_64.tar.gz

# Install to system path
sudo mv anode /usr/local/bin/anode
sudo chmod +x /usr/local/bin/anode

# Verify installation
anode --version
```

## Configuration

### Create Configuration File

#### 1. Create Configuration Directory

```bash
sudo mkdir -p /etc/anode
sudo mkdir -p /var/lib/anode/data
```

#### 2. Create Configuration File

Create `/etc/anode/config.toml`:

```toml
[node]
# Unique node identifier (must be > 0)
id = 1

# Human-readable node name
name = "anode-standalone"

# S3 API bind address
s3_addr = "0.0.0.0:8080"

# gRPC API bind address (internal cluster communication)
grpc_addr = "0.0.0.0:9090"

# Admin/metrics API bind address
admin_addr = "0.0.0.0:8081"

[cluster]
# Cluster name
name = "standalone-cluster"

# For standalone, leave initial_members empty
initial_members = []

# Number of replicas (use 1 for standalone)
replication_factor = 1

# Placement groups (default is fine for standalone)
placement_groups = 128

[storage]
# Root directory for data storage
data_dir = "/var/lib/anode/data"

# Chunk size for object splitting
chunk_size = "4MB"

# Disable sync writes for better performance (less durable)
sync_writes = false

# Metadata cache size (100MB)
metadata_cache_size = 104857600

# Number of I/O worker threads (defaults to CPU cores)
io_threads = 8

[s3]
# Maximum request body size (5GB)
max_body_size = 5368709120

# Request timeout in seconds
request_timeout_secs = 300

# Enable multipart upload support
enable_multipart = true

# Minimum part size for multipart uploads (5MB)
min_part_size = 5242880

# Use path-style requests (bucket/key vs bucket.domain.com/key)
virtual_host_style = false

[raft]
# Snapshot interval in log entries
snapshot_interval = 10000

# Maximum log entries in memory
max_log_entries = 100000

# Enable log compaction
enable_compaction = true
compaction_threshold = 50000

[parquet]
# Enable Parquet optimizations
enable_cache = true
cache_size = 268435456  # 256MB
enable_predicate_pushdown = true
enable_column_pruning = true

[metrics]
# Enable Prometheus metrics
enabled = true
interval_secs = 15

[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Enable JSON formatted logs
json_format = false

# Log file path (optional)
# file = "/var/log/anode/anode.log"
```

#### 3. Set Permissions

```bash
# Set ownership to anode user (create if doesn't exist)
sudo useradd -r -s /bin/false -M anode || true
sudo chown -R anode:anode /var/lib/anode
sudo chown -R anode:anode /etc/anode
```

### Environment Variables

Anode supports environment variable overrides:

```bash
# Log level override
export RUST_LOG=debug

# Data directory override
export ANODE_DATA_DIR=/custom/data/path

# Backtraces on errors
export RUST_BACKTRACE=1
```

Environment variables take precedence over configuration file settings.

## Starting the Server

### Manual Start (Testing)

For testing or development, start Anode manually:

```bash
# Start in foreground (bootstrap a new cluster)
anode \
    --config /etc/anode/config.toml \
    --node-id 1 \
    --bootstrap

# Or start with command-line overrides
anode \
    --config /etc/anode/config.toml \
    --node-id 1 \
    --s3-addr 0.0.0.0:8080 \
    --admin-addr 0.0.0.0:8081 \
    --grpc-addr 0.0.0.0:9090 \
    --data-dir /var/lib/anode/data \
    --log-level info \
    --bootstrap
```

### Background Start

Run in background with nohup:

```bash
# Start in background
nohup anode \
    --config /etc/anode/config.toml \
    --node-id 1 \
    --bootstrap \
    > /var/log/anode/anode.log 2>&1 &

# Save PID for later
echo $! > /var/run/anode.pid

# View logs
tail -f /var/log/anode/anode.log
```

### Using systemd (Recommended)

See [Systemd Service Setup](#systemd-service-setup) below for production deployments.

## Verifying Installation

### 1. Check Server Health

```bash
# Health check endpoint
curl http://localhost:8081/health

# Expected output:
# {"status":"healthy","node_id":1,"role":"leader","cluster_size":1}
```

### 2. Check Prometheus Metrics

```bash
# View all metrics
curl http://localhost:8081/metrics

# Check specific metrics
curl http://localhost:8081/metrics | grep anode_raft_leader
```

### 3. Test S3 API

Install AWS CLI if not already installed:

```bash
# Ubuntu/Debian
sudo apt-get install awscli

# Or via pip
pip install awscli

# Or via Homebrew (macOS)
brew install awscli
```

Configure AWS CLI:

```bash
# Configure credentials (use any values for local testing)
aws configure
# AWS Access Key ID: minioadmin
# AWS Secret Access Key: minioadmin
# Default region: us-east-1
# Default output format: json
```

Test basic operations:

```bash
# Create a bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket

# Upload a file
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/

# Download file
aws --endpoint-url http://localhost:8080 s3 cp s3://test-bucket/test.txt downloaded.txt
cat downloaded.txt

# Delete file
aws --endpoint-url http://localhost:8080 s3 rm s3://test-bucket/test.txt

# Delete bucket
aws --endpoint-url http://localhost:8080 s3 rb s3://test-bucket
```

## Systemd Service Setup

For production deployments, use systemd to manage the Anode service.

### 1. Create Systemd Service File

Create `/etc/systemd/system/anode.service`:

```ini
[Unit]
Description=Anode Object Storage Server
Documentation=https://github.com/christopherpaika/anode
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=anode
Group=anode

# Service start command
ExecStart=/usr/local/bin/anode \
    --config /etc/anode/config.toml \
    --node-id 1 \
    --bootstrap

# Restart policy
Restart=on-failure
RestartSec=5s

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Environment
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

# Directories
WorkingDirectory=/var/lib/anode
ReadWritePaths=/var/lib/anode

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=anode

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/anode /var/log/anode

[Install]
WantedBy=multi-user.target
```

### 2. Enable and Start Service

```bash
# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable anode

# Start the service
sudo systemctl start anode

# Check status
sudo systemctl status anode
```

### 3. Manage the Service

```bash
# View logs
sudo journalctl -u anode -f

# View recent logs
sudo journalctl -u anode -n 100

# Stop service
sudo systemctl stop anode

# Restart service
sudo systemctl restart anode

# Reload configuration (graceful)
sudo systemctl reload anode
```

### 4. Log Rotation

Create `/etc/logrotate.d/anode`:

```
/var/log/anode/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 anode anode
    sharedscripts
    postrotate
        systemctl reload anode > /dev/null 2>&1 || true
    endscript
}
```

## Common Configuration Scenarios

### High-Performance Configuration

For maximum throughput on high-end hardware:

```toml
[storage]
chunk_size = "8MB"          # Larger chunks for better throughput
sync_writes = false         # Disable fsync (faster but less durable)
metadata_cache_size = 1073741824  # 1GB cache
io_threads = 32             # More threads for I/O

[parquet]
cache_size = 536870912      # 512MB Parquet cache
```

### High-Durability Configuration

For maximum data safety:

```toml
[storage]
sync_writes = true          # Enable fsync on every write
chunk_size = "4MB"

[raft]
snapshot_interval = 5000    # More frequent snapshots
enable_compaction = true
```

### Low-Memory Configuration

For resource-constrained environments:

```toml
[storage]
metadata_cache_size = 52428800  # 50MB cache
io_threads = 4              # Fewer threads

[parquet]
cache_size = 104857600      # 100MB cache

[raft]
max_log_entries = 50000     # Smaller log buffer
```

### Debug Configuration

For troubleshooting:

```toml
[logging]
level = "debug"
json_format = false

# Or per-module levels
# module_levels = "anode_storage=debug,anode_raft=trace"
```

### Production Configuration with Logging

For production deployments:

```toml
[logging]
level = "info"
json_format = true
file = "/var/log/anode/anode.log"
rotate = true
max_size = 524288000  # 500MB

[metrics]
enabled = true
interval_secs = 15

[storage]
sync_writes = false  # Use filesystem journaling instead
metadata_cache_size = 536870912  # 512MB
```

## Upgrading

### Upgrade Process

1. **Backup Data** (always do this first):
```bash
# Stop service
sudo systemctl stop anode

# Backup data directory
sudo tar -czf /backup/anode-$(date +%Y%m%d).tar.gz /var/lib/anode/data

# Backup configuration
sudo cp /etc/anode/config.toml /backup/config.toml.bak
```

2. **Install New Version**:
```bash
# Download new binary
curl -LO https://github.com/christopherpaika/anode/releases/download/v0.2.0/anode-v0.2.0-linux-x86_64.tar.gz

# Backup old binary
sudo cp /usr/local/bin/anode /usr/local/bin/anode.old

# Install new binary
tar -xzf anode-v0.2.0-linux-x86_64.tar.gz
sudo mv anode /usr/local/bin/anode
sudo chmod +x /usr/local/bin/anode
```

3. **Verify New Version**:
```bash
anode --version
```

4. **Start Service**:
```bash
sudo systemctl start anode
sudo systemctl status anode
```

5. **Verify Health**:
```bash
curl http://localhost:8081/health
```

### Rollback

If upgrade fails:

```bash
# Stop service
sudo systemctl stop anode

# Restore old binary
sudo cp /usr/local/bin/anode.old /usr/local/bin/anode

# Restore data if needed
sudo tar -xzf /backup/anode-YYYYMMDD.tar.gz -C /

# Start service
sudo systemctl start anode
```

## Troubleshooting

### Service Won't Start

**Check logs:**
```bash
sudo journalctl -u anode -n 50
```

**Common issues:**

1. **Port already in use:**
```bash
# Check what's using the port
sudo ss -tulpn | grep :8080

# Change port in config or stop conflicting service
```

2. **Permission denied:**
```bash
# Fix data directory permissions
sudo chown -R anode:anode /var/lib/anode
sudo chmod 755 /var/lib/anode
```

3. **Configuration error:**
```bash
# Test configuration
anode --config /etc/anode/config.toml --node-id 1 --help

# Check for syntax errors
cat /etc/anode/config.toml
```

### High Memory Usage

**Check metrics:**
```bash
curl http://localhost:8081/metrics | grep memory
```

**Solutions:**
- Reduce `metadata_cache_size` in config
- Reduce `parquet.cache_size`
- Reduce `io_threads`
- Enable log compaction

### Slow Performance

**Check disk I/O:**
```bash
iostat -x 1 10
```

**Solutions:**
- Enable `sync_writes = false` (if data durability allows)
- Use SSD/NVMe storage
- Increase `io_threads`
- Increase `chunk_size` for large files

### Disk Space Issues

**Check usage:**
```bash
df -h /var/lib/anode
du -sh /var/lib/anode/data
```

**Solutions:**
```bash
# Run garbage collection
curl -X POST http://localhost:8081/admin/gc

# Delete old data
aws --endpoint-url http://localhost:8080 s3 rb s3://old-bucket --force
```

### Network Connectivity Issues

**Test endpoints:**
```bash
# Test S3 API
curl -v http://localhost:8080/

# Test admin API
curl http://localhost:8081/health

# Test from remote host
curl http://YOUR_SERVER_IP:8080/
```

**Solutions:**
- Check firewall rules: `sudo ufw status`
- Verify bind addresses in config
- Check network connectivity: `ping YOUR_SERVER_IP`

### Data Corruption

**Run integrity check:**
```bash
# Run scrub (checks all chunk checksums)
curl -X POST http://localhost:8081/admin/scrub

# Check results
curl http://localhost:8081/admin/scrub/results
```

**Repair:**
```bash
# For standalone, restore from backup
sudo systemctl stop anode
sudo tar -xzf /backup/anode-YYYYMMDD.tar.gz -C /
sudo systemctl start anode
```

## Next Steps

After successfully deploying a standalone server:

1. **Configure Monitoring**: Set up [Prometheus and Grafana](./monitoring.md) for metrics
2. **Set Up Backups**: Implement regular backup strategy
3. **Security Hardening**: Review [Security Guide](./security.md)
4. **Performance Tuning**: See [Performance Guide](./performance.md)
5. **Expand to Cluster**: See [Operations Guide](./operations.md) for multi-node setup

## Additional Resources

- [Configuration Reference](./configuration.md) - Complete configuration options
- [S3 API Reference](./s3-api.md) - Supported S3 operations
- [Architecture Documentation](./architecture.md) - System design and internals
- [Development Guide](./development.md) - Contributing to Anode

## Getting Help

- **Documentation**: https://github.com/christopherpaika/anode/docs
- **Issue Tracker**: https://github.com/christopherpaika/anode/issues
- **Discussions**: https://github.com/christopherpaika/anode/discussions
- **Security Issues**: security@anode.io (do not open public issues)
