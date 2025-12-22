//! Majority can write chaos test
//!
//! Tests that a majority partition can continue accepting and committing
//! writes during a network partition, and that those writes replicate
//! among the majority nodes.

use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::cluster::TestCluster;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::test]
#[ignore]
async fn test_majority_can_write() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting majority can write chaos test");

    // Create a 5-node cluster
    let mut cluster = TestCluster::new(5).await.expect("Failed to create cluster");
    cluster.start().await.expect("Failed to start cluster");

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create scenario configuration
    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(30),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    // Import and run the scenario
    use anode_chaos_tests::scenarios::network_partition::MajorityCanWriteScenario;
    let scenario = MajorityCanWriteScenario;

    info!("Running scenario: {}", scenario.description());

    let result = scenario
        .run(&mut cluster, &config)
        .await
        .expect("Scenario failed to run");

    info!("Scenario completed: {:?}", result);

    // Stop the cluster
    cluster.stop().await.expect("Failed to stop cluster");

    // Assert the test passed
    assert!(
        result.success,
        "Majority can write scenario failed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity verification failed"
    );

    // Verify that all writes were committed
    if let Some(writes) = result.metrics.get("writes_committed") {
        assert_eq!(
            writes.as_u64(),
            Some(5),
            "Not all writes were committed to majority partition"
        );
    }

    // Verify that all reads succeeded
    if let Some(reads) = result.metrics.get("reads_verified") {
        assert_eq!(
            reads.as_u64(),
            Some(5),
            "Not all reads were verified on majority partition"
        );
    }

    // Verify that writes replicated among majority nodes
    if let Some(replicated) = result.metrics.get("replication_verified") {
        assert_eq!(
            replicated.as_bool(),
            Some(true),
            "Writes did not replicate among majority nodes"
        );
    }

    info!("Test passed successfully");
}

#[tokio::test]
#[ignore]
async fn test_majority_can_write_with_3_nodes() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting majority can write chaos test with 3 nodes");

    // Create a 3-node cluster (minimal cluster for testing)
    let mut cluster = TestCluster::new(3).await.expect("Failed to create cluster");
    cluster.start().await.expect("Failed to start cluster");

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create scenario configuration for 3 nodes
    // This will create a 2-1 partition (2 majority, 1 minority)
    let config = ScenarioConfig {
        num_nodes: 3,
        duration: Duration::from_secs(20),
        verify_integrity: true,
        workload_intensity: 5.0,
        params: serde_json::Value::Null,
    };

    use anode_chaos_tests::scenarios::network_partition::MajorityCanWriteScenario;
    let scenario = MajorityCanWriteScenario;

    info!("Running scenario: {}", scenario.description());

    let result = scenario
        .run(&mut cluster, &config)
        .await
        .expect("Scenario failed to run");

    info!("Scenario completed: {:?}", result);

    // Stop the cluster
    cluster.stop().await.expect("Failed to stop cluster");

    // Assert the test passed
    assert!(
        result.success,
        "Majority can write scenario failed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity verification failed"
    );

    info!("3-node test passed successfully");
}

#[tokio::test]
#[ignore]
async fn test_majority_can_write_with_7_nodes() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting majority can write chaos test with 7 nodes");

    // Create a 7-node cluster (larger cluster)
    let mut cluster = TestCluster::new(7).await.expect("Failed to create cluster");
    cluster.start().await.expect("Failed to start cluster");

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create scenario configuration for 7 nodes
    // This will create a 4-3 partition (4 majority, 3 minority)
    let config = ScenarioConfig {
        num_nodes: 7,
        duration: Duration::from_secs(30),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    use anode_chaos_tests::scenarios::network_partition::MajorityCanWriteScenario;
    let scenario = MajorityCanWriteScenario;

    info!("Running scenario: {}", scenario.description());

    let result = scenario
        .run(&mut cluster, &config)
        .await
        .expect("Scenario failed to run");

    info!("Scenario completed: {:?}", result);

    // Stop the cluster
    cluster.stop().await.expect("Failed to stop cluster");

    // Assert the test passed
    assert!(
        result.success,
        "Majority can write scenario failed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity verification failed"
    );

    // Verify that writes replicated among larger majority
    if let Some(replicated) = result.metrics.get("replication_verified") {
        assert_eq!(
            replicated.as_bool(),
            Some(true),
            "Writes did not replicate among majority nodes in 7-node cluster"
        );
    }

    info!("7-node test passed successfully");
}
