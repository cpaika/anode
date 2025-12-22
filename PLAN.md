# Anode Implementation Plan

## Overview

Anode is a production-grade, Rust-based distributed object storage system for small clusters. This plan outlines 100 tasks organized into phases to achieve a complete, battle-tested implementation.

### Core Priorities
1. **TDD & Correctness** - Custom test harness, formal verification, property-based testing
2. **Chaos Testing** - Network partitions, node crashes, volume loss, corruption simulation
3. **Performance** - Benchmarked, optimized for disk and query throughput
4. **Pure Rust** - No external dependencies, embedded Raft via openraft
5. **Deployability** - Standalone, Kubernetes/Helm, K3d tested
6. **GHA CI/CD** - Comprehensive validation on every PR
7. **Failure Handling** - Data redundancy, corruption detection, automatic rebuild
8. **Parquet Awareness** - Metadata caching, predicate pushdown

---

## Phase 1: Foundation & Core Infrastructure (Tasks 1-15)

### Workspace & Build System
- [ ] **1. Fix remaining openraft 0.9 API compatibility issues**
  - Update `RaftNetworkFactory` trait implementation to match new signatures
  - Fix lifetime parameters on `new_client`, `append_entries`, etc.
  - Verify all storage trait implementations match openraft 0.9 API

- [ ] **2. Fix anode-s3 compilation errors**
  - Resolve handler signature mismatches
  - Fix multipart upload completion logic
  - Ensure all S3 operations compile cleanly

- [ ] **3. Clean up all clippy warnings**
  - Remove unused imports across all crates
  - Fix dead code warnings
  - Address all clippy lints in `clippy.toml`

- [ ] **4. Set up workspace-level feature flags**
  - `parquet-cache` - Enable parquet metadata caching
  - `erasure-coding` - Enable EC support (future)
  - `metrics` - Enable Prometheus metrics
  - `tracing` - Enable distributed tracing

- [ ] **5. Configure Cargo profiles for different environments**
  - `dev` - Fast compilation, debug assertions
  - `release` - Full optimizations, LTO
  - `bench` - Release with debug symbols for profiling
  - `production` - Strip symbols, maximum optimization

### Core Storage Engine
- [ ] **6. Implement atomic write-ahead log for storage engine**
  - Ensure crash consistency for metadata operations
  - Add fsync options configurable per operation
  - Implement batch commit for multiple operations

- [ ] **7. Add content-addressable storage verification**
  - Verify chunk hash on every read
  - Background verification thread
  - Corruption detection and reporting

- [ ] **8. Implement storage quotas per bucket**
  - Track bytes used per bucket
  - Enforce soft and hard limits
  - Quota exceeded error handling

- [ ] **9. Add object versioning support**
  - Version ID generation
  - List object versions API
  - Delete marker support

- [ ] **10. Implement multipart upload state persistence**
  - Persist in-progress uploads to survive restarts
  - Cleanup stale uploads after timeout
  - Resume interrupted uploads

### Raft Consensus
- [ ] **11. Complete openraft integration**
  - Fix all trait implementations to match openraft 0.9
  - Implement proper snapshot support
  - Add leader lease for read optimization

- [ ] **12. Implement Raft configuration changes**
  - Add node to cluster
  - Remove node from cluster
  - Joint consensus for safe membership changes

- [ ] **13. Add Raft metrics and observability**
  - Leader election count
  - Log replication latency
  - Snapshot size and frequency

- [ ] **14. Implement placement group management**
  - PG creation and assignment
  - Rebalancing when nodes join/leave
  - PG leadership tracking

- [ ] **15. Add Raft log compaction**
  - Configurable compaction threshold
  - Snapshot-based log truncation
  - Memory-bounded log buffer

---

## Phase 2: S3 API Completeness (Tasks 16-30)

### Core S3 Operations
- [ ] **16. Complete PUT object implementation**
  - Content-MD5 validation
  - Content-Type handling
  - Custom metadata headers (x-amz-meta-*)

- [ ] **17. Complete GET object implementation**
  - Range requests (bytes=0-100)
  - Conditional gets (If-Match, If-None-Match)
  - Response content disposition

- [ ] **18. Implement DELETE object properly**
  - Delete markers for versioned buckets
  - Quiet mode for batch deletes
  - Proper error responses

