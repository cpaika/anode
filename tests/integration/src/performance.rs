//! Performance regression tests
//!
//! Tests to ensure performance characteristics meet baseline requirements:
//! - Throughput for reads and writes
//! - Latency measurements
//! - Concurrent operation performance
//! - Large object handling
//! - Scalability tests

use crate::{
    setup_cluster, create_test_s3_client, random_bytes, unique_object_key,
    BucketFixture,
};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use serial_test::serial;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

/// Performance baseline: minimum operations per second for small objects
const MIN_SMALL_OPS_PER_SEC: f64 = 100.0;

/// Performance baseline: minimum throughput in MB/s for large objects
const MIN_THROUGHPUT_MBPS: f64 = 50.0;

/// Performance baseline: maximum p99 latency in milliseconds
const MAX_P99_LATENCY_MS: u64 = 500;

#[tokio::test]
#[serial]
async fn test_throughput_baseline() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-throughput").await;

    let object_size = 1024 * 1024; // 1MB
    let num_objects = 100;
    let data = random_bytes(object_size);

    let start = Instant::now();

    // Write objects
    for i in 0..num_objects {
        let key = format!("throughput-object-{}", i);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data.clone())))
            .send()
            .await
            .unwrap();
    }

    let write_duration = start.elapsed();
    let write_throughput_mbps = (num_objects as f64 * object_size as f64)
        / write_duration.as_secs_f64()
        / 1024.0
        / 1024.0;

    println!(
        "Write throughput: {:.2} MB/s ({} objects in {:?})",
        write_throughput_mbps, num_objects, write_duration
    );

    // Read objects
    let start = Instant::now();

    for i in 0..num_objects {
        let key = format!("throughput-object-{}", i);
        let get_result = fixture.client
            .get_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await
            .unwrap();

        // Consume the body
        let _ = get_result.body.collect().await.unwrap();
    }

    let read_duration = start.elapsed();
    let read_throughput_mbps = (num_objects as f64 * object_size as f64)
        / read_duration.as_secs_f64()
        / 1024.0
        / 1024.0;

    println!(
        "Read throughput: {:.2} MB/s ({} objects in {:?})",
        read_throughput_mbps, num_objects, read_duration
    );

    // Verify throughput meets baseline
    assert!(
        write_throughput_mbps >= MIN_THROUGHPUT_MBPS,
        "Write throughput {:.2} MB/s below baseline {} MB/s",
        write_throughput_mbps,
        MIN_THROUGHPUT_MBPS
    );

    assert!(
        read_throughput_mbps >= MIN_THROUGHPUT_MBPS,
        "Read throughput {:.2} MB/s below baseline {} MB/s",
        read_throughput_mbps,
        MIN_THROUGHPUT_MBPS
    );
}

#[tokio::test]
#[serial]
async fn test_latency_baseline() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-latency").await;

    let object_size = 1024; // 1KB
    let num_requests = 100;
    let mut latencies = Vec::new();

    // Warm up
    for i in 0..10 {
        let key = format!("warmup-object-{}", i);
        let data = random_bytes(object_size);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data)))
            .send()
            .await
            .unwrap();
    }

    // Measure write latencies
    for i in 0..num_requests {
        let key = format!("latency-object-{}", i);
        let data = random_bytes(object_size);

        let start = Instant::now();
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data)))
            .send()
            .await
            .unwrap();
        let latency = start.elapsed();

        latencies.push(latency.as_millis() as u64);
    }

    // Calculate percentiles
    latencies.sort();
    let p50 = latencies[latencies.len() * 50 / 100];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;

    println!(
        "Write latency - avg: {}ms, p50: {}ms, p95: {}ms, p99: {}ms",
        avg, p50, p95, p99
    );

    // Measure read latencies
    latencies.clear();
    for i in 0..num_requests {
        let key = format!("latency-object-{}", i);

        let start = Instant::now();
        let get_result = fixture.client
            .get_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
        let _ = get_result.body.collect().await.unwrap();
        let latency = start.elapsed();

        latencies.push(latency.as_millis() as u64);
    }

    latencies.sort();
    let p50 = latencies[latencies.len() * 50 / 100];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;

    println!(
        "Read latency - avg: {}ms, p50: {}ms, p95: {}ms, p99: {}ms",
        avg, p50, p95, p99
    );

    // Verify p99 latency meets baseline
    assert!(
        p99 <= MAX_P99_LATENCY_MS,
        "P99 read latency {}ms exceeds baseline {}ms",
        p99,
        MAX_P99_LATENCY_MS
    );
}

