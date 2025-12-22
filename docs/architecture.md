# Anode Architecture

This document provides a comprehensive overview of the Anode distributed object storage system architecture, including component relationships, data flows, and key design decisions.

## Table of Contents

- [System Overview](#system-overview)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [Request Lifecycle](#request-lifecycle)
- [Consensus and Replication](#consensus-and-replication)
- [Storage Layer Design](#storage-layer-design)
- [Cluster Architecture](#cluster-architecture)
- [Design Decisions](#design-decisions)
- [Extension Points](#extension-points)

## System Overview

Anode is a distributed, S3-compatible object storage system designed for small to medium clusters (3-50 nodes). It combines strong consistency guarantees through Raft consensus with content-addressed storage and Parquet-aware optimizations for analytical workloads.

### Key Characteristics

- **Unified Architecture**: Every node runs the same binary containing S3 API, Raft consensus, and storage engine
- **Strong Consistency**: Raft consensus ensures linearizable reads and writes
- **Content-Addressed Storage**: SHA-256 based deduplication at 4MB chunk granularity
- **Pure Rust**: Memory-safe implementation with zero-cost abstractions
- **Parquet Optimized**: Metadata caching, predicate pushdown, and column pruning

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Anode Node                            │
│                                                           │
│  ┌─────────────────────────────────────────────────┐   │
│  │         S3-Compatible API Layer                  │   │ Port 8080
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │   │
│  │  │ Buckets  │  │ Objects  │  │  Multipart   │ │   │
│  │  └──────────┘  └──────────┘  └──────────────┘ │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │   │
│  │  │  Batch   │  │   Copy   │  │     Auth     │ │   │
│  │  └──────────┘  └──────────┘  └──────────────┘ │   │
│  └─────────────────────────────────────────────────┘   │
│                         ↓                                │
│  ┌─────────────────────────────────────────────────┐   │
│  │         Raft Consensus Layer                     │   │ Port 9090
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │   │
│  │  │   Log    │  │  State   │  │   Network    │ │   │
│  │  │  Store   │  │ Machine  │  │   Manager    │ │   │
│  │  └──────────┘  └──────────┘  └──────────────┘ │   │
│  └─────────────────────────────────────────────────┘   │
│                         ↓                                │
│  ┌─────────────────────────────────────────────────┐   │
│  │         Storage Engine                           │   │
│  │  ┌──────────────────────────────────────────┐  │   │
│  │  │  Metadata Store (redb)                   │  │   │
│  │  │  - Buckets & Objects                     │  │   │
│  │  │  - Chunk References                      │  │   │
│  │  │  - Reference Counts                      │  │   │
│  │  └──────────────────────────────────────────┘  │   │
│  │  ┌──────────────────────────────────────────┐  │   │
│  │  │  Blob Store (file-based)                 │  │   │
│  │  │  - Content-Addressed Chunks              │  │   │
│  │  │  - SHA-256 Deduplication                 │  │   │
│  │  └──────────────────────────────────────────┘  │   │
│  │  ┌──────────────────────────────────────────┐  │   │
│  │  │  Chunk Manager                           │  │   │
│  │  │  - Chunking (4MB)                        │  │   │
│  │  │  - Integrity Verification                │  │   │
│  │  │  - Garbage Collection                    │  │   │
│  │  └──────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────┘   │
│                                                           │
│  ┌─────────────────────────────────────────────────┐   │
│  │      Support Services                            │   │ Port 8081
│  │  ┌────────┐  ┌────────┐  ┌─────────────────┐  │   │
│  │  │ Health │  │ Metrics│  │ Admin/Cluster   │  │   │
│  │  └────────┘  └────────┘  └─────────────────┘  │   │
│  └─────────────────────────────────────────────────┘   │
│                                                           │
│  ┌─────────────────────────────────────────────────┐   │
│  │      Parquet Support                             │   │
│  │  ┌──────────────┐  ┌────────────────────────┐  │   │
│  │  │   Metadata   │  │      Optimizer         │  │   │
│  │  │    Cache     │  │  - Predicate Pushdown  │  │   │
│  │  │   (256MB)    │  │  - Column Pruning      │  │   │
│  │  └──────────────┘  └────────────────────────┘  │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Component Architecture

### 1. S3 API Layer (`crates/anode-s3`)

The S3 API layer provides S3-compatible HTTP endpoints using Axum web framework.

**Responsibilities:**
- Parse and validate S3 HTTP requests
- AWS Signature V4 authentication
- XML request/response serialization
- Route requests to storage engine
- Handle multipart uploads
- Implement batch operations

**Key Modules:**
- `handlers.rs` - Core S3 operation handlers (GET, PUT, DELETE, HEAD, LIST)
- `auth.rs` - AWS SigV4 authentication
- `multipart.rs` - Multipart upload management
- `batch.rs` - Batch delete operations
- `copy.rs` - Server-side object copy
- `xml.rs` - S3 XML response generation
- `router.rs` - HTTP routing and middleware

**Dependencies:**
- `anode-storage` - Storage operations
- `axum` - Web framework
- `tower` - HTTP middleware

### 2. Raft Consensus Layer (`crates/anode-raft`)

Provides distributed consensus using OpenRaft, ensuring strong consistency across the cluster.

**Responsibilities:**
- Leader election
- Log replication
- Snapshot management
- Membership changes
- Network communication between nodes

**Key Components:**
- `LogStore` - Persistent Raft log using redb
- `StateMachine` - Applies committed operations to storage
- `RaftNetworkManager` - gRPC-based inter-node communication
- `RaftConfig` - Configuration management

**Raft Operations:**
- Bucket creation/deletion (replicated)
- Object metadata operations (replicated)
- Cluster membership changes (replicated)
- Chunk reference counting (local, eventually consistent)

**Design Note:** The storage engine handles chunk-level operations locally without Raft coordination, while metadata operations go through Raft for consistency.

### 3. Storage Engine (`crates/anode-storage`)

Core storage implementation with content-addressed chunking and deduplication.

**Architecture:**

```
┌─────────────────────────────────────────────────────┐
│              StorageEngine                           │
├─────────────────────────────────────────────────────┤
│                                                       │
│  ┌──────────────────────────────────────────────┐  │
│  │  MetadataStore (redb - ACID compliant)       │  │
│  │                                               │  │
│  │  Tables:                                      │  │
│  │  - buckets:      bucket_name → BucketConfig  │  │
│  │  - objects:      (bucket, key) → Manifest    │  │
│  │  - chunk_refs:   chunk_id → ref_count        │  │
│  │  - chunk_sizes:  chunk_id → size             │  │
│  └──────────────────────────────────────────────┘  │
│                                                       │
│  ┌──────────────────────────────────────────────┐  │
│  │  ChunkManager                                 │  │
│  │                                               │  │
│  │  - Split data into 4MB chunks                │  │
│  │  - Compute SHA-256 for each chunk            │  │
│  │  - Deduplicate based on chunk ID             │  │
│  │  - Manage reference counts                   │  │
│  │  - Garbage collection                        │  │
│  │  - Integrity verification                    │  │
│  └──────────────────────────────────────────────┘  │
│                                                       │
│  ┌──────────────────────────────────────────────┐  │
│  │  BlobStore (file-based)                      │  │
│  │                                               │  │
│  │  Directory structure:                         │  │
│  │  data_dir/blobs/ab/cd/abcd1234...            │  │
│  │  (2-level hierarchy for filesystem scaling)  │  │
│  │                                               │  │
│  │  - Write chunks atomically                   │  │
│  │  - Read by chunk ID                          │  │
│  │  - Verify on read                            │  │
│  │  - Delete unreferenced chunks                │  │
│  └──────────────────────────────────────────────┘  │
│                                                       │
│  ┌──────────────────────────────────────────────┐  │
│  │  Integrity & Repair Systems                  │  │
│  │                                               │  │
│  │  - IntegrityChecker: Verify chunk hashes     │  │
│  │  - RepairManager: Restore from replicas      │  │
│  │  - Scrubber: Background verification         │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

**Responsibilities:**
- Object CRUD operations
- Chunk splitting and reassembly
- Content-addressed storage
- Deduplication via reference counting
- Integrity verification
- Garbage collection

**Key Modules:**
- `engine.rs` - Main storage interface
- `chunks.rs` - Chunking and deduplication logic
- `blobs.rs` - File-based chunk storage
- `metadata.rs` - redb-based metadata management
- `integrity.rs` - Hash verification
- `repair.rs` - Data recovery from replicas
- `scrubber.rs` - Background integrity checks

### 4. Protocol Layer (`crates/anode-protocol`)

Defines gRPC protocol buffers for inter-node communication.

**Services:**
- `ClusterService` - Node join/leave, cluster status
- `RaftService` - Raft RPC (AppendEntries, RequestVote, etc.)
- `ReplicationService` - Chunk replication and verification

**Common Types:**
- `NodeId` - Unique node identifier
- `ChunkId` - SHA-256 hash of chunk data
- `ObjectKey` - Bucket + object path
- `ObjectManifest` - List of chunk IDs composing an object

### 5. Client Library (`crates/anode-client`)

Rust SDK for interacting with Anode clusters.

**Features:**
- Full S3 API support
- Async/await with Tokio
- AWS SigV4 signing
- Automatic retry with exponential backoff
- Streaming uploads/downloads
- Multipart upload helpers
- Presigned URLs
- Fluent builder APIs

**Example:**
```rust
let client = AnodeClient::new(config)?;
client.create_bucket("my-bucket").await?;
client.put_object("my-bucket", "key", data).await?;
```

### 6. Parquet Support (`crates/anode-parquet`)

Optimizations for Apache Parquet analytical workloads.

**Features:**
- Metadata extraction and caching
- Row group statistics parsing
- Predicate pushdown (filter optimization)
- Column pruning (projection optimization)
- LRU cache (256MB default)

**Optimization Flow:**
```
Query Request
    ↓
Check metadata cache
    ↓
If miss: Read Parquet footer (range request)
    ↓
Parse row group statistics
    ↓
Prune row groups based on predicates
    ↓
Prune columns based on projection
    ↓
Return minimal byte ranges to fetch
```

### 7. Cluster Management (`crates/anode/src/cluster`)

Manages cluster topology and rebalancing.

**Components:**
- `ClusterManager` - Coordinates cluster operations
- `MembershipManager` - Tracks node membership
- `Discovery` - Node discovery mechanisms
- `ClusterBootstrap` - Initial cluster formation
- `Rebalancer` - Data rebalancing across nodes

**Placement Groups:**
- 128 virtual placement groups by default
- Objects assigned to placement groups via hash
- Placement groups distributed across nodes
- Enables incremental rebalancing when nodes join/leave

## Data Flow

### Write Path (PUT Object)

```
1. Client → S3 API
   ├─ Parse HTTP request
   ├─ Validate AWS SigV4 signature
   └─ Extract bucket, key, data

2. S3 API → Storage Engine
   └─ engine.put_object(bucket, key, data, metadata)

3. Storage Engine → Chunk Manager
   ├─ Split data into 4MB chunks
   ├─ Compute SHA-256 for each chunk
   └─ Return Vec<Chunk>

4. Chunk Manager → Blob Store (per chunk)
   ├─ Check if chunk exists (deduplication)
   ├─ If new: Write chunk to disk
   └─ Increment reference count in metadata

5. Storage Engine → Metadata Store
   ├─ Create ObjectManifest (list of chunk IDs)
   ├─ Store manifest in redb
   └─ Update bucket listing

6. Raft (asynchronously)
   ├─ Replicate metadata change
   ├─ Commit when majority acknowledges
   └─ Apply to state machine

7. Response → Client
   └─ Return ETag (hash of manifest)
```

### Read Path (GET Object)

```
1. Client → S3 API
   ├─ Parse HTTP request
   ├─ Validate AWS SigV4 signature
   └─ Extract bucket, key

2. S3 API → Storage Engine
   └─ engine.get_object(bucket, key)

3. Storage Engine → Metadata Store
   ├─ Look up ObjectManifest
   └─ Return list of chunk IDs

4. Storage Engine → Chunk Manager
   └─ retrieve_chunks(chunk_ids)

5. Chunk Manager → Blob Store (per chunk)
   ├─ Read chunk from disk
   ├─ Verify SHA-256 hash
   └─ Return chunk data

6. Chunk Manager
   └─ Concatenate chunks into complete object

7. Response → Client
   └─ Stream object data with metadata
```

### Parquet-Optimized Read Path (GET with Range)

```
1. Client → S3 API (Range: bytes=X-Y)

2. S3 API → Parquet Cache
   ├─ Check if metadata cached
   └─ If miss: Extract footer from object

3. Parquet Optimizer
   ├─ Parse row group statistics
   ├─ Apply predicate pushdown (WHERE clauses)
   ├─ Apply column pruning (SELECT columns)
   └─ Calculate minimal byte ranges

4. S3 API → Storage Engine (optimized ranges)
   └─ get_object_range(bucket, key, offset, length)

5. Storage Engine
   ├─ Map byte range to chunks
   ├─ Read only required chunks
   ├─ Extract requested byte range
   └─ Return partial object data

6. Response → Client
   └─ Return minimal data (5-10x reduction)
```

## Request Lifecycle

### Complete S3 PUT Request Lifecycle

```
┌──────────────────────────────────────────────────────────┐
│ 1. HTTP Request arrives at S3 API (port 8080)            │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 2. Axum Router                                            │
│    - Match route: PUT /{bucket}/{key}                    │
│    - Extract path parameters                              │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 3. Authentication Middleware                              │
│    - Parse Authorization header                           │
│    - Verify AWS SigV4 signature                          │
│    - Check access permissions                             │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 4. PUT Handler (anode-s3/handlers.rs)                   │
│    - Parse request body                                   │
│    - Extract metadata headers                             │
│    - Validate bucket exists                               │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 5. Storage Engine (anode-storage/engine.rs)             │
│    - StorageEngine::put_object()                         │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 6. Chunk Manager (anode-storage/chunks.rs)              │
│    a. Split into chunks (4MB each)                        │
│    b. Compute SHA-256 for each chunk                     │
│    c. Check if chunk exists (deduplication)              │
│    d. Write new chunks to blob store                     │
│    e. Update reference counts                             │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 7. Blob Store (anode-storage/blobs.rs)                  │
│    - Write chunk to: data_dir/blobs/ab/cd/abcd1234...   │
│    - Use atomic rename for durability                    │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 8. Metadata Store (anode-storage/metadata.rs)           │
│    - Create ObjectManifest                                │
│    - Store in redb: objects table                        │
│    - Update bucket listing                                │
│    - Transaction commit (ACID)                            │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 9. Raft Replication (anode-raft)                        │
│    - Propose metadata change to Raft                     │
│    - Replicate to followers                               │
│    - Wait for quorum acknowledgment                       │
│    - Commit to state machine                              │
└──────────────────────────────────────────────────────────┘
                          ↓
┌──────────────────────────────────────────────────────────┐
│ 10. Response                                              │
│     - Generate ETag (hash of manifest)                   │
│     - Build XML response                                  │
│     - Return HTTP 200 OK                                  │
└──────────────────────────────────────────────────────────┘
```

### Error Handling

At each stage, errors are converted to appropriate S3 error codes:
- `NoSuchBucket` → 404
- `AccessDenied` → 403
- `InternalError` → 500
- `InvalidArgument` → 400

## Consensus and Replication

### Raft Integration

**What gets replicated through Raft:**
- Bucket creation/deletion
- Object metadata (manifests, not data)
- Cluster membership changes

**What doesn't go through Raft:**
- Chunk data (replicated separately)
- Chunk reference counts (local per node)
- Read operations (served locally)

### Replication Flow

```
┌─────────────────────────────────────────────────────────┐
│                   Leader Node                            │
├─────────────────────────────────────────────────────────┤
│                                                           │
│ 1. Receive write request                                 │
│    ↓                                                      │
│ 2. Write to local storage                                │
│    ↓                                                      │
│ 3. Propose to Raft log                                   │
│    ├─→ Replicate to Follower 1                          │
│    ├─→ Replicate to Follower 2                          │
│    └─→ Replicate to Follower 3                          │
│    ↓                                                      │
│ 4. Wait for quorum (N/2 + 1)                            │
│    ↓                                                      │
│ 5. Commit log entry                                      │
│    ↓                                                      │
│ 6. Apply to state machine                                │
│    ↓                                                      │
│ 7. Acknowledge client                                    │
│    ↓                                                      │
│ 8. Background: Chunk replication                         │
│    ├─→ Send chunks to placement group nodes             │
│    └─→ Verify replication factor met                    │
└─────────────────────────────────────────────────────────┘
```

### Placement Groups

Anode uses placement groups (PGs) for scalable data distribution:

```
128 Placement Groups (default)
    ↓
Hash(bucket + key) % 128 → PG ID
    ↓
PG assigned to replication_factor nodes
    ↓
Chunks stored on PG's assigned nodes
```

**Benefits:**
- Parallel replication across PGs
- Incremental rebalancing (move PGs, not objects)
- Reduced metadata overhead
- Better load distribution

### Consistency Guarantees

- **Metadata**: Linearizable (via Raft)
- **Reads**: Read-after-write consistency
- **Cross-object operations**: Eventually consistent
- **Listing**: Snapshot isolation

## Storage Layer Design

### Content-Addressed Storage

Every chunk is identified by its SHA-256 hash:

```
Data: "Hello, World!"
   ↓
SHA-256: a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e
   ↓
Chunk ID: a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e
   ↓
File path: blobs/a5/91/a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e
```

### Chunking Strategy

**Why 4MB chunks?**
- Balance between deduplication granularity and overhead
- Good parallelization for network transfers
- Reasonable memory usage for in-flight chunks
- Aligns well with Parquet row group sizes (typically 64-128MB)

**Algorithm:**
```rust
fn split_into_chunks(data: &[u8]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let end = (offset + CHUNK_SIZE).min(data.len());
        let chunk_data = &data[offset..end];
        let chunk = Chunk::new(chunk_data.to_vec());
        chunks.push(chunk);
        offset = end;
    }

    chunks
}
```

### Deduplication

**Reference Counting:**
```
Chunk abcd1234...
    ↓
Referenced by:
    - object1 (bucket1/file1.txt)
    - object2 (bucket2/file2.txt)
    - object3 (bucket1/backup/file1.txt)
    ↓
Reference count: 3
    ↓
On delete of object1: ref_count -= 1
    ↓
When ref_count reaches 0: Delete chunk
```

**Storage Savings:**
- Identical files: ~100% deduplication
- Similar files: 30-90% deduplication
- Random data: ~0% deduplication

### Garbage Collection

**Mark and Sweep GC:**
```
1. Scan all chunks in blob store
2. For each chunk:
   - Get reference count from metadata
   - If ref_count == 0: Mark for deletion
3. Delete marked chunks
4. Report freed space
```

**Triggering:**
- Manual via admin API
- Scheduled (e.g., nightly)
- When storage threshold exceeded

### Integrity Verification

**Hash Verification:**
```
On read:
1. Read chunk from disk
2. Compute SHA-256 of data
3. Compare with chunk ID
4. If mismatch: Attempt repair from replica
5. If repair fails: Return error
```

**Background Scrubbing:**
```
Scrubber runs periodically:
1. Select chunk to verify (round-robin)
2. Read and verify hash
3. If corrupted:
   - Fetch from replica
   - Repair local copy
   - Log event
4. Track scrub progress and statistics
```

## Cluster Architecture

### Node Types

Anode uses a **unified node architecture** - all nodes are identical:

```
Every node has:
- S3 API endpoint (port 8080)
- gRPC endpoint (port 9090)
- Admin API (port 8081)
- Full storage engine
- Raft participation (voter or learner)
```

**No separate roles:**
- No dedicated metadata servers
- No dedicated storage nodes
- No dedicated coordinators
- All nodes can serve reads and writes

### Cluster Formation

**Bootstrap a new cluster:**
```bash
# Node 1 (bootstrap)
anode --node-id 1 --bootstrap \
      --grpc-addr 192.168.1.10:9090 \
      --s3-addr 0.0.0.0:8080

# Node 2 (join)
anode --node-id 2 --join 192.168.1.10:9090 \
      --grpc-addr 192.168.1.11:9090 \
      --s3-addr 0.0.0.0:8080

# Node 3 (join)
anode --node-id 3 --join 192.168.1.10:9090 \
      --grpc-addr 192.168.1.12:9090 \
      --s3-addr 0.0.0.0:8080
```

### Node Communication

```
┌─────────────┐    gRPC     ┌─────────────┐
│   Node 1    │◄───────────►│   Node 2    │
│  (Leader)   │             │ (Follower)  │
└─────────────┘             └─────────────┘
      ▲                            ▲
      │                            │
      │         gRPC               │ gRPC
      │                            │
      ▼                            ▼
┌─────────────┐             ┌─────────────┐
│   Node 3    │◄───────────►│   Node 4    │
│ (Follower)  │    gRPC     │ (Follower)  │
└─────────────┘             └─────────────┘

Communication types:
- Raft RPCs (AppendEntries, RequestVote)
- Chunk replication
- Health checks
- Cluster status
```

### Load Distribution

**Request routing:**
- Clients can connect to any node
- Leader handles all writes (Raft requirement)
- Followers forward writes to leader
- Reads served from local node (may be stale)
- Strongly consistent reads must go through leader

**Placement group distribution:**
```
3 nodes, 128 PGs, replication factor 3:

Node 1: PGs [0-42, 86-127]     (~85 PGs)
Node 2: PGs [0-42, 43-85]      (~85 PGs)
Node 3: PGs [43-127]           (~85 PGs)

Each PG on 3 nodes → Full replication
```

### Failure Scenarios

**Leader failure:**
```
1. Followers detect missing heartbeats
2. Election timeout triggers
3. Candidate requests votes
4. Majority elects new leader
5. New leader accepts writes

Downtime: 3-10 seconds (configurable)
```

**Follower failure:**
```
1. Leader detects missing heartbeats
2. Mark follower as unhealthy
3. Continue serving requests (quorum maintained)
4. When follower recovers:
   - Sync from leader's log
   - Replay missing entries
   - Resume normal operation

No downtime (assuming quorum)
```

**Network partition:**
```
Scenario: 3 nodes split into [1,2] and [3]

Partition 1 [1,2]:
- Has quorum (2/3)
- Can elect leader
- Accepts writes

Partition 2 [3]:
- No quorum (1/3)
- Cannot elect leader
- Rejects writes
- Reads may be stale

On heal:
- Node 3 syncs from leader
- Conflicting writes rejected (Raft guarantees)
```

## Design Decisions

### 1. Unified Node Architecture

**Decision:** All nodes run the same binary with S3 API, Raft, and storage.

**Rationale:**
- Simpler deployment (single binary)
- Lower operational complexity
- Better failure recovery (any node can become leader)
- Easier testing and development

**Trade-offs:**
- Less resource optimization (can't tune per role)
- Higher memory usage per node
- May not scale to 1000+ nodes

**Comparison:**
- **Ceph**: Separate MON, OSD, MDS daemons
- **MinIO**: Separate nodes possible via federation
- **Anode**: Single unified binary

### 2. Raft for Metadata Only

**Decision:** Use Raft for metadata, replicate chunks separately.

**Rationale:**
- Metadata is small (kilobytes per object)
- Chunk data is large (megabytes to gigabytes)
- Raft log would grow too large with data
- Allows async chunk replication for performance

**Trade-offs:**
- Two-phase commit (metadata then chunks)
- Eventual consistency for chunk placement
- More complex recovery logic

**Alternative considered:** Replicate all data through Raft (rejected due to log size)

### 3. Content-Addressed Storage

**Decision:** Use SHA-256 hashes as chunk identifiers.

**Rationale:**
- Automatic deduplication
- Integrity verification built-in
- Immutable chunks simplify caching
- Enables efficient repair and scrubbing

**Trade-offs:**
- Cannot do in-place updates
- Higher CPU usage (hashing)
- 64-byte chunk IDs vs. sequential IDs

**Benefit:** 30-90% storage savings in typical workloads

### 4. 4MB Chunk Size

**Decision:** Fixed 4MB chunks (not variable or larger).

**Rationale:**
- Good balance for network transfer
- Reasonable memory usage
- Works well with Parquet row groups
- Not too fine-grained (overhead)
- Not too coarse (less deduplication)

**Comparison:**
- **Git**: Variable chunking (average ~4KB)
- **S3**: No chunking (except multipart)
- **IPFS**: ~256KB chunks
- **Anode**: Fixed 4MB chunks

### 5. redb for Metadata

**Decision:** Use redb embedded database for metadata.

**Rationale:**
- Pure Rust (no C dependencies)
- ACID transactions
- No separate database server
- Efficient range queries for listing
- Good performance for metadata workloads

**Trade-offs:**
- Single-node only (replicated via Raft, not clustered)
- Less mature than SQLite or RocksDB
- Limited query capabilities (no SQL)

**Alternative considered:** SQLite (rejected due to C dependencies)

### 6. Placement Groups

**Decision:** Use 128 virtual placement groups (configurable).

**Rationale:**
- Enables incremental rebalancing
- Parallel replication operations
- Better load distribution
- Reduces rebalance overhead when nodes join/leave

**Mechanism:**
```
Object → hash(bucket + key) % 128 → PG → [Node1, Node2, Node3]
```

**Trade-off:** More complex placement logic vs. simpler consistent hashing

## Extension Points

### 1. Storage Backend

The storage engine is modular and can be extended:

```rust
// Current: File-based blob store
pub trait BlobStore {
    fn write_chunk(&self, chunk_id: &ChunkId, data: &[u8]) -> Result<()>;
    fn read_chunk(&self, chunk_id: &ChunkId) -> Result<Vec<u8>>;
    fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<()>;
}

// Potential implementations:
// - S3 backend (use S3 as backend storage)
// - Cloud storage (GCS, Azure Blob)
// - Object storage pool (erasure coding)
```

### 2. Authentication

Current: AWS SigV4 only

**Extension points:**
```rust
pub trait Authenticator {
    async fn authenticate(&self, request: &Request) -> Result<Principal>;
}

// Potential implementations:
// - LDAP/Active Directory
// - OAuth2/OIDC
// - API keys
// - mTLS certificates
```

### 3. Replication Strategy

Current: Full replication (factor 3)

**Extension points:**
```rust
pub trait ReplicationStrategy {
    fn replicas_for_object(&self, key: &ObjectKey) -> Vec<NodeId>;
}

// Potential implementations:
// - Erasure coding (6+3 Reed-Solomon)
// - Hybrid (hot = replicated, cold = erasure coded)
// - Geographic replication
```

### 4. Metadata Index

Current: redb with bucket/object tables

**Extension points:**
```rust
pub trait MetadataIndex {
    async fn index_object(&self, manifest: &ObjectManifest) -> Result<()>;
    async fn search(&self, query: &Query) -> Result<Vec<ObjectKey>>;
}

// Potential implementations:
// - Full-text search (Tantivy)
// - SQL interface (SQLite, PostgreSQL)
// - Time-series indexing
```

### 5. Lifecycle Policies

Currently not implemented

**Extension framework:**
```rust
pub trait LifecyclePolicy {
    async fn evaluate(&self, object: &ObjectManifest) -> Action;
}

pub enum Action {
    Keep,
    Transition(StorageClass),
    Delete,
}

// Policies:
// - Age-based deletion
// - Transition to cold storage
// - Transition to erasure coding
```

### 6. Monitoring and Observability

Current: Prometheus metrics

**Extension points:**
```rust
pub trait MetricsCollector {
    fn record_operation(&self, op: Operation, duration: Duration);
    fn record_error(&self, error: &Error);
}

// Potential implementations:
// - OpenTelemetry
// - StatsD
// - Custom metrics backend
```

### 7. Caching Layers

Current: Parquet metadata cache only

**Extension opportunities:**
- Object metadata cache (Redis)
- Hot object cache (in-memory LRU)
- Edge caching (CDN integration)
- Query result cache

## Summary

Anode's architecture provides:

1. **Simplicity**: Unified nodes, single binary, straightforward deployment
2. **Consistency**: Raft consensus for metadata, strong guarantees
3. **Efficiency**: Content-addressed storage, automatic deduplication
4. **Performance**: Parquet optimizations, placement groups, parallel operations
5. **Reliability**: Hash verification, background scrubbing, repair from replicas
6. **Extensibility**: Modular design with clear extension points

The design prioritizes:
- Operational simplicity over maximum scalability
- Strong consistency over eventual consistency
- Storage efficiency through deduplication
- Analytical workload optimization (Parquet)
- Pure Rust implementation for memory safety

Target use case: Small to medium clusters (3-50 nodes) requiring S3-compatible storage with strong consistency and analytical optimizations.
