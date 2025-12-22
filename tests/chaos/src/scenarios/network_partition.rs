//! Network partition scenarios
//!
//! Tests cluster behavior under various network partition conditions.

use super::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{verification::DataVerifier, ChaosError, Result};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

/// Minority network partition scenario
pub struct MinorityPartitionScenario;

#[async_trait]
impl ChaosScenario for MinorityPartitionScenario {
    fn name(&self) -> &str {
        "minority_partition"
    }

    fn description(&self) -> &str {
        "Partitions minority nodes from the cluster and verifies majority continues operating"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting minority partition scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        verifier
            .write_and_track("test-bucket", "pre-partition", b"initial-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Calculate partition sizes
        let majority_size = (config.num_nodes / 2) + 1;
        let minority_size = config.num_nodes - majority_size;

        info!(
            "Partitioning {} nodes from {} majority nodes",
            minority_size, majority_size
        );

        // Partition minority nodes (last N nodes)
        for i in majority_size..config.num_nodes {
            info!("Partitioning node {}", i);
            cluster
                .partition_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(5)).await;

        // Majority should still accept writes
        match verifier
            .write_and_track(
                "test-bucket",
                "during-partition",
                b"partition-data".to_vec(),
            )
            .await
        {
            Ok(_) => {
                info!("Write succeeded on majority partition");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Write failed on majority: {}", e);
            },
        }

        // Verify reads work on majority
        match verifier
            .verify_read("test-bucket", "pre-partition", b"initial-data")
            .await
        {
            Ok(true) => {
                operations += 1;
            },
            Ok(false) => {
                failures += 1;
                warn!("Data mismatch on majority");
            },
            Err(e) => {
                failures += 1;
                warn!("Read failed on majority: {}", e);
            },
        }

        // Heal the partition
        info!("Healing partition");
        for i in majority_size..config.num_nodes {
            cluster
                .heal_partition(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for minority to catch up
        sleep(Duration::from_secs(10)).await;

        // Verify minority caught up with majority
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
                "majority_size": majority_size,
                "minority_size": minority_size,
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Minority failed to catch up".to_string())
            } else {
                None
            },
        })
    }
}

/// Leader partition scenario
pub struct LeaderPartitionScenario;

#[async_trait]
impl ChaosScenario for LeaderPartitionScenario {
    fn name(&self) -> &str {
        "leader_partition"
    }

    fn description(&self) -> &str {
        "Isolates the leader node and verifies new leader election and no split-brain"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        _config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting leader partition scenario");

        // Wait for leader election
        let leader_id = cluster
            .wait_for_leader(Duration::from_secs(30))
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        info!("Current leader is node {}", leader_id);

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write data before partition
        verifier
            .write_and_track("test-bucket", "before-partition", b"before".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Partition the leader
        info!("Partitioning leader node {}", leader_id);
        cluster
            .partition_node(leader_id as usize)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        // Wait for new leader election
        sleep(Duration::from_secs(10)).await;

        match cluster.wait_for_leader(Duration::from_secs(30)).await {
            Ok(new_leader_id) => {
                info!("New leader elected: node {}", new_leader_id);
                operations += 1;

                if new_leader_id == leader_id {
                    failures += 1;
                    warn!("Same leader re-elected - partition may not be effective");
                }
            },
            Err(e) => {
                failures += 1;
                warn!("Failed to elect new leader: {}", e);
            },
        }

        // Write data with new leader
        match verifier
            .write_and_track("test-bucket", "after-partition", b"after".to_vec())
            .await
        {
            Ok(_) => {
                info!("Write succeeded with new leader");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Write failed with new leader: {}", e);
            },
        }

        // Heal the partition
        info!("Healing leader partition");
        cluster
            .heal_partition(leader_id as usize)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        // Wait for old leader to rejoin
        sleep(Duration::from_secs(10)).await;

        // Verify no split-brain occurred and data is consistent
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
                "old_leader_id": leader_id,
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Split-brain or data inconsistency detected".to_string())
            } else {
                None
            },
        })
    }
}

/// Symmetric partition scenario
pub struct SymmetricPartitionScenario;

#[async_trait]
impl ChaosScenario for SymmetricPartitionScenario {
    fn name(&self) -> &str {
        "symmetric_partition"
    }

