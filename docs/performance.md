# Performance Tuning Guide

Optimize Anode for your workload.

## Performance Characteristics

### Expected Performance

**Small Objects (< 1MB):**
- Write: 1000-5000 ops/sec per node
- Read: 5000-10000 ops/sec per node
- Latency: 10-50ms (write), 5-20ms (read)

**Large Objects (> 100MB):**
- Write: Network bandwidth / replication_factor
- Read: Network bandwidth
- Latency: Throughput-bound

**Parquet Queries:**
- 5-10x faster than reading full file
- Depends on selectivity and predicate pushdown

## Benchmarking Your Cluster

### Baseline Benchmark

```bash
# Install s3-benchmark
go install github.com/wasabi-tech/s3-benchmark@latest

# Run benchmark
s3-benchmark \
    -endpoint http://node1:8080 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket benchmark \
    -numClients 10 \
    -numSamples 1000 \
    -objectSize 1M
```

### Custom Benchmark

```rust
use anode_client::Client;
use std::time::Instant;

#[tokio::main]
async fn main() {
    let client = Client::new(config).await.unwrap();
    let data = vec![0u8; 1024 * 1024]; // 1MB

    // Warmup
    for i in 0..100 {
        client.put_object("bench", &format!("warmup-{}", i), data.clone(), None).await.unwrap();
    }

    // Benchmark writes
    let start = Instant::now();
    let count = 1000;

    for i in 0..count {
        client.put_object("bench", &format!("obj-{}", i), data.clone(), None).await.unwrap();
    }

    let duration = start.elapsed();
    let throughput = count as f64 / duration.as_secs_f64();

    println!("Write throughput: {:.2} ops/sec", throughput);
    println!("Average latency: {:.2} ms", duration.as_millis() as f64 / count as f64);
}
```

## Storage Optimization

### Chunk Size Tuning

**Default: 4MB**

**When to increase (8MB, 16MB):**
- Large files (> 1GB)
- Sequential access patterns
- Network bandwidth limited

**When to decrease (1MB, 2MB):**
- Small files (< 10MB)
- Random access patterns
- Better deduplication needed

**Example:**
```toml
[storage]
chunk_size = "8MB"  # For large file workload
```

**Benchmark different chunk sizes:**
```bash
for size in 1 2 4 8 16; do
    echo "Testing chunk_size=${size}MB"
    # Update config
    sed -i "s/chunk_size = .*/chunk_size = \"${size}MB\"/" config.toml
    # Restart cluster
    systemctl restart anode
    # Run benchmark
    ./benchmark.sh
done
```

### Metadata Cache Size

**Default: 100MB**

**Calculation:**
- ~1KB per object
- 100MB = ~100K objects in cache

**Recommendations:**
- Small cluster (<1M objects): 100-200MB
- Medium cluster (1-10M objects): 500MB-1GB
- Large cluster (>10M objects): 1-2GB

**Example:**
```toml
[storage]
metadata_cache_size = 1073741824  # 1GB
```

**Monitor cache hit rate:**
```bash
curl http://node1:8081/metrics | grep cache_hit_ratio
```

Target: > 90% hit rate

### I/O Thread Tuning

**Default: Number of CPU cores**

**Recommendations:**
- CPU-bound workload: 0.5x CPU cores
- Balanced workload: 1x CPU cores
- I/O-bound workload: 2x CPU cores

**Example:**
```toml
[storage]
io_threads = 16  # For I/O-heavy workload on 8-core machine
```

**Monitor thread usage:**
```bash
# CPU usage by thread
top -H -p $(pgrep anode)

# I/O wait time
iostat -x 1
```

### Sync Writes

**Default: false**

**Performance impact:**
- Enabled: +5-10ms per write
- Disabled: No fsync overhead

**Trade-off:**
- Enabled: Better durability (no data loss on crash)
- Disabled: Better performance (risk of data loss)

**Recommendation:**
```toml
[storage]
sync_writes = false  # Production with UPS/battery-backed RAID
sync_writes = true   # Production without UPS
```