#[tokio::test]
#[serial]
async fn test_concurrent_writes() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-concurrent-writes").await;

    let num_concurrent = 50;
    let object_size = 1024 * 1024; // 1MB

    let start = Instant::now();

    let mut tasks = JoinSet::new();

    for i in 0..num_concurrent {
        let client = fixture.client.clone();
        let bucket = fixture.bucket.clone();
        let key = format!("concurrent-object-{}", i);
        let data = random_bytes(object_size);

        tasks.spawn(async move {
            client
                .put_object()
                .bucket(&bucket)
                .key(&key)
                .body(ByteStream::from(Bytes::from(data)))
                .send()
                .await
        });
    }

    // Wait for all tasks to complete
    let mut success_count = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }

    let duration = start.elapsed();
    let ops_per_sec = num_concurrent as f64 / duration.as_secs_f64();

    println!(
        "Concurrent writes: {} succeeded, {:.2} ops/sec",
        success_count, ops_per_sec
    );

    assert_eq!(
        success_count, num_concurrent,
        "All concurrent writes should succeed"
    );

    assert!(
        ops_per_sec >= MIN_SMALL_OPS_PER_SEC,
        "Concurrent write performance {:.2} ops/sec below baseline {} ops/sec",
        ops_per_sec,
        MIN_SMALL_OPS_PER_SEC
    );
}

#[tokio::test]
#[serial]
async fn test_concurrent_reads() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-concurrent-reads").await;

    let num_objects = 50;
    let object_size = 1024 * 1024; // 1MB
    let data = random_bytes(object_size);

    // Create objects
    for i in 0..num_objects {
        let key = format!("read-object-{}", i);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data.clone())))
            .send()
            .await
            .unwrap();
    }

    // Concurrent reads
    let start = Instant::now();

    let mut tasks = JoinSet::new();

    for i in 0..num_objects {
        let client = fixture.client.clone();
        let bucket = fixture.bucket.clone();
        let key = format!("read-object-{}", i);

        tasks.spawn(async move {
            let result = client
                .get_object()
                .bucket(&bucket)
                .key(&key)
                .send()
                .await?;
            result.body.collect().await?;
            Ok::<_, aws_sdk_s3::error::SdkError<_>>(())
        });
    }

    // Wait for all tasks
    let mut success_count = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }

    let duration = start.elapsed();
    let ops_per_sec = num_objects as f64 / duration.as_secs_f64();

    println!(
        "Concurrent reads: {} succeeded, {:.2} ops/sec",
        success_count, ops_per_sec
    );

    assert_eq!(
        success_count, num_objects,
        "All concurrent reads should succeed"
    );

    assert!(
        ops_per_sec >= MIN_SMALL_OPS_PER_SEC,
        "Concurrent read performance {:.2} ops/sec below baseline {} ops/sec",
        ops_per_sec,
        MIN_SMALL_OPS_PER_SEC
    );
}

#[tokio::test]
#[serial]
async fn test_large_object_performance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-large-object").await;

    let object_size = 100 * 1024 * 1024; // 100MB
    let data = random_bytes(object_size);
    let key = "large-object";

    // Measure write time
    let start = Instant::now();
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();
    let write_duration = start.elapsed();

    let write_throughput_mbps = (object_size as f64)
        / write_duration.as_secs_f64()
        / 1024.0
        / 1024.0;

    println!(
        "Large object write: {:.2} MB/s ({} MB in {:?})",
        write_throughput_mbps,
        object_size / 1024 / 1024,
        write_duration
    );

    // Measure read time
    let start = Instant::now();
    let get_result = fixture.client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();
    let _ = get_result.body.collect().await.unwrap();
    let read_duration = start.elapsed();

    let read_throughput_mbps = (object_size as f64)
        / read_duration.as_secs_f64()
        / 1024.0
        / 1024.0;

    println!(
        "Large object read: {:.2} MB/s ({} MB in {:?})",
        read_throughput_mbps,
        object_size / 1024 / 1024,
        read_duration
    );

    assert!(
        write_throughput_mbps >= MIN_THROUGHPUT_MBPS,
        "Large object write throughput {:.2} MB/s below baseline",
        write_throughput_mbps
    );

    assert!(
        read_throughput_mbps >= MIN_THROUGHPUT_MBPS,
        "Large object read throughput {:.2} MB/s below baseline",
        read_throughput_mbps
    );
}

