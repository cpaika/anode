//! Cascade failure chaos tests
//!
//! Tests system behavior under cascading failures

use anode_chaos_tests::scenarios::cascade_failure::*;
use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig};
use anode_test_harness::TestCluster;
use std::time::Duration;
use tracing_subscriber;

#[tokio::test]
#[ignore] // Requires running cluster
async fn test_cascade_failure() {
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

    let scenario = CascadeFailureScenario;
    let result = scenario.run(&mut cluster, &config).await;

    cluster.stop().await.ok();

    let result = result.expect("Scenario failed");
    assert!(result.success, "Cascade failure scenario failed: {:?}", result.error);
    assert!(result.integrity_verified, "Data integrity verification failed");
}

#[tokio::test]
#[ignore]
async fn test_rapid_cascade() {
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

    let scenario = RapidCascadeScenario;
    let result = scenario.run(&mut cluster, &config).await;

    cluster.stop().await.ok();

    let result = result.expect("Scenario failed");
    assert!(result.success, "Rapid cascade scenario failed: {:?}", result.error);
}

#[tokio::test]
#[ignore]
async fn test_recovery_under_load() {
    let _ = tracing_subscriber::fmt::try_init();

    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 20.0,
        params: serde_json::Value::Null,
    };

    let mut cluster = TestCluster::new(config.num_nodes)
        .await
        .expect("Failed to create cluster");

    cluster.start().await.expect("Failed to start cluster");

    let scenario = RecoveryUnderLoadScenario;
    let result = scenario.run(&mut cluster, &config).await;

    cluster.stop().await.ok();

    let result = result.expect("Scenario failed");
    assert!(result.success, "Recovery under load scenario failed: {:?}", result.error);
}
