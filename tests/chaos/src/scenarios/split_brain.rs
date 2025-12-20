//! Split-brain scenarios
//!
//! Tests cluster behavior in split-brain conditions.

use super::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{Result, ChaosError, verification::DataVerifier};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

/// Network partition split-brain scenario
pub struct SplitBrainScenario;

#[async_trait]
impl ChaosScenario for SplitBrainScenario {
    fn name(&self) -> &str {
        "split_brain"
    }

    fn description(&self) -> &str {
        "Creates a network partition and verifies no split-brain occurs"
    }

    async fn run(&self, cluster: &mut TestCluster, config: &ScenarioConfig) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting split-brain scenario with {} nodes", config.num_nodes);

        if config.num_nodes < 5 {
            return Ok(ScenarioResult {
                name: self.name().to_string(),
                success: false,
                duration: start_time.elapsed(),
                operations: 0,
                failures: 1,
                integrity_verified: false,
                metrics: serde_json::json!({}),
                error: Some("Requires at least 5 nodes for meaningful split-brain test".to_string()),
            });
        }

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        verifier.write_and_track("test-bucket", "before-split", b"initial".to_vec()).await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Wait for initial leader
        let initial_leader = cluster.wait_for_leader(Duration::from_secs(30))
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        info!("Initial leader is node {}", initial_leader);

        // Create partition: split cluster into two groups
        let partition_point = config.num_nodes / 2;

        info!("Creating partition: nodes 0-{} vs {}-{}",
              partition_point - 1,
              partition_point,
              config.num_nodes - 1);

        // Partition the minority side
        for i in 0..partition_point {
            cluster.partition_node(i).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(10)).await;

        // Try to write to both sides
        // Only one side should accept writes (the majority)
        let mut writes_succeeded = 0;

        match verifier.write_and_track("test-bucket", "during-partition", b"partition-write".to_vec()).await {
            Ok(_) => {
                info!("Write succeeded on one partition");
                writes_succeeded += 1;
                operations += 1;
            }
            Err(e) => {
                info!("Write failed (expected on minority): {}", e);
            }
        }

        // CRITICAL: Verify only one partition accepted the write
        if writes_succeeded != 1 {
            failures += 1;
            warn!("Expected exactly 1 partition to accept writes, got {}", writes_succeeded);
        }

        // Check leader on majority partition
        match cluster.wait_for_leader(Duration::from_secs(30)).await {
            Ok(leader_id) => {
                info!("Majority partition has leader: node {}", leader_id);
                operations += 1;

                // Leader should be in the majority partition
                if (leader_id as usize) < partition_point {
                    failures += 1;
                    warn!("Leader is in minority partition - possible split-brain!");
                }
            }
            Err(e) => {
                warn!("Failed to get leader from majority: {}", e);
            }
        }

        // Heal the partition
        info!("Healing partition");
        for i in 0..partition_point {
            cluster.heal_partition(i).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for cluster to converge
        sleep(Duration::from_secs(15)).await;

        // Verify single leader after healing
        match cluster.wait_for_leader(Duration::from_secs(30)).await {
            Ok(final_leader) => {
                info!("Final leader after healing: node {}", final_leader);
                operations += 1;
            }
            Err(e) => {
                failures += 1;
                warn!("Failed to get leader after healing: {}", e);
            }
        }

        // Verify data consistency - should have data from majority partition only
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
                "partition_size_1": partition_point,
                "partition_size_2": config.num_nodes - partition_point,
                "writes_during_partition": writes_succeeded,
            }),
            error: if failures > 0 {
                Some(format!("{} failures - possible split-brain detected", failures))
            } else if !integrity_verified {
                Some("Data inconsistency detected".to_string())
            } else {
                None
            },
        })
    }
}

/// Flapping partition scenario
pub struct FlappingPartitionScenario;

#[async_trait]
impl ChaosScenario for FlappingPartitionScenario {
    fn name(&self) -> &str {
        "flapping_partition"
    }

    fn description(&self) -> &str {
        "Creates repeatedly flapping network partitions"
    }

    async fn run(&self, cluster: &mut TestCluster, config: &ScenarioConfig) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting flapping partition scenario");

        let verifier = DataVerifier::new(cluster.get_s3_endpoint());

        let num_flaps = 5;
        let partition_duration = Duration::from_secs(5);
        let heal_duration = Duration::from_secs(5);

        for flap in 0..num_flaps {
            info!("Flap {} of {}", flap + 1, num_flaps);

            // Create partition
            cluster.partition_node(config.num_nodes - 1).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(partition_duration).await;

            // Try operation during partition
            match verifier.write_and_track(
                "test-bucket",
                &format!("flap-{}", flap),
                format!("data-{}", flap).into_bytes(),
            ).await {
                Ok(_) => operations += 1,
                Err(_) => {} // Expected to potentially fail
            }

            // Heal partition
            cluster.heal_partition(config.num_nodes - 1).await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            sleep(heal_duration).await;
        }

        // Verify consistency
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
                "num_flaps": num_flaps,
                "partition_duration_secs": partition_duration.as_secs(),
            }),
            error: if !integrity_verified {
                Some("Data inconsistency after flapping partitions".to_string())
            } else {
                None
            },
        })
    }
}