#[tokio::test]
#[serial]
async fn test_mixed_workload_performance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-mixed-workload").await;

    let num_operations = 100;
    let small_size = 1024; // 1KB
    let large_size = 1024 * 1024; // 1MB

    // Pre-create some objects for reads
    for i in 0..num_operations / 2 {
        let key = format!("mixed-object-{}", i);
        let data = random_bytes(if i % 2 == 0 { small_size } else { large_size });
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data)))
            .send()
            .await
            .unwrap();
    }

    let start = Instant::now();

    let mut tasks = JoinSet::new();

    // Mix of reads and writes
    for i in 0..num_operations {
        let client = fixture.client.clone();
        let bucket = fixture.bucket.clone();

        if i % 2 == 0 {
            // Read existing object
            let key = format!("mixed-object-{}", i % (num_operations / 2));
            tasks.spawn(async move {
                let result = client
                    .get_object()
                    .bucket(&bucket)
                    .key(&key)
                    .send()
                    .await?;
                result.body.collect().await?;
                Ok::<_, aws_sdk_s3::error::SdkError<_>>(())
            });
        } else {
            // Write new object
            let key = format!("mixed-new-object-{}", i);
            let data = random_bytes(if i % 4 == 1 { small_size } else { large_size });
            tasks.spawn(async move {
                client
                    .put_object()
                    .bucket(&bucket)
                    .key(&key)
                    .body(ByteStream::from(Bytes::from(data)))
                    .send()
                    .await
                    .map(|_| ())
            });
        }
    }

    // Wait for all operations
    let mut success_count = 0;
    while let Some(result) = tasks.join_next().await {
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }

    let duration = start.elapsed();
    let ops_per_sec = num_operations as f64 / duration.as_secs_f64();

    println!(
        "Mixed workload: {} operations succeeded, {:.2} ops/sec",
        success_count, ops_per_sec
    );

    assert!(
        success_count >= num_operations * 95 / 100,
        "At least 95% of operations should succeed"
    );
}

#[tokio::test]
#[serial]
async fn test_list_performance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-list-perf").await;

    let num_objects = 1000;

    // Create objects
    for i in 0..num_objects {
        let key = format!("list-object-{:05}", i);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from_static(b"test")))
            .send()
            .await
            .unwrap();
    }

    // Measure list performance
    let start = Instant::now();
    let list_result = fixture.client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .max_keys(num_objects as i32)
        .send()
        .await
        .unwrap();
    let list_duration = start.elapsed();

    let objects_listed = list_result.contents().len();

    println!(
        "List performance: {} objects in {:?} ({:.2} objects/sec)",
        objects_listed,
        list_duration,
        objects_listed as f64 / list_duration.as_secs_f64()
    );

    assert_eq!(
        objects_listed, num_objects,
        "Should list all objects"
    );

    // List operation should complete in reasonable time
    assert!(
        list_duration.as_secs() < 10,
        "List operation took too long: {:?}",
        list_duration
    );
}

#[tokio::test]
#[serial]
async fn test_delete_performance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-delete-perf").await;

    let num_objects = 100;

    // Create objects
    for i in 0..num_objects {
        let key = format!("delete-object-{}", i);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from_static(b"test")))
            .send()
            .await
            .unwrap();
    }

    // Measure delete performance
    let start = Instant::now();

    for i in 0..num_objects {
        let key = format!("delete-object-{}", i);
        fixture.client
            .delete_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
    }

    let delete_duration = start.elapsed();
    let deletes_per_sec = num_objects as f64 / delete_duration.as_secs_f64();

    println!(
        "Delete performance: {} objects in {:?} ({:.2} deletes/sec)",
        num_objects, delete_duration, deletes_per_sec
    );

    assert!(
        deletes_per_sec >= MIN_SMALL_OPS_PER_SEC,
        "Delete performance {:.2} ops/sec below baseline",
        deletes_per_sec
    );
}

#[tokio::test]
#[serial]
async fn test_metadata_operation_performance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-metadata-perf").await;

    let num_objects = 100;

    // Create objects
    for i in 0..num_objects {
        let key = format!("metadata-object-{}", i);
        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from_static(b"test")))
            .send()
            .await
            .unwrap();
    }

    // Measure head_object performance
    let start = Instant::now();

    for i in 0..num_objects {
        let key = format!("metadata-object-{}", i);
        fixture.client
            .head_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await
            .unwrap();
    }

    let head_duration = start.elapsed();
    let heads_per_sec = num_objects as f64 / head_duration.as_secs_f64();

    println!(
        "HeadObject performance: {} operations in {:?} ({:.2} ops/sec)",
        num_objects, head_duration, heads_per_sec
    );

    // Metadata operations should be fast
    assert!(
        heads_per_sec >= MIN_SMALL_OPS_PER_SEC * 2.0,
        "HeadObject performance {:.2} ops/sec below baseline",
        heads_per_sec
    );
}
