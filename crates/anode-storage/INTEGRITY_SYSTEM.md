# Data Integrity Verification and Repair System

This document describes the data integrity verification and repair system implemented for anode storage.

## Overview

The integrity system provides three main components:

1. **IntegrityChecker** (`src/integrity.rs`) - Verifies data integrity
2. **RepairManager** (`src/repair.rs`) - Repairs corrupted or missing data
3. **Scrubber** (`src/scrubber.rs`) - Background continuous integrity checking

## Components

### 1. Integrity Checker (`src/integrity.rs`)

The `IntegrityChecker` provides comprehensive data integrity verification:

#### Features:
- **Full System Scan**: Verifies all chunks and objects in the storage system
- **Individual Object Verification**: Check integrity of specific objects
- **Chunk Verification**: Validate individual chunks against their content-addressed IDs
- **Orphan Detection**: Find chunks not referenced by any object
- **Reference Count Verification**: Ensure metadata reference counts match actual usage

#### Key Types:

```rust
pub struct IntegrityReport {
    pub total_objects: usize,
    pub total_chunks: usize,
    pub corrupted_chunks: Vec<CorruptedChunk>,
    pub missing_chunks: Vec<MissingChunk>,
    pub orphaned_chunks: Vec<ChunkId>,
    pub reference_count_errors: Vec<RefCountError>,
    pub verification_time: Duration,
}
```

#### Usage Example:

```rust
let checker = IntegrityChecker::new(metadata, blobs);

// Run full integrity scan
let report = checker.full_scan().await?;

if !report.is_healthy() {
    println!("Found {} issues", report.total_issues());
}

// Verify specific object
let is_valid = checker.verify_object(&object_key).await?;
```

### 2. Repair Manager (`src/repair.rs`)

The `RepairManager` repairs data integrity issues by fetching from replicas:

#### Features:
- **Corrupted Chunk Repair**: Fetch valid copy from cluster replicas
- **Missing Chunk Rebuild**: Reconstruct missing chunks from available nodes
- **Reference Count Correction**: Fix metadata inconsistencies
- **Orphan Cleanup**: Remove unreferenced chunks

#### Key Types:

```rust
pub struct RepairResult {
    pub chunks_repaired: usize,
    pub chunks_failed: usize,
    pub bytes_transferred: u64,
    pub repair_time: Duration,
    pub errors: Vec<RepairError>,
}
```

#### Usage Example:

```rust
let repair_manager = RepairManager::new(metadata, blobs, cluster_client);

// Repair all issues from integrity report
let result = repair_manager.repair(&integrity_report).await?;

println!("Repaired {}/{} chunks",
    result.chunks_repaired,
    result.chunks_repaired + result.chunks_failed
);

// Repair single chunk
repair_manager.repair_chunk(&chunk_id).await?;
```

#### Cluster Client Interface:

The repair system uses a `ClusterClient` trait for fetching data from other nodes:

```rust
pub trait ClusterClient: Send + Sync {
    fn fetch_chunk_from_node(&self, chunk_id: &ChunkId, node_id: NodeId) -> Result<Option<Vec<u8>>>;
    fn find_chunk_replicas(&self, chunk_id: &ChunkId) -> Result<Vec<NodeId>>;
}
```

A `MockClusterClient` is provided for testing.

### 3. Background Scrubber (`src/scrubber.rs`)

The `Scrubber` runs continuous background integrity checks:

#### Features:
- **Scheduled Scanning**: Periodic full integrity scans
- **Rate Limiting**: Configurable chunks-per-second verification rate
- **Auto-Repair**: Optionally repair issues as they're found
- **Progress Tracking**: Monitor scrub progress and status
- **Manual Triggers**: Force scrub runs outside of schedule

#### Configuration:

```rust
pub struct ScrubberConfig {
    pub full_scrub_interval: Duration,    // How often to run (default: 1 week)
    pub chunks_per_second: u32,           // Rate limiting (default: 100)
    pub auto_repair: bool,                 // Auto-repair found issues (default: false)
    pub scrub_on_startup: bool,           // Run on startup (default: false)
}
```

#### Usage Example:

