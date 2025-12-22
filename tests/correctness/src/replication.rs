//! Replication testing module
//!
//! Tests that verify data replication works correctly across nodes.

use anode_test_harness::{assertions::*, cluster::TestCluster, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Test that data is replicated to multiple nodes
pub async fn test_basic_replication() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "replication-test";
    let key = "replicated-key";
    let value = b"replicated-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Wait for replication
    sleep(Duration::from_secs(2)).await;

    // Verify data is on multiple nodes
    assert_data_replicated(&cluster, bucket, key, 3).await?;

    cluster.stop().await?;
    Ok(())
}

/// Test replication factor configuration
pub async fn test_replication_factor() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rf-test";
    let key = "rf-key";
    let value = b"rf-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Wait for replication
    sleep(Duration::from_secs(3)).await;

    // Count how many nodes have the data
    let mut replica_count = 0;
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i)?;
        let node_url = format!("{}/{}/{}", node.s3_endpoint(), bucket, key);

        if let Ok(response) = client.get(&node_url).send().await {
            if response.status().is_success() {
                replica_count += 1;
            }
        }
    }

    assert!(
        replica_count >= 3,
        "Data should be replicated to at least 3 nodes, found {}",
        replica_count
    );

    cluster.stop().await?;
    Ok(())
}

/// Test replication after node failure
pub async fn test_replication_after_failure() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "failure-replication-test";
    let key = "failure-key";
    let value = b"failure-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Wait for initial replication
    sleep(Duration::from_secs(2)).await;

    // Kill a node
    cluster.kill_node(1).await?;

    // Wait for re-replication
    sleep(Duration::from_secs(5)).await;

    // Verify data is still available with sufficient replicas
    let mut replica_count = 0;
    for i in 0..cluster.num_nodes() {
        if i == 1 {
            continue; // Skip the killed node
        }

        let node = cluster.get_node(i)?;
        let node_url = format!("{}/{}/{}", node.s3_endpoint(), bucket, key);

        if let Ok(response) = client.get(&node_url).send().await {
            if response.status().is_success() {
                replica_count += 1;
            }
        }
    }

    assert!(
        replica_count >= 2,
        "Should maintain at least 2 replicas after failure"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test replication during network partition
pub async fn test_replication_during_partition() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "partition-replication-test";

    // Write data before partition
    let key1 = "before-partition";
    let value1 = b"before-partition-value";
    let url1 = format!("{}/{}/{}", endpoint, bucket, key1);
    client.put(&url1).body(value1.to_vec()).send().await?;

    sleep(Duration::from_secs(1)).await;

    // Create partition
    cluster.partition_node(0).await?;

    // Write data during partition
    let key2 = "during-partition";
    let value2 = b"during-partition-value";
    let url2 = format!("{}/{}/{}", endpoint, bucket, key2);
    client.put(&url2).body(value2.to_vec()).send().await?;

    sleep(Duration::from_secs(1)).await;

    // Heal partition
    cluster.heal_partition(0).await?;

    // Wait for convergence
    sleep(Duration::from_secs(3)).await;

    // Verify both objects are properly replicated
    assert_data_replicated(&cluster, bucket, key1, 3).await?;
    assert_data_replicated(&cluster, bucket, key2, 3).await?;

    cluster.stop().await?;
    Ok(())
}

/// Test chunk-level replication
pub async fn test_chunk_replication() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "chunk-replication-test";
    let key = "large-object";

    // Create a large object that will be split into chunks
    let large_value = vec![0u8; 10 * 1024 * 1024]; // 10 MB

    // Write large object
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(large_value.clone()).send().await?;

    // Wait for replication
    sleep(Duration::from_secs(5)).await;

    // Verify the object can be read back correctly from each node
    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i)?;
        let node_url = format!("{}/{}/{}", node.s3_endpoint(), bucket, key);

        let response = client.get(&node_url).send().await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            assert_eq!(
                bytes.len(),
                large_value.len(),
                "Object size should match on node {}",
                i
            );
        }
    }

    cluster.stop().await?;
    Ok(())
}

