use anode_protocol::ChunkId;
use anode_storage::StorageEngine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

use super::membership::MembershipManager;
use crate::cluster::NodeId;

#[derive(Error, Debug)]
pub enum RebalanceError {
    #[error("Membership error: {0}")]
    MembershipError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Transfer failed: {0}")]
    TransferFailed(String),

    #[error("Planning failed: {0}")]
    PlanningFailed(String),

    #[error("Not enough nodes for rebalancing")]
    InsufficientNodes,

    #[error("Rebalance already in progress")]
    AlreadyInProgress,

    #[error("No rebalance needed")]
    NoRebalanceNeeded,
}

impl From<anode_storage::error::StorageError> for RebalanceError {
    fn from(err: anode_storage::error::StorageError) -> Self {
        RebalanceError::StorageError(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct RebalanceConfig {
    /// Max concurrent transfers
    pub max_concurrent_transfers: usize,
    /// Transfer rate limit (bytes/sec)
    pub rate_limit_bytes: u64,
    /// Whether to run rebalance automatically
    pub auto_rebalance: bool,
    /// Target balance threshold (percentage)
    pub balance_threshold: f64,
}

impl Default for RebalanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 4,
            rate_limit_bytes: 100 * 1024 * 1024, // 100 MB/s
            auto_rebalance: false,
            balance_threshold: 0.1, // 10% imbalance threshold
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalancePlan {
    pub transfers: Vec<ChunkTransfer>,
    pub estimated_bytes: u64,
    pub estimated_duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkTransfer {
    pub chunk_id: ChunkId,
    pub from: NodeId,
    pub to: NodeId,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RebalanceResult {
    pub chunks_transferred: u64,
    pub bytes_transferred: u64,
    pub duration: Duration,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RebalanceStatus {
    Idle,
    Planning,
    InProgress {
        total_transfers: usize,
        completed_transfers: usize,
        bytes_transferred: u64,
    },
    Completed {
        result: RebalanceResult,
    },
    Failed {
        error: String,
    },
}

/// Data rebalancing when cluster membership changes
pub struct Rebalancer {
    membership: Arc<MembershipManager>,
    storage: Arc<StorageEngine>,
    config: RebalanceConfig,
    status: RwLock<RebalanceStatus>,
}

impl Rebalancer {
    pub fn new(
        membership: Arc<MembershipManager>,
        storage: Arc<StorageEngine>,
        config: RebalanceConfig,
    ) -> Self {
        Self {
            membership,
            storage,
            config,
            status: RwLock::new(RebalanceStatus::Idle),
        }
    }

    /// Calculate rebalance plan
    pub async fn plan(&self) -> Result<RebalancePlan, RebalanceError> {
        info!("Calculating rebalance plan");

        {
            let mut status = self.status.write().await;
            *status = RebalanceStatus::Planning;
        }

        let members = self.membership.get_members().await;

        if members.len() < 2 {
            warn!("Not enough nodes for rebalancing");
            return Err(RebalanceError::InsufficientNodes);
        }

        // Get storage stats
        let stats = self.storage.get_stats().await?;

        // Calculate chunk distribution
        let chunks_per_node = stats.chunk_count / members.len() as u64;
        let target_variance = (chunks_per_node as f64 * self.config.balance_threshold) as u64;

        debug!(
            "Target chunks per node: {} (variance: {})",
            chunks_per_node, target_variance
        );

        // In a real implementation, we would:
        // 1. Query each node for their current chunk counts
        // 2. Identify overloaded and underloaded nodes
        // 3. Create transfer plan to balance the load
        // 4. Consider chunk size, not just count
        // 5. Respect placement group constraints

        // For now, return an empty plan
        let plan = RebalancePlan {
            transfers: Vec::new(),
            estimated_bytes: 0,
            estimated_duration: Duration::from_secs(0),
        };

        // Check if rebalance is needed
        if plan.transfers.is_empty() {
            info!("No rebalance needed - cluster is balanced");
            let mut status = self.status.write().await;
            *status = RebalanceStatus::Idle;
            return Err(RebalanceError::NoRebalanceNeeded);
        }

        // Estimate duration based on rate limit
        let estimated_duration = if self.config.rate_limit_bytes > 0 {
            Duration::from_secs(plan.estimated_bytes / self.config.rate_limit_bytes)
        } else {
            Duration::from_secs(plan.estimated_bytes / (100 * 1024 * 1024)) // Default 100 MB/s
        };

        let plan = RebalancePlan {
            transfers: plan.transfers,
            estimated_bytes: plan.estimated_bytes,
            estimated_duration,
        };

        info!(
            "Rebalance plan created: {} transfers, {} bytes, estimated {:?}",
            plan.transfers.len(),
            plan.estimated_bytes,
            plan.estimated_duration
        );

        Ok(plan)
    }

    /// Execute rebalance plan
    pub async fn execute(&self, plan: &RebalancePlan) -> Result<RebalanceResult, RebalanceError> {
        info!(
            "Executing rebalance plan with {} transfers",
            plan.transfers.len()
        );

        // Check if rebalance already in progress
        {
            let status = self.status.read().await;
            if matches!(*status, RebalanceStatus::InProgress { .. }) {
                return Err(RebalanceError::AlreadyInProgress);
            }
        }

        let start_time = std::time::Instant::now();

        // Update status
        {
            let mut status = self.status.write().await;
            *status = RebalanceStatus::InProgress {
                total_transfers: plan.transfers.len(),
                completed_transfers: 0,
                bytes_transferred: 0,
            };
        }

        // Create semaphore for concurrency control
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_transfers));

        let mut handles = Vec::new();
        let mut errors = Vec::new();
        let mut chunks_transferred = 0u64;
        let mut bytes_transferred = 0u64;

        // Execute transfers
        for transfer in &plan.transfers {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let transfer = transfer.clone();

            let handle = tokio::spawn(async move {
                let result = Self::execute_transfer(transfer.clone()).await;
                drop(permit);
                result
            });

            handles.push(handle);
        }

        // Wait for all transfers to complete
        for handle in handles {
            match handle.await {
                Ok(Ok(size)) => {
                    chunks_transferred += 1;
                    bytes_transferred += size;

                    // Update status
                    let mut status = self.status.write().await;
                    if let RebalanceStatus::InProgress {
                        total_transfers,
                        completed_transfers,
                        bytes_transferred: ref mut bytes,
                    } = &mut *status
                    {
                        *completed_transfers += 1;
                        *bytes += size;
                    }
                },
                Ok(Err(e)) => {
                    warn!("Transfer failed: {}", e);
                    errors.push(e.to_string());
                },
                Err(e) => {
                    warn!("Transfer task failed: {}", e);
                    errors.push(e.to_string());
                },
            }
        }

        let duration = start_time.elapsed();

        let result = RebalanceResult {
            chunks_transferred,
            bytes_transferred,
            duration,
            errors: errors.clone(),
        };

        // Update final status
        {
            let mut status = self.status.write().await;
            if errors.is_empty() {
                *status = RebalanceStatus::Completed {
                    result: result.clone(),
                };
            } else {
                *status = RebalanceStatus::Failed {
                    error: format!("{} transfers failed", errors.len()),
                };
            }
        }

        info!(
            "Rebalance complete: {} chunks transferred, {} bytes, {:?}",
            chunks_transferred, bytes_transferred, duration
        );

        Ok(result)
    }

    /// Execute a single chunk transfer
    async fn execute_transfer(transfer: ChunkTransfer) -> Result<u64, RebalanceError> {
        debug!(
            "Transferring chunk {} from node {} to node {}",
            transfer.chunk_id, transfer.from, transfer.to
        );

        // In a real implementation, this would:
        // 1. Read the chunk from the source node
        // 2. Send it to the destination node
        // 3. Verify the transfer
        // 4. Update metadata
        // 5. Optionally delete from source (if not needed for replication)

        // For now, just simulate a transfer
        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(transfer.size)
    }

    /// Get current rebalance status
    pub async fn status(&self) -> RebalanceStatus {
        self.status.read().await.clone()
    }

    /// Calculate cluster balance score (0.0 = perfectly balanced, 1.0 = very imbalanced)
    pub async fn calculate_balance_score(&self) -> Result<f64, RebalanceError> {
        let members = self.membership.get_members().await;

        if members.is_empty() {
            return Ok(0.0);
        }

        if members.len() == 1 {
            return Ok(0.0); // Single node is always balanced
        }

        // In a real implementation, we would query each node for chunk counts
        // and calculate variance
        // For now, return a placeholder

        Ok(0.0)
    }

    /// Check if rebalance is needed based on balance threshold
    pub async fn is_rebalance_needed(&self) -> Result<bool, RebalanceError> {
        let score = self.calculate_balance_score().await?;
        Ok(score > self.config.balance_threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::super::discovery::Discovery;
    use super::*;
    use anode_raft::create_raft_node;
    use anode_storage::StorageConfig;
    use tempfile::TempDir;

    async fn create_test_rebalancer() -> (Rebalancer, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create storage
        let storage_config = StorageConfig {
            data_dir: temp_dir.path().join("blobs"),
            metadata_dir: temp_dir.path().join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: false,
        };
        let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());

        // Create raft
        let raft = Arc::new(
            create_raft_node(1, "test-cluster", temp_dir.path().join("raft"), storage.clone())
                .await
                .unwrap(),
        );

        // Create membership
        let discovery = Arc::new(Discovery::new());
        let membership = Arc::new(MembershipManager::new(raft, discovery));

        let config = RebalanceConfig::default();
        let rebalancer = Rebalancer::new(membership, storage, config);

        (rebalancer, temp_dir)
    }

    #[tokio::test]
    async fn test_rebalance_config_default() {
        let config = RebalanceConfig::default();
        assert_eq!(config.max_concurrent_transfers, 4);
        assert_eq!(config.rate_limit_bytes, 100 * 1024 * 1024);
        assert!(!config.auto_rebalance);
        assert_eq!(config.balance_threshold, 0.1);
    }

    #[tokio::test]
    async fn test_rebalancer_initial_status() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let status = rebalancer.status().await;
        assert_eq!(status, RebalanceStatus::Idle);
    }

    #[tokio::test]
    async fn test_plan_insufficient_nodes() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        // With only one node (or none), planning should fail
        let result = rebalancer.plan().await;
        assert!(matches!(result, Err(RebalanceError::InsufficientNodes)));
    }

    #[tokio::test]
    async fn test_calculate_balance_score_single_node() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let score = rebalancer.calculate_balance_score().await.unwrap();
        assert_eq!(score, 0.0); // Single node is perfectly balanced
    }

    #[tokio::test]
    async fn test_is_rebalance_needed() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let needed = rebalancer.is_rebalance_needed().await.unwrap();
        assert!(!needed); // With default setup, no rebalance needed
    }

    #[tokio::test]
    async fn test_execute_empty_plan() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let plan = RebalancePlan {
            transfers: Vec::new(),
            estimated_bytes: 0,
            estimated_duration: Duration::from_secs(0),
        };

        let result = rebalancer.execute(&plan).await.unwrap();
        assert_eq!(result.chunks_transferred, 0);
        assert_eq!(result.bytes_transferred, 0);
    }

