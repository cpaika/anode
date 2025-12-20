# Anode Correctness Tests - Test Inventory

## Overview

2,054 lines of comprehensive distributed systems correctness tests across 6 modules.

## Test Modules

### 1. Linearizability Tests (`src/linearizability.rs`)

**Total Tests: 8**

- `test_single_key_concurrent_writes()` - Verifies concurrent writes to same key are linearizable
- `test_read_your_writes()` - Ensures read-your-writes consistency
- `test_monotonic_reads()` - Verifies reads never see older versions
- `test_multi_key_linearizability()` - Tests linearizability across multiple keys
- `test_causality_preservation()` - Ensures causal dependencies are maintained

**Helper Components:**
- `LinearizabilityChecker` - Implements Wing & Gong algorithm for checking linearizability
  - `check()` - Analyzes operation history for linearizability violations
  - `check_key_linearizable()` - Per-key linearizability verification
  - `get_stats()` - Operation statistics

### 2. Durability Tests (`src/durability.rs`)

**Total Tests: 7**

- `test_data_survives_restart()` - Data persists through graceful restarts
- `test_data_survives_crash()` - Data persists through crashes (SIGKILL)
- `test_committed_writes_durable()` - All acknowledged writes are durable
- `test_fsync_durability()` - Data survives power loss simulation
- `test_uncommitted_write_behavior()` - Uncommitted writes handled atomically
- `test_durability_concurrent_failures()` - Durability with concurrent node failures
- `test_partial_write_durability()` - Partial writes complete fully or not at all

**Key Features:**
- Simulates various failure modes (crash, power loss, network partition)
- Tests both graceful and ungraceful shutdown scenarios
- Verifies atomicity of write operations

### 3. Consistency Tests (`src/consistency.rs`)

**Total Tests: 5**

- `test_eventual_consistency()` - Replicas eventually converge
- `test_consistency_during_partition()` - Consistency during network partitions
- `test_metadata_data_consistency()` - Metadata matches actual data
- `test_consistency_after_recovery()` - Consistency maintained after recovery
- `test_cross_pg_consistency()` - Consistency across placement groups

**Key Features:**
- Tests both strong and eventual consistency
- Verifies consistency under various failure scenarios
- Validates metadata-data coherence

### 4. Replication Tests (`src/replication.rs`)

**Total Tests: 9**

- `test_basic_replication()` - Data replicated to multiple nodes
- `test_replication_factor()` - Correct number of replicas maintained
- `test_replication_after_failure()` - Re-replication after node failure
- `test_replication_during_partition()` - Replication during partitions
- `test_chunk_replication()` - Large objects chunked and replicated correctly
- `test_replication_rebalancing()` - Data rebalances after node addition
- `test_chunk_reference_counting()` - Deduplication and reference counting
- `test_erasure_coding_replication()` - Erasure coding provides fault tolerance
- `test_replication_with_delays()` - Replication under rapid writes

**Key Features:**
- Tests various replication strategies (full replication, erasure coding)
- Verifies reference counting for deduplication
- Tests replication under load and failures

### 5. Recovery Tests (`src/recovery.rs`)

**Total Tests: 7**

- `test_data_rebuild_after_node_loss()` - Data rebuilt after permanent loss
- `test_corruption_detection_and_repair()` - Corrupted data detected and repaired
- `test_recovery_after_total_shutdown()` - Recovery from total cluster shutdown
- `test_minority_failure_recovery()` - Recovery from minority node failures
- `test_recovery_with_concurrent_operations()` - Recovery during operations
- `test_raft_log_replay()` - Raft log correctly replayed after crash
- `test_snapshot_restore()` - Snapshots created and restored correctly

**Key Features:**
- Tests self-healing capabilities
- Verifies Raft log replay and snapshot mechanisms
- Tests recovery under concurrent load

### 6. Common Utilities (`src/lib.rs`)

**Helper Functions:**
- `init_tracing()` - Initialize logging for tests
- `test_bucket()` - Generate test bucket names
- `test_key()` - Generate test object keys
- `random_data()` - Generate random test data
- `deterministic_data()` - Generate deterministic test data
- `test_value()` - Create test values from strings

## Test Statistics

| Module | Tests | Lines of Code |
|--------|-------|---------------|
| Linearizability | 8 | ~480 |
| Durability | 7 | ~388 |
| Consistency | 5 | ~243 |
| Replication | 9 | ~434 |
| Recovery | 7 | ~432 |
| Common | - | ~77 |
| **Total** | **36** | **~2,054** |

## Running Tests

```bash
# Run all tests
cargo test --package anode-correctness-tests

# Run specific module
cargo test --package anode-correctness-tests linearizability

# Run with logging
RUST_LOG=info cargo test --package anode-correctness-tests -- --nocapture

# Run ignored tests (require running cluster)
cargo test --package anode-correctness-tests -- --ignored
```

## Test Properties Verified

### Correctness Properties
- ✅ Linearizability (single-key and multi-key)
- ✅ Read-your-writes consistency
- ✅ Monotonic reads
- ✅ Causal consistency
- ✅ Eventual consistency
- ✅ Metadata-data consistency

### Durability Properties
- ✅ Persistence across graceful restarts
- ✅ Persistence across crashes
- ✅ Atomicity of writes
- ✅ fsync correctness
- ✅ Durability during concurrent failures

### Replication Properties
- ✅ Replication factor maintained
- ✅ Re-replication after failures
- ✅ Chunk-level replication
- ✅ Reference counting and deduplication
- ✅ Erasure coding fault tolerance

### Recovery Properties
- ✅ Data rebuild after node loss
- ✅ Corruption detection and repair
- ✅ Total cluster shutdown recovery
- ✅ Raft log replay
- ✅ Snapshot creation and restoration

## Dependencies

### Test Harness (`anode-test-harness`)
- `TestCluster` - Multi-node cluster management
- `OperationHistory` - Operation recording and tracking
- `Assertions` - Helper assertion functions
- Test utilities for workload generation and chaos injection

### External Dependencies
- `tokio` - Async runtime
- `reqwest` - HTTP client for S3 API
- `tracing` - Logging and diagnostics
- `futures` - Async utilities
- `rand` - Random data generation

## CI Integration

These tests should be run in CI pipelines:

```yaml
- name: Correctness Tests
  run: |
    cargo build --bin anode
    cargo test --package anode-correctness-tests
  timeout-minutes: 30

- name: Extended Correctness Tests
  run: |
    cargo test --package anode-correctness-tests -- --ignored
  timeout-minutes: 60
  if: github.event_name == 'push' && github.ref == 'refs/heads/main'
```

## Future Enhancements

- [ ] Model checking integration (TLA+)
- [ ] Full Knossos linearizability checker
- [ ] Byzantine fault injection
- [ ] Network chaos (packet loss, delays, reordering)
- [ ] Performance regression detection
- [ ] Distributed tracing integration
- [ ] Fuzzing for edge cases
- [ ] Property-based testing with proptest
