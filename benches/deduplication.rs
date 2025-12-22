// Chunk deduplication ratio benchmark
//
// This benchmark measures the effectiveness of chunk-level deduplication
// by uploading data with varying degrees of overlap and measuring the
// deduplication ratio achieved.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Generate data with a specified duplication ratio
///
/// Creates data blocks where some percentage of chunks are duplicates.
///
/// # Arguments
/// * `total_size` - Total size of data to generate in bytes
/// * `chunk_size` - Size of each chunk in bytes
/// * `duplication_ratio` - Fraction of chunks that should be duplicates (0.0 to 1.0)
fn generate_data_with_duplication(
    total_size: usize,
    chunk_size: usize,
    duplication_ratio: f64,
) -> Vec<u8> {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(42);
    let num_chunks = (total_size + chunk_size - 1) / chunk_size;
    let num_unique = ((num_chunks as f64) * (1.0 - duplication_ratio)).ceil() as usize;
    let num_unique = num_unique.max(1); // At least one unique chunk

    // Generate unique chunks
    let mut unique_chunks: Vec<Vec<u8>> = Vec::new();
    for _ in 0..num_unique {
        let mut chunk = vec![0u8; chunk_size];
        rng.fill(&mut chunk[..]);
        unique_chunks.push(chunk);
    }

    // Build the full data by repeating chunks
    let mut data = Vec::with_capacity(total_size);
    for i in 0..num_chunks {
        let chunk_idx = i % num_unique;
        let chunk = &unique_chunks[chunk_idx];
        let bytes_to_add = chunk_size.min(total_size - data.len());
        data.extend_from_slice(&chunk[..bytes_to_add]);
    }

    data
}

/// Calculate hash of a chunk (simulates content-addressable storage)
fn hash_chunk(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Simulate chunk deduplication and return statistics
///
/// Returns (unique_chunks, total_chunks, unique_bytes, total_bytes)
fn simulate_deduplication(data: &[u8], chunk_size: usize) -> (usize, usize, usize, usize) {
    use std::collections::HashSet;

    let mut unique_hashes = HashSet::new();
    let mut total_chunks = 0;
    let mut unique_bytes = 0;
    let total_bytes = data.len();

    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + chunk_size).min(data.len());
        let chunk = &data[offset..end];
        let hash = hash_chunk(chunk);

        if unique_hashes.insert(hash) {
            unique_bytes += chunk.len();
        }

        total_chunks += 1;
        offset = end;
    }

    let unique_chunks = unique_hashes.len();
    (unique_chunks, total_chunks, unique_bytes, total_bytes)
}

/// Benchmark deduplication with zero duplication (all unique data)
fn bench_dedup_no_duplication(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_no_duplication");

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let total_size = 100 * 1024 * 1024; // 100MB total
    let data = generate_data_with_duplication(total_size, chunk_size, 0.0);

    group.throughput(Throughput::Bytes(total_size as u64));
    group.bench_function("100mb_0pct_dup", |b| {
        b.iter(|| {
            let (unique_chunks, total_chunks, unique_bytes, total_bytes) =
                simulate_deduplication(&data, chunk_size);

            // Should have minimal deduplication
            assert_eq!(unique_chunks, total_chunks);
            assert_eq!(unique_bytes, total_bytes);

            (unique_chunks, total_chunks, unique_bytes, total_bytes)
        });
    });

    group.finish();
}

/// Benchmark deduplication with 50% duplication
fn bench_dedup_medium_duplication(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_medium_duplication");

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let total_size = 100 * 1024 * 1024; // 100MB total
    let data = generate_data_with_duplication(total_size, chunk_size, 0.5);

    group.throughput(Throughput::Bytes(total_size as u64));
    group.bench_function("100mb_50pct_dup", |b| {
        b.iter(|| {
            let (unique_chunks, total_chunks, unique_bytes, total_bytes) =
                simulate_deduplication(&data, chunk_size);

            // Should achieve ~50% deduplication
            let dedup_ratio = 1.0 - (unique_bytes as f64 / total_bytes as f64);
            assert!(dedup_ratio > 0.4 && dedup_ratio < 0.6);

            (unique_chunks, total_chunks, unique_bytes, total_bytes)
        });
    });

    group.finish();
}

