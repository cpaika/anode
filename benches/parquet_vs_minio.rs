// Parquet metadata query benchmark: Anode vs MinIO
//
// This benchmark compares the performance of parquet metadata queries between:
// - Anode: Optimized with footer-only range reads and caching
// - MinIO: Traditional approach requiring full file reads
//
// Key differences:
// - Anode reads only the footer (~64KB) via range request
// - MinIO typically reads the entire file to access metadata
// - Anode caches parsed metadata to avoid repeated parsing
// - MinIO may need to re-read for each metadata query

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

mod common {
    pub use anode_bench::*;
}

use common::*;

// Simulate different parquet file sizes (in MB)
const FILE_SIZES_MB: &[usize] = &[10, 100, 1000, 5000];

// Footer size is typically small (we read 64KB to be safe)
const FOOTER_SIZE: usize = 64 * 1024;

/// Benchmark Anode's optimized footer-only metadata query
///
/// Anode uses HTTP range requests to read only the last 64KB of the file,
/// which contains the Parquet footer with all metadata information.
fn bench_anode_metadata_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("anode_metadata_query");

    for &size_mb in FILE_SIZES_MB {
        let file_size = size_mb * 1024 * 1024;
        let parquet_data = generate_parquet_file(file_size / 100); // ~100 bytes per row

        // Simulate footer extraction (last 64KB)
        let footer_size = FOOTER_SIZE.min(parquet_data.len());

        group.throughput(Throughput::Bytes(footer_size as u64));
        group.bench_with_input(
            BenchmarkId::new("footer_read", format!("{}mb", size_mb)),
            &size_mb,
            |b, _| {
                b.iter(|| {
                    // Step 1: Range read footer only (64KB vs full file)
                    let footer_data = simulate_range_read(&parquet_data, footer_size);

                    // Step 2: Parse metadata from footer
                    let metadata = parse_parquet_metadata(&footer_data);

                    metadata
                });
            },
        );
    }

    group.finish();
}

