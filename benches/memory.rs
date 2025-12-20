// Memory efficiency benchmarks
//
// These benchmarks measure memory usage patterns including:
// - Memory per object
// - Memory per connection
// - Cache efficiency
// - Allocation rates

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark memory usage per object
///
/// Measures memory overhead for storing object metadata.
fn bench_memory_per_object(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_per_object");

    for count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            count,
            |b, &count| {
                b.iter(|| {
                    let (objects, stats) = measure_cpu_memory(|| {
                        let mut objects = Vec::new();
                        for i in 0..count {
                            let obj = ObjectEntry {
                                key: format!("bucket/object-{:05}", i),
                                size: 1024 * 1024,
                                chunk_ids: vec![[i as u8; 32]],
                                metadata: HashMap::new(),
                            };
                            objects.push(obj);
                        }
                        objects
                    });

                    let memory_per_object = (stats.end_memory - stats.start_memory) / count as u64;
                    let _avg = memory_per_object;

                    objects
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage per connection
///
/// Measures memory overhead for client connections.
fn bench_memory_per_connection(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_per_connection");

    for connections in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*connections as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(connections),
            connections,
            |b, &connections| {
                b.iter(|| {
                    let (conns, stats) = measure_cpu_memory(|| {
                        let mut conns = Vec::new();
                        for i in 0..connections {
                            let conn = Connection {
                                id: i,
                                address: format!("192.168.1.{}", i % 255),
                                buffer: vec![0u8; 8192],
                                state: ConnectionState::Active,
                            };
                            conns.push(conn);
                        }
                        conns
                    });

                    let memory_per_conn = (stats.end_memory - stats.start_memory) / connections as u64;
                    let _avg = memory_per_conn;

                    conns
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache efficiency
///
/// Measures memory efficiency of LRU cache with different sizes and hit rates.
fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    for cache_size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("lru_cache", cache_size),
            cache_size,
            |b, &cache_size| {
                b.iter(|| {
                    let (cache, stats) = measure_cpu_memory(|| {
                        let mut cache = LruCache::new(cache_size);

                        // Fill cache
                        for i in 0..cache_size {
                            let key = format!("key-{}", i);
                            let value = generate_random_data(4096);
                            cache.insert(key, value);
                        }

                        // Simulate access pattern (80% hit rate)
                        for i in 0..1000 {
                            let key = if i % 5 == 0 {
                                // 20% miss - new key
                                format!("key-{}", cache_size + i)
                            } else {
                                // 80% hit - existing key
                                format!("key-{}", i % cache_size)
                            };

                            if i % 5 == 0 {
                                let value = generate_random_data(4096);
                                cache.insert(key, value);
                            } else {
                                let _value = cache.get(&key);
                            }
                        }

                        cache
                    });

                    let memory_per_entry = stats.peak_memory / cache_size as u64;
                    let _avg = memory_per_entry;

                    cache
                });
            },
        );
    }

    group.finish();
}

/// Benchmark allocation rate
///
/// Measures allocation pressure under different workloads.
fn bench_allocation_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_rate");

    group.bench_function("high_alloc_workload", |b| {
        b.iter(|| {
            // Simulate high allocation workload
            let mut allocations = Vec::new();

            for i in 0..1000 {
                // Create temporary objects
                let key = format!("object-{}", i);
                let data = generate_random_data(1024);
                let hash = calculate_hash(&data);

                allocations.push((key, data, hash));
            }

            allocations
        });
    });

    group.bench_function("low_alloc_workload", |b| {
        // Pre-allocate buffers
        let mut buffer = vec![0u8; 1024];

        b.iter(|| {
            // Simulate low allocation workload (buffer reuse)
            let mut results = Vec::with_capacity(1000);

            for i in 0..1000 {
                // Reuse buffer
                buffer.fill((i % 256) as u8);
                let hash = calculate_hash(&buffer);
                results.push(hash);
            }

            results
        });
    });

    group.finish();
}

/// Benchmark memory fragmentation
///
/// Measures impact of allocation patterns on memory fragmentation.
fn bench_memory_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_fragmentation");

    group.bench_function("sequential_alloc", |b| {
        b.iter(|| {
            let (_data, _stats) = measure_cpu_memory(|| {
                // Sequential allocations of same size
                let mut data = Vec::new();
                for _ in 0..1000 {
                    data.push(vec![0u8; 4096]);
                }
                data
            });
        });
    });

    group.bench_function("varied_alloc", |b| {
        b.iter(|| {
            let (_data, _stats) = measure_cpu_memory(|| {
                // Varied size allocations (more fragmentation)
                let mut data = Vec::new();
                for i in 0..1000 {
                    let size = (i % 10 + 1) * 1024;
                    data.push(vec![0u8; size]);
                }
                data
            });
        });
    });

    group.finish();
}

/// Benchmark chunk metadata memory overhead
///
/// Measures memory used for content-addressable chunk tracking.
fn bench_chunk_metadata_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_metadata_overhead");

    for num_chunks in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_chunks),
            num_chunks,
            |b, &num_chunks| {
                b.iter(|| {
                    let (chunks, stats) = measure_cpu_memory(|| {
                        let mut chunks = HashMap::new();

                        for i in 0..num_chunks {
                            let mut hash = [0u8; 32];
                            hash[0..8].copy_from_slice(&(i as u64).to_le_bytes());

                            let metadata = ChunkMetadata {
                                hash,
                                size: 4096,
                                ref_count: 1,
                                storage_path: format!("/data/chunks/{:02x}/{:02x}/{}", hash[0], hash[1], i),
                            };

                            chunks.insert(hash, metadata);
                        }

                        chunks
                    });

                    let memory_per_chunk = stats.peak_memory / num_chunks as u64;
                    let _avg = memory_per_chunk;

                    chunks
                });
            },
        );
    }

    group.finish();
}

// Helper structures

#[derive(Clone)]
struct ObjectEntry {
    key: String,
    size: usize,
    chunk_ids: Vec<[u8; 32]>,
    metadata: HashMap<String, String>,
}

#[derive(Clone)]
struct Connection {
    id: usize,
    address: String,
    buffer: Vec<u8>,
    state: ConnectionState,
}

#[derive(Clone)]
enum ConnectionState {
    Active,
    Idle,
    Closed,
}

struct LruCache {
    capacity: usize,
    cache: HashMap<String, Vec<u8>>,
}

impl LruCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
        }
    }

    fn insert(&mut self, key: String, value: Vec<u8>) {
        if self.cache.len() >= self.capacity {
            // Simple eviction: remove first entry (not true LRU, but good enough for bench)
            if let Some(first_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(key, value);
    }

    fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.cache.get(key)
    }
}

#[derive(Clone)]
struct ChunkMetadata {
    hash: [u8; 32],
    size: usize,
    ref_count: usize,
    storage_path: String,
}

fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut sum: u64 = 0;
    for &byte in data {
        sum = sum.wrapping_add(byte as u64);
    }
    hash[0..8].copy_from_slice(&sum.to_le_bytes());
    hash
}

criterion_group!(
    memory_benches,
    bench_memory_per_object,
    bench_memory_per_connection,
    bench_cache_efficiency,
    bench_allocation_rate,
    bench_memory_fragmentation,
    bench_chunk_metadata_overhead,
);

criterion_main!(memory_benches);
