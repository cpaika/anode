# Anode Docker Configuration - File Index

## Quick Navigation

| Category | File | Size | Description |
|----------|------|------|-------------|
| **Core Docker** |
| | [Dockerfile](./Dockerfile) | 3.6K | Multi-stage production build |
| | [.dockerignore](./.dockerignore) | 1.1K | Build context exclusions |
| **Compose Files** |
| | [docker-compose.yml](./docker-compose.yml) | 4.5K | 3-node development cluster |
| | [docker-compose.dev.yml](./docker-compose.dev.yml) | 5.8K | Development with monitoring |
| | [docker-compose.chaos.yml](./docker-compose.chaos.yml) | 8.0K | 5-node chaos testing |
| **Configuration** |
| | [.env.example](./.env.example) | 4.4K | Environment variables template |
| | [config/anode.toml](./config/anode.toml) | 10K | Main configuration file |
| | [config/prometheus.yml](./config/prometheus.yml) | 1.2K | Prometheus scrape config |
| | [config/prometheus-chaos.yml](./config/prometheus-chaos.yml) | 941B | Chaos testing metrics |
| | [config/nginx.conf](./config/nginx.conf) | 3.5K | Load balancer config |
| | [config/toxiproxy.json](./config/toxiproxy.json) | 633B | Network chaos proxies |
| **Grafana** |
| | [config/grafana/provisioning/datasources/prometheus.yml](./config/grafana/provisioning/datasources/prometheus.yml) | 220B | Datasource config |
| | [config/grafana/provisioning/dashboards/dashboards.yml](./config/grafana/provisioning/dashboards/dashboards.yml) | 276B | Dashboard provider |
| | [config/grafana/dashboards/.gitkeep](./config/grafana/dashboards/.gitkeep) | 81B | Dashboard directory |
| **Scripts** |
| | [scripts/entrypoint.sh](./scripts/entrypoint.sh) | 2.6K | Container entrypoint |
| | [scripts/health-check.sh](./scripts/health-check.sh) | 886B | Health verification |
| | [scripts/start-cluster.sh](./scripts/start-cluster.sh) | 5.8K | Cluster startup |
| | [scripts/stop-cluster.sh](./scripts/stop-cluster.sh) | 3.8K | Cluster shutdown |
| | [scripts/run-chaos-tests.sh](./scripts/run-chaos-tests.sh) | 11K | Chaos test runner |
| **Documentation** |
| | [README.md](./README.md) | 9.2K | User guide |
| | [OVERVIEW.md](./OVERVIEW.md) | 10K | Technical overview |
| | [DEPLOYMENT_SUMMARY.md](./DEPLOYMENT_SUMMARY.md) | 11K | Complete summary |
| | [INDEX.md](./INDEX.md) | - | This file |

**Total Files**: 22
**Total Size**: ~97KB

## File Purposes

### Dockerfile
Multi-stage build optimized for:
- Fast rebuilds with dependency caching
- Minimal runtime image (~150MB)
- Non-root user execution
- Proper signal handling
- Health checks

### docker-compose.yml
Basic 3-node cluster featuring:
- Individual persistent volumes
- Health checks
- Resource limits
- Proper startup ordering

### docker-compose.dev.yml
Extended development environment with:
- Prometheus (metrics)
- Grafana (visualization)
- Jaeger (tracing)
- nginx (load balancing)
- Debug logging

### docker-compose.chaos.yml
Chaos testing setup including:
- 5-node cluster
- Toxiproxy (network faults)
- Pumba (container chaos)
- Automated testing

### config/anode.toml
Complete configuration with 80+ options:
- Node configuration
- Network settings
- Cluster formation
- Storage engine
- Replication
- S3 API
- Authentication
- Parquet awareness
- Logging & metrics
- Performance tuning
- Security

### scripts/start-cluster.sh
Automated cluster startup:
- Configurable size
- Development mode
- Build option
- Clean start
- Health checking
- Helpful output

