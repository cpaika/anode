# Anode Architecture

## Overview

Anode is a Rust-based distributed object storage system designed for small clusters (3-50 nodes). It provides S3-compatible object storage with strong consistency guarantees, built on a unified architecture where every node runs the same binary with identical capabilities.

### Key Design Principles

- **Unified Node Architecture**: Every node runs the same binary containing S3 API, Raft consensus, and storage engine
- **Strong Consistency**: Raft consensus ensures linearizable reads and writes
- **Content-Addressed Storage**: SHA-256 based deduplication at the chunk level
- **Parquet-Aware**: Special optimizations for analytical workloads reading Parquet files
- **Pure Rust**: Memory-safe implementation with ACID guarantees

## System Components

### 1. Unified Node Architecture

Unlike traditional object storage systems with separate gateway and storage nodes, Anode uses a unified architecture:

```
┌─────────────────────────────────────────┐
│           Anode Node                     │
│  ┌────────────────────────────────────┐ │
│  │    S3 HTTP API (Axum)              │ │ :8080
│  │  ├─ Authentication (AWS SigV4)     │ │
│  │  ├─ Bucket Operations              │ │
│  │  └─ Object Operations              │ │
│  └────────────────────────────────────┘ │
│                  ↓                       │
│  ┌────────────────────────────────────┐ │
│  │    Raft Consensus (OpenRaft)       │ │ :9090
│  │  ├─ State Machine                  │ │
│  │  ├─ Log Replication                │ │
│  │  └─ Placement Groups                │ │
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

**Benefits of Unified Architecture:**
- Simplified deployment and operations
- No complex role-based configuration
- Automatic load balancing (any node handles any request)
- Easy horizontal scaling (add more identical nodes)
- Reduced operational complexity

### 2. Storage Engine

The storage engine consists of three main components working together:

#### 2.1 Metadata Store (redb)

Uses **redb**, a pure Rust embedded database with ACID guarantees:

**Tables:**
- `objects`: Maps object keys to manifests
- `buckets`: Bucket metadata and configuration
- `chunks`: Chunk reference counting for garbage collection

**Key Features:**
- ACID transactions
- Zero-copy reads
- Crash-safe (write-ahead logging)
- Memory-mapped I/O for performance

**Data Model:**
```rust
ObjectManifest {
    object_key: ObjectKey,
    size: u64,
    chunks: Vec<ChunkId>,        // SHA-256 content addresses
    content_type: Option<String>,
    metadata: HashMap<String, String>,
    created_at: u64,
    modified_at: u64,
    etag: String,
}
```

#### 2.2 Blob Storage

Content-addressed file-based storage for object data:

**Directory Layout:**
```
data/
├── blobs/
│   ├── ab/
│   │   ├── cd/
│   │   │   └── abcdef123456...  # SHA-256 hash
│   │   └── ef/
│   └── 12/
└── metadata/
    └── store.redb
```

**Chunk Management:**
- Default chunk size: 4MB (configurable)
- SHA-256 content addressing
- Automatic deduplication
- Reference counting for garbage collection

**Storage Path Calculation:**
```
ChunkId: abcdef123456789...
Path: data/blobs/ab/cd/abcdef123456789...
      └─ First 2 chars ──┘
         └─ Next 2 chars ─┘
```

#### 2.3 Chunk Deduplication

Content-addressed storage eliminates duplicate data:

```
Object A: [Chunk1, Chunk2, Chunk3]
Object B: [Chunk2, Chunk4, Chunk5]
          └─ Shared chunk (stored once)

Storage:
- Chunk1 (ref_count: 1)
- Chunk2 (ref_count: 2)  ← Deduplicated
- Chunk3 (ref_count: 1)
- Chunk4 (ref_count: 1)
- Chunk5 (ref_count: 1)
```

**Garbage Collection:**
- When object is deleted, chunk ref_count decreases
- When ref_count reaches 0, chunk is eligible for deletion
- Background GC process removes unreferenced chunks

### 3. Raft Consensus

Built on **openraft**, a feature-complete Raft implementation:

#### 3.1 Core Raft Operations

**Leader Election:**
- Heartbeat interval: 1000ms (configurable)
- Election timeout: 3000ms (configurable)
- Leader handles all writes

**Log Replication:**
- Write operations committed to Raft log
- Replicated to majority before acknowledgment
- State machine applies committed entries

**State Machine:**
```rust
enum Request {
    PutObject { key, data, metadata },
    DeleteObject { key },
    CreateBucket { name },
    DeleteBucket { name },
    UpdateMetadata { key, metadata },
}
```

#### 3.2 Placement Groups

Data is partitioned into **placement groups** (PGs) for scalability:

**Purpose:**
- Horizontal scaling beyond single Raft group limits
- Parallel processing of requests
- Isolation of failure domains

**Assignment:**
```
Bucket + Key → Hash → PG ID → Raft Group → Leader Node
```

**Configuration:**
- Default: 128 placement groups
- Each PG is an independent Raft cluster
- Consistent hashing for key distribution

**Example:**
```
PG 0:  Node1 (L), Node2 (F), Node3 (F)
PG 1:  Node2 (L), Node3 (F), Node1 (F)
PG 2:  Node3 (L), Node1 (F), Node2 (F)
...
PG 127: Node1 (L), Node2 (F), Node3 (F)

