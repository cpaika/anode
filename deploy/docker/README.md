# Anode Docker Deployment

This directory contains Docker configurations for running Anode Object Storage in various environments.

## Contents

- **Dockerfile** - Multi-stage production-optimized build
- **docker-compose.yml** - Basic 3-node development cluster
- **docker-compose.dev.yml** - Extended development environment with monitoring
- **docker-compose.chaos.yml** - 5-node chaos testing environment
- **config/** - Configuration files and examples
- **scripts/** - Utility scripts for cluster management

## Quick Start

### Basic 3-Node Cluster

```bash
# Start the cluster
./scripts/start-cluster.sh

# Stop the cluster
./scripts/stop-cluster.sh
```

### Development Environment with Monitoring

```bash
# Start with Prometheus, Grafana, and Jaeger
./scripts/start-cluster.sh --dev

# Access monitoring interfaces:
# - Grafana: http://localhost:3000 (admin/admin)
# - Prometheus: http://localhost:9091
# - Jaeger: http://localhost:16686
```

### Chaos Testing Environment

```bash
# Start 5-node cluster with fault injection
docker-compose -f docker-compose.chaos.yml up -d

# Inject network latency
curl -X POST http://localhost:8474/proxies/anode-1-proxy/toxics \
  -d '{"name":"latency","type":"latency","attributes":{"latency":1000}}'

# Inject packet loss
curl -X POST http://localhost:8474/proxies/anode-2-proxy/toxics \
  -d '{"name":"packet_loss","type":"timeout","attributes":{"timeout":10000}}'
```

## Building Images

### Production Build

```bash
docker build -t anode:latest -f Dockerfile ../..
```

### Development Build with Cache

```bash
./scripts/start-cluster.sh --build
```

### Multi-Architecture Build

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t anode:latest \
  -f Dockerfile \
  ../..
```

## Configuration

### Environment Variables

All configuration can be overridden with environment variables:

```bash
# Node configuration
ANODE_NODE_ID=1
ANODE_NODE_NAME=anode-1
ANODE_DATA_DIR=/data

# Network configuration
ANODE_BIND_ADDR=0.0.0.0
ANODE_ADVERTISE_ADDR=anode-1
ANODE_HTTP_PORT=8080
ANODE_ADMIN_PORT=8081
ANODE_GRPC_PORT=9090

# Cluster configuration
ANODE_BOOTSTRAP=true
ANODE_JOIN_ADDR=anode-1:9090
ANODE_CLUSTER_PEERS=anode-1:9090,anode-2:9090,anode-3:9090

# Logging
ANODE_LOG_LEVEL=info
RUST_LOG=anode=debug,openraft=debug
RUST_BACKTRACE=1
```

### Configuration File

Edit `config/anode.toml` to customize all aspects of the cluster:

```toml
[node]
id = 1
name = "anode-1"
data_dir = "/data"

[network]
bind_addr = "0.0.0.0"
http_port = 8080
admin_port = 8081
grpc_port = 9090

[storage]
chunk_size = 4194304  # 4MB
fsync_enabled = true
blob_cache_size = 1073741824  # 1GB

[replication]
default_replication_factor = 3
num_placement_groups = 128
```

See `config/anode.toml` for complete documentation of all options.

## Using the Cluster

### AWS CLI

Configure the AWS CLI to use Anode:

```bash
# Configure credentials
aws configure set aws_access_key_id minioadmin
aws configure set aws_secret_access_key minioadmin

# Create a bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket

# Upload a file
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://my-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/

# Download a file
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/test.txt downloaded.txt

# Delete an object
aws --endpoint-url http://localhost:8080 s3 rm s3://my-bucket/test.txt
```

### MinIO Client (mc)

```bash
# Configure alias
mc alias set anode http://localhost:8080 minioadmin minioadmin

# Create bucket
mc mb anode/my-bucket

# Upload file
mc cp test.txt anode/my-bucket/

# List buckets
mc ls anode

# Get stats
mc stat anode/my-bucket/test.txt
```

### cURL

```bash
# Health check
curl http://localhost:8081/health

# Metrics
curl http://localhost:8081/metrics

# S3 API (requires proper AWS Signature V4)
curl -X PUT http://localhost:8080/my-bucket \
  -H "Host: localhost:8080" \
  -H "Authorization: AWS4-HMAC-SHA256 ..."
```

## Monitoring

### Health Checks

Each node exposes health check endpoints:

```bash
# Overall health
curl http://localhost:8081/health

# Readiness check (can serve traffic)
curl http://localhost:8081/ready

# Liveness check (process is alive)
curl http://localhost:8081/live
```

### Metrics

Prometheus metrics are exposed at `/metrics`:

```bash
curl http://localhost:8081/metrics
```

Key metrics:
- `anode_storage_objects_total` - Total number of objects
- `anode_storage_bytes_total` - Total storage used in bytes
- `anode_raft_leader_changes_total` - Number of leader elections
- `anode_http_requests_total` - Total HTTP requests
- `anode_grpc_requests_total` - Total gRPC requests

### Logs

```bash
# View all logs
docker-compose logs

# Follow logs for specific node
docker-compose logs -f anode-1

# View last 100 lines
docker-compose logs --tail=100 anode-1
```

## Troubleshooting

### Container won't start

```bash
# Check logs
docker-compose logs anode-1

# Check container status
docker-compose ps

# Restart container
docker-compose restart anode-1
```

### Cluster won't form

```bash
# Check if nodes can communicate
docker exec anode-1 nc -zv anode-2 9090

# Check Raft status (requires admin API)
curl http://localhost:8081/admin/raft/status

# Check cluster membership
curl http://localhost:8081/admin/cluster/members
```

### Performance issues

```bash
# Check resource usage
docker stats

# Check disk I/O
docker exec anode-1 iostat -x 1

# Enable debug logging
docker-compose exec anode-1 kill -USR1 $(pidof anode)
```

### Data corruption

```bash
# Stop the cluster
docker-compose down

# Backup data
docker run --rm -v anode-1-data:/data -v $(pwd):/backup \
  ubuntu tar czf /backup/anode-1-backup.tar.gz /data

# Remove corrupted data
docker volume rm anode-1-data

# Restart and restore
docker-compose up -d
```

## Advanced Usage

### Custom Network Configuration

```bash
# Create custom network
docker network create --subnet=172.30.0.0/16 anode-custom

# Update docker-compose.yml to use custom network
```

### Volume Management

```bash
# List volumes
docker volume ls | grep anode

# Inspect volume
docker volume inspect anode-1-data

# Backup volume
docker run --rm \
  -v anode-1-data:/data \
  -v $(pwd):/backup \
  ubuntu tar czf /backup/backup.tar.gz /data

# Restore volume
docker run --rm \
  -v anode-1-data:/data \
  -v $(pwd):/backup \
  ubuntu tar xzf /backup/backup.tar.gz -C /
```

### Resource Limits

Edit `docker-compose.yml` to adjust resource limits:

```yaml
deploy:
  resources:
    limits:
      memory: 4G
      cpus: '4.0'
    reservations:
      memory: 1G
      cpus: '1.0'
```

### Scaling

```bash
# Scale to 5 nodes
docker-compose up -d --scale anode-2=5

# Or create additional node configurations in docker-compose.yml
```

## Security

### Production Considerations

1. **Change default credentials** in `config/anode.toml`:
   ```toml
   [auth]
   default_access_key = "your-access-key"
   default_secret_key = "your-secret-key"
   ```

2. **Enable TLS**:
   ```toml
   [security]
   tls_enabled = true
   tls_cert_file = "/config/certs/server.crt"
   tls_key_file = "/config/certs/server.key"
   ```

3. **Use secrets management**:
   ```bash
   docker secret create anode_secret_key /path/to/secret
   ```

4. **Enable encryption at rest**:
   ```toml
   [security]
   encryption_at_rest = true
   encryption_key_file = "/config/keys/encryption.key"
   ```

### Network Security

```bash
# Restrict network access
docker network create --internal anode-internal

# Use firewall rules
iptables -A INPUT -p tcp --dport 8080 -s 10.0.0.0/8 -j ACCEPT
iptables -A INPUT -p tcp --dport 8080 -j DROP
```

## Maintenance

### Upgrades

```bash
# Graceful rolling upgrade
for node in anode-1 anode-2 anode-3; do
  docker-compose stop $node
  docker-compose pull $node
  docker-compose up -d $node
  sleep 30  # Wait for node to rejoin
done
```

### Backups

```bash
# Automated backup script
./scripts/backup-cluster.sh --output /backups/$(date +%Y%m%d)
```

### Cleanup

```bash
# Remove stopped containers
docker-compose down

# Remove volumes (WARNING: deletes all data)
./scripts/stop-cluster.sh --volumes

# Remove everything
./scripts/stop-cluster.sh --all
```

## Development

### Running Tests

```bash
# Cluster health test (recommended for validating deployment)
./scripts/test-cluster-health.sh

# Unit tests
docker-compose exec anode-1 cargo test

# Integration tests
docker-compose exec anode-1 cargo test --test integration

# Chaos tests
docker-compose -f docker-compose.chaos.yml up
```

The cluster health test (`test-cluster-health.sh`) performs a comprehensive validation:
- Builds Docker images
- Starts a 3-node cluster
- Waits for all nodes to become healthy
- Verifies health, liveness, and readiness endpoints
- Tests basic S3 operations (create bucket, upload, download, delete)
- Automatically cleans up resources

### Debugging

```bash
# Attach debugger
docker-compose exec anode-1 gdb /usr/local/bin/anode $(pidof anode)

# Enable profiling
docker-compose exec anode-1 curl http://localhost:8081/debug/pprof/profile?seconds=30

# Memory profiling
docker-compose exec anode-1 curl http://localhost:8081/debug/pprof/heap
```

## Performance Tuning

### Kernel Parameters

```bash
# Increase file descriptors
ulimit -n 65536

# Optimize network
sysctl -w net.core.somaxconn=4096
sysctl -w net.ipv4.tcp_max_syn_backlog=4096
```

### Storage Optimization

```bash
# Use SSD with proper scheduler
echo deadline > /sys/block/sda/queue/scheduler

# Disable disk cache for consistency
sync && echo 1 > /proc/sys/vm/drop_caches
```

## Support

For issues, questions, or contributions:
- GitHub Issues: https://github.com/your-org/anode/issues
- Documentation: https://docs.anode.io
- Discord: https://discord.gg/anode

## License

See LICENSE file in the project root.
