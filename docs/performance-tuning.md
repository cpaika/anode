# Anode Performance Tuning Guide

A comprehensive, actionable guide for optimizing Anode performance across different workloads and environments.

## Table of Contents

- [Quick Start](#quick-start)
- [Tuning Methodology](#tuning-methodology)
- [Hardware Selection and Sizing](#hardware-selection-and-sizing)
- [Storage Engine Tuning](#storage-engine-tuning)
- [Raft Consensus Tuning](#raft-consensus-tuning)
- [Memory and Caching Optimization](#memory-and-caching-optimization)
- [Network Optimization](#network-optimization)
- [Operating System Tuning](#operating-system-tuning)
- [Parquet Workload Optimization](#parquet-workload-optimization)
- [Workload-Specific Configurations](#workload-specific-configurations)
- [Monitoring and Metrics](#monitoring-and-metrics)
- [Performance Troubleshooting](#performance-troubleshooting)
- [Benchmarking and Validation](#benchmarking-and-validation)
- [Best Practices and Recommendations](#best-practices-and-recommendations)

## Quick Start

### 5-Minute Performance Boost

For immediate performance improvements, apply these changes:

```toml
# /etc/anode/config.toml

[storage]
# Increase metadata cache (default: 100MB → 1GB)
metadata_cache_size = 1073741824

# Optimize I/O threads for your CPU count (default: CPU cores)
io_threads = 12  # For 8-core CPU: use 1.5x cores

[parquet]
# Enable all Parquet optimizations
enable_cache = true
cache_size = 536870912  # 512MB
enable_predicate_pushdown = true
enable_column_pruning = true
```

Then apply OS-level tuning:

```bash
# Increase file descriptor limits
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Optimize TCP parameters
sysctl -w net.ipv4.tcp_rmem="4096 87380 67108864"
sysctl -w net.ipv4.tcp_wmem="4096 65536 67108864"
```

**Expected improvements:**
- 30-50% higher throughput for small objects
- 20-30% lower latency
- Better cache utilization

### Performance Validation

After applying changes, benchmark your cluster:

```bash
# Clone and build s3-benchmark
go install github.com/wasabi-tech/s3-benchmark@latest

# Run baseline test
s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket perftest \
    -numClients 16 \
    -numSamples 1000 \
    -objectSize 1M
```

Monitor key metrics:

```bash
# Watch request rate and latency
watch "curl -s http://localhost:9002/metrics | grep -E 'anode_requests_total|anode_request_duration'"
```

## Tuning Methodology

### Systematic Approach

Follow this methodology for reliable performance improvements:

1. **Establish Baseline**
   - Run benchmarks with current configuration
   - Record all metrics (latency, throughput, errors)
   - Document system specifications

2. **Identify Bottlenecks**
   - Analyze metrics to find constraints
   - Common bottlenecks: CPU, disk I/O, network, memory

3. **Make One Change at a Time**
   - Modify a single parameter
   - Document the change
   - Measure impact

4. **Validate Improvement**
   - Re-run benchmarks
   - Compare with baseline
   - Check for regressions

5. **Iterate**
   - Apply next optimization
   - Repeat until targets are met

### Decision Tree: What to Tune First?

```
Are you experiencing...

├─ High Latency (>100ms)?
│  ├─ Check disk I/O → Tune storage settings
│  ├─ Check network → Tune network parameters
│  └─ Check Raft lag → Tune consensus timeouts
│
├─ Low Throughput (<1000 ops/sec)?
│  ├─ Check CPU usage → Adjust io_threads
│  ├─ Check cache hit rate → Increase cache sizes
│  └─ Check concurrency → Scale horizontally
│
├─ High Memory Usage?
│  ├─ Check cache sizes → Reduce cache_size
│  ├─ Check Raft log → Increase snapshot_interval
│  └─ Check for leaks → Update to latest version
│
└─ Parquet Queries Slow?
   ├─ Enable caching → Set enable_cache=true
   ├─ Check cache hits → Increase parquet.cache_size
   └─ Verify optimizations → Enable pushdown/pruning
```

## Hardware Selection and Sizing

### Capacity Planning

Calculate your hardware requirements:

**Storage Capacity:**
```
Raw Capacity Needed = (Total Data Size) × (Replication Factor) × 1.3

Example: 10TB data, replication_factor=3, 30% overhead
  = 10TB × 3 × 1.3 = 39TB raw capacity

Recommendation: Provision 50TB to allow for growth
```

**Memory Requirements:**
```
Total RAM = Base + Metadata Cache + Parquet Cache + Working Set + OS Buffer

Base: 2GB (Anode runtime)
Metadata Cache: (Object Count / 100,000) × 100MB
Parquet Cache: (Parquet File Count / 50,000) × 512MB
Working Set: 4GB (connections, buffers)
OS Buffer: 4GB (filesystem cache)

Example: 1M objects, 10K Parquet files
  = 2GB + 1GB + 100MB + 4GB + 4GB = 11.1GB
Recommendation: 16GB RAM per node
```

**CPU Sizing:**
```
Cores Needed = Base + (Expected Throughput / Throughput per Core)

Base: 4 cores (minimum)
Throughput per Core: ~1,000 ops/sec (small objects)

Example: Target 10,000 ops/sec
  = 4 + (10,000 / 1,000) = 14 cores
Recommendation: 16-core CPU
```

### Storage Device Selection

**Performance Tier Comparison:**

| Metric | SATA SSD | NVMe SSD | Enterprise NVMe |
|--------|----------|----------|-----------------|
| Sequential Read | 500 MB/s | 3,500 MB/s | 7,000 MB/s |
| Sequential Write | 450 MB/s | 3,000 MB/s | 5,500 MB/s |
| Random IOPS (4KB) | 90K | 500K | 1M+ |
| Latency | 50-100µs | 10-20µs | 5-10µs |
| Cost per TB | $ | $$ | $$$ |
| **Recommended For** | Development | Production | High Performance |

**Decision Guide:**

- **Development/Testing**: SATA SSD (cost-effective)
- **Production (standard)**: Consumer NVMe SSD (balanced)
- **Production (high-perf)**: Enterprise NVMe with PLP (power-loss protection)
- **Analytics (Parquet)**: NVMe with high sequential throughput

### Network Requirements

**Bandwidth Planning:**

```
Network Bandwidth = (Write Throughput) × (Replication Factor) × 1.5

Example: 1 GB/s write throughput, replication_factor=3
  = 1 GB/s × 3 × 1.5 = 4.5 Gb/s
Recommendation: 10 Gbps network
```

**Network Topology:**

**Single Network (Simple):**
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Node 1    │────▶│   Node 2    │────▶│   Node 3    │
│ 10GbE eth0  │     │ 10GbE eth0  │     │ 10GbE eth0  │
│ S3 + Raft   │     │ S3 + Raft   │     │ S3 + Raft   │
└─────────────┘     └─────────────┘     └─────────────┘
```

**Dual Network (Recommended):**
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Node 1    │     │   Node 2    │     │   Node 3    │
│             │     │             │     │             │
│ eth0: S3    │◀──▶ │ eth0: S3    │◀──▶ │ eth0: S3    │
│   (1GbE)    │     │   (1GbE)    │     │   (1GbE)    │
│             │     │             │     │             │
│ eth1: Raft  │◀──▶ │ eth1: Raft  │◀──▶ │ eth1: Raft  │
│   (10GbE)   │     │   (10GbE)   │     │   (10GbE)   │
└─────────────┘     └─────────────┘     └─────────────┘
```

**Configuration for dual network:**

```toml
[node]
# Client-facing S3 API on public network
s3_addr = "0.0.0.0:9000"

# Cluster communication on private high-speed network
grpc_addr = "10.0.1.10:9001"  # eth1 IP address
```

## Storage Engine Tuning

### Chunk Size Optimization

Chunk size is the most critical storage tuning parameter.

**Impact of Chunk Size:**

| Chunk Size | Object Size | Overhead | Dedup | Throughput | Use Case |
|------------|-------------|----------|-------|------------|----------|
| 1MB | <10MB | High | Best | Lower | Small files |
| 2MB | 10-50MB | Medium | Good | Medium | Mixed |
| **4MB** | 50-500MB | Low | Good | High | **Default** |
| 8MB | 500MB-5GB | Lower | Fair | Higher | Large files |
| 16MB | >5GB | Lowest | Poor | Highest | Streaming |

**Tuning Process:**

1. **Determine your workload profile:**

```bash
# Analyze object sizes in your bucket
aws --endpoint-url http://localhost:9000 s3api list-objects-v2 \
    --bucket mybucket --query 'Contents[*].Size' | \
    python3 -c "
import sys, json, statistics
sizes = json.load(sys.stdin)
print(f'Min: {min(sizes)/1024/1024:.2f} MB')
print(f'Max: {max(sizes)/1024/1024:.2f} MB')
print(f'Median: {statistics.median(sizes)/1024/1024:.2f} MB')
print(f'Mean: {statistics.mean(sizes)/1024/1024:.2f} MB')
"
```

2. **Select chunk size based on results:**

```
Median Object Size    → Recommended Chunk Size
─────────────────────────────────────────────
< 5 MB                → 1 MB
5-20 MB               → 2 MB
20-100 MB             → 4 MB (default)
100-500 MB            → 8 MB
> 500 MB              → 16 MB
```

3. **Update configuration:**

```toml
[storage]
chunk_size = "8MB"  # For large file workload
```

4. **Benchmark the change:**

```bash
# Test with representative object sizes
for size in 1M 10M 100M 1G; do
    echo "Testing ${size} objects..."
    dd if=/dev/urandom of=test_${size}.bin bs=${size} count=1 2>/dev/null

    time aws --endpoint-url http://localhost:9000 s3 cp \
        test_${size}.bin s3://perftest/test_${size}.bin
done
```

**Warning:** Changing chunk size requires re-uploading all existing data. Plan this change during initial deployment.

### Sync Writes Configuration

Control durability vs. performance trade-off:

**sync_writes = true (Safe, Slower):**
- Every write is fsync'd to disk
- +5-10ms latency per write
- No data loss on power failure
- **Use when:** Running on commodity hardware without UPS

**sync_writes = false (Fast, Risk):**
- Writes buffered by OS
- Data may be lost on power failure
- No latency penalty
- **Use when:**
  - Hardware RAID with battery backup
  - UPS protecting all nodes
  - Acceptable to lose <5 seconds of recent writes

**Configuration:**

```toml
[storage]
# Production with UPS or battery-backed RAID
sync_writes = false

# Production without power protection
sync_writes = true

# Development
sync_writes = false
```

**Benchmark the impact:**

```bash
# Test with sync_writes=true
sed -i 's/sync_writes = .*/sync_writes = true/' /etc/anode/config.toml
systemctl restart anode
./benchmark_writes.sh > results_sync_true.txt

# Test with sync_writes=false
sed -i 's/sync_writes = .*/sync_writes = false/' /etc/anode/config.toml
systemctl restart anode
./benchmark_writes.sh > results_sync_false.txt

# Compare results
echo "Latency with sync=true: $(grep 'avg latency' results_sync_true.txt)"
echo "Latency with sync=false: $(grep 'avg latency' results_sync_false.txt)"
```

### I/O Thread Pool Sizing

Tune the number of worker threads for storage operations:

**Sizing Formula:**

```
For CPU-bound workloads (compression, encryption):
  io_threads = 0.5 × CPU_cores

For balanced workloads (default):
  io_threads = 1.0 × CPU_cores

For I/O-bound workloads (many concurrent requests):
  io_threads = 2.0 × CPU_cores

Maximum: 32 threads (diminishing returns beyond this)
```

**Configuration:**

```toml
[storage]
# 8-core CPU, I/O-bound workload
io_threads = 16

# 16-core CPU, CPU-bound workload (encryption enabled)
io_threads = 8

# Let Anode auto-detect (default)
# io_threads = 0  # 0 or omit = auto-detect
```

**Determine your workload type:**

```bash
# Run a typical workload and monitor CPU vs I/O wait
iostat -x 1 10 > iostat.log &
PID=$!

# Generate load (adjust for your workload)
for i in {1..100}; do
    aws --endpoint-url http://localhost:9000 s3 cp \
        test.bin s3://perftest/test_${i}.bin &
done
wait

kill $PID

# Analyze results
echo "Average I/O wait:"
awk '/avg:/ {print $NF}' iostat.log | tail -1

# If iowait > 20%: I/O-bound → increase io_threads
# If iowait < 5%: CPU-bound → decrease io_threads
```

### Metadata Cache Tuning

The metadata cache stores object metadata in memory for fast lookups.

**Cache Size Calculation:**

```
Memory per object ≈ 1KB (metadata + overhead)

For 100K objects:
  100,000 × 1KB = 100MB

For 1M objects:
  1,000,000 × 1KB = 1GB

For 10M objects:
  10,000,000 × 1KB = 10GB
```

**Sizing Recommendations:**

```toml
[storage]
# Small cluster (<100K objects)
metadata_cache_size = 104857600  # 100MB (default)

# Medium cluster (100K-1M objects)
metadata_cache_size = 536870912  # 512MB

# Large cluster (1M-10M objects)
metadata_cache_size = 1073741824  # 1GB

# Very large cluster (>10M objects)
metadata_cache_size = 2147483648  # 2GB
```

**Monitor cache effectiveness:**

```bash
# Check cache hit rate (target: >90%)
curl -s http://localhost:9002/metrics | \
    grep metadata_cache_hit_ratio

# Check cache size and evictions
curl -s http://localhost:9002/metrics | \
    grep -E 'metadata_cache_(size|evictions)'
```

**Optimization strategy:**

1. Start with default (100MB)
2. Monitor hit rate for 24 hours
3. If hit rate < 90%, double cache size
4. Repeat until hit rate > 90% or memory constrained

## Raft Consensus Tuning

### Heartbeat and Election Timeouts

Critical for cluster stability and failure detection.

**Default Settings:**

```toml
[cluster]
heartbeat_interval_ms = 1000    # 1 second
election_timeout_ms = 3000      # 3 seconds
```

**Tuning Guidelines:**

**Rule:** `election_timeout_ms` should be at least 3× `heartbeat_interval_ms`

**Network Latency-Based Tuning:**

```bash
# Measure inter-node latency
ping -c 100 node2.cluster.local | tail -1

# Results:
# rtt min/avg/max/mdev = 0.3/0.5/2.1/0.2 ms → Low latency network
# rtt min/avg/max/mdev = 5.1/10.3/25.4/3.2 ms → Medium latency network
# rtt min/avg/max/mdev = 50.2/100.5/201.3/25.1 ms → High latency network
```

**Recommended Settings:**

```toml
# Low latency network (< 1ms avg, same datacenter)
[cluster]
heartbeat_interval_ms = 500
election_timeout_ms = 1500

# Medium latency network (1-10ms avg, same region)
[cluster]
heartbeat_interval_ms = 1000   # default
election_timeout_ms = 3000     # default

# High latency network (10-50ms avg, cross-region)
[cluster]
heartbeat_interval_ms = 2000
election_timeout_ms = 6000

# Very high latency (>50ms avg, cross-continent)
[cluster]
heartbeat_interval_ms = 5000
election_timeout_ms = 15000
```

**Trade-offs:**

| Lower Timeouts | Higher Timeouts |
|----------------|-----------------|
| Faster failure detection | Slower failure detection |
| More network traffic | Less network traffic |
| Risk of false leader elections | More stable leadership |
| Better for low-latency networks | Better for high-latency networks |

### Snapshot and Compaction

Manage Raft log size and memory usage:

**Configuration Parameters:**

```toml
[raft]
# Trigger snapshot after this many log entries
snapshot_interval = 10000       # default

# Keep this many entries in memory
max_log_entries = 100000        # default

# Trigger compaction at this threshold
compaction_threshold = 50000    # default

# Enable automatic compaction
enable_compaction = true        # default
```

**Tuning for Different Write Rates:**

```toml
# High write rate (>10K ops/sec)
[raft]
snapshot_interval = 50000       # Less frequent snapshots (less CPU)
max_log_entries = 200000        # Keep more in memory
compaction_threshold = 100000   # Compact less often

# Low write rate (<1K ops/sec)
[raft]
snapshot_interval = 5000        # More frequent snapshots
max_log_entries = 25000         # Use less memory
compaction_threshold = 10000    # Compact more often

# Memory-constrained environment
[raft]
snapshot_interval = 5000        # Snapshot often
max_log_entries = 50000         # Limit memory usage
compaction_threshold = 25000    # Aggressive compaction
```

**Monitor Raft log health:**

```bash
# Check log size and growth rate
curl -s http://localhost:9002/metrics | grep raft_log_size

# Check snapshot frequency
curl -s http://localhost:9002/metrics | grep raft_snapshot

# Alert if log size grows unbounded
# (indicates snapshot/compaction issues)
```

## Memory and Caching Optimization

### Memory Budget Allocation

Plan your memory allocation across components:

**Example: 16GB RAM Server**

```
Total: 16GB
├─ OS + System: 4GB (25%)
├─ Anode Runtime: 2GB (12.5%)
├─ Metadata Cache: 4GB (25%)
├─ Parquet Cache: 2GB (12.5%)
├─ Raft Logs: 1GB (6%)
├─ Connection Buffers: 1GB (6%)
└─ Free/Buffer: 2GB (12.5%)
```

**Configuration:**

```toml
[storage]
metadata_cache_size = 4294967296  # 4GB

[parquet]
cache_size = 2147483648  # 2GB

[raft]
max_log_entries = 100000  # ~1GB max
```

### Cache Warming Strategies

Pre-populate caches to avoid cold start performance:

**1. Metadata Cache Warming:**

```bash
# List all buckets to populate bucket metadata
aws --endpoint-url http://localhost:9000 s3 ls

# List all objects (paginated) to populate object metadata
aws --endpoint-url http://localhost:9000 s3 ls s3://mybucket --recursive | \
    head -10000
```

**2. Parquet Cache Warming:**

```bash
# Issue HEAD requests for Parquet files to cache metadata
aws --endpoint-url http://localhost:9000 s3api list-objects-v2 \
    --bucket analytics --query 'Contents[?ends_with(Key, `.parquet`)].Key' | \
    jq -r '.[]' | \
    xargs -P 10 -I {} \
    aws --endpoint-url http://localhost:9000 s3api head-object \
        --bucket analytics --key {}
```

**3. Automated Warming on Startup:**

Create a systemd service to warm caches:

```ini
# /etc/systemd/system/anode-cache-warmer.service
[Unit]
Description=Anode Cache Warmer
After=anode.service
Requires=anode.service

[Service]
Type=oneshot
ExecStartPre=/bin/sleep 30
ExecStart=/usr/local/bin/warm-anode-cache.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
```

```bash
#!/bin/bash
# /usr/local/bin/warm-anode-cache.sh

echo "Warming Anode caches..."

# Wait for Anode to be ready
until curl -s http://localhost:9002/health > /dev/null; do
    sleep 5
done

# Warm metadata cache
aws --endpoint-url http://localhost:9000 s3 ls s3://production --recursive | head -50000

# Warm Parquet cache
# ... (Parquet warming logic)

echo "Cache warming complete"
```

## Network Optimization

### TCP Tuning for Object Storage

Optimize TCP parameters for large object transfers:

```bash
# /etc/sysctl.conf

# Increase TCP buffer sizes for high-bandwidth transfers
net.core.rmem_max = 134217728         # 128MB
net.core.wmem_max = 134217728         # 128MB
net.ipv4.tcp_rmem = 4096 87380 67108864    # min default max (64MB)
net.ipv4.tcp_wmem = 4096 65536 67108864    # min default max (64MB)

# Increase connection queue lengths
net.core.somaxconn = 4096
net.ipv4.tcp_max_syn_backlog = 8192
net.core.netdev_max_backlog = 5000

# Enable TCP window scaling (required for >64KB windows)
net.ipv4.tcp_window_scaling = 1

# Enable TCP timestamps
net.ipv4.tcp_timestamps = 1

# Increase local port range
net.ipv4.ip_local_port_range = 1024 65535

# Faster connection recycling
net.ipv4.tcp_fin_timeout = 30
net.ipv4.tcp_tw_reuse = 1

# Congestion control (for high-bandwidth networks)
net.ipv4.tcp_congestion_control = bbr
net.core.default_qdisc = fq
```

Apply settings:

```bash
sysctl -p /etc/sysctl.conf
```

### Network Interface Tuning

Optimize NIC settings for maximum throughput:

```bash
# Check current settings
ethtool eth0

# Increase ring buffer sizes
ethtool -G eth0 rx 4096 tx 4096

# Enable hardware offloading
ethtool -K eth0 tso on gso on gro on

# Increase interrupt coalescence (reduce CPU interrupts)
ethtool -C eth0 rx-usecs 50 tx-usecs 50

# Make changes persistent
cat > /etc/network/if-up.d/tune-eth0 <<'EOF'
#!/bin/bash
ethtool -G eth0 rx 4096 tx 4096
ethtool -K eth0 tso on gso on gro on
ethtool -C eth0 rx-usecs 50 tx-usecs 50
EOF
chmod +x /etc/network/if-up.d/tune-eth0
```

### Jumbo Frames (9000 MTU)

Enable jumbo frames for intra-cluster communication:

**Benefits:**
- 10-20% higher throughput
- Lower CPU usage
- Fewer packets to process

**Requirements:**
- All nodes must support 9000 MTU
- All switches in path must support jumbo frames
- Same-subnet only (no routing with jumbo frames)

**Configuration:**

```bash
# On each node
ip link set eth1 mtu 9000

# Verify
ip link show eth1 | grep mtu

# Test connectivity
ping -M do -s 8972 -c 5 node2.cluster.local

# Make persistent (Ubuntu/Debian)
cat >> /etc/network/interfaces <<EOF
auto eth1
iface eth1 inet static
    address 10.0.1.10
    netmask 255.255.255.0
    mtu 9000
EOF
```

**Anode configuration for jumbo frames:**

```toml
[node]
# Use MTU 9000 interface for cluster communication
grpc_addr = "10.0.1.10:9001"  # eth1 with MTU 9000
```

## Operating System Tuning

### File Descriptor Limits

Increase limits to support many concurrent connections:

```bash
# Check current limits
ulimit -n

# System-wide limits
cat >> /etc/sysctl.conf <<EOF
fs.file-max = 1048576
fs.nr_open = 1048576
EOF

sysctl -p

# Per-user limits
cat >> /etc/security/limits.conf <<EOF
*               soft    nofile          65536
*               hard    nofile          65536
anode           soft    nofile          1048576
anode           hard    nofile          1048576
EOF

# For systemd services
mkdir -p /etc/systemd/system/anode.service.d/
cat > /etc/systemd/system/anode.service.d/limits.conf <<EOF
[Service]
LimitNOFILE=1048576
EOF

systemctl daemon-reload
systemctl restart anode
```

Verify:

```bash
# Check Anode process limits
cat /proc/$(pgrep anode)/limits | grep "open files"
```

### Virtual Memory Settings

Optimize for SSD storage and database workloads:

```bash
cat >> /etc/sysctl.conf <<EOF
# Reduce swappiness (prefer RAM over swap)
vm.swappiness = 10

# Dirty page thresholds (for write performance)
vm.dirty_ratio = 15                    # % of RAM before blocking writes
vm.dirty_background_ratio = 5          # % of RAM before background writeback

# Cache pressure (balance between page cache and slab)
vm.vfs_cache_pressure = 50            # Default 100, lower = prefer cache

# Memory allocation
vm.overcommit_memory = 1              # Always overcommit (Redis-style)
vm.overcommit_ratio = 100
EOF

sysctl -p
```

### Transparent Huge Pages

Disable THP to avoid latency spikes:

```bash
# Check current status
cat /sys/kernel/mm/transparent_hugepage/enabled

# Disable THP
echo never > /sys/kernel/mm/transparent_hugepage/enabled
echo never > /sys/kernel/mm/transparent_hugepage/defrag

# Make persistent (systemd)
cat > /etc/systemd/system/disable-thp.service <<EOF
[Unit]
Description=Disable Transparent Huge Pages
DefaultDependencies=no
After=sysinit.target local-fs.target
Before=anode.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'echo never > /sys/kernel/mm/transparent_hugepage/enabled'
ExecStart=/bin/sh -c 'echo never > /sys/kernel/mm/transparent_hugepage/defrag'

[Install]
WantedBy=basic.target
EOF

systemctl enable disable-thp
systemctl start disable-thp
```

### CPU Governor

Use performance governor for consistent latency:

```bash
# Check current governor
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Install cpufrequtils
apt-get install cpufrequtils  # Debian/Ubuntu
yum install cpupowerutils      # RHEL/CentOS

# Set to performance mode
cpupower frequency-set -g performance

# Verify
cpupower frequency-info

# Make persistent
cat >> /etc/default/cpufrequtils <<EOF
GOVERNOR="performance"
EOF

systemctl restart cpufrequtils
```

### I/O Scheduler

Choose the right I/O scheduler for your storage:

```bash
# Check current scheduler
cat /sys/block/sda/queue/scheduler

# For NVMe SSDs: none (multi-queue)
echo none > /sys/block/nvme0n1/queue/scheduler

# For SATA SSDs: mq-deadline or kyber
echo mq-deadline > /sys/block/sda/queue/scheduler

# For HDDs: bfq or mq-deadline
echo bfq > /sys/block/sda/queue/scheduler

# Make persistent (udev rule)
cat > /etc/udev/rules.d/60-scheduler.rules <<'EOF'
# NVMe SSDs
ACTION=="add|change", KERNEL=="nvme[0-9]n[0-9]", ATTR{queue/scheduler}="none"

# SATA SSDs
ACTION=="add|change", KERNEL=="sd[a-z]", ATTR{queue/rotational}=="0", ATTR{queue/scheduler}="mq-deadline"

# HDDs
ACTION=="add|change", KERNEL=="sd[a-z]", ATTR{queue/rotational}=="1", ATTR{queue/scheduler}="bfq"
EOF

udevadm control --reload-rules
udevadm trigger
```

## Parquet Workload Optimization

### Parquet-Specific Configuration

Optimize for analytics workloads:

```toml
[parquet]
# Enable all optimizations
enable_cache = true
enable_predicate_pushdown = true
enable_column_pruning = true

# Cache sizing (see calculation below)
cache_size = 536870912  # 512MB

# Row group size tuning
row_group_size = 100000  # default, adjust based on data
```

### Cache Size Calculation

```
Parquet Footer Size ≈ 10KB per file

For 10K Parquet files:
  10,000 × 10KB = 100MB

For 50K Parquet files:
  50,000 × 10KB = 500MB

For 100K Parquet files:
  100,000 × 10KB = 1GB
```

**Recommendations:**

```toml
# Small analytics workload (<10K Parquet files)
[parquet]
cache_size = 268435456  # 256MB

# Medium analytics workload (10K-50K files)
[parquet]
cache_size = 536870912  # 512MB

# Large analytics workload (>50K files)
[parquet]
cache_size = 1073741824  # 1GB
```

### Query Optimization

Leverage predicate pushdown and column pruning:

**Before Optimization:**

```python
import pyarrow.parquet as pq

# Reads entire file (inefficient)
table = pq.read_table('s3://bucket/data.parquet')
filtered = table.filter(pc.field('date') == '2024-01-15')
result = filtered.select(['col1', 'col2'])
```

**After Optimization:**

```python
import pyarrow.parquet as pq
import pyarrow.compute as pc

# Predicate pushdown + column pruning
table = pq.read_table(
    's3://bucket/data.parquet',
    filters=[('date', '=', '2024-01-15')],  # Pushdown: Skip row groups
    columns=['col1', 'col2', 'date']        # Pruning: Read only needed columns
)
result = table.select(['col1', 'col2'])
```

**Performance Comparison:**

| Method | Data Read | Time | Speedup |
|--------|-----------|------|---------|
| No optimization | 1.0 GB | 10s | 1× |
| Column pruning only | 300 MB | 3s | 3× |
| Predicate + pruning | 50 MB | 0.5s | 20× |

### Row Group Sizing

Align row groups with chunk size for optimal I/O:

```toml
[storage]
chunk_size = "8MB"  # 8MB chunks

[parquet]
row_group_size = 100000  # Aim for ~8MB row groups

# Calculate ideal row group size:
# row_group_size = chunk_size / (row_size × 1.5)
#
# Example: 8MB chunks, 50 bytes/row
#   = 8MB / (50 bytes × 1.5)
#   = 8,388,608 / 75
#   ≈ 111,000 rows
```

## Workload-Specific Configurations

### Small Object Workload (<1MB)

**Characteristics:**
- High request rate (>5K ops/sec)
- Latency-sensitive
- Random access pattern
- Examples: Image thumbnails, logs, small JSON documents

**Optimized Configuration:**

```toml
[storage]
chunk_size = "1MB"                      # Small chunks for small objects
metadata_cache_size = 1073741824        # 1GB (cache more objects)
io_threads = 16                         # High concurrency
sync_writes = false                     # Latency over durability

[cluster]
heartbeat_interval_ms = 1000            # Default
election_timeout_ms = 3000              # Default

[raft]
snapshot_interval = 50000               # Less frequent (high write rate)
max_log_entries = 200000

[parquet]
enable_cache = false                    # Not needed for small objects
```

**OS Tuning:**

```bash
# Optimize for high connection count
sysctl -w net.ipv4.tcp_fin_timeout=30
sysctl -w net.ipv4.tcp_tw_reuse=1
sysctl -w net.core.somaxconn=8192

# Increase file descriptor limit
ulimit -n 100000
```

**Expected Performance:**
- Throughput: 5,000-15,000 ops/sec per node
- Latency: P99 < 20ms
- Cache hit rate: >95%

### Large Object Workload (>100MB)

**Characteristics:**
- Throughput-bound
- Sequential access
- Multipart uploads
- Examples: Video files, database backups, scientific datasets

**Optimized Configuration:**

```toml
[storage]
chunk_size = "16MB"                     # Large chunks for large objects
metadata_cache_size = 268435456         # 256MB (fewer objects)
io_threads = 24                         # Maximize I/O parallelism
sync_writes = false

[s3]
max_body_size = 10737418240             # 10GB max upload
enable_multipart = true
min_part_size = 5242880                 # 5MB (S3 minimum)

[cluster]
replication_factor = 2                  # Consider 2× instead of 3× (less bandwidth)

[raft]
snapshot_interval = 10000               # Default
```

**OS Tuning:**

```bash
# Maximize TCP buffer sizes
sysctl -w net.core.rmem_max=268435456   # 256MB
sysctl -w net.core.wmem_max=268435456   # 256MB
sysctl -w net.ipv4.tcp_rmem="4096 131072 268435456"
sysctl -w net.ipv4.tcp_wmem="4096 131072 268435456"

# Enable TCP window scaling
sysctl -w net.ipv4.tcp_window_scaling=1

# Use BBR congestion control
sysctl -w net.ipv4.tcp_congestion_control=bbr
```

**Client-side optimization:**

```bash
# Use multipart uploads for large files
aws --endpoint-url http://localhost:9000 s3 cp \
    large-file.bin s3://bucket/ \
    --multipart-chunk-size-mb 100
```

**Expected Performance:**
- Throughput: Limited by network bandwidth
- 10GbE: ~1.2 GB/s (after replication overhead)
- 25GbE: ~3.0 GB/s
- Latency: Throughput-bound (not latency-sensitive)

### Parquet Analytics Workload

**Characteristics:**
- Read-heavy (95% reads)
- Column-oriented queries
- Predicate filtering
- Examples: Data warehouse, OLAP queries, BI dashboards

**Optimized Configuration:**

```toml
[storage]
chunk_size = "8MB"                      # Align with Parquet row groups
metadata_cache_size = 536870912         # 512MB
io_threads = 12

[parquet]
enable_cache = true
cache_size = 1073741824                 # 1GB Parquet cache
enable_predicate_pushdown = true
enable_column_pruning = true
row_group_size = 100000                 # ~8MB row groups

[cluster]
replication_factor = 2                  # Reads can tolerate lower replication
```

**Query Best Practices:**

```python
# ✓ Good: Use filters and column selection
table = pq.read_table(
    's3://warehouse/sales.parquet',
    filters=[
        ('year', '=', 2024),
        ('month', '>=', 6)
    ],
    columns=['product_id', 'revenue']
)

# ✗ Bad: Read entire file then filter
table = pq.read_table('s3://warehouse/sales.parquet')
result = table.filter(pc.field('year') == 2024)
```

**Expected Performance:**
- Query latency: 50-500ms (with cache)
- Data scanned: 10-50× less than full file
- Cache hit rate: >80% for warm queries

### Mixed Workload

**Characteristics:**
- Varied object sizes (1KB-1GB)
- Balanced read/write ratio (60/40)
- Unpredictable access patterns
- Examples: General-purpose storage, CMS, collaboration tools

**Balanced Configuration:**

```toml
[storage]
chunk_size = "4MB"                      # Balanced default
metadata_cache_size = 536870912         # 512MB
io_threads = 12                         # 1.5× CPU cores
sync_writes = false

[cluster]
replication_factor = 3                  # Standard redundancy
heartbeat_interval_ms = 1000
election_timeout_ms = 3000

[raft]
snapshot_interval = 20000               # Moderate
max_log_entries = 100000

[parquet]
enable_cache = true
cache_size = 268435456                  # 256MB
```

**Monitoring Focus:**

```bash
# Track workload distribution
curl -s http://localhost:9002/metrics | \
    grep -E 'anode_requests_total|anode_response_size'

# Adjust chunk_size based on dominant object size
# Adjust cache sizes based on hit rates
```

## Monitoring and Metrics

### Key Performance Indicators

Track these metrics to validate tuning:

**1. Request Metrics:**

```promql
# Request rate (ops/sec)
rate(anode_requests_total[5m])

# P95 latency (target: <100ms)
histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m]))

# P99 latency (target: <200ms)
histogram_quantile(0.99, rate(anode_request_duration_seconds_bucket[5m]))

# Error rate (target: <0.1%)
sum(rate(anode_requests_total{status=~"5.."}[5m])) /
sum(rate(anode_requests_total[5m])) * 100
```

**2. Storage Metrics:**

```promql
# Cache hit rate (target: >90%)
anode_metadata_cache_hit_ratio

# Disk utilization (alert: >80%)
(anode_disk_usage_bytes / (anode_disk_usage_bytes + anode_disk_available_bytes)) * 100

# Storage growth rate (MB/hour)
deriv(anode_storage_bytes_total[1h]) / 1024 / 1024
```

**3. Raft Metrics:**

```promql
# Replication lag (target: <5s)
anode_raft_replication_lag_seconds

# Leader elections (target: <1/hour)
rate(anode_raft_leader_changes_total[1h])

# Proposal failures (target: 0)
rate(anode_raft_proposal_failures_total[5m])
```

**4. Parquet Metrics:**

```promql
# Parquet cache hit rate (target: >80%)
rate(anode_parquet_cache_hits_total[5m]) /
(rate(anode_parquet_cache_hits_total[5m]) + rate(anode_parquet_cache_misses_total[5m]))

# Query performance
histogram_quantile(0.95, rate(anode_parquet_query_duration_seconds_bucket[5m]))
```

### Grafana Dashboard

Create a performance tuning dashboard:

```json
{
  "dashboard": {
    "title": "Anode Performance Tuning",
    "panels": [
      {
        "title": "Throughput (ops/sec)",
        "targets": [
          {
            "expr": "sum(rate(anode_requests_total[5m]))"
          }
        ]
      },
      {
        "title": "Latency Percentiles",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, rate(anode_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p50"
          },
          {
            "expr": "histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p95"
          },
          {
            "expr": "histogram_quantile(0.99, rate(anode_request_duration_seconds_bucket[5m]))",
            "legendFormat": "p99"
          }
        ]
      },
      {
        "title": "Cache Hit Rates",
        "targets": [
          {
            "expr": "anode_metadata_cache_hit_ratio",
            "legendFormat": "Metadata Cache"
          },
          {
            "expr": "rate(anode_parquet_cache_hits_total[5m]) / (rate(anode_parquet_cache_hits_total[5m]) + rate(anode_parquet_cache_misses_total[5m]))",
            "legendFormat": "Parquet Cache"
          }
        ]
      }
    ]
  }
}
```

Import this dashboard in Grafana to track tuning progress.

## Performance Troubleshooting

### High Latency Diagnosis

**Symptom:** Request latency >100ms (P95)

**Diagnostic Steps:**

```bash
# 1. Check disk I/O latency
iostat -x 1 5
# Look for: %util >80%, await >10ms

# 2. Check network latency
ping -c 100 node2.cluster.local
# Look for: avg >10ms, mdev >5ms

# 3. Check CPU usage
top -bn1 | grep anode
# Look for: %CPU >80%

# 4. Check Raft replication lag
curl -s http://localhost:9002/metrics | grep replication_lag
# Look for: >5 seconds

# 5. Check cache hit rates
curl -s http://localhost:9002/metrics | grep cache_hit_ratio
# Look for: <80%
```

**Solutions by Root Cause:**

| Root Cause | Solution |
|------------|----------|
| Disk I/O slow (await >10ms) | Upgrade to NVMe SSD, increase io_threads |
| Network latency high | Increase heartbeat_interval_ms, use faster network |
| CPU maxed out | Reduce io_threads, disable compression, add nodes |
| Replication lag high | Increase heartbeat_interval_ms, reduce write rate |
| Cache hit rate low | Increase metadata_cache_size or parquet.cache_size |

### Low Throughput Diagnosis

**Symptom:** Throughput <1,000 ops/sec

**Diagnostic Steps:**

```bash
# 1. Check concurrent connections
netstat -an | grep :9000 | grep ESTABLISHED | wc -l
# Look for: <10 connections (client-side bottleneck)

# 2. Check thread utilization
ps -eLf | grep anode | wc -l
# Compare to: io_threads setting

# 3. Check resource limits
cat /proc/$(pgrep anode)/limits
# Look for: low "Max open files"

# 4. Check disk throughput
iostat -x 1 5
# Look for: MB/s < disk capability

# 5. Monitor request queue depth
curl -s http://localhost:9002/metrics | grep active_requests
# Look for: always 0 or always maxed
```

**Solutions:**

| Root Cause | Solution |
|------------|----------|
| Low client concurrency | Increase concurrent clients, use connection pooling |
| Thread starvation | Increase io_threads |
| File descriptor limit | Increase ulimit and fs.file-max |
| Disk bandwidth limit | Upgrade storage, use RAID 0, add nodes |
| Request queueing | Increase io_threads, add nodes |

### Memory Pressure Diagnosis

**Symptom:** High memory usage, OOM kills

**Diagnostic Steps:**

```bash
# 1. Check overall memory usage
free -h
# Look for: low "available" memory

# 2. Check Anode process memory
ps aux | grep anode
# Look for: RSS >80% of total RAM

# 3. Check cache sizes
curl -s http://localhost:9002/metrics | grep cache_size_bytes
# Sum all cache sizes

# 4. Check Raft log size
curl -s http://localhost:9002/metrics | grep raft_log_size
# Look for: continuously growing

# 5. Check for memory leaks
# Monitor RSS over time - should stabilize
for i in {1..60}; do
    ps aux | grep anode | awk '{print $6}'
    sleep 60
done
```

**Solutions:**

| Root Cause | Solution |
|------------|----------|
| Caches too large | Reduce metadata_cache_size and parquet.cache_size |
| Raft log growing | Decrease snapshot_interval, check compaction |
| Too many connections | Limit max connections, use connection pooling |
| Memory leak | Update to latest version, report bug |
| Insufficient RAM | Add more RAM, reduce caches, add nodes |

### Cluster Stability Issues

**Symptom:** Frequent leader elections, split-brain

**Diagnostic Steps:**

```bash
# 1. Check leader election rate
curl -s http://localhost:9002/metrics | grep leader_changes_total
# Look for: increasing rapidly

# 2. Check inter-node latency
for node in node1 node2 node3; do
    echo "$node:"
    ping -c 20 $node | tail -1
done
# Look for: high variance, packet loss

# 3. Check network partitions
# From each node, check connectivity to others
for node in node1 node2 node3; do
    nc -zv $node 9001
done

# 4. Check Raft term
curl -s http://localhost:9002/metrics | grep raft_term
# Look for: rapidly incrementing
```

**Solutions:**

| Root Cause | Solution |
|------------|----------|
| Network instability | Fix network issues, increase election_timeout_ms |
| High inter-node latency | Increase heartbeat_interval_ms and election_timeout_ms |
| Node under load | Reduce load, add nodes, increase resources |
| Split-brain scenario | Fix network partition, ensure quorum |

## Benchmarking and Validation

### Establishing Baseline

Before tuning, establish a performance baseline:

```bash
# 1. Run comprehensive benchmark suite
cd benches
./run_benchmarks.sh

# Results saved to benches/results/<timestamp>/
# View: target/criterion/report/index.html

# 2. Run external S3 benchmark
go install github.com/wasabi-tech/s3-benchmark@latest

s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket baseline \
    -numClients 16 \
    -numSamples 1000 \
    -objectSize 1M \
    -objectNamePrefix baseline_ \
    > baseline_results.txt

# 3. Record system metrics
curl -s http://localhost:9002/metrics > baseline_metrics.txt

# 4. Document configuration
cp /etc/anode/config.toml baseline_config.toml
```

### Comparing Before/After

After each tuning change:

```bash
# 1. Run same benchmarks
s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket tuned \
    -numClients 16 \
    -numSamples 1000 \
    -objectSize 1M \
    -objectNamePrefix tuned_ \
    > tuned_results.txt

# 2. Compare results
echo "=== Baseline ==="
grep -E 'Throughput|Latency' baseline_results.txt

echo "=== After Tuning ==="
grep -E 'Throughput|Latency' tuned_results.txt

# 3. Calculate improvement
python3 <<EOF
import re

def parse_metric(file, metric):
    with open(file) as f:
        for line in f:
            if metric in line:
                return float(re.search(r'[\d.]+', line).group())
    return 0

baseline_tput = parse_metric('baseline_results.txt', 'Throughput')
tuned_tput = parse_metric('tuned_results.txt', 'Throughput')

improvement = ((tuned_tput - baseline_tput) / baseline_tput) * 100
print(f"Throughput improvement: {improvement:.1f}%")
EOF
```

### Stress Testing

Validate tuning under load:

```bash
# 1. Sustained load test (1 hour)
s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket stress \
    -numClients 32 \
    -numSamples 100000 \
    -objectSize 1M \
    -duration 3600

# 2. Monitor metrics during test
watch -n 5 "curl -s http://localhost:9002/metrics | grep -E 'requests_total|duration_seconds|cache_hit'"

# 3. Verify no errors
curl -s http://localhost:9002/metrics | grep error_total

# 4. Check for memory leaks
for i in {1..60}; do
    echo "$(date): $(ps aux | grep anode | awk '{print $6}')"
    sleep 60
done > memory_over_time.txt

# Plot memory usage (should be stable)
gnuplot <<EOF
set terminal png
set output 'memory_usage.png'
plot 'memory_over_time.txt' using 2 with lines title 'Memory (KB)'
EOF
```

### Load Testing Tools

**Option 1: s3-benchmark (Simple)**

```bash
# Installation
go install github.com/wasabi-tech/s3-benchmark@latest

# Basic test
s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket loadtest

# Custom test
s3-benchmark \
    -endpoint http://localhost:9000 \
    -accessKey minioadmin \
    -secretKey minioadmin \
    -bucket loadtest \
    -numClients 64 \
    -numSamples 10000 \
    -objectSize 10M \
    -objectNamePrefix test_
```

**Option 2: warp (Advanced)**

```bash
# Installation
wget https://github.com/minio/warp/releases/latest/download/warp_Linux_x86_64.tar.gz
tar xzf warp_Linux_x86_64.tar.gz

# Mixed workload test
./warp mixed \
    --host localhost:9000 \
    --access-key minioadmin \
    --secret-key minioadmin \
    --bucket=warptest \
    --duration=5m \
    --concurrent=32

# Analyze results
./warp analyze warp-mixed-*.csv
```

**Option 3: Custom benchmark script**

```bash
#!/bin/bash
# custom_benchmark.sh

ENDPOINT="http://localhost:9000"
BUCKET="perftest"
CLIENTS=16
DURATION=300  # 5 minutes

echo "Starting benchmark: $CLIENTS clients, ${DURATION}s duration"

# Create bucket
aws --endpoint-url $ENDPOINT s3 mb s3://$BUCKET 2>/dev/null || true

# Prepare test data
dd if=/dev/urandom of=test_1mb.bin bs=1M count=1 2>/dev/null

START=$(date +%s)
END=$((START + DURATION))
COUNT=0

# Run clients in parallel
for i in $(seq 1 $CLIENTS); do
    (
        while [ $(date +%s) -lt $END ]; do
            aws --endpoint-url $ENDPOINT s3 cp \
                test_1mb.bin s3://$BUCKET/test_${i}_${COUNT}.bin \
                --no-progress 2>/dev/null
            ((COUNT++))
        done
    ) &
done

wait

ELAPSED=$(($(date +%s) - START))
TOTAL_OPS=$((COUNT * CLIENTS))
OPS_PER_SEC=$((TOTAL_OPS / ELAPSED))

echo "Total operations: $TOTAL_OPS"
echo "Throughput: $OPS_PER_SEC ops/sec"
echo "Avg latency: $((ELAPSED * 1000 / TOTAL_OPS)) ms"
```

## Best Practices and Recommendations

### Configuration Management

**1. Version Control:**

```bash
# Store configuration in git
mkdir /etc/anode/config-versions
cd /etc/anode/config-versions
git init

# Track changes
cp /etc/anode/config.toml ./config.toml
git add config.toml
git commit -m "Baseline configuration"

# After tuning
cp /etc/anode/config.toml ./config.toml
git add config.toml
git commit -m "Increased metadata_cache_size to 1GB for better hit rate"
```

**2. Configuration Templates:**

Create templates for common workloads:

```bash
/etc/anode/
├── config.toml -> profiles/production.toml
├── profiles/
│   ├── development.toml
│   ├── production.toml
│   ├── small-objects.toml
│   ├── large-objects.toml
│   └── parquet-analytics.toml
```

**3. Automated Validation:**

```bash
#!/bin/bash
# validate_config.sh

CONFIG="/etc/anode/config.toml"

# Check for required parameters
for param in chunk_size metadata_cache_size io_threads; do
    if ! grep -q "$param" $CONFIG; then
        echo "ERROR: Missing required parameter: $param"
        exit 1
    fi
done

# Validate chunk size is power of 2
CHUNK_SIZE=$(grep chunk_size $CONFIG | grep -oP '\d+')
if ! (( CHUNK_SIZE && !(CHUNK_SIZE & (CHUNK_SIZE - 1)) )); then
    echo "WARNING: chunk_size should be power of 2"
fi

# Validate election timeout > heartbeat * 3
HEARTBEAT=$(grep heartbeat_interval_ms $CONFIG | grep -oP '\d+')
ELECTION=$(grep election_timeout_ms $CONFIG | grep -oP '\d+')
if (( ELECTION < HEARTBEAT * 3 )); then
    echo "ERROR: election_timeout_ms must be >= heartbeat_interval_ms * 3"
    exit 1
fi

echo "Configuration valid"
```

### Capacity Planning

**Predictive Scaling:**

```bash
#!/bin/bash
# capacity_forecast.sh

# Collect storage growth data
curl -s http://localhost:9002/metrics | \
    grep anode_storage_bytes_total | \
    awk '{print $2}' > storage_$(date +%s).txt

# Analyze growth trend (requires multiple samples over time)
python3 <<'EOF'
import glob
import numpy as np
from datetime import datetime, timedelta

files = sorted(glob.glob('storage_*.txt'))
if len(files) < 2:
    print("Need at least 2 samples")
    exit(1)

timestamps = [int(f.split('_')[1].split('.')[0]) for f in files]
sizes = [int(open(f).read()) for f in files]

# Linear regression
coeffs = np.polyfit(timestamps, sizes, 1)
growth_per_second = coeffs[0]

# Project 30 days
current_size = sizes[-1]
future_size = current_size + (growth_per_second * 30 * 86400)

print(f"Current size: {current_size / 1e9:.2f} GB")
print(f"Growth rate: {growth_per_second * 86400 / 1e9:.2f} GB/day")
print(f"Projected (30d): {future_size / 1e9:.2f} GB")

# Alert if >80% capacity in 30 days
TOTAL_CAPACITY = 1e12  # 1TB
if future_size > TOTAL_CAPACITY * 0.8:
    print(f"WARNING: Will reach 80% capacity in 30 days")
    print(f"Action: Add storage capacity")
EOF
```

### Performance Regression Testing

Automate performance testing in CI/CD:

```yaml
# .github/workflows/performance.yml
name: Performance Tests

on:
  pull_request:
  schedule:
    - cron: '0 2 * * *'  # Nightly at 2 AM

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Run benchmarks
        run: |
          cd benches
          ./run_benchmarks.sh

      - name: Compare with baseline
        run: |
          cd benches
          python3 compare.py \
            --baseline results/baseline \
            --current results/latest \
            --threshold 5  # Fail if >5% regression

      - name: Upload results
        uses: actions/upload-artifact@v2
        with:
          name: benchmark-results
          path: benches/results/latest/
```

### Documentation

Document all tuning decisions:

```markdown
# Performance Tuning Log

## 2024-01-15: Metadata Cache Increase

**Change:** Increased metadata_cache_size from 100MB to 1GB

**Reason:** Cache hit rate was 65%, target is >90%

**Impact:**
- Cache hit rate: 65% → 93% (+43%)
- P95 latency: 85ms → 45ms (-47%)
- Memory usage: +900MB

**Validated by:**
- Benchmark run: results/20240115_141530
- 24h monitoring: No issues

**Approved by:** @performance-team

---

## 2024-01-20: Chunk Size Optimization

**Change:** Increased chunk_size from 4MB to 8MB

**Reason:** 90% of objects are >100MB (large file workload)

**Impact:**
- Throughput: 450 MB/s → 620 MB/s (+38%)
- Deduplication ratio: 1.5x → 1.2x (-20%)
- Trade-off acceptable for this workload

**Validated by:**
- Benchmark run: results/20240120_093022
- 7d monitoring: No issues

**Approved by:** @storage-team
```

### Checklist: Production Readiness

Before deploying tuned configuration:

- [ ] Baseline benchmarks completed and documented
- [ ] All tuning changes tested individually
- [ ] Combined configuration tested under load
- [ ] No performance regressions vs. baseline
- [ ] Memory usage within limits (<80% RAM)
- [ ] Disk usage projections calculated
- [ ] Monitoring dashboards configured
- [ ] Alerts configured for anomalies
- [ ] Configuration backed up and version controlled
- [ ] Rollback plan documented
- [ ] Team trained on new configuration
- [ ] Runbook updated with new settings

## Summary

### Key Takeaways

1. **Measure First:** Always establish a baseline before tuning
2. **One Change at a Time:** Isolate the impact of each change
3. **Monitor Continuously:** Track metrics before, during, and after changes
4. **Document Everything:** Record all changes and their rationale
5. **Test Under Load:** Validate tuning with realistic workloads

### Quick Wins

These changes provide immediate improvements with minimal risk:

1. Increase metadata cache to 1GB
2. Enable all Parquet optimizations
3. Optimize OS file descriptor limits
4. Tune TCP buffers for your network
5. Set CPU governor to performance

### Common Pitfalls

Avoid these mistakes:

- Changing multiple parameters simultaneously
- Not monitoring after changes
- Ignoring workload characteristics
- Over-optimizing for benchmarks vs. production
- Neglecting capacity planning

### Next Steps

1. Review your workload profile
2. Select appropriate configuration template
3. Apply tuning incrementally
4. Benchmark and monitor
5. Iterate based on results

### Additional Resources

- [Anode Performance Guide](performance.md) - Overview and expected performance
- [Anode Monitoring Guide](monitoring.md) - Comprehensive monitoring setup
- [Benchmark Suite README](../benches/README.md) - Detailed benchmark documentation
- [Configuration Reference](configuration.md) - All configuration options

For questions or issues, please open a GitHub issue or consult the documentation.