```rust
let scrubber = Arc::new(Scrubber::new(
    checker,
    repair_manager,
    ScrubberConfig::default()
        .with_interval(Duration::from_secs(7 * 24 * 60 * 60)) // Weekly
        .with_auto_repair(true)
));

// Start background task
let handle = scrubber.start();

// Check status
let status = scrubber.status();
println!("Last scrub: {:?}", status.last_scrub);
println!("Next scrub: {:?}", status.next_scrub);

// Manual trigger
let result = scrubber.trigger_scrub().await?;
```

## Testing

All three modules include comprehensive unit tests:

### Running Tests:

```bash
# Run all storage tests
cargo test --package anode-storage

# Run integrity tests only
cargo test --package anode-storage integrity::tests

# Run repair tests only
cargo test --package anode-storage repair::tests

# Run scrubber tests only
cargo test --package anode-storage scrubber::tests
```

### Test Coverage:

#### Integrity Tests:
- Valid chunk verification
- Corrupted chunk detection
- Missing chunk detection
- Object verification
- Orphaned chunk scanning
- Reference count verification
- Full scan with healthy and unhealthy systems

#### Repair Tests:
- Orphan cleanup
- Reference count correction (increment/decrement)
- Missing chunk rebuild from replicas
- Corrupted chunk repair
- Full report repair
- No replica error handling

#### Scrubber Tests:
- Configuration
- Healthy system scanning
- Corruption detection
- Auto-repair functionality
- Status tracking
- Manual trigger
- Progress monitoring

## Integration

### Adding to Storage Engine:

```rust
// In your storage engine initialization
let checker = Arc::new(IntegrityChecker::new(
    metadata.clone(),
    blobs.clone(),
));

let repair_manager = Arc::new(RepairManager::new(
    metadata.clone(),
    blobs.clone(),
    cluster_client.clone(),
));

let scrubber = Arc::new(Scrubber::new(
    checker,
    repair_manager,
    ScrubberConfig::default()
        .with_interval(Duration::from_secs(7 * 24 * 60 * 60))
        .with_auto_repair(true)
        .with_scrub_on_startup(false),
));

// Start background scrubbing
let scrubber_handle = scrubber.start();
```

## Architecture Notes

### Chunk ID Types:

The storage layer uses a string-based `ChunkId` (hex-encoded SHA-256 hash) for filesystem compatibility, while the protocol layer uses `ChunkId([u8; 32])`. Conversions are handled via `to_hex()` and `from_hex()` methods.

### Error Handling:

All operations return `Result<T, StorageError>` and include detailed error information for debugging.

### Performance Considerations:

1. **Rate Limiting**: The scrubber respects the `chunks_per_second` limit to avoid overwhelming the system
2. **Incremental Progress**: Large scans track progress and can be monitored
3. **Replica Selection**: Repair tries multiple replicas if the first fails
4. **Hash Verification**: All repaired chunks are verified before being written

## Future Enhancements

Potential improvements:

1. **Metrics Collection**: Add Prometheus metrics for monitoring
2. **Partial Scans**: Support scanning specific buckets or prefixes
3. **Priority Repair**: Prioritize repair of frequently accessed objects
4. **Checkpointing**: Resume interrupted scans
5. **Parallel Verification**: Verify multiple chunks concurrently
6. **Smart Scheduling**: Adjust scan frequency based on error rates
7. **Notification System**: Alert on integrity issues

## API Summary

### Exported Types:

From `lib.rs`:
```rust
pub use integrity::{
    CorruptedChunk,
    IntegrityChecker,
    IntegrityReport as DetailedIntegrityReport,
    MissingChunk,
    RefCountError,
};

pub use repair::{
    ClusterClient,
    MockClusterClient,
    RepairError,
    RepairManager,
    RepairResult as DetailedRepairResult,
};

pub use scrubber::{
    ScrubProgress,
    ScrubResult,
    ScrubStatus,
    Scrubber,
    ScrubberConfig,
};
```

## Dependencies

New dependencies added to `Cargo.toml`:
- `chrono = "0.4"` - For timestamp handling in scrubber

All other dependencies were already present.
