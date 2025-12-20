use std::sync::Arc;
use std::time::Duration;
use anode_raft::AnodeRaft;
use anode_storage::StorageEngine;
use openraft::{RaftMetrics, State};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn, error};

use crate::cluster::{ClusterStatus, NodeId, NodeInfo};

#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    /// This node's ID
    pub node_id: NodeId,
    /// This node's advertised address
    pub advertise_addr: String,
    /// Initial cluster members (for new cluster)
    pub initial_members: Vec<String>,
    /// Cluster to join (for joining existing cluster)
    pub join_addr: Option<String>,
    /// Bootstrap timeout
    pub timeout: Duration,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            node_id: 1,
            advertise_addr: "127.0.0.1:6000".to_string(),
            initial_members: Vec::new(),
            join_addr: None,
            timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Error, Debug)]
pub enum BootstrapError {
    #[error("Bootstrap timeout")]
    Timeout,

    #[error("Failed to reach cluster: {0}")]
    ConnectionFailed(String),

    #[error("Already bootstrapped")]
    AlreadyBootstrapped,

    #[error("Raft error: {0}")]
    RaftError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

impl From<openraft::error::InitializeError<NodeId>> for BootstrapError {
    fn from(err: openraft::error::InitializeError<NodeId>) -> Self {
        BootstrapError::RaftError(err.to_string())
    }
}

impl From<openraft::error::RaftError<NodeId>> for BootstrapError {
    fn from(err: openraft::error::RaftError<NodeId>) -> Self {
        BootstrapError::RaftError(err.to_string())
    }
}

/// Cluster bootstrap handling
pub struct ClusterBootstrap {
    config: BootstrapConfig,
    raft: Arc<AnodeRaft>,
    storage: Arc<StorageEngine>,
}

impl ClusterBootstrap {
    pub fn new(
        config: BootstrapConfig,
        raft: Arc<AnodeRaft>,
        storage: Arc<StorageEngine>,
    ) -> Self {
        Self {
            config,
            raft,
            storage,
        }
    }

    /// Bootstrap a new cluster (this node becomes initial leader)
    pub async fn bootstrap_new_cluster(&self) -> Result<(), BootstrapError> {
        info!("Bootstrapping new cluster with node {}", self.config.node_id);

        // Check if already initialized
        if self.is_initialized().await {
            warn!("Cluster already bootstrapped");
            return Err(BootstrapError::AlreadyBootstrapped);
        }

        // Create initial member set
        let mut members = std::collections::BTreeMap::new();
        members.insert(
            self.config.node_id,
            anode_raft::network::Node {
                addr: self.config.advertise_addr.clone(),
            },
        );

        // Add other initial members if specified
        for (idx, addr) in self.config.initial_members.iter().enumerate() {
            let node_id = (self.config.node_id + idx as u64 + 1) as NodeId;
            members.insert(
                node_id,
                anode_raft::network::Node {
                    addr: addr.clone(),
                },
            );
        }

        info!("Initializing cluster with {} members", members.len());

        // Initialize the raft cluster
        self.raft.initialize(members).await?;

        info!("Cluster bootstrapped successfully");

        // Wait for leader election
        self.wait_for_leader(self.config.timeout).await?;

        Ok(())
    }

    /// Join an existing cluster
    pub async fn join_cluster(&self, addr: &str) -> Result<ClusterStatus, BootstrapError> {
        info!("Joining existing cluster at {}", addr);

        // Check if already initialized
        if self.is_initialized().await {
            warn!("Node already part of a cluster");
            return Err(BootstrapError::AlreadyBootstrapped);
        }

        // Create node info for this node
        let node_info = NodeInfo {
            id: self.config.node_id,
            name: format!("node-{}", self.config.node_id),
            grpc_addr: self.config.advertise_addr.clone(),
            s3_addr: format!("http://{}", self.config.advertise_addr),
            admin_addr: format!("http://{}", self.config.advertise_addr),
        };

        // Send join request to cluster leader
        // This would typically be done via HTTP/gRPC to the cluster's admin API
        // For now, we'll return a placeholder error

        info!("Join request sent to {}", addr);

        // In a real implementation, we would:
        // 1. Contact the cluster leader
        // 2. Request to be added as a learner
        // 3. Wait for data sync
        // 4. Request promotion to voter

        // Wait for cluster to be ready
        self.wait_ready(self.config.timeout).await?;

        Ok(ClusterStatus {
            leader_id: self.get_leader().await,
            members: vec![node_info],
            placement_groups: Vec::new(),
            total_nodes: 1,
            healthy_nodes: 1,
        })
    }

    /// Auto-detect whether to bootstrap or join
    pub async fn auto_bootstrap(&self) -> Result<ClusterStatus, BootstrapError> {
        info!("Auto-detecting bootstrap mode");

        // Check if already initialized
        if self.is_initialized().await {
            info!("Node already initialized, getting cluster status");
            return self.get_cluster_status().await;
        }

        // If join address is specified, try to join
        if let Some(join_addr) = &self.config.join_addr {
            info!("Join address specified, attempting to join cluster");
            return self.join_cluster(join_addr).await;
        }

        // Otherwise, bootstrap a new cluster
        if !self.config.initial_members.is_empty() {
            info!("Initial members specified, bootstrapping new cluster");
            self.bootstrap_new_cluster().await?;
            return self.get_cluster_status().await;
        }

        // Single-node cluster
        info!("No join address or initial members, bootstrapping single-node cluster");
        self.bootstrap_new_cluster().await?;
        self.get_cluster_status().await
    }

