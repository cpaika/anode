# Operations Guide

Production operations guide for Anode clusters.

## Table of Contents

- [Cluster Setup](#cluster-setup)
- [Monitoring](#monitoring)
- [Backup and Restore](#backup-and-restore)
- [Scaling](#scaling)
- [Upgrades](#upgrades)
- [Troubleshooting](#troubleshooting)
- [Disaster Recovery](#disaster-recovery)
- [Performance Tuning](#performance-tuning)

## Cluster Setup

### Initial Deployment

#### 1. Plan Your Cluster

**Node Count:**
- Minimum: 3 nodes (2f+1 for f=1 failure tolerance)
- Recommended: 5 nodes (tolerates 2 failures)
- Maximum: 50 nodes (current tested limit)

**Hardware Recommendations:**
```
Small Cluster (< 10TB):
- CPU: 8 cores
- RAM: 16GB
- Storage: 2TB NVMe SSD

Medium Cluster (10-100TB):
- CPU: 16 cores
- RAM: 32GB
- Storage: 10TB NVMe SSD

Large Cluster (> 100TB):
- CPU: 32 cores
- RAM: 64GB
- Storage: 20TB+ NVMe SSD
```

**Network:**
- 10 Gbps minimum between nodes
- Low latency (< 1ms) preferred
- Dedicated network for inter-node traffic

#### 2. Bootstrap Cluster

**Node 1 (Bootstrap):**
```bash
anode \
    --node-id 1 \
    --s3-addr 0.0.0.0:8080 \
    --grpc-addr 0.0.0.0:9090 \
    --admin-addr 0.0.0.0:8081 \
    --data-dir /data \
    --config /etc/anode/config.toml \
    --bootstrap
```

**Wait for node 1 to be ready:**
```bash
curl http://node1:8081/health
```

**Nodes 2-N (Join):**
```bash
anode \
    --node-id 2 \
    --s3-addr 0.0.0.0:8080 \
    --grpc-addr 0.0.0.0:9090 \
    --admin-addr 0.0.0.0:8081 \
    --data-dir /data \
    --config /etc/anode/config.toml \
    --join node1:9090
```

#### 3. Verify Cluster

```bash
# Check cluster status
curl http://node1:8081/admin/cluster

# Verify all nodes are members
curl http://node1:8081/admin/members

# Test S3 API
aws --endpoint-url http://node1:8080 s3 mb s3://test-bucket
aws --endpoint-url http://node1:8080 s3 ls
```

## Monitoring

### Prometheus Metrics

Anode exposes Prometheus metrics at `:8081/metrics`.

**Key Metrics to Monitor:**

#### Request Metrics
```
# Total requests
anode_requests_total{method="GET|PUT|DELETE|HEAD"}

# Request duration (seconds)
anode_request_duration_seconds{method, quantile="0.5|0.95|0.99"}

# Requests in flight
anode_requests_in_flight

# Request errors
anode_request_errors_total{error_type}
```

#### Storage Metrics
```
# Total storage used (bytes)
anode_storage_bytes_total

# Total objects
anode_objects_total

# Total chunks
anode_chunks_total

# Metadata cache hit rate
anode_metadata_cache_hit_ratio
```

#### Raft Metrics
```
# Raft role (1 if leader, 0 if follower)
anode_raft_leader

# Current term
anode_raft_term

# Log size
anode_raft_log_size

# Committed index
anode_raft_committed_index

# Replication lag (follower)
anode_raft_replication_lag_seconds
```

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'anode'
    static_configs:
      - targets:
        - 'node1:8081'
        - 'node2:8081'
        - 'node3:8081'
    scrape_interval: 15s
```

### Alerting Rules

```yaml
# alerts.yml
groups:
  - name: anode
    rules:
      # Node down
      - alert: AnodeNodeDown
        expr: up{job="anode"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Anode node {{ $labels.instance }} is down"

      # High latency
      - alert: AnodeHighLatency
        expr: histogram_quantile(0.95, anode_request_duration_seconds) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Anode P95 latency > 1s"

      # Storage usage
      - alert: AnodeStorageHigh
        expr: (anode_storage_bytes_total / anode_storage_capacity_bytes) > 0.8
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Anode storage > 80% full"

      # Raft no leader
      - alert: AnodeNoLeader
        expr: max(anode_raft_leader) by (pg) == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Placement group {{ $labels.pg }} has no leader"
```

### Grafana Dashboard

Import the included Grafana dashboard:

1. Go to Grafana → Dashboards → Import
2. Upload `deploy/grafana/anode-dashboard.json`
3. Select your Prometheus data source
4. Click Import

**Dashboard Panels:**
- Request rate and latency
- Storage usage and growth
- Raft leader status
- Node health
- Error rates
- Cache hit rates

### Health Checks

#### Liveness Probe

```bash
curl http://localhost:8081/health/live
```

Returns 200 if process is alive, regardless of cluster state.

#### Readiness Probe

```bash
curl http://localhost:8081/health/ready
```

Returns 200 only if node is ready to accept traffic:
- Storage is accessible
- Raft is connected
- Sufficient disk space

**Kubernetes Probes:**
```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8081
  initialDelaySeconds: 30
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8081
  initialDelaySeconds: 10
  periodSeconds: 5
```

### Logging

**Log Aggregation:**

Use structured logging (JSON format) with log aggregation tools:

```toml
# config.toml
[logging]
level = "info"
json_format = true
```

**Example with Loki:**
```yaml
# promtail-config.yml
clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: anode
    static_configs:
      - targets:
          - localhost
        labels:
          job: anode
          __path__: /var/log/anode/*.log
```

**Important Log Messages:**
- `ERROR`: Critical failures requiring immediate action
- `WARN raft`: Raft state changes, elections
- `WARN storage`: Storage errors, corruption
- `INFO object`: Object operations (PUT/GET/DELETE)

## Backup and Restore

### Snapshot-Based Backup

#### Create Snapshot

```bash
# Trigger snapshot on each node
curl -X POST http://node1:8081/admin/snapshot
curl -X POST http://node2:8081/admin/snapshot
curl -X POST http://node3:8081/admin/snapshot

# Wait for completion
curl http://node1:8081/admin/snapshot/status
```

#### Backup Data

```bash
# On each node, backup data directory
tar -czf /backup/anode-node1-$(date +%Y%m%d).tar.gz \
    /var/lib/anode/data/
```

**Backup Schedule:**
- Daily: Full backup
- Hourly: Incremental backup (if supported by filesystem)

#### Restore from Backup

```bash
# Stop all nodes
systemctl stop anode

# Restore data on each node
tar -xzf /backup/anode-node1-20231219.tar.gz -C /

# Start nodes
systemctl start anode
```

### Continuous Backup

Use filesystem-level snapshots (LVM, ZFS, Btrfs):

**LVM Example:**
```bash
# Create snapshot
lvcreate -L 10G -s -n anode-snap /dev/vg0/anode-data

# Mount and backup
mount /dev/vg0/anode-snap /mnt/snapshot
rsync -av /mnt/snapshot/ /backup/anode-data/

# Cleanup
umount /mnt/snapshot
lvremove /dev/vg0/anode-snap
```

### Object-Level Backup

Backup individual buckets using S3 API:

```bash
# Sync bucket to local directory
aws --endpoint-url http://node1:8080 s3 sync \
    s3://my-bucket /backup/my-bucket/

# Or sync to another S3-compatible storage
aws s3 sync \
    --endpoint-url http://node1:8080 \
    s3://my-bucket \
    s3://backup-bucket \
    --source-region us-east-1 \
    --region us-west-2
```

## Scaling

### Adding Nodes

#### 1. Prepare New Node

```bash
# Install Anode
# Configure storage
mkdir -p /var/lib/anode/data
chown anode:anode /var/lib/anode/data
```

#### 2. Join Cluster

```bash
anode \
    --node-id 4 \
    --s3-addr 0.0.0.0:8080 \
    --grpc-addr 0.0.0.0:9090 \
    --admin-addr 0.0.0.0:8081 \
    --data-dir /var/lib/anode/data \
    --config /etc/anode/config.toml \
    --join node1:9090
```

#### 3. Wait for Data Rebalancing

```bash
# Monitor rebalancing progress
watch curl -s http://node4:8081/admin/rebalance

# Check placement group distribution
curl http://node1:8081/admin/placement-groups
```

Data automatically rebalances across new nodes based on consistent hashing.

### Removing Nodes

#### 1. Drain Node

```bash
# Mark node for removal (initiates data migration)
curl -X POST http://node1:8081/admin/nodes/4/drain

# Monitor drain progress
watch curl -s http://node1:8081/admin/nodes/4/status
```

#### 2. Remove from Cluster

```bash
# Once drained, remove from cluster
curl -X DELETE http://node1:8081/admin/nodes/4

# Verify removal
curl http://node1:8081/admin/members
```

#### 3. Shutdown Node

```bash
# Stop anode service
systemctl stop anode

# Clean up data (optional)
rm -rf /var/lib/anode/data
```

### Cluster Rebalancing

Manually trigger rebalancing:

```bash
# Start rebalancing
curl -X POST http://node1:8081/admin/rebalance

# Monitor progress
curl http://node1:8081/admin/rebalance/status

# Stop rebalancing (if needed)
curl -X POST http://node1:8081/admin/rebalance/stop
```

## Upgrades

### Rolling Upgrade

Zero-downtime upgrade procedure:

#### 1. Upgrade One Node at a Time

**Node 1:**
```bash
# Stop service
systemctl stop anode

# Backup current binary
cp /usr/local/bin/anode /usr/local/bin/anode.old

# Install new version
cp anode-v0.2.0 /usr/local/bin/anode

# Start service
systemctl start anode

# Verify health
curl http://node1:8081/health
```

**Wait for node to rejoin cluster and catch up:**
```bash
# Check Raft status
curl http://node1:8081/admin/raft

# Check replication lag
curl http://node1:8081/metrics | grep replication_lag
```

**Repeat for remaining nodes** (one at a time)

#### 2. Verify Cluster

```bash
# Check all nodes are same version
for node in node1 node2 node3; do
    curl -s http://$node:8081/admin/version
done

# Run smoke tests
./scripts/smoke-test.sh
```

### Rollback

If upgrade fails, rollback:

```bash
# Stop new version
systemctl stop anode

# Restore old binary
cp /usr/local/bin/anode.old /usr/local/bin/anode

# Start old version
systemctl start anode
```

## Troubleshooting

### Node Won't Start

**Check logs:**
```bash
journalctl -u anode -n 100 -f
```

**Common issues:**
- Port already in use → Change ports or stop conflicting process
- Permission denied → Fix data directory permissions
- Configuration error → Validate config with `--config` flag

### High Latency

**Check metrics:**
```bash
curl http://node1:8081/metrics | grep duration
```

**Possible causes:**
- Disk I/O bottleneck → Check `iostat`, enable `sync_writes = false`
- Network congestion → Check network bandwidth
- High CPU → Check CPU usage, increase `io_threads`
- Large objects → Use multipart upload, increase `chunk_size`

### Raft Leader Election Failures

**Symptoms:**
- Writes failing
- "No leader" errors
- Frequent leader changes

**Diagnosis:**
```bash
# Check Raft state on all nodes
for node in node1 node2 node3; do
    echo "=== $node ==="
    curl -s http://$node:8081/admin/raft | jq
done
```

**Solutions:**
- Network partition → Fix network connectivity
- Time skew → Sync clocks with NTP
- Overloaded leader → Increase heartbeat interval
- Split brain → Verify cluster membership

### Storage Corruption

**Detect:**
```bash
# Run scrub (checks all chunk checksums)
curl -X POST http://node1:8081/admin/scrub

# Check results
curl http://node1:8081/admin/scrub/results
```

**Repair:**
```bash
# Automatic repair from replicas
curl -X POST http://node1:8081/admin/repair

# Monitor repair progress
watch curl -s http://node1:8081/admin/repair/status
```

### Out of Disk Space

**Immediate action:**
```bash
# Stop accepting writes
curl -X POST http://node1:8081/admin/readonly

# Delete old/unused data
aws --endpoint-url http://node1:8080 s3 rb s3://old-bucket --force

# Run garbage collection
curl -X POST http://node1:8081/admin/gc

# Add more disk or add nodes
```

## Disaster Recovery

### Single Node Failure

**Automatic recovery:**
- Raft elects new leader for affected PGs
- Remaining nodes serve requests
- No data loss (if replication factor >= 2)

**Manual intervention needed if:**
- Majority of nodes lost → Restore from backup
- Data corruption on all replicas → Restore from backup

### Cluster-Wide Failure

#### Restore from Backup

```bash
# 1. Stop all nodes
for node in node1 node2 node3; do
    ssh $node "systemctl stop anode"
done

# 2. Restore data on all nodes
for node in node1 node2 node3; do
    ssh $node "tar -xzf /backup/anode-latest.tar.gz -C /"
done

# 3. Bootstrap first node
ssh node1 "anode --bootstrap --config /etc/anode/config.toml" &

# 4. Wait for bootstrap
sleep 30

# 5. Start remaining nodes
for node in node2 node3; do
    ssh $node "anode --join node1:9090 --config /etc/anode/config.toml" &
done
```

### Split Brain Recovery

If cluster splits into multiple groups:

```bash
# 1. Identify majority partition
# 2. Stop minority nodes
ssh minority-node "systemctl stop anode"

# 3. Clear data on minority nodes
ssh minority-node "rm -rf /var/lib/anode/data/*"

# 4. Rejoin minority nodes to majority
ssh minority-node "anode --join majority-node:9090"
```

## Performance Tuning

### Storage Optimization

**Use appropriate chunk size:**
```toml
[storage]
# Small files (< 10MB)
chunk_size = "1MB"

# Large files (> 100MB)
chunk_size = "8MB"
```

**Enable metadata caching:**
```toml
[storage]
metadata_cache_size = 1073741824  # 1GB
```

**Tune I/O threads:**
```toml
[storage]
io_threads = 16  # 2x CPU cores for I/O-heavy workload
```

### Network Optimization

**Increase heartbeat interval on high-latency networks:**
```toml
[cluster]
heartbeat_interval_ms = 2000
election_timeout_ms = 6000
```

**Use dedicated network for inter-node traffic:**
```toml
[node]
grpc_addr = "10.0.1.5:9090"  # Private network
```

### Raft Optimization

**Adjust snapshot interval:**
```toml
[raft]
snapshot_interval = 50000  # More snapshots = faster recovery
```

**Enable compaction:**
```toml
[raft]
enable_compaction = true
compaction_threshold = 100000
```

### Parquet Optimization

**Enable all optimizations:**
```toml
[parquet]
enable_cache = true
cache_size = 536870912  # 512MB
enable_predicate_pushdown = true
enable_column_pruning = true
```

**Benchmark results:**
```
Without optimization: 10s query
With optimization: 1s query (10x faster)
```

## Maintenance Tasks

### Regular Tasks

**Daily:**
- Review error logs
- Check disk space
- Monitor metrics

**Weekly:**
- Run scrub to check data integrity
- Review slow query logs
- Analyze storage usage

**Monthly:**
- Test backup restoration
- Review and update alerts
- Performance benchmarking

### Preventive Maintenance

**Garbage Collection:**
```bash
# Run weekly during low-traffic period
curl -X POST http://node1:8081/admin/gc
```

**Metadata Compaction:**
```bash
# Run monthly
curl -X POST http://node1:8081/admin/compact
```

**Log Rotation:**
```bash
# Configure in logging section
[logging]
rotate = true
max_size = 524288000  # 500MB
```

## Security Best Practices

1. **Network Isolation:**
   - Place gRPC endpoints on private network
   - Use firewall rules to restrict access
   - Enable TLS for all communication

2. **Access Control:**
   - Use strong access keys and secret keys
   - Rotate credentials regularly
   - Implement least-privilege access

3. **Encryption:**
   - Enable TLS for S3 API (terminate at load balancer)
   - Enable at-rest encryption (filesystem-level or chunk-level)
   - Encrypt backups

4. **Monitoring:**
   - Monitor for unusual access patterns
   - Alert on authentication failures
   - Track data access audits

5. **Updates:**
   - Keep Anode up to date with security patches
   - Subscribe to security mailing list
   - Test upgrades in staging first

## Support and Resources

- **Documentation**: https://github.com/christopherpaika/anode/docs
- **Issue Tracker**: https://github.com/christopherpaika/anode/issues
- **Community Forum**: https://github.com/christopherpaika/anode/discussions
- **Slack Channel**: #anode (request invite)
- **Email**: support@anode.io
