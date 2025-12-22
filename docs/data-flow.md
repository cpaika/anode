# Anode Data Flow Diagrams

This document provides comprehensive data flow diagrams for understanding how data moves through the Anode distributed object storage system. These diagrams illustrate the complete request lifecycle for various operations including reads, writes, multipart uploads, replication, cluster changes, and failover scenarios.

## Table of Contents

- [System Overview](#system-overview)
- [Write Path (PUT Object)](#write-path-put-object)
- [Read Path (GET Object)](#read-path-get-object)
- [Multipart Upload Flow](#multipart-upload-flow)
- [Raft Replication Flow](#raft-replication-flow)
- [Cluster Membership Changes](#cluster-membership-changes)
- [Failover Scenarios](#failover-scenarios)

## System Overview

### Overall System Data Flow

This high-level diagram shows how data flows through the major components of an Anode node:

```mermaid
graph TB
    Client[S3 Client]

    subgraph "Anode Node"
        S3API[S3 API Layer<br/>Port 8080]
        Auth[Authentication<br/>AWS SigV4]
        Handlers[Request Handlers]
        MultipartMgr[Multipart Manager]

        subgraph "Raft Consensus"
            RaftAPI[Raft API<br/>Port 9090]
            RaftLog[Raft Log]
            StateMachine[State Machine]
            Network[Network Manager]
        end

        subgraph "Storage Engine"
            Engine[Storage Engine]
            ChunkMgr[Chunk Manager]
            MetaStore[Metadata Store<br/>redb]
            BlobStore[Blob Store<br/>file-based]
        end

        Admin[Admin API<br/>Port 8081]
        Metrics[Metrics/Health]
    end

    OtherNodes[Other Cluster Nodes]

    Client -->|HTTP/S3| S3API
    S3API --> Auth
    Auth --> Handlers
    Handlers --> MultipartMgr
    Handlers --> Engine

    Engine --> ChunkMgr
    ChunkMgr --> MetaStore
    ChunkMgr --> BlobStore

    Engine --> StateMachine
    StateMachine --> RaftLog
    RaftLog --> Network
    Network -->|gRPC| OtherNodes

    Client -->|Admin/Monitor| Admin
    Admin --> Metrics
    Metrics --> Engine
```

**Key Data Paths:**
- **S3 Requests**: Client → S3 API → Auth → Handlers → Storage Engine
- **Replication**: Storage Engine → Raft State Machine → Raft Log → Network → Other Nodes
- **Storage**: Chunk Manager → Metadata Store (for manifests) + Blob Store (for data)
- **Monitoring**: Admin API → Metrics → Storage Engine

## Write Path (PUT Object)

### Complete PUT Object Flow

This diagram shows the end-to-end data flow when a client uploads an object to Anode:

```mermaid
sequenceDiagram
    participant Client
    participant S3API as S3 API Layer
    participant Auth as Authentication
    participant Handler as PUT Handler
    participant Engine as Storage Engine
    participant ChunkMgr as Chunk Manager
    participant BlobStore as Blob Store
    participant MetaStore as Metadata Store
    participant Raft as Raft Consensus
    participant Followers as Follower Nodes

    Client->>S3API: PUT /bucket/key<br/>+ object data
    S3API->>Auth: Verify AWS SigV4 signature
    Auth->>Auth: Validate credentials
    Auth-->>S3API: Authenticated

    S3API->>Handler: Route to PUT handler
    Handler->>Handler: Validate bucket exists
    Handler->>Handler: Extract metadata headers

    Handler->>Engine: put_object(bucket, key, data, metadata)

    Engine->>ChunkMgr: split_into_chunks(data)
    ChunkMgr->>ChunkMgr: Split data into 4MB chunks
    loop For each chunk
        ChunkMgr->>ChunkMgr: Compute SHA-256 hash
        ChunkMgr->>ChunkMgr: chunk_id = hash(chunk_data)
    end
    ChunkMgr-->>Engine: Return Vec<Chunk>

    Engine->>BlobStore: Check if chunks exist (dedup)
    loop For each new chunk
        BlobStore->>BlobStore: Write to blobs/ab/cd/abcd1234...
        BlobStore->>BlobStore: Atomic rename for durability
        BlobStore->>MetaStore: Increment reference count
    end
    BlobStore-->>Engine: Chunks stored

    Engine->>Engine: Create ObjectManifest<br/>(chunk_ids + metadata)
    Engine->>MetaStore: Store manifest in objects table
    MetaStore->>MetaStore: ACID transaction
    MetaStore-->>Engine: Manifest committed

    Engine->>Raft: Propose metadata change
    Raft->>Raft: Append to local log
    Raft->>Followers: AppendEntries RPC
    Followers->>Followers: Append to local logs
    Followers-->>Raft: ACK

    Raft->>Raft: Wait for quorum (N/2 + 1)
    Raft->>Raft: Commit log entry
    Raft->>Raft: Apply to state machine
    Raft-->>Engine: Metadata replicated

    Engine-->>Handler: ObjectManifest with ETag
    Handler->>Handler: Generate ETag from manifest
    Handler-->>S3API: HTTP 200 OK + ETag
    S3API-->>Client: Success response

    Note over Raft,Followers: Async: Chunk replication<br/>to placement group nodes
```

**Key Steps Explained:**

1. **Authentication** (Lines 2-5): AWS SigV4 signature verification ensures the request is from an authorized user
2. **Chunking** (Lines 11-16): Data is split into 4MB chunks and each chunk is hashed with SHA-256
3. **Deduplication** (Lines 18-23): Chunks are deduplicated by checking if the hash already exists
4. **Manifest Creation** (Lines 26-29): An ObjectManifest records the list of chunk IDs that compose the object
5. **Raft Replication** (Lines 31-40): Metadata is replicated through Raft consensus for strong consistency
6. **Response** (Lines 42-44): Client receives success only after quorum commits the metadata

**Performance Characteristics:**
- **Latency**: Typically 10-100ms depending on object size and cluster configuration
- **Throughput**: Limited by chunk hashing speed and network bandwidth
- **Deduplication**: Identical chunks are only stored once, saving storage space

## Read Path (GET Object)

### Complete GET Object Flow

This diagram shows how an object is retrieved from Anode:

```mermaid
sequenceDiagram
    participant Client
    participant S3API as S3 API Layer
    participant Auth as Authentication
    participant Handler as GET Handler
    participant Engine as Storage Engine
    participant MetaStore as Metadata Store
    participant ChunkMgr as Chunk Manager
    participant BlobStore as Blob Store

    Client->>S3API: GET /bucket/key
    S3API->>Auth: Verify AWS SigV4 signature
    Auth-->>S3API: Authenticated

    S3API->>Handler: Route to GET handler
    Handler->>Handler: Validate bucket exists

    Handler->>Engine: get_object(bucket, key)
    Engine->>MetaStore: Lookup ObjectManifest
    MetaStore->>MetaStore: Query objects table<br/>key: (bucket, key)
    MetaStore-->>Engine: ObjectManifest<br/>(chunk_ids + metadata)

    Engine->>ChunkMgr: retrieve_chunks(chunk_ids)
    loop For each chunk_id
        ChunkMgr->>BlobStore: read_chunk(chunk_id)
        BlobStore->>BlobStore: Read blobs/ab/cd/abcd1234...
        BlobStore-->>ChunkMgr: chunk_data
        ChunkMgr->>ChunkMgr: Verify SHA-256 hash
        alt Hash mismatch (corruption)
            ChunkMgr->>ChunkMgr: Attempt repair from replica
            ChunkMgr-->>Handler: Error if repair fails
        end
    end
    ChunkMgr->>ChunkMgr: Concatenate chunks
    ChunkMgr-->>Engine: Complete object data

    Engine-->>Handler: (ObjectManifest, data)
    Handler->>Handler: Extract metadata headers
    Handler-->>S3API: HTTP 200 OK + headers + data
    S3API-->>Client: Stream object data
```

**Key Steps Explained:**

1. **Manifest Lookup** (Lines 7-10): The metadata store is queried to find which chunks compose the object
2. **Chunk Retrieval** (Lines 12-21): Each chunk is read from the blob store and verified
3. **Integrity Verification** (Lines 17-21): SHA-256 hash verification ensures data hasn't been corrupted
4. **Reassembly** (Line 22): Chunks are concatenated in order to reconstruct the original object
5. **Streaming** (Line 27): Data is streamed back to the client efficiently

**Performance Characteristics:**
- **Latency**: 5-50ms for small objects, scales with object size
- **Throughput**: Limited by disk I/O and network bandwidth
- **Integrity**: All data is verified on read, corruption is detected immediately

### Range Request (Partial Object Read)

For range requests (e.g., reading bytes 1000-2000 of an object):

```mermaid
sequenceDiagram
    participant Client
    participant Handler as GET Handler
    participant Engine as Storage Engine
    participant MetaStore as Metadata Store
    participant ChunkMgr as Chunk Manager

    Client->>Handler: GET /bucket/key<br/>Range: bytes=1000-2000
    Handler->>Engine: get_object_range(bucket, key, 1000, 1000)

    Engine->>MetaStore: Lookup ObjectManifest
    MetaStore-->>Engine: chunk_ids list

    Engine->>Engine: Map byte range to chunks<br/>chunk_offset = 1000 / 4MB = 0<br/>chunk_start_byte = 1000 % 4MB

    Engine->>ChunkMgr: Read only required chunks<br/>(chunk 0, starting at byte 1000)
    ChunkMgr-->>Engine: Partial data

    Engine->>Engine: Extract exact byte range
    Engine-->>Handler: 1000 bytes of data
    Handler-->>Client: HTTP 206 Partial Content
```

**Benefits:**
- **Efficiency**: Only reads necessary chunks, not the entire object
- **Parquet Optimization**: Critical for reading Parquet row groups without full file download
- **Bandwidth Savings**: Reduces network transfer for large files

## Multipart Upload Flow

### Multipart Upload Complete Flow

Multipart uploads allow clients to upload large objects (>5GB) in parts. This diagram shows the complete lifecycle:

```mermaid
sequenceDiagram
    participant Client
    participant S3API as S3 API
    participant MultipartMgr as Multipart Manager
    participant Engine as Storage Engine
    participant MetaStore as Metadata Store
    participant BlobStore as Blob Store

    rect rgb(240, 248, 255)
        Note over Client,MultipartMgr: Phase 1: Initiate Upload
        Client->>S3API: POST /bucket/key?uploads
        S3API->>MultipartMgr: create_upload(bucket, key, metadata)
        MultipartMgr->>MultipartMgr: Generate unique upload_id (UUID)
        MultipartMgr->>MultipartMgr: Store upload metadata in-memory
        MultipartMgr-->>S3API: upload_id
        S3API-->>Client: HTTP 200 + upload_id
    end

    rect rgb(255, 248, 240)
        Note over Client,BlobStore: Phase 2: Upload Parts (parallel)
        par Part 1
            Client->>S3API: PUT /bucket/key?partNumber=1&uploadId=xyz<br/>+ part_data_1
            S3API->>MultipartMgr: upload_part(upload_id, 1, data)
            MultipartMgr->>MultipartMgr: Validate part number (1-10000)
            MultipartMgr->>MultipartMgr: Validate size (5MB-5GB)
            MultipartMgr->>MultipartMgr: Compute ETag (SHA-256)
            MultipartMgr->>MultipartMgr: Store part in-memory
            MultipartMgr-->>S3API: ETag_1
            S3API-->>Client: HTTP 200 + ETag_1
        and Part 2
            Client->>S3API: PUT /bucket/key?partNumber=2&uploadId=xyz<br/>+ part_data_2
            S3API->>MultipartMgr: upload_part(upload_id, 2, data)
            MultipartMgr-->>S3API: ETag_2
            S3API-->>Client: HTTP 200 + ETag_2
        and Part N
            Client->>S3API: PUT /bucket/key?partNumber=N&uploadId=xyz<br/>+ part_data_N
            S3API->>MultipartMgr: upload_part(upload_id, N, data)
            MultipartMgr-->>S3API: ETag_N
            S3API-->>Client: HTTP 200 + ETag_N
        end
    end

    rect rgb(240, 255, 240)
        Note over Client,BlobStore: Phase 3: Complete Upload
        Client->>S3API: POST /bucket/key?uploadId=xyz<br/>+ part list (numbers + ETags)
        S3API->>MultipartMgr: complete_upload(upload_id, parts)

        MultipartMgr->>MultipartMgr: Validate all parts present
        MultipartMgr->>MultipartMgr: Verify ETags match
        MultipartMgr->>MultipartMgr: Sort parts by part number

        MultipartMgr->>Engine: Concatenate part data
        Engine->>Engine: Treat as regular PUT object
        Engine->>Engine: Chunk concatenated data (4MB chunks)

        loop For each chunk
            Engine->>BlobStore: Write chunk
            Engine->>MetaStore: Update ref counts
        end

        Engine->>MetaStore: Store ObjectManifest
        Engine-->>MultipartMgr: ObjectManifest + ETag

        MultipartMgr->>MultipartMgr: Clean up upload state
        MultipartMgr-->>S3API: CompleteMultipartResult
        S3API-->>Client: HTTP 200 + final ETag
    end

    Note over Client,BlobStore: Parts can be uploaded in any order<br/>and uploaded in parallel
```

**Key Phases:**

1. **Initiate** (Lines 2-9): Client requests an upload ID and provides metadata
2. **Upload Parts** (Lines 11-38): Parts can be uploaded in parallel, in any order
3. **Complete** (Lines 40-62): All parts are concatenated and stored as a single object

**Advantages:**
- **Parallelism**: Multiple parts can be uploaded simultaneously
- **Resumability**: Failed parts can be retried without re-uploading the entire object
- **Large Files**: Supports objects up to 5TB (10,000 parts × 5GB/part)

### Abort Multipart Upload

```mermaid
sequenceDiagram
    participant Client
    participant S3API as S3 API
    participant MultipartMgr as Multipart Manager

    Client->>S3API: DELETE /bucket/key?uploadId=xyz
    S3API->>MultipartMgr: abort_upload(upload_id)
    MultipartMgr->>MultipartMgr: Remove upload from in-memory map
    MultipartMgr->>MultipartMgr: Discard all uploaded parts
    MultipartMgr-->>S3API: Success
    S3API-->>Client: HTTP 204 No Content

    Note over MultipartMgr: Background cleanup removes<br/>orphaned part data
```

## Raft Replication Flow

### Metadata Replication via Raft

This diagram shows how metadata changes are replicated across the cluster using Raft consensus:

```mermaid
sequenceDiagram
    participant Client
    participant Leader as Leader Node
    participant LeaderRaft as Leader Raft
    participant Follower1 as Follower 1 Raft
    participant Follower2 as Follower 2 Raft
    participant Follower3 as Follower 3 Raft

    Client->>Leader: PUT /bucket/key (write request)

    rect rgb(255, 250, 240)
        Note over Leader,LeaderRaft: Step 1: Write to local storage
        Leader->>Leader: Store chunks in local blob store
        Leader->>Leader: Create ObjectManifest
        Leader->>Leader: Store manifest in local metadata store
    end

    rect rgb(240, 248, 255)
        Note over LeaderRaft,Follower3: Step 2: Propose to Raft
        Leader->>LeaderRaft: Propose metadata change
        LeaderRaft->>LeaderRaft: Append entry to local log<br/>Entry: {bucket, key, manifest}
        LeaderRaft->>LeaderRaft: Assign log index & term
    end

    rect rgb(240, 255, 240)
        Note over LeaderRaft,Follower3: Step 3: Replicate to followers
        par AppendEntries to all followers
            LeaderRaft->>Follower1: AppendEntries RPC<br/>(log entry + metadata)
            Follower1->>Follower1: Validate term & prev_log_index
            Follower1->>Follower1: Append to local log
            Follower1-->>LeaderRaft: ACK
        and
            LeaderRaft->>Follower2: AppendEntries RPC<br/>(log entry + metadata)
            Follower2->>Follower2: Validate term & prev_log_index
            Follower2->>Follower2: Append to local log
            Follower2-->>LeaderRaft: ACK
        and
            LeaderRaft->>Follower3: AppendEntries RPC<br/>(log entry + metadata)
            Follower3->>Follower3: Validate term & prev_log_index
            Follower3->>Follower3: Append to local log
            Follower3-->>LeaderRaft: ACK
        end
    end

    rect rgb(255, 240, 240)
        Note over LeaderRaft: Step 4: Achieve quorum
        LeaderRaft->>LeaderRaft: Count acknowledgments<br/>Need: 3 out of 5 (quorum)
        LeaderRaft->>LeaderRaft: Mark log entry as committed
        LeaderRaft->>LeaderRaft: Update commit_index
    end

    rect rgb(255, 255, 240)
        Note over LeaderRaft,Follower3: Step 5: Apply to state machines
        LeaderRaft->>Leader: Apply committed entry<br/>to state machine
        Leader->>Leader: Update in-memory bucket/object cache

        par Notify followers of commit
            LeaderRaft->>Follower1: Next AppendEntries<br/>(with updated commit_index)
            Follower1->>Follower1: Apply committed entries
        and
            LeaderRaft->>Follower2: Next AppendEntries<br/>(with updated commit_index)
            Follower2->>Follower2: Apply committed entries
        and
            LeaderRaft->>Follower3: Next AppendEntries<br/>(with updated commit_index)
            Follower3->>Follower3: Apply committed entries
        end
    end

    LeaderRaft-->>Leader: Metadata committed & replicated
    Leader-->>Client: HTTP 200 OK

    Note over Leader,Follower3: Background: Chunk data replication<br/>happens asynchronously after metadata commit
```

**Raft Guarantees:**

- **Linearizability**: All metadata operations are strictly ordered
- **Quorum Commit**: Changes are durable once majority acknowledges
- **Leader Election**: New leader elected within 3-10 seconds on failure
- **Log Consistency**: All nodes apply entries in the same order

### Background Chunk Replication

After metadata is committed, chunk data is replicated asynchronously:

```mermaid
graph LR
    subgraph "Leader Node"
        Leader[Storage Engine]
        BlobStore1[Blob Store]
    end

    subgraph "Placement Group"
        PG[Placement Group<br/>hash(bucket+key) % 128]
    end

    subgraph "Replica Node 1"
        Replica1[Storage Engine]
        BlobStore2[Blob Store]
    end

    subgraph "Replica Node 2"
        Replica2[Storage Engine]
        BlobStore3[Blob Store]
    end

    Leader -->|1. Identify PG nodes| PG
    PG -->|2. Send chunks| Replica1
    PG -->|2. Send chunks| Replica2

    Replica1 -->|3. Verify hashes| BlobStore2
    Replica2 -->|3. Verify hashes| BlobStore3

    BlobStore2 -->|4. ACK| Leader
    BlobStore3 -->|4. ACK| Leader

    Leader -->|5. Update replication status| BlobStore1
```

**Replication Process:**

1. **Placement Group Selection**: Object is assigned to a PG based on hash(bucket + key)
2. **Node Selection**: Replication factor nodes are chosen from the PG assignment
3. **Chunk Transfer**: Chunks are sent to replica nodes via gRPC
4. **Verification**: Replicas verify chunk hashes before acknowledging
5. **Status Tracking**: Leader tracks which chunks have achieved replication factor

## Cluster Membership Changes

### Adding a Node to the Cluster

This diagram shows the complete process of adding a new node to an existing cluster:

```mermaid
sequenceDiagram
    participant Admin as Administrator
    participant Leader as Leader Node
    participant Membership as Membership Manager
    participant LeaderRaft as Leader Raft
    participant NewNode as New Node
    participant Followers as Follower Nodes

    Admin->>NewNode: Start node with --join flag<br/>anode --node-id 4 --join leader:9090
    NewNode->>NewNode: Initialize storage engine
    NewNode->>NewNode: Start Raft in follower mode

    rect rgb(240, 248, 255)
        Note over NewNode,Leader: Step 1: Node contacts leader
        NewNode->>Leader: gRPC: RequestToJoin(node_info)
        Leader->>Membership: add_node(node_info)
        Membership->>Membership: Check if leader
        Membership->>Membership: Validate node doesn't exist
    end

    rect rgb(255, 248, 240)
        Note over LeaderRaft,NewNode: Step 2: Add as learner
        Membership->>LeaderRaft: add_learner(node_id, node_addr)
        LeaderRaft->>LeaderRaft: Add to learner list
        LeaderRaft->>NewNode: Begin log replication

        loop Catch up with log
            LeaderRaft->>NewNode: AppendEntries RPC<br/>(historical log entries)
            NewNode->>NewNode: Apply to local log
            NewNode->>NewNode: Apply to state machine
            NewNode-->>LeaderRaft: ACK
        end

        LeaderRaft->>LeaderRaft: Monitor learner progress
        Note over LeaderRaft,NewNode: Learner receives log entries<br/>but doesn't vote or count toward quorum
    end

    rect rgb(240, 255, 240)
        Note over LeaderRaft,Followers: Step 3: Promote to voter
        LeaderRaft->>LeaderRaft: Check learner is caught up<br/>(within N entries of leader)

        Membership->>LeaderRaft: change_membership(add node 4 as voter)
        LeaderRaft->>LeaderRaft: Create joint consensus config<br/>Old: {1,2,3} + New: {1,2,3,4}

        LeaderRaft->>Followers: AppendEntries(membership change)
        LeaderRaft->>NewNode: AppendEntries(membership change)

        Followers-->>LeaderRaft: ACK
        NewNode-->>LeaderRaft: ACK

        LeaderRaft->>LeaderRaft: Commit joint config<br/>(need majority of BOTH configs)

        LeaderRaft->>LeaderRaft: Create new config<br/>New: {1,2,3,4}
        LeaderRaft->>Followers: AppendEntries(new config)
        LeaderRaft->>NewNode: AppendEntries(new config)

        LeaderRaft->>LeaderRaft: Commit new config
    end

    rect rgb(255, 255, 240)
        Note over Leader,NewNode: Step 4: Update internal state
        Membership->>Membership: Add to members cache
        Membership->>Membership: Initialize health status
        Membership-->>Leader: Node added successfully
    end

    Leader-->>Admin: Success: Node 4 joined cluster

    Note over Leader,NewNode: Background: Data rebalancing<br/>Placement groups redistributed to new node
```

**Key Stages:**

1. **Learner Phase** (Lines 13-29): New node receives log entries but doesn't participate in voting
2. **Joint Consensus** (Lines 31-49): Raft uses two-phase membership change to ensure safety
3. **Full Member** (Lines 51-55): Node becomes a voting member of the cluster

**Safety Guarantees:**
- **No Split Brain**: Joint consensus ensures old and new configs both have quorum
- **Availability**: Cluster remains available during membership change
- **Consistency**: Log replication continues uninterrupted

### Removing a Node from the Cluster

```mermaid
sequenceDiagram
    participant Admin
    participant Leader as Leader Node
    participant Membership as Membership Manager
    participant LeaderRaft as Leader Raft
    participant Followers as Follower Nodes
    participant RemovedNode as Node 3 (removing)

    Admin->>Leader: Admin API: DELETE /cluster/nodes/3
    Leader->>Membership: remove_node(node_id=3)

    Membership->>Membership: Check if leader
    Membership->>Membership: Validate node exists
    Membership->>Membership: Check cluster will maintain quorum<br/>(N=5, removing 1, quorum still possible)

    Membership->>LeaderRaft: change_membership(remove node 3)

    rect rgb(255, 248, 240)
        Note over LeaderRaft,RemovedNode: Joint consensus phase
        LeaderRaft->>LeaderRaft: Create joint config<br/>Old: {1,2,3,4,5} + New: {1,2,4,5}

        par Replicate to all nodes
            LeaderRaft->>Followers: AppendEntries(membership change)
            Followers-->>LeaderRaft: ACK
        and
            LeaderRaft->>RemovedNode: AppendEntries(membership change)
            RemovedNode-->>LeaderRaft: ACK
        end

        LeaderRaft->>LeaderRaft: Commit joint config
    end

    rect rgb(240, 255, 240)
        Note over LeaderRaft,Followers: New config phase
        LeaderRaft->>LeaderRaft: Create new config<br/>New: {1,2,4,5}

        LeaderRaft->>Followers: AppendEntries(new config)
        Followers-->>LeaderRaft: ACK

        LeaderRaft->>RemovedNode: AppendEntries(new config)
        Note over RemovedNode: Node 3 receives notification<br/>that it's been removed

        LeaderRaft->>LeaderRaft: Commit new config
    end

    Membership->>Membership: Remove from members cache
    Membership->>Membership: Remove from health tracking
    Membership-->>Leader: Node removed successfully

    Leader-->>Admin: HTTP 200 OK

    RemovedNode->>RemovedNode: Detect removal from cluster
    RemovedNode->>RemovedNode: Transition to shutdown/standalone

    Note over Leader,Followers: Background: Data rebalancing<br/>Placement groups reassigned from removed node
```

**Important Considerations:**
- **Quorum Preservation**: Cannot remove a node if it would prevent quorum
- **Data Migration**: Placement groups must be reassigned before removal
- **Graceful Shutdown**: Removed node should cleanly shutdown after detecting removal

## Failover Scenarios

### Leader Failure and Election

This diagram shows what happens when the leader node fails:

```mermaid
sequenceDiagram
    participant Client
    participant Leader as Leader Node
    participant Follower1 as Follower 1
    participant Follower2 as Follower 2
    participant Follower3 as Follower 3
    participant Follower4 as Follower 4

    rect rgb(255, 240, 240)
        Note over Client,Leader: Normal operation
        Client->>Leader: PUT /bucket/key
        Leader->>Leader: Process write
        Leader->>Follower1: AppendEntries
        Leader->>Follower2: AppendEntries
        Leader->>Follower3: AppendEntries
        Leader->>Follower4: AppendEntries
        Leader-->>Client: HTTP 200 OK
    end

    rect rgb(255, 200, 200)
        Note over Leader: LEADER CRASHES
        Leader->>Leader: ☠️ Node failure
    end

    rect rgb(255, 255, 200)
        Note over Follower1,Follower4: Detection phase (heartbeat timeout)
        Follower1->>Follower1: No heartbeat from leader
        Follower2->>Follower2: No heartbeat from leader
        Follower3->>Follower3: No heartbeat from leader
        Follower4->>Follower4: No heartbeat from leader

        Note over Follower1,Follower4: Wait for election timeout<br/>(randomized: 150-300ms)
    end

    rect rgb(240, 255, 240)
        Note over Follower2,Follower4: Election phase
        Follower2->>Follower2: Election timeout expires first
        Follower2->>Follower2: Increment term: 5 → 6
        Follower2->>Follower2: Transition to CANDIDATE
        Follower2->>Follower2: Vote for self

        par Request votes from all nodes
            Follower2->>Follower1: RequestVote<br/>(term=6, last_log_index=100)
            Follower1->>Follower1: Check term and log
            Follower1->>Follower1: Grant vote (log is up-to-date)
            Follower1-->>Follower2: VoteGranted
        and
            Follower2->>Follower3: RequestVote<br/>(term=6)
            Follower3->>Follower3: Grant vote
            Follower3-->>Follower2: VoteGranted
        and
            Follower2->>Follower4: RequestVote<br/>(term=6)
            Follower4->>Follower4: Grant vote
            Follower4-->>Follower2: VoteGranted
        end

        Follower2->>Follower2: Count votes: 4/5 (self + 3)<br/>Majority achieved!
        Follower2->>Follower2: Transition to LEADER
    end

    rect rgb(240, 248, 255)
        Note over Follower2,Follower4: New leader establishes authority
        Follower2->>Follower1: AppendEntries (heartbeat)<br/>term=6, I am leader
        Follower2->>Follower3: AppendEntries (heartbeat)
        Follower2->>Follower4: AppendEntries (heartbeat)

        Follower1->>Follower1: Accept new leader (term 6)
        Follower3->>Follower3: Accept new leader
        Follower4->>Follower4: Accept new leader

        Follower1-->>Follower2: ACK
        Follower3-->>Follower2: ACK
        Follower4-->>Follower2: ACK
    end

    rect rgb(240, 255, 240)
        Note over Client,Follower2: Resume operations
        Client->>Follower2: PUT /bucket/key<br/>(client retries or redirects)
        Follower2->>Follower2: Process write (I am leader)
        Follower2->>Follower1: AppendEntries
        Follower2->>Follower3: AppendEntries
        Follower2->>Follower4: AppendEntries
        Follower2-->>Client: HTTP 200 OK
    end

    Note over Client,Follower4: Total downtime: 3-10 seconds<br/>(heartbeat timeout + election + stabilization)
```

**Timeline:**
1. **T+0s**: Leader fails
2. **T+0-1.5s**: Heartbeat timeout (followers detect missing leader)
3. **T+1.5-3s**: Election process (RequestVote RPCs)
4. **T+3-5s**: New leader establishes authority
5. **T+5s+**: Normal operation resumes

**Client Impact:**
- Writes in progress may fail and need retry
- Reads may return stale data during election
- 3-10 second window of unavailability for writes

### Follower Failure

```mermaid
sequenceDiagram
    participant Client
    participant Leader as Leader Node
    participant Follower1 as Follower 1
    participant Follower2 as Follower 2 (fails)
    participant Follower3 as Follower 3

    rect rgb(240, 255, 240)
        Note over Client,Follower3: Normal operation (5 nodes)
        Client->>Leader: PUT /bucket/key
        Leader->>Follower1: AppendEntries
        Leader->>Follower2: AppendEntries
        Leader->>Follower3: AppendEntries

        Follower1-->>Leader: ACK
        Follower2-->>Leader: ACK
        Follower3-->>Leader: ACK

        Leader->>Leader: Quorum achieved (3/5)
        Leader-->>Client: HTTP 200 OK
    end

    rect rgb(255, 200, 200)
        Note over Follower2: FOLLOWER 2 CRASHES
        Follower2->>Follower2: ☠️ Node failure
    end

    rect rgb(255, 255, 200)
        Note over Leader,Follower3: Continued operation (4 nodes)
        Client->>Leader: PUT /bucket/key2
        Leader->>Follower1: AppendEntries
        Leader->>Follower2: AppendEntries (timeout)
        Leader->>Follower3: AppendEntries

        Follower1-->>Leader: ACK
        Note over Follower2: No response
        Follower3-->>Leader: ACK

        Leader->>Leader: Quorum still achieved (3/4)<br/>Can tolerate 1 more failure
        Leader-->>Client: HTTP 200 OK
    end

    rect rgb(240, 255, 240)
        Note over Follower2: FOLLOWER 2 RECOVERS
        Follower2->>Follower2: ✓ Node restart
        Follower2->>Leader: AppendEntries response<br/>(log is behind)

        Leader->>Leader: Detect follower is behind<br/>(last_log_index mismatch)

        loop Catch up with log
            Leader->>Follower2: AppendEntries<br/>(missing log entries)
            Follower2->>Follower2: Append to log
            Follower2->>Follower2: Apply to state machine
            Follower2-->>Leader: ACK
        end

        Leader->>Leader: Follower 2 caught up
    end

    rect rgb(240, 255, 255)
        Note over Client,Follower3: Back to normal (5 nodes)
        Client->>Leader: PUT /bucket/key3
        Leader->>Follower1: AppendEntries
        Leader->>Follower2: AppendEntries
        Leader->>Follower3: AppendEntries

        Follower1-->>Leader: ACK
        Follower2-->>Leader: ACK (fully recovered)
        Follower3-->>Leader: ACK

        Leader-->>Client: HTTP 200 OK
    end

    Note over Client,Follower3: No downtime! Cluster maintained quorum<br/>Follower automatically caught up on recovery
```

**Key Points:**
- **No Downtime**: Cluster continues serving requests as long as quorum is maintained
- **Automatic Recovery**: Follower automatically catches up when it restarts
- **Degraded Redundancy**: Fault tolerance is reduced during failure (can tolerate N/2-1 failures)

### Network Partition (Split Brain Prevention)

This shows how Raft prevents split brain during a network partition:

```mermaid
sequenceDiagram
    participant Client1 as Client A
    participant Leader as Leader<br/>Node 1
    participant Follower1 as Follower<br/>Node 2
    participant Follower2 as Follower<br/>Node 3
    participant Follower3 as Follower<br/>Node 4
    participant Client2 as Client B

    rect rgb(240, 255, 240)
        Note over Leader,Follower3: Normal operation (4 nodes)
        Client1->>Leader: PUT /bucket/key1
        Leader-->>Client1: HTTP 200 OK
    end

    rect rgb(255, 200, 200)
        Note over Leader,Follower3: NETWORK PARTITION OCCURS
        Note over Leader,Follower1: Partition 1: Nodes 1, 2<br/>(2/4 - NO QUORUM)
        Note over Follower2,Follower3: Partition 2: Nodes 3, 4<br/>(2/4 - NO QUORUM)
    end

    rect rgb(255, 240, 240)
        Note over Leader,Follower1: Partition 1 behavior
        Client1->>Leader: PUT /bucket/key2
        Leader->>Leader: Attempt to replicate
        Leader->>Follower1: AppendEntries
        Follower1-->>Leader: ACK

        Leader->>Leader: Wait for quorum (3/4)
        Note over Leader: Timeout: Cannot reach nodes 3,4
        Leader->>Leader: Cannot achieve quorum (2/4)
        Leader->>Leader: Step down to FOLLOWER
        Leader-->>Client1: HTTP 503 Service Unavailable

        Note over Leader,Follower1: Both nodes in follower state<br/>No writes accepted
    end

    rect rgb(240, 240, 255)
        Note over Follower2,Follower3: Partition 2 behavior
        Follower2->>Follower2: Heartbeat timeout
        Follower3->>Follower3: Heartbeat timeout

        Follower2->>Follower2: Start election
        Follower2->>Follower3: RequestVote (term 6)
        Follower3->>Follower3: Grant vote

        Follower2->>Follower2: Count votes: 2/4<br/>NO QUORUM - remain in candidate

        Note over Follower2,Follower3: Election fails repeatedly<br/>No writes accepted

        Client2->>Follower2: PUT /bucket/key3
        Follower2-->>Client2: HTTP 503 Service Unavailable
    end

    rect rgb(200, 255, 200)
        Note over Leader,Follower3: NETWORK PARTITION HEALS
        Leader->>Leader: Detect connectivity restored
        Follower2->>Follower2: Detect connectivity restored
    end

    rect rgb(240, 255, 240)
        Note over Leader,Follower3: Re-election with full cluster
        Follower2->>Follower2: Election timeout
        Follower2->>Follower2: Increment term: 6 → 7

        par Request votes from all nodes
            Follower2->>Leader: RequestVote (term 7)
            Leader->>Leader: Higher term, grant vote
            Leader-->>Follower2: VoteGranted
        and
            Follower2->>Follower1: RequestVote
            Follower1-->>Follower2: VoteGranted
        and
            Follower2->>Follower3: RequestVote
            Follower3-->>Follower2: VoteGranted
        end

        Follower2->>Follower2: Quorum achieved (4/4)
        Follower2->>Follower2: Become LEADER (term 7)
    end

    rect rgb(240, 255, 255)
        Note over Client1,Follower3: Resume normal operation
        Client1->>Follower2: PUT /bucket/key4
        Follower2-->>Client1: HTTP 200 OK
    end

    Note over Leader,Follower3: SPLIT BRAIN PREVENTED!<br/>No partition had quorum<br/>No conflicting writes possible
```

**Safety Properties:**
- **No Split Brain**: Neither partition can achieve quorum, preventing divergent writes
- **Consistency Maintained**: No conflicting states can occur
- **Availability Traded**: Writes unavailable during partition (CP in CAP theorem)
- **Automatic Recovery**: Normal operation resumes when partition heals

**Partition Scenarios:**

| Cluster Size | Partition | Quorum Needed | Can Accept Writes? |
|--------------|-----------|---------------|--------------------|
| 3 nodes | 2 vs 1 | 2 | Partition with 2 nodes: YES<br/>Partition with 1 node: NO |
| 5 nodes | 3 vs 2 | 3 | Partition with 3 nodes: YES<br/>Partition with 2 nodes: NO |
| 5 nodes | 2 vs 2 vs 1 | 3 | All partitions: NO |

## Summary

These diagrams illustrate the complete data flow through Anode for various operations:

### Key Takeaways

1. **Write Path**:
   - Objects are chunked (4MB), deduplicated, and stored in content-addressed blob store
   - Metadata is replicated via Raft for strong consistency
   - Chunk data is replicated asynchronously after metadata commit

2. **Read Path**:
   - Metadata lookup is fast (local redb query)
   - Chunks are verified on read with SHA-256 hashes
   - Range requests read only necessary chunks

3. **Multipart Uploads**:
   - Supports large objects (>5GB) with parallel part uploads
   - Parts stored in-memory until completion
   - Final object is chunked and stored like normal PUT

4. **Raft Replication**:
   - Metadata changes go through Raft consensus
   - Quorum commit ensures durability
   - Log replication keeps all nodes consistent

5. **Cluster Membership**:
   - Joint consensus prevents split brain during changes
   - New nodes added as learners first, then promoted to voters
   - Removing nodes requires maintaining quorum

6. **Failover**:
   - Leader failure triggers election (3-10 second downtime)
   - Follower failure has no impact if quorum maintained
   - Network partitions safely prevent split brain by requiring quorum

### Performance Characteristics

| Operation | Latency | Throughput | Notes |
|-----------|---------|------------|-------|
| PUT object (small) | 10-100ms | 1000-5000 ops/sec | Includes Raft replication |
| GET object (small) | 5-50ms | 5000-10000 ops/sec | Local read, hash verification |
| PUT object (large) | Network limited | ~replication_factor | Bandwidth divided by replicas |
| Multipart part | 5-50ms per part | Parallel uploads | Limited by network/disk |
| Raft commit | 5-20ms | ~10000 ops/sec | Depends on cluster size |
| Leader election | 3-10 seconds | N/A | Randomized timeout prevents collisions |

### Design Philosophy

Anode's data flow prioritizes:
- **Strong Consistency**: Raft ensures all nodes agree on metadata
- **Efficiency**: Content-addressed storage enables automatic deduplication
- **Simplicity**: Unified architecture keeps data paths straightforward
- **Safety**: Hash verification and quorum commits prevent data loss
- **Performance**: Async chunk replication and local reads optimize throughput