### scripts/stop-cluster.sh
Graceful shutdown:
- Stop containers
- Optional volume removal
- Optional image removal
- Resource verification

### scripts/run-chaos-tests.sh
Automated chaos testing:
- Network partitions
- Latency injection
- Packet loss
- Node crashes
- Bandwidth limiting
- Consistency checks

## Usage Patterns

### Development Workflow
```bash
# Initial setup
cd /Users/christopherpaika/projects/anode/deploy/docker
cp .env.example .env
# Edit .env as needed

# Start cluster
./scripts/start-cluster.sh --dev

# Test S3 API
aws --endpoint-url http://localhost:8080 s3 mb s3://test

# View metrics
open http://localhost:3000  # Grafana

# Stop cluster
./scripts/stop-cluster.sh
```

### Chaos Testing Workflow
```bash
# Start chaos environment
docker-compose -f docker-compose.chaos.yml up -d

# Monitor test execution
docker-compose -f docker-compose.chaos.yml logs -f chaos-runner

# Check results
docker run --rm -v chaos-results:/results ubuntu cat /results/test-result.txt

# Stop chaos environment
docker-compose -f docker-compose.chaos.yml down
```

### Production Deployment Workflow
```bash
# Build production image
docker build -t anode:v1.0.0 -f Dockerfile ../..

# Tag for registry
docker tag anode:v1.0.0 registry.example.com/anode:v1.0.0

# Push to registry
docker push registry.example.com/anode:v1.0.0

# Deploy with custom config
docker-compose -f docker-compose.yml up -d
```

## Configuration Hierarchy

1. **Command-line arguments** (highest priority)
2. **Environment variables** (.env file)
3. **Configuration file** (config/anode.toml)
4. **Compiled defaults** (lowest priority)

## Port Mappings

### Basic Cluster (docker-compose.yml)
| Node | S3 API | Admin API | gRPC |
|------|--------|-----------|------|
| anode-1 | 8080 | 8081 | 9090 |
| anode-2 | 8082 | 8083 | 9091 |
| anode-3 | 8084 | 8085 | 9092 |

### Development Stack (docker-compose.dev.yml)
| Service | Port | Purpose |
|---------|------|---------|
| nginx | 80 | Load balancer |
| Grafana | 3000 | Visualization |
| Prometheus | 9091 | Metrics |
| Jaeger | 16686 | Tracing UI |
| Jaeger | 14268 | Collector |
| Jaeger | 6831 | Agent UDP |

### Chaos Stack (docker-compose.chaos.yml)
| Service | Port | Purpose |
|---------|------|---------|
| Toxiproxy | 8474 | API |
| Toxiproxy | 9100-9104 | Node proxies |
| Prometheus | 9090 | Metrics |
| Grafana | 3000 | Visualization |

## Volume Mappings

### Development Cluster
- anode-1-data → /data (node 1 storage)
- anode-2-data → /data (node 2 storage)
- anode-3-data → /data (node 3 storage)
- prometheus-data → /prometheus
- grafana-data → /var/lib/grafana

### Chaos Cluster
- chaos-anode-1-data through chaos-anode-5-data
- chaos-results → /results (test outputs)
- chaos-prometheus-data → /prometheus
- chaos-grafana-data → /var/lib/grafana

## Environment Variables

Key variables (see .env.example for full list):

### Node Identity
- ANODE_NODE_ID
- ANODE_NODE_NAME
- ANODE_ADVERTISE_ADDR

### Ports
- ANODE_HTTP_PORT
- ANODE_ADMIN_PORT
- ANODE_GRPC_PORT

### Cluster
- ANODE_BOOTSTRAP
- ANODE_JOIN_ADDR
- ANODE_CLUSTER_PEERS

### Logging
- ANODE_LOG_LEVEL
- RUST_LOG
- RUST_BACKTRACE

### Monitoring
- ANODE_ENABLE_METRICS
- ANODE_ENABLE_TRACING
- ANODE_JAEGER_ENDPOINT

## Monitoring Endpoints

