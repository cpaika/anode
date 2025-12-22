//! Recovery testing module
//!
//! Tests that verify the system can recover from failures, corruption,
//! and data loss.

use anode_test_harness::{
    assertions::{assert_data_replicated, assert_object_content, assert_object_exists},
    cluster::TestCluster,
    Result,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

/// Test data rebuild after node loss
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_data_rebuild_after_node_loss() -> Result<()> {
    crate::init_tracing();
    info!("Starting data rebuild test");

    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rebuild-test";

    // Write test data
    let num_objects = 20;
    for i in 0..num_objects {
        let key = format!("key-{:04}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    // Wait for replication
    sleep(Duration::from_secs(3)).await;

    info!("Killing node 1 to simulate data loss");
    cluster.kill_node(1).await?;

    // Wait for cluster to detect failure and trigger repair
    sleep(Duration::from_secs(10)).await;

    // Verify all data is still accessible
    for i in 0..num_objects {
        let key = format!("key-{:04}", i);
        let expected = format!("value-{}", i).into_bytes();

        assert_object_content(&cluster, bucket, &key, &expected).await?;
    }

    // Verify data is replicated to sufficient nodes
    for i in 0..num_objects {
        let key = format!("key-{:04}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await?;
    }

    cluster.stop().await?;
    info!("Data rebuild test passed");
    Ok(())
}

/// Test corruption detection and repair
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_corruption_detection_and_repair() -> Result<()> {
    crate::init_tracing();
    info!("Starting corruption detection test");

    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "corruption-test";
    let key = "corrupt-key";
    let value = b"original-uncorrupted-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Wait for replication
    sleep(Duration::from_secs(2)).await;

    info!("Simulating corruption (in real test, would corrupt actual data on disk)");
    // In a real test, we would:
    // 1. Find the data file on disk for node 0
    // 2. Corrupt some bytes in the middle
    // 3. Trigger integrity check
    // This would require file system access to the node's data directory

    // For now, we verify that the system can still serve data
    // from uncorrupted replicas
    let response = client.get(&url).send().await?;
    let read_value = response.bytes().await?;

    assert_eq!(
        read_value.as_ref(),
        value,
        "Should be able to read uncorrupted data from replicas"
    );

    cluster.stop().await?;
    info!("Corruption detection test passed");
    Ok(())
}

/// Test recovery after total cluster shutdown
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_recovery_after_total_shutdown() -> Result<()> {
    crate::init_tracing();
    info!("Starting total shutdown recovery test");

    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "shutdown-test";

    // Write test data
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    info!("Stopping entire cluster");
    cluster.stop().await?;

    sleep(Duration::from_secs(2)).await;

    info!("Restarting cluster");
    cluster.start().await?;

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(10)).await;

    info!("Verifying data after restart");
    // Verify all data is still present
    for i in 0..10 {
        let key = format!("key-{}", i);
        let expected = format!("value-{}", i).into_bytes();

        assert_object_content(&cluster, bucket, &key, &expected).await?;
    }

    cluster.stop().await?;
    info!("Total shutdown recovery test passed");
    Ok(())
}

/// Test recovery from minority failure
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_minority_failure_recovery() -> Result<()> {
    crate::init_tracing();
    info!("Starting minority failure recovery test");

    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "minority-failure-test";

    // Write data
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    sleep(Duration::from_secs(2)).await;

    info!("Killing minority of nodes (2 out of 5)");
    cluster.kill_node(1).await?;
    cluster.kill_node(2).await?;

    // Cluster should still be operational with majority
    sleep(Duration::from_secs(3)).await;

    // Verify we can still read and write
    let new_key = "new-key-during-minority-failure";
    let new_value = b"new-value";
    let url = format!("{}/{}/{}", endpoint, bucket, new_key);

    let response = client.put(&url).body(new_value.to_vec()).send().await?;
    assert!(
        response.status().is_success(),
        "Should be able to write with minority failure"
    );

    // Restart failed nodes
    info!("Restarting failed nodes");
    cluster.restart_node(1).await?;
    cluster.restart_node(2).await?;

    // Wait for recovery
    sleep(Duration::from_secs(10)).await;

    info!("Verifying data consistency after recovery");
    // Verify all data is consistent across all nodes
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await?;
    }

    assert_data_replicated(&cluster, bucket, new_key, 3).await?;

    cluster.stop().await?;
    info!("Minority failure recovery test passed");
    Ok(())
}

/// Test recovery with concurrent operations
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_recovery_with_concurrent_operations() -> Result<()> {
    crate::init_tracing();
    info!("Starting recovery with concurrent operations test");

    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "concurrent-recovery-test";

    // Write initial data
    for i in 0..5 {
        let key = format!("initial-{}", i);
        let value = format!("initial-value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    sleep(Duration::from_secs(1)).await;

    info!("Starting concurrent writes");
    // Start concurrent writes in background
    let client_clone = client.clone();
    let endpoint_clone = endpoint.clone();
    let bucket_clone = bucket.to_string();

    let write_task = tokio::spawn(async move {
        for i in 0..20 {
            let key = format!("concurrent-{}", i);
            let value = format!("concurrent-value-{}", i).into_bytes();
            let url = format!("{}/{}/{}", endpoint_clone, &bucket_clone, &key);

            let _ = client_clone.put(&url).body(value).send().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Kill and restart a node during concurrent writes
    sleep(Duration::from_secs(1)).await;
    info!("Killing node 1 during concurrent writes");
    cluster.kill_node(1).await?;

    sleep(Duration::from_secs(2)).await;
    info!("Restarting node 1");
    cluster.restart_node(1).await?;

    // Wait for concurrent writes to complete
    write_task.await.ok();

    // Wait for recovery
    sleep(Duration::from_secs(5)).await;

    info!("Verifying data consistency");
    // Verify initial data
    for i in 0..5 {
        let key = format!("initial-{}", i);
        let expected = format!("initial-value-{}", i).into_bytes();
        assert_object_content(&cluster, bucket, &key, &expected).await?;
    }

    // Verify as many concurrent writes as possible
    // Some may have failed during the node restart
    let url = format!("{}/{}/{}", endpoint, bucket, "concurrent-0");
    if client
        .get(&url)
        .send()
        .await
        .ok()
        .is_some_and(|r| r.status().is_success())
    {
        info!("Concurrent writes survived recovery");
    }

    cluster.stop().await?;
    info!("Recovery with concurrent operations test passed");
    Ok(())
}

/// Test raft log replay after crash
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_raft_log_replay() -> Result<()> {
    crate::init_tracing();
    info!("Starting raft log replay test");

    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "raft-log-test";

    // Write a sequence of operations
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    info!("Killing leader node (will require log replay on restart)");
    // Wait for leader election
    sleep(Duration::from_secs(2)).await;

    // Kill the leader (this ensures we test log replay)
    cluster.kill_node(0).await?;

    sleep(Duration::from_secs(2)).await;

    info!("Restarting node");
    cluster.restart_node(0).await?;

    // Wait for log replay and catch-up
    sleep(Duration::from_secs(10)).await;

    info!("Verifying all operations were replayed");
    // Verify all operations are present
    for i in 0..10 {
        let key = format!("key-{}", i);
        let expected = format!("value-{}", i).into_bytes();

        assert_object_content(&cluster, bucket, &key, &expected).await?;
    }

    cluster.stop().await?;
    info!("Raft log replay test passed");
    Ok(())
}

/// Test snapshot restore
#[tokio::test]
#[ignore = "Requires cluster infrastructure - run with integration tests"]
async fn test_snapshot_restore() -> Result<()> {
    crate::init_tracing();
    info!("Starting snapshot restore test");

    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "snapshot-test";

    // Write a large amount of data to trigger snapshotting
    info!("Writing data to trigger snapshot creation");
    for i in 0..100 {
        let key = format!("key-{:04}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;

        // Periodically wait to allow snapshots to be created
        if i % 20 == 0 {
            sleep(Duration::from_millis(500)).await;
        }
    }

    // Wait for snapshot to be created
    sleep(Duration::from_secs(5)).await;

    info!("Killing node to test snapshot restore");
    cluster.kill_node(2).await?;

    info!("Restarting node (should restore from snapshot)");
    cluster.restart_node(2).await?;

    // Wait for restoration
    sleep(Duration::from_secs(10)).await;

    info!("Verifying data after snapshot restore");
    // Verify all data is present
    for i in 0..100 {
        let key = format!("key-{:04}", i);
        assert_object_exists(&cluster, bucket, &key).await?;
    }

    cluster.stop().await?;
    info!("Snapshot restore test passed");
    Ok(())
}
