# Integration Test Index

Complete listing of all integration tests organized by category.

## S3 Basic Operations (s3_basic.rs)

### Bucket Operations
- `test_create_and_delete_bucket` - Create, verify, and delete buckets

### Object Operations
- `test_put_get_object` - Basic object upload and retrieval
- `test_delete_objects_batch` - Delete multiple objects
- `test_delete_nonexistent_object` - Delete non-existent object (S3 idempotency)
- `test_copy_object` - Copy objects between keys
- `test_empty_object` - Handle zero-byte objects
- `test_object_overwrite` - Overwrite existing objects

### Listing Operations
- `test_list_objects` - List objects with prefix filtering

### Metadata Operations
- `test_object_metadata` - Store and retrieve object metadata
- `test_head_object` - Get object metadata without body

### Advanced Operations
- `test_range_get` - Range requests (partial object reads)
- `test_conditional_get` - Conditional requests (If-Match, If-None-Match)

**Total: 12 tests**

---

## S3 Multipart Operations (s3_multipart.rs)

### Basic Multipart
- `test_multipart_upload` - Complete multipart upload (3 parts × 5MB)
- `test_multipart_abort` - Abort in-progress multipart upload
- `test_multipart_list_parts` - List uploaded parts

### Large Objects
- `test_large_object_multipart` - Upload 100MB object in 10MB parts

### Advanced Multipart
- `test_multipart_with_metadata` - Multipart upload with custom metadata
- `test_multipart_list_uploads` - List in-progress multipart uploads
- `test_multipart_part_size_validation` - Variable part sizes
- `test_multipart_copy` - Copy large objects using multipart

**Total: 8 tests**

---

## Cluster Operations (cluster_ops.rs)

### Status and Health
- `test_cluster_status` - Query cluster status from all nodes
- `test_health_check` - Health endpoint verification
- `test_cluster_info` - Detailed cluster information
- `test_cluster_metrics` - Metrics endpoint validation

### Leader Election
- `test_leader_election` - Verify leader election and consensus

### Node Management
- `test_add_node` - Add new node to cluster
- `test_remove_node` - Remove node from cluster
- `test_node_restart` - Restart node and verify rejoin

### Quorum and Resilience
- `test_cluster_quorum` - Verify quorum with 2/3 nodes
- `test_minority_failure` - Handle minority node failures (2/5)

### Data Operations
- `test_rebalance` - Data rebalancing across nodes

**Total: 11 tests**

---

## Failover Scenarios (failover.rs)

### Leader Failover
- `test_leader_failover` - Leader failure and re-election
- `test_leader_failure_during_write` - Leader fails during concurrent writes
- `test_rapid_leader_changes` - Multiple sequential leader failures

### Follower Failures
- `test_follower_failure` - Single follower failure
- `test_minority_failure` - Multiple follower failures

### Network Partitions
- `test_network_partition` - Network partition and healing
- `test_split_brain_prevention` - Prevent split-brain scenarios

### Cascading Failures
- `test_cascading_failure` - Sequential node failures

### Data Durability
- `test_data_durability_on_failure` - Data survives node failures

**Total: 9 tests**

---

## Performance Regression (performance.rs)

### Throughput Tests
- `test_throughput_baseline` - Read/write throughput (≥50 MB/s)
- `test_large_object_performance` - 100MB object throughput

### Latency Tests
- `test_latency_baseline` - Request latency (p50, p95, p99 ≤500ms)

### Concurrent Operations
- `test_concurrent_writes` - Concurrent write performance (≥100 ops/sec)
- `test_concurrent_reads` - Concurrent read performance (≥100 ops/sec)
- `test_mixed_workload_performance` - Mixed read/write workload

### Operation-Specific Performance
- `test_list_performance` - List 1000 objects performance
- `test_delete_performance` - Bulk delete performance
- `test_metadata_operation_performance` - HeadObject performance

**Total: 9 tests**

---

## Parquet Optimizations (parquet.rs)

### Metadata Caching
- `test_parquet_metadata_caching` - Verify metadata cache hits
- `test_parquet_stats_api` - Parquet statistics API

### Footer Operations
- `test_parquet_footer_optimization` - Footer-only reads
- `test_parquet_range_reads` - Column chunk range reads

### Multi-File Operations
- `test_parquet_multiple_files` - Multiple Parquet files in partitions
- `test_parquet_concurrent_reads` - Concurrent column reads

### Large Files
- `test_parquet_large_file` - 100MB Parquet file handling

### Metadata
- `test_parquet_compression_metadata` - Compression metadata preservation

### Query Optimization
- `test_parquet_query_optimization` - Partition pruning and query optimization

**Total: 9 tests**

---

## Test Utilities (main.rs)

### Setup Functions
- `setup_cluster(num_nodes)` - Create and start a test cluster
- `get_shared_cluster()` - Get shared 3-node cluster
- `create_s3_client(endpoint)` - Create AWS S3 client
- `create_test_s3_client(cluster)` - Create S3 client for cluster

### Helper Functions
- `random_bytes(size)` - Generate random test data
- `unique_bucket_name(prefix)` - Generate unique bucket name
- `unique_object_key(prefix)` - Generate unique object key
- `cleanup_bucket(client, bucket)` - Clean up test bucket

### Fixtures
- `BucketFixture` - Auto-cleanup bucket for tests

---

## Summary Statistics

| Category | Test Count | Lines of Code |
|----------|-----------|---------------|
| S3 Basic | 12 | 565 |
| S3 Multipart | 8 | 650 |
| Cluster Ops | 11 | 501 |
| Failover | 9 | 694 |
| Performance | 9 | 645 |
| Parquet | 9 | 617 |
| Utilities | - | 212 |
| **Total** | **58** | **3,884** |

## Test Characteristics

### Execution
- All tests use `#[tokio::test]` for async execution
- All tests use `#[serial]` to prevent concurrent execution
- Tests are hermetic (self-contained, no external dependencies)

### Cleanup
- Automatic cleanup via `Drop` implementations
- `BucketFixture` cleans up buckets and objects
- `TestCluster` stops all nodes and removes temp directories

### Performance Baselines
- Small object operations: ≥100 ops/sec
- Throughput: ≥50 MB/s
- P99 latency: ≤500ms

### Coverage Areas
- ✅ S3 API compatibility
- ✅ Multipart uploads
- ✅ Cluster management
- ✅ Failure recovery
- ✅ Performance validation
- ✅ Parquet optimizations

## Running Specific Test Categories

```bash
# S3 Basic
cargo test --test integration s3_basic

# Multipart
cargo test --test integration s3_multipart

# Cluster operations
cargo test --test integration cluster_ops

# Failover scenarios
cargo test --test integration failover

# Performance tests
cargo test --test integration performance

# Parquet tests
cargo test --test integration parquet
```

## Test Naming Convention

Tests follow this naming pattern:
- `test_<operation>_<scenario>`
- Examples:
  - `test_put_get_object` - Basic operation
  - `test_leader_failover` - Failure scenario
  - `test_multipart_abort` - Specific multipart operation
  - `test_parquet_metadata_caching` - Parquet optimization

## Future Test Additions

Planned test coverage:
- [ ] Cross-region replication
- [ ] Authentication and authorization
- [ ] Backup and restore operations
- [ ] Schema evolution for Parquet
- [ ] Compaction and garbage collection
- [ ] Rate limiting and quotas
- [ ] Encryption at rest
- [ ] Access logging
