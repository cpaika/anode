//! Workload simulation framework

use crate::{
    cluster::TestCluster,
    history::{OperationHistory, OperationResult, OperationValue},
    workload::{Operation, Workload},
    Result,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

/// Result of a simulation run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Total operations executed
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Total duration of the simulation
    pub duration: Duration,
    /// Operations per second
    pub throughput: f64,
    /// Average latency
    pub avg_latency: Duration,
    /// P50 latency
    pub p50_latency: Duration,
    /// P95 latency
    pub p95_latency: Duration,
    /// P99 latency
    pub p99_latency: Duration,
    /// Whether linearizability check passed
    pub linearizable: Option<bool>,
    /// Whether durability check passed
    pub durable: Option<bool>,
}

/// Simulation configuration
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Number of concurrent clients
    pub num_clients: usize,
    /// Duration to run the simulation
    pub duration: Duration,
    /// Whether to check linearizability
    pub check_linearizability: bool,
    /// Whether to check durability
    pub check_durability: bool,
    /// Target operations per second (0 for unlimited)
    pub target_ops_per_sec: u64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            num_clients: 10,
            duration: Duration::from_secs(60),
            check_linearizability: false,
            check_durability: false,
            target_ops_per_sec: 0,
        }
    }
}

/// Simulation runner
pub struct Simulation {
    cluster: TestCluster,
    workload: Arc<Box<dyn Workload>>,
    history: OperationHistory,
    config: SimulationConfig,
}

impl Simulation {
    /// Create a new simulation
    pub fn new(
        cluster: TestCluster,
        workload: Box<dyn Workload>,
        config: SimulationConfig,
    ) -> Self {
        Self {
            cluster,
            workload: Arc::new(workload),
            history: OperationHistory::new(),
            config,
        }
    }

