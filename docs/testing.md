# Testing Guide

Comprehensive guide to testing Anode.

## Test Types

Anode includes several types of tests:

1. **Unit Tests** - Test individual components
2. **Integration Tests** - Test component interactions
3. **Correctness Tests** - Verify consistency guarantees
4. **Chaos Tests** - Test failure scenarios
5. **Benchmarks** - Performance measurement

## Running Tests

### Unit Tests

Run all unit tests:

```bash
cargo test --workspace
```

Run tests for specific crate:

```bash
cargo test --package anode-storage
cargo test --package anode-raft
cargo test --package anode-s3
```

Run specific test:

```bash
cargo test test_bucket_operations
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

### Integration Tests

Integration tests require a running cluster:

```bash
# Start test cluster
cd tests/harness
cargo run -- start-cluster --nodes 3

# Run integration tests
cargo test --package anode-correctness-tests

# Stop cluster
cargo run -- stop-cluster
```

### Correctness Tests

Test consistency and durability guarantees:

```bash
cd tests/correctness
cargo test --release
```

Key test suites:
- `linearizability_tests` - Verify linearizable reads/writes
- `durability_tests` - Verify data persistence after crashes
- `consistency_tests` - Verify replica consistency

### Chaos Tests

Test behavior under failure conditions:

```bash
cd tests/chaos
./run-chaos-tests.sh
```

Chaos scenarios:
- Node failures
- Network partitions
- Disk failures
- Clock skew
- Resource exhaustion

### Benchmarks

Run performance benchmarks:

```bash
# All benchmarks
cargo bench

# Specific benchmark
cargo bench --bench storage_bench

# With profiling
cargo bench -- --profile-time=5
```

## Test Harness

The test harness provides utilities for testing:

### Starting a Test Cluster

```rust
use anode_test_harness::TestCluster;

#[tokio::test]
async fn test_with_cluster() {
    let cluster = TestCluster::new(3).await.unwrap();

    // Use cluster
    cluster.create_bucket("test-bucket").await.unwrap();
    cluster.put_object("test-bucket", "key", b"data").await.unwrap();

    // Cleanup happens automatically
}
```

### Simulating Failures

```rust
#[tokio::test]
async fn test_node_failure() {
    let mut cluster = TestCluster::new(3).await.unwrap();

    // Kill a node
    cluster.kill_node(1).await.unwrap();

    // Cluster should still work
    cluster.put_object("bucket", "key", b"data").await.unwrap();

    // Restart node
    cluster.restart_node(1).await.unwrap();

    // Wait for sync
    cluster.wait_for_sync().await.unwrap();
}
```

### Network Partitions

```rust
#[tokio::test]
async fn test_partition() {
    let mut cluster = TestCluster::new(5).await.unwrap();

    // Create partition: [1,2] | [3,4,5]
    cluster.partition(&[1, 2], &[3, 4, 5]).await.unwrap();

    // Majority partition should work
    cluster.put_object_on_node(3, "bucket", "key", b"data").await.unwrap();

    // Minority partition should fail
    let result = cluster.put_object_on_node(1, "bucket", "key", b"data").await;
    assert!(result.is_err());

    // Heal partition
    cluster.heal_partition().await.unwrap();
}
```

## Writing Tests

### Unit Test Example

```rust
// crates/anode-storage/src/metadata.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (MetadataStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = MetadataStore::open(&db_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_put_and_get_object() {
        let (store, _temp) = setup();

        let key = ObjectKey::new("bucket", "key");
        let manifest = ObjectManifest::new(key.clone(), 1024, vec![]);

        store.put_object(&manifest).unwrap();

        let retrieved = store.get_object("bucket", "key").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().size, 1024);
    }
}
```

### Integration Test Example

```rust
// tests/correctness/tests/basic_ops.rs

use anode_test_harness::TestCluster;