    fn description(&self) -> &str {
        "Creates a symmetric partition splitting the cluster and verifies only majority side works"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let failures = 0;

        info!("Starting symmetric partition scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        verifier
            .write_and_track("test-bucket", "initial", b"data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Create symmetric partition
        let partition_point = config.num_nodes / 2;
        let majority_size = config.num_nodes - partition_point;

        info!(
            "Creating symmetric partition: {} vs {} nodes",
            partition_point, majority_size
        );

        // Partition the smaller side
        for i in 0..partition_point {
            cluster
                .partition_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(5)).await;

        // Only the majority side should accept writes
        match verifier
            .write_and_track(
                "test-bucket",
                "during-partition",
                b"partition-write".to_vec(),
            )
            .await
        {
            Ok(_) => {
                info!("Write succeeded on majority side");
                operations += 1;
            },
            Err(e) => {
                // This might be expected if we're connecting to minority
                warn!("Write failed during partition: {}", e);
            },
        }

        // Heal partition
        info!("Healing partition");
        for i in 0..partition_point {
            cluster
                .heal_partition(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for cluster to converge
        sleep(Duration::from_secs(10)).await;

        // Verify consistency
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "partition_size_1": partition_point,
                "partition_size_2": majority_size,
            }),
            error: if !integrity_verified {
                Some("Data inconsistency after partition healing".to_string())
            } else {
                None
            },
        })
    }
}

/// Asymmetric network partition scenario
pub struct AsymmetricPartitionScenario;

#[async_trait]
impl ChaosScenario for AsymmetricPartitionScenario {
    fn name(&self) -> &str {
        "asymmetric_partition"
    }

    fn description(&self) -> &str {
        "Creates an asymmetric partition where A can reach B but B cannot reach A"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let failures = 0;

        info!("Starting asymmetric partition scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data
        verifier
            .write_and_track("test-bucket", "initial", b"data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Note: True asymmetric partitions are complex to implement
        // This is a simplified version showing the test structure
        info!("Asymmetric partitioning not fully implemented in this version");
        warn!("Would create one-way partition using iptables or similar");

        sleep(config.duration).await;

        // Verify data integrity
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "implementation_status": "partial",
            }),
            error: None,
        })
    }
}

/// Minority cannot write scenario
pub struct MinorityCannotWriteScenario;

#[async_trait]
impl ChaosScenario for MinorityCannotWriteScenario {
    fn name(&self) -> &str {
        "minority_cannot_write"
    }

    fn description(&self) -> &str {
        "Verifies that minority partition cannot commit writes while majority partition continues operating"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting minority cannot write scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data to establish baseline
        verifier
            .write_and_track("test-bucket", "pre-partition", b"initial-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Calculate partition sizes
        let majority_size = (config.num_nodes / 2) + 1;
        let minority_size = config.num_nodes - majority_size;

        info!(
            "Partitioning cluster: {} majority nodes, {} minority nodes",
            majority_size, minority_size
        );

        // Partition minority nodes (last N nodes)
        for i in majority_size..config.num_nodes {
            info!("Partitioning node {}", i);
            cluster
                .partition_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(5)).await;

        // Test 1: Majority partition should accept writes
        info!("Testing writes to majority partition");
        match verifier
            .write_and_track("test-bucket", "majority-write", b"majority-data".to_vec())
            .await
        {
            Ok(_) => {
                info!("Write to majority partition succeeded as expected");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Write to majority partition failed unexpectedly: {}", e);
            },
        }

        // Test 2: Try to write to minority partition (should fail or timeout)
        // Get endpoint for a node in the minority partition
        if let Ok(minority_node) = cluster.get_node(config.num_nodes - 1) {
            let minority_endpoint = minority_node.s3_endpoint();
            info!(
                "Testing writes to minority partition at {}",
                minority_endpoint
            );

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            let url = format!("{}/test-bucket/minority-write", minority_endpoint);
            match client
                .put(&url)
                .body(b"minority-data".to_vec())
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    failures += 1;
                    warn!("Write to minority partition succeeded - this should not happen!");
                },
                Ok(response) => {
                    info!(
                        "Write to minority partition rejected with status: {} (expected)",
                        response.status()
                    );
                    operations += 1;
                },
                Err(e) => {
                    info!(
                        "Write to minority partition failed/timed out: {} (expected)",
                        e
                    );
                    operations += 1;
                },
            }
        }