L = Leader, F = Follower
```

### 4. S3 API Layer

Implements S3-compatible HTTP API using Axum web framework:

#### 4.1 Supported Operations

**Bucket Operations:**
- CreateBucket
- DeleteBucket (must be empty)
- ListBuckets
- HeadBucket

**Object Operations:**
- PutObject
- GetObject
- DeleteObject
- HeadObject
- CopyObject
- ListObjectsV2

**Multipart Upload:**
- CreateMultipartUpload
- UploadPart
- CompleteMultipartUpload
- AbortMultipartUpload
- ListMultipartUploads
- ListParts

**Advanced Features:**
- Range requests (partial downloads)
- Conditional requests (If-Match, If-None-Match)
- Object metadata
- Object tagging
- Presigned URLs

#### 4.2 Authentication

AWS Signature Version 4 (SigV4):
```
Authorization: AWS4-HMAC-SHA256
    Credential=AKIAIOSFODNN7EXAMPLE/20231219/us-east-1/s3/aws4_request,
    SignedHeaders=host;x-amz-date,
    Signature=<calculated signature>
```

**Signature Calculation:**
1. Create canonical request
2. Create string to sign
3. Derive signing key
4. Calculate signature

**Credential Storage:**
- Access keys stored in metadata store
- Secret keys hashed (bcrypt/argon2)
- Support for multiple credentials per user

### 5. Parquet Optimization

Special features for analytical workloads:

#### 5.1 Metadata Caching

Parquet files contain rich metadata in the footer:
- Schema information
- Row group metadata
- Column statistics (min/max, null count)
- Page indexes
- Bloom filters

**Caching Strategy:**
```
Parquet File: 1GB
Footer: 10KB (cached)

Query: SELECT col1 FROM table WHERE col2 = 'value'
1. Check cache for footer metadata
2. Use statistics to prune row groups
3. Fetch only relevant row groups
4. Apply column projection (read only col1, col2)
```

**Benefits:**
- Avoid reading entire file for metadata
- Row group pruning (skip irrelevant data)
- Column projection (read only needed columns)
- Predicate pushdown (filter at storage layer)

#### 5.2 Query Optimization

The optimizer provides hints for efficient Parquet reads:

```rust
struct QueryHint {
    row_groups_to_read: Vec<usize>,      // Skip pruned row groups
    columns_to_read: Vec<String>,         // Project only needed columns
    byte_ranges: Vec<(u64, u64)>,        // Exact byte ranges to fetch
    predicates: Vec<Predicate>,           // Pushdown filters
}
```

**Example:**
```
File: data.parquet (10 row groups, 100 columns)
Query: SELECT id, name WHERE date = '2023-12-19'

Optimization:
1. Read footer metadata (cached)
2. Check date column statistics
3. Prune row groups 0-7 (dates outside range)
4. Read only row groups 8-9
5. Project only columns: id, name, date
6. Apply date filter during scan

Result: Read ~20% of file instead of 100%
```

### 6. Replication

Data is replicated across nodes for durability:

#### 6.1 Write Path

```
Client → PUT /bucket/key
  │
  ├─ Node 1 (receives request)
  │    ├─ Hash(bucket, key) → PG 42
  │    ├─ Find PG 42 leader (Node 2)
  │    └─ Forward to Node 2
  │
  ├─ Node 2 (PG 42 leader)
  │    ├─ Split data into chunks
  │    ├─ Compute SHA-256 for each chunk
  │    ├─ Write chunks to local storage
  │    ├─ Create Raft log entry
  │    ├─ Replicate log to followers (Node 1, Node 3)
  │    ├─ Wait for majority (2/3) to acknowledge
  │    ├─ Apply to state machine
  │    └─ Return success to client
  │
  └─ Followers (Node 1, Node 3)
       ├─ Receive Raft log entry
       ├─ Fetch chunks from leader (streaming)
       ├─ Write chunks to local storage
       ├─ Acknowledge to leader
       └─ Apply to state machine
```

#### 6.2 Read Path

```
Client → GET /bucket/key
  │
  ├─ Node 1 (receives request)
  │    ├─ Hash(bucket, key) → PG 42
  │    ├─ Check local metadata store
  │    ├─ Find object manifest
  │    └─ Read chunks from local storage
  │
  └─ Stream response to client