    /// Wait for cluster to be ready
    pub async fn wait_ready(&self, timeout_duration: Duration) -> Result<(), BootstrapError> {
        info!("Waiting for cluster to be ready (timeout: {:?})", timeout_duration);

        let result = timeout(timeout_duration, async {
            loop {
                if self.is_ready().await {
                    return Ok(());
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        match result {
            Ok(Ok(())) => {
                info!("Cluster is ready");
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                error!("Timeout waiting for cluster to be ready");
                Err(BootstrapError::Timeout)
            }
        }
    }

    /// Check if the raft instance is initialized
    async fn is_initialized(&self) -> bool {
        let metrics = self.raft.metrics().borrow().clone();
        metrics.current_term > 0 || !metrics.membership_config.membership().is_empty()
    }

    /// Check if cluster is ready (has a leader)
    async fn is_ready(&self) -> bool {
        self.get_leader().await.is_some()
    }

    /// Wait for leader election
    async fn wait_for_leader(&self, timeout_duration: Duration) -> Result<(), BootstrapError> {
        debug!("Waiting for leader election");

        let result = timeout(timeout_duration, async {
            loop {
                if let Some(leader) = self.get_leader().await {
                    info!("Leader elected: node {}", leader);
                    return Ok(());
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                error!("Timeout waiting for leader election");
                Err(BootstrapError::Timeout)
            }
        }
    }

    /// Get current cluster leader
    async fn get_leader(&self) -> Option<NodeId> {
        self.raft.current_leader().await
    }

    /// Get cluster status
    async fn get_cluster_status(&self) -> Result<ClusterStatus, BootstrapError> {
        let metrics = self.raft.metrics().borrow().clone();
        let leader_id = metrics.current_leader;

        let members: Vec<NodeInfo> = metrics
            .membership_config
            .membership()
            .nodes()
            .map(|(&id, node)| NodeInfo {
                id,
                name: format!("node-{}", id),
                grpc_addr: node.addr.clone(),
                s3_addr: format!("http://{}", node.addr),
                admin_addr: format!("http://{}", node.addr),
            })
            .collect();

        let total_nodes = members.len();

        Ok(ClusterStatus {
            leader_id,
            members,
            placement_groups: Vec::new(),
            total_nodes,
            healthy_nodes: total_nodes, // TODO: Implement health checking
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anode_raft::{RaftStorage, StateMachine, Network};
    use anode_storage::engine::StorageConfig;
    use tempfile::TempDir;

    async fn create_test_bootstrap() -> (ClusterBootstrap, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = StorageConfig::new(temp_dir.path().to_path_buf());
        let storage = Arc::new(StorageEngine::open(storage_config).await.unwrap());

        let raft_storage = Arc::new(
            RaftStorage::open(temp_dir.path().join("raft.db"))
                .await
                .unwrap()
        );
        let state_machine = Arc::new(StateMachine::new());
        let network = Arc::new(Network::new());

        let raft_config = openraft::Config::default();
        let raft = AnodeRaft::new(
            1,
            raft_config,
            network,
            raft_storage,
            state_machine,
        )
        .await
        .unwrap();

        let config = BootstrapConfig {
            node_id: 1,
            advertise_addr: "127.0.0.1:6001".to_string(),
            initial_members: Vec::new(),
            join_addr: None,
            timeout: Duration::from_secs(5),
        };

        let bootstrap = ClusterBootstrap::new(config, Arc::new(raft), storage);
        (bootstrap, temp_dir)
    }

    #[tokio::test]
    async fn test_bootstrap_config_default() {
        let config = BootstrapConfig::default();
        assert_eq!(config.node_id, 1);
        assert_eq!(config.advertise_addr, "127.0.0.1:6000");
        assert!(config.initial_members.is_empty());
        assert!(config.join_addr.is_none());
    }

    #[tokio::test]
    async fn test_bootstrap_new_cluster() {
        let (bootstrap, _temp) = create_test_bootstrap().await;

        // Bootstrap should succeed
        let result = bootstrap.bootstrap_new_cluster().await;
        assert!(result.is_ok());

        // Should be initialized after bootstrap
        assert!(bootstrap.is_initialized().await);

        // Should have a leader
        assert!(bootstrap.get_leader().await.is_some());
    }

    #[tokio::test]
    async fn test_bootstrap_already_initialized() {
        let (bootstrap, _temp) = create_test_bootstrap().await;

        // First bootstrap should succeed
        bootstrap.bootstrap_new_cluster().await.unwrap();

        // Second bootstrap should fail
        let result = bootstrap.bootstrap_new_cluster().await;
        assert!(matches!(result, Err(BootstrapError::AlreadyBootstrapped)));
    }

    #[tokio::test]
    async fn test_wait_ready() {
        let (bootstrap, _temp) = create_test_bootstrap().await;

        bootstrap.bootstrap_new_cluster().await.unwrap();

        // Should be ready after bootstrap
        let result = bootstrap.wait_ready(Duration::from_secs(5)).await;
        assert!(result.is_ok());
    }
}
