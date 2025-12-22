# Anode Chaos Testing

Comprehensive chaos testing framework for validating anode's fault tolerance and resilience under various failure conditions.

## Overview

This testing suite implements various chaos scenarios to verify that anode maintains data integrity and availability during:

- Node crashes and restarts
- Network partitions
- Disk failures
- Network degradation (latency, packet loss, bandwidth limits)
- Cascading failures
- Split-brain scenarios

## Architecture

```
tests/chaos/
├── src/
│   ├── lib.rs                    # Main library entry point
│   ├── scenarios/                # Chaos scenarios
│   │   ├── mod.rs
│   │   ├── node_crash.rs         # Node crash scenarios
│   │   ├── network_partition.rs  # Network partition scenarios
│   │   ├── disk_failure.rs       # Disk failure scenarios
│   │   ├── slow_network.rs       # Network degradation scenarios
│   │   ├── rolling_restart.rs    # Rolling update scenarios
│   │   ├── split_brain.rs        # Split-brain scenarios
│   │   └── cascade_failure.rs    # Cascading failure scenarios
│   ├── network.rs                # Network chaos utilities
│   ├── process.rs                # Process chaos utilities
│   ├── disk.rs                   # Disk chaos utilities
│   ├── runner.rs                 # Test runner and reporting
│   └── verification.rs           # Data integrity verification
├── tests/                        # Integration tests
│   ├── node_crash.rs
│   ├── network_partition.rs
│   ├── disk_failure.rs
│   ├── slow_network.rs
│   ├── rolling_restart.rs
│   ├── split_brain.rs
│   └── cascade_failure.rs
├── docker-compose.chaos.yml      # Docker compose for chaos testing
├── Dockerfile.chaos              # Dockerfile for chaos tests
├── Cargo.toml
└── README.md                     # This file
```

## Scenarios

### Node Crash Scenarios

1. **Random Node Crashes** - Randomly crashes nodes during operations and verifies cluster continues functioning
2. **Leader Crash During Write** - Crashes the leader during a write operation and verifies atomicity
3. **Multiple Node Crashes** - Crashes multiple nodes while maintaining quorum
4. **Full Cluster Crash** - Crashes all nodes simultaneously and verifies recovery

### Network Partition Scenarios

1. **Minority Partition** - Partitions minority nodes from the cluster
2. **Leader Partition** - Isolates the leader node and verifies new election
3. **Symmetric Partition** - Creates a symmetric partition splitting the cluster
4. **Asymmetric Partition** - Creates one-way network partitions

### Disk Failure Scenarios

1. **Read-Only Disk** - Simulates disk becoming read-only
2. **Disk Full** - Fills disk to capacity
3. **Disk I/O Errors** - Injects random I/O errors
4. **Data Directory Deleted** - Deletes a node's data and verifies rebuild

### Network Degradation Scenarios

1. **High Latency** - Adds 500ms network latency
2. **Packet Loss** - Simulates 10% packet loss
3. **Bandwidth Limit** - Limits bandwidth to 1MB/s
4. **Network Jitter** - Adds variable 0-200ms latency

### Rolling Restart Scenarios

1. **Basic Rolling Restart** - Restarts all nodes one by one
2. **Continuous Workload Rolling Restart** - Performs rolling restart under load

### Split-Brain Scenarios

1. **Network Partition Split-Brain** - Creates partition and verifies no split-brain
2. **Flapping Partition** - Creates repeatedly flapping network partitions

### Cascade Failure Scenarios

1. **Sequential Node Failure** - Nodes fail one after another
2. **Rapid Cascade** - Rapid sequential failures with minimal delay
3. **Recovery Under Load** - Node failures and recovery while under continuous load

## Running Tests

### Prerequisites

```bash
# Build anode
cargo build --release

# Ensure the binary is accessible
export PATH="$PWD/target/release:$PATH"
```

### Running Individual Tests

```bash
# Run all chaos tests
cargo test --package anode-chaos-tests

# Run specific scenario
cargo test --package anode-chaos-tests --test node_crash

# Run with output
cargo test --package anode-chaos-tests -- --nocapture
```

### Running with Docker Compose

```bash
# Start the cluster with toxiproxy
docker-compose -f docker-compose.chaos.yml up -d

# Run chaos tests
docker-compose -f docker-compose.chaos.yml run chaos-runner

# View logs
docker-compose -f docker-compose.chaos.yml logs -f

# Stop and cleanup
docker-compose -f docker-compose.chaos.yml down -v
```

### Using Toxiproxy for Network Chaos

```bash
# Install toxiproxy CLI
go install github.com/Shopify/toxiproxy/v2/cmd/toxiproxy-cli@latest

# Add latency
toxiproxy-cli toxic add -t latency -a latency=1000 anode-1

# Add packet loss
toxiproxy-cli toxic add -t loss -a loss=0.1 anode-2

# Add bandwidth limit
toxiproxy-cli toxic add -t bandwidth -a rate=100 anode-3

# Remove toxic
toxiproxy-cli toxic remove -n latency_downstream anode-1
```

