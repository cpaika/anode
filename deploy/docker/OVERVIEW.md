# Anode Docker Configuration - Overview

This document provides a comprehensive overview of the Docker configuration created for the Anode project.

## File Structure

```
deploy/docker/
├── Dockerfile                          # Multi-stage production build
├── .dockerignore                       # Build context exclusions
├── docker-compose.yml                  # 3-node development cluster
├── docker-compose.dev.yml              # Extended dev environment
├── docker-compose.chaos.yml            # 5-node chaos testing
├── README.md                           # Comprehensive documentation
├── OVERVIEW.md                         # This file
├── config/
│   ├── anode.toml                      # Main configuration file
│   ├── prometheus.yml                  # Prometheus scrape config
│   ├── prometheus-chaos.yml            # Chaos testing metrics
│   ├── nginx.conf                      # Load balancer config
│   ├── toxiproxy.json                  # Network chaos proxies
│   └── grafana/
│       ├── provisioning/
│       │   ├── datasources/
│       │   │   └── prometheus.yml      # Grafana datasource
│       │   └── dashboards/
│       │       └── dashboards.yml      # Dashboard provider
│       └── dashboards/
│           └── .gitkeep                # Dashboard JSON files
└── scripts/
    ├── entrypoint.sh                   # Container entrypoint
    ├── health-check.sh                 # Health check script
    ├── start-cluster.sh                # Cluster startup
    ├── stop-cluster.sh                 # Cluster shutdown
    └── run-chaos-tests.sh              # Chaos test runner
```

## Key Features

### 1. Multi-Stage Dockerfile

**Optimizations:**
- Separate builder and runtime stages for minimal image size
- Dependency caching layer to speed up rebuilds
- Static linking with stripped binary
- Non-root user (anode:anode, UID/GID 1000)
- Multi-architecture support ready (amd64/arm64)

**Size Comparison:**
- Builder stage: ~2.5GB (includes Rust toolchain)
- Runtime stage: ~150MB (debian:bookworm-slim + binary)

**Security:**
- Non-root user execution
- Minimal runtime dependencies
- Health checks built-in
- Proper signal handling

### 2. Docker Compose Configurations

#### Basic (docker-compose.yml)
- 3-node cluster for development
- Individual volumes per node
- Health checks on all nodes
- Resource limits configured
- Proper startup dependencies

**Ports:**
- Node 1: 8080 (S3), 8081 (Admin), 9090 (gRPC)
- Node 2: 8082 (S3), 8083 (Admin), 9091 (gRPC)
- Node 3: 8084 (S3), 8085 (Admin), 9092 (gRPC)

#### Development (docker-compose.dev.yml)
Extends basic config with:
- Prometheus for metrics collection
- Grafana for visualization
- Jaeger for distributed tracing
- nginx for load balancing
- MinIO client for testing
- Debug logging enabled
- Source code mounting for hot reload

**Additional Ports:**
- Prometheus: 9091
- Grafana: 3000
- Jaeger UI: 16686
- nginx: 80

#### Chaos Testing (docker-compose.chaos.yml)
- 5-node cluster for resilience testing
- Toxiproxy for network fault injection
- Pumba for container chaos
- Automated test runner
- Results volume for test outputs
- Separate network namespace

**Chaos Capabilities:**
- Network partitions
- Latency injection (configurable)
- Packet loss simulation
- Bandwidth limiting
- Node crashes/pauses
- Combined failure scenarios

### 3. Configuration Management

#### anode.toml
Comprehensive configuration covering:
- Node identity and networking
- Cluster formation and Raft settings
- Storage engine parameters
- Replication configuration
- S3 API settings
- Authentication and security
- Parquet-specific features
- Logging and monitoring
- Performance tuning
- Debug options

**Configuration Priority:**
1. Command-line arguments (highest)
2. Environment variables
3. Configuration file
4. Compiled defaults (lowest)

### 4. Monitoring Stack

#### Prometheus
- Scrapes all nodes every 15s
- 7-day retention
- Custom metrics for Anode
- Ready for alerting rules

**Key Metrics:**
- Storage: objects, bytes, chunks
- Raft: leader changes, log entries
- HTTP/gRPC: requests, latency, errors
- System: CPU, memory, disk I/O

#### Grafana
- Pre-configured Prometheus datasource
- Dashboard provisioning ready
- Custom Anode dashboards (to be added)

**Suggested Dashboards:**
- Cluster overview
- Storage metrics
- Raft consensus health
- S3 API performance
- Node resource usage

#### Jaeger
- Distributed tracing for requests
- Shows cross-node communication
- Performance bottleneck detection
- Request flow visualization

### 5. Scripts

#### start-cluster.sh
**Features:**
- Configurable cluster size
- Dev mode option
- Build option
- Clean start option
- Health check waiting
- Helpful output with endpoints

**Usage Examples:**
```bash
# Basic start
./scripts/start-cluster.sh

# With monitoring
./scripts/start-cluster.sh --dev

# Clean rebuild
./scripts/start-cluster.sh --clean --build

# 5-node cluster
./scripts/start-cluster.sh --nodes 5
```

#### stop-cluster.sh
**Features:**
- Graceful shutdown
- Optional volume removal
- Optional image removal
- Resource cleanup verification

**Usage Examples:**
```bash
# Stop containers only
./scripts/stop-cluster.sh

# Remove data
./scripts/stop-cluster.sh --volumes

# Complete cleanup
./scripts/stop-cluster.sh --all
```

