// Raft replication latency benchmarks
//
// These benchmarks measure the latency of replicating entries between Raft nodes
// with different entry sizes and batch sizes. Since we don't have a full network
// stack set up, we simulate replication by measuring the key operations involved:
// serialization, network transfer simulation, and state machine application.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

mod common {
    pub use anode_bench::*;
}

use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};
use anode_raft::{StateMachine, TypeConfig};
use anode_storage::{StorageConfig, StorageEngine};
use common::*;
use openraft::{Entry, EntryPayload, LogId};
use tempfile::TempDir;

/// Create a test log entry with a PutObject command of the given size
fn create_log_entry(index: u64, data_size: usize) -> Entry<TypeConfig> {
    use anode_raft::{Command, Request};

    // Create a manifest with chunk data
    let num_chunks = (data_size / (128 * 1024)).max(1); // 128KB chunks
    let chunks: Vec<ChunkId> = (0..num_chunks)
        .map(|i| {
            let mut hash = [0u8; 32];
            hash[0..8].copy_from_slice(&i.to_le_bytes());
            ChunkId::from_hash(&hash).expect("Failed to create ChunkId")
        })
        .collect();

    let manifest = ObjectManifest::new(
        ObjectKey::new("benchmark-bucket", &format!("object-{}", index)),
        data_size as u64,
        [0u8; 32],
        chunks,
    );

    let request = Request {
        client_id: "benchmark".to_string(),
        serial: index,
        command: Command::PutObject {
            bucket: "benchmark-bucket".to_string(),
            key: format!("object-{}", index),
            manifest,
        },
    };

    Entry::<TypeConfig> {
        log_id: LogId {
            leader_id: openraft::LeaderId::new(1, 0),
            index,
        },
        payload: EntryPayload::Normal(request),
    }
}

/// Simulate network latency with a sleep
fn simulate_network_latency_us(latency_us: u64) {
    std::thread::sleep(std::time::Duration::from_micros(latency_us));
}

