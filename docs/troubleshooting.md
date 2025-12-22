# Troubleshooting Guide

This comprehensive guide will help you diagnose and resolve common issues with Anode deployments. For each problem, you'll find symptoms, possible causes, diagnostic commands, and solutions.

## Table of Contents

- [Reading Logs Effectively](#reading-logs-effectively)
- [Using Metrics for Diagnosis](#using-metrics-for-diagnosis)
- [Common Error Messages](#common-error-messages)
- [Server Startup Problems](#server-startup-problems)
- [Connection and Networking Issues](#connection-and-networking-issues)
- [Authentication Failures](#authentication-failures)
- [Storage Errors and Disk Issues](#storage-errors-and-disk-issues)
- [Cluster Formation Problems](#cluster-formation-problems)
- [Raft Consensus Issues](#raft-consensus-issues)
- [Memory and Performance Problems](#memory-and-performance-problems)
- [S3 API Compatibility Issues](#s3-api-compatibility-issues)
- [Data Integrity Issues](#data-integrity-issues)
- [Emergency Procedures](#emergency-procedures)

## Reading Logs Effectively

### Log Configuration

Anode supports both plain text and JSON logging formats. For production deployments, JSON format is recommended for integration with log aggregation tools.

**Configure logging in `config.toml`:**

```toml
[logging]
level = "info"              # Options: trace, debug, info, warn, error
json_format = true          # Use JSON for structured logging
file = "/var/log/anode/anode.log"
rotate = true
max_size = 104857600        # 100MB
```

**Per-module log levels:**

```toml
[logging]
level = "info"
# Enable debug logging for specific modules
module_levels = "anode_storage=debug,anode_raft=debug"
```

**Environment variable override:**

```bash
RUST_LOG=anode=debug,anode_raft=trace anode --config /etc/anode/config.toml
```

### Viewing Logs

**With systemd:**

```bash
# Follow logs in real-time
journalctl -u anode -f

# Last 100 lines
journalctl -u anode -n 100

# Logs from the last hour
journalctl -u anode --since "1 hour ago"

# Filter by priority (errors only)
journalctl -u anode -p err

# Export to file
journalctl -u anode --since "2023-12-01" > anode.log
```

**With Docker:**

```bash
# Follow logs
docker logs -f anode-node1

# Last 100 lines with timestamps
docker logs --tail 100 --timestamps anode-node1

# Filter using grep
docker logs anode-node1 2>&1 | grep ERROR
```

**With Kubernetes:**

```bash
# Follow logs
kubectl logs -f anode-0

# Previous container (after restart)
kubectl logs anode-0 --previous

# All pods with label
kubectl logs -l app=anode --all-containers=true
```

### Important Log Patterns

**Critical errors requiring immediate attention:**

```bash
# Storage corruption
grep -i "corrupted\|checksum.*mismatch" /var/log/anode/anode.log

# Raft consensus failures
grep -i "no leader\|election.*failed\|split.*brain" /var/log/anode/anode.log

# Database errors
grep -i "database.*error\|redb.*error" /var/log/anode/anode.log

# Out of disk space
grep -i "no space left\|disk.*full" /var/log/anode/anode.log
```

**Performance warnings:**

```bash
# Slow operations
grep "duration.*[0-9]\{4,\}ms" /var/log/anode/anode.log

# High replication lag
grep "replication.*lag" /var/log/anode/anode.log
```

### JSON Log Parsing

With JSON formatted logs, use `jq` for powerful filtering:

```bash
# Filter by log level
cat anode.log | jq 'select(.level == "ERROR")'

# Filter by module
cat anode.log | jq 'select(.target | startswith("anode_storage"))'

# Extract error messages
cat anode.log | jq -r 'select(.level == "ERROR") | .message'

# Count errors by type
cat anode.log | jq -r 'select(.level == "ERROR") | .message' | sort | uniq -c | sort -rn
```

## Using Metrics for Diagnosis

Anode exposes Prometheus metrics at the admin endpoint (default: `:9002/metrics`).

### Quick Health Checks

**Check if metrics endpoint is accessible:**

```bash
curl http://localhost:9002/metrics
```

**Key metrics to check:**

```bash
# Is node alive?
curl -s http://localhost:9002/metrics | grep "anode_uptime_seconds"

# Current Raft leader status (1 = leader, 0 = follower)
curl -s http://localhost:9002/metrics | grep "anode_raft_is_leader"

# Request error rate
curl -s http://localhost:9002/metrics | grep "anode_request_errors_total"

# Storage usage
curl -s http://localhost:9002/metrics | grep "anode_storage_bytes_total"

# Disk space available
curl -s http://localhost:9002/metrics | grep "anode_disk_available_bytes"
```

### Health Check Endpoints

**Liveness probe (is the process running?):**

```bash
curl http://localhost:9002/health/live
```

Returns:
- `200 OK` - Process is alive
- `503 Service Unavailable` - Process is not responding

**Readiness probe (is the node ready to serve traffic?):**

```bash
curl http://localhost:9002/health/ready
```

Returns:
- `200 OK` - Node is ready (has Raft leader, storage accessible)
- `503 Service Unavailable` - Node is not ready

**Detailed health information:**

```bash
curl http://localhost:9002/health | jq
```

Example output:

```json
{
  "storage": {
    "healthy": true,
    "message": "Storage engine operational",
    "details": { "engine": "operational" }
  },
  "raft": {
    "healthy": true,
    "message": "Leader",
    "details": {
      "node_id": 1,
      "current_term": 5,
      "current_leader": 1,
      "is_leader": true
    }
  },
  "cluster": {
    "healthy": true,
    "message": "3/3 nodes healthy",
    "details": {
      "total_nodes": 3,
      "healthy_nodes": 3,
      "leader_id": 1
    }
  }
}
```

### Diagnostic Queries

**Using PromQL (in Prometheus UI or CLI):**

```promql
# Request latency (95th percentile)
histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m]))

# Error rate
sum(rate(anode_request_errors_total[5m]))

# Storage growth rate (bytes per second)
deriv(anode_storage_bytes_total[1h])

# Replication lag
max(anode_cluster_replication_lag_seconds) by (peer_id)

# Disk space percentage used
(anode_disk_usage_bytes / (anode_disk_usage_bytes + anode_disk_available_bytes)) * 100
```

## Common Error Messages

### Storage Errors

| Error Message | Meaning | Solution |
|--------------|---------|----------|
| `Checksum mismatch: expected X, got Y` | Data corruption detected | Run scrub to identify corrupted chunks, then repair from replicas |
| `Database error: Lock is held by another process` | Another Anode process is using the data directory | Stop the conflicting process or change data directory |
| `IO error: No space left on device` | Disk is full | Free up disk space or add storage capacity |
| `Not found: chunk/{id}` | Missing chunk (possible data loss) | Check replica status, restore from backup if needed |
| `Bucket not found: {name}` | Bucket does not exist | Create bucket or check bucket name |
| `Object not found: {bucket}/{key}` | Object does not exist | Verify object path is correct |

### Raft Errors

| Error Message | Meaning | Solution |
|--------------|---------|----------|
| `No leader elected` | Cluster has no leader | Check network connectivity, review logs for election failures |
| `Not leader, current leader: {id}` | Write attempted on non-leader | Redirect request to leader node |
| `Timeout waiting for operation` | Operation took too long | Check network latency, increase timeout values |
| `Network error: connection refused` | Cannot connect to peer node | Verify peer is running and network is accessible |
| `Storage error: {details}` | Raft log storage issue | Check disk space and database health |

### S3 API Errors

| Error Code | HTTP Status | Meaning | Solution |
|-----------|-------------|---------|----------|
| `NoSuchBucket` | 404 | Bucket does not exist | Create bucket first |
| `NoSuchKey` | 404 | Object does not exist | Verify object key is correct |
| `BucketAlreadyExists` | 409 | Bucket name already taken | Use a different bucket name |
| `InvalidBucketName` | 400 | Bucket name violates naming rules | Use lowercase, alphanumeric, hyphens only |
| `SignatureDoesNotMatch` | 403 | Authentication signature invalid | Check access key, secret key, and clock sync |
| `AccessDenied` | 403 | Permission denied | Verify credentials and permissions |
| `EntityTooSmall` | 400 | Multipart part too small | Parts must be >= 5MB (except last part) |
| `NoSuchUpload` | 404 | Multipart upload not found | Upload may have expired or been aborted |

### Configuration Errors

| Error Message | Meaning | Solution |
|--------------|---------|----------|
| `Failed to parse config` | TOML syntax error | Validate TOML syntax, check for typos |
| `Missing required field: {field}` | Required config missing | Add the required configuration field |
| `Invalid configuration: {details}` | Config value invalid | Review config documentation, fix invalid values |
| `Failed to read config file` | Cannot read file | Check file exists and has correct permissions |

## Server Startup Problems

### Symptom: Server Won't Start

**Check logs for errors:**

```bash
journalctl -u anode -n 50
```

**Common causes and solutions:**

#### Port Already in Use

**Error:**
```
Error binding to 0.0.0.0:9000: Address already in use
```

**Diagnosis:**
```bash
# Find process using the port
sudo lsof -i :9000
# Or with ss
ss -tlnp | grep :9000
```

**Solution:**
```bash
# Stop conflicting process
sudo kill <PID>

# Or change Anode port in config
[node]
s3_addr = "0.0.0.0:9001"
```

#### Permission Denied

**Error:**
```
IO error: Permission denied (os error 13)
```

**Diagnosis:**
```bash
# Check data directory permissions
ls -la /var/lib/anode/data

# Check if running as correct user
ps aux | grep anode
```

**Solution:**
```bash
# Fix ownership
sudo chown -R anode:anode /var/lib/anode/data

# Or run with correct user
sudo -u anode anode --config /etc/anode/config.toml
```

#### Data Directory Already in Use

**Error:**
```
Database error: Lock is held by another process
```

**Solution:**
```bash
# Stop all Anode processes
sudo systemctl stop anode
# Or find and kill processes
sudo pkill -9 anode

# Verify no processes remain
ps aux | grep anode

# Restart
sudo systemctl start anode
```

#### Invalid Configuration

**Error:**
```
Invalid configuration: replication_factor must be <= total nodes
```

**Solution:**
```bash
# Edit config
vim /etc/anode/config.toml

# For single node, use replication_factor = 1
[cluster]
replication_factor = 1

# Validate config
anode --config /etc/anode/config.toml --validate
```

### Symptom: Server Starts But Immediately Exits

**Check exit code:**

```bash
systemctl status anode
# Look for "Main process exited, code=exited, status=1/FAILURE"
```

**Common causes:**

1. **Panic during initialization** - Check logs for stack traces
2. **Missing dependencies** - Verify all required libraries are installed
3. **Corrupted data directory** - Try starting with fresh data directory
4. **Configuration validation failure** - Review all config values

**Solution:**
```bash
# Run in foreground to see errors
anode --config /etc/anode/config.toml

# Enable debug logging
RUST_LOG=debug anode --config /etc/anode/config.toml
```

## Connection and Networking Issues

### Symptom: Cannot Connect to S3 API

**Test connectivity:**

```bash
# Basic connectivity
curl http://localhost:9000/

# Should return S3 error XML (since no bucket specified)
curl http://localhost:9000/nonexistent-bucket

# From remote host
curl http://anode-server:9000/
```

**Common causes:**

#### Firewall Blocking Connections

**Diagnosis:**
```bash
# Check firewall rules
sudo iptables -L -n | grep 9000

# Check if port is listening
sudo netstat -tlnp | grep 9000
```

**Solution:**
```bash
# Allow port through firewall
sudo iptables -A INPUT -p tcp --dport 9000 -j ACCEPT

# For firewalld
sudo firewall-cmd --add-port=9000/tcp --permanent
sudo firewall-cmd --reload

# For UFW
sudo ufw allow 9000/tcp
```

#### Wrong Bind Address

**Problem:** Server bound to localhost only

**Check config:**
```toml
[node]
s3_addr = "127.0.0.1:9000"  # Wrong - only localhost can connect
```

**Solution:**
```toml
[node]
s3_addr = "0.0.0.0:9000"     # Correct - all interfaces
```

### Symptom: Cannot Connect to Admin/Metrics API

**Test:**
```bash
curl http://localhost:9002/health
curl http://localhost:9002/metrics
```

**If connection refused:**

1. Check admin_addr in config
2. Verify port is not blocked by firewall
3. Confirm service is running: `systemctl status anode`

### Symptom: Inter-Node Communication Failures

**Error in logs:**
```
Network error: connection refused
```

**Diagnosis:**

```bash
# Test gRPC connectivity between nodes
# From node 2, test connection to node 1
curl http://node1:9001/health

# Check if gRPC port is listening
nc -zv node1 9001

# Test name resolution
ping node1
dig node1
```

**Common causes:**

1. **Firewall blocking gRPC port (default: 9001)**
   ```bash
   sudo iptables -A INPUT -p tcp --dport 9001 -j ACCEPT
   ```

2. **DNS resolution failing**
   - Add entries to `/etc/hosts`
   - Configure proper DNS server

3. **Network partition**
   - Check network connectivity with `ping`, `traceroute`
   - Review switch/router configuration

4. **Wrong gRPC address in config**
   ```toml
   [node]
   grpc_addr = "0.0.0.0:9001"  # Listen on all interfaces
   ```

### Symptom: High Network Latency

**Diagnosis:**

```bash
# Measure RTT between nodes
curl -s http://localhost:9002/metrics | grep "anode_cluster_peer_rtt_seconds"

# Ping test
ping -c 100 node1

# Advanced network testing
iperf3 -s  # On one node
iperf3 -c node1  # On another node
```

**Solutions:**

1. **Increase timeout values:**
   ```toml
   [cluster]
   heartbeat_interval_ms = 2000
   election_timeout_ms = 6000
   ```

2. **Use dedicated network for cluster traffic**
3. **Check for network congestion**
4. **Reduce load on network (QoS, traffic shaping)**

## Authentication Failures

### Symptom: SignatureDoesNotMatch Error

**Error from AWS CLI:**
```
An error occurred (SignatureDoesNotMatch) when calling the PutObject operation:
The request signature we calculated does not match the signature you provided.
```

**Common causes:**

#### Clock Skew

AWS Signature V4 requires accurate time synchronization (within 15 minutes).

**Check time difference:**
```bash
# On client
date -u

# On server
ssh anode-server date -u
```

**Solution:**
```bash
# Install and enable NTP
sudo apt-get install ntp
sudo systemctl start ntp
sudo systemctl enable ntp

# Or use systemd-timesyncd
sudo timedatectl set-ntp true

# Verify sync
timedatectl status
```

#### Incorrect Credentials

**Solution:**
```bash
# Verify AWS credentials
aws configure list

# Test with correct credentials
export AWS_ACCESS_KEY_ID=your-access-key
export AWS_SECRET_ACCESS_KEY=your-secret-key

aws --endpoint-url http://localhost:9000 s3 ls
```

#### Wrong Endpoint URL

**Problem:**
```bash
# Wrong - uses AWS S3, not Anode
aws s3 ls s3://mybucket
```

**Solution:**
```bash
# Correct - specify endpoint
aws --endpoint-url http://localhost:9000 s3 ls s3://mybucket
```

### Symptom: AccessDenied Error

**Error:**
```xml
<Error>
  <Code>AccessDenied</Code>
  <Message>Access Denied</Message>
</Error>
```

**Solutions:**

1. **Verify credentials are valid**
2. **Check user has permission for operation**
3. **Review Anode access control configuration**

## Storage Errors and Disk Issues

### Symptom: Out of Disk Space

**Error:**
```
IO error: No space left on device (os error 28)
```

**Immediate actions:**

```bash
# Check disk usage
df -h /var/lib/anode/data

# Find large files
du -sh /var/lib/anode/data/*

# Make node read-only (stop accepting writes)
curl -X POST http://localhost:9002/admin/readonly
```

**Solutions:**

#### Free Up Space

```bash
# Delete old/unused buckets
aws --endpoint-url http://localhost:9000 s3 rb s3://old-bucket --force

# Run garbage collection
curl -X POST http://localhost:9002/admin/gc

# Check space freed
df -h /var/lib/anode/data
```

#### Add More Storage

```bash
# Mount additional volume
sudo mkfs.ext4 /dev/sdb
sudo mount /dev/sdb /var/lib/anode/data2

# Update config to use new path
[storage]
data_dir = "/var/lib/anode/data2"

# Or extend existing volume (LVM)
sudo lvextend -L +100G /dev/vg0/anode-data
sudo resize2fs /dev/vg0/anode-data
```

#### Add More Nodes

```bash
# Scale cluster horizontally
# Data will automatically rebalance to new nodes
```

### Symptom: Slow Storage Performance

**Diagnosis:**

```bash
# Check I/O wait
top
# Look for high %wa

# Detailed I/O stats
iostat -x 1

# Check disk latency from metrics
curl -s http://localhost:9002/metrics | grep "anode_operation_duration_seconds"
```

**Common causes:**

1. **Disk is overloaded**
   - Check `iostat` for high utilization
   - Consider faster storage (NVMe SSD)

2. **sync_writes=true causing slowdown**
   ```toml
   [storage]
   sync_writes = false  # Better performance, slightly less durable
   ```

3. **Small chunk_size for large files**
   ```toml
   [storage]
   chunk_size = "8MB"  # Increase for large files
   ```

4. **Insufficient I/O threads**
   ```toml
   [storage]
   io_threads = 16  # Increase for I/O-heavy workload
   ```

### Symptom: Data Corruption Detected

**Error:**
```
Checksum mismatch: expected abc123, got def456
```

**Diagnosis:**

```bash
# Run scrub to find all corrupted chunks
curl -X POST http://localhost:9002/admin/scrub

# Check scrub results
curl http://localhost:9002/admin/scrub/results | jq
```

**Automatic repair:**

```bash
# Repair from replicas
curl -X POST http://localhost:9002/admin/repair

# Monitor repair progress
watch curl -s http://localhost:9002/admin/repair/status
```

**If repair fails:**

1. **Restore from backup**
2. **Check disk health** - `smartctl -a /dev/sda`
3. **Review system logs for hardware errors** - `dmesg | grep error`

## Cluster Formation Problems

### Symptom: Single Node Won't Bootstrap

**Error:**
```
Failed to bootstrap cluster
```

**Solution:**

```bash
# Bootstrap single node
anode \
    --node-id 1 \
    --s3-addr 0.0.0.0:9000 \
    --grpc-addr 0.0.0.0:9001 \
    --admin-addr 0.0.0.0:9002 \
    --data-dir /var/lib/anode/data \
    --config /etc/anode/config.toml \
    --bootstrap

# Verify cluster formed
curl http://localhost:9002/admin/cluster | jq
```

**Config for single node:**

```toml
[cluster]
initial_members = []  # Empty for single node
replication_factor = 1
```

### Symptom: Node Cannot Join Cluster

**Error:**
```
Join request failed: connection refused
```

**Diagnosis:**

```bash
# Verify bootstrap node is accessible
curl http://node1:9001/health

# Check bootstrap node has formed cluster
curl http://node1:9002/admin/cluster | jq

# Test network connectivity
nc -zv node1 9001
```

**Solutions:**

1. **Ensure bootstrap node is running**
   ```bash
   ssh node1 systemctl status anode
   ```

2. **Verify gRPC address is correct**
   ```bash
   # Join command should use gRPC address
   anode --join node1:9001  # Correct
   anode --join node1:9000  # Wrong (S3 port)
   ```

3. **Check firewall allows gRPC traffic**
   ```bash
   sudo iptables -A INPUT -p tcp --dport 9001 -j ACCEPT
   ```

### Symptom: Cluster Member List Incorrect

**Check current members:**

```bash
curl http://localhost:9002/admin/members | jq
```

**Problem:** Dead node still in member list

**Solution:**

```bash
# Remove dead node
curl -X DELETE http://localhost:9002/admin/nodes/{node_id}

# Verify removal
curl http://localhost:9002/admin/members | jq
```

### Symptom: Cannot Form Quorum

**Error:**
```
Cluster lost quorum
```

**Diagnosis:**

```bash
# Check how many nodes are up
for i in 1 2 3; do
    echo "Node $i:"
    curl -s http://node$i:9002/health/ready && echo "UP" || echo "DOWN"
done
```

**Solutions:**

1. **Majority of nodes must be available**
   - 3-node cluster: need 2 nodes
   - 5-node cluster: need 3 nodes

2. **Start missing nodes**
   ```bash
   systemctl start anode
   ```

3. **If majority is lost permanently, restore from backup**

## Raft Consensus Issues

### Symptom: Frequent Leader Elections

**Diagnosis:**

```bash
# Check leader changes
curl -s http://localhost:9002/metrics | grep "anode_raft_leader_changes_total"

# Check current leader
curl http://localhost:9002/admin/raft | jq .current_leader

# Check Raft logs for election messages
journalctl -u anode | grep "election\|leader"
```

**Common causes:**

#### Network Instability

**Solution:**
```toml
[cluster]
# Increase timeouts for unstable networks
heartbeat_interval_ms = 2000
election_timeout_ms = 6000
```

#### Leader Overloaded

**Diagnosis:**
```bash
# Check CPU/memory on leader
ssh leader-node top
```

**Solution:**
- Scale vertically (more CPU/RAM)
- Reduce load (rate limiting, more clients)
- Optimize configuration (increase io_threads)

#### Clock Skew

**Solution:**
```bash
# Sync clocks on all nodes
sudo timedatectl set-ntp true
```

### Symptom: No Leader Elected

**Error:**
```
No Raft leader elected
```

**Diagnosis:**

```bash
# Check Raft status on all nodes
for i in 1 2 3; do
    echo "=== Node $i ==="
    curl -s http://node$i:9002/admin/raft | jq
done
```

**Common causes:**

1. **Network partition preventing quorum**
   - Check network connectivity between all nodes

2. **All nodes restarted simultaneously**
   - Wait for election timeout (a few seconds)

3. **Insufficient voting members**
   - Need majority of nodes online

**Solutions:**

```bash
# Restart nodes one at a time
systemctl restart anode

# Check logs for election activity
journalctl -u anode -f | grep election
```

### Symptom: Split Brain

**Symptoms:**
- Multiple leaders detected
- Inconsistent data across nodes
- Write operations succeed but are not replicated

**Emergency fix:**

```bash
# Identify minority partition
curl http://node1:9002/admin/cluster | jq .total_nodes
curl http://node2:9002/admin/cluster | jq .total_nodes

# Stop minority nodes
ssh minority-node "systemctl stop anode"

# Clear data on minority nodes
ssh minority-node "rm -rf /var/lib/anode/data/*"

# Rejoin to majority partition
ssh minority-node "anode --join majority-node:9001"
```

### Symptom: High Replication Lag

**Diagnosis:**

```bash
# Check replication lag
curl -s http://localhost:9002/metrics | grep "anode_cluster_replication_lag_seconds"

# Check Raft indices
curl http://localhost:9002/admin/raft | jq '{commit_index, last_applied}'
```

**Common causes:**

1. **Follower is slow (disk I/O bottleneck)**
   - Upgrade storage to faster SSD
   - Reduce write load

2. **Network bandwidth saturated**
   - Use dedicated network for cluster traffic
   - Reduce snapshot frequency

3. **Large objects causing big Raft entries**
   - Decrease chunk_size
   - Use multipart uploads

**Solutions:**

```toml
[raft]
# Adjust snapshot settings
snapshot_interval = 5000  # Smaller = faster catchup, more overhead
```

## Memory and Performance Problems

### Symptom: High Memory Usage

**Diagnosis:**

```bash
# Check memory usage
free -h

# Check Anode process memory
ps aux | grep anode

# From metrics
curl -s http://localhost:9002/metrics | grep "anode_memory_usage_bytes"
```

**Common causes:**

#### Metadata Cache Too Large

**Solution:**
```toml
[storage]
metadata_cache_size = 52428800  # Reduce to 50MB
```

#### Parquet Cache Too Large

**Solution:**
```toml
[parquet]
cache_size = 134217728  # Reduce to 128MB
```

#### Memory Leak

**Diagnosis:**
```bash
# Monitor memory over time
watch -n 60 'curl -s http://localhost:9002/metrics | grep anode_memory_usage_bytes'
```

**Solution:**
- Restart service (temporary fix)
- Report bug with logs and metrics
- Upgrade to latest version

### Symptom: High CPU Usage

**Diagnosis:**

```bash
# Check CPU usage
top

# CPU usage from metrics
curl -s http://localhost:9002/metrics | grep "anode_cpu_usage_percent"

# Profile with perf (Linux)
sudo perf record -p $(pgrep anode) -g -- sleep 30
sudo perf report
```

**Common causes:**

1. **High request rate**
   - Scale horizontally (add nodes)
   - Add caching layer (CDN, reverse proxy)

2. **Inefficient queries (Parquet)**
   - Enable predicate pushdown and column pruning
   - Optimize query patterns

3. **Too many I/O threads competing**
   ```toml
   [storage]
   io_threads = 8  # Reduce if CPU-bound
   ```

### Symptom: Slow Request Performance

**Diagnosis:**

```bash
# Check latency percentiles
curl -s http://localhost:9002/metrics | grep "anode_request_duration_seconds"

# Test directly
time aws --endpoint-url http://localhost:9000 s3 cp test.dat s3://bucket/
```

**Common causes and solutions:**

1. **Disk I/O bottleneck**
   ```bash
   iostat -x 1  # Check %util
   # Upgrade to NVMe SSD
   ```

2. **Network latency**
   ```bash
   ping -c 100 anode-server
   # Use local network or direct attach
   ```

3. **Small chunk size**
   ```toml
   [storage]
   chunk_size = "8MB"  # Increase for large files
   ```

4. **Sync writes enabled**
   ```toml
   [storage]
   sync_writes = false  # Better performance
   ```

5. **Cache miss rate high**
   ```bash
   # Check cache hit rate
   curl -s http://localhost:9002/metrics | grep "cache_hit"

   # Increase cache sizes
   [storage]
   metadata_cache_size = 1073741824  # 1GB
   ```

## S3 API Compatibility Issues

### Symptom: AWS CLI Commands Fail

**Test basic operations:**

```bash
# List buckets
aws --endpoint-url http://localhost:9000 s3 ls

# Create bucket
aws --endpoint-url http://localhost:9000 s3 mb s3://test

# Upload file
aws --endpoint-url http://localhost:9000 s3 cp file.txt s3://test/

# Download file
aws --endpoint-url http://localhost:9000 s3 cp s3://test/file.txt file2.txt
```

**Common issues:**

#### Wrong Region

Some tools default to specific AWS regions.

**Solution:**
```bash
# Specify region (any value works)
aws --endpoint-url http://localhost:9000 --region us-east-1 s3 ls

# Or configure default region
aws configure set region us-east-1
```

#### SSL/TLS Errors

**Error:**
```
SSL validation failed
```

**Solution:**
```bash
# Disable SSL verification (dev only!)
aws --endpoint-url http://localhost:9000 --no-verify-ssl s3 ls

# Or use HTTPS with proper certificates
```

### Symptom: Multipart Upload Fails

**Error:**
```
An error occurred (EntityTooSmall) when calling the CompleteMultipartUpload operation
```

**Cause:** Parts smaller than 5MB (except last part)

**Solution:**
```bash
# Ensure parts are >= 5MB
aws s3 cp largefile.dat s3://bucket/ \
    --endpoint-url http://localhost:9000 \
    --multipart-chunk-size 5MB
```

### Symptom: Presigned URL Not Working

**Common issues:**

1. **Expired URL** - Generate new one with longer expiration
2. **Clock skew** - Sync clocks on client and server
3. **Wrong signature version** - Anode uses SigV4

**Generate presigned URL:**

```bash
aws s3 presign s3://bucket/object.txt \
    --endpoint-url http://localhost:9000 \
    --expires-in 3600
```

### Symptom: SDK Integration Issues

**Python (boto3):**

```python
import boto3

s3 = boto3.client(
    's3',
    endpoint_url='http://localhost:9000',
    aws_access_key_id='your-access-key',
    aws_secret_access_key='your-secret-key',
    region_name='us-east-1'
)

# Test
s3.list_buckets()
```

**Common issues:**

1. **Missing `endpoint_url`** - SDK defaults to AWS S3
2. **Missing `region_name`** - Some SDKs require it
3. **SSL verification** - Disable for HTTP endpoints

## Data Integrity Issues

### Symptom: Checksum Verification Failures

**Run integrity check:**

```bash
# Scrub all data (checks checksums)
curl -X POST http://localhost:9002/admin/scrub

# Monitor progress
watch curl -s http://localhost:9002/admin/scrub/status

# Get results
curl http://localhost:9002/admin/scrub/results | jq
```

**If corruption found:**

```bash
# Repair from replicas
curl -X POST http://localhost:9002/admin/repair

# Monitor repair
curl http://localhost:9002/admin/repair/status | jq
```

### Symptom: Object Data Mismatch

**Symptoms:**
- Object content differs from expected
- Partial reads return wrong data
- Silent data corruption

**Diagnosis:**

```bash
# Download object and verify checksum
aws --endpoint-url http://localhost:9000 s3 cp s3://bucket/object.txt - | sha256sum

# Compare with original checksum
```

**Preventive measures:**

1. **Enable sync_writes for critical data**
   ```toml
   [storage]
   sync_writes = true
   ```

2. **Use ECC memory**
3. **Regular scrubbing**
   ```bash
   # Add to cron
   0 2 * * 0 curl -X POST http://localhost:9002/admin/scrub
   ```

## Emergency Procedures

### Complete Cluster Failure

**Restore from backup:**

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
ssh node1 "systemctl start anode"
sleep 30

# 4. Start remaining nodes
for node in node2 node3; do
    ssh $node "systemctl start anode"
done

# 5. Verify cluster
curl http://node1:9002/admin/cluster | jq
```

### Data Recovery

**If data is lost and no backup:**

1. **Check if replicas exist on other nodes**
   ```bash
   curl http://node2:9002/admin/chunks/{chunk_id}
   ```

2. **Attempt recovery from filesystem**
   ```bash
   # Search for chunk files
   find /var/lib/anode/data -name "*{chunk_id}*"
   ```

3. **Contact support with logs and metrics**

### Emergency Shutdown

**Graceful shutdown:**

```bash
# Stop accepting new requests
curl -X POST http://localhost:9002/admin/readonly

# Wait for in-flight requests to complete
sleep 30

# Shutdown
systemctl stop anode
```

**Force shutdown (data loss possible):**

```bash
sudo pkill -9 anode
```

### Rolling Back Failed Upgrade

```bash
# Stop new version
systemctl stop anode

# Restore old binary
cp /usr/local/bin/anode.old /usr/local/bin/anode

# Clear any incompatible data (if necessary)
# WARNING: May cause data loss
# rm -rf /var/lib/anode/data/metadata/*

# Start old version
systemctl start anode

# Verify health
curl http://localhost:9002/health | jq
```

## Getting Help

### Gathering Diagnostic Information

**Before opening a support ticket, collect:**

```bash
#!/bin/bash
# Save this as collect-diagnostics.sh

OUTDIR="anode-diagnostics-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUTDIR"

# Configuration
cp /etc/anode/config.toml "$OUTDIR/"

# Logs (last 1000 lines)
journalctl -u anode -n 1000 > "$OUTDIR/logs.txt"

# Metrics
curl -s http://localhost:9002/metrics > "$OUTDIR/metrics.txt"

# Health
curl -s http://localhost:9002/health | jq > "$OUTDIR/health.json"

# Raft status
curl -s http://localhost:9002/admin/raft | jq > "$OUTDIR/raft.json"

# Cluster status
curl -s http://localhost:9002/admin/cluster | jq > "$OUTDIR/cluster.json"

# System info
uname -a > "$OUTDIR/system.txt"
df -h >> "$OUTDIR/system.txt"
free -h >> "$OUTDIR/system.txt"

# Create tarball
tar -czf "$OUTDIR.tar.gz" "$OUTDIR"
echo "Diagnostics saved to $OUTDIR.tar.gz"
```

### Support Channels

- **GitHub Issues**: https://github.com/christopherpaika/anode/issues
- **Documentation**: https://github.com/christopherpaika/anode/docs
- **Community Forum**: https://github.com/christopherpaika/anode/discussions

### Useful Links

- [Operations Guide](operations.md) - Production operations procedures
- [Monitoring Guide](monitoring.md) - Metrics and alerting
- [Configuration Reference](configuration.md) - All configuration options
- [Architecture Overview](architecture.md) - System design and components
- [Performance Tuning](performance.md) - Optimization techniques

---

**Last Updated**: December 2023
**Anode Version**: 0.1.0
