# Docker Deployment Guide

This guide covers deploying Anode Object Storage using Docker and Docker Compose, from single-node development setups to multi-node production clusters with full monitoring.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Single-Node Docker Deployment](#single-node-docker-deployment)
- [Multi-Node Docker Compose Cluster](#multi-node-docker-compose-cluster)
- [Monitoring Stack Setup](#monitoring-stack-setup)
- [Volume Management and Persistence](#volume-management-and-persistence)
- [Environment Variables and Configuration](#environment-variables-and-configuration)
- [Networking Considerations](#networking-considerations)
- [Health Checks](#health-checks)
- [Troubleshooting Common Docker Issues](#troubleshooting-common-docker-issues)
- [Production Deployment Checklist](#production-deployment-checklist)
- [Advanced Topics](#advanced-topics)

## Prerequisites

### System Requirements

**Minimum requirements for Docker deployment:**

- Docker Engine 20.10+ or Docker Desktop
- Docker Compose 2.0+ (included with Docker Desktop)
- 4GB RAM available for containers
- 20GB available disk space
- Linux, macOS, or Windows with WSL2

**Recommended for production:**

- Docker Engine 24.0+
- 16GB+ RAM
- 100GB+ SSD storage
- Dedicated network interface
- Linux host (Ubuntu 22.04 or Debian 12)

### Installing Docker

**Linux (Ubuntu/Debian):**

```bash
# Install Docker using official script
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Add your user to docker group
sudo usermod -aG docker $USER

# Log out and back in for group changes to take effect
newgrp docker

# Verify installation
docker --version
docker compose version
```

**macOS:**

```bash
# Install Docker Desktop via Homebrew
brew install --cask docker

# Or download from: https://docs.docker.com/desktop/install/mac-install/

# Start Docker Desktop from Applications
# Verify installation
docker --version
docker compose version
```

**Windows:**

1. Install WSL2: https://docs.microsoft.com/en-us/windows/wsl/install
2. Install Docker Desktop: https://docs.docker.com/desktop/install/windows-install/
3. Enable WSL2 integration in Docker Desktop settings
4. Verify in PowerShell or WSL2 terminal:

```powershell
docker --version
docker compose version
```

### Verifying Docker Installation

```bash
# Test Docker
docker run hello-world

# Check Docker system info
docker info

# Check available resources
docker system df
```

## Single-Node Docker Deployment

A single-node deployment is perfect for development, testing, or small-scale production workloads.

### Quick Start with Docker Run

The fastest way to get started:

```bash
# Pull the latest image
docker pull ghcr.io/christopherpaika/anode:latest

# Create directories for data and configuration
mkdir -p ~/anode/data
mkdir -p ~/anode/config

# Download example configuration
curl -o ~/anode/config/anode.toml \
  https://raw.githubusercontent.com/christopherpaika/anode/master/crates/anode/config/anode.example.toml

# Run Anode container
docker run -d \
  --name anode \
  -p 8080:8080 \
  -p 8081:8081 \
  -p 9090:9090 \
  -v ~/anode/data:/data \
  -v ~/anode/config:/config:ro \
  -e ANODE_NODE_ID=1 \
  -e ANODE_NODE_NAME=anode-standalone \
  -e ANODE_BOOTSTRAP=true \
  -e ANODE_LOG_LEVEL=info \
  -e RUST_BACKTRACE=1 \
  --restart unless-stopped \
  ghcr.io/christopherpaika/anode:latest

# Check container is running
docker ps

# View logs
docker logs -f anode

# Check health
curl http://localhost:8081/health
```

### Testing the Deployment

Once your container is running, test basic S3 operations:

```bash
# Configure AWS CLI
aws configure set aws_access_key_id minioadmin
aws configure set aws_secret_access_key minioadmin

# Create a bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket

# Upload a file
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/

# Download the file
aws --endpoint-url http://localhost:8080 s3 cp s3://test-bucket/test.txt downloaded.txt
cat downloaded.txt

# Clean up
aws --endpoint-url http://localhost:8080 s3 rm s3://test-bucket/test.txt
aws --endpoint-url http://localhost:8080 s3 rb s3://test-bucket
```

### Using Docker Compose for Single Node

For better management, use Docker Compose:

Create `docker-compose.yml`:

```yaml
version: '3.9'

services:
  anode:
    image: ghcr.io/christopherpaika/anode:latest
    container_name: anode-standalone
    hostname: anode
    ports:
      - "8080:8080"  # S3 API
      - "8081:8081"  # Admin API
      - "9090:9090"  # gRPC
    volumes:
      - anode-data:/data
      - ./config:/config:ro
    environment:
      - ANODE_NODE_ID=1
      - ANODE_NODE_NAME=anode-standalone
      - ANODE_BIND_ADDR=0.0.0.0
      - ANODE_BOOTSTRAP=true
      - ANODE_LOG_LEVEL=info
      - RUST_BACKTRACE=1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8081/health"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 30s
    restart: unless-stopped
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '2.0'
        reservations:
          memory: 512M
          cpus: '0.5'

volumes:
  anode-data:
    driver: local
```

Start the service:

```bash
# Start in detached mode
docker compose up -d

# View logs
docker compose logs -f

# Check status
docker compose ps

# Stop
docker compose down

# Stop and remove volumes (WARNING: deletes data)
docker compose down -v
```

## Multi-Node Docker Compose Cluster

A multi-node cluster provides high availability, data replication, and fault tolerance.

### Basic 3-Node Cluster

The Anode project includes a production-ready 3-node cluster configuration. Navigate to the `deploy/docker` directory:

```bash
cd deploy/docker

# Review the configuration
cat docker-compose.yml

# Start the cluster using the helper script
./scripts/start-cluster.sh

# Or start manually
docker compose up -d

# Check all nodes are healthy
docker compose ps

# View logs from all nodes
docker compose logs -f

# View logs from specific node
docker compose logs -f anode-1
```

### Cluster Architecture

The 3-node cluster consists of:

- **anode-1** (Bootstrap node):
  - S3 API: http://localhost:8080
  - Admin API: http://localhost:8081
  - gRPC: localhost:9090

- **anode-2**:
  - S3 API: http://localhost:8082
  - Admin API: http://localhost:8083
  - gRPC: localhost:9091

- **anode-3**:
  - S3 API: http://localhost:8084
  - Admin API: http://localhost:8085
  - gRPC: localhost:9092

### Cluster Formation

The cluster uses the following formation strategy:

1. **anode-1** starts with `ANODE_BOOTSTRAP=true` and initializes the Raft cluster
2. **anode-2** and **anode-3** join by connecting to `anode-1:9090`
3. All nodes exchange cluster membership information via gRPC
4. Raft leader election occurs (typically anode-1 becomes leader)
5. Data replication begins with configurable replication factor

### Verifying Cluster Health

```bash
# Check each node's health
curl http://localhost:8081/health  # anode-1
curl http://localhost:8083/health  # anode-2
curl http://localhost:8085/health  # anode-3

# Check cluster membership
curl http://localhost:8081/admin/cluster/members

# Check Raft status
curl http://localhost:8081/admin/raft/status

# Run comprehensive health test
./scripts/test-cluster-health.sh
```

The health test script performs:
- Image build verification
- Container startup checks
- Health endpoint validation
- Basic S3 operations (create bucket, upload, download, delete)
- Automatic cleanup

### Load Balancing Across Nodes

You can distribute requests across all nodes. Each node can handle S3 requests:

```bash
# Upload to different nodes
aws --endpoint-url http://localhost:8080 s3 cp file1.txt s3://bucket/  # anode-1
aws --endpoint-url http://localhost:8082 s3 cp file2.txt s3://bucket/  # anode-2
aws --endpoint-url http://localhost:8084 s3 cp file3.txt s3://bucket/  # anode-3

# All files are replicated across the cluster
aws --endpoint-url http://localhost:8080 s3 ls s3://bucket/
```

For production load balancing, add a reverse proxy (see [Advanced Topics](#advanced-topics)).

### Cluster Management Commands

```bash
# Stop cluster gracefully
./scripts/stop-cluster.sh

# Stop and remove all data
./scripts/stop-cluster.sh --volumes

# Restart single node
docker compose restart anode-2

# Scale cluster (add more nodes by editing docker-compose.yml)
# Add node configuration, then:
docker compose up -d anode-4

# Remove a node from cluster
docker compose stop anode-3
docker compose rm -f anode-3
```

## Monitoring Stack Setup

Anode includes a comprehensive monitoring stack with Prometheus, Grafana, and Alertmanager.

### Starting the Monitoring Stack

The monitoring stack is provided in a separate compose file:

```bash
cd deploy/docker

# Start Anode cluster first
docker compose up -d

# Start monitoring stack
docker compose -f docker-compose.monitoring.yml up -d

# Or use combined command
docker compose -f docker-compose.yml -f docker-compose.monitoring.yml up -d
```

### Accessing Monitoring Dashboards

Once started, access the monitoring interfaces:

- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `admin` (change on first login)

- **Prometheus**: http://localhost:9090
  - Query interface for metrics
  - Target health status
  - Alert rules

- **Alertmanager**: http://localhost:9093
  - Alert management
  - Notification routing
  - Silence management

- **Node Exporter**: http://localhost:9100/metrics
  - System-level metrics (CPU, memory, disk, network)

### Prometheus Configuration

The Prometheus configuration (`deploy/docker/prometheus/prometheus.yml`) scrapes metrics from:

- All Anode nodes (ports 8081, 8083, 8085)
- Node Exporter (port 9100)
- cAdvisor (port 8080)
- Pushgateway (port 9091)

Metrics are retained for 30 days and stored in Docker volumes.

### Grafana Setup

Grafana comes pre-configured with Prometheus as a data source. To import Anode dashboards:

1. Navigate to http://localhost:3000
2. Login with admin/admin
3. Go to Dashboards > Import
4. Import dashboards from `deploy/docker/grafana/provisioning/dashboards/`

Key dashboards include:
- Cluster Overview
- Storage Metrics
- Raft Consensus Status
- HTTP/S3 API Performance
- System Resources

### Alert Configuration

Alertmanager is configured to handle alerts based on rules in `deploy/docker/prometheus/alerts.yml`:

- High error rate on S3 API
- Node down
- Raft leader election failures
- High memory usage
- Disk space warnings

Configure notification channels by editing `deploy/docker/alertmanager/config.yml`:

```yaml
receivers:
  - name: 'email'
    email_configs:
      - to: 'ops@example.com'
        from: 'alertmanager@example.com'
        smarthost: 'smtp.example.com:587'
        auth_username: 'alertmanager@example.com'
        auth_password: 'password'

  - name: 'slack'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
        channel: '#alerts'
```

Restart Alertmanager after configuration changes:

```bash
docker compose -f docker-compose.monitoring.yml restart alertmanager
```

### Key Metrics to Monitor

Monitor these critical metrics in production:

**Storage Metrics:**
- `anode_storage_objects_total` - Total objects stored
- `anode_storage_bytes_total` - Total storage used
- `anode_storage_operations_total` - Operation counts by type

**Raft Metrics:**
- `anode_raft_leader_changes_total` - Leader elections
- `anode_raft_log_entries` - Raft log size
- `anode_raft_commit_duration_seconds` - Commit latency

**HTTP Metrics:**
- `anode_http_requests_total` - Request counts by method/status
- `anode_http_request_duration_seconds` - Request latency
- `anode_http_requests_in_flight` - Concurrent requests

**System Metrics:**
- `node_memory_MemAvailable_bytes` - Available memory
- `node_filesystem_avail_bytes` - Available disk space
- `node_network_receive_bytes_total` - Network throughput

## Volume Management and Persistence

Docker volumes ensure data persists across container restarts.

### Volume Structure

The default configuration creates these volumes:

```bash
# List Anode volumes
docker volume ls | grep anode

# Typical output:
# deploy_anode-1-data      # Node 1 data
# deploy_anode-2-data      # Node 2 data
# deploy_anode-3-data      # Node 3 data
# anode-prometheus-data    # Prometheus metrics
# anode-grafana-data       # Grafana dashboards
# anode-alertmanager-data  # Alertmanager state
```

### Inspecting Volumes

```bash
# Inspect volume details
docker volume inspect deploy_anode-1-data

# View volume contents
docker run --rm -v deploy_anode-1-data:/data \
  ubuntu ls -lah /data
```

### Backing Up Volumes

**Backup using tar:**

```bash
# Backup single node data
docker run --rm \
  -v deploy_anode-1-data:/data \
  -v $(pwd):/backup \
  ubuntu tar czf /backup/anode-1-backup-$(date +%Y%m%d).tar.gz -C /data .

# Backup all node data
for node in 1 2 3; do
  docker run --rm \
    -v deploy_anode-${node}-data:/data \
    -v $(pwd):/backup \
    ubuntu tar czf /backup/anode-${node}-backup-$(date +%Y%m%d).tar.gz -C /data .
done

# Backup monitoring data
docker run --rm \
  -v anode-prometheus-data:/data \
  -v $(pwd):/backup \
  ubuntu tar czf /backup/prometheus-backup-$(date +%Y%m%d).tar.gz -C /data .
```

**Backup using volume copy:**

```bash
# Create backup volume
docker volume create anode-1-backup

# Copy data to backup volume
docker run --rm \
  -v deploy_anode-1-data:/source:ro \
  -v anode-1-backup:/backup \
  ubuntu cp -av /source/. /backup/
```

### Restoring Volumes

**Restore from tar backup:**

```bash
# Stop cluster
docker compose down

# Remove old volume
docker volume rm deploy_anode-1-data

# Create new volume
docker volume create deploy_anode-1-data

# Restore backup
docker run --rm \
  -v deploy_anode-1-data:/data \
  -v $(pwd):/backup \
  ubuntu tar xzf /backup/anode-1-backup-20250101.tar.gz -C /data

# Start cluster
docker compose up -d
```

**Restore from volume backup:**

```bash
# Remove old volume
docker volume rm deploy_anode-1-data

# Create new volume
docker volume create deploy_anode-1-data

# Restore from backup
docker run --rm \
  -v anode-1-backup:/source:ro \
  -v deploy_anode-1-data:/target \
  ubuntu cp -av /source/. /target/
```

### Using Host Bind Mounts

For easier access to data, use bind mounts instead of volumes:

```yaml
services:
  anode-1:
    volumes:
      # Bind mount instead of named volume
      - /var/lib/anode/node-1:/data
      - ./config:/config:ro
```

This allows direct file system access:

```bash
# Backup with rsync
rsync -av /var/lib/anode/node-1/ /backup/anode-1/

# View files directly
ls -lah /var/lib/anode/node-1/
```

**Important:** Ensure proper permissions when using bind mounts:

```bash
# Create directories
sudo mkdir -p /var/lib/anode/{node-1,node-2,node-3}

# Set ownership (UID 65532 is the nonroot user in distroless images)
sudo chown -R 65532:65532 /var/lib/anode/

# Or use the runtime-debian target which uses UID 1000
sudo chown -R 1000:1000 /var/lib/anode/
```

### Automated Backup Script

Create a backup script for regular backups:

```bash
#!/bin/bash
# save as: backup-anode.sh

BACKUP_DIR="/backup/anode"
DATE=$(date +%Y%m%d-%H%M%S)

mkdir -p "$BACKUP_DIR"

# Backup all node data
for node in 1 2 3; do
  echo "Backing up anode-$node..."
  docker run --rm \
    -v deploy_anode-${node}-data:/data:ro \
    -v $BACKUP_DIR:/backup \
    ubuntu tar czf /backup/anode-${node}-${DATE}.tar.gz -C /data .
done

# Backup configuration
echo "Backing up configuration..."
tar czf "$BACKUP_DIR/config-${DATE}.tar.gz" deploy/docker/config

# Cleanup old backups (keep last 7 days)
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +7 -delete

echo "Backup completed: $BACKUP_DIR"
```

Schedule with cron:

```bash
# Run daily at 2 AM
0 2 * * * /path/to/backup-anode.sh
```

## Environment Variables and Configuration

Anode can be configured via environment variables, configuration files, or a combination of both.

### Environment Variable Reference

**Node Configuration:**

```bash
ANODE_NODE_ID=1                    # Unique node identifier (required)
ANODE_NODE_NAME=anode-1            # Human-readable name
ANODE_DATA_DIR=/data               # Data storage directory
ANODE_CONFIG_DIR=/config           # Configuration directory
```

**Network Configuration:**

```bash
ANODE_BIND_ADDR=0.0.0.0            # Bind address for all interfaces
ANODE_ADVERTISE_ADDR=anode-1       # Address advertised to other nodes
ANODE_HTTP_PORT=8080               # S3 API port
ANODE_ADMIN_PORT=8081              # Admin/metrics API port
ANODE_GRPC_PORT=9090               # gRPC communication port
```

**Cluster Configuration:**

```bash
ANODE_BOOTSTRAP=false              # Initialize new cluster (only first node)
ANODE_JOIN_ADDR=anode-1:9090       # Address of node to join
ANODE_CLUSTER_PEERS=anode-1:9090,anode-2:9090,anode-3:9090  # All cluster members
```

**Logging Configuration:**

```bash
ANODE_LOG_LEVEL=info               # Log level: trace, debug, info, warn, error
ANODE_LOG_FORMAT=json              # Log format: json or text
RUST_LOG=anode=info,openraft=info  # Fine-grained logging per module
RUST_BACKTRACE=1                   # Enable backtraces on errors
```

**Storage Configuration:**

```bash
ANODE_CHUNK_SIZE=4194304           # Chunk size in bytes (4MB)
ANODE_FSYNC_ENABLED=true           # Fsync on writes for durability
ANODE_BLOB_CACHE_SIZE=1073741824   # Blob cache size in bytes (1GB)
ANODE_METADATA_CACHE_SIZE=268435456  # Metadata cache in bytes (256MB)
```

**Replication Configuration:**

```bash
ANODE_DEFAULT_REPLICATION_FACTOR=3  # Number of replicas
ANODE_NUM_PLACEMENT_GROUPS=128      # Placement groups for distribution
```

**Authentication Configuration:**

```bash
ANODE_AUTH_ENABLED=true            # Enable authentication
ANODE_DEFAULT_ACCESS_KEY=minioadmin  # Default access key
ANODE_DEFAULT_SECRET_KEY=minioadmin  # Default secret key
```

**Performance Configuration:**

```bash
ANODE_WORKER_THREADS=0             # Worker threads (0 = CPU cores)
ANODE_BLOCKING_THREADS=512         # Blocking I/O threads
```

**Security Configuration:**

```bash
ANODE_TLS_ENABLED=false            # Enable TLS
ANODE_ENCRYPTION_AT_REST=false     # Enable encryption at rest
```

### Using .env Files

Create a `.env` file in the same directory as `docker-compose.yml`:

```bash
# Copy example
cp deploy/docker/.env.example .env

# Edit configuration
nano .env
```

Example `.env`:

```env
# Node configuration
ANODE_NODE_ID=1
ANODE_NODE_NAME=anode-production

# Cluster
ANODE_DEFAULT_REPLICATION_FACTOR=3
ANODE_NUM_PLACEMENT_GROUPS=256

# Performance
ANODE_CHUNK_SIZE=8388608
ANODE_BLOB_CACHE_SIZE=2147483648
ANODE_WORKER_THREADS=16

# Security (CHANGE THESE!)
ANODE_DEFAULT_ACCESS_KEY=your-access-key
ANODE_DEFAULT_SECRET_KEY=your-secret-key-min-8-chars

# Logging
ANODE_LOG_LEVEL=info
ANODE_LOG_FORMAT=json
```

Docker Compose automatically loads `.env` files:

```bash
docker compose up -d
```

### Configuration File Priority

Configuration is loaded in this order (later overrides earlier):

1. Default values
2. Configuration file (`/config/anode.toml`)
3. Environment variables
4. Command-line arguments

### Passing Configuration Files

Mount custom configuration:

```yaml
services:
  anode:
    volumes:
      - ./custom-config.toml:/config/anode.toml:ro
```

Or use environment variable to specify config path:

```yaml
services:
  anode:
    environment:
      - ANODE_CONFIG_FILE=/custom/path/config.toml
    volumes:
      - ./custom-config.toml:/custom/path/config.toml:ro
```

## Networking Considerations

### Docker Network Modes

**Bridge Network (Default):**

The default configuration uses a custom bridge network:

```yaml
networks:
  anode-net:
    driver: bridge
    ipam:
      config:
        - subnet: 172.28.0.0/16
```

Benefits:
- Container isolation
- Automatic DNS resolution
- Custom subnet control

**Host Network:**

For maximum performance, use host networking:

```yaml
services:
  anode:
    network_mode: host
```

Considerations:
- No port mapping needed
- Direct access to host network
- No network isolation
- Linux only

**Overlay Network (Swarm):**

For multi-host deployments:

```yaml
networks:
  anode-net:
    driver: overlay
    attachable: true
```

### Port Mapping

Understand the port mappings:

```yaml
ports:
  - "8080:8080"  # host:container
```

For multiple instances on same host, map to different host ports:

```yaml
# Node 1
ports:
  - "8080:8080"

# Node 2
ports:
  - "8082:8080"  # Different host port, same container port
```

### Firewall Configuration

**Docker automatically manages iptables**, but for external access:

**UFW (Ubuntu):**

```bash
# Allow S3 API
sudo ufw allow 8080/tcp

# Allow Admin API (restrict to internal network)
sudo ufw allow from 10.0.0.0/8 to any port 8081

# Allow gRPC (restrict to cluster network)
sudo ufw allow from 172.28.0.0/16 to any port 9090

# Apply rules
sudo ufw reload
```

**firewalld (RHEL/CentOS):**

```bash
# Allow S3 API
sudo firewall-cmd --permanent --add-port=8080/tcp

# Allow Admin API from internal network
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="8081" protocol="tcp" accept'

# Reload
sudo firewall-cmd --reload
```

### DNS and Service Discovery

Docker Compose provides automatic DNS:

```yaml
services:
  anode-1:
    hostname: anode-1  # Resolvable as 'anode-1' within network
```

Containers can communicate using hostnames:

```bash
# From anode-2 container
docker exec anode-2 ping anode-1
docker exec anode-2 curl http://anode-1:8081/health
```

### External Access and Reverse Proxy

For production, use a reverse proxy (nginx, traefik, HAProxy):

**nginx example:**

```nginx
upstream anode_s3 {
    least_conn;
    server localhost:8080;
    server localhost:8082;
    server localhost:8084;
}

server {
    listen 80;
    server_name s3.example.com;

    location / {
        proxy_pass http://anode_s3;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # For large uploads
        client_max_body_size 5G;
        proxy_request_buffering off;
    }
}
```

**Traefik with Docker Compose:**

```yaml
services:
  traefik:
    image: traefik:v2.10
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./traefik.yml:/traefik.yml:ro
    networks:
      - anode-net

  anode-1:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.anode.rule=Host(`s3.example.com`)"
      - "traefik.http.services.anode.loadbalancer.server.port=8080"
```

### IPv6 Support

Enable IPv6 in Docker daemon (`/etc/docker/daemon.json`):

```json
{
  "ipv6": true,
  "fixed-cidr-v6": "2001:db8:1::/64"
}
```

Restart Docker:

```bash
sudo systemctl restart docker
```

Update docker-compose.yml:

```yaml
networks:
  anode-net:
    enable_ipv6: true
    ipam:
      config:
        - subnet: 172.28.0.0/16
        - subnet: 2001:db8:1::/64
```

## Health Checks

Health checks ensure containers are running correctly and enable automatic recovery.

### Health Check Configuration

The default health check configuration:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8081/health"]
  interval: 10s       # Check every 10 seconds
  timeout: 5s         # Timeout after 5 seconds
  retries: 3          # Retry 3 times before marking unhealthy
  start_period: 30s   # Grace period for startup
```

### Health Endpoints

Anode provides multiple health endpoints:

**Overall Health:** `/health`

```bash
curl http://localhost:8081/health

# Response:
{
  "status": "healthy",
  "node_id": 1,
  "role": "leader",
  "cluster_size": 3
}
```

**Liveness:** `/live`

Checks if the process is alive:

```bash
curl http://localhost:8081/live

# Returns: 200 OK if alive, 503 if dead
```

**Readiness:** `/ready`

Checks if the node can serve traffic:

```bash
curl http://localhost:8081/ready

# Returns: 200 OK if ready, 503 if not ready
```

### Monitoring Health Status

```bash
# Check health of all containers
docker compose ps

# View health check logs
docker inspect anode-1 --format='{{json .State.Health}}' | jq

# Watch health status
watch -n 2 'docker compose ps'
```

### Custom Health Checks

For the distroless image (no curl), use TCP check:

```yaml
healthcheck:
  test: ["CMD-SHELL", "timeout 1 bash -c '</dev/tcp/localhost/8081'"]
  interval: 10s
  timeout: 5s
  retries: 3
```

Or use the debian variant with curl available:

```dockerfile
# Build with debian target
docker build --target runtime-debian -t anode:debian .
```

### Automatic Restart Policies

Configure restart behavior:

```yaml
services:
  anode:
    restart: unless-stopped  # Recommended for most cases
    # Other options:
    # restart: no            # Never restart
    # restart: always        # Always restart
    # restart: on-failure    # Restart only on failure
```

### Healthcheck Integration with Dependencies

Ensure dependent services start only when dependencies are healthy:

```yaml
services:
  anode-1:
    # Bootstrap node

  anode-2:
    depends_on:
      anode-1:
        condition: service_healthy  # Wait for anode-1 to be healthy
```

## Troubleshooting Common Docker Issues

### Container Won't Start

**Check logs:**

```bash
# View recent logs
docker logs anode-1

# Follow logs in real-time
docker logs -f anode-1

# View last 100 lines
docker logs --tail 100 anode-1

# Show timestamps
docker logs -t anode-1
```

**Common issues:**

**1. Port already in use:**

```bash
# Find what's using the port
sudo lsof -i :8080
# or
sudo ss -tulpn | grep :8080

# Solution: Stop conflicting service or change port mapping
docker compose down
# Edit docker-compose.yml to use different port
docker compose up -d
```

**2. Permission denied on volumes:**

```bash
# Check volume permissions
docker run --rm -v deploy_anode-1-data:/data ubuntu ls -la /data

# Fix permissions (for distroless image, UID 65532)
docker run --rm -v deploy_anode-1-data:/data ubuntu chown -R 65532:65532 /data

# For debian image (UID 1000)
docker run --rm -v deploy_anode-1-data:/data ubuntu chown -R 1000:1000 /data
```

**3. Configuration file errors:**

```bash
# Validate configuration file exists
docker run --rm -v $(pwd)/config:/config ubuntu cat /config/anode.toml

# Check environment variables
docker exec anode-1 env | grep ANODE
```

**4. Image pull failures:**

```bash
# Retry pull
docker pull ghcr.io/christopherpaika/anode:latest

# Use different registry
docker pull docker.io/christopherpaika/anode:latest

# Build locally if pull fails
cd deploy/docker
docker build -t anode:latest -f Dockerfile ../..
```

### Cluster Won't Form

**Check network connectivity:**

```bash
# Verify network exists
docker network ls | grep anode

# Inspect network
docker network inspect deploy_anode-net

# Test connectivity between containers
docker exec anode-2 ping anode-1
docker exec anode-2 nc -zv anode-1 9090

# Check DNS resolution
docker exec anode-2 nslookup anode-1
```

**Check cluster configuration:**

```bash
# Verify ANODE_CLUSTER_PEERS is correct
docker exec anode-1 env | grep CLUSTER

# Check join address
docker exec anode-2 env | grep JOIN

# Verify bootstrap flag (only on first node)
docker exec anode-1 env | grep BOOTSTRAP
```

**Check Raft status:**

```bash
# View Raft status from each node
curl http://localhost:8081/admin/raft/status
curl http://localhost:8083/admin/raft/status
curl http://localhost:8085/admin/raft/status

# Check cluster membership
curl http://localhost:8081/admin/cluster/members
```

**Solution steps:**

```bash
# 1. Stop all nodes
docker compose down

# 2. Remove old Raft state (if needed)
docker volume rm deploy_anode-1-data deploy_anode-2-data deploy_anode-3-data

# 3. Start bootstrap node first
docker compose up -d anode-1

# 4. Wait for bootstrap node to be healthy
docker logs -f anode-1
# Look for: "Raft initialized" or "Became leader"

# 5. Start remaining nodes
docker compose up -d anode-2 anode-3

# 6. Verify cluster formation
curl http://localhost:8081/admin/cluster/members
```

### High Memory Usage

**Check memory usage:**

```bash
# View resource usage
docker stats

# Check specific container
docker stats anode-1 --no-stream

# Inspect memory limits
docker inspect anode-1 | grep -A 10 Memory
```

**Solutions:**

```yaml
# Reduce memory limits in docker-compose.yml
deploy:
  resources:
    limits:
      memory: 1G  # Reduced from 2G
```

```bash
# Reduce cache sizes via environment
ANODE_BLOB_CACHE_SIZE=536870912      # 512MB instead of 1GB
ANODE_METADATA_CACHE_SIZE=134217728  # 128MB instead of 256MB
```

```bash
# Restart with new limits
docker compose down
docker compose up -d
```

### Disk Space Issues

**Check disk usage:**

```bash
# Check Docker disk usage
docker system df

# Detailed usage breakdown
docker system df -v

# Check specific volume
docker run --rm -v deploy_anode-1-data:/data ubuntu du -sh /data
```

**Clean up:**

```bash
# Remove stopped containers
docker container prune -f

# Remove unused images
docker image prune -a -f

# Remove unused volumes
docker volume prune -f

# Remove everything unused
docker system prune -a --volumes -f

# WARNING: The above removes ALL unused resources
```

**Increase available space:**

```bash
# Move Docker data directory
# Edit /etc/docker/daemon.json
{
  "data-root": "/new/path/to/docker"
}

# Restart Docker
sudo systemctl stop docker
sudo mv /var/lib/docker /new/path/to/docker
sudo systemctl start docker
```

### Network Issues

**DNS resolution problems:**

```bash
# Test DNS
docker exec anode-1 nslookup google.com
docker exec anode-1 cat /etc/resolv.conf

# Use custom DNS in docker-compose.yml
services:
  anode-1:
    dns:
      - 8.8.8.8
      - 8.8.4.4
```

**Connection timeouts:**

```bash
# Increase timeouts
services:
  anode-1:
    environment:
      - ANODE_REQUEST_TIMEOUT_SECS=600  # 10 minutes
```

**MTU issues:**

```bash
# Set custom MTU
networks:
  anode-net:
    driver: bridge
    driver_opts:
      com.docker.network.driver.mtu: 1450
```

### Performance Issues

**Check container resources:**

```bash
# Real-time stats
docker stats

# Check if CPU throttling
docker inspect anode-1 | grep -A 5 CpuStats
```

**Enable performance monitoring:**

```bash
# Start with monitoring stack
docker compose -f docker-compose.yml -f docker-compose.monitoring.yml up -d

# Access Grafana
open http://localhost:3000
```

**Optimize performance:**

```yaml
# Use host network mode (Linux only)
network_mode: host

# Increase resource limits
deploy:
  resources:
    limits:
      cpus: '4.0'
      memory: 4G

# Disable unnecessary features
environment:
  - ANODE_ENABLE_TRACING=false
  - ANODE_FSYNC_ENABLED=false  # Less durable but faster
```

### Logs Not Appearing

**Check logging driver:**

```bash
# View logging configuration
docker inspect anode-1 | grep -A 5 LogConfig

# Use json-file driver explicitly
services:
  anode:
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

**View logs with different drivers:**

```bash
# For journald driver
sudo journalctl CONTAINER_NAME=anode-1

# For syslog driver
tail -f /var/log/syslog | grep anode
```

### Debug Mode

Enable debug logging:

```bash
# Temporary debug mode
docker compose exec anode-1 kill -USR1 1

# Or set via environment
services:
  anode:
    environment:
      - RUST_LOG=debug
      - ANODE_LOG_LEVEL=debug
```

## Production Deployment Checklist

Before deploying to production, verify:

### Security

- [ ] Change default credentials (`ANODE_DEFAULT_ACCESS_KEY` and `ANODE_DEFAULT_SECRET_KEY`)
- [ ] Enable TLS/HTTPS for S3 API
- [ ] Restrict admin API access (firewall rules)
- [ ] Enable encryption at rest if required
- [ ] Use Docker secrets for sensitive data
- [ ] Run containers as non-root user (default in Anode)
- [ ] Enable audit logging
- [ ] Implement network segmentation
- [ ] Regular security updates

### Reliability

- [ ] Configure proper health checks
- [ ] Set restart policies (`restart: unless-stopped`)
- [ ] Configure resource limits (CPU, memory)
- [ ] Set up monitoring and alerting
- [ ] Implement automated backups
- [ ] Test disaster recovery procedures
- [ ] Configure log rotation
- [ ] Set appropriate replication factor (≥3)
- [ ] Use persistent volumes (not ephemeral)

### Performance

- [ ] Size resources appropriately (CPU, memory, disk)
- [ ] Use SSD/NVMe storage
- [ ] Optimize chunk size for workload
- [ ] Configure cache sizes based on available RAM
- [ ] Enable fsync based on durability requirements
- [ ] Set appropriate worker thread counts
- [ ] Configure kernel parameters (file descriptors, network)
- [ ] Implement load balancing
- [ ] Monitor and optimize based on metrics

### Operations

- [ ] Document deployment architecture
- [ ] Create runbooks for common operations
- [ ] Set up centralized logging
- [ ] Configure backup retention policies
- [ ] Implement monitoring dashboards
- [ ] Set up alerting and on-call rotation
- [ ] Test upgrade procedures
- [ ] Perform capacity planning
- [ ] Document disaster recovery procedures

### Configuration

- [ ] Use `.env` files for environment-specific config
- [ ] Version control configuration files
- [ ] Validate configuration before deployment
- [ ] Use meaningful node names and IDs
- [ ] Configure appropriate placement groups
- [ ] Set retention policies for Prometheus/Grafana
- [ ] Configure alerting thresholds
- [ ] Document all configuration changes

## Advanced Topics

### Building Custom Images

Build from source with custom options:

```bash
# Clone repository
git clone https://github.com/christopherpaika/anode.git
cd anode

# Build with specific target
docker build \
  --target runtime-debian \
  -t anode:custom \
  -f deploy/docker/Dockerfile \
  .

# Build with build args
docker build \
  --build-arg RUST_VERSION=1.75 \
  -t anode:rust-1.75 \
  -f deploy/docker/Dockerfile \
  .

# Multi-architecture build
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t anode:multiarch \
  -f deploy/docker/Dockerfile \
  .
```

### Docker Swarm Deployment

For multi-host deployments:

```bash
# Initialize swarm
docker swarm init

# Deploy stack
docker stack deploy -c docker-compose.yml anode

# List services
docker service ls

# Scale service
docker service scale anode_anode-2=5

# Remove stack
docker stack rm anode
```

### Using Docker Secrets

Secure credential management:

```bash
# Create secrets
echo "your-access-key" | docker secret create anode_access_key -
echo "your-secret-key" | docker secret create anode_secret_key -

# Use in docker-compose.yml
services:
  anode:
    secrets:
      - anode_access_key
      - anode_secret_key
    environment:
      - ANODE_DEFAULT_ACCESS_KEY_FILE=/run/secrets/anode_access_key
      - ANODE_DEFAULT_SECRET_KEY_FILE=/run/secrets/anode_secret_key

secrets:
  anode_access_key:
    external: true
  anode_secret_key:
    external: true
```

### Integration with CI/CD

**GitHub Actions example:**

```yaml
name: Deploy Anode
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Deploy to production
        run: |
          docker compose -f deploy/docker/docker-compose.yml pull
          docker compose -f deploy/docker/docker-compose.yml up -d
          docker compose -f deploy/docker/docker-compose.yml ps
```

### High Availability Setup

```yaml
# Use overlay network for multi-host
networks:
  anode-net:
    driver: overlay
    attachable: true

# Deploy across multiple hosts with constraints
services:
  anode-1:
    deploy:
      placement:
        constraints:
          - node.hostname == host1

  anode-2:
    deploy:
      placement:
        constraints:
          - node.hostname == host2

  anode-3:
    deploy:
      placement:
        constraints:
          - node.hostname == host3
```

## Additional Resources

- [Standalone Deployment Guide](./deployment-standalone.md) - Non-Docker deployments
- [Operations Guide](./operations.md) - Production operations
- [Monitoring Guide](./monitoring.md) - Detailed monitoring setup
- [Security Guide](./security.md) - Security best practices
- [Docker Documentation](https://docs.docker.com/) - Official Docker docs
- [Docker Compose Reference](https://docs.docker.com/compose/compose-file/) - Compose file format

## Getting Help

If you encounter issues:

1. Check logs: `docker compose logs -f`
2. Review this troubleshooting guide
3. Search existing issues: https://github.com/christopherpaika/anode/issues
4. Join discussions: https://github.com/christopherpaika/anode/discussions
5. Open a new issue with:
   - Docker version: `docker --version`
   - Compose version: `docker compose version`
   - OS and version
   - Full error logs
   - Steps to reproduce

For security issues, email security@anode.io instead of opening a public issue.