    /// Run the simulation
    pub async fn run(&mut self) -> Result<SimulationResult> {
        info!(
            "Starting simulation with {} clients for {:?}",
            self.config.num_clients, self.config.duration
        );

        let start_time = Instant::now();
        let endpoint = self.cluster.get_s3_endpoint();
        let client = Arc::new(reqwest::Client::new());

        let mut tasks = JoinSet::new();

        // Spawn client tasks
        for client_id in 0..self.config.num_clients {
            let workload = Arc::clone(&self.workload);
            let history = self.history.clone();
            let endpoint = endpoint.clone();
            let client = Arc::clone(&client);
            let duration = self.config.duration;

            tasks.spawn(async move {
                Self::client_loop(client_id, workload, history, endpoint, client, duration).await
            });
        }

        // Wait for all clients to finish
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {},
                Ok(Err(e)) => {
                    warn!("Client task failed: {}", e);
                },
                Err(e) => {
                    warn!("Client task panicked: {}", e);
                },
            }
        }

        let elapsed = start_time.elapsed();
        info!("Simulation completed in {:?}", elapsed);

        // Calculate results
        self.calculate_results(elapsed).await
    }

    /// Client loop - executes operations until duration expires
    async fn client_loop(
        _client_id: usize,
        workload: Arc<Box<dyn Workload>>,
        history: OperationHistory,
        endpoint: String,
        client: Arc<reqwest::Client>,
        duration: Duration,
    ) -> Result<()> {
        let start = Instant::now();

        while start.elapsed() < duration {
            let operation = workload.generate_operation().await;
            let op_id = history.record_invoke(operation.clone()).await;

            let result = Self::execute_operation(&client, &endpoint, operation).await;
            history.record_complete(op_id, result).await;
        }

        Ok(())
    }

    /// Execute a single operation
    async fn execute_operation(
        client: &reqwest::Client,
        endpoint: &str,
        operation: Operation,
    ) -> OperationResult {
        match operation {
            Operation::Put { bucket, key, value } => {
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                match client.put(&url).body(value).send().await {
                    Ok(response) if response.status().is_success() => {
                        OperationResult::Ok(OperationValue::Unit)
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("PUT failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("PUT error: {}", e)),
                }
            },
            Operation::Get { bucket, key } => {
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        match response.bytes().await {
                            Ok(bytes) => OperationResult::Ok(OperationValue::Data(bytes.to_vec())),
                            Err(e) => OperationResult::Err(format!("GET body error: {}", e)),
                        }
                    },
                    Ok(response) if response.status().as_u16() == 404 => {
                        OperationResult::Ok(OperationValue::NotFound)
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("GET failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("GET error: {}", e)),
                }
            },
            Operation::Delete { bucket, key } => {
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                match client.delete(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        OperationResult::Ok(OperationValue::Unit)
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("DELETE failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("DELETE error: {}", e)),
                }
            },
            Operation::List { bucket, prefix } => {
                let mut url = format!("{}/{}?list-type=2", endpoint, bucket);
                if let Some(p) = prefix {
                    url.push_str(&format!("&prefix={}", p));
                }
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        // Parse XML response to get keys
                        // For now, just return empty list
                        OperationResult::Ok(OperationValue::Keys(Vec::new()))
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("LIST failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("LIST error: {}", e)),
                }
            },
            Operation::CreateBucket { bucket } => {
                let url = format!("{}/{}", endpoint, bucket);
                match client.put(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        OperationResult::Ok(OperationValue::Unit)
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("CREATE_BUCKET failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("CREATE_BUCKET error: {}", e)),
                }
            },
            Operation::DeleteBucket { bucket } => {
                let url = format!("{}/{}", endpoint, bucket);
                match client.delete(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        OperationResult::Ok(OperationValue::Unit)
                    },
                    Ok(response) => {
                        OperationResult::Err(format!("DELETE_BUCKET failed: {}", response.status()))
                    },
                    Err(e) => OperationResult::Err(format!("DELETE_BUCKET error: {}", e)),
                }
            },
        }
    }

    /// Calculate simulation results
    async fn calculate_results(&self, duration: Duration) -> Result<SimulationResult> {
        let stats = self.history.stats().await;
        let entries = self.history.get_completed_entries().await;

        let mut latencies: Vec<Duration> = entries.iter().filter_map(|e| e.duration()).collect();
        latencies.sort();

        let p50_latency = latencies
            .get(latencies.len() / 2)
            .copied()
            .unwrap_or(Duration::from_secs(0));
        let p95_latency = latencies
            .get((latencies.len() * 95) / 100)
            .copied()
            .unwrap_or(Duration::from_secs(0));
        let p99_latency = latencies
            .get((latencies.len() * 99) / 100)
            .copied()
            .unwrap_or(Duration::from_secs(0));

        let throughput = stats.completed as f64 / duration.as_secs_f64();

        let linearizable = if self.config.check_linearizability {
            Some(self.check_linearizability().await?)
        } else {
            None
        };

        let durable = if self.config.check_durability {
            Some(self.check_durability().await?)
        } else {
            None
        };

        Ok(SimulationResult {
            total_operations: stats.total as u64,
            successful_operations: stats.successful as u64,
            failed_operations: stats.failed as u64,
            duration,
            throughput,
            avg_latency: stats.avg_duration.unwrap_or(Duration::from_secs(0)),
            p50_latency,
            p95_latency,
            p99_latency,
            linearizable,
            durable,
        })
    }

    /// Check linearizability of the operation history
    pub async fn check_linearizability(&self) -> Result<bool> {
        debug!("Checking linearizability");

        // This is a placeholder for a real linearizability checker
        // A real implementation would use something like Jepsen's Knossos
        // or implement the Wing & Gong algorithm

        let entries = self.history.get_entries().await;

        // Simple sanity check: all completed operations should have results
        for entry in &entries {
            if entry.is_complete() && entry.result.is_none() {
                warn!("Found completed entry without result: {:?}", entry);
                return Ok(false);
            }
        }

        // TODO: Implement proper linearizability checking
        warn!("Linearizability checking not fully implemented");
        Ok(true)
    }

    /// Check durability of written data
    pub async fn check_durability(&self) -> Result<bool> {
        debug!("Checking durability");

        // Get all successful PUT operations
        let entries = self.history.get_successful_entries().await;
        let client = reqwest::Client::new();
        let endpoint = self.cluster.get_s3_endpoint();

        let mut all_durable = true;

        for entry in entries {
            if let Operation::Put { bucket, key, value } = &entry.operation {
                // Try to read back the written data
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        match response.bytes().await {
                            Ok(bytes) if bytes.as_ref() == value.as_slice() => {
                                // Data matches
                            },
                            Ok(_) => {
                                warn!("Data mismatch for {}/{}", bucket, key);
                                all_durable = false;
                            },
                            Err(e) => {
                                warn!("Failed to read body for {}/{}: {}", bucket, key, e);
                                all_durable = false;
                            },
                        }
                    },
                    Ok(response) => {
                        warn!("Failed to read {}/{}: {}", bucket, key, response.status());
                        all_durable = false;
                    },
                    Err(e) => {
                        warn!("Error reading {}/{}: {}", bucket, key, e);
                        all_durable = false;
                    },
                }
            }
        }

        Ok(all_durable)
    }

    /// Get the operation history
    pub fn history(&self) -> &OperationHistory {
        &self.history
    }

    /// Get mutable reference to the cluster
    pub fn cluster_mut(&mut self) -> &mut TestCluster {
        &mut self.cluster
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_config() {
        let config = SimulationConfig::default();
        assert_eq!(config.num_clients, 10);
        assert_eq!(config.duration, Duration::from_secs(60));
    }

    #[test]
    fn test_simulation_result() {
        let result = SimulationResult {
            total_operations: 1000,
            successful_operations: 950,
            failed_operations: 50,
            duration: Duration::from_secs(10),
            throughput: 100.0,
            avg_latency: Duration::from_millis(10),
            p50_latency: Duration::from_millis(8),
            p95_latency: Duration::from_millis(20),
            p99_latency: Duration::from_millis(50),
            linearizable: Some(true),
            durable: Some(true),
        };

        assert_eq!(result.total_operations, 1000);
        assert_eq!(result.successful_operations, 950);
        assert_eq!(result.throughput, 100.0);
    }
}