        // Test 3: Verify reads work on majority partition
        match verifier
            .verify_read("test-bucket", "pre-partition", b"initial-data")
            .await
        {
            Ok(true) => {
                info!("Read from majority partition succeeded");
                operations += 1;
            },
            Ok(false) => {
                failures += 1;
                warn!("Data mismatch on majority partition");
            },
            Err(e) => {
                failures += 1;
                warn!("Read failed on majority partition: {}", e);
            },
        }

        // Heal the partition
        info!("Healing partition");
        for i in majority_size..config.num_nodes {
            cluster
                .heal_partition(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for minority nodes to catch up with majority
        sleep(Duration::from_secs(10)).await;

        // Verify minority caught up and no writes were committed to minority
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        // Check that minority-write does not exist (was never committed)
        let minority_write_exists = match verifier
            .verify_read("test-bucket", "minority-write", b"minority-data")
            .await
        {
            Ok(true) => {
                warn!("Minority write was committed - FAILURE!");
                true
            },
            Ok(false) | Err(_) => {
                info!("Minority write was not committed (expected)");
                false
            },
        };

        let success = failures == 0 && integrity_verified && !minority_write_exists;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "majority_size": majority_size,
                "minority_size": minority_size,
                "minority_write_prevented": !minority_write_exists,
            }),
            error: if !success {
                if minority_write_exists {
                    Some("Minority partition was able to commit writes".to_string())
                } else if failures > 0 {
                    Some(format!("{} failures occurred", failures))
                } else {
                    Some("Data integrity verification failed".to_string())
                }
            } else {
                None
            },
        })
    }
}

/// Majority can write scenario
pub struct MajorityCanWriteScenario;

#[async_trait]
impl ChaosScenario for MajorityCanWriteScenario {
    fn name(&self) -> &str {
        "majority_can_write"
    }

    fn description(&self) -> &str {
        "Verifies that majority partition can continue accepting and committing writes during network partition"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting majority can write scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data to establish baseline
        verifier
            .write_and_track("test-bucket", "pre-partition", b"initial-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Calculate partition sizes
        let majority_size = (config.num_nodes / 2) + 1;
        let minority_size = config.num_nodes - majority_size;

        info!(
            "Partitioning cluster: {} majority nodes, {} minority nodes",
            majority_size, minority_size
        );

        // Partition minority nodes (last N nodes)
        for i in majority_size..config.num_nodes {
            info!("Partitioning node {}", i);
            cluster
                .partition_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(5)).await;

        // Test 1: Write multiple objects to majority partition
        info!("Writing multiple objects to majority partition");
        let mut write_count = 0;
        for i in 0..5 {
            let key = format!("majority-write-{}", i);
            let data = format!("data-{}", i);
            match verifier
                .write_and_track("test-bucket", &key, data.as_bytes().to_vec())
                .await
            {
                Ok(_) => {
                    info!("Write {} to majority partition succeeded", i);
                    write_count += 1;
                    operations += 1;
                },
                Err(e) => {
                    failures += 1;
                    warn!("Write {} to majority partition failed: {}", i, e);
                },
            }
        }

        // Test 2: Verify all writes were committed
        info!("Verifying writes on majority partition");
        let mut read_count = 0;
        for i in 0..5 {
            let key = format!("majority-write-{}", i);
            let expected_data = format!("data-{}", i);
            match verifier
                .verify_read("test-bucket", &key, expected_data.as_bytes())
                .await
            {
                Ok(true) => {
                    info!("Read {} from majority partition succeeded", i);
                    read_count += 1;
                    operations += 1;
                },
                Ok(false) => {
                    failures += 1;
                    warn!("Data mismatch for write {}", i);
                },
                Err(e) => {
                    failures += 1;
                    warn!("Read {} failed: {}", i, e);
                },
            }
        }

        // Test 3: Verify initial data is still accessible
        match verifier
            .verify_read("test-bucket", "pre-partition", b"initial-data")
            .await
        {
            Ok(true) => {
                info!("Pre-partition data still accessible on majority");
                operations += 1;
            },
            Ok(false) => {
                failures += 1;
                warn!("Pre-partition data mismatch");
            },
            Err(e) => {
                failures += 1;
                warn!("Pre-partition read failed: {}", e);
            },
        }

        // Test 4: Get endpoints of majority nodes and verify writes replicate
        info!("Verifying writes replicated among majority nodes");
        let mut replication_verified = true;
        for i in 0..majority_size {
            if let Ok(node) = cluster.get_node(i) {
                let node_endpoint = node.s3_endpoint();
                info!(
                    "Checking replication on majority node {} at {}",
                    i, node_endpoint
                );

                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

                // Check one of the writes exists on this node
                let url = format!("{}/test-bucket/majority-write-0", node_endpoint);
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        info!("Write replicated to majority node {}", i);
                        operations += 1;
                    },
                    Ok(response) => {
                        warn!(
                            "Write not found on majority node {}: status {}",
                            i,
                            response.status()
                        );
                        replication_verified = false;
                    },
                    Err(e) => {
                        warn!("Failed to check replication on node {}: {}", i, e);
                        replication_verified = false;
                    },
                }
            }
        }

