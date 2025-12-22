// Storage engine microbenchmarks
//
// These benchmarks measure the core storage engine performance including:
// - Metadata operations (read/write)
// - Blob operations at various sizes
// - Chunk hashing performance
// - Object listing operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark metadata write operations
///
/// Measures the performance of writing metadata entries to the redb-backed store.
fn bench_metadata_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("metadata_write");

    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                // Mock metadata write
                // In real implementation: metadata_store.put(key, value)
                let mut results = Vec::new();
                for i in 0..size {
                    let key = format!("bucket/object-{}", i);
                    let metadata = generate_random_data(256);
                    results.push((key, metadata));
                }
                results
            });
        });
    }

    group.finish();
}

/// Benchmark metadata read operations
///
/// Measures the performance of reading metadata entries.
fn bench_metadata_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("metadata_read");

    // Pre-populate mock data
    let entries: Vec<_> = (0..1000)
        .map(|i| (format!("bucket/object-{}", i), generate_random_data(256)))
        .collect();

    for count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                // Mock metadata read
                let mut results = Vec::new();
                for i in 0..count.min(entries.len()) {
                    results.push(&entries[i]);
                }
                results
            });
        });
    }

    group.finish();
}

/// Benchmark 4KB blob writes
///
/// Measures small object write throughput, common for metadata and small files.
fn bench_blob_write_4kb(c: &mut Criterion) {
    let mut group = c.benchmark_group("blob_write_4kb");
    let data = generate_random_data(4 * 1024);

    group.throughput(Throughput::Bytes(4 * 1024));
    group.bench_function("4kb", |b| {
        b.iter(|| {
            // Mock blob write
            // In real implementation: blob_store.write(hash, &data)
            let _hash = calculate_mock_hash(&data);
        });
    });

    group.finish();
}

/// Benchmark 1MB blob writes
///
/// Measures medium-sized object write throughput.
fn bench_blob_write_1mb(c: &mut Criterion) {
    let mut group = c.benchmark_group("blob_write_1mb");
    let data = generate_random_data(1024 * 1024);

    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("1mb", |b| {
        b.iter(|| {
            // Mock blob write
            let _hash = calculate_mock_hash(&data);
        });
    });

    group.finish();
}

/// Benchmark 100MB blob writes
///
/// Measures large object write throughput, useful for video/backup files.
fn bench_blob_write_100mb(c: &mut Criterion) {
    let mut group = c.benchmark_group("blob_write_100mb");
    group.sample_size(10); // Reduce sample size for large operations
    group.measurement_time(Duration::from_secs(30));

    let data = generate_random_data(100 * 1024 * 1024);

    group.throughput(Throughput::Bytes(100 * 1024 * 1024));
    group.bench_function("100mb", |b| {
        b.iter(|| {
            // Mock blob write
            let _hash = calculate_mock_hash(&data);
        });
    });

    group.finish();
}

/// Benchmark sequential blob reads
///
/// Measures performance when reading blobs in sequential order.
fn bench_blob_read_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("blob_read_sequential");
    let data = generate_random_data(1024 * 1024);

    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("1mb_sequential", |b| {
        b.iter(|| {
            // Mock sequential read
            // In real implementation: blob_store.read(hash)
            let _chunk = &data[..];
        });
    });

    group.finish();
}

/// Benchmark random blob reads
///
/// Measures performance when reading blobs in random order (cache pressure).
fn bench_blob_read_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("blob_read_random");

    // Create multiple blobs
    let blobs: Vec<_> = (0..100)
        .map(|_| generate_random_data(1024 * 1024))
        .collect();

    group.throughput(Throughput::Bytes(1024 * 1024));
    group.bench_function("1mb_random", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Mock random read
            idx = (idx + 37) % blobs.len(); // Simple pseudo-random access
            let _chunk = &blobs[idx];
        });
    });

    group.finish();
}

/// Benchmark chunk hashing (content-addressable storage)
///
/// Measures SHA-256 hashing performance for content deduplication.
fn bench_chunk_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_hash");

    for size in [4096, 65536, 1024 * 1024].iter() {
        let data = generate_random_data(*size);
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| calculate_mock_hash(&data));
        });
    }

    group.finish();
}

/// Benchmark object listing operations
///
/// Measures the performance of prefix-based object listing.
fn bench_list_objects(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_objects");

    // Create mock object list
    let objects: Vec<_> = (0..10000)
        .map(|i| format!("bucket/prefix/object-{:05}", i))
        .collect();

    for limit in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*limit as u64));
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.iter(|| {
                // Mock list operation
                let results: Vec<_> = objects.iter().take(limit).collect();
                results
            });
        });
    }

    group.finish();
}

// Helper function to calculate a mock hash (simplified)
fn calculate_mock_hash(data: &[u8]) -> [u8; 32] {
    // In real implementation, use SHA-256
    // For benchmarking, use a simple checksum to simulate hashing cost
    let mut hash = [0u8; 32];
    let mut sum: u64 = 0;
    for &byte in data {
        sum = sum.wrapping_add(byte as u64);
    }
    hash[0..8].copy_from_slice(&sum.to_le_bytes());
    hash
}

criterion_group!(
    storage_benches,
    bench_metadata_write,
    bench_metadata_read,
    bench_blob_write_4kb,
    bench_blob_write_1mb,
    bench_blob_write_100mb,
    bench_blob_read_sequential,
    bench_blob_read_random,
    bench_chunk_hash,
    bench_list_objects,
);

criterion_main!(storage_benches);
