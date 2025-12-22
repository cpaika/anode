//! Minority cannot write chaos test
//!
//! Tests that a minority partition cannot commit writes while
//! a majority partition continues to operate correctly.

use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::cluster::TestCluster;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::test]
#[ignore]
async fn test_minority_cannot_write() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting minority cannot write chaos test");

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
    use anode_chaos_tests::scenarios::network_partition::MinorityCannotWriteScenario;
    let scenario = MinorityCannotWriteScenario;

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
        "Minority cannot write scenario failed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity verification failed"
    );

    // Verify that minority write was prevented
    if let Some(prevented) = result.metrics.get("minority_write_prevented") {
        assert_eq!(
            prevented.as_bool(),
            Some(true),
            "Minority partition was able to commit writes"
        );
    }

    info!("Test passed successfully");
}

#[tokio::test]
#[ignore]
async fn test_minority_cannot_write_with_3_nodes() {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting minority cannot write chaos test with 3 nodes");

    // Create a 3-node cluster (minimal cluster for testing)
    let mut cluster = TestCluster::new(3).await.expect("Failed to create cluster");
    cluster.start().await.expect("Failed to start cluster");

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Create scenario configuration for 3 nodes
    // This will create a 2-1 partition
    let config = ScenarioConfig {
        num_nodes: 3,
        duration: Duration::from_secs(20),
        verify_integrity: true,
        workload_intensity: 5.0,
        params: serde_json::Value::Null,
    };

    use anode_chaos_tests::scenarios::network_partition::MinorityCannotWriteScenario;
    let scenario = MinorityCannotWriteScenario;

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
        "Minority cannot write scenario failed: {:?}",
        result.error
    );

    info!("3-node test passed successfully");
}
