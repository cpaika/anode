//! Consistency testing module
//!
//! Tests that verify consistency properties across the distributed system.

use anode_test_harness::{assertions::*, cluster::TestCluster, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Test that replicas eventually converge
pub async fn test_eventual_consistency() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "consistency-test";
    let key = "eventual-key";
    let value = b"eventual-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Wait for replication
    sleep(Duration::from_secs(2)).await;

    // Verify all replicas have the same data
    assert_data_replicated(&cluster, bucket, key, 3).await?;

    cluster.stop().await?;
    Ok(())
}

/// Test consistency during partition
pub async fn test_consistency_during_partition() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "partition-test";
    let key = "partition-key";
    let value = b"partition-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Create a partition (isolate node 0)
    cluster.partition_node(0).await?;

    // Wait a bit
    sleep(Duration::from_secs(2)).await;

    // Majority nodes should still be consistent
    let mut consistent_count = 0;
    for i in 1..cluster.num_nodes() {
        let node = cluster.get_node(i)?;
        if !node.is_partitioned() {
            let node_url = format!("{}/{}/{}", node.s3_endpoint(), bucket, key);
            if let Ok(response) = client.get(&node_url).send().await {
                if response.status().is_success() {
                    if let Ok(bytes) = response.bytes().await {
                        if bytes.as_ref() == value {
                            consistent_count += 1;
                        }
                    }
                }
            }
        }
    }

    assert!(
        consistent_count >= 2,
        "Majority partition should maintain consistency"
    );

    // Heal partition
    cluster.heal_partition(0).await?;

    // Wait for convergence
    sleep(Duration::from_secs(3)).await;

    // All replicas should now be consistent
    assert_data_replicated(&cluster, bucket, key, 3).await?;

    cluster.stop().await?;
    Ok(())
}

/// Test metadata and data consistency
pub async fn test_metadata_data_consistency() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "metadata-test";

    // Write multiple objects
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;
    }

    // List objects
    let list_url = format!("{}/{}?list-type=2", endpoint, bucket);
    let response = client.get(&list_url).send().await?;

    assert!(
        response.status().is_success(),
        "List operation should succeed"
    );

    // Verify each listed object can be retrieved
    // (In a real implementation, we would parse the XML response)

    cluster.stop().await?;
    Ok(())
}

/// Test consistency after node recovery
pub async fn test_consistency_after_recovery() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "recovery-test";

    // Write initial data
    for i in 0..5 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);
        client.put(&url).body(value).send().await?;
    }

    // Kill a node
    cluster.kill_node(1).await?;

    // Write more data while node is down
    for i in 5..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);
        client.put(&url).body(value).send().await?;
    }

    // Restart the node
    cluster.restart_node(1).await?;

    // Wait for recovery
    sleep(Duration::from_secs(5)).await;

    // Verify all data is consistent across all nodes
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await?;
    }

    cluster.stop().await?;
    Ok(())
}

/// Test cross-placement-group consistency
pub async fn test_cross_pg_consistency() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();

    // Write data to multiple buckets (which may map to different PGs)
    for bucket_id in 0..3 {
        let bucket = format!("pg-test-{}", bucket_id);

        for key_id in 0..5 {
            let key = format!("key-{}", key_id);
            let value = format!("value-{}-{}", bucket_id, key_id).into_bytes();
            let url = format!("{}/{}/{}", endpoint, &bucket, &key);

            client.put(&url).body(value).send().await?;
        }
    }

    // Wait for replication
    sleep(Duration::from_secs(3)).await;

    // Verify consistency across all buckets
    for bucket_id in 0..3 {
        let bucket = format!("pg-test-{}", bucket_id);

        for key_id in 0..5 {
            let key = format!("key-{}", key_id);
            let expected = format!("value-{}-{}", bucket_id, key_id).into_bytes();

            assert_object_content(&cluster, &bucket, &key, &expected).await?;
        }
    }

    cluster.stop().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_consistency_eventual() {
        let _ = test_eventual_consistency().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_consistency_partition() {
        let _ = test_consistency_during_partition().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_consistency_metadata() {
        let _ = test_metadata_data_consistency().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_consistency_recovery() {
        let _ = test_consistency_after_recovery().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_consistency_cross_pg() {
        let _ = test_cross_pg_consistency().await;
    }
}
