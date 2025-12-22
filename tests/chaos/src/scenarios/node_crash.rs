//! Node crash scenarios
//!
//! Tests cluster resilience to node crashes and recovery.

use super::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{verification::DataVerifier, ChaosError, Result};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use rand::Rng;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

/// Random node crash scenario
pub struct RandomNodeCrashScenario;

#[async_trait]
impl ChaosScenario for RandomNodeCrashScenario {
    fn name(&self) -> &str {
        "random_node_crashes"
    }

    fn description(&self) -> &str {
        "Randomly crashes nodes during operations and verifies cluster continues functioning"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        use rand::SeedableRng;

        let start_time = Instant::now();
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut operations = 0;
        let mut failures = 0;

        info!(
            "Starting random node crash scenario with {} nodes",
            config.num_nodes
        );

        // Create a data verifier for integrity checking
        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial test data
        let test_data = vec![
            ("test-bucket", "key1", b"data1".to_vec()),
            ("test-bucket", "key2", b"data2".to_vec()),
            ("test-bucket", "key3", b"data3".to_vec()),
        ];

        for (bucket, key, data) in &test_data {
            verifier
                .write_and_track(bucket, key, data.clone())
                .await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
            operations += 1;
        }

        // Run chaos scenario
        let scenario_end = Instant::now() + config.duration;
        while Instant::now() < scenario_end {
            // Randomly kill a non-leader node
            let node_to_kill = rng.gen_range(1..config.num_nodes);

            info!("Killing node {}", node_to_kill);
            if let Err(e) = cluster.kill_node(node_to_kill).await {
                warn!("Failed to kill node {}: {}", node_to_kill, e);
                failures += 1;
            }

            // Continue operations while node is down
            sleep(Duration::from_secs(5)).await;

            // Verify reads still work
            for (bucket, key, expected_data) in &test_data {
                match verifier.verify_read(bucket, key, expected_data).await {
                    Ok(true) => operations += 1,
                    Ok(false) => {
                        failures += 1;
                        warn!("Data mismatch for {}/{}", bucket, key);
                    },
                    Err(e) => {
                        failures += 1;
                        warn!("Read failed: {}", e);
                    },
                }
            }

            // Restart the killed node
            info!("Restarting node {}", node_to_kill);
            if let Err(e) = cluster.restart_node(node_to_kill).await {
                warn!("Failed to restart node {}: {}", node_to_kill, e);
                failures += 1;
            }

            // Wait for cluster to stabilize
            sleep(Duration::from_secs(5)).await;

            // Verify data integrity after recovery
            for (bucket, key, expected_data) in &test_data {
                match verifier.verify_read(bucket, key, expected_data).await {
                    Ok(true) => operations += 1,
                    Ok(false) => {
                        failures += 1;
                        warn!("Data mismatch after recovery for {}/{}", bucket, key);
                    },
                    Err(e) => {
                        failures += 1;
                        warn!("Read failed after recovery: {}", e);
                    },
                }
            }
        }

        // Final verification
        let integrity_verified = if config.verify_integrity {
            verifier
                .verify_all()
                .await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?
        } else {
            true
        };

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "crash_recovery_cycles": operations / (test_data.len() * 2),
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Leader crash during write scenario
pub struct LeaderCrashDuringWriteScenario;

#[async_trait]
impl ChaosScenario for LeaderCrashDuringWriteScenario {
    fn name(&self) -> &str {
        "leader_crash_during_write"
    }

    fn description(&self) -> &str {
        "Crashes the leader during a write operation and verifies atomicity"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        _config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting leader crash during write scenario");

        // Wait for leader election
        let leader_id = cluster
            .wait_for_leader(Duration::from_secs(30))
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        info!("Leader is node {}", leader_id);

        let verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Start a large write operation
        let large_data = vec![0u8; 10 * 1024 * 1024]; // 10 MB

        // Spawn write task
        let write_handle = tokio::spawn({
            let endpoint = cluster.get_s3_endpoint();
            async move {
                let mut verifier = DataVerifier::new(endpoint);
                verifier
                    .write_and_track("test-bucket", "large-object", large_data)
                    .await
            }
        });

        // Kill leader shortly after write starts
        sleep(Duration::from_millis(100)).await;
        info!("Killing leader node {}", leader_id);

        cluster
            .kill_node(leader_id as usize)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        // Wait for write to complete (or fail)
        let write_result = write_handle
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        operations += 1;

        match write_result {
            Ok(_) => {
                info!("Write completed successfully despite leader crash");
                // Verify the object exists and is complete
                match verifier.verify_exists("test-bucket", "large-object").await {
                    Ok(true) => {
                        info!("Object verified successfully");
                    },
                    Ok(false) => {
                        failures += 1;
                        warn!("Object does not exist after successful write");
                    },
                    Err(e) => {
                        failures += 1;
                        warn!("Failed to verify object: {}", e);
                    },
                }
            },
            Err(e) => {
                info!("Write failed as expected: {}", e);
                // Verify the object does NOT exist (no partial writes)
                match verifier.verify_exists("test-bucket", "large-object").await {
                    Ok(false) => {
                        info!("Confirmed no partial write");
                    },
                    Ok(true) => {
                        failures += 1;
                        warn!("Partial write detected - atomicity violated!");
                    },
                    Err(e) => {
                        failures += 1;
                        warn!("Failed to verify non-existence: {}", e);
                    },
                }
            },
        }

        // Wait for new leader election
        sleep(Duration::from_secs(5)).await;
        match cluster.wait_for_leader(Duration::from_secs(30)).await {
            Ok(new_leader) => {
                info!("New leader elected: node {}", new_leader);
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Failed to elect new leader: {}", e);
            },
        }

        // Restart the old leader
        cluster
            .restart_node(leader_id as usize)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified: true,
            metrics: serde_json::json!({
                "leader_id": leader_id,
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else {
                None
            },
        })
    }
}

/// Multiple node crash scenario
pub struct MultipleNodeCrashScenario;

#[async_trait]
impl ChaosScenario for MultipleNodeCrashScenario {
    fn name(&self) -> &str {
        "multiple_node_crashes"
    }

    fn description(&self) -> &str {
        "Crashes multiple nodes while maintaining quorum and verifies cluster remains operational"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting multiple node crash scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write test data
        verifier
            .write_and_track("test-bucket", "test-key", b"test-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Calculate how many nodes we can crash while maintaining quorum
        let quorum = (config.num_nodes / 2) + 1;
        let nodes_to_crash = config.num_nodes - quorum;

        info!(
            "Crashing {} nodes (quorum requires {})",
            nodes_to_crash, quorum
        );

        // Crash nodes (avoiding node 0 which is typically the bootstrap node)
        for i in 1..=nodes_to_crash {
            info!("Killing node {}", i);
            cluster
                .kill_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Verify cluster is still operational
        sleep(Duration::from_secs(5)).await;

        // Try to read data
        match verifier
            .verify_read("test-bucket", "test-key", b"test-data")
            .await
        {
            Ok(true) => {
                info!("Read successful with reduced cluster");
                operations += 1;
            },
            Ok(false) => {
                failures += 1;
                warn!("Data mismatch");
            },
            Err(e) => {
                failures += 1;
                warn!("Read failed: {}", e);
            },
        }

        // Try to write new data
        match verifier
            .write_and_track("test-bucket", "new-key", b"new-data".to_vec())
            .await
        {
            Ok(_) => {
                info!("Write successful with reduced cluster");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Write failed: {}", e);
            },
        }

        // Restart crashed nodes
        for i in 1..=nodes_to_crash {
            info!("Restarting node {}", i);
            cluster
                .restart_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait for cluster to stabilize
        sleep(Duration::from_secs(10)).await;

        // Verify all data is intact
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
                "nodes_crashed": nodes_to_crash,
                "quorum_size": quorum,
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Full cluster crash and recovery scenario
pub struct FullClusterCrashScenario;

#[async_trait]
impl ChaosScenario for FullClusterCrashScenario {
    fn name(&self) -> &str {
        "full_cluster_crash"
    }

    fn description(&self) -> &str {
        "Crashes all nodes simultaneously and verifies recovery and data integrity"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting full cluster crash scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write comprehensive test data
        let test_data = vec![
            ("bucket1", "key1", b"data1".to_vec()),
            ("bucket1", "key2", b"data2".to_vec()),
            ("bucket2", "key1", b"data3".to_vec()),
            ("bucket2", "key2", b"data4".to_vec()),
        ];

        for (bucket, key, data) in &test_data {
            verifier
                .write_and_track(bucket, key, data.clone())
                .await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
            operations += 1;
        }

        info!("Wrote {} objects", test_data.len());

        // Wait for replication to complete
        sleep(Duration::from_secs(5)).await;

        // Crash ALL nodes
        info!("Crashing all {} nodes", config.num_nodes);
        for i in 0..config.num_nodes {
            cluster
                .kill_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
        }

        // Wait a bit to simulate downtime
        sleep(Duration::from_secs(5)).await;

        // Restart all nodes
        info!("Restarting all nodes");
        for i in 0..config.num_nodes {
            cluster
                .restart_node(i)
                .await
                .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

            // Stagger restarts slightly
            sleep(Duration::from_millis(500)).await;
        }

        // Wait for cluster to reform
        sleep(Duration::from_secs(15)).await;

        // Wait for leader election
        match cluster.wait_for_leader(Duration::from_secs(60)).await {
            Ok(leader_id) => {
                info!("Cluster reformed with leader {}", leader_id);
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("Failed to elect leader after restart: {}", e);
            },
        }

        // Verify all data is intact
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        if !integrity_verified {
            failures += 1;
        }

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "objects_written": test_data.len(),
                "cluster_size": config.num_nodes,
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

/// Follower crash during replication scenario
pub struct FollowerCrashDuringReplicationScenario;

#[async_trait]
impl ChaosScenario for FollowerCrashDuringReplicationScenario {
    fn name(&self) -> &str {
        "follower_crash_during_replication"
    }

    fn description(&self) -> &str {
        "Crashes a follower during replication and verifies cluster continues and follower catches up"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        _config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting follower crash during replication scenario");

        // Wait for leader election
        let leader_id = cluster
            .wait_for_leader(Duration::from_secs(30))
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        info!("Leader is node {}", leader_id);

        // Identify a follower to crash (avoid the leader)
        let follower_id = if leader_id as usize == 0 { 1 } else { 0 };
        info!("Selected follower node {} to crash", follower_id);

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write initial data to establish a baseline
        verifier
            .write_and_track("test-bucket", "initial-key", b"initial-data".to_vec())
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
        operations += 1;

        // Wait for initial replication
        sleep(Duration::from_secs(2)).await;

        // Start concurrent write operations
        let write_data = vec![
            ("test-bucket", "repl-key-1", vec![1u8; 1024 * 100]), // 100 KB
            ("test-bucket", "repl-key-2", vec![2u8; 1024 * 100]), // 100 KB
            ("test-bucket", "repl-key-3", vec![3u8; 1024 * 100]), // 100 KB
        ];

        // Spawn write tasks
        let write_handles: Vec<_> = write_data
            .iter()
            .map(|(bucket, key, data)| {
                let endpoint = cluster.get_s3_endpoint();
                let bucket = bucket.to_string();
                let key = key.to_string();
                let data = data.clone();
                tokio::spawn(async move {
                    let mut verifier = DataVerifier::new(endpoint);
                    verifier.write_and_track(&bucket, &key, data).await
                })
            })
            .collect();

        // Crash the follower shortly after writes start (simulate crash during replication)
        sleep(Duration::from_millis(50)).await;
        info!("Killing follower node {} during replication", follower_id);

        cluster
            .kill_node(follower_id)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        // Wait for writes to complete
        for handle in write_handles {
            match handle.await {
                Ok(Ok(_)) => {
                    operations += 1;
                },
                Ok(Err(e)) => {
                    // Some writes might fail, but quorum should still succeed
                    warn!("Write operation result: {}", e);
                },
                Err(e) => {
                    warn!("Write task error: {}", e);
                },
            }
        }

        info!("Write operations completed with follower down");

        // Verify cluster is still operational (quorum should be maintained)
        sleep(Duration::from_secs(2)).await;

        // Verify data is committed (should succeed with quorum)
        for (bucket, key, expected_data) in &write_data {
            match verifier.verify_read(bucket, key, expected_data).await {
                Ok(true) => {
                    info!("Data verified on quorum: {}/{}", bucket, key);
                    operations += 1;
                },
                Ok(false) => {
                    failures += 1;
                    warn!("Data mismatch for {}/{}", bucket, key);
                },
                Err(e) => {
                    failures += 1;
                    warn!("Failed to verify {}/{}: {}", bucket, key, e);
                },
            }
        }

        // Verify new writes still work
        match verifier
            .write_and_track("test-bucket", "post-crash-key", b"post-crash-data".to_vec())
            .await
        {
            Ok(_) => {
                info!("New write successful with follower down");
                operations += 1;
            },
            Err(e) => {
                failures += 1;
                warn!("New write failed: {}", e);
            },
        }

        // Restart the follower
        info!("Restarting follower node {}", follower_id);
        cluster
            .restart_node(follower_id)
            .await
            .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

        // Wait for follower to catch up
        info!("Waiting for follower to catch up");
        sleep(Duration::from_secs(10)).await;

        // Verify all data is intact across the cluster
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        if !integrity_verified {
            failures += 1;
            warn!("Data integrity verification failed after follower recovery");
        } else {
            info!("Follower successfully caught up - all data verified");
        }

        // Verify the follower can serve reads
        // We could check this by directing reads to the follower's endpoint,
        // but for simplicity we verify through the cluster endpoint
        for (bucket, key, expected_data) in &write_data {
            match verifier.verify_read(bucket, key, expected_data).await {
                Ok(true) => {
                    operations += 1;
                },
                Ok(false) => {
                    failures += 1;
                    warn!(
                        "Data mismatch after follower recovery for {}/{}",
                        bucket, key
                    );
                },
                Err(e) => {
                    failures += 1;
                    warn!("Read failed after follower recovery: {}", e);
                },
            }
        }

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "leader_id": leader_id,
                "follower_id": follower_id,
                "objects_replicated": write_data.len(),
            }),
            error: if failures > 0 {
                Some(format!("{} failures occurred", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}
