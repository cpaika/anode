# Anode Raft - Raft Consensus Integration

This crate provides a complete Raft consensus implementation for Anode using OpenRaft v0.9.

## Overview

The anode-raft crate integrates OpenRaft to provide:

- **Distributed consensus** for metadata operations (bucket management, object metadata)
- **Persistent log storage** using redb for durability
- **State machine** that applies replicated commands to the storage engine
- **Network layer** with gRPC support (integration point for proto-generated services)
- **Snapshot support** for efficient state transfer

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Anode Raft Node                       │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐         ┌──────────────┐            │
│  │   Config     │         │   Network    │            │
│  │  RaftConfig  │         │   Manager    │            │
│  └──────────────┘         └──────────────┘            │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │         OpenRaft (Raft<TypeConfig>)              │  │
│  ├──────────────────────────────────────────────────┤  │
│  │                                                   │  │
│  │  ┌────────────┐         ┌─────────────────────┐ │  │
│  │  │ LogStore   │         │   StateMachine      │ │  │
│  │  │  (redb)    │         │  (Storage Engine)   │ │  │
│  │  └────────────┘         └─────────────────────┘ │  │
│  │                                                   │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## Module Structure

### 1. `types.rs` - Type Configuration

Defines the core types for Raft:

- `TypeConfig`: OpenRaft type configuration
- `Request`/`Response`: Command request and response types
- `Command`: Enum of all replicated commands
  - `PutObject`: Add or update object metadata
  - `DeleteObject`: Remove object metadata
  - `CreateBucket`: Create a new bucket
  - `DeleteBucket`: Remove a bucket
- `BucketConfig`: Bucket configuration metadata

### 2. `config.rs` - Configuration

`RaftConfig` provides:

- Node ID and cluster name
- Data directory paths (logs, snapshots)
- Timing parameters (heartbeat interval, election timeouts)
- Snapshot configuration (threshold, chunk size, compression)
- Validation to ensure configuration correctness

### 3. `storage.rs` - Log Storage

`LogStore` implements persistent Raft log using redb:

- **Persistent storage** of log entries
- **Vote persistence** for election safety
- **Log operations**: append, truncate, purge
- **Thread-safe** with proper isolation

Tables:
- `logs`: Stores log entries by index
- `vote`: Stores current vote information
- `state`: Stores additional state metadata

### 4. `state_machine.rs` - State Machine

`StateMachine` applies commands to the storage engine:

- **Command execution**: Applies replicated commands in order
- **Snapshot creation**: Builds consistent snapshots of state
- **Snapshot installation**: Restores state from snapshots
- **Integration** with anode-storage for actual data operations

### 5. `network.rs` - Network Layer

`RaftNetworkManager` provides gRPC-based networking:

- **Connection pooling** for efficient communication
- **Node registration** and discovery
- **RPC implementation** (placeholder - requires proto integration)

Network methods:
- `append_entries`: Log replication
- `vote`: Leader election
- `install_snapshot`: Snapshot transfer

### 6. `error.rs` - Error Handling

`RaftError` provides comprehensive error types:

- Storage errors
- Network errors
- Leadership errors (NotLeader)
- Serialization errors
- Database errors
- Configuration errors

### 7. `lib.rs` - Public API

Public API includes:

- `RaftNodeBuilder`: Fluent builder for creating Raft nodes
- `create_raft_node`: Convenience function for quick setup
- `RaftStorage`: Combined log + state machine storage
- Re-exports of all major types

## Usage

### Basic Setup

```rust
use anode_raft::{create_raft_node, RaftConfig};
use anode_storage::StorageEngine;
use std::sync::Arc;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage engine
    let storage = Arc::new(StorageEngine::new(PathBuf::from("/data/storage"))?);

    // Create Raft node
    let raft = create_raft_node(
        1,                           // node_id
        "my-cluster",               // cluster_name
        PathBuf::from("/data/raft"), // data_dir
        storage,
    ).await?;

    // Initialize as single-node cluster
    raft.initialize(std::collections::BTreeSet::from([1])).await?;

    // Node is now ready to accept operations
    Ok(())
}
```

### Advanced Setup with Builder