    #[tokio::test]
    async fn test_execute_plan_with_transfers() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let mut chunk1_bytes = [0u8; 32];
        chunk1_bytes[..6].copy_from_slice(b"chunk1");
        let mut chunk2_bytes = [0u8; 32];
        chunk2_bytes[..6].copy_from_slice(b"chunk2");

        let transfers = vec![
            ChunkTransfer {
                chunk_id: ChunkId(chunk1_bytes),
                from: 1,
                to: 2,
                size: 1024,
            },
            ChunkTransfer {
                chunk_id: ChunkId(chunk2_bytes),
                from: 1,
                to: 2,
                size: 2048,
            },
        ];

        let plan = RebalancePlan {
            transfers,
            estimated_bytes: 3072,
            estimated_duration: Duration::from_secs(1),
        };

        let result = rebalancer.execute(&plan).await.unwrap();
        assert_eq!(result.chunks_transferred, 2);
        assert_eq!(result.bytes_transferred, 3072);
    }

    #[tokio::test]
    async fn test_status_during_execution() {
        let (rebalancer, _temp) = create_test_rebalancer().await;

        let mut chunk1_bytes = [0u8; 32];
        chunk1_bytes[..6].copy_from_slice(b"chunk1");

        let transfers = vec![ChunkTransfer {
            chunk_id: ChunkId(chunk1_bytes),
            from: 1,
            to: 2,
            size: 1024,
        }];

        let plan = RebalancePlan {
            transfers,
            estimated_bytes: 1024,
            estimated_duration: Duration::from_secs(1),
        };

        let rebalancer_clone = Arc::new(rebalancer);
        let rebalancer_ref = rebalancer_clone.clone();

        tokio::spawn(async move { rebalancer_ref.execute(&plan).await });

        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(1)).await;

        // Check status - it might be InProgress or Completed depending on timing
        let status = rebalancer_clone.status().await;
        assert!(matches!(
            status,
            RebalanceStatus::InProgress { .. } | RebalanceStatus::Completed { .. }
        ));
    }

    #[test]
    fn test_chunk_transfer_serialization() {
        let mut chunk_bytes = [0u8; 32];
        chunk_bytes[..10].copy_from_slice(b"test-chunk");

        let transfer = ChunkTransfer {
            chunk_id: ChunkId(chunk_bytes),
            from: 1,
            to: 2,
            size: 1024,
        };

        let json = serde_json::to_string(&transfer).unwrap();
        let deserialized: ChunkTransfer = serde_json::from_str(&json).unwrap();

        assert_eq!(transfer.chunk_id, deserialized.chunk_id);
        assert_eq!(transfer.from, deserialized.from);
        assert_eq!(transfer.to, deserialized.to);
        assert_eq!(transfer.size, deserialized.size);
    }
}
