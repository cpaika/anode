//! Cascade failure scenarios
//!
//! Tests cluster resilience to cascading failures.

use super::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{verification::DataVerifier, ChaosError, Result};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

/// Sequential node failure scenario
pub struct CascadeFailureScenario;

#[async_trait]
impl ChaosScenario for CascadeFailureScenario {
    fn name(&self) -> &str {
        "cascade_failure"
    }

    fn description(&self) -> &str {
        "Simulates cascading failures where nodes fail sequentially"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!(
            "Starting cascade failure scenario with {} nodes",
            config.num_nodes
        );

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        for i in 0..5 {
            verifier
                .write_and_track(
                    "test-bucket",
                    &format!("initial-{}", i),
                    format!("data-{}", i).into_bytes(),
                )
                .await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
            operations += 1;
        }

        // Calculate how many nodes can fail before losing quorum
        let quorum = (config.num_nodes / 2) + 1;
        let max_failures = config.num_nodes - quorum;

        info!(
            "Will cascade-fail {} nodes (quorum requires {})",
            max_failures, quorum
        );

        let failure_interval = Duration::from_secs(5);

        // Kill nodes one by one
        for i in 0..max_failures {
            let node_to_kill = config.num_nodes - 1 - i; // Kill from the end

            info!(
                "Cascade failure {}/{}: killing node {}",
                i + 1,
                max_failures,
                node_to_kill
            );

            cluster
                .kill_node(node_to_kill)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(failure_interval).await;

            // Verify cluster is still operational
            match verifier
                .write_and_track(
                    "test-bucket",
                    &format!("after-failure-{}", i),
                    format!("data-after-{}", i).into_bytes(),
                )
                .await
            {
                Ok(_) => {
                    info!("Cluster still operational after {} failures", i + 1);
                    operations += 1;
                },
                Err(e) => {
                    failures += 1;
                    warn!("Write failed after {} failures: {}", i + 1, e);
                },
            }

            // Verify reads still work
            match verifier
                .verify_read("test-bucket", "initial-0", b"data-0")
                .await
            {
                Ok(true) => operations += 1,
                Ok(false) => {
                    failures += 1;
                    warn!("Data mismatch after {} failures", i + 1);
                },
                Err(e) => {
                    failures += 1;
                    warn!("Read failed after {} failures: {}", i + 1, e);
                },
            }
        }

        // Cluster should still be operational with quorum
        info!("Testing cluster with minimum quorum");

        match verifier
            .write_and_track("test-bucket", "min-quorum-test", b"test".to_vec())
            .await
        {
            Ok(_) => {
                info!("Cluster operational at minimum quorum");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Cluster not operational at minimum quorum: {}", e);
            },
        }

        // Restart failed nodes
        info!("Restarting failed nodes");
        for i in 0..max_failures {
            let node_to_restart = config.num_nodes - 1 - i;
            cluster
                .restart_node(node_to_restart)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
            sleep(Duration::from_secs(2)).await;
        }

        // Wait for cluster to stabilize
        sleep(Duration::from_secs(10)).await;

        // Verify data integrity
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "cascade_failures": max_failures,
                "quorum_size": quorum,
                "failure_interval_secs": failure_interval.as_secs(),
            }),
            error: if failures > 0 {
                Some(format!("{} operations failed during cascade", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Rapid sequential failures scenario
pub struct RapidCascadeScenario;

#[async_trait]
impl ChaosScenario for RapidCascadeScenario {
    fn name(&self) -> &str {
        "rapid_cascade_failure"
    }

    fn description(&self) -> &str {
        "Simulates rapid cascading failures with minimal delay between failures"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting rapid cascade failure scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        verifier
            .write_and_track("test-bucket", "initial", b"data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        let quorum = (config.num_nodes / 2) + 1;
        let max_failures = config.num_nodes - quorum;

        info!("Rapidly failing {} nodes", max_failures);

        // Kill nodes in rapid succession
        for i in 0..max_failures {
            let node_to_kill = config.num_nodes - 1 - i;
            cluster
                .kill_node(node_to_kill)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            // Only 500ms between failures
            sleep(Duration::from_millis(500)).await;
        }

        // Give cluster a moment to stabilize
        sleep(Duration::from_secs(5)).await;

        // Verify cluster still works
        match verifier
            .write_and_track("test-bucket", "after-rapid-cascade", b"test".to_vec())
            .await
        {
            Ok(_) => {
                info!("Cluster survived rapid cascade");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Cluster failed after rapid cascade: {}", e);
            },
        }

        // Restart all nodes
        for i in 0..max_failures {
            let node_to_restart = config.num_nodes - 1 - i;
            cluster
                .restart_node(node_to_restart)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(10)).await;

        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "cascade_failures": max_failures,
                "failure_interval_ms": 500,
            }),
            error: if failures > 0 {
                Some("Cluster failed under rapid cascade".to_string())
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Recovery under load scenario
pub struct RecoveryUnderLoadScenario;

#[async_trait]
impl ChaosScenario for RecoveryUnderLoadScenario {
    fn name(&self) -> &str {
        "recovery_under_load"
    }

    fn description(&self) -> &str {
        "Fails nodes and attempts recovery while under continuous load"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let operations = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let failures = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        info!("Starting recovery under load scenario");

        // Start continuous workload
        let workload_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let workload_running_clone = workload_running.clone();
        let operations_clone = operations.clone();
        let failures_clone = failures.clone();
        let endpoint = cluster.get_s3_endpoint();

        let workload_handle = tokio::spawn(async move {
            let mut verifier = DataVerifier::new(endpoint);
            let mut counter = 0;

            while workload_running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                match verifier
                    .write_and_track(
                        "test-bucket",
                        &format!("load-{}", counter),
                        format!("data-{}", counter).into_bytes(),
                    )
                    .await
                {
                    Ok(_) => {
                        operations_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    },
                    Err(_) => {
                        failures_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    },
                }
                counter += 1;
                sleep(Duration::from_millis(50)).await;
            }
        });

        // Let workload run for a bit
        sleep(Duration::from_secs(5)).await;

        // Start failing nodes
        let quorum = (config.num_nodes / 2) + 1;
        let max_failures = config.num_nodes - quorum;

        for i in 0..max_failures {
            let node_to_kill = config.num_nodes - 1 - i;
            info!("Killing node {} under load", node_to_kill);

            cluster
                .kill_node(node_to_kill)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(Duration::from_secs(3)).await;
        }

        // Let it run with reduced cluster
        sleep(Duration::from_secs(10)).await;

        // Start recovering nodes
        for i in 0..max_failures {
            let node_to_restart = config.num_nodes - 1 - i;
            info!("Restarting node {} under load", node_to_restart);

            cluster
                .restart_node(node_to_restart)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(Duration::from_secs(5)).await;
        }

        // Let workload run with recovered cluster
        sleep(Duration::from_secs(10)).await;

        // Stop workload
        workload_running.store(false, std::sync::atomic::Ordering::Relaxed);
        workload_handle
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        let final_operations = operations.load(std::sync::atomic::Ordering::Relaxed);
        let final_failures = failures.load(std::sync::atomic::Ordering::Relaxed);

        let verifier = DataVerifier::new(cluster.get_s3_endpoint());
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        // Allow up to 20% failure rate during recovery
        let success = final_failures < final_operations / 5 && integrity_verified;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            operations: final_operations,
            failures: final_failures,
            integrity_verified,
            metrics: serde_json::json!({
                "cascade_failures": max_failures,
                "failure_rate": final_failures as f64 / final_operations as f64,
            }),
            error: if !success {
                Some(format!(
                    "Failure rate too high during recovery: {}/{}",
                    final_failures, final_operations
                ))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}