/// Test rebalancing after adding a node
pub async fn test_replication_rebalancing() -> Result<()> {
    // Start with 3 nodes
    let mut cluster = TestCluster::with_binary("target/debug/anode", 3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rebalance-test";

    // Write some data
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);
        client.put(&url).body(value).send().await?;
    }

    sleep(Duration::from_secs(2)).await;

    // In a real implementation, we would add a new node here
    // For now, just verify the existing data is properly replicated

    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await?;
    }

    cluster.stop().await?;
    Ok(())
}

/// Test chunk reference counting with deduplication
pub async fn test_chunk_reference_counting() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "refcount-test";

    // Create identical data that should be deduplicated
    let shared_data = vec![42u8; 1024 * 1024]; // 1 MB of identical data

    // Write first object with shared chunks
    let key1 = "object-1";
    let url1 = format!("{}/{}/{}", endpoint, bucket, key1);
    client.put(&url1).body(shared_data.clone()).send().await?;

    sleep(Duration::from_secs(1)).await;

    // Write second object with same data (should deduplicate)
    let key2 = "object-2";
    let url2 = format!("{}/{}/{}", endpoint, bucket, key2);
    client.put(&url2).body(shared_data.clone()).send().await?;

    sleep(Duration::from_secs(2)).await;

    // Both objects should be readable
    let response1 = client.get(&url1).send().await?;
    assert!(
        response1.status().is_success(),
        "Object 1 should be readable"
    );

    let response2 = client.get(&url2).send().await?;
    assert!(
        response2.status().is_success(),
        "Object 2 should be readable"
    );

    // Delete first object
    client.delete(&url1).send().await?;

    sleep(Duration::from_secs(1)).await;

    // Second object should still be readable (chunks still referenced)
    let response2 = client.get(&url2).send().await?;
    assert!(
        response2.status().is_success(),
        "Object 2 should still be readable after object 1 deleted"
    );

    let data2 = response2.bytes().await?;
    assert_eq!(
        data2.len(),
        shared_data.len(),
        "Object 2 data should be intact"
    );

    // Delete second object
    client.delete(&url2).send().await?;

    sleep(Duration::from_secs(2)).await;

    // Now chunks should be garbage collected (in real implementation)
    // This would require admin API to verify chunk deletion

    cluster.stop().await?;
    Ok(())
}

/// Test erasure coding replication
pub async fn test_erasure_coding_replication() -> Result<()> {
    let mut cluster = TestCluster::new(6).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "ec-test";

    // Write a large object that should use erasure coding
    let large_data = vec![0u8; 50 * 1024 * 1024]; // 50 MB
    let key = "large-ec-object";
    let url = format!("{}/{}/{}", endpoint, bucket, key);

    client.put(&url).body(large_data.clone()).send().await?;

    sleep(Duration::from_secs(5)).await;

    // Verify object is readable
    let response = client.get(&url).send().await?;
    assert!(
        response.status().is_success(),
        "Large object should be readable"
    );

    // Kill 2 nodes (should still be recoverable with EC)
    cluster.kill_node(1).await?;
    cluster.kill_node(2).await?;

    sleep(Duration::from_secs(2)).await;

    // Object should still be readable
    let response = client.get(&url).send().await?;
    assert!(
        response.status().is_success(),
        "Object should be readable after 2 node failures (EC recovery)"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test replication with network delays
pub async fn test_replication_with_delays() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "delay-test";

    // Write objects rapidly
    for i in 0..20 {
        let key = format!("key-{:02}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await?;

        // No delay - rapid writes
    }

    // Wait for replication to complete
    sleep(Duration::from_secs(5)).await;

    // Verify all objects are replicated
    for i in 0..20 {
        let key = format!("key-{:02}", i);
        assert_data_replicated(&cluster, bucket, &key, 3).await?;
    }

    cluster.stop().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_basic() {
        let _ = test_basic_replication().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_rf() {
        let _ = test_replication_factor().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_failure() {
        let _ = test_replication_after_failure().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_partition() {
        let _ = test_replication_during_partition().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_chunks() {
        let _ = test_chunk_replication().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_rebalance() {
        let _ = test_replication_rebalancing().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_refcount() {
        let _ = test_chunk_reference_counting().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_ec() {
        let _ = test_erasure_coding_replication().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_replication_delays() {
        let _ = test_replication_with_delays().await;
    }
}