```

**Read Optimization:**
- Any node can serve reads (eventual consistency option)
- Linearizable reads go through Raft leader
- Local reads for performance

#### 6.3 Chunked Replication

Large objects are replicated in parallel:

```
Object: 100MB = 25 chunks × 4MB each

Replication (parallel):
  Chunk 0 → Node 1 ──┐
  Chunk 1 → Node 1 ──┤
  Chunk 2 → Node 1 ──┼→ Replicate → Node 2, Node 3
  ...                 │
  Chunk 24 → Node 1 ─┘

Throughput: Network bandwidth × replication_factor
```

## Data Flow Diagrams

### Write Request Flow

```
┌──────┐     PUT      ┌────────┐    Forward    ┌────────┐
│Client│─────────────→│ Node 1 │──────────────→│ Node 2 │
└──────┘              │(Any)   │               │(Leader)│
                      └────────┘               └────┬───┘
                                                    │
                      ┌─────────────────────────────┤
                      │ 1. Split into chunks        │
                      │ 2. Compute SHA-256          │
                      │ 3. Write to local storage   │
                      │ 4. Create Raft log entry    │
                      └─────────────────────────────┘
                                                    │
                      ┌─────────────────────────────┼─────────────────────────┐
                      │                             │                         │
                      ▼                             ▼                         ▼
                 ┌─────────┐                  ┌─────────┐             ┌─────────┐
                 │ Node 1  │                  │ Node 2  │             │ Node 3  │
                 │(Follower)│◀─────Raft───────│(Leader) │─────Raft───→│(Follower)│
                 └─────────┘   Replicate      └─────────┘   Replicate └─────────┘
                      │                             │                       │
                      │ 5. Fetch chunks             │                       │
                      │ 6. Write locally            │                       │
                      │ 7. Acknowledge              │                       │
                      ├────────────────────────────→│                       │
                      │                             │◀──────────────────────┤
                      │                             │                       │
                      │        8. Majority reached  │                       │
                      │        9. Commit            │                       │
                      │        10. Return success   │                       │
                      │                             │                       │
```

### Read Request Flow

```
┌──────┐     GET      ┌────────┐
│Client│─────────────→│ Node 1 │
└──────┘              │(Any)   │
   ▲                  └────┬───┘
   │                       │
   │                       │ 1. Lookup metadata
   │                       │ 2. Get chunk list
   │                       │
   │                  ┌────▼─────────────┐
   │                  │ Local Metadata   │
   │                  │ Store (redb)     │
   │                  └────┬─────────────┘
   │                       │
   │                       │ 3. Read chunks
   │                       │
   │                  ┌────▼─────────────┐
   │                  │ Local Blob       │
   │                  │ Storage          │
   │                  └────┬─────────────┘
   │                       │
   └───────────────────────┘ 4. Stream to client
```

## Failure Handling

### Node Failure

**Detection:**
- Raft heartbeat timeout (3s default)
- Health check endpoint failure

**Recovery:**
1. Raft elects new leader for affected PGs
2. Clients retry requests (automatic)
3. New leader continues servicing requests
4. When failed node recovers, it rejoins as follower
5. Catches up via Raft log replay or snapshot

### Network Partition

**Split Brain Prevention:**
- Raft requires majority for writes
- Minority partition cannot commit writes
- Reads may be stale in minority partition

**Example (3-node cluster):**
```
Partition: [Node1] | [Node2, Node3]

Minority (Node1):
- Cannot elect leader (no majority)
- Cannot commit writes
- Can serve stale reads (optional)