```rust
use anode_raft::{RaftNodeBuilder, RaftConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create custom configuration
    let mut config = RaftConfig::new(
        1,
        "production-cluster",
        PathBuf::from("/data/raft")
    );
    config.heartbeat_interval_ms = 100;
    config.election_timeout_min_ms = 300;
    config.election_timeout_max_ms = 600;
    config.snapshot_threshold = 50000;

    // Validate configuration
    config.validate()?;

    // Create storage
    let storage = Arc::new(
        anode_storage::StorageEngine::new(config.data_dir.clone())?
    );

    // Build Raft node
    let raft = RaftNodeBuilder::new(config)
        .storage_engine(storage)
        .build()
        .await?;

    Ok(())
}
```

### Submitting Operations

```rust
use anode_raft::{Request, Command, BucketConfig};
use openraft::raft::ClientWriteRequest;

// Create a bucket
let request = Request {
    client_id: "client-1".to_string(),
    serial: 1,
    command: Command::CreateBucket {
        name: "my-bucket".to_string(),
        config: BucketConfig {
            name: "my-bucket".to_string(),
            replication_factor: 3,
            created_at: chrono::Utc::now().timestamp(),
        },
    },
};

// Submit to Raft
let response = raft.client_write(ClientWriteRequest::new(request)).await?;
```

## Testing

The crate includes comprehensive unit tests for all modules:

```bash
# Run all tests
cargo test -p anode-raft

# Run specific module tests
cargo test -p anode-raft --lib storage
cargo test -p anode-raft --lib state_machine
cargo test -p anode-raft --lib config
```

## Integration Points

### gRPC Network Layer

The network layer currently has placeholder implementations for gRPC calls.
To complete the integration:

1. Ensure proto files are compiled (requires `protoc`)
2. Import generated `RaftServiceClient` from anode-protocol
3. Replace placeholder implementations in `network.rs` with actual gRPC calls
4. Implement server-side handlers in main application

Example integration:

```rust
// In network.rs, replace the placeholder:
let channel = self.manager.get_connection(self.target).await
    .map_err(|e| RPCError::Network(NetworkError::from(e)))?;

let mut client = anode_protocol::raft::raft_service_client::RaftServiceClient::new(channel);

let response = client.append_entries(
    convert_to_proto(rpc)
).await?;

Ok(convert_from_proto(response.into_inner()))
```

### Storage Engine Integration

The state machine integrates with anode-storage for actual data operations.
Commands are applied to the storage engine after being replicated:

- `PutObject`: Validates manifest and stores metadata
- `DeleteObject`: Removes object and associated chunks
- `CreateBucket`/`DeleteBucket`: Manages bucket lifecycle

## Performance Considerations

1. **Log Compaction**: Automatic snapshots when log exceeds `snapshot_threshold`
2. **Batch Processing**: Multiple entries can be processed in single transaction
3. **Connection Pooling**: Network connections are cached and reused
4. **Async I/O**: All operations are non-blocking using tokio

## Safety and Correctness

1. **ACID Transactions**: redb provides transactional guarantees
2. **Crash Recovery**: Logs and state are persisted durably
3. **Type Safety**: Strong typing prevents many classes of bugs
4. **Error Handling**: Comprehensive error types with proper propagation

## Production Deployment

### Directory Structure

```
/data/raft/
├── logs/
│   └── raft.db          # Persistent log storage
└── snapshots/
    └── snapshot-*       # State snapshots
```

### Monitoring

Key metrics to monitor:

- Log size and growth rate
- Snapshot frequency and size
- Election timeout occurrences
- Leader changes
- Append entries latency
- Vote request/response patterns

### Backup and Recovery

1. **Regular snapshots**: Automatic based on `snapshot_threshold`
2. **Log archival**: Old logs can be purged after snapshot
3. **State recovery**: Can restore from snapshot + remaining logs

## Limitations and Future Work

1. **gRPC Integration**: Requires completion of proto-generated service integration
2. **Metrics**: Add Prometheus/OpenTelemetry instrumentation
3. **Dynamic Membership**: Add/remove nodes at runtime
4. **Read Optimization**: Implement read-only queries without log replication
5. **Snapshot Streaming**: Optimize large snapshot transfer

## Dependencies

- `openraft` (v0.9): Raft consensus implementation
- `redb` (v2.2): Embedded database for log storage
- `tonic` (v0.12): gRPC framework
- `tokio` (v1.42): Async runtime
- `serde`/`bincode`: Serialization
- `parking_lot`: High-performance locks

## License

Same as parent project (MIT OR Apache-2.0)