### Health Checks
- GET /health - Overall health status
- GET /ready - Readiness for traffic
- GET /live - Liveness check

### Metrics
- GET /metrics - Prometheus format metrics

### Admin (Future)
- GET /admin/raft/status - Raft state
- GET /admin/cluster/members - Cluster membership
- POST /admin/cluster/add - Add node
- DELETE /admin/cluster/remove - Remove node

## Security Considerations

### Development (Current)
- Default credentials
- No TLS
- No encryption at rest
- Internal network only

### Production (Required)
- Change credentials in config/anode.toml
- Enable TLS (tls_enabled = true)
- Use certificate files
- Enable encryption at rest
- Use secrets management (Docker secrets or Kubernetes secrets)
- Network policies
- Regular security updates

## Troubleshooting

### Common Issues

**Cluster won't start**
```bash
# Check logs
docker-compose logs

# Check container status
docker-compose ps

# Verify network
docker network inspect anode-net
```

**Node won't join cluster**
```bash
# Check connectivity
docker exec anode-2 nc -zv anode-1 9090

# Check logs
docker-compose logs anode-2

# Verify environment
docker exec anode-2 env | grep ANODE
```

**Health check failing**
```bash
# Manual health check
curl -v http://localhost:8081/health

# Check container logs
docker-compose logs anode-1

# Check resource usage
docker stats
```

**Performance issues**
```bash
# Check resource limits
docker inspect anode-1 | grep -A 10 Resources

# Monitor I/O
docker stats --no-stream

# Check disk space
docker exec anode-1 df -h
```

## Performance Tuning

### Resource Limits
Edit docker-compose.yml:
```yaml
deploy:
  resources:
    limits:
      memory: 4G
      cpus: '4.0'
```

### Storage Configuration
Edit config/anode.toml:
```toml
[storage]
chunk_size = 4194304         # 4MB chunks
blob_cache_size = 2147483648 # 2GB cache
fsync_enabled = false        # Faster but less durable
```

### Network Configuration
Edit config/anode.toml:
```toml
[performance]
worker_threads = 8           # More threads
tcp_nodelay = true          # Lower latency
listen_backlog = 2048       # More connections
```

## Development Tips

### Fast Iteration
```bash
# Use dev mode with source mounting
./scripts/start-cluster.sh --dev

# Make changes to Rust code
# Rebuild in container
docker-compose exec anode-1 cargo build --release

# Restart node
docker-compose restart anode-1
```

### Debugging
```bash
# Enable debug logging
docker-compose exec anode-1 kill -USR1 $(pidof anode)

# View detailed logs
docker-compose logs -f --tail=100 anode-1

# Access container shell
docker-compose exec anode-1 /bin/sh

# Check process
docker-compose exec anode-1 ps aux

# Check network
docker-compose exec anode-1 netstat -tuln
```

### Testing
```bash
# Unit tests
docker-compose exec anode-1 cargo test

# Integration tests
docker-compose exec anode-1 cargo test --test integration

# Benchmarks
docker-compose exec anode-1 cargo bench
```

## Next Steps

1. **Implement Rust code** in crates/anode
2. **Test Docker build** - Verify Dockerfile works
3. **Create Grafana dashboards** - Add to config/grafana/dashboards/
4. **Add Prometheus alerts** - Create alerting rules
5. **Write backup scripts** - Automate data backup
6. **Create Helm charts** - Kubernetes deployment (Phase 13)
7. **Set up CI/CD** - GitHub Actions (Phase 15)

## Support & Resources

- **README.md** - User guide with examples
- **OVERVIEW.md** - Technical details and architecture
- **DEPLOYMENT_SUMMARY.md** - Complete feature summary
- **config/anode.toml** - Inline documentation for all options

## Version

- **Created**: 2025-12-19
- **Docker Compose Version**: 3.9
- **Rust Version**: 1.75
- **Base Image**: debian:bookworm-slim

## Status

✅ **COMPLETE** - Ready for Phase 12 implementation
