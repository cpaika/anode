// Parquet-specific benchmarks
//
// These benchmarks measure Parquet-aware features including:
// - Metadata parsing performance
// - Cache hit/miss performance
// - Footer reading optimization
// - Statistics extraction

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark Parquet metadata parsing
///
/// Measures the performance of parsing Parquet file footers and extracting metadata.
fn bench_parquet_metadata_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parquet_metadata_parse");

    for rows in [1000, 10000, 100000].iter() {
        let parquet_data = generate_parquet_file(*rows);

        group.throughput(Throughput::Elements(*rows as u64));
        group.bench_with_input(
            BenchmarkId::new("parse_footer", rows),
            rows,
            |b, _| {
                b.iter(|| {
                    // Mock Parquet footer parsing
                    let metadata = parse_parquet_metadata(&parquet_data);
                    metadata
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Parquet cache hit performance
///
/// Measures latency when metadata is already cached.
fn bench_parquet_cache_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("parquet_cache_hit");

    // Pre-populate cache
    let mut cache = ParquetMetadataCache::new(1000);
    for i in 0..100 {
        let key = format!("bucket/file-{}.parquet", i);
        let data = generate_parquet_file(10000);
        let metadata = parse_parquet_metadata(&data);
        cache.insert(key, metadata);
    }

    group.bench_function("cache_hit", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Access cached metadata
            let key = format!("bucket/file-{}.parquet", idx % 100);
            let _metadata = cache.get(&key);
            idx += 1;
        });
    });

    group.finish();
}

/// Benchmark Parquet cache miss performance
///
/// Measures latency when metadata must be parsed from storage.
fn bench_parquet_cache_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("parquet_cache_miss");

    let parquet_data = generate_parquet_file(10000);
    let mut cache = ParquetMetadataCache::new(1000);

    group.bench_function("cache_miss", |b| {
        let mut idx = 0;
        b.iter(|| {
            // Cache miss - must parse
            let key = format!("bucket/file-{}.parquet", idx);

            if cache.get(&key).is_none() {
                let metadata = parse_parquet_metadata(&parquet_data);
                cache.insert(key, metadata);
            }

            idx += 1;
        });
    });

    group.finish();
}