/// Benchmark end-to-end replication latency for single entries
///
/// Measures the complete replication flow:
/// 1. Serialize entry on leader
/// 2. Simulate network transfer
/// 3. Deserialize on follower
/// 4. Apply to state machine
fn bench_single_replication_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_single_replication");

    // Test different entry sizes
    let sizes = [
        (4 * 1024, "4kb"),          // Small object
        (64 * 1024, "64kb"),        // Medium object
        (1024 * 1024, "1mb"),       // Large object
        (10 * 1024 * 1024, "10mb"), // Very large object
    ];

    // Network latencies to simulate (in microseconds)
    let network_latencies = [
        (0, "local"),  // Local network (essentially zero)
        (100, "lan"),  // LAN latency
        (1000, "wan"), // WAN latency within region
    ];

    for (size, size_label) in sizes.iter() {
        for (latency_us, latency_label) in network_latencies.iter() {
            group.throughput(Throughput::Bytes(*size as u64));

            group.bench_with_input(
                BenchmarkId::new(
                    format!("{}_{}", size_label, latency_label),
                    format!("{}bytes_{}us", size, latency_us),
                ),
                &(*size, *latency_us),
                |b, &(size, latency_us)| {
                    b.iter_custom(|iters| {
                        let mut latencies = Vec::new();

                        for iter_idx in 0..iters {
                            // Create entry
                            let entry = create_log_entry(iter_idx + 1, size);

                            let start = Instant::now();

                            // Step 1: Serialize on leader
                            let encoded = bincode::serialize(&entry).unwrap();

                            // Step 2: Simulate network transfer
                            simulate_network_latency_us(latency_us);

                            // Step 3: Deserialize on follower
                            let _decoded: Entry<TypeConfig> =
                                bincode::deserialize(&encoded).unwrap();

                            // Step 4: Simulate state machine application
                            // (In reality, this would involve storage operations)

                            latencies.push(start.elapsed());
                        }

                        latencies.iter().sum()
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark batch replication latency
///
/// Measures replication of multiple entries in a single batch,
/// which is the common case in high-throughput scenarios.
fn bench_batch_replication_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_batch_replication");

    // Test different batch sizes
    let batch_sizes = [1, 10, 50, 100];
    let entry_size = 64 * 1024; // 64KB per entry
    let network_latency_us = 100; // LAN latency

    for batch_size in batch_sizes.iter() {
        group.throughput(Throughput::Bytes((batch_size * entry_size) as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", format!("{}entries", batch_size)),
            batch_size,
            |b, &batch_size| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        // Create batch of entries
                        let entries: Vec<_> = (1..=batch_size)
                            .map(|i| create_log_entry(i as u64, entry_size))
                            .collect();

                        let start = Instant::now();

                        // Step 1: Serialize all entries
                        let encoded: Vec<Vec<u8>> = entries
                            .iter()
                            .map(|e| bincode::serialize(e).unwrap())
                            .collect();

                        // Step 2: Simulate network transfer (single round-trip for batch)
                        simulate_network_latency_us(network_latency_us);

                        // Step 3: Deserialize all entries on follower
                        let _decoded: Vec<Entry<TypeConfig>> = encoded
                            .iter()
                            .map(|data| bincode::deserialize(data).unwrap())
                            .collect();

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark serialization overhead for replication
///
/// Isolates just the serialization/deserialization cost to understand
/// how much of replication latency is CPU-bound encoding.
fn bench_replication_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_replication_serialization");

    let sizes = [
        (4 * 1024, "4kb"),
        (64 * 1024, "64kb"),
        (1024 * 1024, "1mb"),
        (10 * 1024 * 1024, "10mb"),
    ];

    for (size, label) in sizes.iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("serialize_deserialize", label),
            size,
            |b, &size| {
                let entry = create_log_entry(1, size);

                b.iter(|| {
                    // Serialize
                    let encoded = bincode::serialize(&entry).unwrap();
                    // Deserialize
                    let _decoded: Entry<TypeConfig> = bincode::deserialize(&encoded).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark state machine application latency
///
/// Measures the time to apply entries to the state machine,
/// which is a critical part of the replication process.
fn bench_state_machine_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_state_machine_apply");

    let sizes = [(4 * 1024, "4kb"), (64 * 1024, "64kb"), (1024 * 1024, "1mb")];

    for (size, label) in sizes.iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("apply", label), size, |b, &size| {
            b.iter_custom(|iters| {
                let mut latencies = Vec::new();

                // Setup: Create fresh storage once for all iterations
                let temp_dir = TempDir::new().unwrap();
                let storage_config = StorageConfig {
                    data_dir: temp_dir.path().join("blobs"),
                    metadata_dir: temp_dir.path().join("metadata"),
                    chunk_size: anode_storage::chunks::CHUNK_SIZE,
                    sync_writes: false,
                };

                let rt = tokio::runtime::Runtime::new().unwrap();
                let storage = rt.block_on(async {
                    Arc::new(StorageEngine::open(storage_config).await.unwrap())
                });

                let snapshot_dir = temp_dir.path().join("snapshots");
                std::fs::create_dir_all(&snapshot_dir).unwrap();
                let mut state_machine = StateMachine::new(storage, snapshot_dir);

                // Create bucket for put operations
                rt.block_on(async {
                    use anode_raft::{BucketConfig, Command, Request};
                    use openraft::storage::RaftStateMachine;
                    let create_bucket = Entry::<TypeConfig> {
                        log_id: LogId {
                            leader_id: openraft::LeaderId::new(1, 0),
                            index: 0,
                        },
                        payload: EntryPayload::Normal(Request {
                            client_id: "benchmark".to_string(),
                            serial: 0,
                            command: Command::CreateBucket {
                                name: "benchmark-bucket".to_string(),
                                config: BucketConfig {
                                    name: "benchmark-bucket".to_string(),
                                    replication_factor: 3,
                                    created_at: 0,
                                },
                            },
                        }),
                    };
                    let _responses = state_machine.apply(vec![create_bucket]).await;
                });

                for iter_idx in 0..iters {
                    // Create entry
                    let entry = create_log_entry(iter_idx + 1, size);

                    let start = Instant::now();

                    // Apply entry to state machine
                    rt.block_on(async {
                        use openraft::storage::RaftStateMachine;
                        let _responses = state_machine.apply(vec![entry]).await;
                    });

                    latencies.push(start.elapsed());
                }

                latencies.iter().sum()
            });
        });
    }

    group.finish();
}

/// Benchmark multi-node replication simulation
///
/// Simulates replication to multiple followers to measure
/// the impact of replication factor on latency.
fn bench_multi_node_replication(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_multi_node_replication");

    let entry_size = 64 * 1024; // 64KB
    let network_latency_us = 100; // LAN latency
    let replication_factors = [3, 5, 7]; // Common cluster sizes

    for replication_factor in replication_factors.iter() {
        group.throughput(Throughput::Bytes(entry_size as u64));

        group.bench_with_input(
            BenchmarkId::new("replicate", format!("{}nodes", replication_factor)),
            replication_factor,
            |b, &replication_factor| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for iter_idx in 0..iters {
                        let entry = create_log_entry(iter_idx + 1, entry_size);

                        let start = Instant::now();

                        // Serialize once on leader
                        let encoded = bincode::serialize(&entry).unwrap();

                        // Replicate to each follower (can be done in parallel, but we measure sequential)
                        for _ in 0..replication_factor - 1 {
                            // Simulate network transfer to this follower
                            simulate_network_latency_us(network_latency_us);

                            // Follower deserializes
                            let _decoded: Entry<TypeConfig> =
                                bincode::deserialize(&encoded).unwrap();
                        }

                        // Wait for quorum (majority of nodes)
                        // In real implementation, leader waits for (replication_factor / 2 + 1) responses

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark pipeline replication
///
/// Simulates pipelined replication where the leader can send multiple
/// batches without waiting for acknowledgments, measuring throughput.
fn bench_pipeline_replication(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_pipeline_replication");

    let entry_size = 64 * 1024; // 64KB
    let network_latency_us = 100;
    let pipeline_depths = [1, 5, 10, 20]; // Number of in-flight batches

    for pipeline_depth in pipeline_depths.iter() {
        group.throughput(Throughput::Bytes((pipeline_depth * entry_size) as u64));

        group.bench_with_input(
            BenchmarkId::new("pipeline", format!("depth{}", pipeline_depth)),
            pipeline_depth,
            |b, &pipeline_depth| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        // Create pipeline_depth entries
                        let entries: Vec<_> = (1..=pipeline_depth)
                            .map(|i| create_log_entry(i as u64, entry_size))
                            .collect();

                        let start = Instant::now();

                        // Simulate sending all entries in pipeline
                        for entry in &entries {
                            let encoded = bincode::serialize(entry).unwrap();
                            // In real pipeline, wouldn't wait here
                            let _decoded: Entry<TypeConfig> =
                                bincode::deserialize(&encoded).unwrap();
                        }

                        // Simulate single network round-trip for acknowledgment
                        simulate_network_latency_us(network_latency_us);

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    raft_replication_benches,
    bench_single_replication_latency,
    bench_batch_replication_latency,
    bench_replication_serialization,
    bench_state_machine_apply,
    bench_multi_node_replication,
    bench_pipeline_replication,
);

criterion_main!(raft_replication_benches);
