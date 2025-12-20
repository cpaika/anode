//! Node crash chaos tests
//!
//! Tests system behavior when nodes crash unexpectedly

use anode_chaos_tests::{scenarios, utils};
use anode_test_harness::{
    assertions::*,
    cluster::TestCluster,
    workload::{Operation, SequentialWorkload, Workload},
};
use std::time::Duration;

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_single_node_crash() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "crash-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Crash a follower node
    cluster.kill_node(1).await.unwrap();

    // System should still be operational
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify data is still accessible
    assert_object_exists(&cluster, bucket, key).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_leader_crash_and_recovery() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for initial leader election
    let leader_id = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();

    // Kill the leader
    cluster.kill_node(leader_id as usize).await.unwrap();

    // New leader should be elected
    let new_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    assert_ne!(leader_id, new_leader, "New leader should be elected");

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_crash_during_writes() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "crash-writes-test";

    // Start writing data
    let write_task = tokio::spawn({
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        async move {
            let workload = SequentialWorkload::new(bucket.clone(), 1024);

            for _ in 0..100 {
                let op = workload.generate_operation().await;

                if let Operation::Put { bucket, key, value } = op {
                    let url = format!("{}/{}/{}", endpoint, bucket, key);
                    let _ = reqwest::Client::new().put(&url).body(value).send().await;
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    });

    // Crash a node after some writes
    tokio::time::sleep(Duration::from_millis(200)).await;
    cluster.kill_node(2).await.unwrap();

    // Wait for writes to complete
    write_task.await.unwrap();

    // Verify cluster is still healthy
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_multiple_concurrent_crashes() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "multi-crash-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Crash two nodes simultaneously (but maintain quorum)
    cluster.kill_node(0).await.unwrap();
    cluster.kill_node(1).await.unwrap();

    // System should still have quorum and be operational
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Verify data is still accessible
    assert_object_exists(&cluster, bucket, key).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_crash_and_restart_cycle() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "cycle-test";

    // Perform multiple crash-restart cycles
    for cycle in 0..3 {
        let key = format!("key-{}", cycle);
        let value = format!("value-{}", cycle).into_bytes();

        // Write data
        let url = format!("{}/{}/{}", endpoint, bucket, &key);
        client.put(&url).body(value.clone()).send().await.unwrap();

        // Crash a node
        let node_idx = cycle % 3;
        cluster.kill_node(node_idx).await.unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        // Restart the node
        cluster.restart_node(node_idx).await.unwrap();

        tokio::time::sleep(Duration::from_secs(2)).await;

        // Verify data is still accessible
        assert_object_content(&cluster, bucket, &key, &value).await.unwrap();
    }

    cluster.stop().await.unwrap();
}
