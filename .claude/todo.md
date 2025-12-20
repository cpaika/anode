# Anode Task Queue

This is the comprehensive task queue for building anode - a production-grade distributed object storage system in Rust.

Run `/todo-all` to process all tasks sequentially with subagents.

---

## Phase 0: Critical Fixes (Get to Green)

- [x] Fix anode-raft/src/storage.rs lifetime issue on line 56 - save table.iter()?.last() result to local variable before returning
- [x] Fix all remaining openraft 0.9 API errors in anode-raft/src/storage.rs
- [x] Fix anode-raft/src/state_machine.rs to match openraft 0.9 RaftStateMachine trait
- [x] Fix anode-raft/src/types.rs TypeConfig to match openraft 0.9 RaftTypeConfig requirements
- [x] Fix anode-raft/src/lib.rs module exports and RaftNode builder
- [x] Fix anode-raft/src/error.rs to properly convert between error types
- [x] Run cargo check -p anode-raft and verify zero errors
- [x] Run cargo check on entire workspace and verify zero errors
- [x] Run cargo clippy -p anode-storage and fix all warnings
- [x] Run cargo clippy -p anode-raft and fix all warnings
- [x] Run cargo clippy -p anode-s3 and fix all warnings
- [x] Run cargo clippy -p anode-protocol and fix all warnings
- [x] Run cargo clippy -p anode-parquet and fix all warnings
- [x] Run cargo clippy -p anode-client and fix all warnings
- [x] Run cargo clippy -p anode and fix all warnings (main binary)
- [x] Run cargo clippy on test harness crates and fix all warnings
- [x] Run cargo fmt --all --check and fix any formatting issues
- [x] Run cargo test on core crates (storage: 60 pass, raft: 22 pass, parquet: 42 pass, s3: 41 pass)

## Phase 1: Storage Engine Unit Tests

- [x] Add unit test for ChunkManager::split_into_chunks with 0 byte input
- [x] Add unit test for ChunkManager::split_into_chunks with 1 byte input
- [x] Add unit test for ChunkManager::split_into_chunks with exactly 4MB input
- [x] Add unit test for ChunkManager::split_into_chunks with 4MB+1 byte input
- [x] Add unit test for ChunkManager::split_into_chunks with 100MB input
- [ ] Add unit test for ChunkId::from_content produces consistent SHA-256 hash
- [ ] Add unit test for identical content produces identical ChunkId (deduplication)
- [ ] Add unit test for ChunkManager::retrieve_chunks roundtrip with small data
- [ ] Add unit test for ChunkManager::retrieve_chunks roundtrip with large data
- [ ] Add unit test for BlobStore::new creates data directory
- [ ] Add unit test for BlobStore::write_chunk stores data at correct path
- [ ] Add unit test for BlobStore::read_chunk returns correct data
- [ ] Add unit test for BlobStore::read_chunk returns error for non-existent chunk
- [ ] Add unit test for BlobStore::delete_chunk removes file
- [ ] Add unit test for BlobStore::exists_chunk returns correct result
- [ ] Add unit test for MetadataStore::new creates redb database
- [ ] Add unit test for MetadataStore::create_bucket creates bucket entry
- [ ] Add unit test for MetadataStore::create_bucket fails for duplicate
- [ ] Add unit test for MetadataStore::delete_bucket removes bucket
- [ ] Add unit test for MetadataStore::delete_bucket fails for non-existent
- [ ] Add unit test for MetadataStore::list_buckets returns all buckets
- [ ] Add unit test for MetadataStore::put_object stores manifest
- [ ] Add unit test for MetadataStore::get_object retrieves manifest
- [ ] Add unit test for MetadataStore::get_object returns None for non-existent
- [ ] Add unit test for MetadataStore::delete_object removes manifest
- [ ] Add unit test for MetadataStore::list_objects returns all objects
- [ ] Add unit test for MetadataStore::list_objects with prefix filter
- [ ] Add unit test for MetadataStore::list_objects with limit (pagination)

## Phase 2: Storage Engine Integration Tests

