# Anode Integration Test Suite

Comprehensive integration tests for the Anode distributed object storage system.

## Overview

This test suite validates end-to-end functionality of Anode, including:

- **S3 API Compatibility**: Basic and advanced S3 operations
- **Multipart Uploads**: Complete multipart upload workflows
- **Cluster Operations**: Node management and cluster coordination
- **Failover Scenarios**: Leader election, network partitions, and failure recovery
- **Performance**: Throughput and latency baselines
- **Parquet Optimizations**: Metadata caching and efficient queries

## Test Modules

### s3_basic.rs
Tests fundamental S3 operations:
- Bucket creation and deletion
- Object PUT, GET, DELETE
- Object listing and filtering
- Metadata operations
- Range requests
- Conditional operations
- Object copying

### s3_multipart.rs
Tests multipart upload functionality:
- Multipart upload lifecycle (create, upload, complete)
- Abort multipart uploads
- List parts
- Large object uploads (100MB+)
- Multipart copy operations
- Metadata preservation

### cluster_ops.rs
Tests cluster management:
- Cluster status and health checks
- Leader election verification
- Node addition and removal
- Quorum requirements
- Node restart and recovery
- Cluster information queries

### failover.rs
Tests failure scenarios:
- Leader failover and re-election
- Follower failures
- Minority node failures
- Network partitions
- Split-brain prevention
- Data durability during failures
- Rapid leader changes

### performance.rs
Performance regression tests:
- Throughput baselines (reads and writes)
- Latency measurements (p50, p95, p99)
- Concurrent operation performance
- Large object handling
- Mixed workload testing
- List and delete performance
- Metadata operation performance

### parquet.rs
Parquet-specific optimizations:
- Metadata caching
- Footer-only reads
- Range reads for column chunks
- Multi-file handling
- Concurrent column reads
- Large Parquet file performance
- Query optimization and partition pruning

## Running Tests

### Prerequisites

1. Build the anode binary:
   ```bash
   cargo build
   ```

2. Ensure you have enough disk space for temporary test data (several GB)

### Run All Tests

```bash
cd tests/integration
cargo test
```

### Run Specific Test Module

```bash
cargo test --test integration s3_basic
cargo test --test integration s3_multipart
cargo test --test integration cluster_ops
cargo test --test integration failover
cargo test --test integration performance
cargo test --test integration parquet
```

### Run Individual Test

```bash
cargo test --test integration test_create_and_delete_bucket
cargo test --test integration test_leader_failover
```

### Run with Logging

```bash
RUST_LOG=debug cargo test --test integration
```

## Test Configuration

Tests use the `serial_test` crate to prevent concurrent execution, as they:
- Start and stop cluster nodes
- Use shared resources (ports, disk)
- May interfere with each other

### Performance Baselines

The performance tests validate against these baselines:

- **Small object ops**: ≥100 ops/sec
- **Throughput**: ≥50 MB/s
- **P99 latency**: ≤500ms

Adjust baselines in `src/performance.rs` as needed.

## Test Architecture

### Cluster Setup

Tests use the `TestCluster` from the `anode-test-harness` crate, which:
- Automatically finds available ports
- Creates temporary directories for node data
- Manages node processes
- Handles cleanup on test completion

### Test Fixtures

The `BucketFixture` provides:
- Automatic bucket creation
- S3 client setup
- Cleanup after test completion

### Shared Cluster

Some tests use a shared cluster instance to reduce setup overhead. The shared cluster is initialized once and reused across tests.

## Writing New Tests

### Basic Test Template

```rust
use crate::{setup_cluster, create_test_s3_client, BucketFixture};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_my_feature() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-my-feature").await;

    // Your test code here

    // Cleanup is automatic via Drop implementations
}
```

### Using Shared Cluster

```rust
use crate::get_shared_cluster;

#[tokio::test]
#[serial]
async fn test_with_shared_cluster() {
    let cluster = get_shared_cluster().await;
    // Test code
}
```

## Troubleshooting

### Tests Failing with Port Conflicts

The test harness automatically finds available ports, but if you see port conflicts:
- Ensure no other anode processes are running
- Check for zombie processes: `ps aux | grep anode`

### Tests Timing Out

Leader election and cluster operations can take time:
- Default timeout is 30 seconds for leader election
- Increase timeouts if running on slow hardware
- Check logs with `RUST_LOG=debug`

### Cleanup Issues

If tests leave behind resources:
- Check `/tmp` for test directories
- Kill any orphaned anode processes
- The test harness should clean up automatically via Drop

### Performance Test Failures

Performance tests may fail on slow/busy systems:
- Run on dedicated hardware when possible
- Adjust baseline constants in `performance.rs`
- Consider the system load when interpreting results

## CI Integration

These tests are suitable for CI/CD pipelines:
- All tests are hermetic (self-contained)
- No external dependencies required
- Automatic cleanup prevents resource leaks
- Serial execution prevents flaky tests

### Recommended CI Configuration

```yaml
test:
  script:
    - cargo build --release
    - cd tests/integration
    - cargo test --release -- --test-threads=1
  timeout: 30m
```

## Test Coverage

The integration test suite covers:

- ✅ S3 API compatibility (basic operations)
- ✅ S3 API compatibility (advanced features)
- ✅ Multipart uploads
- ✅ Cluster formation and management
- ✅ Leader election
- ✅ Failure scenarios
- ✅ Data durability
- ✅ Performance baselines
- ✅ Parquet optimizations

## Future Enhancements

Planned additions:
- Cross-region replication tests
- Security and authentication tests
- Backup and restore tests
- Upgrade/migration tests
- Stress testing with sustained load
- More comprehensive Parquet query tests

## Contributing

When adding new tests:
1. Use descriptive test names
2. Include cleanup logic
3. Add proper assertions with helpful messages
4. Document any special requirements
5. Use `#[serial]` attribute for cluster tests
6. Update this README with new test coverage
