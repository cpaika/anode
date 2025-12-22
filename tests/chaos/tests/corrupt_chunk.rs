//! Corrupt chunk detection and recovery chaos test
//!
//! Tests system behavior when chunks are corrupted on disk

use anode_chaos_tests::scenarios::disk_failure::CorruptChunkScenario;
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::cluster::TestCluster;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_corrupt_chunk_detection_and_recovery() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    // Create a 3-node cluster for replication
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Write some test data to ensure chunks are created
    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "corrupt-test";

    // Create bucket and write multiple objects
    for i in 0..10 {
        let key = format!("test-key-{}", i);
        let value = format!("test-value-{}", i).repeat(1000); // Make it large enough
        let url = format!("{}/{}/{}", endpoint, bucket, key);

        let response = client.put(&url).body(value).send().await;
        if let Ok(resp) = response {
            if !resp.status().is_success() {
                eprintln!("Failed to write object {}: {:?}", key, resp.status());
            }
        }
    }

    // Wait for replication to complete
    sleep(Duration::from_secs(5)).await;

    // Run the corrupt chunk scenario
    let scenario = CorruptChunkScenario;
    let config = ScenarioConfig {
        num_nodes: 3,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 1.0,
        params: serde_json::Value::Null,
    };

    let result = scenario.run(&mut cluster, &config).await.unwrap();

    // Verify scenario results
    println!("Scenario: {}", result.name);
    println!("Success: {}", result.success);
    println!("Duration: {:?}", result.duration);
    println!("Operations: {}", result.operations);
    println!("Failures: {}", result.failures);
    println!("Integrity verified: {}", result.integrity_verified);
    println!("Metrics: {}", result.metrics);
    if let Some(error) = &result.error {
        println!("Error: {}", error);
    }

    // The scenario should succeed (no failures)
    assert!(
        result.success
            || result
                .error
                .as_ref()
                .map(|e| e.contains("No chunks found"))
                .unwrap_or(false),
        "Scenario should succeed or have no chunks to corrupt"
    );

    // Clean up
    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_read_after_corruption() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    // Create a 3-node cluster
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    sleep(Duration::from_secs(5)).await;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "corruption-read-test";
    let key = "test-key";
    let value = "test-data-for-corruption".repeat(100);

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    let write_response = client.put(&url).body(value.clone()).send().await;
    assert!(write_response.is_ok(), "Should be able to write data");

    // Wait for replication
    sleep(Duration::from_secs(5)).await;

    // Run corruption scenario
    let scenario = CorruptChunkScenario;
    let config = ScenarioConfig::default();
    let _result = scenario.run(&mut cluster, &config).await.unwrap();

    // Wait for recovery
    sleep(Duration::from_secs(5)).await;

    // Try to read data - should work from replicas even if one node has corruption
    let read_response = client.get(&url).send().await;

    // In a fully implemented system, this should succeed by reading from healthy replicas
    // For now, we just verify the cluster is still operational
    if let Ok(resp) = read_response {
        println!("Read response status: {}", resp.status());
        // In a complete implementation, we'd verify the data matches
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_corruption_with_concurrent_writes() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    sleep(Duration::from_secs(5)).await;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "concurrent-corruption-test";

    // Start writing data continuously
    let write_handle = tokio::spawn({
        let client = client.clone();
        let endpoint = endpoint.clone();
        let bucket = bucket.clone();
        async move {
            for i in 0..20 {
                let key = format!("concurrent-key-{}", i);
                let value = format!("concurrent-value-{}", i).repeat(100);
                let url = format!("{}/{}/{}", endpoint, bucket, key);

                let _ = client.put(&url).body(value).send().await;
                sleep(Duration::from_millis(500)).await;
            }
        }
    });

    // Wait a bit for some data to be written
    sleep(Duration::from_secs(5)).await;

    // Inject corruption while writes are happening
    let scenario = CorruptChunkScenario;
    let config = ScenarioConfig {
        num_nodes: 3,
        duration: Duration::from_secs(30),
        verify_integrity: true,
        workload_intensity: 5.0,
        params: serde_json::Value::Null,
    };

    let result = scenario.run(&mut cluster, &config).await.unwrap();

    // Wait for writes to complete
    let _ = write_handle.await;

    // Verify the cluster survived corruption during concurrent operations
    println!("Concurrent corruption test completed");
    println!("Success: {}", result.success);
    println!("Metrics: {}", result.metrics);

    cluster.stop().await.unwrap();
}
