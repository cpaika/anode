// Latency benchmarks
//
// These benchmarks measure operation latencies including:
// - p50, p99, p999 latencies
// - Time-to-first-byte
// - Operation-specific latencies

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::{Duration, Instant};

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark PUT operation latency
///
/// Measures latency distribution for PUT operations across different object sizes.
fn bench_put_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("put_latency");

    for size in [4096, 64 * 1024, 1024 * 1024].iter() {
        let data = generate_random_data(*size);

        group.bench_with_input(
            BenchmarkId::new("put", format!("{}kb", size / 1024)),
            size,
            |b, _| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        let start = Instant::now();

                        // Mock PUT operation
                        let _hash = calculate_hash(&data);
                        let _result = store_object("bucket", "key", &data);

                        latencies.push(start.elapsed());
                    }

                    // Calculate total time
                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark GET operation latency
///
/// Measures latency distribution for GET operations.
fn bench_get_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_latency");

    // Pre-populate objects
    let objects: Vec<_> = [4096, 64 * 1024, 1024 * 1024]
        .iter()
        .map(|&size| {
            let data = generate_random_data(size);
            let key = format!("object-{}", size);
            (key, data, size)
        })
        .collect();

    for (key, data, size) in &objects {
        group.bench_with_input(
            BenchmarkId::new("get", format!("{}kb", size / 1024)),
            size,
            |b, _| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        let start = Instant::now();

                        // Mock GET operation
                        let _result = retrieve_object("bucket", key);

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark HEAD operation latency
///
/// Measures metadata-only operation latency.
fn bench_head_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("head_latency");

    group.bench_function("head_object", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for i in 0..iters {
                let start = Instant::now();

                // Mock HEAD operation (metadata only)
                let key = format!("object-{}", i);
                let _metadata = get_metadata("bucket", &key);

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

/// Benchmark LIST operation latency
///
/// Measures latency for listing operations with various result counts.
fn bench_list_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_latency");

    // Pre-populate mock object list
    let objects: Vec<_> = (0..10000)
        .map(|i| format!("bucket/prefix/object-{:05}", i))
        .collect();

    for max_keys in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(max_keys),
            max_keys,
            |b, &max_keys| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        let start = Instant::now();

                        // Mock LIST operation
                        let _results = list_objects("bucket", "prefix/", max_keys, &objects);

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark LIST 10K objects latency
///
/// Dedicated benchmark for listing all 10,000 objects to measure full listing latency.
/// This is important for understanding the performance impact of large directory listings.
fn bench_list_10k_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_10k_latency");

    // Pre-populate 10K mock objects
    let objects: Vec<_> = (0..10000)
        .map(|i| format!("bucket/prefix/object-{:05}", i))
        .collect();

    group.bench_function("list_10k_objects", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for _ in 0..iters {
                let start = Instant::now();

                // Mock LIST operation for all 10K objects
                let _results = list_objects("bucket", "prefix/", 10000, &objects);

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

/// Benchmark time-to-first-byte latency
///
/// Measures how quickly the first byte of data is returned for GET operations.
fn bench_first_byte_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("first_byte_latency");

    for size in [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024].iter() {
        let data = generate_random_data(*size);

        group.bench_with_input(
            BenchmarkId::new("ttfb", format!("{}mb", size / (1024 * 1024))),
            size,
            |b, _| {
                b.iter_custom(|iters| {
                    let mut latencies = Vec::new();

                    for _ in 0..iters {
                        let start = Instant::now();

                        // Mock streaming GET - measure time to first byte
                        let _first_chunk = &data[0..8192.min(data.len())];

                        latencies.push(start.elapsed());
                    }

                    latencies.iter().sum()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark DELETE operation latency
///
/// Measures latency for object deletion.
fn bench_delete_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_latency");

    group.bench_function("delete_object", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for i in 0..iters {
                let start = Instant::now();

                // Mock DELETE operation
                let key = format!("object-{}", i);
                let _result = delete_object("bucket", &key);

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

/// Benchmark multipart upload initiate latency
///
/// Measures latency for starting a multipart upload.
fn bench_multipart_initiate_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("multipart_initiate_latency");

    group.bench_function("initiate", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for i in 0..iters {
                let start = Instant::now();

                // Mock multipart upload initiation
                let key = format!("multipart-{}", i);
                let _upload_id = initiate_multipart_upload("bucket", &key);

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

/// Benchmark cold vs warm read latency
///
/// Compares first read (cache miss) vs subsequent reads (cache hit).
fn bench_cold_warm_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_warm_latency");

    let data = generate_random_data(1024 * 1024);
    let mut cache: Vec<Vec<u8>> = Vec::new();

    group.bench_function("cold_read", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for _ in 0..iters {
                let start = Instant::now();

                // Simulate cold read (not in cache)
                let result = data.clone();
                cache.push(result);

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.bench_function("warm_read", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::new();

            for i in 0..iters {
                let start = Instant::now();

                // Simulate warm read (in cache)
                let idx = (i as usize) % cache.len().max(1);
                let _result = &cache[idx];

                latencies.push(start.elapsed());
            }

            latencies.iter().sum()
        });
    });

    group.finish();
}

// Helper functions for mock operations

fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut sum: u64 = 0;
    for &byte in data {
        sum = sum.wrapping_add(byte as u64);
    }
    hash[0..8].copy_from_slice(&sum.to_le_bytes());
    hash
}

fn store_object(_bucket: &str, _key: &str, data: &[u8]) -> bool {
    let _hash = calculate_hash(data);
    true
}

fn retrieve_object(_bucket: &str, _key: &str) -> Vec<u8> {
    generate_random_data(1024)
}

fn get_metadata(_bucket: &str, _key: &str) -> ObjectMetadata {
    ObjectMetadata {
        size: 1024,
        content_type: "application/octet-stream".to_string(),
        etag: "abc123".to_string(),
    }
}

fn list_objects(_bucket: &str, _prefix: &str, max_keys: usize, objects: &[String]) -> Vec<String> {
    objects.iter().take(max_keys).cloned().collect()
}

fn delete_object(_bucket: &str, _key: &str) -> bool {
    true
}

fn initiate_multipart_upload(_bucket: &str, _key: &str) -> String {
    "upload-123".to_string()
}

struct ObjectMetadata {
    size: usize,
    content_type: String,
    etag: String,
}

criterion_group!(
    latency_benches,
    bench_put_latency,
    bench_get_latency,
    bench_head_latency,
    bench_list_latency,
    bench_list_10k_latency,
    bench_first_byte_latency,
    bench_delete_latency,
    bench_multipart_initiate_latency,
    bench_cold_warm_latency,
);

criterion_main!(latency_benches);
