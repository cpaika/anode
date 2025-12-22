//! Correctness assertion helpers for distributed systems testing

use crate::{cluster::TestCluster, Result, TestHarnessError};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info};

/// Assert that an object exists in the cluster
pub async fn assert_object_exists(cluster: &TestCluster, bucket: &str, key: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/{}/{}", cluster.get_s3_endpoint(), bucket, key);

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            debug!("Object {}/{} exists", bucket, key);
            Ok(())
        },
        Ok(response) => Err(TestHarnessError::AssertionFailed(format!(
            "Object {}/{} does not exist: status {}",
            bucket,
            key,
            response.status()
        ))),
        Err(e) => Err(TestHarnessError::AssertionFailed(format!(
            "Failed to check object {}/{}: {}",
            bucket, key, e
        ))),
    }
}

/// Assert that an object has specific content
pub async fn assert_object_content(
    cluster: &TestCluster,
    bucket: &str,
    key: &str,
    expected: &[u8],
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/{}/{}", cluster.get_s3_endpoint(), bucket, key);

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => match response.bytes().await {
            Ok(bytes) if bytes.as_ref() == expected => {
                debug!("Object {}/{} has expected content", bucket, key);
                Ok(())
            },
            Ok(bytes) => Err(TestHarnessError::AssertionFailed(format!(
                "Object {}/{} content mismatch: expected {} bytes, got {} bytes",
                bucket,
                key,
                expected.len(),
                bytes.len()
            ))),
            Err(e) => Err(TestHarnessError::AssertionFailed(format!(
                "Failed to read object {}/{}: {}",
                bucket, key, e
            ))),
        },
        Ok(response) => Err(TestHarnessError::AssertionFailed(format!(
            "Object {}/{} does not exist: status {}",
            bucket,
            key,
            response.status()
        ))),
        Err(e) => Err(TestHarnessError::AssertionFailed(format!(
            "Failed to check object {}/{}: {}",
            bucket, key, e
        ))),
    }
}

/// Assert that an object does not exist
pub async fn assert_object_not_exists(
    cluster: &TestCluster,
    bucket: &str,
    key: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/{}/{}", cluster.get_s3_endpoint(), bucket, key);

    match client.get(&url).send().await {
        Ok(response) if response.status().as_u16() == 404 => {
            debug!("Object {}/{} does not exist (as expected)", bucket, key);
            Ok(())
        },
        Ok(response) if response.status().is_success() => Err(TestHarnessError::AssertionFailed(
            format!("Object {}/{} exists but should not", bucket, key),
        )),
        Ok(response) => Err(TestHarnessError::AssertionFailed(format!(
            "Unexpected status checking object {}/{}: {}",
            bucket,
            key,
            response.status()
        ))),
        Err(e) => Err(TestHarnessError::AssertionFailed(format!(
            "Failed to check object {}/{}: {}",
            bucket, key, e
        ))),
    }
}

/// Assert that a bucket exists
pub async fn assert_bucket_exists(cluster: &TestCluster, bucket: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/{}", cluster.get_s3_endpoint(), bucket);

    match client.head(&url).send().await {
        Ok(response) if response.status().is_success() => {
            debug!("Bucket {} exists", bucket);
            Ok(())
        },
        Ok(response) => Err(TestHarnessError::AssertionFailed(format!(
            "Bucket {} does not exist: status {}",
            bucket,
            response.status()
        ))),
        Err(e) => Err(TestHarnessError::AssertionFailed(format!(
            "Failed to check bucket {}: {}",
            bucket, e
        ))),
    }
}

/// Assert that the cluster is healthy
pub async fn assert_cluster_healthy(cluster: &TestCluster) -> Result<()> {
    let client = reqwest::Client::new();

    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i)?;
        let url = format!("{}/health", node.admin_endpoint());

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                debug!("Node {} is healthy", node.id());
            },
            Ok(response) => {
                return Err(TestHarnessError::AssertionFailed(format!(
                    "Node {} is unhealthy: status {}",
                    node.id(),
                    response.status()
                )));
            },
            Err(e) => {
                return Err(TestHarnessError::AssertionFailed(format!(
                    "Failed to check health of node {}: {}",
                    node.id(),
                    e
                )));
            },
        }
    }

    info!("All nodes are healthy");
    Ok(())
}

