// Raft log append latency benchmarks
//
// These benchmarks measure the latency of appending entries to the Raft log
// with different entry sizes and batch sizes.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

mod common {
    pub use anode_bench::*;
}

use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};
use anode_raft::TypeConfig;
use common::*;
use openraft::{Entry, EntryPayload, LogId};
use parking_lot::RwLock;
use redb::{Database, ReadableTable, TableDefinition};
use tempfile::TempDir;

// Table definitions for redb (must match LogStore)
const LOGS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("logs");

/// Test-only log store for direct benchmarking
struct TestLogStore {
    db: Arc<Database>,
}

impl TestLogStore {
    fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
        let db = Database::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _logs = write_txn.open_table(LOGS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    fn append_entry(&self, entry: &Entry<TypeConfig>) -> Result<(), Box<dyn std::error::Error>> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LOGS_TABLE)?;
            let encoded = bincode::serialize(&entry)?;
            table.insert(entry.log_id.index, encoded.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn append_entries(
        &self,
        entries: &[Entry<TypeConfig>],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LOGS_TABLE)?;
            for entry in entries {
                let encoded = bincode::serialize(&entry)?;
                table.insert(entry.log_id.index, encoded.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    fn read_entry(&self, index: u64) -> Result<Entry<TypeConfig>, Box<dyn std::error::Error>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LOGS_TABLE)?;
        let value = table.get(index)?.ok_or("Entry not found")?;
        let entry: Entry<TypeConfig> = bincode::deserialize(value.value())?;
        Ok(entry)
    }
}

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

/// Benchmark single log entry append with different entry sizes
///
/// Measures the latency of appending a single entry to the Raft log
/// for different payload sizes (small, medium, large).
fn bench_single_append_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_single_append");

    // Test different entry sizes representing different object sizes
    let sizes = [
        (4 * 1024, "4kb"),          // Small object
        (64 * 1024, "64kb"),        // Medium object
        (1024 * 1024, "1mb"),       // Large object
        (10 * 1024 * 1024, "10mb"), // Very large object
    ];

    for (size, label) in sizes.iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("append", label), size, |b, &size| {
            b.iter_custom(|iters| {
                let mut latencies = Vec::new();

                for iter_idx in 0..iters {
                    // Create a fresh log store for each iteration to avoid
                    // interference from previous iterations
                    let temp_dir = TempDir::new().unwrap();
                    let log_path = temp_dir.path().join("raft.db");
                    let log_store = TestLogStore::open(&log_path).unwrap();

                    // Create log entry
                    let entry = create_log_entry(iter_idx + 1, size);

                    let start = Instant::now();

                    // Append entry to database
                    log_store.append_entry(&entry).unwrap();

                    latencies.push(start.elapsed());
                }

                latencies.iter().sum()
            });
        });
    }

    group.finish();
}

/// Benchmark batch log entry appends
///
/// Measures the latency of appending multiple entries in a single transaction,
/// which is common in high-throughput scenarios.
fn bench_batch_append_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_batch_append");

    // Test different batch sizes
    let batch_sizes = [1, 10, 50, 100];
    let entry_size = 64 * 1024; // 64KB per entry

    for batch_size in batch_sizes.iter() {
        group.throughput(Throughput::Bytes((batch_size * entry_size) as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", format!("{}entries", batch_size)),
            batch_size,
            |b, &batch_size| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        // Create a fresh log store for each iteration
                        let temp_dir = TempDir::new().unwrap();
                        let log_path = temp_dir.path().join("raft.db");
                        let log_store = TestLogStore::open(&log_path).unwrap();

                        // Create batch of entries
                        let entries: Vec<_> = (1..=batch_size)
                            .map(|i| create_log_entry(i as u64, entry_size))
                            .collect();

                        let start = Instant::now();

                        // Append all entries in a single transaction
                        log_store.append_entries(&entries).unwrap();

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark serialization overhead
///
/// Measures just the serialization cost (without database I/O) to understand
/// how much of the append latency is due to encoding.
fn bench_serialization_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_serialization");

    let sizes = [
        (4 * 1024, "4kb"),
        (64 * 1024, "64kb"),
        (1024 * 1024, "1mb"),
        (10 * 1024 * 1024, "10mb"),
    ];

    for (size, label) in sizes.iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("serialize", label), size, |b, &size| {
            // Pre-create entry to avoid allocation overhead in measurement
            let entry = create_log_entry(1, size);

            b.iter(|| {
                let _encoded = bincode::serialize(&entry).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark deserialization (read) latency
///
/// Measures the cost of reading and deserializing log entries from storage.
fn bench_read_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_read");

    let sizes = [
        (4 * 1024, "4kb"),
        (64 * 1024, "64kb"),
        (1024 * 1024, "1mb"),
        (10 * 1024 * 1024, "10mb"),
    ];

    for (size, label) in sizes.iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("read", label), size, |b, &size| {
            // Setup: Create a log store with a pre-populated entry
            let temp_dir = TempDir::new().unwrap();
            let log_path = temp_dir.path().join("raft.db");
            let log_store = TestLogStore::open(&log_path).unwrap();

            let entry = create_log_entry(1, size);

            // Pre-populate the entry
            log_store.append_entry(&entry).unwrap();

            b.iter(|| {
                // Read and deserialize the entry
                let _entry = log_store.read_entry(1).unwrap();
            });

            // Keep temp_dir alive until benchmark completes
            drop(temp_dir);
        });
    }

    group.finish();
}

/// Benchmark fsync overhead
///
/// Measures the impact of durable writes (fsync) on append latency.
/// This helps understand the cost of durability guarantees.
fn bench_fsync_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("raft_fsync");

    let entry_size = 64 * 1024; // 64KB

    group.bench_function("with_fsync", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for iter_idx in 0..iters {
                let temp_dir = TempDir::new().unwrap();
                let log_path = temp_dir.path().join("raft.db");
                let log_store = TestLogStore::open(&log_path).unwrap();

                let entry = create_log_entry(iter_idx + 1, entry_size);

                let start = Instant::now();

                // append_entry() includes fsync via commit
                log_store.append_entry(&entry).unwrap();

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

criterion_group!(
    raft_log_benches,
    bench_single_append_latency,
    bench_batch_append_latency,
    bench_serialization_latency,
    bench_read_latency,
    bench_fsync_overhead,
);

criterion_main!(raft_log_benches);
