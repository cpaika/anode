# Anode Server Binary

The main Anode server binary that provides distributed object storage with S3-compatible API.

## Overview

Anode is a distributed object storage system designed for small to medium clusters (3-15 nodes). It combines:

- **S3-compatible API** - Full compatibility with AWS S3 clients
- **Raft consensus** - Distributed coordination and replication
- **Parquet integration** - Optimized for analytical workloads
- **Placement groups** - Intelligent data distribution

## Architecture

### Components

1. **Storage Engine** (`anode-storage`)
   - Blob storage with chunking
   - Metadata management
   - Object manifests

2. **Raft Consensus** (`anode-raft`)
   - Leader election
   - Log replication
   - Cluster membership

3. **S3 API** (`anode-s3`)
   - S3-compatible REST API
   - Authentication
   - Multipart uploads

4. **Parquet Integration** (`anode-parquet`)
   - Metadata caching
   - Query optimization
   - Column pruning

5. **Cluster Manager**
   - Node discovery
   - Placement group management
   - Rebalancing

## Usage

### Starting a Node

```bash
# Bootstrap a new cluster
anode --config /etc/anode/config.toml --bootstrap

# Join an existing cluster
anode --config /etc/anode/config.toml --join 192.168.1.10:9001

# With CLI overrides
anode \
  --config /etc/anode/config.toml \
  --node-id 2 \
  --s3-addr 0.0.0.0:9000 \
  --grpc-addr 0.0.0.0:9001 \
  --admin-addr 0.0.0.0:9002 \
  --data-dir /data/anode
```

### Configuration

See [`config/anode.example.toml`](config/anode.example.toml) for a complete configuration example.

Key configuration sections:

- **node** - Node identity and bind addresses
- **cluster** - Cluster membership and replication
- **storage** - Data directories and I/O settings
- **s3** - S3 API configuration
- **raft** - Consensus algorithm tuning
- **parquet** - Parquet optimization settings
- **metrics** - Prometheus metrics
- **logging** - Log levels and output

### Environment Variables

You can override configuration with environment variables:

- `ANODE_LOG_LEVEL` - Set log level (trace, debug, info, warn, error)
- `RUST_LOG` - More granular logging control

## API Endpoints

### S3 API (Port 9000)

Standard S3 operations:
- Bucket operations (CreateBucket, ListBuckets, DeleteBucket)
- Object operations (PutObject, GetObject, DeleteObject, ListObjects)
- Multipart uploads

### Admin API (Port 9002)

Health and monitoring:
- `GET /health` - Overall health status
- `GET /health/live` - Liveness probe
- `GET /health/ready` - Readiness probe
- `GET /health/detailed` - Detailed component health
- `GET /metrics` - Prometheus metrics
- `GET /cluster/status` - Cluster status
- `GET /node/status` - Node status

### gRPC API (Port 9001)

Internal cluster communication:
- Raft RPC
- Node-to-node replication
- Cluster coordination

## Metrics

Prometheus metrics are exposed at `/metrics` on the admin port. Key metrics include:

### Request Metrics
- `anode_requests_total` - Total request count
- `anode_request_duration_seconds` - Request latency
- `anode_request_size_bytes` - Request size distribution
- `anode_response_size_bytes` - Response size distribution

### Storage Metrics
- `anode_objects_total` - Total object count per bucket
- `anode_storage_bytes_total` - Storage usage per bucket
- `anode_operations_total` - Storage operation count
- `anode_operation_errors_total` - Error count by type

### Raft Metrics
- `anode_raft_term` - Current Raft term
- `anode_raft_is_leader` - Leader status (1=leader, 0=follower)
- `anode_raft_commit_index` - Committed log index
- `anode_raft_proposals_total` - Proposals submitted
- `anode_raft_proposal_failures_total` - Failed proposals

### Cluster Metrics
- `anode_cluster_nodes_total` - Total nodes in cluster
- `anode_cluster_healthy_nodes` - Healthy node count

### Parquet Metrics
- `anode_parquet_cache_hits_total` - Cache hits
- `anode_parquet_cache_misses_total` - Cache misses
- `anode_parquet_cache_size_bytes` - Cache size
- `anode_parquet_queries_total` - Query count

## Health Checks

### Liveness
Indicates the server is running. Returns 200 if alive.

```bash
curl http://localhost:9002/health/live
```

### Readiness
Indicates the server is ready to serve requests. Checks:
- Raft has a leader
- Storage is accessible

```bash
curl http://localhost:9002/health/ready
```

### Detailed Health
Returns detailed status of all components:
- Storage engine
- Raft consensus
- Cluster membership

```bash
curl http://localhost:9002/health/detailed
```

## Deployment

### Single Node (Development)

```bash
anode --config config.toml --bootstrap
```

### Three Node Cluster (Production)

Node 1 (bootstrap):
```bash
anode --config node1.toml --bootstrap
```

Node 2:
```bash
anode --config node2.toml --join node1:9001
```

Node 3:
```bash
anode --config node3.toml --join node1:9001
```

### Kubernetes

See the [deployment examples](../../deploy/kubernetes/) for Kubernetes manifests.

## Graceful Shutdown

The server handles `SIGTERM` and `SIGINT` for graceful shutdown:

1. Stops accepting new requests
2. Completes in-flight requests (30s timeout)
3. Flushes Raft log
4. Closes storage engine
5. Exits

## Logging

Structured logging with tracing:

```bash
# Info level (default)
anode --log-level info

# Debug level
anode --log-level debug

# Trace specific modules
RUST_LOG=anode=debug,anode_raft=trace anode
```

## Performance Tuning

### Storage

- `chunk_size` - Larger chunks reduce metadata overhead but increase memory usage
- `sync_writes` - Enable for durability, disable for performance
- `io_threads` - Match to available CPU cores

### Raft

- `heartbeat_interval_ms` - Lower values detect failures faster but increase network traffic
- `election_timeout_ms` - Should be 3-5x heartbeat interval
- `snapshot_interval` - Lower values reduce log size but increase I/O

### Parquet

- `cache_size` - Larger cache improves query performance
- `row_group_size` - Tune based on typical query patterns

## Troubleshooting

### No Leader Elected

Check network connectivity between nodes:
```bash
# On each node
curl http://localhost:9002/cluster/status
```

### High Memory Usage

Reduce cache sizes:
- `storage.metadata_cache_size`
- `parquet.cache_size`

### Slow Queries

Enable Parquet optimizations:
- `parquet.enable_predicate_pushdown = true`
- `parquet.enable_column_pruning = true`

Increase cache:
- `parquet.cache_size`

## Development

### Building

```bash
cargo build --release --package anode
```

### Testing

```bash
cargo test --package anode
```

### Linting

```bash
cargo clippy --package anode
```

## License

See [LICENSE](../../LICENSE) for details.
