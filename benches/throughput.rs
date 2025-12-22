// System throughput benchmarks
//
// These benchmarks measure end-to-end system performance including:
// - Requests per second
// - Megabytes per second
// - Concurrent client handling
// - Mixed workload performance

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark PUT operation throughput
///
/// Measures how many PUT requests the system can handle per second.
fn bench_put_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_throughput");

    for size in [1024, 64 * 1024, 1024 * 1024].iter() {
        let data = generate_random_data(*size);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("put", format!("{}kb", size / 1024)),
            size,
            |b, _| {
                b.iter(|| {
                    // Mock PUT operation
                    // In real implementation: client.put_object("bucket", "key", &data)
                    let _hash = calculate_hash(&data);
                    let _metadata = ObjectMetadata::new("key", data.len());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark PUT 1KB object throughput (ops/sec)
///
/// Dedicated benchmark measuring PUT operations per second for 1KB objects.
/// This is a common use case for small file uploads and metadata storage.
fn bench_put_1kb_ops_per_sec(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_1kb_ops_per_sec");
    let data = generate_random_data(1024);

    // Configure to measure operations per second
    group.throughput(Throughput::Elements(1));

    group.bench_function("1kb_put_ops", |b| {
        let mut counter = 0;
        b.iter(|| {
            // Mock PUT operation for 1KB object
            // In real implementation: client.put_object("bucket", format!("key-{}", counter), &data)
            let key = format!("object-{}", counter);
            let _hash = calculate_hash(&data);
            let _metadata = ObjectMetadata::new(&key, data.len());
            counter += 1;
        });
    });

    group.finish();
}

/// Benchmark PUT 1MB object throughput (MB/sec)
///
/// Dedicated benchmark measuring megabytes per second for 1MB object PUTs.
/// This is a common use case for large file uploads and assessing throughput capacity.
fn bench_put_1mb_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_1mb_throughput");
    let data = generate_random_data(1024 * 1024); // 1MB

    // Configure to measure bytes per second (MB/sec)
    group.throughput(Throughput::Bytes(1024 * 1024));

    group.bench_function("1mb_put_mbps", |b| {
        let mut counter = 0;
        b.iter(|| {
            // Mock PUT operation for 1MB object
            // In real implementation: client.put_object("bucket", format!("key-{}", counter), &data)
            let key = format!("object-{}", counter);
            let _hash = calculate_hash(&data);
            let _metadata = ObjectMetadata::new(&key, data.len());
            counter += 1;
        });
    });

    group.finish();
}

/// Benchmark PUT 100MB object throughput (MB/sec)
///
/// Dedicated benchmark measuring megabytes per second for 100MB object PUTs.
/// This is a common use case for very large file uploads and assessing maximum throughput capacity.
fn bench_put_100mb_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_100mb_throughput");
    let data = generate_random_data(100 * 1024 * 1024); // 100MB

    // Configure to measure bytes per second (MB/sec)
    group.throughput(Throughput::Bytes(100 * 1024 * 1024));

    group.bench_function("100mb_put_mbps", |b| {
        let mut counter = 0;
        b.iter(|| {
            // Mock PUT operation for 100MB object
            // In real implementation: client.put_object("bucket", format!("key-{}", counter), &data)
            let key = format!("object-{}", counter);
            let _hash = calculate_hash(&data);
            let _metadata = ObjectMetadata::new(&key, data.len());
            counter += 1;
        });
    });

    group.finish();
}

/// Benchmark GET operation throughput
///
/// Measures how many GET requests the system can handle per second.
fn bench_get_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_throughput");

    // Pre-populate mock objects
    let objects: Vec<_> = (0..100)
        .map(|i| {
            let size = 1024 * 1024;
            (format!("object-{}", i), generate_random_data(size))
        })
        .collect();

    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("get_1mb", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Mock GET operation
            idx = (idx + 1) % objects.len();
            let _data = &objects[idx].1;
        });
    });

    group.finish();
}

/// Benchmark GET 1KB object throughput (ops/sec)
///
/// Dedicated benchmark measuring GET operations per second for 1KB objects.
/// This is a common use case for small file reads and metadata retrieval.
fn bench_get_1kb_ops_per_sec(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_1kb_ops_per_sec");

    // Pre-populate mock 1KB objects
    let objects: Vec<_> = (0..100)
        .map(|i| (format!("object-{}", i), generate_random_data(1024)))
        .collect();

    // Configure to measure operations per second
    group.throughput(Throughput::Elements(1));

    group.bench_function("1kb_get_ops", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Mock GET operation for 1KB object
            // In real implementation: client.get_object("bucket", format!("key-{}", idx))
            idx = (idx + 1) % objects.len();
            let _data = &objects[idx].1;
        });
    });

    group.finish();
}

/// Benchmark GET 1MB object throughput (MB/sec)
///
/// Dedicated benchmark measuring megabytes per second for 1MB object GETs.
/// This is a common use case for large file downloads and assessing throughput capacity.
fn bench_get_1mb_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_1mb_throughput");

    // Pre-populate mock 1MB objects
    let objects: Vec<_> = (0..100)
        .map(|i| (format!("object-{}", i), generate_random_data(1024 * 1024)))
        .collect();

    // Configure to measure bytes per second (MB/sec)
    group.throughput(Throughput::Bytes(1024 * 1024));

    group.bench_function("1mb_get_mbps", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Mock GET operation for 1MB object
            // In real implementation: client.get_object("bucket", format!("key-{}", idx))
            idx = (idx + 1) % objects.len();
            let _data = &objects[idx].1;
        });
    });

    group.finish();
}