#### run-chaos-tests.sh
**Scenarios:**
- Network partition (split-brain)
- Latency injection (500ms + jitter)
- Packet loss (30%)
- Node crashes (kill + restart)
- Bandwidth limiting (1MB/s)
- Combined failures

**Test Flow:**
1. Wait for cluster health
2. Run scenarios in sequence
3. Verify consistency after each
4. Repeat for duration
5. Final health check
6. Write results

### 6. Network Architecture

#### Development Network
- Subnet: 172.28.0.0/16
- Bridge mode
- DNS resolution between containers
- External port mapping

#### Chaos Network
- Subnet: 172.29.0.0/16
- Isolated from dev network
- Toxiproxy intercepts traffic
- Pumba manipulates containers

### 7. Volume Management

**Persistent Volumes:**
- anode-1-data, anode-2-data, anode-3-data
- Separate volumes prevent cross-contamination
- Survives container restarts
- Supports backup/restore

**Volume Contents:**
```
/data/
├── blobs/          # Content-addressed chunks
├── metadata/       # redb database files
└── parquet-cache/  # Cached Parquet metadata
```

### 8. Security Considerations

**Current (Development):**
- Default credentials (minioadmin/minioadmin)
- No TLS
- No encryption at rest
- Internal network only

**Production Recommendations:**
- Change default credentials
- Enable TLS for all connections
- Use proper secret management
- Enable encryption at rest
- Network policies/firewall rules
- Regular security updates
- Audit logging

### 9. Performance Tuning

**Resource Limits (per node):**
- Memory: 2GB limit, 512MB reserved
- CPU: 2.0 cores limit, 0.5 reserved

**Adjustable Parameters:**
```toml
[performance]
worker_threads = 0              # CPU cores
blocking_threads = 512
tcp_nodelay = true
tcp_send_buffer = 0
tcp_recv_buffer = 0
listen_backlog = 1024
```

**Storage Optimization:**
```toml
[storage]
chunk_size = 4194304           # 4MB
fsync_enabled = true
direct_io = false
mmap_reads = true
blob_cache_size = 1073741824   # 1GB
```

### 10. Testing Strategy

#### Unit Testing
```bash
docker-compose exec anode-1 cargo test
```

#### Integration Testing
```bash
# Start cluster
./scripts/start-cluster.sh

# Run S3 compatibility tests
aws s3api ... --endpoint-url http://localhost:8080

# Stop cluster
./scripts/stop-cluster.sh
```

#### Chaos Testing
```bash
# Start chaos environment
docker-compose -f docker-compose.chaos.yml up -d

# Tests run automatically
# Check results in chaos-results volume
```

#### Load Testing
```bash
# Use s3-benchmark or similar
s3-benchmark \
  -endpoint=http://localhost:80 \
  -accessKey=minioadmin \
  -secretKey=minioadmin \
  -bucket=benchmark \
  -numClients=100 \
  -objectSize=1MB
```

## Quick Reference

### Essential Commands

```bash
# Start development cluster
./scripts/start-cluster.sh --dev

# Check cluster health
curl http://localhost:8081/health
curl http://localhost:8083/health
curl http://localhost:8085/health

# View metrics
curl http://localhost:8081/metrics

# Watch logs
docker-compose logs -f

# Access Grafana
open http://localhost:3000  # admin/admin

# Stop and cleanup
./scripts/stop-cluster.sh --volumes
```

### AWS CLI S3 Operations

```bash
# Configure
aws configure set aws_access_key_id minioadmin
aws configure set aws_secret_access_key minioadmin

# Create bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://test

# Upload
aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://test/

# List
aws --endpoint-url http://localhost:8080 s3 ls s3://test/

# Download
aws --endpoint-url http://localhost:8080 s3 cp s3://test/file.txt .

# Delete
aws --endpoint-url http://localhost:8080 s3 rm s3://test/file.txt
```

### Chaos Testing

```bash
# Start chaos cluster
docker-compose -f docker-compose.chaos.yml up -d

# Inject latency manually
curl -X POST http://localhost:8474/proxies/anode-1-proxy/toxics \
  -d '{"name":"latency","type":"latency","attributes":{"latency":1000}}'

# Remove toxic
curl -X DELETE http://localhost:8474/proxies/anode-1-proxy/toxics/latency

# Stop chaos cluster
docker-compose -f docker-compose.chaos.yml down
```

## Next Steps

1. **Implement Actual Binary**
   - Complete the Rust implementation
   - Build and test with Docker

2. **Add Grafana Dashboards**
   - Create cluster overview dashboard
   - Add storage metrics dashboard
   - Add Raft health dashboard

3. **Enhance Chaos Tests**
   - Add data consistency verification
   - Implement automated recovery testing
   - Add performance degradation metrics

4. **Production Hardening**
   - Add TLS certificates
   - Implement proper secret management
   - Add security scanning to CI/CD
   - Create production deployment guide

5. **Documentation**
   - Add troubleshooting guides
   - Create architecture diagrams
   - Document performance tuning
   - Write operational runbooks

## Support

For questions or issues:
- See README.md for detailed documentation
- Check logs: `docker-compose logs`
- Verify health: `curl http://localhost:8081/health`
- Monitor metrics: http://localhost:3000 (Grafana)