## Configuration

Scenarios can be configured via `ScenarioConfig`:

```rust
let config = ScenarioConfig {
    num_nodes: 5,                               // Cluster size
    duration: Duration::from_secs(60),          // Test duration
    verify_integrity: true,                     // Verify data integrity
    workload_intensity: 10.0,                   // Operations per second
    params: serde_json::json!({                 // Scenario-specific params
        "custom_param": "value"
    }),
};
```

## Data Verification

All scenarios use `DataVerifier` to:

1. Track all written objects
2. Verify reads match expected data
3. Detect missing or corrupted objects
4. Generate comprehensive verification reports

Example:

```rust
let mut verifier = DataVerifier::new(endpoint);

// Write and track
verifier.write_and_track("bucket", "key", data).await?;

// Verify specific object
let matches = verifier.verify_read("bucket", "key", &expected_data).await?;

// Verify all tracked objects
let all_ok = verifier.verify_all().await?;

// Get detailed results
let results = verifier.get_verification_results().await?;
println!("Verified: {}/{}", results.verified_objects, results.total_objects);
```

## Writing Custom Scenarios

Implement the `ChaosScenario` trait:

```rust
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig, ScenarioResult};
use async_trait::async_trait;

pub struct CustomScenario;

#[async_trait]
impl ChaosScenario for CustomScenario {
    fn name(&self) -> &str {
        "custom_scenario"
    }

    fn description(&self) -> &str {
        "My custom chaos scenario"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        // Implement your chaos scenario
        // ...

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: true,
            duration: start_time.elapsed(),
            operations: 100,
            failures: 0,
            integrity_verified: true,
            metrics: serde_json::json!({}),
            error: None,
        })
    }
}
```

## Test Reports

The chaos runner generates comprehensive reports:

```
==========================================
Chaos Testing Report
==========================================
Total scenarios: 15
Passed: 14
Failed: 1
Total duration: 15m 30s
==========================================
[PASS] random_node_crashes - 2m 15s (150 ops, 0 failures)
[PASS] leader_crash_during_write - 1m 30s (45 ops, 0 failures)
[FAIL] split_brain - 3m 45s (200 ops, 5 failures)
      Error: Data inconsistency detected
...
```

## Limitations

### Network Chaos

Some network chaos features require privileged access:

- **tc (traffic control)** - Required for latency, packet loss, bandwidth limiting
- **iptables** - Required for network partitions
- **dm-flakey** - Required for disk I/O error injection

These features may not work in:
- Containers without `--privileged` flag
- Systems without root access
- Non-Linux platforms

### Platform Support

Full chaos testing is best supported on:
- Linux with root/sudo access
- Docker with privileged mode
- Kubernetes with appropriate permissions

Some features have limited support on:
- macOS (no tc, dm-flakey)
- Windows (different network tools)

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Chaos Tests

on:
  push:
    branches: [main]
  pull_request:

jobs:
  chaos:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Build
        run: cargo build --release

      - name: Run chaos tests
        run: |
          cargo test --package anode-chaos-tests -- \
            --test-threads=1 \
            --nocapture
        env:
          RUST_LOG: info
```

## Best Practices

1. **Run tests in isolation** - Use `--test-threads=1` to avoid interference
2. **Clean up resources** - Always ensure cluster cleanup in test teardown
3. **Use timeouts** - Set appropriate timeouts to prevent hanging tests
4. **Verify data integrity** - Always enable `verify_integrity` in production tests
5. **Monitor resource usage** - Chaos tests can be resource-intensive
6. **Test incrementally** - Start with simple scenarios before complex ones

## Troubleshooting

### Tests hang or timeout

- Increase timeout values in `ScenarioConfig`
- Check if nodes are starting properly
- Verify network connectivity between nodes

### Permission denied errors

- Run tests with appropriate privileges
- Use Docker with `--privileged` flag
- Check file/directory permissions

### Data verification failures

- Check logs for write/read errors
- Verify cluster has quorum
- Ensure proper cleanup between tests

## Contributing

When adding new chaos scenarios:

1. Create scenario implementation in `src/scenarios/`
2. Add integration test in `tests/`
3. Update `Cargo.toml` with new test target
4. Document the scenario in this README
5. Ensure data integrity verification is included

## References

- [Jepsen](https://jepsen.io/) - Distributed systems testing framework
- [Chaos Engineering](https://principlesofchaos.org/) - Principles of chaos engineering
- [Toxiproxy](https://github.com/Shopify/toxiproxy) - Network chaos proxy
- [Chaos Mesh](https://chaos-mesh.org/) - Cloud-native chaos engineering platform