/// Benchmark GET 100MB object throughput (MB/sec)
///
/// Dedicated benchmark measuring megabytes per second for 100MB object GETs.
/// This is a common use case for very large file downloads and assessing maximum throughput capacity.
fn bench_get_100mb_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_100mb_throughput");

    // Pre-populate mock 100MB objects
    let objects: Vec<_> = (0..10)
        .map(|i| {
            (
                format!("object-{}", i),
                generate_random_data(100 * 1024 * 1024),
            )
        })
        .collect();

    // Configure to measure bytes per second (MB/sec)
    group.throughput(Throughput::Bytes(100 * 1024 * 1024));

    group.bench_function("100mb_get_mbps", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Mock GET operation for 100MB object
            // In real implementation: client.get_object("bucket", format!("key-{}", idx))
            idx = (idx + 1) % objects.len();
            let _data = &objects[idx].1;
        });
    });

    group.finish();
}

/// Benchmark mixed workload (reads and writes)
///
/// Simulates realistic workload with both reads and writes.
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    let config = WorkloadConfig {
        operations: 100,
        object_size: 64 * 1024,
        concurrency: 1,
        read_ratio: 0.7, // 70% reads, 30% writes
    };

    let workload = generate_workload(&config);
    let mut storage: Vec<(String, Vec<u8>)> = Vec::new();

    // Pre-populate some data
    for i in 0..50 {
        storage.push((
            format!("object-{}", i),
            generate_random_data(config.object_size),
        ));
    }

    group.throughput(Throughput::Elements(workload.len() as u64));
    group.bench_function("70r_30w", |b| {
        let mut write_idx = 50;
        let mut read_idx = 0;

        b.iter(|| {
            for op in &workload {
                match op {
                    Operation::Read => {
                        if !storage.is_empty() {
                            read_idx = (read_idx + 1) % storage.len();
                            let _data = &storage[read_idx];
                        }
                    },
                    Operation::Write => {
                        let key = format!("object-{}", write_idx);
                        let data = generate_random_data(config.object_size);
                        storage.push((key, data));
                        write_idx += 1;
                    },
                    _ => {},
                }
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent client handling
///
/// Measures system performance under concurrent client load.
fn bench_concurrent_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_clients");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    for clients in [1, 4, 16, 64].iter() {
        let data = generate_random_data(64 * 1024);

        group.bench_with_input(
            BenchmarkId::from_parameter(clients),
            clients,
            |b, &clients| {
                b.iter(|| {
                    // Simulate concurrent clients
                    // In real implementation: spawn multiple tokio tasks
                    for i in 0..clients {
                        let key = format!("client-{}/object", i);
                        let _hash = calculate_hash(&data);
                        let _metadata = ObjectMetadata::new(&key, data.len());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel multipart uploads
///
/// Measures throughput of parallel multipart upload operations.
fn bench_parallel_uploads(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_uploads");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    let part_size = 5 * 1024 * 1024; // 5MB parts
    let num_parts = 10;

    let parts: Vec<_> = (0..num_parts)
        .map(|_| generate_random_data(part_size))
        .collect();

    group.throughput(Throughput::Bytes((part_size * num_parts) as u64));
    group.bench_function("10_parts_5mb", |b| {
        b.iter(|| {
            // Mock multipart upload
            let mut part_hashes = Vec::new();
            for part in &parts {
                let hash = calculate_hash(part);
                part_hashes.push(hash);
            }
            // Mock completion
            let _final_hash = calculate_hash(&part_hashes.concat());
        });
    });

    group.finish();
}

/// Benchmark sustained throughput over time
///
/// Measures if the system can maintain consistent throughput.
fn bench_sustained_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_throughput");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    let data = generate_random_data(1024 * 1024);
    let operations = 100;

    group.throughput(Throughput::Bytes((data.len() * operations) as u64));
    group.bench_function("100_ops_1mb", |b| {
        b.iter(|| {
            for i in 0..operations {
                let key = format!("sustained-{}", i);
                let _hash = calculate_hash(&data);
                let _metadata = ObjectMetadata::new(&key, data.len());
            }
        });
    });

    group.finish();
}

// Helper structures
#[derive(Clone)]
struct ObjectMetadata {
    key: String,
    size: usize,
    hash: [u8; 32],
}

impl ObjectMetadata {
    fn new(key: &str, size: usize) -> Self {
        Self {
            key: key.to_string(),
            size,
            hash: [0u8; 32],
        }
    }
}

fn calculate_hash(data: &[u8]) -> [u8; 32] {
    // Simplified hash for benchmarking
    let mut hash = [0u8; 32];
    let mut sum: u64 = 0;
    for &byte in data {
        sum = sum.wrapping_add(byte as u64);
    }
    hash[0..8].copy_from_slice(&sum.to_le_bytes());
    hash
}

criterion_group!(
    throughput_benches,
    bench_put_throughput,
    bench_put_1kb_ops_per_sec,
    bench_put_1mb_throughput,
    bench_put_100mb_throughput,
    bench_get_throughput,
    bench_get_1kb_ops_per_sec,
    bench_get_1mb_throughput,
    bench_get_100mb_throughput,
    bench_mixed_workload,
    bench_concurrent_clients,
    bench_parallel_uploads,
    bench_sustained_throughput,
);

criterion_main!(throughput_benches);