## Network Optimization

### Dedicated Network for Replication

Use separate network for inter-node traffic:

**Node 1:**
```toml
[node]
s3_addr = "0.0.0.0:8080"        # Public network
grpc_addr = "10.0.1.5:9090"     # Private 10GbE network
```

**Benefits:**
- Isolates replication traffic
- Prevents S3 traffic from impacting cluster communication
- Better utilization of network bandwidth

### Network Tuning (Linux)

```bash
# Increase TCP buffer sizes
sysctl -w net.core.rmem_max=134217728
sysctl -w net.core.wmem_max=134217728
sysctl -w net.ipv4.tcp_rmem="4096 87380 67108864"
sysctl -w net.ipv4.tcp_wmem="4096 65536 67108864"

# Increase connection backlog
sysctl -w net.core.somaxconn=4096
sysctl -w net.ipv4.tcp_max_syn_backlog=4096

# Enable TCP window scaling
sysctl -w net.ipv4.tcp_window_scaling=1

# Make permanent
cat >> /etc/sysctl.conf <<EOF
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864
net.core.somaxconn = 4096
net.ipv4.tcp_max_syn_backlog = 4096
net.ipv4.tcp_window_scaling = 1
EOF
```

## Raft Optimization

### Heartbeat and Election Timeouts

**Default:**
- Heartbeat: 1000ms
- Election timeout: 3000ms

**Low latency network (< 1ms):**
```toml
[cluster]
heartbeat_interval_ms = 500
election_timeout_ms = 1500
```

**High latency network (> 10ms):**
```toml
[cluster]
heartbeat_interval_ms = 2000
election_timeout_ms = 6000
```

**Trade-offs:**
- Lower timeouts: Faster failure detection, more network traffic
- Higher timeouts: Slower failure detection, less network traffic

### Snapshot Configuration

**Default:**
- Snapshot interval: 10000 log entries
- Compaction threshold: 50000 log entries

**High write rate:**
```toml
[raft]
snapshot_interval = 50000      # Less frequent snapshots
compaction_threshold = 100000
```

**Low memory:**
```toml
[raft]
snapshot_interval = 5000       # More frequent snapshots
max_log_entries = 50000        # Keep fewer logs in memory
```

**Monitor log size:**
```bash
curl http://node1:8081/metrics | grep raft_log_size
```

## Parquet Optimization

### Enable All Optimizations

```toml
[parquet]
enable_cache = true
cache_size = 536870912              # 512MB
enable_predicate_pushdown = true
enable_column_pruning = true
```

### Parquet Cache Size

**Calculation:**
- Parquet footer: ~10KB per file
- 512MB = ~50K Parquet files

**Recommendations:**
- Few large files: 256MB
- Many small files: 512MB-1GB

**Monitor cache effectiveness:**
```bash
curl http://node1:8081/metrics | grep parquet_cache
```

Metrics to watch:
- `parquet_cache_hit_ratio` (target: > 80%)
- `parquet_row_groups_scanned` (lower is better)
- `parquet_bytes_scanned` (lower is better)

### Query Optimization Example

**Before optimization:**
```sql
SELECT col1, col2 FROM table WHERE date = '2023-12-19'
-- Reads: 1GB (entire file)
-- Time: 10s
```

**After optimization:**
```sql
-- Same query, optimized:
-- 1. Check cached footer metadata
-- 2. Prune row groups using date statistics
-- 3. Read only col1, col2, date columns
-- 4. Apply date filter during scan

-- Reads: 100MB (only relevant row groups and columns)
-- Time: 1s (10x faster)
```

## Hardware Recommendations

### CPU

**Minimum:**
- 4 cores per node
- 2.0 GHz+

**Recommended:**
- 8-16 cores per node
- 3.0 GHz+
- Prefer higher core count over higher frequency

**Workload-specific:**
- I/O-bound: 4-8 cores sufficient
- CPU-bound (compression, encryption): 16+ cores

### Memory

**Minimum:**
- 8GB per node

**Recommended:**
- 16-32GB per node
- More for large metadata cache or Parquet cache

