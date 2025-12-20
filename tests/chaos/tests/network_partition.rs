//! Network partition chaos tests
//!
//! Tests system behavior during network partitions

use anode_test_harness::{
    assertions::*,
    cluster::TestCluster,
};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_minority_partition() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "partition-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data before partition
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Create minority partition (2 nodes out of 5)
    cluster.partition_node(0).await.unwrap();
    cluster.partition_node(1).await.unwrap();

    sleep(Duration::from_secs(2)).await;

    // Majority partition should still serve reads
    assert_object_exists(&cluster, bucket, key).await.unwrap();

    // Heal partition
    cluster.heal_partition(0).await.unwrap();
    cluster.heal_partition(1).await.unwrap();

    sleep(Duration::from_secs(3)).await;

    // All nodes should eventually converge
    assert_data_replicated(&cluster, bucket, key, 3).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_partition_and_writes() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "partition-writes-test";

    // Write data before partition
    let key1 = "before-partition";
    let value1 = b"before-value";
    let url1 = format!("{}/{}/{}", endpoint, bucket, key1);
    client.put(&url1).body(value1.to_vec()).send().await.unwrap();

    // Create partition
    cluster.partition_node(0).await.unwrap();

    sleep(Duration::from_secs(1)).await;

    // Write data during partition (should succeed in majority)
    let key2 = "during-partition";
    let value2 = b"during-value";
    let url2 = format!("{}/{}/{}", endpoint, bucket, key2);
    client.put(&url2).body(value2.to_vec()).send().await.unwrap();

    // Heal partition
    cluster.heal_partition(0).await.unwrap();

    sleep(Duration::from_secs(3)).await;

    // Both writes should be visible
    assert_object_exists(&cluster, bucket, key1).await.unwrap();
    assert_object_exists(&cluster, bucket, key2).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_split_brain_prevention() {
    let mut cluster = TestCluster::new(4).await.unwrap(); // Even number for split
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "split-brain-test";
    let key = "test-key";
    let value = b"test-value";

    // Write initial data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Create 2-2 partition
    cluster.partition_node(0).await.unwrap();
    cluster.partition_node(1).await.unwrap();

    sleep(Duration::from_secs(3)).await;

    // Neither partition has quorum, so writes should fail or be delayed
    // The system should prevent split-brain

    // Heal partition
    cluster.heal_partition(0).await.unwrap();
    cluster.heal_partition(1).await.unwrap();

    sleep(Duration::from_secs(5)).await;

    // Original data should still be intact
    assert_object_content(&cluster, bucket, key, value).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_partition_isolation() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "isolation-test";

    // Partition one node completely
    cluster.partition_node(0).await.unwrap();

    // Write data to majority partition
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await.unwrap();
    }

    sleep(Duration::from_secs(2)).await;

    // Heal partition
    cluster.heal_partition(0).await.unwrap();

    // Wait for node to catch up
    sleep(Duration::from_secs(5)).await;

    // All data should be replicated to the previously partitioned node
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_intermittent_partition() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "intermittent-test";

    // Simulate flapping network
    for cycle in 0..5 {
        let key = format!("key-{}", cycle);
        let value = format!("value-{}", cycle).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        // Write data
        client.put(&url).body(value).send().await.unwrap();

        // Create partition
        cluster.partition_node(0).await.unwrap();
        sleep(Duration::from_millis(500)).await;

        // Heal partition
        cluster.heal_partition(0).await.unwrap();
        sleep(Duration::from_millis(500)).await;
    }

    // Wait for convergence
    sleep(Duration::from_secs(3)).await;

    // Verify all data is consistent
    for cycle in 0..5 {
        let key = format!("key-{}", cycle);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}
