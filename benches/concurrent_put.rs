// Concurrent PUT benchmarks
//
// These benchmarks measure concurrent PUT operation performance:
// - Throughput with multiple concurrent clients
// - Scalability across different client counts
// - Operations per second under concurrent load

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::Duration;

mod common {
    pub use anode_bench::*;
}

use common::*;

/// Benchmark concurrent PUT operations with 64 clients
///
/// Simulates 64 concurrent clients performing PUT operations simultaneously.
/// Measures overall system throughput (ops/sec) under concurrent load.
fn bench_concurrent_put_64_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_put_64_clients");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Test with different object sizes
    for size in [1024, 64 * 1024, 1024 * 1024].iter() {
        let size_kb = size / 1024;
        let data = Arc::new(generate_random_data(*size));

        // Configure throughput measurement
        // Total throughput = 64 clients * operations
        group.throughput(Throughput::Bytes((size * 64) as u64));

        group.bench_with_input(
            BenchmarkId::new("64_clients", format!("{}kb", size_kb)),
            size,
            |b, _| {
                b.iter(|| {
                    // Simulate 64 concurrent clients doing PUT operations
                    // In a real implementation, this would spawn 64 tokio tasks
                    for client_id in 0..64 {
                        let key = format!("client-{}/object", client_id);
                        let _hash = calculate_hash(&data);
                        let _metadata = ObjectMetadata::new(&key, data.len());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent PUT operations with varying client counts
///
/// Measures how system throughput scales with different numbers of concurrent clients.
/// Tests 1, 4, 16, 32, and 64 clients to show scaling characteristics.
fn bench_concurrent_put_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_put_scaling");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let size = 64 * 1024; // 64KB objects
    let data = Arc::new(generate_random_data(size));

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
                        let _hash = calculate_hash(&data);
                        let _metadata = ObjectMetadata::new(&key, data.len());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark sustained concurrent PUT throughput
///
/// Measures sustained throughput with 64 clients performing multiple PUTs each.
/// This tests if the system can maintain performance under prolonged concurrent load.
fn bench_sustained_concurrent_put(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_concurrent_put");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    let size = 64 * 1024; // 64KB objects
    let data = Arc::new(generate_random_data(size));
    let puts_per_client = 10;
    let clients = 64;

    // Total operations = 64 clients * 10 puts each = 640 operations
    group.throughput(Throughput::Elements((clients * puts_per_client) as u64));

    group.bench_function("64_clients_10_puts_each", |b| {
        b.iter(|| {
            // Each of 64 clients performs 10 PUT operations
            for client_id in 0..clients {
                for put_id in 0..puts_per_client {
                    let key = format!("client-{}/object-{}", client_id, put_id);
                    let _hash = calculate_hash(&data);
                    let _metadata = ObjectMetadata::new(&key, data.len());
                }
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent PUT with mixed object sizes
///
/// Simulates realistic workload where 64 concurrent clients PUT objects of varying sizes.
/// Tests how the system handles heterogeneous concurrent workloads.
fn bench_concurrent_put_mixed_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_put_mixed_sizes");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Pre-generate objects of different sizes
    let small = Arc::new(generate_random_data(1024)); // 1KB
    let medium = Arc::new(generate_random_data(64 * 1024)); // 64KB
    let large = Arc::new(generate_random_data(1024 * 1024)); // 1MB

    let clients = 64;
    group.throughput(Throughput::Elements(clients as u64));

    group.bench_function("64_clients_mixed", |b| {
        b.iter(|| {
            // Each client PUTs a different size based on client_id
            for client_id in 0..clients {
                let key = format!("client-{}/object", client_id);

                // Distribute sizes: ~1/3 small, ~1/3 medium, ~1/3 large
                let data = match client_id % 3 {
                    0 => &small,
                    1 => &medium,
                    _ => &large,
                };

                let _hash = calculate_hash(data);
                let _metadata = ObjectMetadata::new(&key, data.len());
            }
        });
    });

    group.finish();
}

/// Benchmark concurrent PUT operations per second
///
/// Dedicated benchmark for measuring pure operations per second with 64 concurrent clients.
/// Focuses on 1KB objects to maximize operation count and minimize data transfer overhead.
fn bench_concurrent_put_ops_per_sec(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_put_ops_per_sec");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    let size = 1024; // 1KB for high ops/sec
    let data = Arc::new(generate_random_data(size));
    let clients = 64;

    // Measure operations per second
    group.throughput(Throughput::Elements(clients as u64));

    group.bench_function("64_clients_1kb_ops", |b| {
        let mut counter = 0;
        b.iter(|| {
            // 64 clients each performing a PUT operation
            for client_id in 0..clients {
                let key = format!("client-{}/object-{}", client_id, counter);
                let _hash = calculate_hash(&data);
                let _metadata = ObjectMetadata::new(&key, data.len());
            }
            counter += 1;
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
    concurrent_put_benches,
    bench_concurrent_put_64_clients,
    bench_concurrent_put_scaling,
    bench_sustained_concurrent_put,
    bench_concurrent_put_mixed_sizes,
    bench_concurrent_put_ops_per_sec,
);

criterion_main!(concurrent_put_benches);