#[tokio::test]
async fn test_basic_put_get() {
    let cluster = TestCluster::new(3).await.unwrap();

    cluster.create_bucket("test-bucket").await.unwrap();

    let data = b"Hello, Anode!";
    cluster.put_object("test-bucket", "test-key", data).await.unwrap();

    let retrieved = cluster.get_object("test-bucket", "test-key").await.unwrap();
    assert_eq!(retrieved, data);
}
```

### Property-Based Testing

Use proptest for property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_chunk_roundtrip(data in prop::collection::vec(any::<u8>(), 1..10000)) {
        let chunk_id = compute_chunk_id(&data);
        let stored = store_chunk(&chunk_id, &data)?;
        let retrieved = get_chunk(&chunk_id)?;
        prop_assert_eq!(data, retrieved);
    }
}
```

## Correctness Testing

### Linearizability Testing

Verify that operations appear to execute atomically:

```rust
#[tokio::test]
async fn test_linearizability() {
    let cluster = TestCluster::new(3).await.unwrap();
    let history = Arc::new(Mutex::new(Vec::new()));

    // Concurrent operations
    let mut handles = vec![];
    for i in 0..100 {
        let cluster = cluster.clone();
        let history = history.clone();

        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            cluster.put_object("bucket", "key", &format!("value-{}", i).into_bytes()).await.unwrap();
            let end = Instant::now();
            history.lock().await.push(Operation::Write {
                value: i,
                start,
                end
            });
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Check linearizability
    let history = history.lock().await;
    assert!(is_linearizable(&history));
}
```

### Durability Testing

Verify data survives crashes:

```rust
#[tokio::test]
async fn test_durability() {
    let mut cluster = TestCluster::new(3).await.unwrap();

    // Write data
    cluster.put_object("bucket", "key", b"data").await.unwrap();

    // Kill all nodes ungracefully
    cluster.kill_all_nodes().await.unwrap();

    // Restart cluster
    cluster.restart_all_nodes().await.unwrap();

    // Data should still be there
    let data = cluster.get_object("bucket", "key").await.unwrap();
    assert_eq!(data, b"data");
}
```

### Consistency Testing

Verify replicas stay consistent:

```rust
#[tokio::test]
async fn test_consistency() {
    let cluster = TestCluster::new(3).await.unwrap();

    // Write many objects
    for i in 0..1000 {
        cluster.put_object("bucket", &format!("key-{}", i), b"data").await.unwrap();
    }

    // Wait for replication
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Check all nodes have same data
    let checksums = cluster.get_all_node_checksums().await.unwrap();
    assert!(checksums.iter().all(|c| c == &checksums[0]));
}
```

## Chaos Testing

### Docker Compose Chaos Tests

Using Toxiproxy for network chaos:

```yaml
# docker-compose.chaos.yml
services:
  toxiproxy:
    image: ghcr.io/shopify/toxiproxy
    ports:
      - "8474:8474"

  anode-1:
    depends_on:
      - toxiproxy
    environment:
      - ANODE_GRPC_ADDR=toxiproxy:9090
```

Run chaos scenarios:

```bash
cd deploy/docker
docker-compose -f docker-compose.chaos.yml up -d

# Add latency
./scripts/chaos/add-latency.sh 100ms

# Add packet loss
./scripts/chaos/add-packet-loss.sh 10%

# Network partition
./scripts/chaos/partition.sh node1 node2,node3

# Heal
./scripts/chaos/heal.sh
```

### Chaos Test Scenarios

#### Scenario 1: Rolling Node Failures

```bash
#!/bin/bash
# tests/chaos/rolling-failures.sh

for node in node1 node2 node3; do
    echo "Killing $node..."
    docker-compose kill $node
    sleep 30

    echo "Restarting $node..."
    docker-compose up -d $node
    sleep 30
done

echo "All nodes recovered"
```

#### Scenario 2: Split Brain

```bash
#!/bin/bash
# tests/chaos/split-brain.sh

# Create partition
./scripts/chaos/partition.sh node1 node2,node3

# Try writes on both sides
aws --endpoint-url http://node1:8080 s3 cp test.txt s3://bucket/minority || true
aws --endpoint-url http://node2:8080 s3 cp test.txt s3://bucket/majority

# Heal partition
./scripts/chaos/heal.sh

# Verify consistency
./scripts/verify-consistency.sh
```

#### Scenario 3: Disk Full

