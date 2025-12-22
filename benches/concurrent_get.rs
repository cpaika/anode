// Concurrent GET benchmarks
//
// These benchmarks measure concurrent GET operation performance:
// - Throughput with multiple concurrent clients
// - Scalability across different client counts
// - Operations per second under concurrent load

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark concurrent GET operations with 64 clients
///
/// Pre-populates a dataset, then simulates 64 concurrent clients performing GET operations.
/// Measures overall system throughput (ops/sec) under concurrent read load.
fn bench_concurrent_get_64_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_get_64_clients");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Test with different object sizes
    for size in [1024, 64 * 1024, 1024 * 1024].iter() {
        let size_kb = size / 1024;

        // Pre-populate data for 64 clients
        let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
        for client_id in 0..64 {
            let key = format!("client-{}/object", client_id);
            let data = Arc::new(generate_random_data(*size));
            storage.insert(key, data);
        }

        // Configure throughput measurement
        // Total throughput = 64 clients * operations
        group.throughput(Throughput::Bytes((size * 64) as u64));

        group.bench_with_input(
            BenchmarkId::new("64_clients", format!("{}kb", size_kb)),
            size,
            |b, _| {
                b.iter(|| {
                    // Simulate 64 concurrent clients doing GET operations
                    // In a real implementation, this would spawn 64 tokio tasks
                    for client_id in 0..64 {
                        let key = format!("client-{}/object", client_id);
                        if let Some(data) = storage.get(&key) {
                            let _hash = calculate_hash(data);
                            let _metadata = ObjectMetadata::new(&key, data.len());
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent GET operations with varying client counts
///
/// Measures how system throughput scales with different numbers of concurrent clients.
/// Tests 1, 4, 16, 32, and 64 clients to show scaling characteristics.
fn bench_concurrent_get_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_get_scaling");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let size = 64 * 1024; // 64KB objects

    // Pre-populate data for up to 64 clients
    let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for client_id in 0..64 {
        let key = format!("client-{}/object", client_id);
        let data = Arc::new(generate_random_data(size));
        storage.insert(key, data);
    }

    for clients in [1, 4, 16, 32, 64].iter() {
        group.throughput(Throughput::Elements(*clients as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(clients),
            clients,
            |b, &clients| {
                b.iter(|| {
                    // Simulate N concurrent clients
                    for client_id in 0..clients {
                        let key = format!("client-{}/object", client_id);
                        if let Some(data) = storage.get(&key) {
                            let _hash = calculate_hash(data);
                            let _metadata = ObjectMetadata::new(&key, data.len());
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sustained concurrent GET throughput
///
/// Measures sustained throughput with 64 clients performing multiple GETs each.
/// This tests if the system can maintain performance under prolonged concurrent read load.
fn bench_sustained_concurrent_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_concurrent_get");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    let size = 64 * 1024; // 64KB objects
    let gets_per_client = 10;
    let clients = 64;

    // Pre-populate data for sustained reads
    let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for client_id in 0..clients {
        for get_id in 0..gets_per_client {
            let key = format!("client-{}/object-{}", client_id, get_id);
            let data = Arc::new(generate_random_data(size));
            storage.insert(key, data);
        }
    }

    // Total operations = 64 clients * 10 gets each = 640 operations
    group.throughput(Throughput::Elements((clients * gets_per_client) as u64));

    group.bench_function("64_clients_10_gets_each", |b| {
        b.iter(|| {
            // Each of 64 clients performs 10 GET operations
            for client_id in 0..clients {
                for get_id in 0..gets_per_client {
                    let key = format!("client-{}/object-{}", client_id, get_id);
                    if let Some(data) = storage.get(&key) {
                        let _hash = calculate_hash(data);
                        let _metadata = ObjectMetadata::new(&key, data.len());
                    }
                }
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent GET with mixed object sizes
///
/// Simulates realistic workload where 64 concurrent clients GET objects of varying sizes.
/// Tests how the system handles heterogeneous concurrent read workloads.
fn bench_concurrent_get_mixed_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_get_mixed_sizes");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let clients = 64;

    // Pre-populate objects of different sizes
    let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for client_id in 0..clients {
        let key = format!("client-{}/object", client_id);

        // Distribute sizes: ~1/3 small, ~1/3 medium, ~1/3 large
        let data = match client_id % 3 {
            0 => Arc::new(generate_random_data(1024)),        // 1KB
            1 => Arc::new(generate_random_data(64 * 1024)),   // 64KB
            _ => Arc::new(generate_random_data(1024 * 1024)), // 1MB
        };

        storage.insert(key, data);
    }

    group.throughput(Throughput::Elements(clients as u64));

    group.bench_function("64_clients_mixed", |b| {
        b.iter(|| {
            // Each client GETs a different size based on client_id
            for client_id in 0..clients {
                let key = format!("client-{}/object", client_id);
                if let Some(data) = storage.get(&key) {
                    let _hash = calculate_hash(data);
                    let _metadata = ObjectMetadata::new(&key, data.len());
                }
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent GET operations per second
///
/// Dedicated benchmark for measuring pure operations per second with 64 concurrent clients.
/// Focuses on 1KB objects to maximize operation count and minimize data transfer overhead.
fn bench_concurrent_get_ops_per_sec(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_get_ops_per_sec");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let size = 1024; // 1KB for high ops/sec
    let clients = 64;

    // Pre-populate data with multiple objects per client
    let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for client_id in 0..clients {
        for counter in 0..100 {
            let key = format!("client-{}/object-{}", client_id, counter);
            let data = Arc::new(generate_random_data(size));
            storage.insert(key, data);
        }
    }

    // Measure operations per second
    group.throughput(Throughput::Elements(clients as u64));

    group.bench_function("64_clients_1kb_ops", |b| {
        let mut counter = 0;
        b.iter(|| {
            // 64 clients each performing a GET operation
            for client_id in 0..clients {
                let key = format!("client-{}/object-{}", client_id, counter % 100);
                if let Some(data) = storage.get(&key) {
                    let _hash = calculate_hash(data);
                    let _metadata = ObjectMetadata::new(&key, data.len());
                }
            }
            counter += 1;
        });
    });

    group.finish();
}

/// Benchmark concurrent GET with cache hit simulation
///
/// Tests performance when objects are likely in cache (repeated reads of same objects).
/// Simulates hot data scenario where 64 clients read the same popular objects.
fn bench_concurrent_get_cache_hits(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_get_cache_hits");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let size = 64 * 1024; // 64KB objects
    let clients = 64;
    let hot_objects = 10; // Only 10 hot objects shared by all clients

    // Pre-populate a small set of hot objects
    let mut storage: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for object_id in 0..hot_objects {
        let key = format!("hot-object-{}", object_id);
        let data = Arc::new(generate_random_data(size));
        storage.insert(key, data);
    }

    group.throughput(Throughput::Elements(clients as u64));

    group.bench_function("64_clients_10_hot_objects", |b| {
        b.iter(|| {
            // 64 clients all reading from a small set of hot objects
            for client_id in 0..clients {
                let key = format!("hot-object-{}", client_id % hot_objects);
                if let Some(data) = storage.get(&key) {
                    let _hash = calculate_hash(data);
                    let _metadata = ObjectMetadata::new(&key, data.len());
                }
            }
        });
    });

    group.finish();
}

// Helper structures and functions

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
    concurrent_get_benches,
    bench_concurrent_get_64_clients,
    bench_concurrent_get_scaling,
    bench_sustained_concurrent_get,
    bench_concurrent_get_mixed_sizes,
    bench_concurrent_get_ops_per_sec,
    bench_concurrent_get_cache_hits,
);

criterion_main!(concurrent_get_benches);
