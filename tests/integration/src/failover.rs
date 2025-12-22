//! Failover scenario tests
//!
//! Tests for various failure scenarios including:
//! - Leader failure and re-election
//! - Follower failures
//! - Network partitions
//! - Simultaneous failures
//! - Data consistency during failures

use crate::{random_bytes, setup_cluster, BucketFixture};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[serial]
async fn test_leader_failover() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-leader-failover").await;

    // Get current leader
    let initial_leader = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .unwrap();

    // Write some data before leader failure
    let key = "pre-failover-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Kill the leader
    cluster.kill_node(initial_leader as usize).await.unwrap();

    // Wait for new leader election
    let new_leader = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("New leader should be elected");

    assert_ne!(
        new_leader, initial_leader,
        "New leader should be different from killed leader"
    );

    // Verify pre-failover data is still accessible
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data written before failover should be accessible"
    );

    // Write new data after failover
    let new_key = "post-failover-object";
    let new_data = random_bytes(2048);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(new_key)
        .body(ByteStream::from(Bytes::from(new_data.clone())))
        .send()
        .await
        .unwrap();

    // Verify new data is readable
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(new_key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        new_data, retrieved_data,
        "New data written after failover should be accessible"
    );
}

#[tokio::test]
#[serial]
async fn test_follower_failure() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-follower-failure").await;

    // Get leader
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .unwrap();

    // Find a follower
    let follower_index = if leader_id == 0 { 1 } else { 0 };

    // Write data before failure
    let key = "before-follower-failure";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Kill a follower
    cluster.kill_node(follower_index).await.unwrap();

    // Wait a bit for cluster to detect failure
    sleep(Duration::from_secs(2)).await;

    // Leader should remain the same
    let current_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    assert_eq!(
        current_leader, leader_id,
        "Leader should not change when follower fails"
    );

    // Should still be able to read and write with 2/3 nodes
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Should be able to read after follower failure"
    );

    // Write new data
    let new_key = "after-follower-failure";
    let new_data = random_bytes(2048);

    let write_result = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(new_key)
        .body(ByteStream::from(Bytes::from(new_data)))
        .send()
        .await;

    assert!(
        write_result.is_ok(),
        "Should be able to write after follower failure"
    );
}

#[tokio::test]
#[serial]
async fn test_minority_failure() {
    let mut cluster = setup_cluster(5).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-minority").await;

    // Write initial data
    let key = "minority-failure-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Kill minority of nodes (2 out of 5)
    cluster.kill_node(3).await.unwrap();
    cluster.kill_node(4).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(3)).await;

    // Cluster should still be operational with 3/5 nodes (majority)
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await;

    assert!(
        get_result.is_ok(),
        "Should be able to read with majority (3/5 nodes)"
    );

    // Verify data integrity
    let retrieved_data = get_result
        .unwrap()
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data should be intact after minority failure"
    );

    // Should be able to write new data
    let new_key = "after-minority-failure";
    let write_result = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(new_key)
        .body(ByteStream::from(Bytes::from_static(b"new data")))
        .send()
        .await;

    assert!(
        write_result.is_ok(),
        "Should be able to write with majority"
    );
}

#[tokio::test]
#[serial]
async fn test_network_partition() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-partition").await;

    // Write data before partition
    let key = "partition-test-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Partition one node
    cluster.partition_node(2).await.unwrap();

    // Wait for cluster to detect partition
    sleep(Duration::from_secs(2)).await;

    // Majority partition (2/3) should still be operational
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await;

    assert!(
        get_result.is_ok(),
        "Should be able to read in majority partition"
    );

    // Heal the partition
    cluster.heal_partition(2).await.unwrap();

    // Wait for node to rejoin
    sleep(Duration::from_secs(3)).await;

    // Verify data is still accessible
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data should be accessible after partition heals"
    );
}

#[tokio::test]
#[serial]
async fn test_cascading_failure() {
    let mut cluster = setup_cluster(5).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-cascading").await;

    // Write initial data
    let key = "cascading-failure-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Kill nodes one by one
    // First failure (4/5 nodes remain)
    cluster.kill_node(4).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    // Should still be operational
    assert!(
        fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(key)
            .send()
            .await
            .is_ok(),
        "Should work with 4/5 nodes"
    );

    // Second failure (3/5 nodes remain)
    cluster.kill_node(3).await.unwrap();
    sleep(Duration::from_secs(1)).await;

    // Should still be operational (majority)
    assert!(
        fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(key)
            .send()
            .await
            .is_ok(),
        "Should work with 3/5 nodes (majority)"
    );

    // Verify data integrity
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data should remain intact through cascading failures"
    );
}

