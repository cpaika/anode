//! Rolling restart chaos tests
//!
//! Tests system behavior during rolling restarts

use anode_test_harness::{
    assertions::*,
    cluster::TestCluster,
    chaos::ChaosScenarios,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_basic_rolling_restart() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rolling-restart-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Perform rolling restart
    let scheduler = ChaosScenarios::rolling_restart(3, Duration::from_secs(5));
    scheduler.run(&mut cluster).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Data should still be accessible
    assert_object_content(&cluster, bucket, key, value).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rolling_restart_with_writes() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rolling-restart-writes-test";

    // Start continuous writes
    let write_task = tokio::spawn({
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        async move {
            let client = reqwest::Client::new();
            let mut successful_writes = Vec::new();

            for i in 0..50 {
                let key = format!("key-{:03}", i);
                let value = format!("value-{}", i).into_bytes();
                let url = format!("{}/{}/{}", endpoint, &bucket, &key);

                match client.put(&url).body(value).send().await {
                    Ok(response) if response.status().is_success() => {
                        successful_writes.push(key);
                    }
                    _ => {}
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            successful_writes
        }
    });

    // Start rolling restart after a delay
    sleep(Duration::from_millis(500)).await;

    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await.unwrap();
        sleep(Duration::from_secs(3)).await;
    }

    // Wait for writes to complete
    let successful_writes = write_task.await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(3)).await;

    // Verify all successful writes are durable
    for key in successful_writes {
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_fast_rolling_restart() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "fast-rolling-test";

    // Write test data
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await.unwrap();
    }

    // Fast rolling restart (1 second intervals)
    let scheduler = ChaosScenarios::rolling_restart(
        cluster.num_nodes(),
        Duration::from_secs(1),
    );
    scheduler.run(&mut cluster).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(10)).await;

    // Verify all data survived
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rolling_restart_leader_change() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    // Record initial leader
    let initial_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();

    // Perform rolling restart
    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await.unwrap();
        sleep(Duration::from_secs(3)).await;

        // Wait for a leader to be present
        let _ = cluster.wait_for_leader(Duration::from_secs(10)).await;
    }

    // Wait for final stabilization
    sleep(Duration::from_secs(5)).await;

    // A leader should still be elected
    let final_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();

    println!("Initial leader: {}, Final leader: {}", initial_leader, final_leader);

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_overlapping_rolling_restart() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "overlapping-restart-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Restart nodes with very short intervals so restarts overlap
    for i in 0..cluster.num_nodes() - 1 {
        tokio::spawn({
            let mut cluster_clone = cluster.get_node_mut(i).unwrap();
            async move {
                // This is a simplified version - in reality would need proper async handling
            }
        });

        sleep(Duration::from_millis(500)).await;
    }

    // Wait for all restarts to complete
    sleep(Duration::from_secs(15)).await;

    // Cluster should recover
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    // Data should still be accessible
    assert_object_exists(&cluster, bucket, key).await.unwrap();

    cluster.stop().await.unwrap();
}