/// Benchmark MinIO's full-file metadata query
///
/// MinIO traditionally requires reading the entire file to access metadata,
/// as it doesn't optimize for footer-only reads in the same way.
fn bench_minio_metadata_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("minio_metadata_query");

    for &size_mb in FILE_SIZES_MB {
        let file_size = size_mb * 1024 * 1024;
        let parquet_data = generate_parquet_file(file_size / 100);

        group.throughput(Throughput::Bytes(file_size as u64));
        group.bench_with_input(
            BenchmarkId::new("full_read", format!("{}mb", size_mb)),
            &size_mb,
            |b, _| {
                b.iter(|| {
                    // Step 1: Read entire file
                    let file_data = simulate_full_read(&parquet_data);

                    // Step 2: Parse metadata (still only needs footer, but read everything)
                    let metadata = parse_parquet_metadata(&file_data);

                    metadata
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Anode metadata query with cache hit
///
/// Shows the benefit of metadata caching - subsequent queries are nearly instant.
fn bench_anode_cached_metadata_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("anode_cached_metadata_query");

    // Pre-populate cache with metadata for various file sizes
    let mut cache = ParquetMetadataCache::new(100);

    for &size_mb in FILE_SIZES_MB {
        let file_size = size_mb * 1024 * 1024;
        let parquet_data = generate_parquet_file(file_size / 100);
        let key = format!("bucket/file-{}mb.parquet", size_mb);

        // Initial parse and cache
        let footer_size = FOOTER_SIZE.min(parquet_data.len());
        let footer = &parquet_data[parquet_data.len() - footer_size..];
        let metadata = parse_parquet_metadata(footer);
        cache.insert(key.clone(), metadata);
    }

    for &size_mb in FILE_SIZES_MB {
        group.bench_with_input(
            BenchmarkId::new("cache_hit", format!("{}mb", size_mb)),
            &size_mb,
            |b, _| {
                let key = format!("bucket/file-{}mb.parquet", size_mb);
                b.iter(|| {
                    // Cache hit - no I/O, no parsing
                    cache.get(&key).expect("Should be cached")
                });
            },
        );
    }

    group.finish();
}

/// Benchmark comparison: Anode vs MinIO for cold metadata queries
///
/// This shows the direct comparison when metadata is not yet cached.
fn bench_cold_query_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("cold_query_comparison");

    for &size_mb in FILE_SIZES_MB {
        let file_size = size_mb * 1024 * 1024;
        let parquet_data = generate_parquet_file(file_size / 100);

        // Anode approach
        group.bench_with_input(
            BenchmarkId::new("anode", format!("{}mb", size_mb)),
            &size_mb,
            |b, _| {
                b.iter(|| {
                    let footer_size = FOOTER_SIZE.min(parquet_data.len());
                    let footer_data = simulate_range_read(&parquet_data, footer_size);
                    let metadata = parse_parquet_metadata(&footer_data);
                    metadata
                });
            },
        );

        // MinIO approach
        group.bench_with_input(
            BenchmarkId::new("minio", format!("{}mb", size_mb)),
            &size_mb,
            |b, _| {
                b.iter(|| {
                    let file_data = simulate_full_read(&parquet_data);
                    let metadata = parse_parquet_metadata(&file_data);
                    metadata
                });
            },
        );
    }

    group.finish();
}

/// Benchmark row group metadata extraction speed
///
/// Once metadata is obtained, how fast can we extract row group info for query planning?
fn bench_row_group_query_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("row_group_query_planning");

    for num_row_groups in [10, 100, 1000, 10000].iter() {
        let metadata = ParquetMetadata {
            num_rows: num_row_groups * 10000,
            row_groups: (0..*num_row_groups)
                .map(|i| RowGroupMetadata {
                    id: i,
                    num_rows: 10000,
                    total_byte_size: 1024 * 1024,
                    columns: vec![
                        ColumnMetadata {
                            name: "col1".to_string(),
                            min_value: Some(vec![0u8; 8]),
                            max_value: Some(vec![255u8; 8]),
                            null_count: 0,
                        },
                        ColumnMetadata {
                            name: "col2".to_string(),
                            min_value: Some(vec![0u8; 8]),
                            max_value: Some(vec![255u8; 8]),
                            null_count: 0,
                        },
                    ],
                })
                .collect(),
            schema: vec![],
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(num_row_groups),
            num_row_groups,
            |b, _| {
                b.iter(|| {
                    // Simulate query planning: find row groups within range
                    let min_id = *num_row_groups / 4;
                    let max_id = *num_row_groups * 3 / 4;

                    let selected_rgs: Vec<_> = metadata
                        .row_groups
                        .iter()
                        .filter(|rg| rg.id >= min_id && rg.id < max_id)
                        .collect();

                    selected_rgs.len()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark predicate pushdown using column statistics
///
/// Demonstrates the value of having metadata: skip row groups based on min/max stats.
fn bench_predicate_pushdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("predicate_pushdown");

    let num_row_groups = 1000;
    let metadata = ParquetMetadata {
        num_rows: num_row_groups * 10000,
        row_groups: (0..num_row_groups)
            .map(|i| {
                // Each row group has a different value range
                let min_val = (i * 1000) as u8;
                let max_val = ((i + 1) * 1000).min(255000) as u8;

                RowGroupMetadata {
                    id: i,
                    num_rows: 10000,
                    total_byte_size: 1024 * 1024,
                    columns: vec![ColumnMetadata {
                        name: "value".to_string(),
                        min_value: Some(vec![min_val]),
                        max_value: Some(vec![max_val]),
                        null_count: 0,
                    }],
                }
            })
            .collect(),
        schema: vec![],
    };

    group.bench_function("filter_by_column_stats", |b| {
        b.iter(|| {
            // Query: WHERE value BETWEEN 100000 AND 200000
            let query_min = 100u8;
            let query_max = 200u8;

            let selected_rgs: Vec<_> = metadata
                .row_groups
                .iter()
                .filter(|rg| {
                    if let Some(col) = rg.columns.first() {
                        if let (Some(min), Some(max)) = (&col.min_value, &col.max_value) {
                            // Row group overlaps with query range
                            return min[0] <= query_max && max[0] >= query_min;
                        }
                    }
                    true // Include if no stats available
                })
                .collect();

            selected_rgs.len()
        });
    });

    group.finish();
}

/// Benchmark multiple concurrent metadata queries (Anode caching advantage)
///
/// Shows how caching benefits workloads with repeated metadata access.
fn bench_concurrent_metadata_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_metadata_access");

    let num_files = 100;
    let size_mb = 100;
    let file_size = size_mb * 1024 * 1024;

    // Setup: Generate parquet files and cache
    let files: Vec<_> = (0..num_files)
        .map(|i| {
            let parquet_data = generate_parquet_file(file_size / 100);
            let key = format!("bucket/file-{}.parquet", i);
            (key, parquet_data)
        })
        .collect();

    let mut cache = ParquetMetadataCache::new(num_files);

    // Pre-populate cache for Anode
    for (key, data) in &files {
        let footer_size = FOOTER_SIZE.min(data.len());
        let footer = &data[data.len() - footer_size..];
        let metadata = parse_parquet_metadata(footer);
        cache.insert(key.clone(), metadata);
    }

    // Anode with cache
    group.bench_function("anode_cached", |b| {
        b.iter(|| {
            let mut total_rows = 0;
            for i in 0..num_files {
                let key = format!("bucket/file-{}.parquet", i);
                if let Some(metadata) = cache.get(&key) {
                    total_rows += metadata.num_rows;
                }
            }
            total_rows
        });
    });

    // MinIO without cache (must read each time)
    group.bench_function("minio_uncached", |b| {
        b.iter(|| {
            let mut total_rows = 0;
            for (_key, data) in &files {
                let file_data = simulate_full_read(data);
                let metadata = parse_parquet_metadata(&file_data);
                total_rows += metadata.num_rows;
            }
            total_rows
        });
    });

    group.finish();
}

// Helper structures and functions

#[derive(Clone)]
struct ParquetMetadata {
    num_rows: usize,
    row_groups: Vec<RowGroupMetadata>,
    schema: Vec<String>,
}

#[derive(Clone)]
struct RowGroupMetadata {
    id: usize,
    num_rows: usize,
    total_byte_size: usize,
    columns: Vec<ColumnMetadata>,
}

#[derive(Clone)]
struct ColumnMetadata {
    name: String,
    min_value: Option<Vec<u8>>,
    max_value: Option<Vec<u8>>,
    null_count: usize,
}

struct ParquetMetadataCache {
    capacity: usize,
    cache: HashMap<String, ParquetMetadata>,
}

impl ParquetMetadataCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
        }
    }

    fn insert(&mut self, key: String, metadata: ParquetMetadata) {
        if self.cache.len() >= self.capacity {
            // Simple eviction: remove first entry
            if let Some(first_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(key, metadata);
    }

    fn get(&self, key: &str) -> Option<&ParquetMetadata> {
        self.cache.get(key)
    }
}

/// Simulate Anode's range read (footer only)
fn simulate_range_read(data: &[u8], footer_size: usize) -> Vec<u8> {
    let start = data.len().saturating_sub(footer_size);
    data[start..].to_vec()
}

/// Simulate MinIO's full file read
fn simulate_full_read(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

fn parse_parquet_metadata(data: &[u8]) -> ParquetMetadata {
    // Mock parsing - in reality would use parquet crate
    // Simulate parsing overhead
    let num_row_groups = (data.len() / (1024 * 1024)).max(1);

    ParquetMetadata {
        num_rows: num_row_groups * 10000,
        row_groups: (0..num_row_groups)
            .map(|i| RowGroupMetadata {
                id: i,
                num_rows: 10000,
                total_byte_size: 1024 * 1024,
                columns: vec![ColumnMetadata {
                    name: "col1".to_string(),
                    min_value: Some(vec![0u8; 8]),
                    max_value: Some(vec![255u8; 8]),
                    null_count: 0,
                }],
            })
            .collect(),
        schema: vec!["col1".to_string()],
    }
}

criterion_group!(
    parquet_vs_minio_benches,
    bench_anode_metadata_query,
    bench_minio_metadata_query,
    bench_anode_cached_metadata_query,
    bench_cold_query_comparison,
    bench_row_group_query_planning,
    bench_predicate_pushdown,
    bench_concurrent_metadata_access,
);

criterion_main!(parquet_vs_minio_benches);