#[tokio::test]
#[serial]
async fn test_leader_failure_during_write() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-write-failure").await;

    // Get initial leader
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .unwrap();

    // Start multiple concurrent writes
    let mut write_handles = Vec::new();

    for i in 0..10 {
        let client = fixture.client.clone();
        let bucket = fixture.bucket.clone();
        let key = format!("concurrent-object-{}", i);
        let key_for_handle = key.clone();
        let data = random_bytes(1024);

        let handle = tokio::spawn(async move {
            client
                .put_object()
                .bucket(&bucket)
                .key(&key)
                .body(ByteStream::from(Bytes::from(data)))
                .send()
                .await
        });

        write_handles.push((key_for_handle, handle));
    }

    // Kill leader during writes
    sleep(Duration::from_millis(100)).await;
    cluster.kill_node(leader_id as usize).await.unwrap();

    // Wait for writes to complete (some may succeed, some may fail)
    let mut successful_writes = Vec::new();
    for (key, handle) in write_handles {
        if let Ok(Ok(_)) = handle.await {
            successful_writes.push(key);
        }
    }

    // Wait for new leader
    cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("New leader should be elected");

    sleep(Duration::from_secs(2)).await;

    // All successful writes should be readable
    for key in &successful_writes {
        let get_result = fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(key)
            .send()
            .await;

        assert!(
            get_result.is_ok(),
            "Successfully written object {} should be readable",
            key
        );
    }
}

#[tokio::test]
#[serial]
async fn test_split_brain_prevention() {
    let mut cluster = setup_cluster(5).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-split-brain").await;

    // Write initial data
    let key = "split-brain-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Create a partition (2 nodes vs 3 nodes)
    cluster.partition_node(3).await.unwrap();
    cluster.partition_node(4).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(3)).await;

    // The majority partition (3 nodes) should remain operational
    // The minority partition (2 nodes) should not accept writes

    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await;

    assert!(get_result.is_ok(), "Majority partition should allow reads");

    // Heal the partition
    cluster.heal_partition(3).await.unwrap();
    cluster.heal_partition(4).await.unwrap();

    sleep(Duration::from_secs(3)).await;

    // Verify data is consistent across all nodes
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data should be consistent after partition heals"
    );
}

#[tokio::test]
#[serial]
async fn test_data_durability_on_failure() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-durability").await;

    // Write multiple objects
    let mut keys_and_data = Vec::new();
    for i in 0..5 {
        let key = format!("durable-object-{}", i);
        let data = random_bytes(1024);

        fixture
            .client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data.clone())))
            .send()
            .await
            .unwrap();

        keys_and_data.push((key, data));
    }

    // Kill one node
    cluster.kill_node(2).await.unwrap();
    sleep(Duration::from_secs(2)).await;

    // Verify all data is still accessible
    for (key, expected_data) in &keys_and_data {
        let get_result = fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(key)
            .send()
            .await
            .unwrap();

        let retrieved_data = get_result
            .body
            .collect()
            .await
            .unwrap()
            .into_bytes()
            .to_vec();

        assert_eq!(
            expected_data, &retrieved_data,
            "Object {} should be durable after node failure",
            key
        );
    }

    // Restart the failed node
    cluster.restart_node(2).await.unwrap();
    sleep(Duration::from_secs(5)).await;

    // Verify all data is still accessible after node restart
    for (key, expected_data) in &keys_and_data {
        let get_result = fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(key)
            .send()
            .await
            .unwrap();

        let retrieved_data = get_result
            .body
            .collect()
            .await
            .unwrap()
            .into_bytes()
            .to_vec();

        assert_eq!(
            expected_data, &retrieved_data,
            "Object {} should be accessible after node restart",
            key
        );
    }
}

#[tokio::test]
#[serial]
async fn test_rapid_leader_changes() {
    let mut cluster = setup_cluster(5).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-rapid-changes").await;

    // Write initial data
    let key = "rapid-change-object";
    let data = random_bytes(1024);

    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Cause rapid leader changes by killing leaders sequentially
    for _ in 0..2 {
        let leader_id = cluster
            .wait_for_leader(Duration::from_secs(30))
            .await
            .unwrap();

        cluster.kill_node(leader_id as usize).await.unwrap();
        sleep(Duration::from_secs(2)).await;
    }

    // Wait for cluster to stabilize
    cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("Cluster should stabilize with a leader");

    sleep(Duration::from_secs(2)).await;

    // Verify data is still accessible and intact
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Data should survive rapid leader changes"
    );
}
