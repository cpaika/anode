//! Rolling restart scenarios
//!
//! Tests cluster resilience during rolling updates/restarts.

use super::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{Result, ChaosError, verification::DataVerifier};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::info;

/// Basic rolling restart scenario
pub struct RollingRestartScenario;

#[async_trait]
impl ChaosScenario for RollingRestartScenario {
    fn name(&self) -> &str {
        "rolling_restart"
    }

    fn description(&self) -> &str {
        "Performs a rolling restart of all nodes while maintaining availability"
    }

    async fn run(&self, cluster: &mut TestCluster, config: &ScenarioConfig) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting rolling restart scenario with {} nodes", config.num_nodes);

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        for i in 0..10 {
            verifier.write_and_track(
                "test-bucket",
                &format!("pre-restart-{}", i),
                format!("data-{}", i).into_bytes(),
            ).await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
            operations += 1;
        }

        // Perform rolling restart
        let restart_interval = Duration::from_secs(10);

        for i in 0..config.num_nodes {
            info!("Restarting node {} of {}", i + 1, config.num_nodes);

            // Restart the node
            cluster.restart_node(i).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            // Wait for node to come back up
            sleep(Duration::from_secs(5)).await;

            // Verify cluster is still operational during restart
            match verifier.write_and_track(
                "test-bucket",
                &format!("during-restart-{}", i),
                format!("data-during-{}", i).into_bytes(),
            ).await {
                Ok(_) => {
                    operations += 1;
                }
                Err(e) => {
                    failures += 1;
                    info!("Write failed during restart of node {}: {}", i, e);
                }
            }

            // Wait before restarting next node
            if i < config.num_nodes - 1 {
                sleep(restart_interval).await;
            }
        }

        // Verify all nodes are back up
        sleep(Duration::from_secs(10)).await;

        // Verify all data is intact
        let integrity_verified = verifier.verify_all().await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "nodes_restarted": config.num_nodes,
                "restart_interval_secs": restart_interval.as_secs(),
            }),
            error: if failures > 0 {
                Some(format!("{} operations failed during rolling restart", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Continuous workload during rolling restart scenario
pub struct ContinuousWorkloadRollingRestartScenario;

#[async_trait]
impl ChaosScenario for ContinuousWorkloadRollingRestartScenario {
    fn name(&self) -> &str {
        "continuous_workload_rolling_restart"
    }

    fn description(&self) -> &str {
        "Performs rolling restart while running continuous workload"
    }

    async fn run(&self, cluster: &mut TestCluster, config: &ScenarioConfig) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting continuous workload rolling restart scenario");

        let verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Spawn continuous workload task
        let workload_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let workload_running_clone = workload_running.clone();
        let endpoint = cluster.get_s3_endpoint();

        let workload_handle = tokio::spawn(async move {
            let mut ops = 0;
            let mut fails = 0;
            let verifier = DataVerifier::new(endpoint);

            while workload_running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                let key = format!("workload-{}", ops);
                match verifier.write_and_track(
                    "test-bucket",
                    &key,
                    format!("data-{}", ops).into_bytes(),
                ).await {
                    Ok(_) => ops += 1,
                    Err(_) => fails += 1,
                }
                sleep(Duration::from_millis(100)).await;
            }

            (ops, fails)
        });

        // Perform rolling restart
        for i in 0..config.num_nodes {
            info!("Restarting node {} of {}", i + 1, config.num_nodes);

            cluster.restart_node(i).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(Duration::from_secs(15)).await;
        }

        // Stop workload
        workload_running.store(false, std::sync::atomic::Ordering::Relaxed);

        // Wait for workload to complete
        let (workload_ops, workload_fails) = workload_handle.await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        operations = workload_ops;
        failures = workload_fails;

        // Verify data integrity
        let integrity_verified = verifier.verify_all().await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures < operations / 10 && integrity_verified, // Allow up to 10% failures
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "nodes_restarted": config.num_nodes,
                "failure_rate": failures as f64 / operations as f64,
            }),
            error: if failures >= operations / 10 {
                Some(format!("Failure rate too high: {}/{}", failures, operations))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}
