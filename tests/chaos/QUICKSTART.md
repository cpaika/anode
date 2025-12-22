# Chaos Testing Quick Start Guide

Get started with anode chaos testing in 5 minutes.

## Prerequisites

```bash
# Ensure Rust is installed
rustc --version

# Build anode
cd /path/to/anode
cargo build --release
```

## Quick Test Run

### 1. Run a Single Scenario

```bash
cd tests/chaos

# Run node crash tests
cargo test --test node_crash -- --nocapture

# Run network partition tests
cargo test --test network_partition -- --nocapture

# Run specific test
cargo test --test node_crash test_single_node_crash -- --nocapture
```

### 2. Run All Scenarios

```bash
# Run all chaos tests
cargo test -- --nocapture

# Run with specific log level
RUST_LOG=debug cargo test -- --nocapture
```

### 3. Run Example Programs

```bash
# Run all scenarios sequentially
cargo run --example run_all_scenarios

# Run custom scenario
cargo run --example custom_scenario
```

## Docker Quick Start

### 1. Build Images

```bash
# Build anode image
docker build -t anode:latest ../../

# Build chaos test image
docker build -f Dockerfile.chaos -t anode-chaos:latest .
```

### 2. Start Test Environment

```bash
# Start cluster with toxiproxy
docker-compose -f docker-compose.chaos.yml up -d

# Wait for services to be healthy
docker-compose -f docker-compose.chaos.yml ps

# Check logs
docker-compose -f docker-compose.chaos.yml logs -f anode-1
```

### 3. Run Tests

```bash
# Run chaos tests
docker-compose -f docker-compose.chaos.yml run chaos-runner

# Run specific test
docker-compose -f docker-compose.chaos.yml run chaos-runner \
  cargo test --test node_crash -- --nocapture
```

### 4. Cleanup

```bash
# Stop and remove everything
docker-compose -f docker-compose.chaos.yml down -v
```

## Common Scenarios

### Test Node Crash Recovery

```rust
use anode_chaos_tests::scenarios::node_crash::RandomNodeCrashScenario;
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::TestCluster;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        ..Default::default()
    };

    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let scenario = RandomNodeCrashScenario;
    let result = scenario.run(&mut cluster, &config).await.unwrap();

    println!("Success: {}", result.success);
    cluster.stop().await.unwrap();
}
```

### Test Network Partition

```rust
use anode_chaos_tests::scenarios::network_partition::MinorityPartitionScenario;

// Same setup as above...

let scenario = MinorityPartitionScenario;
let result = scenario.run(&mut cluster, &config).await.unwrap();
```

### Run Multiple Scenarios

```rust
use anode_chaos_tests::runner::ChaosRunner;

let mut runner = ChaosRunner::new(config);
runner = runner
    .add_scenario(Box::new(RandomNodeCrashScenario))
    .add_scenario(Box::new(MinorityPartitionScenario));

let report = runner.run_all(&mut cluster).await.unwrap();
report.print_summary();
```

## Network Chaos with Toxiproxy

### Add Latency

```bash
# Install toxiproxy CLI
brew install toxiproxy  # macOS
# or
go install github.com/Shopify/toxiproxy/v2/cmd/toxiproxy-cli@latest

# Add 500ms latency to node 1
toxiproxy-cli toxic add \
  -t latency \
  -a latency=500 \
  anode-1-proxy
```

### Add Packet Loss

```bash
# Add 10% packet loss
toxiproxy-cli toxic add \
  -t loss \
  -a loss=0.1 \
  anode-2-proxy
```

### Limit Bandwidth

```bash
# Limit to 1MB/s
toxiproxy-cli toxic add \
  -t bandwidth \
  -a rate=1024 \
  anode-3-proxy
```

### Remove Toxics

```bash
# Remove all toxics from a proxy
toxiproxy-cli toxic remove \
  -n latency_downstream \
  anode-1-proxy

# Or reset all
toxiproxy-cli reset
```

## Verification

### Check Data Integrity

```rust
use anode_chaos_tests::verification::DataVerifier;

let mut verifier = DataVerifier::new("http://localhost:9000".to_string());

// Write and track
verifier.write_and_track("bucket", "key", b"data".to_vec()).await?;

// Verify all
let all_ok = verifier.verify_all().await?;
println!("All data intact: {}", all_ok);

// Get detailed results
let results = verifier.get_verification_results().await?;
println!("Verified {}/{} objects",
    results.verified_objects,
    results.total_objects
);
```

## Troubleshooting

### Tests Fail to Start

```bash
# Check if anode binary exists
ls -la ../../target/release/anode

# Rebuild if needed
cargo build --release --manifest-path ../../Cargo.toml
```

### Permission Errors

```bash
# Some chaos features require privileges
sudo cargo test --test network_partition

# Or use Docker with privileged mode
docker-compose -f docker-compose.chaos.yml run --privileged chaos-runner
```

### Port Conflicts

```bash
# Find and kill processes using test ports
lsof -ti:9000-9010 | xargs kill -9

# Or change base port in tests
export ANODE_TEST_BASE_PORT=20000
```

## Next Steps

1. Read the full [README.md](README.md) for detailed documentation
2. Explore scenario implementations in `src/scenarios/`
3. Check out examples in `examples/`
4. Write your own custom scenarios using the `ChaosScenario` trait

## Resources

- Main documentation: [README.md](README.md)
- Test harness: [../harness/](../harness/)
- Correctness tests: [../correctness/](../correctness/)
- Project root: [../../](../../)

## Getting Help

- Check logs with `RUST_LOG=debug`
- Review test output with `--nocapture`
- Examine cluster state via admin API: `http://localhost:11001/cluster/status`
- Read scenario source code for implementation details