**Calculation:**
```
Total RAM = Base + Metadata Cache + Parquet Cache + Working Set

Base: 2GB
Metadata Cache: 100MB per 100K objects
Parquet Cache: 10KB per Parquet file
Working Set: 4GB (connections, buffers, etc.)

Example (1M objects, 10K Parquet files):
2GB + 1GB + 100MB + 4GB = ~7GB minimum
Recommended: 16GB (2x minimum)
```

### Storage

**Minimum:**
- SATA SSD
- 500 MB/s read/write

**Recommended:**
- NVMe SSD
- 2000+ MB/s read/write
- 100K+ IOPS

**Optimal:**
- Enterprise NVMe SSD
- 3000+ MB/s read/write
- 500K+ IOPS
- Power-loss protection

**RAID:**
- Not required (Anode handles replication)
- RAID 0 for performance (if replication factor >= 3)
- RAID 1 for durability (if replication factor = 2)

**Filesystem:**
- ext4: Good general purpose
- XFS: Better for large files
- ZFS: Built-in compression and checksums
- Btrfs: Snapshots and deduplication

### Network

**Minimum:**
- 1 Gbps between nodes
- < 10ms latency

**Recommended:**
- 10 Gbps between nodes
- < 1ms latency
- Separate network for replication

**Optimal:**
- 25+ Gbps between nodes
- < 0.5ms latency
- RDMA (future support)

## OS Tuning

### Linux Kernel Parameters

```bash
# /etc/sysctl.conf

# Network
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 67108864
net.ipv4.tcp_wmem = 4096 65536 67108864

# File descriptors
fs.file-max = 1048576
fs.nr_open = 1048576

# Virtual memory
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5

# For SSD
vm.vfs_cache_pressure = 50
```

### File Descriptor Limits

```bash
# /etc/security/limits.conf
anode soft nofile 65536
anode hard nofile 65536
```

### Disable Transparent Huge Pages

```bash
echo never > /sys/kernel/mm/transparent_hugepage/enabled
echo never > /sys/kernel/mm/transparent_hugepage/defrag

# Make permanent
cat >> /etc/rc.local <<EOF
echo never > /sys/kernel/mm/transparent_hugepage/enabled
echo never > /sys/kernel/mm/transparent_hugepage/defrag
EOF
```

### CPU Governor

```bash
# Use performance governor
cpupower frequency-set -g performance

# Verify
cpupower frequency-info
```

## Workload-Specific Tuning

### Small Object Workload

**Characteristics:**
- Object size: < 1MB
- High request rate
- Latency-sensitive

**Optimizations:**
```toml
[storage]
chunk_size = "1MB"
metadata_cache_size = 1073741824  # 1GB
io_threads = 8

[cluster]
heartbeat_interval_ms = 1000
```

**Monitoring:**
```bash
# Watch request rate
watch "curl -s http://node1:8081/metrics | grep requests_total"

# Watch latency
watch "curl -s http://node1:8081/metrics | grep duration_seconds"
```

### Large Object Workload

**Characteristics:**
- Object size: > 100MB
- Throughput-sensitive
- Multipart uploads

**Optimizations:**
```toml
[storage]
chunk_size = "16MB"
io_threads = 16

[s3]
max_body_size = 10737418240  # 10GB
```

**Use multipart uploads:**
```bash
aws --endpoint-url http://node1:8080 s3 cp \
    large-file.bin s3://bucket/ \
    --storage-class STANDARD \
    --metadata "uploaded=true"
```

### Parquet Analytics Workload

**Characteristics:**
- Reading Parquet files
- Column projection
- Predicate pushdown

**Optimizations:**
```toml
[parquet]
enable_cache = true
cache_size = 1073741824  # 1GB
enable_predicate_pushdown = true
enable_column_pruning = true

[storage]
chunk_size = "8MB"  # Align with Parquet row groups
```

**Query optimization:**
```python
import pyarrow.parquet as pq

# Use filters
table = pq.read_table(
    's3://bucket/file.parquet',
    filters=[('date', '=', '2023-12-19')],
    columns=['col1', 'col2']
)
```

