//! Full cluster restart chaos test
//!
//! Tests that data survives when all nodes in the cluster are stopped and restarted.
//! This is a critical test for disaster recovery scenarios.

use anode_chaos_tests::scenarios::node_crash::FullClusterCrashScenario;
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::TestCluster;
use std::time::Duration;

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_full_cluster_restart_data_survives() {
    // Create a 5-node cluster
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    let scenario = FullClusterCrashScenario;
    let result = scenario.run(&mut cluster, &config).await.unwrap();

    // Verify the scenario succeeded
    assert!(
        result.success,
        "Full cluster restart scenario should succeed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity should be verified after full cluster restart"
    );
    assert_eq!(
        result.failures, 0,
        "Should have no failures during full cluster restart"
    );

    println!("\n=== Full Cluster Restart Test Results ===");
    println!("Success: {}", result.success);
    println!("Duration: {:?}", result.duration);
    println!("Operations: {}", result.operations);
    println!("Failures: {}", result.failures);
    println!("Integrity Verified: {}", result.integrity_verified);
    println!(
        "Metrics: {}",
        serde_json::to_string_pretty(&result.metrics).unwrap()
    );

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_full_cluster_restart_with_large_dataset() {
    use anode_chaos_tests::verification::DataVerifier;

    // Create a 3-node cluster for faster testing
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(3)).await;

    let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

    // Write a larger dataset
    println!("Writing test dataset...");
    for i in 0..50 {
        let bucket = format!("bucket-{}", i % 5);
        let key = format!("key-{}", i);
        let data = format!("test-data-{}", i).repeat(100).into_bytes();

        verifier
            .write_and_track(&bucket, &key, data)
            .await
            .expect("Failed to write test data");
    }

    println!("Wrote 50 objects across 5 buckets");

    // Wait for replication
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Stop all nodes
    println!("Stopping all nodes...");
    for i in 0..3 {
        cluster.kill_node(i).await.unwrap();
    }

    // Wait to simulate downtime
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Restart all nodes
    println!("Restarting all nodes...");
    for i in 0..3 {
        cluster.restart_node(i).await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Wait for cluster to reform
    println!("Waiting for cluster to reform...");
    tokio::time::sleep(Duration::from_secs(15)).await;

    // Wait for leader election
    let leader = cluster
        .wait_for_leader(Duration::from_secs(60))
        .await
        .expect("Failed to elect leader after restart");
    println!("New leader elected: node {}", leader);

    // Verify all data is intact
    println!("Verifying data integrity...");
    let integrity_verified = verifier.verify_all().await.expect("Failed to verify data");

    assert!(
        integrity_verified,
        "All data should be intact after full cluster restart"
    );

    println!("✓ All 50 objects verified successfully after full cluster restart");

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_multiple_full_cluster_restart_cycles() {
    use anode_chaos_tests::verification::DataVerifier;

    // Create a 3-node cluster
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(3)).await;

    let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

    // Write initial dataset
    println!("Writing initial dataset...");
    for i in 0..10 {
        let key = format!("initial-key-{}", i);
        let data = format!("initial-data-{}", i).into_bytes();

        verifier
            .write_and_track("test-bucket", &key, data)
            .await
            .expect("Failed to write initial data");
    }

    // Perform multiple full cluster restart cycles
    for cycle in 0..3 {
        println!("\n=== Restart Cycle {} ===", cycle + 1);

        // Write cycle-specific data
        let key = format!("cycle-{}-data", cycle);
        let data = format!("data-from-cycle-{}", cycle).into_bytes();
        verifier
            .write_and_track("test-bucket", &key, data)
            .await
            .expect("Failed to write cycle data");

        // Wait for replication
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Stop all nodes
        println!("Stopping all nodes...");
        for i in 0..3 {
            cluster.kill_node(i).await.unwrap();
        }

        tokio::time::sleep(Duration::from_secs(2)).await;

        // Restart all nodes
        println!("Restarting all nodes...");
        for i in 0..3 {
            cluster.restart_node(i).await.unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Wait for cluster to reform
        tokio::time::sleep(Duration::from_secs(10)).await;

        // Wait for leader election
        cluster
            .wait_for_leader(Duration::from_secs(60))
            .await
            .expect("Failed to elect leader");

        // Verify all data
        println!("Verifying data after cycle {}...", cycle + 1);
        let integrity_verified = verifier.verify_all().await.expect("Failed to verify data");

        assert!(
            integrity_verified,
            "Data should be intact after restart cycle {}",
            cycle + 1
        );
    }

    println!("\n✓ All data survived {} full cluster restart cycles", 3);

    cluster.stop().await.unwrap();
}
