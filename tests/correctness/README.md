# Anode Correctness Tests

Comprehensive distributed systems correctness testing suite for Anode, inspired by [Jepsen](https://jepsen.io/) and modern distributed systems testing methodologies.

## Overview

This test suite verifies fundamental correctness properties of the Anode distributed object storage system:

- **Linearizability** - All operations appear to execute atomically at some point between invocation and completion
- **Durability** - Committed data survives failures, crashes, and restarts
- **Consistency** - All replicas eventually converge to the same state
- **Replication** - Data is correctly replicated across nodes with proper fault tolerance
- **Recovery** - The system can recover from various failure scenarios

## Test Modules

### Linearizability Tests (`src/linearizability.rs`)

Tests that verify linearizability properties using operation history tracking:

- `test_single_key_concurrent_writes()` - Concurrent writes to the same key
- `test_read_your_writes()` - Read-your-writes consistency
- `test_monotonic_reads()` - Reads never return older versions
- `test_multi_key_linearizability()` - Linearizability across multiple keys
- `test_causality_preservation()` - Causal dependencies are preserved

**Key Implementation**: `LinearizabilityChecker` implements a simplified Wing & Gong algorithm for checking linearizability by verifying that all reads return values from writes that completed before the read invocation.

### Durability Tests (`src/durability.rs`)

Tests that verify data persistence across various failure modes:

- `test_data_survives_restart()` - Data persists through graceful node restarts
- `test_data_survives_crash()` - Data persists through ungraceful crashes (SIGKILL)
- `test_committed_writes_durable()` - All acknowledged writes are durable
- `test_fsync_durability()` - Data survives power loss simulation
- `test_uncommitted_write_behavior()` - Uncommitted writes are handled atomically
- `test_durability_concurrent_failures()` - Durability with concurrent node failures
- `test_partial_write_durability()` - Partial writes complete fully or not at all

### Consistency Tests (`src/consistency.rs`)

Tests that verify consistency properties across replicas:

- `test_eventual_consistency()` - Replicas eventually converge
- `test_consistency_during_partition()` - Consistency maintained during network partitions
- `test_metadata_data_consistency()` - Metadata matches actual data state
- `test_consistency_after_recovery()` - Consistency maintained after node recovery
- `test_cross_pg_consistency()` - Consistency across placement groups

### Replication Tests (`src/replication.rs`)

Tests that verify replication correctness:

- `test_basic_replication()` - Data is replicated to multiple nodes
- `test_replication_factor()` - Correct number of replicas are maintained
- `test_replication_after_failure()` - Re-replication after node failure
- `test_replication_during_partition()` - Replication during network partitions
- `test_chunk_replication()` - Large objects are correctly chunked and replicated
- `test_chunk_reference_counting()` - Deduplication and reference counting work correctly
- `test_erasure_coding_replication()` - Erasure coding provides fault tolerance
- `test_replication_with_delays()` - Replication under rapid write loads

### Recovery Tests (`src/recovery.rs`)

Tests that verify recovery and self-healing:

- `test_data_rebuild_after_node_loss()` - Data is rebuilt after permanent node loss
- `test_corruption_detection_and_repair()` - Corrupted data is detected and repaired
- `test_recovery_after_total_shutdown()` - Cluster recovers from total shutdown
- `test_minority_failure_recovery()` - Recovery from minority node failures
- `test_recovery_with_concurrent_operations()` - Recovery while operations continue
- `test_raft_log_replay()` - Raft log is correctly replayed after crash
- `test_snapshot_restore()` - Snapshots are correctly created and restored

## Running the Tests

### Prerequisites

1. Build the Anode binary:
   ```bash
   cargo build --bin anode
   ```

2. Ensure sufficient system resources (tests spawn multiple nodes):
   - At least 4GB RAM
   - Available ports in range 10000-60000
   - Sufficient disk space for temporary test data

### Run All Tests

```bash
cargo test --package anode-correctness-tests
```

### Run Specific Test Modules

```bash
# Linearizability tests
cargo test --package anode-correctness-tests linearizability

# Durability tests
cargo test --package anode-correctness-tests durability

# Consistency tests
cargo test --package anode-correctness-tests consistency

# Replication tests
cargo test --package anode-correctness-tests replication

# Recovery tests
cargo test --package anode-correctness-tests recovery
```

### Run Individual Tests

```bash
# Run a specific test
cargo test --package anode-correctness-tests test_single_key_concurrent_writes

# Run with logging
RUST_LOG=info cargo test --package anode-correctness-tests test_data_survives_crash -- --nocapture
```

### Running Ignored Tests

Many tests are marked with `#[ignore]` because they require a running cluster. To run them:

```bash
cargo test --package anode-correctness-tests -- --ignored
```

## Test Architecture

### Test Harness

The tests use the `anode-test-harness` crate which provides:

- **TestCluster** - Manages multi-node test clusters
- **OperationHistory** - Tracks operation invocations and completions
- **Assertions** - Helper functions for common assertions
- **Chaos** - Controlled failure injection
- **Workload** - Realistic workload generation

### Operation Recording

Tests record operations with precise timestamps:

```rust
let history = OperationHistory::new();
let op_id = history.record_invoke(operation).await;
// ... perform operation ...
history.record_complete(op_id, result).await;
```

This enables linearizability checking by analyzing the temporal ordering of operations.

### Failure Injection

Tests can inject various failures:

```rust
cluster.kill_node(0).await?;           // Simulate crash (SIGKILL)
cluster.restart_node(0).await?;        // Restart node
cluster.partition_node(0).await?;      // Network partition
cluster.heal_partition(0).await?;      // Heal partition
```

## Test Development Guidelines

### Writing New Tests

1. **Start simple** - Test one property at a time
2. **Use helpers** - Leverage assertion helpers from test harness
3. **Record operations** - Use OperationHistory for linearizability tests
4. **Clean up** - Always stop clusters in tests (use `?` operator)
5. **Add documentation** - Explain what property is being tested

### Example Test Structure

```rust
pub async fn test_my_property() -> Result<()> {
    crate::init_tracing();
    info!("Starting my property test");

    // Setup
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    // Test body
    // ... perform operations ...

    // Assertions
    assert_object_exists(&cluster, bucket, key).await?;

    // Cleanup
    cluster.stop().await?;
    info!("My property test passed");
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_my_property_wrapper() {
    let _ = test_my_property().await;
}
```

### Common Patterns

**Wait for stabilization:**
```rust
sleep(Duration::from_secs(5)).await;
```

**Concurrent operations:**
```rust
let mut tasks = JoinSet::new();
for i in 0..10 {
    tasks.spawn(async move { /* ... */ });
}
while tasks.join_next().await.is_some() {}
```

**Check eventual consistency:**
```rust
assert_eventually(
    || async { check_condition().await },
    Duration::from_secs(30),
    Duration::from_millis(100),
).await?;
```

## Interpreting Test Results

### Success

Tests pass when all assertions succeed and the cluster is properly cleaned up.

### Failures

Common failure modes:

1. **Timeout** - Cluster didn't stabilize or operation took too long
   - Increase timeouts or check node logs

2. **Assertion failure** - Correctness property violated
   - This indicates a real bug in the system
   - Export operation history for analysis

3. **Node start failure** - Node didn't start properly
   - Check port availability
   - Verify binary path

### Debugging

Enable verbose logging:
```bash
RUST_LOG=debug cargo test --package anode-correctness-tests test_name -- --nocapture
```

Export operation history:
```rust
let json = history.export_json().await?;
std::fs::write("history.json", json)?;
```

## Continuous Integration

These tests should be run in CI on every commit:

```yaml
- name: Run correctness tests
  run: |
    cargo build --bin anode
    cargo test --package anode-correctness-tests
```

For long-running tests:
```yaml
- name: Run extended correctness tests
  run: |
    cargo test --package anode-correctness-tests -- --ignored --test-threads=1
  timeout-minutes: 60
```

## Future Enhancements

- [ ] Model checking with TLA+
- [ ] Formal verification of core algorithms
- [ ] Performance regression testing
- [ ] Fuzzing for edge cases
- [ ] Distributed tracing integration
- [ ] Full Knossos linearizability checker
- [ ] Byzantine fault injection
- [ ] Network chaos (packet loss, delays, reordering)

## References

- [Jepsen: On the perils of network partitions](https://aphyr.com/posts/281-jepsen-on-the-perils-of-network-partitions)
- [Linearizability: A Correctness Condition for Concurrent Objects](https://cs.brown.edu/~mph/HerlihyW90/p463-herlihy.pdf)
- [Testing Distributed Systems](https://asatarin.github.io/testing-distributed-systems/)
- [Raft Consensus Algorithm](https://raft.github.io/)