### Mixed Workload

**Characteristics:**
- Variety of object sizes
- Mix of reads and writes
- Variable access patterns

**Optimizations:**
```toml
[storage]
chunk_size = "4MB"              # Balanced
metadata_cache_size = 536870912 # 512MB
io_threads = 12                 # 1.5x CPU cores

[raft]
snapshot_interval = 20000       # Moderate
```

## Performance Monitoring

### Key Metrics

**Request metrics:**
```bash
# Request rate
rate(anode_requests_total[5m])

# Latency (P95)
histogram_quantile(0.95, anode_request_duration_seconds)

# Error rate
rate(anode_request_errors_total[5m])
```

**Storage metrics:**
```bash
# Disk usage
anode_storage_bytes_total

# Cache hit rate
anode_metadata_cache_hit_ratio

# I/O operations
rate(anode_storage_io_operations_total[5m])
```

**Raft metrics:**
```bash
# Replication lag
anode_raft_replication_lag_seconds

# Log size
anode_raft_log_size

# Leader elections
rate(anode_raft_leader_elections_total[5m])
```

### Performance Dashboards

**Grafana queries:**

```promql
# Throughput
sum(rate(anode_requests_total[5m])) by (method)

# Latency heatmap
histogram_quantile(0.95, sum(rate(anode_request_duration_seconds_bucket[5m])) by (le))

# Storage growth
deriv(anode_storage_bytes_total[1h])
```

## Troubleshooting Performance Issues

### High Latency

**Diagnosis:**
```bash
# Check disk I/O
iostat -x 1

# Check network
iftop -i eth0

# Check CPU
top -H -p $(pgrep anode)

# Check Raft lag
curl http://node1:8081/metrics | grep replication_lag
```

**Solutions:**
- Disk slow → Use faster SSD, enable caching
- Network slow → Increase bandwidth, dedicated network
- CPU high → Increase cores, reduce compression
- Raft lag → Increase heartbeat interval, snapshot more often

### Low Throughput

**Diagnosis:**
```bash
# Check concurrent connections
netstat -an | grep :8080 | wc -l

# Check thread utilization
ps -eLf | grep anode | wc -l

# Check resource limits
ulimit -a
```

**Solutions:**
- Low concurrency → Increase client parallelism
- Thread contention → Increase `io_threads`
- Resource limits → Increase file descriptors, connection limits

### Memory Usage

**Diagnosis:**
```bash
# Memory breakdown
ps aux | grep anode

# Cache sizes
curl http://node1:8081/metrics | grep cache_size
```

**Solutions:**
- High metadata cache → Reduce `metadata_cache_size`
- High Parquet cache → Reduce `parquet.cache_size`
- Memory leak → Update to latest version, report issue

## Best Practices

1. **Benchmark before and after changes**
2. **Change one parameter at a time**
3. **Monitor metrics continuously**
4. **Test with realistic workload**
5. **Use dedicated hardware for performance testing**
6. **Document your configuration**
7. **Keep cluster configuration consistent**
8. **Regular performance regression testing**

## Performance Checklist

**Hardware:**
- [ ] NVMe SSD for storage
- [ ] 10+ Gbps network
- [ ] 8+ CPU cores per node
- [ ] 16+ GB RAM per node

**Configuration:**
- [ ] Tuned chunk size for workload
- [ ] Appropriate cache sizes
- [ ] I/O threads = CPU cores (or tuned)
- [ ] Raft timeouts tuned for network latency

**Operating System:**
- [ ] Kernel parameters tuned
- [ ] File descriptor limits increased
- [ ] THP disabled
- [ ] CPU governor = performance

**Monitoring:**
- [ ] Prometheus scraping metrics
- [ ] Grafana dashboards configured
- [ ] Alerts for anomalies
- [ ] Regular performance testing

**Optimization:**
- [ ] Parquet optimizations enabled
- [ ] Metadata caching enabled
- [ ] Appropriate replication factor
- [ ] Balanced placement group distribution