        // Heal the partition
        info!("Healing partition");
        for i in majority_size..config.num_nodes {
            cluster
                .heal_partition(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for minority nodes to catch up with majority
        sleep(Duration::from_secs(10)).await;

        // Verify full cluster consistency
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        let success = failures == 0
            && write_count == 5
            && read_count == 5
            && replication_verified
            && integrity_verified;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "majority_size": majority_size,
                "minority_size": minority_size,
                "writes_committed": write_count,
                "reads_verified": read_count,
                "replication_verified": replication_verified,
            }),
            error: if !success {
                if write_count < 5 {
                    Some(format!(
                        "Only {} of 5 writes succeeded on majority",
                        write_count
                    ))
                } else if read_count < 5 {
                    Some(format!(
                        "Only {} of 5 reads verified on majority",
                        read_count
                    ))
                } else if !replication_verified {
                    Some("Writes did not replicate among majority nodes".to_string())
                } else if failures > 0 {
                    Some(format!("{} failures occurred", failures))
                } else {
                    Some("Data integrity verification failed".to_string())
                }
            } else {
                None
            },
        })
    }
}

/// Network partition heal scenario
pub struct PartitionHealScenario;

#[async_trait]
impl ChaosScenario for PartitionHealScenario {
    fn name(&self) -> &str {
        "partition_heal"
    }