- [ ] **19. Complete HEAD object/bucket**
  - All metadata headers
  - Proper status codes
  - ETag handling

- [ ] **20. Implement LIST objects v2**
  - Continuation tokens
  - Prefix and delimiter support
  - Common prefixes for directory-like listing

### Multipart Upload
- [ ] **21. Fix multipart upload initiation**
  - Generate upload ID
  - Store upload metadata
  - Handle concurrent initiations

- [ ] **22. Implement part upload**
  - Part number validation (1-10000)
  - ETag generation per part
  - Part size validation (5MB minimum except last)

- [ ] **23. Implement complete multipart upload**
  - Part ordering and validation
  - Final object assembly
  - Atomic commit

- [ ] **24. Implement abort multipart upload**
  - Clean up uploaded parts
  - Release storage space
  - Handle concurrent abort

- [ ] **25. Implement list parts**
  - Pagination support
  - Part metadata (size, ETag, last modified)

### Bucket Operations
- [ ] **26. Implement bucket lifecycle policies**
  - Expiration rules
  - Transition rules (cold storage)
  - Filter by prefix and tags

- [ ] **27. Add bucket CORS configuration**
  - Store CORS rules per bucket
  - Apply CORS headers to responses
  - Preflight request handling

- [ ] **28. Implement bucket tagging**
  - GET/PUT/DELETE bucket tagging
  - Tag-based access control (future)

- [ ] **29. Add bucket policy support**
  - IAM-style policy documents
  - Policy evaluation engine
  - Principal matching

- [ ] **30. Implement presigned URLs**
  - Signature generation
  - Expiration handling
  - Query string authentication

---

## Phase 3: Cluster Operations (Tasks 31-45)

### Node Management
- [ ] **31. Implement node discovery**
  - DNS-based discovery
  - Static seed list
  - Kubernetes headless service discovery

- [ ] **32. Add node health checking**
  - Heartbeat mechanism
  - Failure detection timeout
  - Health status API

- [ ] **33. Implement graceful shutdown**
  - Drain connections
  - Transfer leadership
  - Wait for replication

- [ ] **34. Add node decommissioning**
  - Migrate data off node
  - Update cluster membership
  - Verify data redundancy maintained

- [ ] **35. Implement rolling restart support**
  - One-at-a-time restart coordination
  - Quorum maintenance
  - Automatic leadership rebalancing

### Data Distribution
- [ ] **36. Implement consistent hashing for object placement**
  - Hash ring management
  - Virtual nodes for balance
  - Minimal disruption on topology change

- [ ] **37. Add replication factor configuration**
  - Per-bucket replication factor
  - Minimum 1, maximum cluster size
  - Runtime reconfiguration

- [ ] **38. Implement data rebalancing**
  - Background data movement
  - Throttling to limit impact
  - Progress tracking and reporting

- [ ] **39. Add cross-node chunk replication**
  - Streaming replication protocol
  - Checksum verification on transfer
  - Retry logic for transient failures

- [ ] **40. Implement read repair**
  - Detect inconsistencies on read
  - Automatic repair from healthy replicas
  - Log repair events

### Cluster State
- [ ] **41. Implement cluster configuration storage**
  - Raft-replicated config
  - Version tracking
  - Safe concurrent updates

- [ ] **42. Add cluster status API**
  - Node list with status
  - PG distribution
  - Replication health

- [ ] **43. Implement leader election monitoring**
  - Track election events
  - Alert on frequent elections
  - Metrics for election latency

- [ ] **44. Add split-brain prevention**
  - Quorum enforcement
  - Fencing for old leaders
  - Network partition detection

- [ ] **45. Implement cluster version compatibility**
  - Protocol versioning
  - Rolling upgrade support
  - Feature flags for new functionality

---

## Phase 4: Testing Infrastructure (Tasks 46-60)

### Test Harness
- [ ] **46. Complete custom test harness implementation**
  - Multi-process cluster spawning
  - Shared state for verification
  - Deterministic test execution

- [ ] **47. Implement linearizability checker**
  - Operation history recording
  - Jepsen-style verification
  - Counterexample generation

- [ ] **48. Add property-based testing with proptest**
  - Arbitrary object key/value generation
  - Shrinking for minimal counterexamples
  - Stateful testing for cluster operations

