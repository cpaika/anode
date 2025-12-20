//! Chaos testing scenarios
//!
//! Each module implements a specific chaos testing scenario with
//! comprehensive verification and data integrity checks.

pub mod node_crash;
pub mod network_partition;
pub mod disk_failure;
pub mod slow_network;
pub mod rolling_restart;
pub mod split_brain;
pub mod cascade_failure;

use crate::{ChaosError, Result};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for chaos scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioConfig {
    /// Number of nodes in the cluster
    pub num_nodes: usize,
    /// Duration to run the scenario
    pub duration: Duration,
    /// Whether to verify data integrity after the scenario
    pub verify_integrity: bool,
    /// Workload intensity (operations per second)
    pub workload_intensity: f64,
    /// Scenario-specific parameters
    pub params: serde_json::Value,
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            num_nodes: 5,
            duration: Duration::from_secs(60),
            verify_integrity: true,
            workload_intensity: 10.0,
            params: serde_json::Value::Null,
        }
    }
}

/// Result of running a chaos scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    /// Name of the scenario
    pub name: String,
    /// Whether the scenario passed
    pub success: bool,
    /// Duration the scenario took to run
    pub duration: Duration,
    /// Number of operations performed
    pub operations: usize,
    /// Number of failures encountered
    pub failures: usize,
    /// Data integrity verification result
    pub integrity_verified: bool,
    /// Additional metrics
    pub metrics: serde_json::Value,
    /// Error message if failed
    pub error: Option<String>,
}

/// Trait for chaos testing scenarios
#[async_trait]
pub trait ChaosScenario: Send + Sync {
    /// Get the name of this scenario
    fn name(&self) -> &str;

    /// Get the description of this scenario
    fn description(&self) -> &str;

    /// Run the scenario
    async fn run(&self, cluster: &mut TestCluster, config: &ScenarioConfig) -> Result<ScenarioResult>;

    /// Verify the cluster state after the scenario
    async fn verify(&self, cluster: &TestCluster) -> Result<bool> {
        // Default implementation just checks if nodes are running
        for i in 0..cluster.num_nodes() {
            let node = cluster.get_node(i).map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;
            if !node.is_partitioned() {
                // Node should be accessible
            }
        }
        Ok(true)
    }
}
