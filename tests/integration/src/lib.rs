//! Anode Integration Test Suite
//!
//! Comprehensive integration tests for the Anode distributed object storage system.
//! These tests verify end-to-end functionality including S3 API compatibility,
//! cluster operations, failover scenarios, and performance characteristics.

pub mod cluster_ops;
pub mod failover;
pub mod https_connections;
pub mod parquet;
pub mod performance;
pub mod s3_basic;
pub mod s3_multipart;

use anode_test_harness::{TestCluster, TestHarnessError};
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::Client as S3Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Global test cluster instance
static TEST_CLUSTER: OnceCell<Arc<TestCluster>> = OnceCell::const_new();

/// Initialize logging for tests
pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,anode=debug")),
        )
        .with_test_writer()
        .try_init();
}

/// Setup a test cluster with the specified number of nodes
pub async fn setup_cluster(num_nodes: usize) -> Result<TestCluster, TestHarnessError> {
    init_logging();

    info!("Setting up test cluster with {} nodes", num_nodes);

    let mut cluster = TestCluster::new(num_nodes).await?;
    cluster.start().await?;

    // Wait for cluster to stabilize and elect a leader
    match cluster.wait_for_leader(Duration::from_secs(30)).await {
        Ok(leader_id) => {
            info!("Cluster ready with leader: {}", leader_id);
        },
        Err(e) => {
            error!("Failed to wait for leader: {}", e);
            cluster.stop().await?;
            return Err(e);
        },
    }

    Ok(cluster)
}

/// Get or create the shared test cluster (3 nodes)
pub async fn get_shared_cluster() -> Arc<TestCluster> {
    TEST_CLUSTER
        .get_or_init(|| async {
            match setup_cluster(3).await {
                Ok(cluster) => Arc::new(cluster),
                Err(e) => {
                    panic!("Failed to setup shared test cluster: {}", e);
                },
            }
        })
        .await
        .clone()
}

/// Create an S3 client configured for the test cluster
pub async fn create_s3_client(endpoint: &str) -> S3Client {
    let credentials = Credentials::new(
        "test-access-key",
        "test-secret-key",
        None,
        None,
        "test-provider",
    );

    let config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(aws_config::Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .load()
        .await;

    S3Client::new(&config)
}

/// Create an S3 client for the test cluster
pub async fn create_test_s3_client(cluster: &TestCluster) -> S3Client {
    create_s3_client(&cluster.get_s3_endpoint()).await
}

/// Generate random bytes for testing
pub fn random_bytes(size: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen()).collect()
}

/// Generate a unique bucket name for testing
pub fn unique_bucket_name(prefix: &str) -> String {
    use rand::Rng;
    let suffix: u32 = rand::thread_rng().gen();
    format!("{}-{}", prefix, suffix)
}

/// Generate a unique object key for testing
pub fn unique_object_key(prefix: &str) -> String {
    use rand::Rng;
    let suffix: u32 = rand::thread_rng().gen();
    format!("{}-{}", prefix, suffix)
}

/// Cleanup helper to ensure bucket is deleted
pub async fn cleanup_bucket(client: &S3Client, bucket: &str) {
    // Try to delete all objects first
    if let Ok(list_output) = client.list_objects_v2().bucket(bucket).send().await {
        if let Some(contents) = list_output.contents {
            for object in contents {
                if let Some(key) = object.key {
                    let _ = client.delete_object().bucket(bucket).key(&key).send().await;
                }
            }
        }
    }

    // Delete the bucket
    let _ = client.delete_bucket().bucket(bucket).send().await;
}

/// Test fixture that provides a clean bucket for each test
pub struct BucketFixture {
    pub client: S3Client,
    pub bucket: String,
}

impl BucketFixture {
    pub async fn new(cluster: &TestCluster, bucket_prefix: &str) -> Self {
        let client = create_test_s3_client(cluster).await;
        let bucket = unique_bucket_name(bucket_prefix);

        // Create the bucket
        client
            .create_bucket()
            .bucket(&bucket)
            .send()
            .await
            .expect("Failed to create test bucket");

        Self { client, bucket }
    }
}

impl Drop for BucketFixture {
    fn drop(&mut self) {
        // Cleanup in background
        let client = self.client.clone();
        let bucket = self.bucket.clone();

        tokio::spawn(async move {
            cleanup_bucket(&client, &bucket).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_setup_utilities() {
        // Test random bytes generation
        let bytes = random_bytes(100);
        assert_eq!(bytes.len(), 100);

        // Test unique name generation
        let bucket1 = unique_bucket_name("test");
        let bucket2 = unique_bucket_name("test");
        assert_ne!(bucket1, bucket2);

        let key1 = unique_object_key("obj");
        let key2 = unique_object_key("obj");
        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_logging_init() {
        init_logging();
        // Should not panic if called multiple times
        init_logging();
    }
}
