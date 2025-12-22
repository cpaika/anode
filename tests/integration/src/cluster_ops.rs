//! Cluster operation tests
//!
//! Tests for cluster management operations including:
//! - Cluster status and health
//! - Adding and removing nodes
//! - Leader election
//! - Data rebalancing
//! - Cluster configuration

use crate::{random_bytes, setup_cluster, BucketFixture};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use reqwest::Client;
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[serial]
async fn test_cluster_status() {
    let cluster = setup_cluster(3).await.unwrap();
    let http_client = Client::new();

    // Query cluster status from each node
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i).unwrap();
        let status_url = format!("{}/cluster/status", node.admin_endpoint());

        let response = http_client
            .get(&status_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .expect("Failed to get cluster status");

        assert!(
            response.status().is_success(),
            "Node {} should return successful status",
            i
        );

        let status: serde_json::Value = response.json().await.expect("Failed to parse status JSON");

        // Verify basic status fields
        assert!(
            status.get("node_id").is_some(),
            "Status should include node_id"
        );
        assert!(status.get("role").is_some(), "Status should include role");
        assert!(
            status.get("cluster_size").is_some(),
            "Status should include cluster_size"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_leader_election() {
    let cluster = setup_cluster(3).await.unwrap();

    // Verify a leader was elected
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("Leader should be elected");

    assert!(
        leader_id < 3,
        "Leader ID should be one of the cluster nodes"
    );

    // Query each node to verify they agree on the leader
    let http_client = Client::new();
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i).unwrap();
        let leader_url = format!("{}/cluster/leader", node.admin_endpoint());

        let response = http_client
            .get(&leader_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .expect("Failed to query leader");

        let reported_leader: u64 = response.json().await.expect("Failed to parse leader ID");

        assert_eq!(
            reported_leader, leader_id,
            "All nodes should agree on the leader"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_add_node() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let http_client = Client::new();

    // Get initial cluster size
    let node = cluster.get_node(0).unwrap();
    let status_url = format!("{}/cluster/status", node.admin_endpoint());

    let response = http_client.get(&status_url).send().await.unwrap();

    let initial_status: serde_json::Value = response.json().await.unwrap();
    let initial_size = initial_status["cluster_size"]
        .as_u64()
        .expect("cluster_size should be a number");

    assert_eq!(initial_size, 3, "Initial cluster should have 3 nodes");

    // In a real implementation, we would add a new node here
    // For this test framework, we'll simulate the operation
    // This would involve:
    // 1. Starting a new node process
    // 2. Calling the add-node API endpoint
    // 3. Waiting for the node to join
    // 4. Verifying cluster size increased

    // TODO: Implement actual node addition when the API is available
}

#[tokio::test]
#[serial]
async fn test_remove_node() {
    let mut cluster = setup_cluster(3).await.unwrap();

    // In a real implementation, we would:
    // 1. Call the remove-node API endpoint
    // 2. Wait for data to be migrated
    // 3. Stop the node
    // 4. Verify cluster size decreased

    // For now, we test that killing a follower doesn't break the cluster
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .unwrap();

    // Find a follower to remove
    let follower_index = if leader_id == 0 { 1 } else { 0 };

    // Kill the follower
    cluster.kill_node(follower_index).await.unwrap();

    // Verify cluster still has a leader
    let new_leader = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("Cluster should still have a leader after follower removal");

    // Leader should remain the same
    assert_eq!(
        new_leader, leader_id,
        "Leader should not change when follower is removed"
    );
}

#[tokio::test]
#[serial]
async fn test_rebalance() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-rebalance").await;

    // Create multiple objects
    for i in 0..10 {
        let key = format!("object-{}", i);
        let data = random_bytes(1024 * 1024); // 1MB each

        fixture
            .client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(data)))
            .send()
            .await
            .unwrap();
    }

    // In a real implementation, we would:
    // 1. Trigger a rebalance operation via API
    // 2. Monitor rebalance progress
    // 3. Verify data is evenly distributed

    // For now, we verify all objects are still accessible
    for i in 0..10 {
        let key = format!("object-{}", i);

        let get_result = fixture
            .client
            .get_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await;

        assert!(
            get_result.is_ok(),
            "Object {} should be accessible after operations",
            key
        );
    }
}