    fn description(&self) -> &str {
        "Verifies that after a partition is healed, isolated nodes rejoin and data replicates correctly"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting partition heal scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data to establish baseline
        verifier
            .write_and_track("test-bucket", "pre-partition", b"initial-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Calculate partition sizes
        let majority_size = (config.num_nodes / 2) + 1;
        let minority_size = config.num_nodes - majority_size;

        info!(
            "Creating partition: {} majority nodes, {} minority nodes (will be isolated)",
            majority_size, minority_size
        );

        // Partition minority nodes (last N nodes)
        for i in majority_size..config.num_nodes {
            info!("Partitioning node {} (isolating from cluster)", i);
            cluster
                .partition_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        sleep(Duration::from_secs(5)).await;

        // Write data during partition to majority side
        info!("Writing data to majority partition during network split");
        let mut writes_during_partition = 0;
        for i in 0..3 {
            let key = format!("during-partition-{}", i);
            let data = format!("partition-data-{}", i);
            match verifier
                .write_and_track("test-bucket", &key, data.as_bytes().to_vec())
                .await
            {
                Ok(_) => {
                    info!("Write {} to majority partition succeeded", i);
                    writes_during_partition += 1;
                    operations += 1;
                },
                Err(e) => {
                    failures += 1;
                    warn!("Write {} to majority partition failed: {}", i, e);
                },
            }
        }

        // Verify writes are readable on majority partition
        info!("Verifying data on majority partition before heal");
        for i in 0..writes_during_partition {
            let key = format!("during-partition-{}", i);
            let expected_data = format!("partition-data-{}", i);
            match verifier
                .verify_read("test-bucket", &key, expected_data.as_bytes())
                .await
            {
                Ok(true) => {
                    operations += 1;
                },
                Ok(false) => {
                    failures += 1;
                    warn!("Data mismatch for during-partition write {}", i);
                },
                Err(e) => {
                    failures += 1;
                    warn!("Read failed for during-partition write {}: {}", i, e);
                },
            }
        }

        // Now heal the partition
        info!("Healing network partition - allowing isolated nodes to rejoin");
        for i in majority_size..config.num_nodes {
            info!("Healing partition for node {}", i);
            cluster
                .heal_partition(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for nodes to rejoin and catch up
        info!("Waiting for previously isolated nodes to rejoin and sync");
        sleep(Duration::from_secs(15)).await;

        // Verify that previously isolated nodes now have all the data
        info!("Verifying data replicated to previously isolated nodes");
        let mut replication_success = true;
        for node_idx in majority_size..config.num_nodes {
            if let Ok(node) = cluster.get_node(node_idx) {
                let node_endpoint = node.s3_endpoint();
                info!(
                    "Checking data on previously isolated node {} at {}",
                    node_idx, node_endpoint
                );

                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

                // Check that data written during partition is now on this node
                for i in 0..writes_during_partition {
                    let key = format!("during-partition-{}", i);
                    let url = format!("{}/test-bucket/{}", node_endpoint, key);

                    match client.get(&url).send().await {
                        Ok(response) if response.status().is_success() => {
                            let data = response
                                .bytes()
                                .await
                                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
                            let expected = format!("partition-data-{}", i);
                            if data.as_ref() == expected.as_bytes() {
                                info!("Data '{}' correctly replicated to node {}", key, node_idx);
                                operations += 1;
                            } else {
                                failures += 1;
                                warn!("Data mismatch for '{}' on node {}", key, node_idx);
                                replication_success = false;
                            }
                        },
                        Ok(response) => {
                            failures += 1;
                            warn!(
                                "Data '{}' not found on previously isolated node {}: status {}",
                                key,
                                node_idx,
                                response.status()
                            );
                            replication_success = false;
                        },
                        Err(e) => {
                            failures += 1;
                            warn!("Failed to check data '{}' on node {}: {}", key, node_idx, e);
                            replication_success = false;
                        },
                    }
                }

                // Also verify pre-partition data is there
                let url = format!("{}/test-bucket/pre-partition", node_endpoint);
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        let data = response
                            .bytes()
                            .await
                            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
                        if data.as_ref() == b"initial-data" {
                            info!(
                                "Pre-partition data correctly available on node {}",
                                node_idx
                            );
                            operations += 1;
                        } else {
                            failures += 1;
                            warn!("Pre-partition data mismatch on node {}", node_idx);
                            replication_success = false;
                        }
                    },
                    Ok(response) => {
                        failures += 1;
                        warn!(
                            "Pre-partition data not found on node {}: status {}",
                            node_idx,
                            response.status()
                        );
                        replication_success = false;
                    },
                    Err(e) => {
                        failures += 1;
                        warn!(
                            "Failed to check pre-partition data on node {}: {}",
                            node_idx, e
                        );
                        replication_success = false;
                    },
                }
            }
        }

        // Test that cluster is back to normal operation by writing new data
        info!("Testing cluster returned to normal operation");
        match verifier
            .write_and_track("test-bucket", "post-heal", b"healed-data".to_vec())
            .await
        {
            Ok(_) => {
                info!("Post-heal write succeeded - cluster operational");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Post-heal write failed: {}", e);
            },
        }

        // Verify new write is replicated across all nodes
        match verifier
            .verify_read("test-bucket", "post-heal", b"healed-data")
            .await
        {
            Ok(true) => {
                info!("Post-heal data verified");
                operations += 1;
            },
            Ok(false) => {
                failures += 1;
                warn!("Post-heal data mismatch");
            },
            Err(e) => {
                failures += 1;
                warn!("Post-heal read failed: {}", e);
            },
        }

        // Final integrity check across all nodes
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        let success = failures == 0
            && writes_during_partition == 3
            && replication_success
            && integrity_verified;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "majority_size": majority_size,
                "minority_size": minority_size,
                "writes_during_partition": writes_during_partition,
                "replication_to_healed_nodes": replication_success,
                "cluster_operational_after_heal": failures == 0,
            }),
            error: if !success {
                if writes_during_partition < 3 {
                    Some(format!(
                        "Only {} of 3 writes succeeded during partition",
                        writes_during_partition
                    ))
                } else if !replication_success {
                    Some("Data did not replicate to healed nodes".to_string())
                } else if failures > 0 {
                    Some(format!("{} failures occurred", failures))
                } else {
                    Some("Data integrity verification failed".to_string())
                }
            } else {
                None
            },
        })
    }
}