/// Assert that data is replicated to the expected number of nodes
pub async fn assert_data_replicated(
    cluster: &TestCluster,
    bucket: &str,
    key: &str,
    expected_replicas: usize,
) -> Result<()> {
    let client = reqwest::Client::new();
    let mut replica_count = 0;

    for i in 0..cluster.num_nodes() {
        let node = cluster.get_node(i)?;
        if node.is_partitioned() {
            continue;
        }

        let url = format!("{}/{}/{}", node.s3_endpoint(), bucket, key);

        match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                replica_count += 1;
            },
            _ => {},
        }
    }

    if replica_count >= expected_replicas {
        debug!(
            "Object {}/{} replicated to {} nodes (expected {})",
            bucket, key, replica_count, expected_replicas
        );
        Ok(())
    } else {
        Err(TestHarnessError::AssertionFailed(format!(
            "Object {}/{} replicated to {} nodes, expected {}",
            bucket, key, replica_count, expected_replicas
        )))
    }
}

/// Wait for a leader to be elected and return the leader ID
pub async fn assert_leader_elected(
    cluster: &TestCluster,
    timeout: Duration,
) -> Result<crate::NodeId> {
    cluster.wait_for_leader(timeout).await
}

/// Assert that a leader exists
pub async fn assert_has_leader(cluster: &TestCluster) -> Result<()> {
    cluster.wait_for_leader(Duration::from_secs(10)).await?;
    Ok(())
}

/// Assert that eventually a condition becomes true
pub async fn assert_eventually<F, Fut>(
    condition: F,
    timeout: Duration,
    check_interval: Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        sleep(check_interval).await;
    }

    Err(TestHarnessError::AssertionFailed(
        "Condition did not become true within timeout".to_string(),
    ))
}

/// Assert that a condition remains true for a duration
pub async fn assert_always<F, Fut>(
    condition: F,
    duration: Duration,
    check_interval: Duration,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();

    while start.elapsed() < duration {
        if !condition().await {
            return Err(TestHarnessError::AssertionFailed(
                "Condition became false".to_string(),
            ));
        }
        sleep(check_interval).await;
    }

    Ok(())
}

/// Assert that no errors occurred during operation
pub async fn assert_no_errors<T, E: std::fmt::Display>(
    result: std::result::Result<T, E>,
) -> Result<T> {
    result.map_err(|e| TestHarnessError::AssertionFailed(format!("Operation failed: {}", e)))
}

/// Wait for a condition with retries
pub async fn wait_for<F, Fut, T>(
    mut operation: F,
    timeout: Duration,
    retry_interval: Duration,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let start = std::time::Instant::now();
    let mut last_error = None;

    while start.elapsed() < timeout {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                last_error = Some(e);
                sleep(retry_interval).await;
            },
        }
    }

    Err(last_error.unwrap_or_else(|| TestHarnessError::Timeout("Operation timed out".to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_assert_eventually() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();

        // Spawn a task that increments the counter after 100ms
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            counter_clone.store(1, std::sync::atomic::Ordering::SeqCst);
        });

        let result = assert_eventually(
            || {
                let c = counter.clone();
                async move { c.load(std::sync::atomic::Ordering::SeqCst) == 1 }
            },
            Duration::from_secs(1),
            Duration::from_millis(10),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_assert_always() {
        let result = assert_always(
            || async { true },
            Duration::from_millis(100),
            Duration::from_millis(10),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_assert_no_errors() {
        let result: std::result::Result<i32, &str> = Ok(42);
        let value = assert_no_errors(result).await;
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), 42);
    }
}
