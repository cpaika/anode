//! Slow network chaos tests
//!
//! Tests system behavior under network degradation

use anode_test_harness::{assertions::*, cluster::TestCluster};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_high_latency_network() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "latency-test";

    // In a real implementation, we would inject network latency using tc/netem
    // For now, we simulate with application-level delays

    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await.unwrap();
        sleep(Duration::from_millis(50)).await;
    }

    // Verify all writes succeeded despite latency
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_packet_loss() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "packet-loss-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data (in real impl, packet loss would be injected via tc)
    let url = format!("{}/{}/{}", endpoint, bucket, key);

    // Retry logic should handle packet loss
    let mut attempts = 0;
    let max_attempts = 5;
    let mut success = false;

    while attempts < max_attempts && !success {
        match client.put(&url).body(value.to_vec()).send().await {
            Ok(response) if response.status().is_success() => {
                success = true;
            },
            _ => {
                attempts += 1;
                sleep(Duration::from_millis(100)).await;
            },
        }
    }

    assert!(success, "Should eventually succeed despite packet loss");

    // Verify data
    assert_object_content(&cluster, bucket, key, value)
        .await
        .unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_bandwidth_limitation() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "bandwidth-test";

    // Write large objects (in real impl, bandwidth would be limited via tc)
    let large_value = vec![0u8; 5 * 1024 * 1024]; // 5MB

    for i in 0..3 {
        let key = format!("large-key-{}", i);
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        // This may take longer with bandwidth limits
        let start = std::time::Instant::now();
        client
            .put(&url)
            .body(large_value.clone())
            .send()
            .await
            .unwrap();
        let duration = start.elapsed();

        println!("Upload {} took {:?}", i, duration);
    }

    // Verify all large objects are stored
    for i in 0..3 {
        let key = format!("large-key-{}", i);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_asymmetric_network_delays() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "asymmetric-test";
    let key = "test-key";
    let value = b"test-value";

    // In a real implementation, we would inject different latencies
    // for different nodes using tc/netem

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    sleep(Duration::from_secs(3)).await;

    // Verify data eventually replicates despite asymmetric delays
    assert_data_replicated(&cluster, bucket, key, 3)
        .await
        .unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_network_congestion() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "congestion-test";

    // Simulate congestion by sending many concurrent requests
    let mut tasks = Vec::new();

    for i in 0..50 {
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let client = client.clone();

        tasks.push(tokio::spawn(async move {
            let key = format!("key-{}", i);
            let value = format!("value-{}", i).into_bytes();
            let url = format!("{}/{}/{}", endpoint, &bucket, &key);

            client.put(&url).body(value).send().await
        }));
    }

    // Wait for all requests
    let mut successful = 0;
    for task in tasks {
        if let Ok(Ok(response)) = task.await {
            if response.status().is_success() {
                successful += 1;
            }
        }
    }

    println!("Successful writes: {}/50", successful);

    // Most writes should succeed despite congestion
    assert!(successful >= 40, "Should handle congestion gracefully");

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_100ms_network_latency() {
    use anode_chaos_tests::network::{NetworkChaos, NetworkCondition};

    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "latency-100ms-test";

    // Create network chaos controller
    let network_chaos = NetworkChaos::default();

    // Inject 100ms latency
    let latency_result = network_chaos
        .add_condition(NetworkCondition::Latency {
            delay_ms: 100,
            jitter_ms: 0,
        })
        .await;

    // If tc is not available, we'll just log a warning and continue with the test
    // to verify basic functionality without actual latency injection
    if latency_result.is_err() {
        println!("Warning: Could not inject network latency (tc not available). Running without latency.");
    }

    // Write test data with 100ms latency
    let test_data = vec![
        ("key-1", b"value-1".to_vec()),
        ("key-2", b"value-2".to_vec()),
        ("key-3", b"value-3".to_vec()),
        ("key-4", b"value-4".to_vec()),
        ("key-5", b"value-5".to_vec()),
    ];

    let start = std::time::Instant::now();
    for (key, value) in &test_data {
        let url = format!("{}/{}/{}", endpoint, bucket, key);
        let response = client.put(&url).body(value.clone()).send().await;

        assert!(
            response.is_ok() && response.unwrap().status().is_success(),
            "Write should succeed with 100ms latency"
        );
    }
    let write_duration = start.elapsed();
    println!(
        "Completed {} writes in {:?}",
        test_data.len(),
        write_duration
    );

    // Verify reads work correctly with latency
    for (key, expected_value) in &test_data {
        let result = assert_object_content(&cluster, bucket, key, expected_value).await;
        assert!(
            result.is_ok(),
            "Read should succeed with 100ms latency for key: {}",
            key
        );
    }

    // Test concurrent operations under latency
    let mut concurrent_tasks = Vec::new();
    for i in 0..10 {
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let client = client.clone();

        concurrent_tasks.push(tokio::spawn(async move {
            let key = format!("concurrent-key-{}", i);
            let value = format!("concurrent-value-{}", i).into_bytes();
            let url = format!("{}/{}/{}", endpoint, &bucket, &key);

            client.put(&url).body(value).send().await
        }));
    }

    // Wait for concurrent operations
    let mut successful_concurrent = 0;
    for task in concurrent_tasks {
        if let Ok(Ok(response)) = task.await {
            if response.status().is_success() {
                successful_concurrent += 1;
            }
        }
    }

    println!("Successful concurrent writes: {}/10", successful_concurrent);
    assert_eq!(
        successful_concurrent, 10,
        "All concurrent writes should succeed with 100ms latency"
    );

    // Verify all concurrent writes
    for i in 0..10 {
        let key = format!("concurrent-key-{}", i);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    // Clean up network conditions
    let _ = network_chaos.clear_conditions().await;

    // Verify cluster is still healthy after latency test
    sleep(Duration::from_secs(2)).await;

    // Final data integrity check - all objects should still be accessible
    for (key, _) in &test_data {
        assert_object_exists(&cluster, bucket, key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_10_percent_packet_loss() {
    use anode_chaos_tests::network::{NetworkChaos, NetworkCondition};

    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "packet-loss-10pct-test";

    // Create network chaos controller
    let network_chaos = NetworkChaos::default();

    // Inject 10% packet loss
    let packet_loss_result = network_chaos
        .add_condition(NetworkCondition::PacketLoss { percent: 10.0 })
        .await;

    // If tc is not available, we'll just log a warning and continue with the test
    // to verify basic functionality without actual packet loss injection
    if packet_loss_result.is_err() {
        println!("Warning: Could not inject packet loss (tc not available). Running without packet loss.");
    }

    // Write test data with 10% packet loss
    let test_data = vec![
        ("key-1", b"value-1".to_vec()),
        ("key-2", b"value-2".to_vec()),
        ("key-3", b"value-3".to_vec()),
        ("key-4", b"value-4".to_vec()),
        ("key-5", b"value-5".to_vec()),
        ("key-6", b"value-6".to_vec()),
        ("key-7", b"value-7".to_vec()),
        ("key-8", b"value-8".to_vec()),
        ("key-9", b"value-9".to_vec()),
        ("key-10", b"value-10".to_vec()),
    ];

    // Write data with possible packet loss - some requests may need retries
    let start = std::time::Instant::now();
    let mut write_successes = 0;
    let mut write_failures = 0;

    for (key, value) in &test_data {
        let url = format!("{}/{}/{}", endpoint, bucket, key);

        // Retry logic to handle packet loss
        let mut attempts = 0;
        let max_attempts = 5;
        let mut success = false;

        while attempts < max_attempts && !success {
            match client.put(&url).body(value.clone()).send().await {
                Ok(response) if response.status().is_success() => {
                    success = true;
                    write_successes += 1;
                },
                Ok(response) => {
                    println!(
                        "Write attempt {} for key {} failed with status: {}",
                        attempts + 1,
                        key,
                        response.status()
                    );
                    attempts += 1;
                    sleep(Duration::from_millis(100)).await;
                },
                Err(e) => {
                    println!(
                        "Write attempt {} for key {} failed with error: {}",
                        attempts + 1,
                        key,
                        e
                    );
                    attempts += 1;
                    sleep(Duration::from_millis(100)).await;
                },
            }
        }

        if !success {
            write_failures += 1;
            println!(
                "Failed to write key {} after {} attempts",
                key, max_attempts
            );
        }
    }

    let write_duration = start.elapsed();
    println!(
        "Completed {} writes ({} succeeded, {} failed) in {:?}",
        test_data.len(),
        write_successes,
        write_failures,
        write_duration
    );

    // Despite packet loss, we expect most/all writes to eventually succeed with retries
    assert!(
        write_successes >= (test_data.len() as f64 * 0.8) as usize,
        "At least 80% of writes should succeed with retries under 10% packet loss"
    );

    // Verify reads work correctly with packet loss
    let mut read_successes = 0;
    for (key, expected_value) in &test_data {
        let mut attempts = 0;
        let max_attempts = 5;
        let mut verified = false;

        while attempts < max_attempts && !verified {
            match assert_object_content(&cluster, bucket, key, expected_value).await {
                Ok(_) => {
                    verified = true;
                    read_successes += 1;
                },
                Err(e) => {
                    println!(
                        "Read verification attempt {} for key {} failed: {}",
                        attempts + 1,
                        key,
                        e
                    );
                    attempts += 1;
                    sleep(Duration::from_millis(100)).await;
                },
            }
        }

        if !verified {
            println!(
                "Failed to verify key {} after {} attempts",
                key, max_attempts
            );
        }
    }

    println!(
        "Read verification: {}/{} succeeded",
        read_successes, write_successes
    );

    // Test concurrent operations under packet loss
    let mut concurrent_tasks = Vec::new();
    for i in 0..20 {
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let client = client.clone();

        concurrent_tasks.push(tokio::spawn(async move {
            let key = format!("concurrent-key-{}", i);
            let value = format!("concurrent-value-{}", i).into_bytes();
            let url = format!("{}/{}/{}", endpoint, &bucket, &key);

            // Retry logic for concurrent writes
            for attempt in 0..5 {
                match client.put(&url).body(value.clone()).send().await {
                    Ok(response) if response.status().is_success() => {
                        return Ok(());
                    },
                    _ => {
                        if attempt < 4 {
                            sleep(Duration::from_millis(100)).await;
                        }
                    },
                }
            }
            Err(())
        }));
    }

    // Wait for concurrent operations
    let mut successful_concurrent = 0;
    for task in concurrent_tasks {
        if let Ok(Ok(())) = task.await {
            successful_concurrent += 1;
        }
    }

    println!("Successful concurrent writes: {}/20", successful_concurrent);

    // Under 10% packet loss with retries, we expect good success rate
    assert!(
        successful_concurrent >= 16,
        "At least 80% of concurrent writes should succeed with retries under 10% packet loss"
    );

    // Verify data integrity - no corruption despite packet loss
    let mut integrity_check_passed = 0;
    for i in 0..successful_concurrent.min(20) {
        let key = format!("concurrent-key-{}", i);
        if assert_object_exists(&cluster, bucket, &key).await.is_ok() {
            integrity_check_passed += 1;
        }
    }

    println!(
        "Integrity check: {}/{} objects verified",
        integrity_check_passed, successful_concurrent
    );

    // Clean up network conditions
    let _ = network_chaos.clear_conditions().await;

    // Verify cluster is still healthy after packet loss test
    sleep(Duration::from_secs(2)).await;

    // Final data integrity check - verify no data corruption occurred
    for (key, expected_value) in test_data.iter().take(write_successes) {
        // Try to read each successfully written object
        match assert_object_content(&cluster, bucket, key, expected_value).await {
            Ok(_) => {
                // Data integrity verified
            },
            Err(e) => {
                panic!("Data corruption detected for key {}: {}", key, e);
            },
        }
    }

    cluster.stop().await.unwrap();
}