- [ ] Add integration test for StorageEngine::put_object with small data
- [ ] Add integration test for StorageEngine::put_object with large data (10MB)
- [ ] Add integration test for StorageEngine::put_object with metadata
- [ ] Add integration test for StorageEngine::get_object returns correct data
- [ ] Add integration test for StorageEngine::get_object returns correct metadata
- [ ] Add integration test for StorageEngine::get_object returns None for missing
- [ ] Add integration test for StorageEngine::delete_object removes object
- [ ] Add integration test for StorageEngine::delete_object is idempotent
- [ ] Add integration test for StorageEngine::list_objects basic listing
- [ ] Add integration test for StorageEngine::list_objects with prefix
- [ ] Add integration test for StorageEngine::create_bucket and delete_bucket
- [ ] Add integration test for concurrent puts to different keys succeed
- [ ] Add integration test for concurrent puts to same key are serialized
- [ ] Add integration test for put then get returns identical data (100 iterations)
- [ ] Add integration test for large object (100MB) put and get roundtrip
- [ ] Add integration test for StorageEngine::stats returns accurate counts

## Phase 3: Raft Consensus Unit Tests

- [ ] Add unit test for LogStore::open creates database file
- [ ] Add unit test for LogStore::save_vote persists vote
- [ ] Add unit test for LogStore::read_vote returns saved vote
- [ ] Add unit test for LogStore::read_vote returns None when no vote saved
- [ ] Add unit test for LogStore append single entry
- [ ] Add unit test for LogStore append multiple entries
- [ ] Add unit test for LogStore::try_get_log_entries range query
- [ ] Add unit test for LogStore::truncate removes entries after index
- [ ] Add unit test for LogStore::purge removes entries up to index
- [ ] Add unit test for LogStore::get_log_state returns correct state
- [ ] Add unit test for state machine apply PutObject command
- [ ] Add unit test for state machine apply DeleteObject command
- [ ] Add unit test for state machine apply CreateBucket command
- [ ] Add unit test for state machine apply DeleteBucket command
- [ ] Add unit test for RaftNetworkManager::add_node adds to registry
- [ ] Add unit test for RaftNetworkManager::remove_node removes from registry
- [ ] Add unit test for TypeConfig satisfies RaftTypeConfig trait bounds

## Phase 4: S3 API Unit Tests

- [ ] Add unit test for S3 XML list objects response generation
- [ ] Add unit test for S3 XML list buckets response generation
- [ ] Add unit test for S3 XML error response generation
- [ ] Add unit test for S3 XML multipart initiate response generation
- [ ] Add unit test for S3 XML complete multipart request parsing
- [ ] Add unit test for S3 XML delete objects request parsing
- [ ] Add unit test for ETag computation for single-part upload
- [ ] Add unit test for ETag computation for multipart upload
- [ ] Add unit test for AWS SigV4 signature verification (valid signature)
- [ ] Add unit test for AWS SigV4 signature verification (invalid signature)
- [ ] Add unit test for presigned URL signature generation
- [ ] Add unit test for presigned URL expiration check
- [ ] Add unit test for range header parsing (bytes=0-100)
- [ ] Add unit test for range header parsing (bytes=100-)
- [ ] Add unit test for range header parsing (bytes=-100)

## Phase 5: S3 API Integration Tests

- [ ] Add integration test for PUT object basic
- [ ] Add integration test for PUT object with Content-Type header
- [ ] Add integration test for PUT object with custom metadata (x-amz-meta-*)
- [ ] Add integration test for PUT object with Content-MD5 validation
- [ ] Add integration test for PUT object overwrite existing object
- [ ] Add integration test for GET object basic
- [ ] Add integration test for GET object with range request
- [ ] Add integration test for GET object returns 404 for missing
- [ ] Add integration test for GET object If-Match success
- [ ] Add integration test for GET object If-Match failure (412)
- [ ] Add integration test for GET object If-None-Match (304)
- [ ] Add integration test for HEAD object returns correct headers
- [ ] Add integration test for HEAD object returns 404 for missing
- [ ] Add integration test for DELETE object basic
- [ ] Add integration test for DELETE object idempotent (success on missing)
- [ ] Add integration test for LIST objects basic
- [ ] Add integration test for LIST objects with prefix
- [ ] Add integration test for LIST objects with delimiter
- [ ] Add integration test for LIST objects with max-keys
- [ ] Add integration test for LIST objects pagination with continuation-token
- [ ] Add integration test for LIST objects CommonPrefixes
- [ ] Add integration test for multipart upload initiate
- [ ] Add integration test for multipart upload part
- [ ] Add integration test for multipart upload complete
- [ ] Add integration test for multipart upload abort
- [ ] Add integration test for multipart upload list parts
- [ ] Add integration test for PUT bucket create
- [ ] Add integration test for DELETE bucket empty
- [ ] Add integration test for DELETE bucket non-empty fails
- [ ] Add integration test for LIST buckets