/// Benchmark Parquet footer reading
///
/// Measures performance of reading just the footer for metadata extraction.
fn bench_parquet_footer_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("parquet_footer_read");

    for file_size in [1_000_000, 10_000_000, 100_000_000].iter() {
        let rows = file_size / 100;
        let parquet_data = generate_parquet_file(rows);

        group.throughput(Throughput::Bytes(*file_size as u64));
        group.bench_with_input(
            BenchmarkId::new("footer_read", format!("{}mb", file_size / 1_000_000)),
            file_size,
            |b, _| {
                b.iter(|| {
                    // Mock footer reading (last few KB of file)
                    let footer_size = 4096.min(parquet_data.len());
                    let footer = &parquet_data[parquet_data.len() - footer_size..];
                    let _metadata = parse_footer(footer);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark row group metadata extraction
///
/// Measures performance of extracting row group information.
fn bench_row_group_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("row_group_extraction");

    for num_row_groups in [10, 100, 1000].iter() {
        let metadata = ParquetMetadata {
            num_rows: num_row_groups * 10000,
            row_groups: (0..*num_row_groups)
                .map(|i| RowGroupMetadata {
                    id: i,
                    num_rows: 10000,
                    total_byte_size: 1024 * 1024,
                    columns: vec![],
                })
                .collect(),
            schema: vec![],
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(num_row_groups),
            num_row_groups,
            |b, _| {
                b.iter(|| {
                    // Extract row group info
                    let mut total_rows = 0;
                    let mut total_bytes = 0;

                    for rg in &metadata.row_groups {
                        total_rows += rg.num_rows;
                        total_bytes += rg.total_byte_size;
                    }

                    (total_rows, total_bytes)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark column statistics extraction
///
/// Measures performance of extracting column-level statistics for query optimization.
fn bench_column_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("column_statistics");

    let num_columns = 100;
    let row_groups = (0..10)
        .map(|i| RowGroupMetadata {
            id: i,
            num_rows: 10000,
            total_byte_size: 1024 * 1024,
            columns: (0..num_columns)
                .map(|col| ColumnMetadata {
                    name: format!("col_{}", col),
                    min_value: Some(vec![0u8; 8]),
                    max_value: Some(vec![255u8; 8]),
                    null_count: 0,
                })
                .collect(),
        })
        .collect();

    let metadata = ParquetMetadata {
        num_rows: 100000,
        row_groups,
        schema: vec![],
    };

    group.bench_function("extract_statistics", |b| {
        b.iter(|| {
            // Extract column statistics across all row groups
            let mut column_stats: HashMap<String, ColumnStats> = HashMap::new();

            for rg in &metadata.row_groups {
                for col in &rg.columns {
                    let stats = column_stats.entry(col.name.clone()).or_insert(ColumnStats {
                        min: col.min_value.clone(),
                        max: col.max_value.clone(),
                        total_null_count: 0,
                    });

                    stats.total_null_count += col.null_count;
                }
            }

            column_stats
        });
    });

    group.finish();
}

/// Benchmark Parquet schema parsing
///
/// Measures performance of parsing and caching schema information.
fn bench_schema_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema_parsing");

    for num_columns in [10, 100, 1000].iter() {
        let schema_data = generate_schema(*num_columns);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_columns),
            num_columns,
            |b, _| {
                b.iter(|| {
                    // Mock schema parsing
                    let schema = parse_schema(&schema_data);
                    schema
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache eviction performance
///
/// Measures LRU cache eviction overhead when cache is full.
fn bench_cache_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_eviction");

    let cache_size = 100;
    let mut cache = ParquetMetadataCache::new(cache_size);

    // Pre-fill cache
    for i in 0..cache_size {
        let key = format!("bucket/file-{}.parquet", i);
        let data = generate_parquet_file(1000);
        let metadata = parse_parquet_metadata(&data);
        cache.insert(key, metadata);
    }

    group.bench_function("evict_and_insert", |b| {
        let mut idx = cache_size;
        b.iter(|| {
            // Insert new item, triggering eviction
            let key = format!("bucket/file-{}.parquet", idx);
            let data = generate_parquet_file(1000);
            let metadata = parse_parquet_metadata(&data);
            cache.insert(key, metadata);
            idx += 1;
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

#[derive(Clone)]
struct ColumnStats {
    min: Option<Vec<u8>>,
    max: Option<Vec<u8>>,
    total_null_count: usize,
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

fn parse_parquet_metadata(data: &[u8]) -> ParquetMetadata {
    // Mock parsing - in reality would use parquet crate
    let footer = parse_footer(&data[data.len() - 100.min(data.len())..]);
    footer
}

fn parse_footer(_footer_data: &[u8]) -> ParquetMetadata {
    // Mock footer parsing
    ParquetMetadata {
        num_rows: 10000,
        row_groups: vec![RowGroupMetadata {
            id: 0,
            num_rows: 10000,
            total_byte_size: 1024 * 1024,
            columns: vec![],
        }],
        schema: vec![],
    }
}

fn generate_schema(num_columns: usize) -> Vec<u8> {
    // Mock schema generation
    let mut schema = Vec::new();
    for i in 0..num_columns {
        let col_def = format!("column_{}: int64\n", i);
        schema.extend_from_slice(col_def.as_bytes());
    }
    schema
}

fn parse_schema(schema_data: &[u8]) -> Vec<String> {
    // Mock schema parsing
    let schema_str = String::from_utf8_lossy(schema_data);
    schema_str.lines().map(|s| s.to_string()).collect()
}

criterion_group!(
    parquet_benches,
    bench_parquet_metadata_parse,
    bench_parquet_cache_hit,
    bench_parquet_cache_miss,
    bench_parquet_footer_read,
    bench_row_group_extraction,
    bench_column_statistics,
    bench_schema_parsing,
    bench_cache_eviction,
);

criterion_main!(parquet_benches);