/// Benchmark deduplication with 75% duplication
fn bench_dedup_high_duplication(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_high_duplication");

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let total_size = 100 * 1024 * 1024; // 100MB total
    let data = generate_data_with_duplication(total_size, chunk_size, 0.75);

    group.throughput(Throughput::Bytes(total_size as u64));
    group.bench_function("100mb_75pct_dup", |b| {
        b.iter(|| {
            let (unique_chunks, total_chunks, unique_bytes, total_bytes) =
                simulate_deduplication(&data, chunk_size);

            // Should achieve ~75% deduplication
            let dedup_ratio = 1.0 - (unique_bytes as f64 / total_bytes as f64);
            assert!(dedup_ratio > 0.65 && dedup_ratio < 0.85);

            (unique_chunks, total_chunks, unique_bytes, total_bytes)
        });
    });

    group.finish();
}

/// Benchmark deduplication with 95% duplication (very high)
fn bench_dedup_very_high_duplication(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_very_high_duplication");

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let total_size = 100 * 1024 * 1024; // 100MB total
    let data = generate_data_with_duplication(total_size, chunk_size, 0.95);

    group.throughput(Throughput::Bytes(total_size as u64));
    group.bench_function("100mb_95pct_dup", |b| {
        b.iter(|| {
            let (unique_chunks, total_chunks, unique_bytes, total_bytes) =
                simulate_deduplication(&data, chunk_size);

            // Should achieve ~95% deduplication
            let dedup_ratio = 1.0 - (unique_bytes as f64 / total_bytes as f64);
            assert!(dedup_ratio > 0.90);

            (unique_chunks, total_chunks, unique_bytes, total_bytes)
        });
    });

    group.finish();
}

/// Benchmark deduplication ratio across different chunk sizes
fn bench_dedup_varying_chunk_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_varying_chunk_sizes");

    let total_size = 64 * 1024 * 1024; // 64MB total
    let duplication_ratio = 0.5; // 50% duplication

    for chunk_size in [64 * 1024, 256 * 1024, 1024 * 1024, 4 * 1024 * 1024].iter() {
        let data = generate_data_with_duplication(total_size, *chunk_size, duplication_ratio);

        group.throughput(Throughput::Bytes(total_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}kb", chunk_size / 1024)),
            chunk_size,
            |b, &chunk_size| {
                b.iter(|| simulate_deduplication(&data, chunk_size));
            },
        );
    }

    group.finish();
}

/// Benchmark deduplication with varying data sizes
fn bench_dedup_varying_data_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_varying_data_sizes");
    group.sample_size(10); // Reduce sample size for large operations
    group.measurement_time(Duration::from_secs(20));

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let duplication_ratio = 0.5; // 50% duplication

    for size_mb in [10, 50, 100, 500].iter() {
        let total_size = size_mb * 1024 * 1024;
        let data = generate_data_with_duplication(total_size, chunk_size, duplication_ratio);

        group.throughput(Throughput::Bytes(total_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}mb", size_mb)),
            &total_size,
            |b, _| {
                b.iter(|| simulate_deduplication(&data, chunk_size));
            },
        );
    }

    group.finish();
}

/// Benchmark the space savings from deduplication
fn bench_dedup_space_savings(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_space_savings");

    let chunk_size = 4 * 1024 * 1024; // 4MB chunks
    let total_size = 100 * 1024 * 1024; // 100MB total

    // Test different duplication ratios
    for dup_pct in [0, 25, 50, 75, 90].iter() {
        let dup_ratio = (*dup_pct as f64) / 100.0;
        let data = generate_data_with_duplication(total_size, chunk_size, dup_ratio);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pct_dup", dup_pct)),
            &dup_ratio,
            |b, _| {
                b.iter(|| {
                    let (unique_chunks, total_chunks, unique_bytes, total_bytes) =
                        simulate_deduplication(&data, chunk_size);

                    // Calculate space savings
                    let space_saved = total_bytes - unique_bytes;
                    let savings_ratio = space_saved as f64 / total_bytes as f64;

                    (
                        unique_chunks,
                        total_chunks,
                        unique_bytes,
                        total_bytes,
                        savings_ratio,
                    )
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    dedup_benches,
    bench_dedup_no_duplication,
    bench_dedup_medium_duplication,
    bench_dedup_high_duplication,
    bench_dedup_very_high_duplication,
    bench_dedup_varying_chunk_sizes,
    bench_dedup_varying_data_sizes,
    bench_dedup_space_savings,
);

criterion_main!(dedup_benches);
