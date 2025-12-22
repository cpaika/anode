# Anode

[![CI](https://github.com/christopherpaika/anode/workflows/CI/badge.svg)](https://github.com/christopherpaika/anode/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**Distributed object storage for small clusters**

Anode is a high-performance, S3-compatible object storage system built in Rust, designed for small to medium-sized clusters (3-50 nodes). It combines strong consistency guarantees with Parquet-aware optimizations for analytical workloads.

## Features

- **S3-Compatible API** - Drop-in replacement for S3, works with AWS CLI, boto3, and other S3 tools
- **Strong Consistency** - Raft consensus ensures linearizable reads and writes
- **Unified Architecture** - Every node runs the same binary with S3 API, Raft, and storage engine
- **Content-Addressed Storage** - SHA-256 deduplication at the chunk level
- **Parquet Optimizations** - Metadata caching, predicate pushdown, and column pruning for analytics
- **Pure Rust** - Memory-safe implementation with ACID guarantees using redb
- **Easy Deployment** - Docker, Kubernetes, and Helm support included

## Quick Start

### Using Docker Compose

```bash
# Clone repository
git clone https://github.com/christopherpaika/anode
cd anode/deploy/docker

# Start 3-node cluster
docker-compose up -d

# Verify cluster
curl http://localhost:8081/health
```

### Using the S3 API

```bash
# Configure AWS CLI
aws configure set aws_access_key_id minioadmin
aws configure set aws_secret_access_key minioadmin

# Create bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket

# Upload file
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://my-bucket/

# Download file
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/test.txt downloaded.txt
```

## Documentation

- **[Quickstart Guide](docs/quickstart.md)** - Get up and running in minutes
- **[Architecture](docs/architecture.md)** - System design and components
- **[Configuration](docs/configuration.md)** - Complete configuration reference
- **[Operations Guide](docs/operations.md)** - Production deployment and management
- **[S3 API Reference](docs/s3-api.md)** - Supported S3 operations
- **[Performance Tuning](docs/performance.md)** - Optimize for your workload
- **[Security Guide](docs/security.md)** - Authentication, encryption, and best practices
- **[Testing Guide](docs/testing.md)** - Running and writing tests
- **[Development Guide](docs/development.md)** - Contributing to Anode

## Architecture

Anode uses a unified node architecture where every node runs the same binary containing:

```
┌─────────────────────────────────────────┐
│           Anode Node                     │
│  ┌────────────────────────────────────┐ │
│  │    S3 HTTP API (Axum)              │ │ :8080
│  └────────────────────────────────────┘ │
│                  ↓                       │
│  ┌────────────────────────────────────┐ │
│  │    Raft Consensus (OpenRaft)       │ │ :9090
│  └────────────────────────────────────┘ │
│                  ↓                       │
│  ┌────────────────────────────────────┐ │
│  │    Storage Engine                   │ │
│  │  ├─ Metadata (redb)                │ │
│  │  ├─ Blobs (file-based)             │ │
│  │  └─ Chunk Deduplication            │ │
│  └────────────────────────────────────┘ │
│                                          │
│  Admin/Metrics API :8081                 │
└─────────────────────────────────────────┘
```

**Key Features:**
- **Metadata Store**: Pure Rust embedded database (redb) with ACID guarantees
- **Blob Storage**: Content-addressed file-based storage with SHA-256 hashing
- **Raft Consensus**: Strong consistency with placement groups for scalability
- **Parquet Aware**: Metadata caching and query optimization for analytical workloads

See [Architecture Documentation](docs/architecture.md) for detailed design.

## Installation

### From Source

```bash
# Install dependencies
# macOS
brew install protobuf

# Ubuntu/Debian
sudo apt-get install protobuf-compiler libprotobuf-dev

# Build
git clone https://github.com/christopherpaika/anode
cd anode
cargo build --release

# Binary at: target/release/anode
```

### Using Docker

```bash
docker pull ghcr.io/christopherpaika/anode:latest

docker run -d \
    -p 8080:8080 \
    -p 8081:8081 \
    -p 9090:9090 \
    -v anode-data:/data \
    ghcr.io/christopherpaika/anode:latest
```

### Using Helm

```bash
helm repo add anode https://christopherpaika.github.io/anode/helm
helm repo update

helm install my-anode anode/anode \
    --set replicaCount=3 \
    --set persistence.size=100Gi
```

## Configuration

Basic configuration example:

```toml
[node]
id = 1
name = "anode-1"
s3_addr = "0.0.0.0:8080"
grpc_addr = "0.0.0.0:9090"
admin_addr = "0.0.0.0:8081"

[cluster]
name = "my-cluster"
replication_factor = 3
placement_groups = 128

[storage]
data_dir = "/var/lib/anode/data"
chunk_size = "4MB"
metadata_cache_size = 104857600  # 100MB

[parquet]
enable_cache = true
cache_size = 268435456  # 256MB
enable_predicate_pushdown = true
enable_column_pruning = true
```

See [Configuration Reference](docs/configuration.md) for all options.

## Performance

### Benchmarks

**Small Objects (1MB):**
- Write: 1000-5000 ops/sec per node
- Read: 5000-10000 ops/sec per node
- Latency: P95 < 50ms

**Large Objects (100MB+):**
- Throughput: Network bandwidth / replication_factor
- Multipart upload support for >5GB files

**Parquet Queries:**
- 5-10x faster than reading full file
- Row group pruning using statistics
- Column projection for minimal I/O

See [Performance Tuning Guide](docs/performance.md) for optimization tips.

## Use Cases

### Object Storage

Standard S3-compatible object storage for:
- Application data
- Media files
- Backups
- Archives

### Data Lake

Optimized for analytical workloads:
- Parquet file storage and querying
- Metadata caching for fast queries
- Predicate pushdown to minimize I/O
- Column projection support

### Backup Storage

Reliable storage with strong consistency:
- Configurable replication factor
- Content-addressed deduplication
- Durable Raft consensus
- Point-in-time snapshots

## Comparison

### vs. MinIO

| Feature | Anode | MinIO |
|---------|-------|-------|
| Consistency | Strong (Raft) | Eventual |
| Deduplication | Automatic | No |
| Parquet Support | Optimized | Standard |
| Language | Rust | Go |
| Target Size | 3-50 nodes | 4-32+ nodes |

### vs. Ceph

| Feature | Anode | Ceph |
|---------|-------|------|
| Architecture | Unified | Separate components |
| Deployment | Single binary | Multiple daemons |
| Complexity | Low | High |
| Scalability | Small clusters | Large clusters |

See [Architecture Documentation](docs/architecture.md) for detailed comparison.

## Roadmap

### Current (v0.1.0)
- ✅ Basic S3 operations
- ✅ Raft consensus
- ✅ Content-addressed storage
- ✅ Parquet metadata caching
- ✅ Docker and Kubernetes support

### Planned (v0.2.0)
- Object versioning
- Server-side encryption
- Advanced bucket policies
- S3 Select support

### Future
- Erasure coding
- Multi-region replication
- Lambda-style event processing
- Object lifecycle policies

See [ROADMAP.md](ROADMAP.md) for details.

## Community

- **GitHub Issues**: [Report bugs](https://github.com/christopherpaika/anode/issues)
- **Discussions**: [Ask questions](https://github.com/christopherpaika/anode/discussions)
- **Contributing**: See [Development Guide](docs/development.md)

## Testing

Anode includes comprehensive testing:

```bash
# Unit tests
cargo test --workspace

# Integration tests
cd tests/correctness
cargo test --release

# Chaos tests
cd tests/chaos
./run-chaos-tests.sh

# Benchmarks
cargo bench
```

See [Testing Guide](docs/testing.md) for more information.

## Security

- **Authentication**: AWS Signature V4
- **Encryption**: TLS for all communication (via proxy)
- **Authorization**: Bucket and object-level permissions
- **Audit Logging**: Complete audit trail

See [Security Guide](docs/security.md) for best practices.

## License

Anode is dual-licensed under:
- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

You may choose either license for your use.

## Credits

Anode is built on top of excellent open-source projects:

- [openraft](https://github.com/datafuselabs/openraft) - Raft consensus
- [redb](https://github.com/cberner/redb) - Embedded database
- [axum](https://github.com/tokio-rs/axum) - Web framework
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime
- [parquet](https://github.com/apache/arrow-rs) - Parquet support

## Support

- **Documentation**: https://github.com/christopherpaika/anode/docs
- **Bug Reports**: https://github.com/christopherpaika/anode/issues
- **Security Issues**: security@anode.io (Do not open public issues)

---

**Built with ❤️ in Rust**