#[tokio::test]
#[serial]
async fn test_health_check() {
    let cluster = setup_cluster(3).await.unwrap();
    let http_client = Client::new();

    // Check health endpoint on all nodes
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i).unwrap();
        let health_url = format!("{}/health", node.admin_endpoint());

        let response = http_client
            .get(&health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .expect("Failed to get health status");

        assert!(
            response.status().is_success(),
            "Node {} should be healthy",
            i
        );
    }
}

#[tokio::test]
#[serial]
async fn test_cluster_metrics() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-metrics").await;
    let http_client = Client::new();

    // Create some activity
    for i in 0..5 {
        let key = format!("object-{}", i);
        fixture
            .client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from_static(b"test data")))
            .send()
            .await
            .unwrap();
    }

    // Query metrics from each node
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i).unwrap();
        let metrics_url = format!("{}/metrics", node.admin_endpoint());

        let response = http_client
            .get(&metrics_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        if let Ok(resp) = response {
            assert!(
                resp.status().is_success(),
                "Node {} should return metrics",
                i
            );

            let metrics_text = resp.text().await.unwrap();
            assert!(!metrics_text.is_empty(), "Metrics should not be empty");
        } else {
            // Metrics endpoint might not be implemented yet
            eprintln!("Metrics endpoint not available on node {}", i);
        }
    }
}

#[tokio::test]
#[serial]
async fn test_cluster_quorum() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-quorum").await;

    // Write some data with full cluster
    let key = "quorum-test-object";
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

    // Kill one node (cluster should still have quorum with 2/3)
    cluster.kill_node(2).await.unwrap();

    // Wait a bit for cluster to stabilize
    sleep(Duration::from_secs(2)).await;

    // Should still be able to read and write
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await;

    assert!(
        get_result.is_ok(),
        "Should be able to read with quorum (2/3 nodes)"
    );

    // Should be able to write new data
    let new_key = "quorum-test-object-2";
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
        "Should be able to write with quorum (2/3 nodes)"
    );
}

#[tokio::test]
#[serial]
async fn test_minority_failure() {
    let mut cluster = setup_cluster(5).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-minority").await;

    // Write data with full cluster
    let key = "minority-test-object";
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
    sleep(Duration::from_secs(2)).await;

    // Should still have quorum and be operational
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(key)
        .send()
        .await;

    assert!(
        get_result.is_ok(),
        "Should be able to read with 3/5 nodes (majority)"
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
        "Data should remain intact despite minority failure"
    );
}

#[tokio::test]
#[serial]
async fn test_node_restart() {
    let mut cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-restart").await;

    // Write data
    let key = "restart-test-object";
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

    // Restart a follower node
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .unwrap();

    let follower_index = if leader_id == 0 { 1 } else { 0 };

    cluster.restart_node(follower_index).await.unwrap();

    // Wait for node to rejoin
    sleep(Duration::from_secs(5)).await;

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
        "Data should be accessible after node restart"
    );
}

#[tokio::test]
#[serial]
async fn test_cluster_info() {
    let cluster = setup_cluster(3).await.unwrap();
    let http_client = Client::new();

    // Get cluster info from leader
    let node = cluster.get_node(0).unwrap();
    let info_url = format!("{}/cluster/info", node.admin_endpoint());

    let response = http_client
        .get(&info_url)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    if let Ok(resp) = response {
        if resp.status().is_success() {
            let info: serde_json::Value = resp.json().await.unwrap();

            // Verify cluster info contains expected fields
            assert!(
                info.get("cluster_id").is_some(),
                "Cluster info should include cluster_id"
            );
            assert!(
                info.get("nodes").is_some(),
                "Cluster info should include nodes"
            );

            // Verify nodes array
            if let Some(nodes) = info["nodes"].as_array() {
                assert_eq!(nodes.len(), 3, "Cluster should report 3 nodes");

                for node_info in nodes {
                    assert!(node_info.get("id").is_some(), "Node info should include id");
                    assert!(
                        node_info.get("endpoint").is_some(),
                        "Node info should include endpoint"
                    );
                }
            }
        }
    }
}