## Phase 6: Property-Based Tests

- [ ] Add proptest for chunk split/retrieve roundtrip is identity
- [ ] Add proptest for object put/get roundtrip is identity
- [ ] Add proptest for any valid bucket name is accepted
- [ ] Add proptest for any valid object key is accepted
- [ ] Add proptest for metadata roundtrip preserves all keys/values
- [ ] Add proptest for ETag is deterministic for same content
- [ ] Add proptest for list objects returns all put objects
- [ ] Add proptest for delete then get returns None
- [ ] Add proptest for concurrent operations maintain consistency
- [ ] Add stateful proptest for CRUD operations sequence

## Phase 7: Cluster Integration Tests

- [ ] Add test for 3-node cluster bootstrap
- [ ] Add test for leader election occurs after startup
- [ ] Add test for write to leader succeeds
- [ ] Add test for write to follower redirects or fails appropriately
- [ ] Add test for read from any node returns consistent data
- [ ] Add test for leader failure triggers new election
- [ ] Add test for new leader serves writes after election
- [ ] Add test for follower rejoin after restart
- [ ] Add test for data survives single node restart
- [ ] Add test for data survives minority node failures
- [ ] Add test for split brain is prevented (minority cannot write)

## Phase 8: Chaos Tests

- [ ] Add chaos test for network partition minority cannot write
- [ ] Add chaos test for network partition majority can write
- [ ] Add chaos test for network partition heal restores cluster
- [ ] Add chaos test for leader crash during write
- [ ] Add chaos test for follower crash during replication
- [ ] Add chaos test for all nodes restart data survives
- [ ] Add chaos test for rolling restart zero data loss
- [ ] Add chaos test for slow network (100ms latency)
- [ ] Add chaos test for packet loss (10%)
- [ ] Add chaos test for disk full simulation
- [ ] Add chaos test for corrupt chunk detection and recovery

## Phase 9: Performance Benchmarks

- [ ] Add benchmark for PUT 1KB object throughput (ops/sec)
- [ ] Add benchmark for PUT 1MB object throughput (MB/sec)
- [ ] Add benchmark for PUT 100MB object throughput (MB/sec)
- [ ] Add benchmark for GET 1KB object throughput (ops/sec)
- [ ] Add benchmark for GET 1MB object throughput (MB/sec)
- [ ] Add benchmark for GET 100MB object throughput (MB/sec)
- [ ] Add benchmark for LIST 10K objects latency
- [ ] Add benchmark for concurrent PUT (64 clients)
- [ ] Add benchmark for concurrent GET (64 clients)
- [ ] Add benchmark for mixed workload (80% read, 20% write)
- [ ] Add benchmark for chunk deduplication ratio
- [ ] Add benchmark for Raft log append latency
- [ ] Add benchmark for Raft replication latency
- [ ] Create benchmark report generator outputting BENCHMARKS.md
- [ ] Add CI workflow to run benchmarks and comment on PRs

## Phase 10: Parquet Support

- [ ] Add unit test for parquet footer parsing
- [ ] Add unit test for parquet row group extraction
- [ ] Add unit test for parquet column statistics extraction
- [ ] Add unit test for parquet metadata cache hit
- [ ] Add unit test for parquet metadata cache miss
- [ ] Add unit test for parquet metadata cache invalidation
- [ ] Add integration test for parquet file upload
- [ ] Add integration test for parquet file download
- [ ] Add integration test for parquet range read optimization
- [ ] Add benchmark for parquet metadata query vs MinIO

## Phase 11: Observability

- [ ] Implement /metrics Prometheus endpoint
- [ ] Add request_count metric with labels (method, status)
- [ ] Add request_latency_seconds histogram
- [ ] Add request_size_bytes histogram
- [ ] Add storage_bytes_total gauge
- [ ] Add storage_objects_total gauge
- [ ] Add raft_term gauge
- [ ] Add raft_is_leader gauge
- [ ] Add raft_commit_index gauge
- [ ] Add test for /metrics endpoint returns valid prometheus format
- [ ] Implement /health endpoint returning cluster health
- [ ] Implement /ready endpoint for Kubernetes readiness probe
- [ ] Implement /live endpoint for Kubernetes liveness probe
- [ ] Add test for health endpoints
- [ ] Implement structured JSON logging
- [ ] Add request_id to all log entries
- [ ] Add configurable log level per module

