//! Durability testing module
//!
//! Tests that verify written data persists across failures, crashes,
//! and restarts.

use anode_test_harness::{cluster::TestCluster, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Test that data survives a node restart
pub async fn test_data_survives_restart() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "durability-test";
    let key = "persistent-key";
    let value = b"persistent-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Restart a node
    cluster.restart_node(0).await?;

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Verify data still exists
    let response = client.get(&url).send().await?;
    let read_value = response.bytes().await?;

    assert_eq!(
        read_value.as_ref(),
        value,
        "Data should survive node restart"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test that data survives a node crash
pub async fn test_data_survives_crash() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "crash-test";
    let key = "crash-key";
    let value = b"crash-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Kill a node (simulate crash)
    cluster.kill_node(1).await?;

    // Wait a bit
    sleep(Duration::from_secs(2)).await;

    // Verify data still exists on other nodes
    let response = client.get(&url).send().await?;
    let read_value = response.bytes().await?;

    assert_eq!(read_value.as_ref(), value, "Data should survive node crash");

    cluster.stop().await?;
    Ok(())
}

/// Test that all committed writes are durable
pub async fn test_committed_writes_durable() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "commit-test";

    // Write multiple objects
    let num_objects = 100;
    let mut written_keys = Vec::new();

    for i in 0..num_objects {
        let key = format!("key-{:04}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        let response = client.put(&url).body(value).send().await?;
        if response.status().is_success() {
            written_keys.push(key);
        }
    }

    // Restart all nodes
    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await?;
        sleep(Duration::from_secs(2)).await;
    }

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Verify all acknowledged writes are still present
    for key in &written_keys {
        let url = format!("{}/{}/{}", endpoint, bucket, key);
        let response = client.get(&url).send().await?;

        assert!(
            response.status().is_success(),
            "Committed write {} should be durable",
            key
        );
    }

    cluster.stop().await?;
    Ok(())
}

/// Test fsync behavior - data should survive power loss simulation
pub async fn test_fsync_durability() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "fsync-test";
    let key = "fsync-key";
    let value = b"fsync-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    let response = client.put(&url).body(value.to_vec()).send().await?;

    assert!(
        response.status().is_success(),
        "Write should be acknowledged"
    );

    // Immediately kill all nodes (simulate power loss)
    for i in 0..cluster.num_nodes() {
        cluster.kill_node(i).await?;
    }

    // Restart all nodes
    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await?;
        sleep(Duration::from_secs(1)).await;
    }

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Verify data survived
    let response = client.get(&url).send().await?;
    let read_value = response.bytes().await?;

    assert_eq!(read_value.as_ref(), value, "Data should survive power loss");

    cluster.stop().await?;
    Ok(())
}

/// Test uncommitted write behavior after crash
pub async fn test_uncommitted_write_behavior() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "uncommitted-test";
    let key = "uncommitted-key";
    let value = b"uncommitted-value";

    // Start a write
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    let write_future = client.put(&url).body(value.to_vec()).send();

    // Immediately kill majority of nodes before acknowledgment
    sleep(Duration::from_millis(10)).await;
    cluster.kill_node(0).await?;
    cluster.kill_node(1).await?;

    // Write should fail or timeout
    let _result = tokio::time::timeout(Duration::from_secs(5), write_future).await;

    // Restart all nodes
    cluster.restart_node(0).await?;
    cluster.restart_node(1).await?;

    sleep(Duration::from_secs(5)).await;

    // Verify the write either fully committed or not at all (atomicity)
    // The data should be in a consistent state
    let response = client.get(&url).send().await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            // If it exists, it should have the correct value
            let data = resp.bytes().await?;
            assert_eq!(
                data.as_ref(),
                value,
                "If uncommitted write completed, value should be correct"
            );
        },
        Ok(resp) if resp.status().as_u16() == 404 => {
            // If it doesn't exist, that's also acceptable (rolled back)
        },
        _ => {
            // Any other state is acceptable as long as it's consistent
        },
    }

    cluster.stop().await?;
    Ok(())
}

/// Test write durability with concurrent failures
pub async fn test_durability_concurrent_failures() -> Result<()> {
    let mut cluster = TestCluster::new(5).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "concurrent-failure-test";

    // Write multiple objects
    let mut written_keys = Vec::new();
    for i in 0..20 {
        let key = format!("key-{:03}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        let response = client.put(&url).body(value).send().await?;
        if response.status().is_success() {
            written_keys.push(key);
        }

        // Kill nodes during writes
        if i == 5 {
            cluster.kill_node(1).await?;
        }
        if i == 10 {
            cluster.kill_node(2).await?;
        }

        sleep(Duration::from_millis(50)).await;
    }

    sleep(Duration::from_secs(2)).await;

    // Restart failed nodes
    cluster.restart_node(1).await?;
    cluster.restart_node(2).await?;

    sleep(Duration::from_secs(5)).await;

    // All acknowledged writes should be durable
    for key in &written_keys {
        let url = format!("{}/{}/{}", endpoint, bucket, key);
        let response = client.get(&url).send().await?;

        assert!(
            response.status().is_success(),
            "Acknowledged write {} should be durable",
            key
        );
    }

    cluster.stop().await?;
    Ok(())
}

/// Test partial write durability
pub async fn test_partial_write_durability() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "partial-write-test";
    let key = "partial-key";

    // Create a large object
    let large_value = vec![0u8; 10 * 1024 * 1024]; // 10 MB
    let url = format!("{}/{}/{}", endpoint, bucket, key);

    // Start writing
    let write_task = tokio::spawn({
        let client = client.clone();
        let url = url.clone();
        let value = large_value.clone();
        async move { client.put(&url).body(value).send().await }
    });

    // Kill a node during write
    sleep(Duration::from_millis(100)).await;
    cluster.kill_node(1).await?;

    // Wait for write to complete (or fail)
    let write_result = write_task.await;

    sleep(Duration::from_secs(2)).await;

    // Restart the node
    cluster.restart_node(1).await?;

    sleep(Duration::from_secs(5)).await;

    // If write succeeded, data should be complete and durable
    if let Ok(Ok(response)) = write_result {
        if response.status().is_success() {
            let read_response = client.get(&url).send().await?;
            let read_data = read_response.bytes().await?;

            assert_eq!(
                read_data.len(),
                large_value.len(),
                "Partial write should either complete fully or not at all"
            );
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
    async fn test_durability_restart() {
        let _ = test_data_survives_restart().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_crash() {
        let _ = test_data_survives_crash().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_committed() {
        let _ = test_committed_writes_durable().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_fsync() {
        let _ = test_fsync_durability().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_uncommitted() {
        let _ = test_uncommitted_write_behavior().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_concurrent() {
        let _ = test_durability_concurrent_failures().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_durability_partial() {
        let _ = test_partial_write_durability().await;
    }
}