```bash
#!/bin/bash
# tests/chaos/disk-full.sh

# Fill disk to 95%
docker exec anode-1 dd if=/dev/zero of=/data/fill bs=1M count=1000

# Try to write object (should fail gracefully)
aws --endpoint-url http://node1:8080 s3 cp large-file.bin s3://bucket/

# Clean up
docker exec anode-1 rm /data/fill

# Verify cluster recovered
./scripts/health-check.sh
```

## Benchmarking

### Storage Benchmarks

```rust
// benches/storage_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_put_object(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_object");

    for size in [1024, 10240, 102400, 1024000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let data = vec![0u8; size];
                b.iter(|| {
                    storage.put_object(&key, &data).unwrap();
                });
            }
        );
    }

    group.finish();
}

criterion_group!(benches, bench_put_object);
criterion_main!(benches);
```

### Cluster Benchmarks

```bash
# Run with different cluster sizes
for nodes in 3 5 7; do
    echo "Testing with $nodes nodes..."
    ./tests/harness/start-cluster --nodes $nodes
    cargo bench --bench cluster_bench
    ./tests/harness/stop-cluster
done
```

### Performance Testing

```bash
# Test throughput
./tests/performance/throughput-test.sh \
    --nodes 3 \
    --duration 60s \
    --object-size 1MB \
    --concurrency 10

# Test latency
./tests/performance/latency-test.sh \
    --nodes 3 \
    --requests 10000 \
    --object-size 1KB

# Test mixed workload
./tests/performance/mixed-workload.sh \
    --nodes 3 \
    --duration 300s \
    --read-ratio 0.7 \
    --write-ratio 0.3
```

## Continuous Integration

### GitHub Actions

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Run unit tests
        run: cargo test --workspace

      - name: Run integration tests
        run: |
          cd tests/correctness
          cargo test --release

      - name: Run chaos tests
        run: |
          cd tests/chaos
          ./run-chaos-tests.sh

      - name: Upload test results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: test-results
          path: test-results/
```

## Test Coverage

### Generate Coverage Report

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --out Html --output-dir coverage/

# View report
open coverage/index.html
```

### Coverage Targets

- Overall: > 70%
- Storage: > 80%
- Raft: > 75%
- S3 API: > 60%

## Best Practices

1. **Write Tests First** - TDD approach for new features
2. **Test Edge Cases** - Empty inputs, large inputs, invalid inputs
3. **Use Fixtures** - Reusable test data and setup
4. **Isolate Tests** - Each test should be independent
5. **Fast Unit Tests** - Keep unit tests under 1 second
6. **Realistic Integration Tests** - Use actual cluster, not mocks
7. **Regular Chaos Testing** - Run chaos tests nightly
8. **Benchmark Regressions** - Alert on performance degradation
9. **Document Tests** - Clear test names and comments
10. **Clean Up** - Always clean up test resources

## Debugging Tests

### Enable Debug Logging

```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Run Single Test

```bash
cargo test test_name --package anode-storage -- --nocapture
```

### Debug with lldb/gdb

```bash
rust-lldb target/debug/deps/anode_storage-<hash>
(lldb) break metadata.rs:100
(lldb) run test_name
```

### Print Debugging

```rust
#[test]
fn test_something() {
    eprintln!("Debug: value = {:?}", value);
    // Use eprintln! instead of println! for tests
}
```

## Test Data Management

### Generate Test Data

```bash
# Create test files
dd if=/dev/urandom of=test-1kb.bin bs=1024 count=1
dd if=/dev/urandom of=test-1mb.bin bs=1M count=1
dd if=/dev/urandom of=test-100mb.bin bs=1M count=100

# Create Parquet test files
python tests/data/generate-parquet.py --rows 1000000 --output test.parquet
```

### Cleanup Test Data

```bash
# Clean up after tests
cargo test
rm -rf /tmp/anode-test-*
docker-compose -f docker-compose.test.yml down -v
```

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Jepsen Testing](https://jepsen.io/)
- [Chaos Engineering Principles](https://principlesofchaos.org/)
- [Property-Based Testing](https://hypothesis.works/articles/what-is-property-based-testing/)