## Phase 12: Docker

- [ ] Create optimized multi-stage Dockerfile
- [ ] Use distroless base image for minimal attack surface
- [ ] Add non-root user to Dockerfile
- [ ] Add HEALTHCHECK instruction to Dockerfile
- [ ] Add test for docker build succeeds
- [ ] Add test for docker run starts server
- [ ] Create docker-compose.yml for 3-node dev cluster
- [ ] Add Prometheus and Grafana to docker-compose
- [ ] Add test for docker-compose cluster healthy
- [ ] Create docker-compose.chaos.yml for chaos testing

## Phase 13: Kubernetes/Helm

- [ ] Run helm lint on chart
- [ ] Add test for helm template renders correctly
- [ ] Verify StatefulSet has correct pod management policy
- [ ] Verify headless Service for peer discovery
- [ ] Verify ConfigMap for configuration
- [ ] Add Secret for access credentials
- [ ] Add PodDisruptionBudget for quorum maintenance
- [ ] Add ServiceMonitor for Prometheus Operator
- [ ] Add PrometheusRule for alerts
- [ ] Add Ingress template
- [ ] Add NetworkPolicy template
- [ ] Add helm test hooks for connectivity
- [ ] Create K3d cluster creation script
- [ ] Add test for Helm install in K3d
- [ ] Add test for S3 operations in K3d cluster
- [ ] Add test for node failure recovery in K3d

## Phase 14: CI/CD

- [ ] Verify ci.yml cargo check job passes
- [ ] Verify ci.yml cargo clippy job passes
- [ ] Verify ci.yml cargo fmt job passes
- [ ] Verify ci.yml cargo test job passes
- [ ] Verify security.yml cargo-audit passes
- [ ] Verify security.yml cargo-deny passes
- [ ] Add MSRV check (minimum supported Rust version)
- [ ] Add cargo-semver-checks for API compatibility
- [ ] Verify bench.yml runs benchmarks
- [ ] Verify k3d.yml runs integration tests
- [ ] Verify chaos.yml runs chaos tests
- [ ] Add release workflow with semantic versioning
- [ ] Add changelog generation to release workflow
- [ ] Add Docker image publish to release workflow
- [ ] Add Helm chart publish to release workflow

## Phase 15: Documentation

- [ ] Add rustdoc to all public types in anode-storage
- [ ] Add rustdoc to all public types in anode-raft
- [ ] Add rustdoc to all public types in anode-s3
- [ ] Add rustdoc to all public types in anode-protocol
- [ ] Add rustdoc to all public types in anode-parquet
- [ ] Add rustdoc to all public types in anode-client
- [ ] Add code examples in rustdoc for StorageEngine
- [ ] Add code examples in rustdoc for S3 client
- [ ] Write S3 API reference documentation
- [ ] Write Admin API reference documentation
- [ ] Write deployment guide for standalone
- [ ] Write deployment guide for Docker
- [ ] Write deployment guide for Kubernetes
- [ ] Write configuration reference
- [ ] Write troubleshooting guide
- [ ] Write performance tuning guide
- [ ] Create architecture overview document
- [ ] Create data flow diagrams

## Phase 16: Security

- [ ] Implement access key management API
- [ ] Add test for access key creation
- [ ] Add test for access key deletion
- [ ] Add test for access key authentication
- [ ] Implement TLS termination for S3 API
- [ ] Add test for HTTPS connections
- [ ] Implement mTLS for inter-node communication
- [ ] Add test for mTLS between nodes
- [ ] Implement bucket policy support
- [ ] Add test for bucket policy enforcement

## Phase 17: Advanced Features

- [ ] Implement object versioning version ID generation
- [ ] Add test for versioned PUT creates new version
- [ ] Add test for GET specific version
- [ ] Add test for delete marker creation
- [ ] Implement list object versions API
- [ ] Add test for list versions
- [ ] Implement storage quota per bucket
- [ ] Add test for quota enforcement
- [ ] Implement quota exceeded error (403)
- [ ] Implement bucket lifecycle rules
- [ ] Add test for object expiration

## Completed

<!-- Completed tasks will be moved here by the /todo scripts -->