Majority (Node2, Node3):
- Elects new leader
- Accepts writes
- Serves consistent reads
```

**Partition Healing:**
- Minority rejoins cluster
- Raft log reconciliation
- Stale data overwritten

### Data Corruption

**Detection:**
- SHA-256 checksum verification on read
- Periodic scrubbing (background process)

**Recovery:**
1. Detect corrupted chunk (checksum mismatch)
2. Fetch correct chunk from replica
3. Overwrite corrupted chunk
4. Verify checksum
5. Log corruption event

### Disk Failure

**Single Disk Failure:**
- Node marks itself degraded
- Redirects reads to replicas
- Triggers data rebuild from replicas

**Recovery:**
1. Replace failed disk
2. Rejoin cluster
3. Fetch data from replicas
4. Verify checksums
5. Resume normal operation

## Performance Characteristics

### Latency

**Write Latency:**
- Network RTT × 2 (client → leader → follower → leader → client)
- Disk sync (if enabled): +5-10ms
- Small objects (<1MB): ~10-50ms
- Large objects (>100MB): Throughput-bound

**Read Latency:**
- Local metadata lookup: <1ms
- Small objects (<1MB): ~5-20ms
- Large objects: Throughput-bound
- Network-local reads (same rack): Lower latency

### Throughput

**Write Throughput:**
- Limited by Raft leader and replication
- Single object: Network bandwidth / replication_factor
- Concurrent writes: Scales with number of PGs

**Read Throughput:**
- Scales linearly with nodes (local reads)
- Limited by storage bandwidth
- Parquet optimization: 5-10x improvement for analytics

### Scalability

**Node Count:**
- Tested: 3-50 nodes
- Recommended: 5-20 nodes for optimal performance
- Maximum: Limited by PG count and coordination overhead

**Storage Capacity:**
- Per-node: Disk capacity
- Cluster: Sum of all nodes / replication_factor
- Metadata: ~1KB per object (cached in memory)

**Object Count:**
- Millions of objects per cluster
- Limited by metadata storage and memory

## Security

### Authentication

- AWS Signature V4 (industry standard)
- Access key + secret key
- Request signing prevents tampering

### Authorization

- Bucket-level permissions
- Object-level ACLs (basic)
- IAM-like policies (planned)

### Encryption

**In Transit:**
- TLS 1.3 for S3 API
- gRPC with TLS for inter-node communication

**At Rest (optional):**
- Chunk-level encryption (AES-256)
- Key management integration (planned)

### Network Isolation

- Private cluster network (gRPC)
- Public S3 API endpoint
- NetworkPolicy support (Kubernetes)

## Monitoring and Observability

### Metrics (Prometheus)

**Exposed at:** `:8081/metrics`

**Key Metrics:**
- Request rate and latency (P50, P95, P99)
- Raft state (leader/follower, log size, commit index)
- Storage usage (disk, memory, objects, chunks)
- Replication lag
- Error rates

### Logging

**Structured Logging (JSON):**
```json
{
  "timestamp": "2023-12-19T10:30:00Z",
  "level": "info",
  "target": "anode::storage",
  "message": "Object stored successfully",
  "bucket": "my-bucket",
  "key": "my-object",
  "size": 1048576,
  "chunks": 1,
  "duration_ms": 45
}
```

**Log Levels:**
- ERROR: Critical failures
- WARN: Degraded operation
- INFO: Normal operations
- DEBUG: Detailed debugging
- TRACE: Very verbose

### Tracing

- OpenTelemetry support (planned)
- Distributed tracing across nodes
- Request flow visualization

### Health Checks

**Endpoints:**
- `/health` - Basic health (HTTP 200 = healthy)
- `/health/ready` - Ready to accept traffic
- `/health/live` - Process is alive

**Checks:**
- Raft connectivity
- Storage accessibility
- Memory usage
- Disk space

## Comparison with Other Systems

### vs. MinIO

| Feature | Anode | MinIO |
|---------|-------|-------|
| Consensus | Raft (strong consistency) | Erasure coding (eventual consistency) |
| Deduplication | Chunk-level (SHA-256) | No |
| Parquet Support | Metadata caching, optimization | Standard S3 |
| Target Cluster Size | 3-50 nodes | 4-32+ nodes |
| Language | Rust | Go |

### vs. Ceph

| Feature | Anode | Ceph |
|---------|-------|------|
| Architecture | Unified nodes | Separate OSD/MON/MDS |
| Deployment | Single binary | Multiple daemons |
| S3 Compatibility | Native | Via RGW gateway |
| Complexity | Low | High |
| Scalability | Small clusters | Large clusters (100s of nodes) |

### vs. SeaweedFS

| Feature | Anode | SeaweedFS |
|---------|-------|-----------|
| Consistency | Strong (Raft) | Eventual |
| Metadata Store | redb (embedded) | Separate master servers |
| Deduplication | Automatic (content-addressed) | Manual |
| Parquet Optimization | Yes | No |

## Future Enhancements

### Planned Features

1. **Erasure Coding**: Reduce storage overhead vs. replication
2. **Multi-Region Support**: Cross-datacenter replication
3. **S3 Select**: Query data without downloading
4. **Lambda Integration**: Event-driven processing
5. **Lifecycle Policies**: Automatic data archival/deletion
6. **Versioning**: Object version history
7. **Object Locking**: WORM (Write Once Read Many)
8. **Bucket Replication**: Cross-bucket replication

### Performance Improvements

1. **Pipelined Replication**: Overlap chunk transfer with Raft
2. **Adaptive Chunking**: Variable chunk sizes based on object size
3. **Intelligent Caching**: ML-based cache eviction
4. **RDMA Support**: Low-latency networking for data transfer

### Operational Enhancements

1. **Rolling Upgrades**: Zero-downtime updates
2. **Auto-Scaling**: Dynamic node addition/removal
3. **Self-Healing**: Automatic repair of corrupted data
4. **Multi-Tenancy**: Isolated storage per tenant
