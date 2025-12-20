# Anode Test Infrastructure

Comprehensive testing framework for the Anode distributed object storage system.

## Overview

This directory contains the test harness and test suites for Anode:

- **harness/**: Core test harness infrastructure
- **correctness/**: Correctness property tests
- **chaos/**: Chaos engineering tests

## Test Harness

The test harness (`tests/harness`) provides:

- **Multi-node cluster management**: Spawn and manage test clusters
- **Workload simulation**: Generate realistic workloads
- **Chaos injection**: Inject failures and network issues
- **Correctness assertions**: Verify distributed system properties
- **Metrics collection**: Track performance and correctness metrics

### Key Components

```rust
use anode_test_harness::*;

// Create and start a 3-node cluster
let mut cluster = TestCluster::new(3).await?;
cluster.start().await?;

// Generate workload
let workload = RandomWorkload::new(5, 1000, (1024, 4096), 0.7, 0.2, 0.1);

// Run simulation
let mut simulation = Simulation::new(cluster, Box::new(workload), config);
let result = simulation.run().await?;

// Inject chaos
let scheduler = ChaosScheduler::random(Duration::from_secs(60), 0.1);
scheduler.run(&mut cluster).await?;
```

## Correctness Tests

The correctness tests verify fundamental distributed systems properties:

### Linearizability

Tests that all operations appear to execute atomically in a total order consistent with real time.

```bash
cargo test --package anode-correctness-tests
```

### Durability

Tests that committed writes survive failures, crashes, and restarts.

```bash
cargo test --package anode-correctness-tests --test durability
```

### Consistency

Tests that replicas eventually converge and maintain consistency.

```bash
cargo test --package anode-correctness-tests --test consistency
```

### Replication

Tests that data is properly replicated across nodes.

```bash
cargo test --package anode-correctness-tests --test replication
```

## Chaos Tests

Chaos tests verify system resilience under various failure scenarios:

### Node Crash

Tests behavior when nodes crash unexpectedly.

```bash
cargo test --package anode-chaos-tests --test node_crash
```

### Network Partition

Tests behavior during network partitions (split-brain scenarios).

```bash
cargo test --package anode-chaos-tests --test network_partition
```

### Disk Failure

Tests behavior when disks fill up or fail.

```bash
cargo test --package anode-chaos-tests --test disk_failure
```

### Slow Network

Tests behavior under network degradation (latency, packet loss).

```bash
cargo test --package anode-chaos-tests --test slow_network
```

### Rolling Restart

Tests behavior during rolling restarts.

```bash
cargo test --package anode-chaos-tests --test rolling_restart
```

## Running Tests

### Prerequisites

1. Build the Anode binary:
   ```bash
   cargo build --release
   ```

2. Ensure sufficient system resources:
   - At least 8GB RAM
   - Plenty of disk space for temporary directories
   - Available port range (10000-60000)

### Run All Tests

```bash
# Run all correctness tests
cargo test --package anode-correctness-tests

# Run all chaos tests
cargo test --package anode-chaos-tests

# Run a specific test
cargo test --package anode-correctness-tests test_linearizability
```

### Test Configuration

Tests can be configured via environment variables:

```bash
# Enable verbose logging
RUST_LOG=debug cargo test --package anode-chaos-tests

# Set custom binary path
ANODE_BINARY=/path/to/anode cargo test

# Set custom timeout
TEST_TIMEOUT=600 cargo test
```

## Writing Tests

### Correctness Test Example

```rust
use anode_test_harness::*;

#[tokio::test]
async fn test_my_property() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    // Test logic here
    assert_object_exists(&cluster, "bucket", "key").await?;

    cluster.stop().await?;
    Ok(())
}
```

### Chaos Test Example

```rust
use anode_test_harness::chaos::*;

#[tokio::test]
async fn test_my_chaos_scenario() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    // Inject chaos
    let scheduler = ChaosScheduler::new()
        .add_event(Duration::from_secs(5), ChaosEvent::KillNode(0))
        .add_event(Duration::from_secs(10), ChaosEvent::RestartNode(0));

    scheduler.run(&mut cluster).await?;

    // Verify system recovered
    assert_cluster_healthy(&cluster).await?;

    cluster.stop().await?;
    Ok(())
}
```

## Continuous Integration

Tests are automatically run in CI:

- **Unit tests**: On every commit
- **Correctness tests**: On pull requests
- **Chaos tests**: Nightly builds
- **Long-running tests**: Weekly

## Test Results

Test results are collected and can be exported:

```rust
// Export metrics
let json = metrics.export_json().await?;
std::fs::write("test_results.json", json)?;

// Generate report
let reporter = MetricsReporter::new("My Test");
reporter.add_snapshot(metrics.snapshot().await);
let markdown = reporter.generate_markdown();
```

## Troubleshooting

### Tests Timeout

Increase the timeout:
```bash
TEST_TIMEOUT=1200 cargo test
```

### Port Conflicts

The harness automatically finds available ports, but if you encounter issues:
```bash
# Check for port conflicts
lsof -i :10000-60000
```

### Cleanup

Tests create temporary directories that are automatically cleaned up. If you need to manually clean:

```bash
# Clean up temp directories
rm -rf /tmp/anode-test-*

# Kill any orphaned processes
pkill anode
```

## Performance Considerations

- Chaos tests can take 10-30 minutes
- Correctness tests typically take 2-5 minutes
- Tests use significant system resources
- Recommended to run one test suite at a time

## Contributing

When adding new tests:

1. Add tests to the appropriate suite (correctness or chaos)
2. Use the harness utilities for cluster management
3. Clean up resources properly
4. Document expected behavior
5. Add appropriate timeouts
6. Consider test isolation

## License

Same as the main Anode project.
