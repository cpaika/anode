//! Split-brain chaos tests
//!
//! Tests system behavior in split-brain scenarios

use anode_chaos_tests::scenarios::split_brain::*;
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::TestCluster;
use std::time::Duration;
use tracing_subscriber;

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_split_brain_prevention() {
    let _ = tracing_subscriber::fmt::try_init();

    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    let mut cluster = TestCluster::new(config.num_nodes)
        .await
        .expect("Failed to create cluster");

    cluster.start().await.expect("Failed to start cluster");

    let scenario = SplitBrainScenario;
    let result = scenario.run(&mut cluster, &config).await;

    cluster.stop().await.ok();

    let result = result.expect("Scenario failed");
    assert!(
        result.success,
        "Split-brain scenario failed: {:?}",
        result.error
    );
    assert!(
        result.integrity_verified,
        "Data integrity verification failed"
    );
}

#[tokio::test]
#[ignore]
async fn test_flapping_partition() {
    let _ = tracing_subscriber::fmt::try_init();

    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(30),
        verify_integrity: true,
        workload_intensity: 5.0,
        params: serde_json::Value::Null,
    };

    let mut cluster = TestCluster::new(config.num_nodes)
        .await
        .expect("Failed to create cluster");

    cluster.start().await.expect("Failed to start cluster");

    let scenario = FlappingPartitionScenario;
    let result = scenario.run(&mut cluster, &config).await;

    cluster.stop().await.ok();

    let result = result.expect("Scenario failed");
    assert!(
        result.success,
        "Flapping partition scenario failed: {:?}",
        result.error
    );
}