- [ ] **49. Implement simulation testing mode**
  - Deterministic scheduling
  - Fault injection points
  - Time simulation for timeouts

- [ ] **50. Add performance regression testing**
  - Baseline measurement storage
  - Automatic comparison on PR
  - Alert on regressions > 5%

### Chaos Testing
- [ ] **51. Implement network partition simulation**
  - iptables-based partition (Linux)
  - Full partition (A cannot reach B)
  - Asymmetric partition (A->B works, B->A doesn't)

- [ ] **52. Add node crash simulation**
  - SIGKILL for hard crash
  - SIGTERM for graceful shutdown
  - Crash during specific operations

- [ ] **53. Implement disk failure simulation**
  - Read errors
  - Write errors
  - Full disk simulation

- [ ] **54. Add slow network simulation**
  - Latency injection (tc netem)
  - Packet loss
  - Bandwidth limiting

- [ ] **55. Implement clock skew testing**
  - Fake clock for deterministic testing
  - Large time jumps
  - Backward time movement

### Integration Tests
- [ ] **56. Add S3 compatibility test suite**
  - AWS SDK compatibility
  - MinIO client compatibility
  - s3cmd compatibility

- [ ] **57. Implement durability tests**
  - Write data, crash all nodes, restart, verify
  - Partial cluster survival
  - Data integrity after recovery

- [ ] **58. Add concurrent operation tests**
  - Many clients writing same key
  - Interleaved reads and writes
  - Multipart upload concurrency

- [ ] **59. Implement long-running soak tests**
  - 24-hour stability test
  - Memory leak detection
  - Resource exhaustion testing

- [ ] **60. Add upgrade testing**
  - Rolling upgrade simulation
  - Version compatibility verification
  - Downgrade testing

---

## Phase 5: Performance & Optimization (Tasks 61-75)

### Benchmarking
- [ ] **61. Implement comprehensive benchmark suite**
  - PUT throughput (1KB, 1MB, 100MB objects)
  - GET throughput and latency
  - LIST performance at scale

- [ ] **62. Add CPU profiling integration**
  - perf integration for Linux
  - Flamegraph generation
  - CPU cycles per operation tracking

- [ ] **63. Implement memory profiling**
  - Allocation tracking with jemalloc
  - Peak memory usage
  - Memory per connection/request

- [ ] **64. Add I/O profiling**
  - Disk read/write bytes
  - Write amplification measurement
  - IOPS per operation type

- [ ] **65. Implement network profiling**
  - Bytes transferred per operation
  - Raft message overhead
  - Inter-node bandwidth usage

### Optimizations
- [ ] **66. Optimize chunk storage layout**
  - Directory sharding by hash prefix
  - Batch file operations
  - Minimize syscalls

- [ ] **67. Implement connection pooling**
  - Pool for inter-node gRPC connections
  - Pool for client connections
  - Idle connection timeout

- [ ] **68. Add request batching**
  - Batch small PUTs
  - Batch metadata updates
  - Configurable batch size/timeout

- [ ] **69. Optimize Raft log storage**
  - Batch log entries
  - Async fsync with callback
  - Compression for log entries

- [ ] **70. Implement zero-copy reads**
  - Memory-mapped file reads
  - sendfile for large transfers
  - Avoid unnecessary allocations

### Caching
- [ ] **71. Add metadata cache**
  - LRU cache for object metadata
  - Configurable size
  - Cache invalidation on update

- [ ] **72. Implement chunk cache**
  - Hot chunk caching
  - Cache hit ratio metrics
  - Adaptive cache sizing

- [ ] **73. Add query result cache**
  - LIST result caching
  - Prefix-based cache keys
  - TTL-based invalidation

- [ ] **74. Optimize parquet metadata cache**
  - Footer parsing and caching
  - Row group location cache
  - Column statistics cache

- [ ] **75. Implement read-ahead for sequential access**
  - Detect sequential read patterns
  - Prefetch next chunks
  - Configurable prefetch depth

---

## Phase 6: Observability & Operations (Tasks 76-85)

### Metrics
- [ ] **76. Implement Prometheus metrics endpoint**
  - Request count and latency histograms
  - Error rates by type
  - Cluster health metrics

- [ ] **77. Add storage metrics**
  - Bytes used per bucket
  - Object count
  - Chunk deduplication ratio

- [ ] **78. Implement Raft metrics**
  - Replication lag
  - Leader changes
  - Log size and compaction

- [ ] **79. Add performance metrics**
  - P50/P99/P999 latencies
  - Throughput (ops/sec, bytes/sec)
  - Queue depths

- [ ] **80. Implement alerting rules**
  - PrometheusRule resources
  - Critical alerts (quorum loss, disk full)
  - Warning alerts (high latency, replication lag)

### Logging & Tracing
- [ ] **81. Implement structured logging**
  - JSON format for production
  - Request ID propagation
  - Configurable log levels per module

- [ ] **82. Add distributed tracing**
  - OpenTelemetry integration
  - Trace context propagation
  - Span for each operation

- [ ] **83. Implement audit logging**
  - All data access logged
  - Admin operations logged
  - Configurable retention

### Admin API
- [ ] **84. Implement admin HTTP API**
  - Cluster status
  - Node management
  - Configuration updates

- [ ] **85. Add CLI tool for operations**
  - `anodectl` binary
  - Cluster management commands
  - Debugging utilities

---

## Phase 7: Deployment & Infrastructure (Tasks 86-95)

### Docker
- [ ] **86. Optimize Dockerfile**
  - Multi-stage build
  - Minimal runtime image (distroless)
  - Non-root user

- [ ] **87. Create docker-compose for development**
  - 3-node cluster
  - Prometheus + Grafana
  - Volume persistence

- [ ] **88. Add chaos testing docker-compose**
  - Toxiproxy for network simulation
  - Pumba for container chaos
  - Test orchestration

### Kubernetes/Helm
- [ ] **89. Complete Helm chart**
  - StatefulSet with proper ordering
  - Headless service for discovery
  - ConfigMap/Secret management

- [ ] **90. Add Helm chart tests**
  - helm test hooks
  - Connectivity tests
  - Data persistence tests

- [ ] **91. Implement PodDisruptionBudget**
  - Maintain quorum during updates
  - Rolling update strategy
  - MaxUnavailable configuration

- [ ] **92. Add HorizontalPodAutoscaler support**
  - CPU/memory based scaling
  - Custom metrics scaling
  - Scale-up/down cooldowns

- [ ] **93. Implement K3d integration tests**
  - Automated cluster creation
  - Helm install and test
  - Cleanup after tests

### CI/CD
- [ ] **94. Complete GitHub Actions workflows**
  - Build and test on every PR
  - Clippy and rustfmt checks
  - Security scanning (cargo-audit)

- [ ] **95. Add release automation**
  - Semantic versioning
  - Changelog generation
  - Container image publishing

---

## Phase 8: Documentation & Polish (Tasks 96-100)

### Documentation
- [ ] **96. Complete API documentation**
  - S3 API reference
  - Admin API reference
  - gRPC protocol documentation

- [ ] **97. Write operations guide**
  - Deployment procedures
  - Backup and restore
  - Troubleshooting guide

- [ ] **98. Create architecture documentation**
  - System design overview
  - Data flow diagrams
  - Failure mode analysis

- [ ] **99. Add performance tuning guide**
  - Hardware recommendations
  - Configuration tuning
  - Benchmark interpretation

- [ ] **100. Create security hardening guide**
  - TLS configuration
  - Authentication setup
  - Network security best practices

---

## Implementation Order Recommendation

### Week 1-2: Get to Green
1. Tasks 1-3: Fix all compilation errors, pass clippy
2. Task 11: Complete openraft integration
3. Tasks 16-20: Core S3 operations working

### Week 3-4: Testing Foundation
4. Tasks 46-48: Test harness and property testing
5. Tasks 51-54: Basic chaos testing
6. Tasks 56-58: S3 compatibility and integration tests

### Week 5-6: Cluster Robustness
7. Tasks 31-35: Node management
8. Tasks 36-40: Data distribution
9. Tasks 41-45: Cluster state management

### Week 7-8: Performance
10. Tasks 61-65: Benchmarking infrastructure
11. Tasks 66-70: Core optimizations
12. Tasks 71-75: Caching layer

### Week 9-10: Production Readiness
13. Tasks 76-85: Observability
14. Tasks 86-95: Deployment infrastructure
15. Tasks 96-100: Documentation

---

---

## Appendix A: Formal Verification Strategy

### Rust + Rocq/Coq Integration

Formal verification is critical for a storage system. We'll use a layered approach:

#### Layer 1: Property-Based Testing (Immediate)
```rust
// Using proptest for automated property testing
proptest! {
    #[test]
    fn chunk_roundtrip_is_identity(data: Vec<u8>) {
        let chunks = ChunkManager::split_into_chunks(&data);
        let chunk_ids: Vec<_> = chunks.iter().map(|c| c.id.clone()).collect();
        let reassembled = manager.retrieve_chunks(&chunk_ids).await?;
        prop_assert_eq!(data, reassembled);
    }

    #[test]
    fn sha256_is_collision_resistant(a: Vec<u8>, b: Vec<u8>) {
        prop_assume!(a != b);
        let hash_a = compute_chunk_id(&a);
        let hash_b = compute_chunk_id(&b);
        prop_assert_ne!(hash_a, hash_b);
    }
}
```

#### Layer 2: Model Checking with Stateright
```rust
// Formal model of Raft consensus
use stateright::*;

struct RaftModel {
    nodes: Vec<NodeState>,
    network: Network,
}

impl Model for RaftModel {
    type State = ClusterState;
    type Action = RaftAction;

    fn init_states(&self) -> Vec<Self::State> {
        // All possible initial states
    }

    fn actions(&self, state: &Self::State) -> Vec<Self::Action> {
        // All possible actions from state
    }

    fn next_state(&self, state: &Self::State, action: &Self::Action) -> Self::State {
        // State transition function
    }
}

// Properties to verify
fn safety_properties(state: &ClusterState) -> bool {
    // At most one leader per term
    let leaders: Vec<_> = state.nodes.iter()
        .filter(|n| n.role == Role::Leader)
        .collect();
    leaders.len() <= 1
}
```

#### Layer 3: Coq/Rocq Proofs for Critical Algorithms
```coq
(* Proof that chunk replication maintains data integrity *)
Theorem chunk_replication_preserves_data:
  forall (chunk: Chunk) (replicas: list Node),
    length replicas >= replication_factor ->
    exists n, In n replicas /\ read_chunk n chunk.id = Some chunk.data.

(* Proof that Raft maintains linearizability *)
Theorem raft_linearizable:
  forall (ops: list Operation) (history: History),
    valid_raft_execution ops history ->
    linearizable history.
```

#### Verification Targets
- [ ] **V1: Chunk integrity** - SHA-256 verification is correct
- [ ] **V2: Replication safety** - Data survives f failures with 2f+1 replicas
- [ ] **V3: Linearizability** - All operations appear atomic
- [ ] **V4: Durability** - Committed data survives crashes
- [ ] **V5: Consistency** - No split-brain scenarios

---

## Appendix B: Comprehensive Benchmark Suite

### Benchmark Categories

#### B1: Microbenchmarks (criterion)
```rust
// benches/storage.rs
fn bench_put_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_small");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter(|| {
                    engine.put_object("bench", "key", &data[..size], HashMap::new())
                });
            },
        );
    }
    group.finish();
}
```

#### B2: Workload Benchmarks
| Workload | Description | Metrics |
|----------|-------------|---------|
| YCSB-A | 50% read, 50% update | ops/sec, p99 latency |
| YCSB-B | 95% read, 5% update | ops/sec, p99 latency |
| YCSB-C | 100% read | ops/sec, p99 latency |
| YCSB-D | 95% read latest, 5% insert | ops/sec, p99 latency |
| Write-Heavy | 100% write, varying sizes | throughput MB/s |
| Read-Heavy | 100% read, random access | IOPS, latency |
| Mixed-Large | 50/50 read/write, 100MB objects | throughput MB/s |
| Parquet-Scan | Parquet metadata queries | queries/sec |

#### B3: Chaos Benchmarks
| Scenario | Description | Success Criteria |
|----------|-------------|------------------|
| Leader-Failover | Kill leader during load | < 5s recovery, no data loss |
| Network-Partition | Split cluster in half | Correct quorum behavior |
| Slow-Follower | 500ms latency to one node | Throughput within 80% |
| Rolling-Restart | Restart each node | Zero downtime |

### Auto-Generated Benchmark Report

The benchmark suite generates `BENCHMARKS.md` on each run:

```markdown
# Anode Benchmark Report

Generated: 2024-01-15T14:30:00Z
Commit: abc123
Hardware: 8-core AMD EPYC, 32GB RAM, NVMe SSD

## Summary

| Metric | Value | vs Previous | Status |
|--------|-------|-------------|--------|
| PUT 1KB ops/sec | 45,230 | +2.3% | :white_check_mark: |
| PUT 1MB MB/sec | 2,340 | -0.5% | :white_check_mark: |
| GET 1KB ops/sec | 89,120 | +1.1% | :white_check_mark: |
| GET 1MB MB/sec | 3,890 | +0.2% | :white_check_mark: |
| p99 latency (ms) | 4.2 | -5.0% | :white_check_mark: |

## Detailed Results

### PUT Performance by Object Size
...
```

### Comparison with Other Object Stores

#### Comparison Targets
1. **MinIO** - Most popular S3-compatible object store
2. **SeaweedFS** - Fast, distributed storage
3. **Garage** - Rust-based, geo-distributed
4. **OpenIO** - High-performance object store

#### Benchmark Methodology
```yaml
# benchmark-comparison.yaml
scenarios:
  - name: small_objects
    object_size: 4KB
    object_count: 100000
    concurrency: 64
    operations: [put, get, delete]

  - name: large_objects
    object_size: 100MB
    object_count: 100
    concurrency: 8
    operations: [put, get]

  - name: mixed_workload
    object_sizes: [4KB, 64KB, 1MB, 10MB]
    distribution: [0.7, 0.2, 0.08, 0.02]
    read_ratio: 0.8
    duration: 300s
```

#### Expected Competitive Position
| Workload | vs MinIO | vs SeaweedFS | vs Garage |
|----------|----------|--------------|-----------|
| Small PUT | Target: 1.2x | Target: 1.5x | Target: 1.0x |
| Large PUT | Target: 1.0x | Target: 1.0x | Target: 1.1x |
| Small GET | Target: 1.3x | Target: 1.2x | Target: 1.1x |
| Large GET | Target: 1.0x | Target: 1.0x | Target: 1.0x |
| Parquet | Target: 2.0x | N/A | N/A |

---

## Appendix C: Enhanced Testing Strategy

### Test Pyramid

```
                    /\
                   /  \  E2E Tests (K3d, Docker)
                  /    \  10 tests, 30 min
                 /------\
                /        \  Integration Tests
               /          \  100 tests, 10 min
              /------------\
             /              \  Property-Based Tests
            /                \  50 tests, 5 min
           /------------------\
          /                    \  Unit Tests
         /                      \  500 tests, 2 min
        /------------------------\
```

### Test Categories

#### T1: Unit Tests (per crate)
```rust
#[cfg(test)]
mod tests {
    // Fast, isolated tests
    // Mock all dependencies
    // Run in parallel
}
```

#### T2: Integration Tests (cross-crate)
```rust
// tests/integration/s3_operations.rs
#[tokio::test]
async fn test_put_get_delete_cycle() {
    let cluster = TestCluster::new(3).await;
    // Test against real cluster
}
```

#### T3: Property-Based Tests
```rust
// tests/property/consistency.rs
proptest! {
    #[test]
    fn writes_are_durable(ops in vec(operation_strategy(), 1..100)) {
        // Generate random operations
        // Execute against cluster
        // Verify all committed writes survive restart
    }
}
```

#### T4: Chaos Tests
```rust
// tests/chaos/network_partition.rs
#[tokio::test]
async fn test_minority_partition_cannot_write() {
    let cluster = TestCluster::new(5).await;

    // Partition nodes 0,1 from nodes 2,3,4
    cluster.partition(vec![0, 1], vec![2, 3, 4]).await;

    // Writes to minority should fail
    let result = cluster.node(0).put("key", "value").await;
    assert!(result.is_err());

    // Writes to majority should succeed
    let result = cluster.node(2).put("key", "value").await;
    assert!(result.is_ok());
}
```

#### T5: E2E Tests (K3d)
```bash
#!/bin/bash
# tests/e2e/k3d_test.sh

# Create cluster
k3d cluster create anode-test --servers 3

# Install anode via Helm
helm install anode ./deploy/helm/anode \
  --set replicas=3 \
  --wait --timeout 5m

# Run S3 compatibility tests
aws s3 --endpoint-url=http://localhost:8080 mb s3://test-bucket
aws s3 --endpoint-url=http://localhost:8080 cp /tmp/testfile s3://test-bucket/
aws s3 --endpoint-url=http://localhost:8080 ls s3://test-bucket/

# Cleanup
k3d cluster delete anode-test
```

### Test Data Generation

```rust
// tests/harness/src/generators.rs

pub fn random_object_key() -> String {
    format!("test/{}/{}", Uuid::new_v4(), Uuid::new_v4())
}

pub fn random_parquet_file(rows: usize) -> Vec<u8> {
    // Generate valid parquet file with random data
}

pub fn realistic_workload(duration: Duration) -> WorkloadSpec {
    WorkloadSpec {
        operations: vec![
            (Operation::Put, 0.2),
            (Operation::Get, 0.7),
            (Operation::Delete, 0.05),
            (Operation::List, 0.05),
        ],
        object_sizes: ObjectSizeDistribution::Zipf { alpha: 1.2 },
        key_pattern: KeyPattern::Hierarchical { depth: 3..6 },
    }
}
```

---

## Appendix D: CI/CD Pipeline Details

### GitHub Actions Workflows

#### Main CI (`ci.yml`)
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --all-targets --all-features

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --all-targets --all-features -- -D warnings

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

#### Benchmark CI (`bench.yml`)
```yaml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Run benchmarks
        run: cargo bench --all-features -- --save-baseline main

      - name: Generate report
        run: cargo run --bin bench-report > BENCHMARKS.md

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: benchmarks
          path: |
            target/criterion
            BENCHMARKS.md

      - name: Comment on PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const report = fs.readFileSync('BENCHMARKS.md', 'utf8');
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: '## Benchmark Results\n\n' + report
            });
```

#### K3d Integration (`k3d.yml`)
```yaml
name: K3d Integration

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  k3d-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install k3d
        run: |
          curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash

      - name: Create cluster
        run: k3d cluster create anode-ci --servers 3 --wait

      - name: Build and load image
        run: |
          docker build -t anode:ci .
          k3d image import anode:ci -c anode-ci

      - name: Install Helm chart
        run: |
          helm install anode ./deploy/helm/anode \
            --set image.repository=anode \
            --set image.tag=ci \
            --wait --timeout 5m

      - name: Run integration tests
        run: ./tests/e2e/run_tests.sh

      - name: Collect logs on failure
        if: failure()
        run: |
          kubectl logs -l app=anode --all-containers > anode-logs.txt

      - name: Cleanup
        if: always()
        run: k3d cluster delete anode-ci
```

#### Chaos Testing (`chaos.yml`)
```yaml
name: Chaos Tests

on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM
  workflow_dispatch:

jobs:
  chaos:
    runs-on: ubuntu-latest
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Build chaos test binary
        run: cargo build --release -p anode-chaos-tests

      - name: Start docker-compose cluster
        run: docker-compose -f deploy/docker/docker-compose.chaos.yml up -d

      - name: Run chaos scenarios
        run: |
          cargo run --release -p anode-chaos-tests -- \
            --scenario network-partition \
            --scenario node-crash \
            --scenario slow-network \
            --scenario rolling-restart \
            --duration 10m

      - name: Collect results
        run: |
          mkdir -p chaos-results
          cp target/chaos/*.json chaos-results/

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: chaos-results
          path: chaos-results/
```

---

## Success Criteria

- [ ] All tests pass (unit, integration, chaos)
- [ ] Clippy clean with all lints enabled
- [ ] Benchmark baselines established
- [ ] K3d integration tests pass
- [ ] Documentation complete
- [ ] Security scan clean
- [ ] Formal verification for critical paths
- [ ] Benchmark comparison with MinIO, SeaweedFS, Garage
- [ ] 3-node cluster survives:
  - Single node failure
  - Network partition
  - Disk corruption
  - Rolling restart
